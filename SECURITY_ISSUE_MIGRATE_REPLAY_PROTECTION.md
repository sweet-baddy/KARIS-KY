# [SECURITY] Add Replay Protection for Migrate Calls

## Issue Summary

The `migrate()` entrypoint is currently vulnerable to transaction replay attacks when migration logic is eventually implemented. While the current codebase has auth guards documented as a requirement before adding migration paths, the security gap exists in the design and must be explicitly addressed before any migration logic is added.

**Severity:** HIGH (when migration logic is implemented)  
**Status:** Pre-implementation security gap  
**Affected version:** v7 (current) and beyond  
**Component:** `escrow/src/lib.rs::LiquifactEscrow::migrate()`

---

## Problem Description

### Current State

The `migrate()` function currently:
- Requires admin authorization (`Self::load_escrow_require_admin(&env)`)
- Validates the stored version matches the `from_version` parameter
- Returns typed errors on all paths (codes 90–92)
- Performs **no storage writes** before panicking

```rust
pub fn migrate(env: Env, from_version: u32) -> u32 {
    Self::load_escrow_require_admin(&env);
    let stored: u32 = env.storage().instance().get(&DataKey::Version).unwrap_or(0);
    ensure(&env, stored == from_version, EscrowError::MigrationVersionMismatch);
    
    if from_version >= SCHEMA_VERSION {
        fail(&env, EscrowError::AlreadyCurrentSchemaVersion)
    } else {
        fail(&env, EscrowError::NoMigrationPath)
    }
}
```

### The Vulnerability

**Scenario:** When a developer implements a real migration path (e.g., v6 → v7) with state rewrites, the current approach lacks **idempotency protection and transaction replay detection**.

**Attack vector:** If the following sequence occurs:

1. Operator calls `migrate(from_version=6)` in a transaction `TX_A`
2. Transaction `TX_A` is finalized on-ledger; the version is updated to 7
3. An attacker (or vulnerable off-chain system) **replays `TX_A`** by:
   - Re-signing or reusing the same transaction envelope
   - Re-submitting to the same contract instance (same address)
   - The Soroban host normally prevents this with its built-in replay protection based on the auth envelope's nonce

**However:**

- If the migration logic is **not idempotent** (e.g., accumulates yield, creates duplicate audit logs, or performs arithmetic checks that fail on second run), the replay can cause:
  - Double-application of state transformations
  - Inconsistent accounting
  - Partial or failed migrations on second execution
  - Silent data loss if the second call fails silently

- If the migration is protected only by the Soroban host's transaction nonce (signature replay protection), it relies on:
  - Admin maintaining secure nonces
  - No admin key compromise
  - Correct auth envelope semantics across all SDK integrations

- **Soroban host replay semantics are NOT equivalent to contract-level idempotency guarantees.**

---

## Root Cause

The `OPERATOR_RUNBOOK.md` already documents the proper implementation pattern (requires admin auth before version checks), but:

1. **No explicit idempotency marker** is stored after a successful migration to prevent re-execution in the same instance
2. **No "migration in progress" state** guards against partial failures and retries
3. **No per-migration session nonce or timestamp** to distinguish first execution from replays within the contract's own logic
4. **Documentation does not mention replay protection** as a requirement for migration implementation

---

## Steps to Reproduce

### Precondition
- Implement a real migration path in `migrate()` that rewrites storage (e.g., v6 → v7)

### Reproduction

```rust
// Step 1: Operator calls migrate(6) successfully
let result1 = escrow::LiquifactEscrow::migrate(env.clone(), 6);
assert_eq!(result1, 7); // Version is now 7

// Step 2: Simulate replay (same transaction re-submitted)
// This is normally blocked by Soroban host auth nonce.
// But if an implementation mistake bypasses host nonce or uses a weak auth check:
let result2 = escrow::LiquifactEscrow::migrate(env.clone(), 6);

// Without contract-level idempotency:
// - If from_version=6 is re-checked, it may fail (version is now 7) ✓
// - But if the migration logic is called again before version check: 🔴 state rewrite happens twice
// - Per-investor yield or collateral audits could be duplicated
```

### Expected vs. Actual

| Aspect | Expected | Actual (Pre-fix) |
|--------|----------|------------------|
| First `migrate(6)` call | Version updated to 7; state transformed once | ✓ Works |
| Replay of same `migrate(6)` tx | Host nonce prevents re-execution | ✓ Prevented by host |
| Contract-level idempotency | Second call fails cleanly or is no-op | ❌ Not guaranteed |
| Audit log consistency | Exactly one migration event per version bump | ❌ Risk if retries occur |
| Per-investor claim locks | Applied exactly once | ❌ Risk if reapplied |

---

## Proposed Solution

Implement **contract-level migrate idempotency protection** using a combination of:

### 1. **Single-Use Migration Nonce** (Primary Defense)

Store a migration execution nonce that:
- Is written **after** migration logic but **before** version update succeeds
- Prevents the same version transition from being applied twice
- Survives contract upgrades (stored in persistent storage)

