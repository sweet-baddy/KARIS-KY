# ADR-009: Per-Investor Keys in Persistent Storage — TTL, Footprint, and Migration

**Status:** Accepted
**Date:** 2026-07-27
**Refs:** `escrow/src/lib.rs` — `DataKey::InvestorContribution`, `DataKey::InvestorEffectiveYield`, `DataKey::InvestorClaimNotBefore`, `DataKey::InvestorClaimed`, `DataKey::InvestorAllowlisted`, `SCHEMA_VERSION`, `migrate`; ADR-007; `docs/escrow-data-model.md`; `docs/escrow-gas-storage-notes.md`

> **Note on numbering:** This ADR is filed as **009** because [ADR-007 (Storage Key Evolution and Additive-Key Policy)](ADR-007-storage-key-evolution.md) already exists and covers this topic in its `Rule 5`. ADR-007 establishes the **policy**; this ADR provides the **dedicated rationale**, **tradeoffs**, and **v5 → v6 migration plan** that the original storage-evolution policy references. Both ADRs must be read together for the full picture.

---

## Context

Prior to schema version 6, the four per-investor `DataKey` variants — `InvestorContribution`, `InvestorEffectiveYield`, `InvestorClaimNotBefore`, and `InvestorClaimed` — were stored in **instance storage**. That meant every distinct investor address contributed one entry to the contract instance's storage footprint, and all those entries shared the **same TTL** as the contract instance.

As investor cardinality grew, two structural problems emerged:

1. **Unbounded instance footprint.** Each investor address added a row to the contract instance's key set. Soroban instance storage is bound by per-entry size and total entry count; with many investors the aggregate size, the per-host-function read/write cost, and the rent/archival risk all grew together.
2. **Coupled TTL.** Every investor's contribution entry was tied to the instance TTL. Bumping instance TTL extended *every* investor's entry; *not* bumping it meant a single near-archival instance could silently freeze *all* investors at once, including ones who had just deposited seconds earlier.

Schema version 6 relocated the per-investor keys to **persistent storage** so each address has its own TTL row and the contract instance footprint is no longer a function of investor count.

This ADR records why we chose persistent storage (and not, for example, a side table or an off-chain index), enumerates the tradeoffs we accepted, and documents the v5 → v6 migration path that operators must follow.

---

## Decision

### 1. Per-investor keys live in `env.storage().persistent()`

Starting at `SCHEMA_VERSION = 6`, the following keys use persistent storage instead of instance storage:

| Key | Type | Persistent | Independent TTL |
|-----|------|-----------|-----------------|
| `DataKey::InvestorContribution(Address)` | `i128` | ✓ | per-address |
| `DataKey::InvestorEffectiveYield(Address)` | `i64` | ✓ | per-address |
| `DataKey::InvestorClaimNotBefore(Address)` | `u64` | ✓ | per-address |
| `DataKey::InvestorClaimed(Address)` | `bool` | ✓ | per-address |
| `DataKey::InvestorAllowlisted(Address)` | `bool` | ✓ | per-address |

Each variant is read with `.get(...).unwrap_or(default)` so the contract treats "absent" as the safe default (`0`, base `yield_bps`, `0`, `false`, `false`). No entrypoint semantics change for callers; only the storage location does.

### 2. Contract instance footprint is bounded

After this change, the **contract instance** holds only the keys enumerated under `DataKey` whose discriminant is *not* an `(Address)` tuple. Investor cardinality is decoupled from instance storage size and from instance-storage rent/archival risk. The instance footprint now scales with **escrow configuration**, not with **investor count**.

### 3. TTL is managed per-address

Each persistent entry has an independent TTL. The contract already extends TTL on relevant paths via `LiquifactEscrow::bump_ttl` for time-sensitive keys (see `docs/escrow-gas-storage-notes.md`). Per-address TTL means a near-archival entry on one investor cannot affect any other investor; bumps and restorations are localised.

### 4. No silent `migrate` from v5 → v6

There is no on-chain migration path from v5 (instance-stored) to v6 (persistent-stored) per-investor entries:

- Soroban does not provide a way to enumerate instance-storage keys by `Address` discriminator.
- We cannot copy "all investor entries" from instance storage to persistent storage without enumerating them, which we cannot do from inside the contract.
- Forging a fake enumeration at `migrate` time would silently drop principal and create a fund-theft vector.

`migrate(from_version)` therefore returns [`EscrowError::NoMigrationPath`] (code 92) for `from_version < SCHEMA_VERSION`. Operators must **redeploy** fresh contract instances at schema version 6 and migrate off-chain state (per-investor balances, claim flags, allowlist membership) into the new instance via standard funding / settlement flows.

---

## Rationale

### Why persistent storage (and not "keep in instance" or "move off-chain")

**Persistent storage** wins because:

- It is the only on-chain storage tier that combines **per-key TTL** with **per-key independence** and **bounded instance footprint**. Temporary storage would lose data; instance storage couples TTL.
- Persistent entries are part of the same trust domain as the contract. The contract can still read/write them deterministically without trusting an external indexer.
- The Soroban host provides `bump_ttl` on persistent entries, so we can extend individual addresses' TTL on access paths (deposit, claim) without touching others.

