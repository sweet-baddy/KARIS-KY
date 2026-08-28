# Design Document: Escrow Health Warnings & Delta-Encoded Snapshots

**Date:** 2026-07-28  
**Features:** #231 (Health Warnings), #217 (Delta Snapshots)  
**Authors:** Implementation  
**Status:** Design Phase

---

## Executive Summary

This document outlines the implementation approach for two complementary escrow contract features:

1. **Escrow Health Warning System (#231)**: Emit typed warning events when an escrow enters unhealthy states (e.g., low funding levels close to maturity).
2. **Delta-Encoded State Snapshots (#217)**: Store snapshots as deltas from the previous version to reduce on-chain storage consumption.

Both features follow the additive-key policy defined in ADR-007 and preserve backward compatibility.

---

## Feature #231: Escrow Health Warning System

### Goals

- Provide off-chain indexers and integrators early visibility into escrow risk states.
- Emit typed, structured events (not log strings) for deterministic parsing.
- Minimal on-chain overhead; warnings are metadata only.
- Non-blocking: warnings do not prevent valid escrow transitions.

### Design

#### 1. Health Warning Event

Define a new event type to signal risk conditions:

```rust
#[contractevent]
pub struct EscrowHealthWarning {
    #[topic]
    pub name: Symbol,                    // "health_warn"
    #[topic]
    pub invoice_id: Symbol,
    pub warning_type: u32,               // Warning category code
    pub funded_amount: i128,             // Current funded principal
    pub funding_target: i128,
    pub funded_ratio_bps: i64,           // Basis points: (funded_amount / funding_target) * 10000
    pub time_to_maturity_secs: i64,      // Seconds until maturity (negative if past)
    pub recorded_at_ledger_timestamp: u64,
}
```

#### 2. Warning Type Codes

New typed codes added to escrow-error-messages.md for reference (non-blocking, informational):

| Code | Condition | Trigger |
|------|-----------|---------|
| 4001 | `LowFundingRatio` | `funded_ratio_bps < 5000` (50%) when close to maturity |
| 4002 | `CloseToMaturity` | `time_to_maturity_secs < 86400` (< 1 day) |
| 4003 | `OverMaturity` | `time_to_maturity_secs < 0` (past maturity, unfunded) |
| 4004 | `FundingStalled` | No new funding in last N blocks (~7 days) and `funded_ratio < funding_target` |

#### 3. Health Check Logic

Add a helper function to compute health metrics:

```rust
fn compute_escrow_health(
    escrow: &InvoiceEscrow,
    now: u64,
) -> (u32, i64, i64) {
    // Returns (warning_type, funded_ratio_bps, time_to_maturity_secs)
    
    let funded_ratio_bps = if escrow.funding_target > 0 {
        ((escrow.funded_amount as i128 * 10_000) / escrow.funding_target) as i64
    } else {
        10_000_i64
    };
    
    let time_to_maturity_secs = if escrow.maturity > 0 {
        (escrow.maturity as i64) - (now as i64)
    } else {
        i64::MAX // No maturity constraint
    };
    
    let warning_type = if time_to_maturity_secs < 0 && escrow.status == 0 {
        4003 // OverMaturity
    } else if time_to_maturity_secs >= 0 && time_to_maturity_secs < 86400 {
        if funded_ratio_bps < 5000 {
            4001 // LowFundingRatio
        } else {
            4002 // CloseToMaturity
        }
    } else if funded_ratio_bps < 5000 && escrow.status == 0 {
        4001 // LowFundingRatio (open, time-independent)
    } else {
        0 // No warning
    };
    
    (warning_type, funded_ratio_bps, time_to_maturity_secs)
}
```

#### 4. Emission Points

Emit health warnings at key transitions where risk assessment is valuable:

- **On fund/fund_with_commitment:** After escrow state is updated, if a warning condition exists.
- **On settle:** If the escrow is funding-locked or underfunded.
- **On claim_investor_payout:** If any remaining escrow risk state warrants a warning.
- **Optional: Standalone health check entrypoint** (read-only, call to inspect current state without funding).

#### 5. Storage & Backward Compatibility

- **No new persistent storage keys** required; warnings are events only (off-chain indexed).
- **Additive event type**: existing contract instances can upgrade without redeploy.
- **Optional read-only entrypoint** for status inspection:
  ```rust
  pub fn get_escrow_health(env: Env) -> (u32, i64, i64) { ... }
  ```

### Implementation Notes

1. **Idempotency**: Multiple emissions of the same warning type in one transaction are acceptable; indexers deduplicate by (invoice_id, warning_type, block_height).
2. **Non-blocking**: A health warning never prevents valid escrow operations.
3. **Threshold tuning**: The thresholds (50% funding ratio, 1 day to maturity, etc.) are policy and can be adjusted in future versions.
4. **Maturity == 0 handling**: When maturity is not configured, time-based warnings are skipped.

---

## Feature #217: Delta-Encoded State Snapshots

### Goals

- Reduce on-chain storage for repeated snapshots by storing deltas instead of full copies.
- Maintain backward compatibility with existing snapshot reads.
- Preserve immutability of the first snapshot (FundingCloseSnapshot).
- Enable efficient auditing of snapshot evolution.

### Design

#### 1. Snapshot Delta Structure

Define a new structure to represent incremental changes:

```rust
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotDelta {
    /// Unique ID of this delta (monotonically increasing).
    pub delta_id: u32,
    
    /// Timestamp of the ledger when this delta was recorded.
    pub recorded_at: u64,
    
    /// Previous delta ID this one is based on (or 0 for the baseline/full snapshot).
    pub based_on_delta_id: u32,
    
    /// Funded amount change (signed delta).
    pub funded_amount_delta: i128,
    
    /// New maturity (0 if unchanged; negative values reserved for "remove").
    pub maturity: i64,
    
    /// New status (255 if unchanged).
    pub status: u8,
    
    /// New admin address (None if unchanged).
    pub admin: Option<Address>,
    
    /// New beneficiary/SME address (None if unchanged).
    pub sme_address: Option<Address>,
}
```

#### 2. Storage Keys

Add two new optional keys (per ADR-007, additive):

```rust
// In DataKey enum:
/// Full snapshot or None; baseline for delta chain.
/// Set once when funding transitions to status 1; immutable thereafter.
FullSnapshot,

/// Head of the delta chain: points to the latest applied delta.
/// Absent ⇒ no deltas (only full snapshot).
SnapshotDeltaChain,

/// Per-delta storage: mapping delta_id -> SnapshotDelta.
/// Keyed as (delta_id) in the new SnapshotDelta(u32) variant.
SnapshotDelta(u32),
```

#### 3. Snapshot Capture & Delta Logic

**At first funding transition (status 0 → 1):**
- Capture and store the full `FundingCloseSnapshot` as before under `DataKey::FundingCloseSnapshot`.
- Optionally, also store a baseline full snapshot under `DataKey::FullSnapshot` for delta reference.
- Initialize delta chain head as `None` (no deltas yet).

**On subsequent state updates (status 1 → 2, beneficiary rotation, etc.):**
- Compute the delta from the current escrow state vs. the full snapshot + all applied deltas.
- Store the new delta under `DataKey::SnapshotDelta(new_delta_id)`.
- Update `SnapshotDeltaChain` to point to the new delta.

**Reconstruction (for reads):**
```rust
fn reconstruct_snapshot(env: &Env) -> Option<InvoiceEscrow> {
    // 1. Try to load the full snapshot.
    if let Some(full) = env.storage().instance().get::<_, InvoiceEscrow>(&DataKey::FullSnapshot) {
        // 2. Walk the delta chain from head and apply each delta in order.
        let mut current = full;
        if let Some(delta_id) = env.storage().instance().get::<_, u32>(&DataKey::SnapshotDeltaChain) {
            current = apply_delta_chain(env, current, delta_id)?;
        }
        Some(current)
    } else {
        None
    }
}
```

#### 4. Delta Application

```rust
fn apply_delta_chain(env: &Env, mut escrow: InvoiceEscrow, head_delta_id: u32) -> Option<InvoiceEscrow> {
    let mut current_delta_id = head_delta_id;
    let mut deltas = Vec::new();
    
    // Load all deltas in the chain (walk backwards via based_on_delta_id).
    while current_delta_id != 0 {
        if let Some(delta) = env.storage().instance().get::<_, SnapshotDelta>(&DataKey::SnapshotDelta(current_delta_id)) {
            deltas.push(delta);
            current_delta_id = delta.based_on_delta_id;
        } else {
            return None; // Broken chain; corruption detected.
        }
    }
    
    // Apply deltas in chronological order (reverse the loaded order).
    deltas.reverse();
    for delta in deltas {
        if delta.funded_amount_delta != 0 {
            escrow.funded_amount = escrow.funded_amount
                .checked_add(delta.funded_amount_delta)
                .ok()?;
        }
        if delta.maturity >= 0 {
            escrow.maturity = delta.maturity as u64;
        }
        if delta.status != 255 {
            escrow.status = delta.status as u32;
        }
        if let Some(admin) = delta.admin {
            escrow.admin = admin;
        }
        if let Some(sme) = delta.sme_address {
            escrow.sme_address = sme;
        }
    }
    
    Some(escrow)
}
```

#### 5. Backward Compatibility

**Read operations:**
- `get_escrow()` still reads `DataKey::Escrow` (unchanged behavior for now).
- Optional new read path: `get_escrow_reconstructed()` to verify delta chain is correct.
- During transition period, both full snapshot and deltas coexist; reconciliation tests ensure consistency.

**Migration path (non-blocking):**
- Existing instances keep storing the full escrow under `DataKey::Escrow`.
- Opt-in: new instances (or migrated ones) use delta encoding for post-snapshot updates.
- No forced redeploy; gradual rollout via versioning.

#### 6. Storage Key Evolution

**Per ADR-007:**
- New keys (`FullSnapshot`, `SnapshotDeltaChain`, `SnapshotDelta(u32)`) are additive.
- Do **not** increment `SCHEMA_VERSION`.
- Existing instances continue using `DataKey::Escrow`; new instances can opt into delta encoding.
- Reconciliation tests verify delta chain matches reconstructed state.

### Implementation Strategy

#### Phase 1: Snapshot Delta Structure (minimal blocker)
1. Define `SnapshotDelta` struct.
2. Add new `DataKey` variants.
3. Write snapshot capture/storage helpers.

#### Phase 2: Delta Application & Reconstruction
1. Implement `apply_delta_chain()` and `reconstruct_snapshot()`.
2. Add tests for delta application and round-trip consistency.

#### Phase 3: Integration with State Transitions
1. Update `fund_impl`, `settle`, `withdraw`, `rotate_beneficiary` to optionally emit deltas.
2. Add a configuration flag (or policy) to enable delta encoding per instance.

#### Phase 4: Testing & Verification
1. Reconciliation tests: verify delta chain output matches full snapshot.
2. Storage comparison tests: measure delta overhead vs. full snapshots.
3. Fuzz tests: random state transitions + delta chain validation.

---

## Integration: Health Warnings + Delta Snapshots

### Synergy

Both features are independent but complementary:

- **Health warnings** emit metadata about current escrow risk; they rely on reading the latest escrow state.
- **Delta snapshots** optimize how that state is stored and reconstructed.

When both are enabled:
1. Health check logic reads the reconstructed escrow (via `reconstruct_snapshot()` if deltas are in use).
2. Health warning events are emitted alongside any state-changing transition.
3. Off-chain indexers consume both warnings (for alerting) and snapshot deltas (for state history).

### No Conflicts

- Health warnings do **not** depend on delta encoding; they work with the existing snapshot model.
- Delta encoding is completely additive and does **not** require health warnings to function.
- Each feature can be deployed and tested independently.

---

## Testing Strategy

### Feature #231 (Health Warnings)

1. **Unit tests** for health metric computation:
   - Low funding ratio detection.
   - Time-to-maturity calculations.
   - Status-specific thresholds.

2. **Integration tests**:
   - Fund near target → emit `CloseToMaturity` or `LowFundingRatio`.
   - Funding passes maturity timestamp → emit `OverMaturity`.
   - Settle with warnings active → verify no blocking.

3. **Event parsing tests**:
   - Verify warning event structure matches schema.
   - Off-chain indexer simulation: parse and classify warnings.

### Feature #217 (Delta Snapshots)

1. **Unit tests** for delta application:
   - Apply single delta.
   - Apply chain of N deltas.
   - Verify round-trip: full snapshot → delta chain → reconstructed snapshot.

2. **Integration tests**:
   - Fund → emit first snapshot.
   - Settle → emit delta.
   - Withdraw → emit delta.
   - Verify reconstructed state matches full escrow state.

3. **Storage efficiency tests**:
   - Measure bytes consumed by full snapshot vs. delta chain.
   - Test chain length limits (e.g., max 50 deltas before compaction).

4. **Fuzz tests** (proptest):
   - Random state transitions.
   - Verify delta chain integrity after each transition.
   - Compare reconstructed vs. full for consistency.

---

## Error Handling

### Health Warnings

- **No new errors**: warnings are non-blocking and never cause a transaction to fail.
- Graceful degradation: if metrics cannot be computed (e.g., underflow), skip emission.

### Delta Snapshots

- **Broken chain detection**: if `based_on_delta_id` points to a missing delta, fail reconstruction with a new error code.
  - `SnapshotDeltaChainCorrupted = 165`
  - `SnapshotDeltaNotFound = 166`
- **Overflow in delta application**: checked arithmetic for `funded_amount_delta`.

---

## Deployment & Rollout

### Phase 1: Canary (Health Warnings)
1. Merge health warning event and basic emission logic.
2. Deploy to testnet; validate event structure and parsing.
3. Gather indexer feedback; adjust thresholds if needed.

### Phase 2: Canary (Delta Snapshots)
1. Merge delta structures and reconstruction logic.
2. Add `get_escrow_reconstructed()` for opt-in testing.
3. Deploy to testnet; verify reconciliation with full snapshots.
4. Measure storage savings in simulated high-traffic scenarios.

### Phase 3: Production
1. Both features enabled by default on new instances.
2. Existing instances can opt-in via admin entrypoint (if desired).
3. No forced migration; backward compatibility preserved.

---

## Future Enhancements

### Health Warnings
- Configurable thresholds via admin settings.
- Per-investor health metrics (e.g., claim lock expiry warnings).
- Scheduled health check (e.g., weekly) to catch stalled funding.

### Delta Snapshots
- Automatic compaction: after N deltas, collapse chain into a new full snapshot.
- Partial deltas: store only changed fields (space optimized).
- Snapshot rollback: revert to a prior state via admin (for recovery scenarios).

---

## Appendix: Implementation Checklist

- [ ] Define `EscrowHealthWarning` event and warning codes.
- [ ] Implement `compute_escrow_health()` helper.
- [ ] Add health warning emissions in `fund_impl`, `settle`, `claim_investor_payout`.
- [ ] Define `SnapshotDelta` struct and new `DataKey` variants.
- [ ] Implement `apply_delta_chain()` and `reconstruct_snapshot()`.
- [ ] Integrate delta emission in state-changing transitions (optional flag).
- [ ] Write unit tests for both features.
- [ ] Write integration tests (happy path + edge cases).
- [ ] Add fuzz tests for delta chain integrity.
- [ ] Update ADR documents (ADR-008, ADR-009).
- [ ] Verify no compilation errors (`cargo build`, `cargo clippy`).
- [ ] Run full test suite and achieve 95% coverage.
- [ ] Update error messages documentation.

---

## References

- [ADR-001: State Model](docs/adr/ADR-001-state-model.md)
- [ADR-007: Storage Key Evolution](docs/adr/ADR-007-storage-key-evolution.md)
- [Escrow Error Messages](docs/escrow-error-messages.md)
- [Escrow Data Model](docs/escrow-data-model.md)
