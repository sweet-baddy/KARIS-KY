# [SECURITY] Rate-Limit Append_Attestation_Digest Calls Per Ledger to Prevent Log Spam

## Issue Summary

The `append_attestation_digest()` entrypoint can be called multiple times per ledger without per-ledger rate-limiting, creating a **log spam denial-of-service (DoS) vector**. While the log capacity is bounded at `MAX_ATTESTATION_APPEND_ENTRIES = 32`, an attacker (or malfunctioning off-chain system) calling the function multiple times in the same ledger can cause:

- Rapid log saturation
- Inflated per-ledger gas costs
- Loss of legitimate audit records
- Audit trail manipulation

**Severity:** MEDIUM (requires admin key compromise + intentional attack)  
**Status:** Enhancement / design improvement  
**Affected version:** v7 (current) and all prior versions  
**Component:** `escrow/src/lib.rs::LiquifactEscrow::append_attestation_digest()`

---

## Problem Description

### Current Behavior

The `append_attestation_digest()` function:
- **Requires admin authorization** (correctly gated via `require_auth()`)
- **Checks log capacity** against `MAX_ATTESTATION_APPEND_ENTRIES = 32`
- **Allows unlimited calls per ledger** (only global capacity check, no per-ledger throttling)
- **Stores entry with timestamp** (enables filtering, but does not prevent spam within same ledger)

```rust
pub fn append_attestation_digest(env: Env, digest: BytesN<32>, tag: Symbol) {
    let escrow = Self::load_escrow_require_admin(&env);
    
    // ... read count from storage ...
    let count: u32 = env.storage().instance().get(&DataKey::AttestationAppendLogCount).unwrap_or(0);
    
    // Global capacity check (only constraint today)
    ensure(
        &env,
        count < MAX_ATTESTATION_APPEND_ENTRIES,  // = 32
        EscrowError::AttestationAppendLogCapacityReached,
    );
    
    // No per-ledger rate-limit!
    // Multiple calls in the same ledger all succeed until capacity reached
    
    let idx = count;
    env.storage().instance().set(&DataKey::AttestationLogEntry(idx), &digest);
    env.storage().instance().set(&DataKey::AttestationAppendLogCount, &(count + 1));
    env.storage().instance().set(&DataKey::AttestationTimestamp(idx), &ts);
    // ...
}
```

### Vulnerability Scenario

**Attack flow:**

1. **Admin account** (compromised or malicious) is authenticated
2. Admin calls `append_attestation_digest()` multiple times in the same ledger:
   ```
   Ledger 1000:
   ├─ Call 1: append digest A → log[0] = A, count = 1
   ├─ Call 2: append digest B → log[1] = B, count = 2
   ├─ Call 3: append digest C → log[2] = C, count = 3
   ...
   ├─ Call 31: append digest Z → log[30] = Z, count = 31
   └─ Call 32: append digest X → log[31] = X, count = 32 (FULL)
   ```

3. **Result:** Log is filled to capacity in a **single ledger** with spam entries
4. **Impact:**
   - Legitimate audit records cannot be added until entries are revoked
   - Ledger gas consumption spike (32 storage writes in one block)
   - Off-chain indexers see burst of meaningless entries
   - No way to distinguish legitimate updates from spam (both have `timestamp` in same second)

### Root Cause