```rust
pub enum DataKey {
    // ... existing keys
    /// Migration execution records: (from_version, to_version) → ledger_sequence
    /// Used to prevent replaying the same migration path on the same instance.
    /// Stores the ledger sequence at which the migration was successfully applied.
    MigrationExecutionLog(u32, u32),  // (from_version, to_version)
}
```

### 2. **Version Monotonicity Check**

Before applying any migration logic:
- Verify the stored `DataKey::Version` equals `from_version` (already done)
- Verify `from_version < SCHEMA_VERSION` (already done)
- **NEW:** Check if `DataKey::MigrationExecutionLog(from_version, SCHEMA_VERSION)` already exists
  - If it exists, the migration was already applied; return error or no-op

```rust
pub fn migrate(env: Env, from_version: u32) -> u32 {
    // 1. Auth check (required before any state write)
    Self::load_escrow_require_admin(&env);
    
    // 2. Retrieve stored version
    let stored: u32 = env.storage().instance().get(&DataKey::Version).unwrap_or(0);
    ensure(&env, stored == from_version, EscrowError::MigrationVersionMismatch);
    
    // 3. **NEW:** Check if this migration was already applied
    let migration_key = DataKey::MigrationExecutionLog(from_version, SCHEMA_VERSION);
    if env.storage().instance().has(&migration_key) {
        // This migration was already successfully applied; fail to prevent re-application
        fail(&env, EscrowError::MigrationAlreadyApplied)  // New error code
    }
    
    if from_version >= SCHEMA_VERSION {
        fail(&env, EscrowError::AlreadyCurrentSchemaVersion)
    }
    
    // 4. Apply migration logic (future developers add here)
    // if from_version == 6 && SCHEMA_VERSION == 7 {
    //     // ... perform state rewrite ...
    //     env.storage().instance().set(&DataKey::Version, &7u32);
    //     env.storage().instance().set(&migration_key, &env.ledger().sequence());
    //     return 7;
    // }
    
    fail(&env, EscrowError::NoMigrationPath)
}
```

### 3. **New Error Code**

Add to `EscrowError` enum:

```rust
/// Migration has already been applied to this instance and cannot be replayed.
/// Indicates the version transition was completed in a prior transaction.
MigrationAlreadyApplied = 93,
```

### 4. **Audit Trail**

Store the ledger sequence (or timestamp) when the migration completes:

```rust
pub enum DataKey {
    // ... existing keys
    /// Timestamp (from env.ledger().timestamp()) when a migration was last applied.
    /// Used for audit trails and operator verification that a migration succeeded.
    /// Entries keyed as (from_version, to_version).
    MigrationCompletedAt(u32, u32),
}
```

Then in migrate:
```rust
env.storage().instance().set(&DataKey::MigrationCompletedAt(from_version, SCHEMA_VERSION), 
                             &env.ledger().timestamp());
```

---

## Acceptance Criteria

- [ ] **AC1:** `DataKey::MigrationExecutionLog(u32, u32)` variant is added to the `DataKey` enum
- [ ] **AC2:** `EscrowError::MigrationAlreadyApplied = 93` is added to the error enum
- [ ] **AC3:** `migrate()` checks for prior execution before applying migration logic:
  - Retrieves `DataKey::MigrationExecutionLog(from_version, SCHEMA_VERSION)`
  - Fails with `MigrationAlreadyApplied` if the key exists
- [ ] **AC4:** Successful migration writes the execution nonce:
  - `env.storage().instance().set(&DataKey::MigrationExecutionLog(...), &env.ledger().sequence())`
  - This write happens **after** state transformation but **before** returning
- [ ] **AC5:** Error code 93 is added to `docs/escrow-error-messages.md` with recovery guidance:
  - **Message:** "Migration to this schema version has already been applied"
  - **Recovery:** "Migration is idempotent; no action required. Verify on-chain version matches target."
- [ ] **AC6:** Updated `OPERATOR_RUNBOOK.md` §2:
  - Explicitly states: "Migrations are guarded against replay via contract-level idempotency check"
  - Add step to migration implementation: "Write idempotency nonce after version bump"
- [ ] **AC7:** Unit tests added:
  - `test_migrate_idempotency_prevents_second_call` — verify calling migrate twice with same from_version fails with code 93
  - `test_migrate_nonce_persists_across_upgrades` — verify the nonce survives WASM upgrades (instance storage persistence)
  - `test_migrate_different_version_pair_allowed` — verify migrating 6→7 then 7→8 does NOT trigger false positive (different from_version)
- [ ] **AC8:** Documentation additions:
  - Add security note in `escrow-security-checklist.md` §5.1 (replace the existing "no auth guard" section):
    - "Before v7, `migrate()` had no auth guard. Starting v7, it includes auth guard and idempotency nonce."
  - Add replay-protection note to ADR-007 or new ADR-010 (Schema Migration Idempotency)
