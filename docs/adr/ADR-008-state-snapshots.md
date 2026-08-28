# ADR-008: State Snapshot Recovery and Rollback

**Status:** Accepted  
**Date:** 2026-07-28  
**Authors:** karis-ky governance  

## Context

The escrow contract is an autonomous state machine that handles material events (funding, settlement, withdrawal, claims). In production, unforeseen bugs, governance policy changes, or user errors may require the admin to roll back the contract to a prior known-good state without redeploying.

Prior solutions required **contract redeployment** or **complex off-chain migrations**. A governance-controlled snapshot and revert mechanism reduces operational risk by providing a **single-ledger recovery path** that preserves audit trails (events remain unchanged on-chain).

## Decision

We implement two new **admin-only** entrypoints:

1. **`create_state_snapshot(name: String)`** — Captures the full escrow state (`InvoiceEscrow`) and metadata (timestamp, admin, ledger sequence) under a named key.
2. **`revert_to_snapshot(name: String)`** — Overwrites the current escrow state with a previously saved snapshot.

### Constraints & Scope

#### What Is Reverted
- **Only** the main `InvoiceEscrow` struct (status, funded_amount, yield_bps, maturity, SME address, etc.)
- Metadata: snapshot name, creation timestamp, admin address

#### What Is NOT Reverted (Intentional)
- **Per-investor persistent entries:** contributions, effective yields, claim-not-before times, claimed flags, allowlist status
- **Rationale:** Investor entries are in persistent storage with independent TTLs. Reverting them would require enumerating all investors (impossible in Soroban; addresses are not enumerable). Selective revert of the main escrow state allows emergency recovery without breaking per-investor consistency guarantees.

#### Implications of Partial Revert
- **Consistency risk:** If an investor funded after snapshot was taken, their contribution remains in storage but the escrow's `funded_amount` reverts. Off-chain audits must reconcile investor-side records with the reverted escrow state.
- **Pro-rata impact:** Investor claims are computed from `get_contribution(investor) / snapshot.total_principal`. A revert may change `funded_amount` and `total_principal`, altering effective claim shares. Governance must communicate reversions to all participants.
- **Mitigation:** Snapshots are **emergency tools**, not routine undo. Operators should:
  - Document the reason and timestamp of every revert.
  - Notify affected investors immediately.
  - Conduct manual audits to verify off-chain share calculations.
  - Consider governance approval before revert in high-stakes scenarios.

## Implementation

### Storage

```rust
pub enum DataKey {
    // ...
    /// Snapshot metadata: name, timestamp, admin, sequence
    StateSnapshotMetadata(Symbol),
    /// Full escrow state at snapshot time
    StateSnapshotState(Symbol),
}

pub struct SnapshotMetadata {
    pub name: Symbol,
    pub created_at_ledger_timestamp: u64,
    pub created_at_ledger_sequence: u32,
    pub created_by: Address,
}

pub struct StateSnapshot {
    pub escrow: InvoiceEscrow,
}
```

### Constraints

- **Name validation:** 1–32 UTF-8 bytes, alphanumeric + `_` only (matches Soroban Symbol constraints).
- **Maximum snapshots:** [`MAX_STATE_SNAPSHOTS = 16`] per escrow instance to bound storage.
- **Admin-only:** Both entrypoints require `escrow.admin.require_auth()`.
- **Immutability:** Snapshots are immutable once created. New snapshots overwrite by name (simplified recovery).

### Events

```rust
pub struct StateSnapshotCreated {
    #[topic] pub name: Symbol,                          // Contract name
    #[topic] pub invoice_id: Symbol,
    #[topic] pub snapshot_name: Symbol,
    pub created_at_ledger_timestamp: u64,
    pub created_by: Address,
    pub escrow_snapshot: InvoiceEscrow,
}

pub struct StateSnapshotReverted {
    #[topic] pub name: Symbol,
    #[topic] pub invoice_id: Symbol,
    #[topic] pub snapshot_name: Symbol,
    pub reverted_at_ledger_timestamp: u64,
    pub reverted_by: Address,
    pub prior_escrow_state: InvoiceEscrow,
    pub new_escrow_state: InvoiceEscrow,
}
```

Both events are emitted to provide an immutable on-chain audit trail of all snapshot operations.

### Error Codes

- **`EscrowError::InvalidSnapshotName` (170):** Name validation failed (empty, too long, invalid charset).
- **`EscrowError::SnapshotStorageCapacityReached` (171):** Exceeded [`MAX_STATE_SNAPSHOTS`] limit.
- **`EscrowError::SnapshotNotFound` (172):** Snapshot name does not exist (on revert).