**No per-ledger rate-limit** exists. The only guard is:
- **Global capacity** (`MAX_ATTESTATION_APPEND_ENTRIES = 32`)
- **Admin authorization** (auth gate is correct, but doesn't prevent abuse by authenticated admin)
- **No call count tracking per ledger** (missing mechanism)

The design assumes admin is always well-behaved, but:
- Admin key could be compromised
- Off-chain orchestrator could malfunction and retry/loop
- Multi-sig coordination could lead to accidental duplicate submissions
- Future governance changes might increase admin permissions

---

## Steps to Reproduce

### Precondition
- Escrow initialized with admin address
- Attestation log has capacity available (count < 32)

### Reproduction

```rust
#[test]
fn test_append_attestation_spam_fills_log_in_single_ledger() {
    let env = Env::default();
    env.mock_all_auths();
    
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    
    // Spawn 32 attestation appends in the same ledger (same tx block)
    for i in 0u8..32 {
        let digest = BytesN::from_array(&env, &[i; 32]);
        client.append_attestation_digest(&digest, &symbol_short!("spam"));
        // No rate-limiting blocks this
    }
    
    // Verify log is completely filled with spam entries
    let log = client.get_attestation_append_log();
    assert_eq!(log.len(), 32); // All entries are spam, zero space for legitimate records
    
    // Next append fails (log full)
    let digest_33 = BytesN::from_array(&env, &[0xFFu8; 32]);
    let result = client.try_append_attestation_digest(&digest_33, &symbol_short!("fail"));
    assert_eq!(result, Err(Ok(ContractError::from(EscrowError::AttestationAppendLogCapacityReached))));
}
```

### Expected vs. Actual

| Scenario | Expected Behavior | Actual Behavior | Risk |
|----------|-------------------|-----------------|------|
| **Single append per ledger** | ✅ Succeeds; entry recorded | ✅ Works | None |
| **Multiple appends (5/ledger)** | ✅ Succeeds; entries recorded (normal audit trail) | ✅ Works | None |
| **Spam attack (32 appends/ledger)** | ❌ SHOULD rate-limit after N appends | ✅ All succeed (bug) | **HIGH** |
| **Subsequent legitimate appends** | ✅ New entry added | ❌ Fails with "log full" | Data loss |

---

## Proposed Solution

Implement **per-ledger rate-limiting** to bound the number of `append_attestation_digest()` calls allowed per ledger.

### Design

#### 1. Define per-ledger rate-limit constant

```rust
/// Maximum attestation digest appends allowed per ledger.
/// Prevents spam attacks where an admin maliciously or accidentally
/// calls append_attestation_digest multiple times in the same ledger,
/// rapidly saturating the bounded log and wasting ledger gas.
/// 
/// Rationale: 1 append per ledger is typical for legitimate audits;
/// 5 per ledger allows some legitimate batch operations without
/// enabling spam. This is independent of MAX_ATTESTATION_APPEND_ENTRIES (global capacity).
pub const MAX_ATTESTATION_APPENDS_PER_LEDGER: u32 = 5;
```

#### 2. Add storage keys to track per-ledger usage

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// Current ledger sequence when attestation append tracking started.
    /// When a new ledger is reached, the counter resets.
    /// Absent ⇒ no tracking yet; first append sets this.
    AttestationAppendLedger,
    
    /// Number of append_attestation_digest calls in the current ledger.
    /// Incremented on each call; reset to 0 when ledger advances (ledger_seq changes).
    /// Absent ⇒ 0 (no appends yet in this ledger).
    AttestationAppendCountPerLedger,
}
```

#### 3. Implement rate-limit check in `append_attestation_digest()`

```rust
pub fn append_attestation_digest(env: Env, digest: BytesN<32>, tag: Symbol) {
    let escrow = Self::load_escrow_require_admin(&env);
    
    // ... existing code: auto-migrate, global capacity check ...
    
    // NEW: Per-ledger rate-limit check
    let current_ledger = env.ledger().sequence();
    let tracked_ledger: u32 = env.storage().instance()
        .get(&DataKey::AttestationAppendLedger)
        .unwrap_or(0);
    
    // If we've moved to a new ledger, reset the per-ledger counter
    let mut per_ledger_count: u32 = env.storage().instance()
        .get(&DataKey::AttestationAppendCountPerLedger)
        .unwrap_or(0);
    
    if tracked_ledger != current_ledger {
        // New ledger: reset counter and update tracked ledger
        per_ledger_count = 0;
        env.storage().instance().set(&DataKey::AttestationAppendLedger, &current_ledger);
    }
    
    // Check per-ledger rate-limit
    ensure(
        &env,
        per_ledger_count < MAX_ATTESTATION_APPENDS_PER_LEDGER,
        EscrowError::AttestationAppendRateLimitExceeded,  // New error code
    );
    
    // ... existing code: append entry, bump global count ...
    
    // NEW: Increment and store per-ledger counter
    env.storage().instance().set(
        &DataKey::AttestationAppendCountPerLedger,
        &(per_ledger_count + 1)
    );
}
```

#### 4. Add new error code

```rust
pub enum EscrowError {
    // ... existing errors ...
    
