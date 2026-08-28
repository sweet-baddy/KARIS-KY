# Escrow Upgrade Decision Trees and Pitfalls

Quick reference for determining upgrade strategy and avoiding common mistakes.

---

## Decision Tree 1: What upgrade path should I use?

```
START: You have a new WASM release ready for mainnet

  1. Has any #[contracttype] struct layout changed?
     │
     ├─ YES (InvoiceEscrow, SmeCollateralCommitment, etc.)
     │  └─ → REDEPLOY required
     │       Reason: Old WASM cannot decode new XDR shape
     │
     └─ NO
        │
        2. Has any existing DataKey variant been removed or renamed?
           │
           ├─ YES
           │  └─ → REDEPLOY required
           │       Reason: Old instances will panic on read
           │
           └─ NO
              │
              3. Are ALL changes purely additive?
                 (new DataKey variants, new functions, logic fixes)
                 │
                 ├─ YES
                 │  └─ → ADDITIVE WASM UPGRADE safe
                 │       (if admin-gated upgrade entrypoint exists)
                 │
                 └─ NO / UNSURE
                    └─ → Ask security team for code review
```

---

## Decision Tree 2: Should I call migrate()?

```
You're about to upgrade or redeploy.

  1. Did you implement a new migration branch in migrate()?
     │
     ├─ YES
     │  │
     │  2. Is the migration tested with all edge cases?
     │     │
     │     ├─ YES
     │     │  └─ → Call migrate(from_version: N) after upgrade
     │     │       Check: contract panics on mismatch → expected
     │     │
     │     └─ NO
     │        └─ → DO NOT call migrate()
     │            Wait for test coverage and code review
     │
     └─ NO (or migrate() only has the error returns)
        │
        2. Is this an additive WASM upgrade?
           │
           ├─ YES
           │  └─ → DO NOT call migrate()
           │       Reason: No storage rewrite; additive keys default to None
           │
           ├─ NO (redeploy)
           │  └─ → DO NOT call migrate()
           │       Reason: New instances start fresh; old instances are archived
           │
           └─ UNSURE
              └─ → Contact security team before calling migrate()
                   Calling migrate() with wrong from_version panics the contract
```

---

## Decision Tree 3: Is this instance safe to upgrade right now?

```
Before starting any upgrade on a specific contract instance:

  1. Is the escrow status "open" (0)?
     │
     ├─ YES (still funding / no funding close yet)
     │  │
     │  2. Are there active investor deposits in flight?
     │     │
     │     ├─ YES
     │     │  └─ → Delay upgrade
     │     │       Reason: New funding may fail mid-upgrade
     │     │       Wait until: funding closes or funding closes naturally
     │     │
     │     └─ NO
     │        └─ → OK to upgrade
     │
     ├─ NO (funded, settled, withdrawn, or cancelled)
     │  │
     │  2. Is legal hold active?
     │     │
     │     ├─ YES
     │     │  └─ → Clear legal hold FIRST, then upgrade
     │     │       (or keep hold active during upgrade for safety)
     │     │
     │     └─ NO
     │        └─ → OK to upgrade
     │
     └─ UNKNOWN (didn't query get_escrow)
        └─ → Query get_escrow and retry tree
```

---

## Decision Tree 4: What do I do if the upgrade fails?

```
You called upgrade/deploy/init and got a contract error (typed code or panic).

  1. What error code or panic did you get?
     │
     ├─ "already current schema version" (91)
     │  └─ → Expected if you called migrate() twice
     │       Action: Don't retry. Proceed with next instance.
     │
     ├─ "migration version mismatch" (90)
     │  └─ → You passed wrong from_version to migrate()
     │       Action: Query get_version, then call migrate() with correct version
     │
     ├─ "no migration path" (92)
     │  └─ → No migration implementation for this version change
     │       Action: Redeploy instead of upgrading in place
     │
     ├─ Storage/layout panic (XDR decode error)
     │  └─ → You tried to upgrade to incompatible WASM
     │       Action: Redeploy. Do not retry upgrade on this instance.
     │
     ├─ Timeout or RPC connection error
     │  └─ → Network issue, not contract issue
     │       Action: Wait 30 sec, retry once with same parameters
     │
     └─ Other error (unknown code)
        └─ → Check escrow-error-messages.md for typed code meaning
            If still unclear, escalate to security team
```

---

## Common Pitfalls (and how to avoid them)

### Pitfall 1: Calling migrate() during an additive upgrade

**Symptom:** Contract returns error 91 (AlreadyCurrentSchemaVersion)

**Why it happened:** You added new `DataKey` variants but didn't implement a storage rewrite. Calling `migrate()` tries to do bookkeeping that doesn't exist.

