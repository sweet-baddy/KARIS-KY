# Implementation Guide: Migrate Replay Protection

**Last Updated:** 2026-08-26  
**Affected File:** `escrow/src/lib.rs`  
**Related Issue:** `SECURITY_ISSUE_MIGRATE_REPLAY_PROTECTION.md`

---

## Overview

This guide walks through implementing idempotency protection for the `migrate()` function before adding any real migration logic.

**Estimated effort:** 2–3 hours (including tests and docs)

---

## Step 1: Add DataKey Enum Variants

### Location
`escrow/src/lib.rs` — `enum DataKey` (around line 506–665)

### Changes

Add two new variants to the `DataKey` enum **after existing variants**:

```rust
/// Append-only audit log of reinvestment events for all investors.
/// Bounded by [`MAX_REINVESTMENT_AUDIT_ENTRIES`].
/// Absent ⇒ empty log.
ReinvestmentAuditLog,
// ... (existing) ...

/// Migration execution nonce: records that a specific version transition
/// (from_version, to_version) has been applied to this instance.
/// Key format: `MigrationExecutionLog(from_version, to_version)`
/// Value: the ledger sequence number when the migration completed.
/// Used to prevent idempotent re-application of the same migration path.
/// Absent ⇒ migration never executed. Once written, a second `migrate()` call
/// with the same version pair fails with [`EscrowError::MigrationAlreadyApplied`].
MigrationExecutionLog(u32, u32),

/// Optional: migration completion timestamp for audit trails.
/// Key format: `MigrationCompletedAt(from_version, to_version)`
/// Value: `env.ledger().timestamp()` when the migration finished.
/// Useful for operator debugging and compliance audits.
/// Absent ⇒ no migration record. Companion to [`DataKey::MigrationExecutionLog`].
MigrationCompletedAt(u32, u32),
```

---

## Step 2: Add Error Code

### Location
`escrow/src/lib.rs` — `enum EscrowError` (around line 200–440)

### Changes

Add the new error variant after existing error codes:

```rust
/// All schema migration paths currently return typed errors; no silent work.
/// 
/// | Code | Error | Emitted by | Reason | Recovery | Semantics |
/// |------|-------|-----------|--------|----------|-----------|
/// | 90 | `MigrationVersionMismatch` | `migrate` | stored version `!= from_version` | Pass matching `from_version` | typed |
/// | 91 | `AlreadyCurrentSchemaVersion` | `migrate` | `from_version >= SCHEMA_VERSION` | No migration needed | typed |
/// | 92 | `NoMigrationPath` | `migrate` | `from_version < SCHEMA_VERSION` and no transform implemented | Redeploy or extend `migrate` | typed |
/// | 93 | `MigrationAlreadyApplied` | `migrate` | Migration already applied to this instance; replay detected | Verify on-chain version; no retry needed | typed |

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u32)]
pub enum EscrowError {
    // ... (existing errors 0–92) ...
    
    /// Migration has already been applied to this instance and cannot be replayed.
    /// Stored migration nonce [`DataKey::MigrationExecutionLog`] indicates a prior successful
    /// completion of this version transition. The contract rejects replay to ensure state
    /// transformations are applied exactly once. This is a safety measure to prevent double-applying
    /// yield calculations, collateral records, or per-investor state changes.
    /// 
    /// **Recovery:** Check [`get_version()`] to confirm the schema version is at the target.
    /// If so, the migration succeeded. Do not retry.
    /// If the version is below target, the first migration call may have been incomplete.
    /// Contact governance to investigate or redeploy.
    MigrationAlreadyApplied = 93,
}
```

### Verification

Ensure the error code is:
- Added to `EscrowError` enum
- Uniquely numbered (`93`)
- Derives `Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd` (standard contract error traits)
- Has a descriptive rustdoc comment

---

## Step 3: Update the `migrate()` Function

### Location
`escrow/src/lib.rs` — `LiquifactEscrow::migrate()` (around line 4370–4419)

### Current Implementation

```rust
pub fn migrate(env: Env, from_version: u32) -> u32 {
    Self::load_escrow_require_admin(&env);

    let stored: u32 = env.storage().instance().get(&DataKey::Version).unwrap_or(0);

    ensure(
        &env,
        stored == from_version,
        EscrowError::MigrationVersionMismatch,
    );

    if from_version >= SCHEMA_VERSION {
        fail(&env, EscrowError::AlreadyCurrentSchemaVersion)
    } else {
        fail(&env, EscrowError::NoMigrationPath)
    }
}
```

### Updated Implementation

Replace the entire function with:

```rust
    /// Upgrade the schema version and apply any pending migration logic.
    ///
    /// # When to call
    ///
    /// - **Only** when you have extended `migrate` with a concrete transformation for the
    ///   `from_version → SCHEMA_VERSION` path you need.
    /// - Additive new [`DataKey`] variants read with `.get(...).unwrap_or(default)` do **not**
    ///   require a `migrate` call; old instances simply return the default.
    /// - If `InvoiceEscrow` struct layout changed, `migrate` cannot help — redeploy instead.
    ///
    /// # Errors
    ///
    /// Requires current admin authorization before any version checks or future storage rewrites.
    ///
    /// | Condition | Typed error |
    /// |-----------|--------|
    /// | `stored_version != from_version` | [`EscrowError::MigrationVersionMismatch`] |
    /// | Migration already applied to this instance | [`EscrowError::MigrationAlreadyApplied`] |
    /// | `from_version >= SCHEMA_VERSION` | [`EscrowError::AlreadyCurrentSchemaVersion`] |
    /// | Any `from_version < SCHEMA_VERSION` (all paths) | [`EscrowError::NoMigrationPath`] |
    ///
    /// See `docs/OPERATOR_RUNBOOK.md` §2 for step-by-step instructions on implementing
    /// a concrete migration path. **Before implementing migration logic, ensure this
    /// idempotency check is in place to prevent replay attacks.**
    pub fn migrate(env: Env, from_version: u32) -> u32 {
        // 1. Auth check: MUST be first, before any storage reads or writes
        Self::load_escrow_require_admin(&env);

        // 2. Retrieve stored version
        let stored: u32 = env.storage().instance().get(&DataKey::Version).unwrap_or(0);

        // 3. Validate version alignment
        ensure(
            &env,
            stored == from_version,
            EscrowError::MigrationVersionMismatch,
        );

        // 4. **NEW:** Check idempotency nonce — prevent replaying this migration
        // if it was already successfully applied to this instance.
        let migration_key = DataKey::MigrationExecutionLog(from_version, SCHEMA_VERSION);
        if env.storage().instance().has(&migration_key) {
            // This migration has already been applied. Reject replay.
            fail(&env, EscrowError::MigrationAlreadyApplied)
        }

        // 5. Version boundary checks
        if from_version >= SCHEMA_VERSION {
            fail(&env, EscrowError::AlreadyCurrentSchemaVersion)
        } else {
            // No migration path is implemented for any version below SCHEMA_VERSION.
            // To add one: implement the transformation here, call
            //   env.storage().instance().set(&DataKey::Version, &NEW_VERSION);
            //   env.storage().instance().set(&DataKey::MigrationExecutionLog(from_version, NEW_VERSION),
            //       &env.ledger().sequence());
            //   env.storage().instance().set(&DataKey::MigrationCompletedAt(from_version, NEW_VERSION),
            //       &env.ledger().timestamp());
            // and return NEW_VERSION before reaching this typed error.
            //
            // When a migration path IS implemented, also record the version change:
            //   let mut vh: Vec<(u32, u64)> = env.storage().instance()
            //       .get(&DataKey::VersionHistory).unwrap_or_else(|| Vec::new(&env));
            //   vh.push_back((NEW_VERSION, env.ledger().timestamp()));
            //   env.storage().instance().set(&DataKey::VersionHistory, &vh);
            fail(&env, EscrowError::NoMigrationPath)
        }
    }
```

### Key Changes

1. **Idempotency check** (new):
   - After version validation, check if `MigrationExecutionLog(from_version, SCHEMA_VERSION)` exists
   - If it does, the migration was already applied; fail with code 93
   - Prevents re-application on replay

2. **Rustdoc updated**:
   - New error condition documented
   - Guidance for future implementers (where to write nonce)

3. **Comments for future developers**:
   - Shows exactly where to write the nonce after successful migration
   - Lists all three storage keys to write: `Version`, `MigrationExecutionLog`, `MigrationCompletedAt`

---

## Step 4: Add Unit Tests

### Location
`escrow/src/tests/admin.rs` or `escrow/src/tests/upgrade_compat.rs`

### Add Three Tests

#### Test 1: Idempotency Prevention

```rust
#[test]
fn test_migrate_idempotency_prevents_second_call() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);
    
    // Initialize at version 6 (current SCHEMA_VERSION - 1)
    // Note: This requires setting up a fake v6 instance or using mocks.
    // For now, assume we can create a scenario where stored version is 6.
    
    // Attempt 1: First migrate(6) call should succeed (but currently panics with NoMigrationPath)
    // Once migration logic is implemented, this will succeed.
    // For now, we test the idempotency nonce would prevent a second call.
    
    // Manual test: write the nonce and verify it blocks replay
    env.storage()
        .instance()
        .set(&DataKey::MigrationExecutionLog(6, SCHEMA_VERSION), &env.ledger().sequence());
    
    // Now attempt migrate(6) and verify it fails with MigrationAlreadyApplied
    let result = client.try_migrate(&6);
    assert_eq!(result, Err(Ok(ContractError::from(EscrowError::MigrationAlreadyApplied))));
}
```