    /// Attestation append rate-limit exceeded for this ledger.
    /// The maximum number of `append_attestation_digest()` calls per ledger has been reached.
    /// This limit prevents spam attacks and rapid log saturation.
    /// Rate-limit resets at the next ledger boundary.
    AttestationAppendRateLimitExceeded = 94,
}
```

---

## Acceptance Criteria

- [ ] **AC1:** `DataKey::AttestationAppendLedger` variant added to enum
- [ ] **AC2:** `DataKey::AttestationAppendCountPerLedger` variant added to enum
- [ ] **AC3:** `MAX_ATTESTATION_APPENDS_PER_LEDGER = 5` constant defined
- [ ] **AC4:** `EscrowError::AttestationAppendRateLimitExceeded = 94` error code added
- [ ] **AC5:** `append_attestation_digest()` checks current ledger and resets counter on ledger boundary
- [ ] **AC6:** `append_attestation_digest()` enforces per-ledger limit and fails with code 94 when exceeded
- [ ] **AC7:** Error code 94 documented in `docs/escrow-error-messages.md`
- [ ] **AC8:** `docs/escrow-security-checklist.md` updated with new rate-limit note
- [ ] **AC9:** `docs/dos_analysis.rs` test updated to verify rate-limit enforcement
- [ ] **AC10:** Unit tests added:
  - `test_append_attestation_rate_limit_per_ledger` — verify limit enforced
  - `test_append_attestation_rate_limit_resets_on_ledger_advance` — verify counter resets
  - `test_append_attestation_allows_5_per_ledger_then_blocks` — boundary testing
- [ ] **AC11:** DOS analysis section updated (migration from unbounded to bounded)

---

## Implementation Checklist

- [ ] Add `DataKey` enum variants (2 new entries)
- [ ] Add `MAX_ATTESTATION_APPENDS_PER_LEDGER = 5` constant
- [ ] Add `EscrowError::AttestationAppendRateLimitExceeded = 94` error code
- [ ] Modify `append_attestation_digest()`:
  - Read current ledger sequence
  - Track ledger boundary
  - Reset counter on new ledger
  - Check per-ledger limit
  - Increment and store counter
- [ ] Add 3 unit tests
- [ ] Update error reference documentation (1 file)
- [ ] Update security checklist (1 file)
- [ ] Update DOS analysis tests (1 file)
- [ ] CI verification: cargo build, cargo test, cargo clippy, cargo llvm-cov

---

## Security Considerations

### Threat Model

**Attacker:** Admin account (compromised or malicious) or faulty orchestrator  
**Attack:** Call `append_attestation_digest()` repeatedly in same ledger  
**Impact:** Log spam, gas waste, legitimate record loss  
**Mitigation:** Per-ledger rate-limit (this issue)

### Defense In Depth

| Layer | Current | After Fix |
|-------|---------|-----------|
| **Global capacity** | 32 entries max | Still 32 (unchanged) |
| **Per-ledger rate-limit** | Unbounded | 5 calls per ledger (NEW) |
| **Admin authorization** | ✅ Required | ✅ Still required |

### Interaction with Other Limits

| Limit | Purpose | Value | Relation |
|-------|---------|-------|----------|
| `MAX_ATTESTATION_APPEND_ENTRIES` | Global log capacity | 32 | Upper bound (independent) |
| `MAX_ATTESTATION_APPENDS_PER_LEDGER` | Per-ledger spam prevention | 5 | Rate-limit (new) |
| `MAX_ATTESTATION_APPEND_ENTRIES / MAX_ATTESTATION_APPENDS_PER_LEDGER` | Ledgers to fill log (at max rate) | 32/5 = 6.4 ≈ 7 ledgers | Minimum time to spam |

### Out of Scope (Handled by Soroban Host)

- Transaction signature validation
- Per-transaction gas budgets
- Network-level spam prevention
- Ledger finality guarantees

---

## Testing Strategy

### Unit Tests

#### Test 1: Rate-limit enforced per ledger

```rust
#[test]
fn test_append_attestation_rate_limit_per_ledger() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    
    // First 5 calls in ledger succeed
    for i in 0u8..5 {
        let digest = BytesN::from_array(&env, &[i; 32]);
        client.append_attestation_digest(&digest, &symbol_short!("ok"));
    }
    
    // 6th call fails with rate-limit error
    let digest_6 = BytesN::from_array(&env, &[5u8; 32]);
    let result = client.try_append_attestation_digest(&digest_6, &symbol_short!("fail"));
    assert_eq!(
        result,
        Err(Ok(ContractError::from(EscrowError::AttestationAppendRateLimitExceeded)))
    );
}
```

#### Test 2: Counter resets on ledger boundary

```rust
#[test]
fn test_append_attestation_rate_limit_resets_on_ledger_advance() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    
    // Ledger 1: 5 appends (hit limit)
    for i in 0u8..5 {
        let digest = BytesN::from_array(&env, &[i; 32]);
        client.append_attestation_digest(&digest, &symbol_short!("ledger1"));
    }
    
    // 6th call in ledger 1 fails
    let digest_fail = BytesN::from_array(&env, &[5u8; 32]);
    assert!(client.try_append_attestation_digest(&digest_fail, &symbol_short!("fail")).is_err());
    
    // Advance to ledger 2
    env.ledger().set_sequence(env.ledger().sequence() + 1);
    
    // Now 5 more appends succeed (counter reset)
    for i in 5u8..10 {
        let digest = BytesN::from_array(&env, &[i; 32]);
        client.append_attestation_digest(&digest, &symbol_short!("ledger2"));
    }
    
    // Verify total log has 10 entries (5 from each ledger)
    let log = client.get_attestation_append_log();
    assert_eq!(log.len(), 10);
}
```

#### Test 3: Boundary conditions

```rust
#[test]
fn test_append_attestation_allows_5_per_ledger_then_blocks() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    
    // Exactly 5 should succeed
    for i in 0u8..5 {
        let digest = BytesN::from_array(&env, &[i; 32]);
        client.append_attestation_digest(&digest, &symbol_short!("ok"));
    }
    
    // 6th should fail
    let digest_6 = BytesN::from_array(&env, &[5u8; 32]);
    let result = client.try_append_attestation_digest(&digest_6, &symbol_short!("fail"));
    assert_eq!(result, Err(Ok(ContractError::from(EscrowError::AttestationAppendRateLimitExceeded))));
    
    // Verify log has exactly 5 entries
    let log = client.get_attestation_append_log();
    assert_eq!(log.len(), 5);
}
```

### Verification Checklist

- [ ] Compiles: `cargo build --target wasm32v1-none --release`
- [ ] Tests pass: `cargo test -p karis-ky_escrow`
- [ ] Clippy passes: `cargo clippy -p karis-ky_escrow -- -D warnings`
- [ ] Coverage maintained: `cargo llvm-cov --fail-under-lines 95`
- [ ] Error code 94 added to error reference
- [ ] DOS analysis documentation updated

---

## Deployment Impact

### Before Fix
- **Risk:** Admin can spam log in single ledger
- **Mitigation:** None (design weakness)
- **Operational impact:** Low (requires admin key compromise + intentional abuse)

### After Fix
- **Risk:** Reduced (bounded to 5/ledger)
- **Mitigation:** Per-ledger rate-limit enforced
- **Operational impact:** Minimal
  - Legitimate audits typically use ≤1 append per ledger
  - Batch operations need coordination (spread across ledgers)
  - Error code 94 provides clear feedback

### Backward Compatibility

✅ **Backward compatible**
- New error code is additive (94)
- New storage keys won't conflict
- Existing instances will work fine (keys initialized on first use)
- No redeploy required (WASM upgrade sufficient)

---

## Error Code Reference

### Error 94: AttestationAppendRateLimitExceeded

| Property | Value |
|----------|-------|
| **Code** | 94 |
| **Name** | AttestationAppendRateLimitExceeded |
| **Emitted by** | `append_attestation_digest()` |
| **Condition** | More than `MAX_ATTESTATION_APPENDS_PER_LEDGER = 5` calls in same ledger |
| **Recovery** | Wait for next ledger; retry then (counter resets at ledger boundary) |
| **Semantics** | Rate-limit exceeded; safe to retry later |

**Operator guidance:**
- This error is **expected and safe** — it prevents spam
- If you see code 94, your `append_attestation_digest()` calls exceeded 5 per ledger
- Solution: Space calls across multiple ledgers or retry in next ledger
- No data is lost (counter resets automatically)

---

## Related Documentation

- **DOS Analysis:** `escrow/src/tests/dos_analysis.rs` § "Attestation Log Append"
- **Security Checklist:** `docs/escrow-security-checklist.md` § "Authentication Matrix"
- **Error Reference:** `docs/escrow-error-messages.md`
- **Attestation Guide:** `docs/escrow-attestations.md` (if exists)

---

## References

- **Soroban Ledger Model:** https://developers.stellar.org/docs/learn/storing-data
- **Rate-limiting patterns:** https://en.wikipedia.org/wiki/Rate_limiting
- **DOS attack surface:** OWASP, CWE-770 (Allocation of Resources Without Limits)

---

## Implementation Notes

### Rationale for MAX_ATTESTATION_APPENDS_PER_LEDGER = 5

- **1 per ledger:** Typical single-entry audit record
- **5 per ledger:** Allows legitimate batch operations (multi-entry audits, KYC updates)
- **32 per ledger:** Would enable spam attack (can fill entire 32-entry log in one ledger)
- **Chosen value:** 5 provides reasonable throughput while preventing spam

### Storage Keys Design

- **`AttestationAppendLedger`:** Tracks which ledger the counter applies to
- **`AttestationAppendCountPerLedger`:** Counter that resets on ledger boundary
- Both stored in **instance storage** (not persistent) — per-escrow, short TTL OK

### Ledger Boundary Detection

```rust
if tracked_ledger != current_ledger {
    per_ledger_count = 0;  // Reset counter
    env.storage().instance().set(&DataKey::AttestationAppendLedger, &current_ledger);
}
```

This works because:
- `env.ledger().sequence()` is deterministic per-ledger
- Monotonically increases
- Uniquely identifies ledger boundary
- No wall-clock or external oracle needed

---

## Security Checklist

- [ ] Rate-limit is enforced before state mutation
- [ ] Rate-limit check does not rely on untrusted input
- [ ] Counter reset is automatic (no manual intervention needed)
- [ ] Ledger sequence comparison is safe (no overflow risk)
- [ ] Error code is specific and actionable (94, not panic)
- [ ] No timing leak (constant-time not needed; no secret input)
- [ ] Storage keys don't collide with existing keys
- [ ] Rate-limit can be increased in future without breaking changes

---

## Commit Message Template

```
[SECURITY] Add per-ledger rate-limit to append_attestation_digest()