**Fix:**
- Do NOT call `migrate()` for additive upgrades
- New keys default to `None` when read — no explicit rewrite needed
- Only call `migrate()` when you implement a real transformation branch

**Prevention:**
- Review: Does `migrate()` have branches for your version change? If not, skip calling it.
- Test on testnet: Verify `get_version` returns expected schema **without** calling `migrate()`

---

### Pitfall 2: Upgrading an instance with incompatible XDR changes

**Symptom:** After upgrade, `get_escrow` panics with XDR decode error

**Why it happened:** You changed `InvoiceEscrow` struct layout but tried to upgrade in place instead of redeploy. The new WASM cannot decode the old stored data.

**Fix:**
- Redeploy required
- Deploy new instance, init with same parameters, migrate investor data offline
- Retire old instance (legal hold, archive ID)

**Prevention:**
- Code review before upgrade: `git diff HEAD~1 escrow/src/lib.rs | grep -A15 '#\[contracttype\]'`
- If any stored struct changed: **REDEPLOY**, not upgrade
- Test on testnet with pre-redeploy state: does new WASM read old data correctly?

---

### Pitfall 3: Missing pre-upgrade legal hold, then settlement tries to execute

**Symptom:** Settlement transaction succeeds during upgrade, but settlement is inconsistent with funding close snapshot

**Why it happened:** Concurrent operations during upgrade window. Legal hold was not activated.

**Fix:**
- Activate legal hold on **all funded instances** before upgrading
- Clear legal hold only after verifying upgrade succeeded
- Document hold activation/clear times

**Prevention:**
- Always activate legal hold on funded instances in your upgrade runbook
- Use template script that batches hold activation before ANY upgrade step
- Monitor logs: confirm no settlement invocations during upgrade window

---

### Pitfall 4: Redeploy without notifying indexer, UI shows wrong contract ID

**Symptom:** Investors try to claim payouts on old contract ID, fail. UI still shows old address.

**Why it happened:** Redeploy creates new instance with new contract ID. Integrators weren't updated.

**Fix:**
- Update indexer: migrate all pointers to new contract ID
- Update API: return new ID in discovery endpoints
- Update UI: clear caches, force refresh

**Prevention:**
- Before redeploy: prepare migration manifest with old → new mappings
- Notify integrators in parallel with deployment (not after)
- Test on testnet: verify all downstream systems pick up new ID

---

### Pitfall 5: Rollback attempted after redeploy (not possible)

**Symptom:** New instance has bug. You try to invoke `upgrade()` entrypoint to revert to old contract ID. Contract doesn't have upgrade entrypoint (or it doesn't exist on old instance).

**Why it happened:** Redeploy creates a new instance with a new contract ID. You can't "rollback" to the old one — old instance is still archived on-chain but not active.

**Fix:**
- For redeploy: there is no rollback via upgrade entrypoint
- Emergency response: activate legal hold on new instance, disable investor onboarding, proceed with fix and redeploy again
- Salvage old instance: if it was operational, you could route future funding there (but new ID is already broadcast)

**Prevention:**
- Redeploy is riskier than additive upgrade — do extra testing on testnet
- Implement an upgrade entrypoint on the contract if you want rollback capability
- For redeploy: document rollback plan as "manual off-chain recovery" (no on-chain revert)

---

### Pitfall 6: Running upgrade on testnet but forgetting to upload to mainnet

**Symptom:** You verified testnet upgrade works, then you forget to upload new WASM to mainnet. You invoke upgrade with wrong hash (or old hash) and wonder why nothing changed.

**Why it happened:** Testnet and mainnet WASM hashes are different (different networks, different uploaded times). Uploading to one does not auto-upload to the other.

**Fix:**
- Always upload WASM to **each network** separately
- Record WASM hash for testnet and mainnet in upgrade checklist
- Verify hash before invoking upgrade: `stellar contract invoke --id ... -- get_wasm_hash` (if entrypoint exists)

**Prevention:**
- Use template script that uploads to **both** networks explicitly
- Add validation: confirm WASM hash was uploaded to current network before invoking upgrade
- Test on testnet: verify you're calling upgrade with testnet hash, not mainnet

---

### Pitfall 7: Legal hold left active after upgrade, blocking investor claims

**Symptom:** Upgrade complete, investors report they cannot claim payouts. Error: legal hold active.

**Why it happened:** You activated legal hold before upgrade but forgot to clear it after verification.

**Fix:**
- Immediately call `set_legal_hold --active false` on each instance
- Notify investors that claims are now unblocked
- Document clear time in upgrade log