**Why not off-chain?**

- Per-investor principal (`InvestorContribution`) is load-bearing for `settle`, `withdraw`, `claim_investor_payout`, `refund`, and `sweep_terminal_dust` accounting. Putting it off-chain would require an oracle, reintroducing a trust assumption we explicitly avoid by keeping all liability data on-chain.

### Why the migration is "redeploy, not migrate"

ADR-001 makes status transitions strictly forward-only. Investors' principal is the contract's liability. A "migration" that loses or duplicates any principal entry is, by definition, a fund-theft or fund-loss bug. The only safe v5 → v6 transition is one where the new instance starts with **zero liability** and investors re-record their principal via standard deposit flows on the new instance.

In practice, this means:

1. Stop funding on the v5 instance (or freeze via `set_legal_hold(true)`).
2. Settle any open obligations on the v5 instance using its on-chain data.
3. Deploy a fresh v6 instance.
4. Migrate off-chain records (investor addresses, intended principal, allowlist) into the new instance's configuration.
5. Investors execute a normal `fund` (or `fund_with_commitment`) on the new instance.

Operators with low-cardinality investor sets can do this in a single maintenance window. For high-cardinality instances, the recommended pattern is to **run v5 and v6 in parallel** during the transition, freeze v5, and let investors "opt in" to the new instance at their own pace.

---

## Tradeoffs

| Tradeoff | What we accepted | Mitigation |
|----------|------------------|-----------|
| **Read latency** | Persistent reads incur an extra host-function call compared to instance reads in some paths. | Per-investor reads are rare on hot paths (deposit, claim). `fund` and `fund_with_commitment` already require multiple storage reads; the additional read is amortised. |
| **Storage read consistency** | Persistent entries are not jointly snapshotted with the instance, so a `get_escrow_summary`-style call may see a consistent instance view but a slightly stale persistent view if a concurrent transaction is mid-flight. | Soroban's single-writer host model guarantees within-transaction consistency. Cross-transaction ordering is preserved because all state mutations happen inside the contract's own entrypoints (no external writer). Indexers that need a join must read inside one transaction. |
| **TTL management** | Each persistent entry has its own TTL and can individually near-archival. Operators must call `bump_ttl` (or rely on access-path bumps) to keep them alive. | `bump_ttl` extends TTL for the four per-investor keys on relevant entrypoints. Operators can monitor persistent-entry TTL via RPC (`getLedgerEntries`) and run an automated bumper as a cron job. See `docs/escrow-gas-storage-notes.md`. |
| **Storage cost** | Persistent storage rent is per-entry, so 1,000 investors costs 1,000 rent units instead of "free inside instance." | Persistent rent is the **explicit cost** of decoupling per-investor TTL. Compared to the alternative (instance storage growing without bound), this is the cheaper long-term option for any escrow with > 100 investors. |
| **Migration ceremony** | v5 instances cannot upgrade in place to v6; operators must redeploy. | Documented redeploy workflow; can be automated with the deploy script (`scripts/deploy.sh`) and verification script (`scripts/verify_deployment.sh`). Parallel-run pattern minimises user disruption. |

### What we did *not* accept

- **Loss of on-chain liability tracking.** Off-chain indexing was rejected because it reintroduces oracle trust and complicates dispute resolution.
- **A `migrate` path that rewrites instance keys.** Impossible to do safely (cannot enumerate) and would silently drop principal.
- **A "shadow write" pattern** where v5 keeps writing both instance and persistent entries during a transition. This doubles storage cost during the transition window and complicates reconciliation. The parallel-run pattern is cleaner.

---

## Migration strategy: v5 → v6

The v5 → v6 migration is a **redeploy**, not an in-place upgrade. The canonical sequence is:

### Phase 1 — Pre-flight (operator)

- [ ] Read `get_version` on every v5 instance; confirm it returns `5`.
- [ ] Call `get_escrow_summary` and `verify_asset_custody` on each; confirm `funded_amount` matches the on-chain funding-token balance.
- [ ] Snapshot all read-only state via `backend/backup_escrow_state.py` (per-investor principal included via the supplied address list).
- [ ] Decide whether to freeze via `set_legal_hold(true)` (recommended for any escrow with non-trivial funded principal) so no new deposits arrive during the transition.
- [ ] Notify investors via the indexer's `set_legal_hold` event.

### Phase 2 — Deploy v6 (operator)

- [ ] Build the v6 WASM (`cargo build --target wasm32v1-none --release -p karis_ky_escrow`).
- [ ] Deploy to the **same network** as the v5 instance.
- [ ] Run `scripts/verify_deployment.sh` against the new instance; confirm exit code `0`.
- [ ] Call `get_version`; confirm it returns `6`.
- [ ] Call `get_build_metadata`; confirm the embedded commit hash matches the v6 release tag.