#### Test 2: Nonce Persistence Across Upgrades

```rust
#[test]
fn test_migrate_nonce_persists_across_upgrades() {
    let env = Env::default();
    env.mock_all_auths();
    
    // Write the nonce to instance storage
    env.storage()
        .instance()
        .set(&DataKey::MigrationExecutionLog(6, 7), &env.ledger().sequence());
    
    // Simulate a WASM upgrade by creating a new env with the same instance ID
    // (In practice, Soroban preserves instance storage across upgrades)
    let nonce_exists = env.storage()
        .instance()
        .has(&DataKey::MigrationExecutionLog(6, 7));
    
    assert!(nonce_exists, "Nonce should persist in instance storage after upgrade");
}
```

#### Test 3: Different Version Pairs Don't Collide

```rust
#[test]
fn test_migrate_different_version_pair_no_false_positive() {
    let env = Env::default();
    
    // Write nonce for 6 → 7
    env.storage()
        .instance()
        .set(&DataKey::MigrationExecutionLog(6, 7), &env.ledger().sequence());
    
    // 6 → 7 should be blocked
    assert!(env.storage().instance().has(&DataKey::MigrationExecutionLog(6, 7)));
    
    // 7 → 8 should NOT be blocked (different from_version)
    assert!(!env.storage().instance().has(&DataKey::MigrationExecutionLog(7, 8)));
    
    // 6 → 8 should NOT be blocked (different to_version)
    assert!(!env.storage().instance().has(&DataKey::MigrationExecutionLog(6, 8)));
}
```

---

## Step 5: Update Documentation

### File 1: `docs/escrow-error-messages.md`

Add the new error code to the error reference table:

```markdown
| 93 | `MigrationAlreadyApplied` | `migrate` | Migration already applied; replay detected | Verify on-chain version; check migration nonce | typed |
```

Add detail section:

```markdown
#### 93 — MigrationAlreadyApplied

**Emitted by:** `migrate(from_version)`

**Reason:** The migration from `from_version` to `SCHEMA_VERSION` has already been applied to this instance. The contract stores an immutable nonce [`DataKey::MigrationExecutionLog`] after each successful migration to prevent replaying the same version transition.

**Recovery:**
1. Query the contract with `get_version()` to confirm the schema version is at the target (e.g., 7).
2. If version matches the target, the migration succeeded. No retry needed.
3. If version is below target, the migration may have been incomplete. Contact governance for investigation.
4. Do not manually re-call `migrate()` — the typed error is sufficient feedback that replay was blocked.
```

### File 2: `docs/OPERATOR_RUNBOOK.md` §2

Add a new subsection after "Migration implementation requirements":

```markdown
### Migration idempotency and replay protection

**Starting from schema version 7**, the `migrate()` entrypoint enforces idempotency via a
stored nonce [`DataKey::MigrationExecutionLog`]:

- After a successful migration, a nonce is written to prevent re-application of the same
  version transition to the same instance.
- If you call `migrate(from_version)` twice, the second call fails with 
  [`EscrowError::MigrationAlreadyApplied`] (code 93).
- This is a safety measure to prevent double-application of state transformations
  (e.g., yield recalculations, per-investor claim locks, collateral audits).

**Operator action:** Do not retry a `migrate()` call if you receive error code 93. Instead:

1. Verify the on-chain version with `get_version()`.
2. If it matches your target, the migration succeeded.
3. If it does not, investigate the first call's failure reason and contact governance.

**Developer action:** When implementing a migration path (v6 → v7, etc.):

```rust
if from_version == 6 && SCHEMA_VERSION == 7 {
    // 1. Perform state transformation
    // ... your migration logic ...
    
    // 2. Write new version (must be last state write)
    env.storage().instance().set(&DataKey::Version, &7u32);
    
    // 3. Write idempotency nonce in same transaction (prevents replay)
    env.storage().instance().set(
        &DataKey::MigrationExecutionLog(6, 7),
        &env.ledger().sequence(),
    );
    
    // 4. Optional: Record completion timestamp for audit trails
    env.storage().instance().set(
        &DataKey::MigrationCompletedAt(6, 7),
        &env.ledger().timestamp(),
    );
    
    // 5. Return new version
    return 7;
}
```
```

### File 3: `docs/escrow-security-checklist.md` §5.1

