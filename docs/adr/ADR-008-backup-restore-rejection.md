# ADR-008: On-Chain Backup / Restore — Decision and Safe Alternatives

**Status:** Accepted  
**Date:** 2026-07-25  
**Refs:** `escrow/src/lib.rs` — `InvoiceEscrow`, `DataKey`, `SCHEMA_VERSION`; ADR-001; ADR-007;
`docs/escrow-indexer.md`; `backend/backup_escrow_state.py`

---

## Context

A feature request proposed two new on-chain entrypoints:

- `backup_state(backup_name)` — create a named snapshot of full escrow state, stored in persistent
  storage; max 10 backups per escrow with oldest auto-pruned.
- `restore_from_backup(backup_name)` — admin-only; overwrite live contract state from a named
  snapshot, with a confirmation flag.

The stated goal was **point-in-time restore capability** for disaster recovery.

---

## Decision

**On-chain `restore_from_backup` is rejected.** `backup_state` is also rejected as a standalone
feature because it only has value if restore is possible.

The safe alternative — an **off-chain backup script** — is adopted instead. See
`backend/backup_escrow_state.py`.

---

## Reasons for rejection

### 1. `restore_from_backup` breaks financial invariants (critical)

ADR-001 defines status transitions as strictly forward-only:

```
0 (open) → 1 (funded) → 2 (settled)
                       → 3 (withdrawn)
         → 4 (cancelled)
```

No entrypoint moves status backward. This invariant is load-bearing: `settle`, `withdraw`, and
`refund` transfer real tokens out of the contract account. Restoring a stale state snapshot after
those transfers would create a permanent desync:

- The contract storage would report `status = 0` (open) or `status = 1` (funded).
- The on-chain token balance of the contract would already be reduced by the distributed principal.
- Subsequent `withdraw` or `refund` calls would attempt to transfer tokens the contract no longer
  holds, either panicking at the balance-sufficiency check or — if the contract received
  unrelated token deposits — moving a different depositor's funds to the SME or an investor.

This is a fund-theft vector, not a recoverable inconsistency.

### 2. State and token balance are not jointly snapshotted

Soroban does not provide atomic snapshots of both contract storage and the account's token balance.
`backup_state` can only capture `DataKey` entries. It cannot capture:

- The actual SEP-41 token balance held by the contract account (owned by the token contract, not
  this contract's storage).
- `DistributedPrincipal` accuracy relative to tokens already sent.
- Per-investor `InvestorClaimed` and `InvestorRefunded` markers that prevent double-payouts.

A restore that replays storage without replaying token movements produces a contract whose
accounting is irrecoverably wrong.

### 3. It undermines the audit and compliance trail

The attestation append log, legal hold history, and `CollateralRecordedEvt` events exist
specifically so auditors and regulators can reconstruct the authoritative history of an escrow.
A restore that rolls back these fields silently erases compliance records. Combined with legal-hold
semantics (ADR-004), a restore could reactivate a hold that was lawfully cleared, or clear a hold
that governance specifically imposed.

### 4. Storage cost in Soroban instance storage

ADR-007 and the README explicitly bound per-instance storage to control rent cost and entry size.
Storing up to 10 full `InvoiceEscrow` snapshots — plus metadata, backup names, and an index — in
instance storage would multiply the entry size by an order of magnitude, working directly against
the ADR-007 goal of decoupling instance footprint from investor cardinality.

### 5. `InvoiceEscrow` is intentionally non-`Clone`

The `Clone` derive is explicitly omitted from `InvoiceEscrow` (documented in the README and in
source comments) to prevent accidental full-state duplication in hot paths. A backup/restore
mechanism is exactly the pattern that omission is designed to prevent.

### 6. `require_auth` alone is not sufficient mitigation

Admin-gating `restore_from_backup` does not make it safe. The admin role is already the highest
privilege in the system; if the admin key is compromised, the attacker can call any admin
entrypoint. Adding a new entrypoint that can revert the contract to a pre-settlement state gives
a compromised admin key the ability to cause unbounded financial harm (re-run funding, re-attempt
withdrawal after tokens are already out) rather than merely freezing the escrow via `set_legal_hold`.

---

## Safe alternatives already in the contract

| Recovery need | Existing mechanism |
|---------------|--------------------|
| Freeze all state-changing ops | `set_legal_hold(true)` |
| Rotate compromised admin key | `propose_admin` + `accept_admin` (two-step, both keys must sign) |
| Fix misconfigured maturity | `update_maturity` (open state only) |
| Fix misconfigured funding target | `update_funding_target` (open state only) |
| Lower over-generous investor cap | `lower_max_unique_investors` (open state only) |
| Schema migration after WASM upgrade | `migrate` entrypoint + `upgrade` |
| Cancel a live escrow before funded | `cancel_funding` (open state only, legal-hold gated) |

### Point-in-time state reconstruction (safe path)

The contract emits a typed event for every state transition (see `docs/escrow-events.md` and
`docs/EVENT_SCHEMA.md`). An event-sourcing indexer can reconstruct the exact state of any escrow
at any past ledger without storing snapshots on-chain. This is the canonical approach for
point-in-time observability on Soroban.

For operational snapshots (e.g. backup before a planned upgrade), the off-chain script at
`backend/backup_escrow_state.py` queries all read-only entrypoints and writes a timestamped JSON
file. This captures the full observable state without touching contract storage or requiring any
auth.

---

## Adopted alternative: off-chain backup script

`backend/backup_escrow_state.py` provides:

- A point-in-time JSON snapshot of all read-only escrow state (escrow summary, funding snapshot,
  attestations, collateral, legal hold, version, per-investor data for a supplied address list).
- Named output files with ledger sequence and UTC timestamp embedded in the filename.
- A restore-readiness check that validates the snapshot is internally consistent (status matches
  distributed principal, etc.) — useful before an upgrade, not as a rollback mechanism.
- No contract auth required; no on-chain storage consumed.

**Retention policy:** snapshots are plain files. Operators manage retention via standard filesystem
or object-storage lifecycle rules (e.g. keep last 10, or keep 30 days). The script does not
auto-prune; pruning policy belongs in the operator's backup infrastructure, not in contract logic.

---

## Consequences

- No new `DataKey` variants, no `SCHEMA_VERSION` bump, no `migrate` path required.
- Operators who need a pre-upgrade state record run `backup_escrow_state.py` against their RPC
  endpoint before deploying a new WASM.
- Point-in-time audit queries go through the event indexer (see `docs/escrow-indexer.md`).
- Any future proposal for on-chain state rollback must address the token-balance desync problem
  described in Reason 1 above before it can be considered.

## Rejected implementation variants

- **Read-only `backup_state` without restore:** rejected because it consumes significant
  instance-storage budget for data the indexer already covers, with no unique value.
- **Backup to persistent storage (not instance):** rejected for the same token-desync reason;
  storage location does not change the financial invariant violation.
- **Admin confirmation flag on `restore_from_backup`:** rejected; a boolean confirmation
  parameter does not eliminate the fund-theft vector described in Reason 1.
- **Time-locked restore (timelocked `restore_from_backup`):** rejected; a timelock only adds
  latency to an unsafe operation. The underlying invariant violation is unchanged.
