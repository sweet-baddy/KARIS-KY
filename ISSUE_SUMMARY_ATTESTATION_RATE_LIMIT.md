# Issue Summary: Attestation Append Rate-Limiting

**File:** `SECURITY_ISSUE_ATTESTATION_RATE_LIMIT.md`  
**Type:** Security / DoS prevention  
**Priority:** MEDIUM  
**Severity:** MEDIUM  
**Status:** Backlog (design enhancement)

---

## Quick Reference

| Field | Value |
|-------|-------|
| **Issue** | Rate-limit append_attestation_digest calls per ledger to prevent log spam |
| **Severity** | MEDIUM |
| **Status** | Backlog |
| **Component** | `escrow/src/lib.rs::LiquifactEscrow::append_attestation_digest()` |
| **Affected Versions** | v7 (current) and all prior versions |
| **Requires Compromise** | Admin key (authentication not bypassed) |
| **Blocked By** | Nothing |
| **Blocks** | Nothing (independent enhancement) |

---

## Problem

The `append_attestation_digest()` function has **no per-ledger rate-limiting**. An attacker with admin key access can call the function multiple times in the same ledger, causing:

- Log spam (fill 32-entry log rapidly)
- Ledger gas waste (32 storage writes in one block)
- Loss of legitimate audit records
- Audit trail manipulation

**Today's constraint:** Global capacity of 32 total entries  
**Missing constraint:** Per-ledger rate-limit (unbounded calls per ledger)

### Current Risk Level

**MEDIUM** — Requires admin key compromise + intentional abuse  
**Threat:** Compromised admin + DoS attack against audit trail

### Comparison: Before vs. After

| Scenario | Before | After | Improvement |
|----------|--------|-------|-------------|
| **1 append/ledger (legitimate)** | ✅ Works | ✅ Works | No change |
| **5 appends/ledger (batch audit)** | ✅ Works | ✅ Works | No change |
| **32 appends/ledger (spam attack)** | 🔴 All succeed (bug) | ✅ Blocked after 5 | **Prevented** |
| **Time to fill log (at max rate)** | 1 ledger | 7 ledgers | **7x slower spam** |

---

## Solution

Implement **per-ledger rate-limiting** using:

1. **New constant:** `MAX_ATTESTATION_APPENDS_PER_LEDGER = 5`
2. **Ledger tracking:** Store current ledger sequence
3. **Per-ledger counter:** Reset on ledger boundary
4. **Rate-limit check:** Fail with error code 94 if exceeded

### How It Works

```rust
// On each append_attestation_digest() call:

current_ledger = env.ledger().sequence()

if current_ledger != stored_ledger:
    reset_per_ledger_counter()  // New ledger: start fresh
    
if per_ledger_counter >= MAX_ATTESTATION_APPENDS_PER_LEDGER:
    fail(EscrowError::AttestationAppendRateLimitExceeded)  // Code 94
    
// Otherwise append succeeds and counter increments
```

### Why Per-Ledger?

- **Global limit (32 entries)** ← Already exists, prevents infinite growth
- **Per-ledger limit (5 calls)** ← NEW, prevents rapid saturation within one block
- **Both work together** for defense-in-depth

---

## Acceptance Criteria

✅ All 11 criteria defined in full specification:

- [ ] `DataKey::AttestationAppendLedger` variant added
- [ ] `DataKey::AttestationAppendCountPerLedger` variant added
- [ ] `MAX_ATTESTATION_APPENDS_PER_LEDGER = 5` constant defined
- [ ] `EscrowError::AttestationAppendRateLimitExceeded = 94` added
- [ ] Ledger boundary detection and counter reset implemented
- [ ] Rate-limit check enforced in `append_attestation_digest()`
- [ ] Error code 94 documented
- [ ] Security checklist updated
- [ ] DOS analysis updated
- [ ] 3 unit tests added
- [ ] Verification checklist passed

---

## Implementation

| Aspect | Detail |
|--------|--------|
| **Effort** | 2-3 hours |
| **Code changes** | ~50 lines (enum variants, rate-limit logic) |
| **New error code** | 94 (AttestationAppendRateLimitExceeded) |
| **Tests** | 3 unit tests (rate-limit, boundary, reset) |
| **Breaking change** | NO (error code is additive) |
| **Requires redeploy** | NO (WASM upgrade sufficient) |
| **Backward compatible** | YES (new storage keys initialize on first use) |

---

## Testing Strategy