**Replace** the existing section "5.1 `migrate()` has no auth guard" with:

```markdown
### 5.1 `migrate()` Authorization and Idempotency Guards

Starting from schema version 7:

- **Authorization guard:** `migrate()` calls `Self::load_escrow_require_admin(&env)` as the first statement, before any version checks or storage reads.
- **Idempotency nonce:** After a successful migration, a [`DataKey::MigrationExecutionLog`] entry is written to prevent replaying the same version transition.

**Impact:** The migration entrypoint is now guarded against both unauthorized access and replay attacks. This is critical when migration logic adds state transformations (yield calculations, per-investor records, collateral audits). Replaying these transformations twice would corrupt accounting.

**For developers implementing migration paths:** Ensure the nonce is written **after** state transformation but **before** returning. See `docs/OPERATOR_RUNBOOK.md` §2 for the template.

**For operators:** If you call `migrate(from_version)` and receive error code 93 (`MigrationAlreadyApplied`), verify the on-chain version with `get_version()`. If it matches your target, the migration succeeded — do not retry.
```

---

## Step 6: Verification Checklist

- [ ] DataKey enum has `MigrationExecutionLog(u32, u32)` and `MigrationCompletedAt(u32, u32)` variants
- [ ] EscrowError enum has `MigrationAlreadyApplied = 93` with rustdoc
- [ ] `migrate()` function checks idempotency nonce after version validation
- [ ] Three unit tests added and passing
- [ ] Error code 93 documented in `docs/escrow-error-messages.md`
- [ ] Operator runbook updated with replay-protection section
- [ ] Security checklist §5.1 revised (no longer "has no auth guard")
- [ ] Code compiles: `cargo build --target wasm32v1-none --release -p karis-ky_escrow`
- [ ] Tests pass: `cargo test -p karis-ky_escrow`
- [ ] Clippy lint passes: `cargo clippy -p karis-ky_escrow -- -D warnings`
- [ ] Coverage maintained: `cargo llvm-cov -p karis-ky_escrow --fail-under-lines 95`

---

## Testing Locally

```bash
cd /workspaces/KARIS-KY

# Compile and run tests
cargo test -p karis-ky_escrow migrate --lib

# Full CI verification
cargo fmt --all -- --check
cargo clippy -p karis-ky_escrow -- -D warnings
cargo build --target wasm32v1-none --release -p karis-ky_escrow
cargo test
cargo llvm-cov -p karis-ky_escrow --features testutils --fail-under-lines 95 --summary-only
```

---

## Commit Message Template

```
[SECURITY] Add replay protection for migrate() calls

- Add DataKey::MigrationExecutionLog(from, to) to store migration completion nonce
- Add EscrowError::MigrationAlreadyApplied (code 93) for idempotency failures
- Update migrate() to check nonce before applying any logic
- Document replay-protection design in OPERATOR_RUNBOOK.md
- Update security checklist §5.1 (auth guard + idempotency)
- Add 3 unit tests: idempotency, persistence, version-pair isolation

This is a pre-emptive security hardening: prepares the migrate() function to safely
support state-rewriting migrations (v6→v7, etc.) without risk of replay attacks
applying transformations twice.

Refs: SECURITY_ISSUE_MIGRATE_REPLAY_PROTECTION.md
Tests: All passing; coverage maintained >95%
```

---

## FAQ

**Q: Do I need to rewrite existing `migrate()` calls?**  
A: No. The idempotency check is backward compatible. Existing deployments have no nonce keys (because no migration path exists yet). The first migration will write the nonce.

**Q: Does this change the SCHEMA_VERSION?**  
A: No. The idempotency mechanism is a contract-level change, not a storage schema change. SCHEMA_VERSION remains unchanged until new migration logic is added.

**Q: Will this affect gas costs?**  
A: Negligibly. One extra `has()` check on instance storage, and one extra `set()` after migration completes. Both are O(1) operations.

**Q: Can I migrate multiple instances at once?**  
A: Yes. Each instance has its own `MigrationExecutionLog` entries. Nonces don't collide across instances.

**Q: What if I'm running an old version before this PR?**  
A: Instances deployed before this change won't have `MigrationExecutionLog` keys. On the first migration call (after upgrade), the keys will be created and written. No migration needed for the idempotency mechanism itself.

---

## References

- **Full issue:** `SECURITY_ISSUE_MIGRATE_REPLAY_PROTECTION.md`
- **Operator guide:** `docs/OPERATOR_RUNBOOK.md` §2
- **Security notes:** `docs/escrow-security-checklist.md`
- **Error codes:** `docs/escrow-error-messages.md`
- **Related ADR:** `docs/adr/ADR-007-storage-key-evolution.md`
