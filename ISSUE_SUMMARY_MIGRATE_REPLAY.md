# Issue Summary: Migrate Replay Protection

**File:** `SECURITY_ISSUE_MIGRATE_REPLAY_PROTECTION.md`  
**Type:** Security / Pre-implementation gap  
**Priority:** HIGH  
**Affects:** Schema migration future implementation

---

## Quick Reference

| Field | Value |
|-------|-------|
| **Issue** | Add replay protection for migrate calls |
| **Severity** | HIGH |
| **Status** | Backlog (pre-implementation security gap) |
| **Component** | `escrow/src/lib.rs::LiquifactEscrow::migrate()` |
| **Affected Versions** | v7+ (when migration logic is added) |
| **Blocked By** | None |
| **Blocks** | Any migration path implementation (v6→v7, etc.) |

---

## Problem

The `migrate()` function will be vulnerable to transaction replay attacks when migration logic is implemented. While Soroban's host provides transaction-level replay protection via auth nonces, the contract lacks **idempotency guards** to prevent double-application of state transformations if:

- An admin key is compromised and used to replay an old transaction
- Off-chain systems accidentally re-submit migration transactions
- The host's auth semantics fail or are bypassed

---

## Current Risk

**TODAY:** No risk. All code paths panic before any storage writes.

**FUTURE:** When adding v6→v7 or later migration paths, the logic will rewrite storage (e.g., yield calculations, per-investor records). A replay would apply these transformations twice:
- ✓ Version check would fail the second time (prevents silent re-application)
- ❌ But intermediate state writes could be replayed if logic is incomplete
- ❌ Audit logs could duplicate entries
- ❌ Per-investor claim locks could be reapplied incorrectly

---

## Solution

Implement **contract-level idempotency** using a migration execution nonce:

1. Add `DataKey::MigrationExecutionLog(from_version, to_version)` to store completion marker
2. Add `EscrowError::MigrationAlreadyApplied = 93` error code
3. Before applying migration logic:
   - Check if `MigrationExecutionLog` for this version pair exists
   - Fail with code 93 if it does (already applied)
4. After successful migration:
   - Write the nonce to prevent re-application
5. Update docs to explain the protection

---

## Acceptance Criteria

✅ All 9 acceptance criteria defined in full issue document

### Key Deliverables

- [ ] Implement idempotency check in `migrate()` function
- [ ] Add `DataKey::MigrationExecutionLog(u32, u32)` and `EscrowError::MigrationAlreadyApplied`
- [ ] Add 3 unit tests for idempotency, persistence, and version transitions
- [ ] Update error code docs (add code 93)
- [ ] Update operator runbook with replay-protection guidance
- [ ] Update security checklist (replace outdated §5.1)

---

## Implementation Notes

### When to Implement

- **MUST** be completed before implementing any real migration path (v6→v7, etc.)
- Can be merged independently (all changes are additive)
- No impact on current codebase (all migrate paths panic today)

### Minimal Changes

- 3 new variants/keys in enums
- ~10 lines of code in `migrate()` function
- ~3 unit tests
- Documentation updates only

### Soroban Context

Soroban's host provides transaction-level replay prevention via auth envelope nonces. This issue adds **contract-level idempotency** as a defense-in-depth measure. Both work together:

- **Host nonce:** Prevents the same signed transaction from being executed twice
- **Contract nonce:** Prevents the same version migration from being applied twice (guards against manual re-calls, replayed code, or implementation errors)

---

## Related Issues

- ADR-007: Storage key evolution strategy
- ADR-009: Per-investor persistent storage (v5→v6 redeploy, no migration)
- Operator runbook §2: Migration implementation guidance
- Security checklist §5.1: Authorization guards (outdated; needs revision)

---

## Testing Strategy

| Test | Purpose |
|------|---------|
| `test_migrate_idempotency_prevents_second_call` | Call migrate(6) twice; second fails with code 93 |
| `test_migrate_nonce_persists_across_upgrades` | Nonce survives WASM upgrade; replay still blocked |
| `test_migrate_different_version_pair_allowed` | Different version pairs don't collide (6→7 then 7→8) |

---

## Operator Guidance (TBD in full runbook)

### For Operators

When you call `migrate(from_version)`:

1. Check that `DataKey::Version == from_version` (contract validates this)
2. Migration succeeds and version is updated
3. An idempotency marker is written (internal; not visible to you)
4. If you call `migrate(from_version)` again, you get error code 93: "Migration already applied"
5. **Do not retry.** The first call succeeded. Verify on-chain version to confirm.

### For Developers Implementing Migration Paths

Before writing your migration logic, ensure you:

1. Keep `Self::load_escrow_require_admin(&env)` as the first statement (auth guard)
2. Check the idempotency nonce early (already in template)
3. Write your state transformation logic
4. Write `DataKey::Version` last
5. Write the idempotency nonce in the same transaction
6. Add unit tests for repeated calls and auth failures

---

## Files Modified (When Implemented)

- `escrow/src/lib.rs` — Enum variants, migrate() logic, tests
- `docs/escrow-error-messages.md` — Error code 93 documentation
- `docs/OPERATOR_RUNBOOK.md` — Migration implementation steps
- `docs/escrow-security-checklist.md` — §5.1 revision (outdated auth guard note)

---

## Deployment Impact

- **No impact today** (all paths panic)
- **Before first migration:** Must be implemented to prevent replay
- **Backward compatible** (new error code is additive)
- **No gas overhead** (nonce write is minimal)

---

## Questions & Clarifications

**Q: Why not rely on Soroban host replay protection?**  
A: The host prevents the same signed transaction from executing twice. But if the admin manually calls `migrate()` again (with a new signature), or if there's a retry loop in off-chain orchestration, the contract should reject it idempotently. Defense in depth.

**Q: Will this affect existing instances?**  
A: No. Existing instances have no `MigrationExecutionLog` keys. The first migration will write them. A fresh init won't have them either (no migration needed).

**Q: Can I upgrade from v6 to v7 without calling migrate()?**  
A: Only if v7 adds new optional keys (additive). If v7 rewrites existing storage, you must call `migrate()` once per instance. The idempotency nonce prevents you from calling it twice.

**Q: What if I deploy a second instance of the same contract?**  
A: Each instance has its own storage. Each will have its own `MigrationExecutionLog` keys. No collision.

---

## References

- Full issue: `SECURITY_ISSUE_MIGRATE_REPLAY_PROTECTION.md`
- Soroban auth model: https://developers.stellar.org/docs/build/guides/auth/contract-authorization
- Storage versioning: `docs/adr/ADR-007-storage-key-evolution.md`