## Security Analysis

### Threat Model

1. **Admin key compromise:** A malicious admin could revert the escrow repeatedly to undo legitimate investor claims or settlements. **Mitigation:** Governance must use a multisig admin so no single key can unilaterally revert. See [ADR-002](ADR-002-auth-boundaries.md).
2. **Stale snapshots:** An operator could accidentally revert to a snapshot that is months old, losing recent state. **Mitigation:** Snapshots are stored with metadata (timestamp, admin); operators must verify before calling revert. Document snapshot creation and deletion procedures in operational runbooks.
3. **Partial revert inconsistency:** Investor contributions are not reverted, leading to off-chain share calculation errors. **Mitigation:** This is a known design tradeoff. Operators must manually audit and reconcile off-chain ledgers after revert.

### Invariants Preserved

- **Event immutability:** All events (funding, settlement, claims) remain on-chain and are never mutated or reverted. Off-chain indexers see the full history.
- **Per-investor data isolation:** Investor persistent entries are independent from the escrow state. No cross-address invariant is broken by partial revert.
- **Authorization:** Both snapshot and revert operations require `admin.require_auth()` before any state mutation.

## Operational Guidance

### When to Use Snapshots

- **Contract bug:** A recently discovered bug in settlement math caused incorrect payouts. Revert to before settlement and patch.
- **User error:** Admin mistakenly changed the maturity or funding target. Revert to correct state and re-apply changes.
- **Compliance hold recovery:** A hold was cleared incorrectly. Revert to before the hold was cleared, reinstate the hold, and conduct audit.

### When NOT to Use Snapshots

- **Investor dispute:** If investor A claims their contribution was miscounted, do not revert. Instead, conduct an audit, document findings, and apply targeted fixes if needed.
- **Testing and development:** Use Soroban test utilities (`env.mock_all_auths()`, etc.) instead. Snapshots are for production recovery.
- **Performance tuning:** Snapshots are not caches; they are emergency recovery tools.

### Operational Checklist

1. **Before revert:**
   - Verify the snapshot name and timestamp.
   - Obtain governance approval if required by policy.
   - Notify all affected investors of the potential revert.
   - Backup all off-chain state (indexer database, investor records, settlement ledgers).

2. **During revert:**
   - Call `revert_to_snapshot(name)` with admin authorization.
   - Verify the [`StateSnapshotReverted`] event was emitted.
   - Query `get_escrow()` to confirm the state matches the snapshot.

3. **After revert:**
   - Conduct a full audit of investor contributions and claims.
   - Reconcile off-chain ledgers with on-chain state.
   - Document the revert reason, timestamp, and involved parties.
   - Update risk controls and governance policy if needed to prevent recurrence.

## Alternatives Considered

### 1. Full On-Chain Undo via Transaction Logs
**Rejected:** Requires storing the entire transaction history on-chain, which is prohibitively expensive in Soroban. Manual snapshots are more controllable.

### 2. Automatic Checkpoints on Every State Mutation
**Rejected:** Would double storage overhead and make every entrypoint slower. Snapshots are manual and governance-controlled, suitable for emergency recovery.

### 3. Revert Per-Investor Data as Well
**Rejected:** Soroban cannot enumerate addresses, so there is no way to discover all investors to revert. Partial revert is the only feasible approach.

## Testing

Comprehensive test coverage includes:
- Snapshot creation with valid/invalid names
- State capture and restoration
- Multiple snapshots
- Unauthorized access prevention
- Not-initialized and not-found error cases
- Event emission verification

See `escrow/src/tests/snapshots.rs` for the full test suite.

## Migration and Deployment

- **Additive feature:** Snapshots are stored under new DataKey variants. No migration is needed for existing deployments.
- **Schema version:** Remains at 6; snapshots are an optional feature and do not require data layout changes.
- **Backward compatibility:** Deployments without snapshots created will gracefully handle missing snapshot keys (`unwrap_or(None)`).

## References

- [ADR-001: State Model](ADR-001-state-model.md) — Escrow status transitions
- [ADR-002: Auth Boundaries](ADR-002-auth-boundaries.md) — Role-based authorization
- [ADR-007: Storage Key Evolution](ADR-007-storage-key-evolution.md) — Additive key policy
- `escrow/src/lib.rs` — Implementation
- `docs/OPERATOR_RUNBOOK.md` — Production operational procedures
- `docs/escrow-security-checklist.md` — Security best practices