**Prevention:**
- Use batch script that **always clears** legal hold at end of upgrade flow
- Add health check after clear: verify `get_escrow` shows `legal_hold_active: false`
- Calendar reminder: if legal hold is still active 24h post-upgrade, escalate

---

### Pitfall 8: Instance inventory out of sync with mainnet reality

**Symptom:** You try to upgrade contract ID `CAAAA...` but it doesn't exist on mainnet. Or you upgrade the wrong contract.

**Why it happened:** Inventory spreadsheet was not updated after a previous redeploy. Multiple versions of contract ID list floating around.

**Fix:**
- Verify inventory before upgrade: call `stellar contract invoke --id <CONTRACT_ID> -- get_version` for each
- If instance missing: check if it was archived/redeployed; find new ID from indexer or logs
- If multiple IDs for same invoice: clarify which is active and which is archived

**Prevention:**
- Maintain a **single source of truth** for contract IDs (e.g., git-tracked JSON or internal API)
- Before every upgrade: sync inventory by querying each instance (timestamp for audit trail)
- Add script: `verify_inventory.sh` that queries all instances and compares to recorded state

---

### Pitfall 9: Analyst compares pre/post state dumps and finds "missing data" (actually just new defaults)

**Symptom:** New key `DataKey::FooBar` is missing from pre-upgrade snapshot (because it didn't exist). Post-upgrade, `get_escrow` includes foo_bar field with value 0. Analyst thinks data was lost.

**Why it happened:** Additive keys are new. Pre-upgrade instances don't have them stored. When read on new WASM, they default to 0 / None. This is expected and correct.

**Fix:**
- Document in audit log: "New keys default to None/0 on old instances; expected behavior"
- Show analyst the specific line in upgrade guide: "Old instances return defaults for missing new keys"

**Prevention:**
- In post-upgrade monitoring: explicitly note which keys are new
- Compare state delta to changelog: confirm all "missing" keys are documented as additive
- Analyst checklist: don't flag new keys as data loss; verify they're in changelog

---

### Pitfall 10: Nobody has admin secret key, upgrade cannot proceed

**Symptom:** Upgrade time arrives. You try to invoke upgrade entrypoint but don't have the admin secret key. Admin is a multisig, and multisig signers are in different timezones.

**Why it happened:** Admin key management not coordinated with upgrade schedule.

**Fix:**
- Reschedule upgrade until multisig is available
- Or, use a backup admin (if governance allows key rotation in advance)

**Prevention:**
- In pre-upgrade checklist: confirm admin multisig members are available during upgrade window
- Coordinate with legal/compliance: legal hold activation requires governance approval
- Staging: 24–48 hours before upgrade, confirm all keys are accessible and signers are online

---

## Symptom-to-action lookup

| Symptom | Likely cause | First action |
|---------|--------------|--------------|
| Upgrade invocation times out | RPC congestion or invalid params | Check `stellar contract invoke` command syntax; retry in 2 min |
| `get_version` still returns old schema | Upgrade didn't apply or wrong hash | Check upgrade invocation result; redeploy if needed |
| `get_escrow` panics with XDR error | Struct layout change, incompatible WASM | Redeploy required; do not retry upgrade |
| Settlement fails with error 30 (legal hold) | Legal hold still active | Call `set_legal_hold --active false` on instance |
| Investor claim fails, says escrow not initialized | Instance wasn't initialized after redeploy | Verify new instance ID; call init if missing |
| New WASM slower than old WASM | Code complexity or new logic inefficiency | Profile on testnet; optimize before mainnet rollout |
| State diff shows unexpected new fields | Additive keys in new WASM | Check CHANGELOG; verify fields are new (expected) |
| Admin key rejected (multisig error) | Wrong signer or threshold not met | Confirm all required signers are included; retry if network delay |

---

## Success criteria for each upgrade type

### Additive upgrade success = all boxes checked

- [ ] All instances upgraded without errors
- [ ] `get_version` returns same schema before and after
- [ ] `get_escrow` reads successfully (spot-check 3+ instances)
- [ ] Investor funding resumes without errors
- [ ] Settlement/payout transactions (if any) execute successfully
- [ ] 72-hour monitoring window: zero unexpected errors
- [ ] State snapshot hashes match (or new keys account for delta)

### Redeploy success = all boxes checked

- [ ] New instances deployed and initialized
- [ ] Investor data restored to new instances (if previously funded)
- [ ] Old instances archived with legal hold active
- [ ] All integrators updated with new contract IDs
- [ ] API/indexer serving new IDs in discovery
- [ ] UI shows new contract IDs (old IDs cleared from cache)
- [ ] Investors notified (if required) of new address
- [ ] 72-hour monitoring window: zero data loss or corruption
- [ ] Compliance/legal sign-off: migration completed per agreements