### Phase 3 — Init v6 (operator)

- [ ] Call `init` on the v6 instance with the same parameters as v5 (`admin`, `invoice_id`, `sme_address`, `amount`, `yield_bps`, `maturity`, `funding_token`, `treasury`, optional `registry`, `yield_tiers`, etc.).
- [ ] Re-record the allowlist (if used) via `set_investors_allowlisted` on the v6 instance. Note: v6 introduces `DataKey::InvestorAllowlisted(Address)` in persistent storage; v5 stored this in instance. Migration is a fresh re-recording.

### Phase 4 — Investor re-deposit (per investor, off-chain coordinated)

- [ ] Each investor calls `fund` (or `fund_with_commitment` if tiering is in use) on the **v6** instance for their original principal.
- [ ] v6 records `InvestorContribution`, `InvestorEffectiveYield` (only via `fund_with_commitment`), `InvestorClaimNotBefore` (only via `fund_with_commitment`), and (when applicable) `InvestorAllowlisted` under persistent storage.

### Phase 5 — Settle v5 (operator)

- [ ] Continue normal settlement, withdrawal, and claim flows on the v5 instance for any investors who have not yet migrated.
- [ ] Once v5 reaches a **terminal status** (settled, withdrawn, cancelled, archived), run `sweep_terminal_dust` to recover any residual funding-token balance to the v5 treasury.

### Phase 6 — Decommission v5 (operator)

- [ ] Confirm v5 instance has no outstanding obligations (`funded_amount == distributed_principal`).
- [ ] Archive v5 via `archive_escrow` (terminal status required).
- [ ] Keep the v5 WASM hash on file for audit; do not delete the instance.

### Rollback

If a v6 deployment is found to be broken **before any investor re-deposits**:

- v5 is still authoritative. No investor funds are at risk on v5.
- Investigate the v6 issue off-chain.
- Re-deploy a corrected v6 and repeat Phase 2–4.

If a v6 deployment is found to be broken **after investors have re-deposited**:

- Stop further re-deposits immediately.
- Determine whether the bug is recoverable on v6 (most bugs are) or whether a fresh v6 redeploy is needed.
- Investors who have re-deposited on v6 will need to repeat the re-deposit on the corrected v6. This is the strongest reason to invest in `scripts/verify_deployment.sh` — running it before announcing the v6 address catches most issues at Phase 2.

---

## Compatibility test plan

For every release that touches per-investor keys:

1. Deploy at `SCHEMA_VERSION = 6+`; exercise `init`, `fund`, `fund_with_commitment`, `settle`, `claim_investor_payout`, `refund`.
2. Verify `DataKey::InvestorContribution(addr)` is in **persistent** storage (use Soroban RPC `getLedgerEntries` to inspect).
3. Verify TTL on a single persistent entry can be extended without affecting any other persistent entry (call `bump_ttl` for `addr1`; assert `addr2`'s TTL is unchanged).
4. Verify absence-of-key semantics: a never-funded address returns `0` for contribution, base yield for `effective_yield`, `0` for `claim_not_before`, `false` for `claimed`.
5. Verify `migrate(5)` returns `EscrowError::NoMigrationPath` (code 92); confirm typed error rather than silent no-op.

---

## Consequences

- Reviewers can approve any PR that **only** changes the implementation of a per-investor key's read/write path without requiring a `SCHEMA_VERSION` bump, as long as the key's XDR shape and absence-default semantics are preserved.
- `SCHEMA_VERSION` remains a reliable signal: a stored version of `5` means per-investor keys are still in instance storage and the instance has not migrated; a stored version of `6+` means per-investor keys are in persistent storage.
- Storage-growth tests act as regression guards for the per-investor key footprint (see ADR-007 `Rule 4` and the storage-growth regression suite in `escrow/src/tests/`).
- Operators gain a clean redeploy path with explicit phases instead of a hidden, error-prone in-place migration.
- Operators accept the cost of explicit redeployment and re-deposit coordination. For long-lived escrows with hundreds of investors, this is a one-time cost that buys indefinite scalability.

---

## Rejected alternatives

- **In-place `migrate` that copies instance per-investor entries to persistent.** Rejected: Soroban does not allow enumerating instance keys by discriminant, so there is no safe on-chain way to find all per-investor entries to copy.
- **Off-chain per-investor ledger with on-chain hash anchors.** Rejected: reintroduces oracle trust; complicates dispute resolution; makes `sweep_terminal_dust` correctness dependent on an external system.
- **Bump `SCHEMA_VERSION` to 6 but keep per-investor keys in instance storage.** Rejected: this is the status quo ante and is the problem this ADR is fixing.
- **Side table keyed by a small investor ID counter.** Rejected: investors expect to be addressed by their Stellar `Address`; introducing an opaque counter breaks SDK ergonomics and the `fund(addr, ...)` API.
- **Shadow-write during transition (write both instance and persistent).** Rejected: doubles storage cost during the transition window; reconciliation is non-trivial if a host function fails after one write but before the other.