- [ ] **AC9:** Code comments in `migrate()`:
  - Explain the idempotency check before the migration branch
  - Document the purpose of `MigrationExecutionLog` in the DataKey enum

---

## Implementation Checklist

- [ ] Modify `DataKey` enum: add `MigrationExecutionLog(u32, u32)` and optionally `MigrationCompletedAt(u32, u32)`
- [ ] Modify `EscrowError` enum: add `MigrationAlreadyApplied = 93`
- [ ] Update `migrate()` function logic:
  - Insert idempotency check after auth and version validation
  - Write nonce after state transformation
- [ ] Update `docs/escrow-error-messages.md`:
  - Add entry for error code 93
- [ ] Update `OPERATOR_RUNBOOK.md`:
  - Add replay-protection guidance to §2 migration implementation steps
- [ ] Update `docs/escrow-security-checklist.md`:
  - Revise §5.1 to reflect new auth guard + idempotency protection
- [ ] Add unit tests:
  - `test_migrate_idempotency_prevents_second_call`
  - `test_migrate_nonce_persists_across_upgrades`
  - `test_migrate_different_version_pair_allowed`
- [ ] CI verification:
  - All existing tests pass
  - New tests pass
  - Code coverage maintained (>95%)
  - Clippy lint passes

---

## Security Considerations

### Out of Scope (Handled by Soroban Host)
- Transaction-level signature replay prevention (host nonce envelope)
- Authorization ledger entry validation
- Contract invocation idempotency at the transport layer

### In Scope (Contract Level)
- Idempotent state transitions for migration
- Clear audit trail of version changes
- Typed error semantics for operator clarity
- No silent re-application of state changes

### Threat Model
- **Attacker:** Compromise of admin key, or accidental re-submission of old transaction
- **Impact:** Double-application of yield calculations, collateral records, or version upgrades
- **Mitigation:** Nonce-based contract-level idempotency guard (this issue)

---

## Testing Strategy

### Unit Tests

1. **test_migrate_idempotency_prevents_second_call**
   - Call `migrate(6)` successfully (assume v6 → v7 path exists)
   - Call `migrate(6)` again (same from_version)
   - Assert second call returns `MigrationAlreadyApplied` error (code 93)

2. **test_migrate_nonce_persists_across_upgrades**
   - Call `migrate(6)` on version 6 instance
   - Verify `DataKey::MigrationExecutionLog(6, 7)` is set
   - Simulate WASM upgrade (env stays same, storage persists)
   - Verify nonce is still readable, second migrate(6) still fails

3. **test_migrate_different_version_pair_allowed**
   - Call `migrate(6)` (6 → 7)
   - Later, add v7 → v8 migration logic
   - Call `migrate(7)` (7 → 8)
   - Assert both succeed without false-positive idempotency collision

4. **test_migrate_unauthorized_fails_before_nonce_check**
   - Attempt `migrate(6)` without admin auth
   - Assert auth check fails before nonce lookup (proves auth ordering)

---

## Related Documentation

- **Current:** `docs/escrow-security-checklist.md` §5.1 — "migrate() has no auth guard"
- **Current:** `OPERATOR_RUNBOOK.md` §2 — Migration implementation steps
- **Related ADR:** `docs/adr/ADR-007-storage-key-evolution.md` — Storage versioning strategy
- **Related ADR:** `docs/adr/ADR-009-per-investor-persistent-storage.md` — v5 → v6 redeploy (no migration)

---

## Deployment Impact

### Before Migration Logic Exists
- No change required (all paths panic; no idempotency risk)
- Deployment can proceed when any non-migration changes are needed

### After Migration Logic Is Added
- **MUST complete this issue before shipping migration path**
- Idempotency nonce writes happen in instance storage (no new gas overhead)
- Backward compatible (new error code 93 is additive)

### Operational Runbook Update
- Operators calling `migrate()` see typed error 93 if called twice (safe behavior)
- No change to success path (version still updates, nonce is written automatically)

---

## References

- **Soroban Replay Protection:** https://developers.stellar.org/docs/build/guides/auth/contract-authorization
- **Contract Authorization Lifecycle:** https://developers.stellar.org/docs/learn/storing-data/contract-storage#authorization
- **Ledger Time Semantics:** `docs/escrow-ledger-time.md`
- **Error Code Reference:** `docs/escrow-error-messages.md`
- **OPERATOR_RUNBOOK.md:** Migration implementation guidance

---

## Author Notes

This issue documents a **pre-emptive security gap**: the vulnerability does not exist until migration logic is added, but the fix must be designed now to ensure future implementations are idempotent from day one. The proposed solution:

1. **Adds minimal overhead** (one persistent key write per migration path per instance)
2. **Integrates cleanly** with the existing auth and version-check pattern
3. **Provides clear operator feedback** (typed error code 93)
4. **Requires no SDK changes** (contract-level change only)

**Timeline:** Implement before adding any v6 → v7 migration path or later version transitions.
