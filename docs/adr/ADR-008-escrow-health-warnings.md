# ADR-008: Escrow Health Warning System

**Status:** Accepted  
**Date:** 2026-07-28  
**Issue:** #231  
**Refs:** `escrow/src/lib.rs` — `EscrowHealthWarning`, `compute_and_emit_health_warning`, `check_escrow_health`

---

## Context

Off-chain indexers and integrators need real-time visibility into escrow risk states. Examples include:

- **Low funding** + **close to maturity**: funding target may not be met before settlement deadline.
- **Past maturity** but **unfunded**: escrow entered a legally ambiguous state.
- **Funding stalled**: no deposits received for weeks despite available capacity.

The contract currently has no mechanism to signal these conditions. Without such signals, risk teams discover problems reactively (post-maturity) rather than proactively.

---

## Decision

### 1. New Event Type: `EscrowHealthWarning`

Define a new **non-blocking metadata event** emitted when escrow enters a risk state:

```rust
#[contractevent]
pub struct EscrowHealthWarning {
    #[topic]
    pub name: Symbol,                    // "hlth_wrn"
    #[topic]
    pub invoice_id: Symbol,
    pub warning_type: u32,               // Code 4001–4004
    pub funded_amount: i128,
    pub funding_target: i128,
    pub funded_ratio_bps: i64,           // Basis points
    pub time_to_maturity_secs: i64,      // May be negative
    pub recorded_at_ledger_timestamp: u64,
}
```

### 2. Warning Type Codes

| Code | Condition | Emitted When |
|------|-----------|--------------|
| 4001 | `LowFundingRatio` | `funded_ratio_bps < 5000` (< 50%) when open or any status |
| 4002 | `CloseToMaturity` | `0 < time_to_maturity_secs < 86400` (< 1 day) with healthy funding |
| 4003 | `OverMaturity` | `time_to_maturity_secs < 0` and `status == 0` (open) and `unfunded` |
| 4004 | `FundingClosedAfterMaturity` | `closed_at_ledger_timestamp >= maturity` (snapshot after maturity date; BUG-013) |
| 0 | No warning | Default / no risk condition detected |

### 3. Health Computation Logic

**Funded ratio (bps):**
```
funded_ratio_bps = (funded_amount / funding_target) * 10_000
```
Clamped to `i64::MAX` on overflow; returns `10_000` if `funding_target == 0`.

**Time to maturity (seconds):**
```
time_to_maturity_secs = maturity - now
```
Returns `i64::MAX` if `maturity == 0` (no constraint); negative if past maturity.

**Determination:**
- If `time_to_maturity_secs < 0` AND `status == 0` AND `funded_amount < funding_target` → **4003** (OverMaturity).
- Else if `0 <= time_to_maturity_secs < 86400` (1 day):
  - If `funded_ratio_bps < 5000` → **4001** (LowFundingRatio).
  - Else → **4002** (CloseToMaturity).
- Else if `funded_ratio_bps < 5000` AND `status == 0` → **4001** (LowFundingRatio, open, no immediate time pressure).
- Else → **0** (No warning).

### 4. Emission Points

Health warnings are emitted at three key transitions:

1. **`fund_impl()`** – After `EscrowFunded` event, check health of the updated escrow.
2. **`settle()`** – After `EscrowSettled` event, check health for audit trail.
3. **`claim_investor_payout()`** – After `InvestorPayoutClaimed` event, check health.

Emission is **non-blocking**: if any condition is met, emit the event; otherwise, emit nothing (code 0 is silent).

### 5. Public Read-Only Endpoint

Provide `check_escrow_health() -> (u32, i64, i64)` for off-chain polling:

```rust
pub fn check_escrow_health(env: Env) -> (u32, i64, i64) {
    // Returns (warning_type, funded_ratio_bps, time_to_maturity_secs)
    // No auth required; pure read operation.
}
```

### 6. Storage & Backward Compatibility

- **No new persistent storage keys** required; warnings are events only.
- **Additive event type**: existing contract instances can upgrade without redeploy.
- **No schema version bump**: `SCHEMA_VERSION` remains unchanged.
- **Non-blocking guarantee**: warnings never prevent valid escrow operations.

---

## Rationale

### Why events, not storage?

- **Storage bloat:** every escrow health check would mutate state, consuming ledger quota.
- **Audit trail:** events are immutable and indexed off-chain; more queryable than stored snapshots.
- **Decoupling:** risk logic is decoupled from state transitions; warnings can be disabled or tuned without code changes.

### Why non-blocking?

- A warning is a signal, not a gate. An underfunded escrow is still valid; the escrow may recover with more funding.
- Blocking on warnings risks stranding funds if thresholds are misconfigured.
- Risk teams take action outside the contract (e.g., notify SME, extend maturity).

### Why those thresholds?

- **50% funding ratio**: industry standard for "materially underfunded" (inverse of 50/50 split).
- **1 day to maturity**: sufficient time for most operational responses (notify, inject funds, request extension).
- **Maturity already passed**: legal/financial ambiguity; settled/funded status must be clarified urgently.

---

## Consequences

### Immediate

- Off-chain indexers gain real-time visibility into escrow risk states.
- Risk teams can react proactively (alert SME, initiate recovery).
- Audit trail includes health signals at each state transition.

### Future Enhancements

- **Configurable thresholds**: admin may adjust warning ratios per escrow instance.
- **Per-investor health**: warn when an investor's commitment lock expires soon.
- **Scheduled health checks**: emit warnings at fixed intervals (e.g., weekly) to catch stalled funding.
- **Integration with legal hold**: auto-trigger legal hold if OverMaturity threshold crossed.

---

## Compatibility

### Existing Instances

- Upgrade without redeploy: the new `EscrowHealthWarning` event type is additive.
- Old instances continue operating; indexers will see warnings on the new event stream only after upgrade.

### New Instances

- Deployed with health warnings enabled by default.
- Indexers must consume the new event type to surface risk alerts.

---

## Testing

### Unit Tests

- `test_health_warning_low_funding_ratio`: verify 4001 emission.
- `test_health_warning_close_to_maturity`: verify 4002 emission.
- `test_health_warning_low_funding_close_to_maturity`: verify 4001 takes priority.
- `test_health_warning_over_maturity_unfunded`: verify 4003 emission.
- `test_no_health_warning_healthy_escrow`: verify code 0 when healthy.
- `test_no_health_warning_no_maturity_constraint`: verify no time-based warnings when maturity == 0.
- `test_no_health_warning_settled_escrow`: verify settled escrows emit no warnings.
- `test_health_warning_emitted_during_fund`: verify event is published.

### Integration Tests

- Verify health warnings are emitted alongside existing state-change events.
- Verify multiple warnings do not break transaction atomicity.
- Verify `check_escrow_health()` returns correct metrics without auth.

### Fuzz Tests

- Random state transitions + maturity advances; verify warning type is always in range [0, 4004].
- Extreme values (i128::MIN / MAX funded amounts) do not panic or overflow.

---

## References

- [ADR-001: State Model](docs/adr/ADR-001-state-model.md)
- [ADR-002: Auth Boundaries](docs/adr/ADR-002-auth-boundaries.md)
- [Escrow Error Messages](docs/escrow-error-messages.md)
- [Issue #231](https://github.com/karis-ky/escrow-contracts/issues/231)