- Add DataKey::AttestationAppendLedger to track current ledger
- Add DataKey::AttestationAppendCountPerLedger to count appends per ledger
- Add MAX_ATTESTATION_APPENDS_PER_LEDGER = 5 constant
- Add EscrowError::AttestationAppendRateLimitExceeded (code 94)
- Implement ledger boundary detection and counter reset
- Add 3 unit tests: rate-limit, ledger boundary, boundary conditions
- Update DOS analysis documentation
- Update error reference documentation

Prevents spam attacks where admin maliciously or accidentally calls
append_attestation_digest() multiple times per ledger, rapidly saturating
the bounded log and wasting ledger gas.

Refs: SECURITY_ISSUE_ATTESTATION_RATE_LIMIT.md
Tests: All passing; coverage maintained >95%
Effort: 2-3 hours
```

---

## FAQ

**Q: Why per-ledger instead of global limit?**  
A: Global limit already exists (32 total). Per-ledger limit prevents saturation in a single block while allowing legitimate multi-entry audits over time.

**Q: Why 5 and not 10 or 3?**  
A: 5 allows reasonable batch operations (e.g., multi-part KYC audit, corrective entries) while preventing spam. Can be tuned in future via SCHEMA_VERSION bump if needed.

**Q: What if I need >5 appends per ledger?**  
A: Coordinate across ledgers. Call 5 in ledger N, 5 in ledger N+1, etc. Each ledger boundary resets the counter.

**Q: Does this break existing code?**  
A: No. New keys initialize on first use. Existing instances unaffected. Error code 94 is additive.

**Q: Can an attacker bypass this?**  
A: Only if they control the admin key (already assumes compromise). If so, they can still only add 5 entries per ledger (vs. 32 today).

**Q: What about persistent storage vs. instance storage?**  
A: Counters are per-ledger, so instance storage is appropriate (TTL can be short). No need for persistent storage.

---

**Status:** Ready for backlog  
**Owner:** Security & Engineering Team