| Test | Purpose |
|------|---------|
| `test_append_attestation_rate_limit_per_ledger` | Verify ≤5 succeed, 6th fails with code 94 |
| `test_append_attestation_rate_limit_resets_on_ledger_advance` | Verify counter resets when ledger advances |
| `test_append_attestation_allows_5_per_ledger_then_blocks` | Boundary condition: exactly 5 then reject |

---

## Error Code 94

**Name:** `AttestationAppendRateLimitExceeded`  
**Meaning:** More than 5 calls to `append_attestation_digest()` in same ledger  
**Recovery:** Wait for next ledger; counter resets automatically  
**Operator action:** Safe; retry later or spread calls across ledgers

---

## Security Impact

### What's Protected

✅ Prevents rapid log saturation via spam attacks  
✅ Bounds per-ledger gas consumption from attestations  
✅ Preserves audit trail integrity (harder to manipulate)

### What Remains Protected (unchanged)

✅ Admin authorization still required  
✅ Global capacity still 32 entries  
✅ Ledger signature validation (host-level)

### What's NOT Protected

❌ Does not prevent compromise of admin key itself  
❌ Does not prevent other forms of DoS (e.g., other entrypoints)  
❌ Does not prevent all log spam (just rate-limits it)

---

## Deployment

### Pre-Deployment Testing

```bash
cargo build --target wasm32v1-none --release
cargo test -p karis-ky_escrow
cargo clippy -p karis-ky_escrow -- -D warnings
cargo llvm-cov --fail-under-lines 95
```

### Deployment Strategy

1. Implement feature branch (2-3 hours)
2. Code review (1 hour)
3. Merge to main
4. Deploy as WASM upgrade (preserves contract address & state)
5. Verify on testnet, then mainnet

### Operational Runbook Update

- Document error code 94 in operator guides
- Explain rate-limit to users (not a bug; intentional)
- Show how to spread calls across ledgers if needed

---

## Related Issues

- **Migrate replay protection:** Similar defense-in-depth approach (per-instance idempotency)
- **Fund batch limits:** Similar per-call DOS prevention (MAX_FUND_BATCH)
- **DOS analysis:** `escrow/src/tests/dos_analysis.rs` (existing framework)

---

## FAQ

**Q: Why is this not critical?**  
A: Requires admin key compromise AND intentional abuse. Not a critical vulnerability, but a design weakness that should be fixed.

**Q: What if I legitimately need >5 per ledger?**  
A: Coordinate across multiple ledgers. Call 5 in ledger N, 5 in ledger N+1, etc.

**Q: Can this be tuned later?**  
A: Yes. Change `MAX_ATTESTATION_APPENDS_PER_LEDGER` constant in next schema version if needed. Rate-limit is orthogonal to other limits.

**Q: Does this affect existing instances?**  
A: No. New storage keys auto-initialize on first use. No migration needed.

**Q: What's the performance impact?**  
A: Negligible. One extra ledger sequence check and one counter increment per call.

---

## Files Modified (When Implemented)

- `escrow/src/lib.rs` — Enum variants, constant, rate-limit logic, tests
- `docs/escrow-error-messages.md` — Error code 94 documentation
- `docs/escrow-security-checklist.md` — Rate-limit note
- `escrow/src/tests/dos_analysis.rs` — DOS analysis update

---

## Implementation Checklist

- [ ] Add DataKey enum variants (2)
- [ ] Add MAX_ATTESTATION_APPENDS_PER_LEDGER constant
- [ ] Add EscrowError code 94
- [ ] Implement rate-limit logic in append_attestation_digest()
- [ ] Add 3 unit tests
- [ ] Update error documentation
- [ ] Update security documentation
- [ ] Update DOS analysis
- [ ] Verify: cargo build, test, clippy, coverage
- [ ] Create PR with all changes
- [ ] Code review and merge

---

## Questions?

**For understanding the issue:**  
→ Read this summary first, then full specification

**For threat model details:**  
→ See full spec, section "Threat Model"

**For implementation details:**  
→ See full spec, section "Proposed Solution" with code examples

**For timeline:**  
→ 2-3 hours implementation + 1 hour review

---

## References

- Full issue: `SECURITY_ISSUE_ATTESTATION_RATE_LIMIT.md`
- DOS analysis: `escrow/src/tests/dos_analysis.rs`
- Error reference: `docs/escrow-error-messages.md`
- Security checklist: `docs/escrow-security-checklist.md`
