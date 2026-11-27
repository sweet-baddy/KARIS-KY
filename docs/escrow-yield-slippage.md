# Real-Time Yield Slippage Detection

**Feature**: #232  
**Status**: Implemented  
**Last Updated**: 2026-07-25

## Overview

The yield slippage detection feature provides real-time monitoring of yield calculations during investor claim operations. When configured, the contract detects anomalies (e.g., actual yield differs significantly from expected) and emits warning events that allow admins and off-chain systems to review and audit potentially risky claims.

## Problem Statement

Without slippage detection, yield discrepancies between what investors expect and what they actually receive could go unnoticed:

- An investor deposits with an expected base yield (e.g., 8% = 800 bps)
- Due to tiered yields or other mechanisms, they receive a different effective yield (e.g., 12% = 1200 bps)
- The deviation (4% = 400 bps) could indicate misconfiguration or data errors
- The contract settles and pays out without alerting anyone

**Real-time slippage detection** catches these anomalies at claim time, enabling:
- Immediate administrative review of suspicious claims
- Off-chain auditing and alerting for downstream systems
- Data integrity assurance before funds transfer

## Design

### Configuration at Initialization

The `yield_slippage_threshold` is an optional parameter in [`LiquifactEscrow::init`](../escrow/src/lib.rs):

```rust
pub fn init(
    // ... other parameters ...
    yield_slippage_threshold: Option<i64>,  // New parameter
) -> InvoiceEscrow
```

**Validation Rules**:
- If provided, must be in range `0..=10_000` basis points (0% to 100%)
- `0` or `None` disables slippage detection entirely (default)
- Value is **immutable** after initialization
- Values at boundaries have specific behavior:
  - **0 (disabled)**: No slippage check runs; detection is entirely disabled
  - **1–9999 (enabled, variable strictness)**: Deviation must be strictly greater than threshold to trigger warning
  - **10,000 (100%, effectively disabled)**: Any deviation ≤ 10,000 bps passes; functionally disables detection

**Storage**:
- Stored in [`DataKey::YieldSlippageThreshold`](../escrow/src/lib.rs)
- Persisted as instance storage (accessible across all investor claims)

### Threshold Boundary Semantics

The threshold comparison is **strictly greater than** (`deviation > threshold`):

| Deviation | Threshold | Comparison | Action |
|-----------|-----------|------------|--------|
| 100 bps | 100 bps | 100 > 100? NO | No warning |
| 101 bps | 100 bps | 101 > 100? YES | Emit warning |
| 199 bps | 200 bps | 199 > 200? NO | No warning |
| 200 bps | 200 bps | 200 > 200? NO | No warning |
| 201 bps | 200 bps | 201 > 200? YES | Emit warning |

**Key implications**:
- Deviation must **exceed** the threshold, not equal it
- Off-by-one edge case: `threshold = 200` blocks warnings at exactly 200 bps but not 199 bps
- Minimum meaningful threshold: `1` bps (any deviation ≥ 2 bps triggers warning)
- Maximum meaningful threshold: `9_999` bps (10_000 bps would never trigger since max deviation is 9_200 bps)

### Practical Configuration Guidance

| Threshold | Use Case | Behavior |
|-----------|----------|----------|
| **0 or None** | No monitoring | Skips all checks; fastest code path |
| **1** | Paranoid monitoring | Any positive deviation triggers warning |
| **50–200** | Conservative | Catches small to moderate deviations |
| **500–1_000** | Moderate | Catches larger deviations (5%–10%) |
| **10_000** | Disabled (alternative) | Equivalent to 0 but explicit in config |

### Slippage Calculation

During [`LiquifactEscrow::claim_investor_payout`](../escrow/src/lib.rs), a real-time check runs:

```rust
detect_yield_slippage(env: &Env, investor: Address, escrow: &InvoiceEscrow)
```

**Computation**:

| Component | Source | Notes |
|-----------|--------|-------|
| **Expected Yield** | `escrow.yield_bps` | Configured base yield for the invoice |
| **Actual Yield** | `DataKey::InvestorEffectiveYield(investor)` or falls back to base | Investor-specific tier selection (if tiered yields) |
| **Deviation** | `(actual - expected).abs()` | Absolute difference in basis points |
| **Threshold** | `DataKey::YieldSlippageThreshold` | Configured max allowed deviation |

**Example**:

```
Escrow base yield:        800 bps (8%)
Investor effective yield: 1200 bps (12%)  [selected tiered rate]
Deviation:                400 bps (4%)

Threshold configured:     150 bps (1.5%)

Result: 400 > 150  ⟹  Emit YieldSlippageWarning event
```

### Event Emission

When `deviation > threshold`, a [`YieldSlippageWarning`](../escrow/src/lib.rs) event is emitted:

```rust
#[contractevent]
pub struct YieldSlippageWarning {
    #[topic]
    pub name: Symbol,                          // "yield_slip"
    #[topic]
    pub investor: Address,                     // Claiming investor
    #[topic]
    pub invoice_id: Symbol,                    // Invoice identifier
    pub expected_yield_bps: i64,               // Base yield
    pub actual_yield_bps: i64,                 // Effective yield for investor
    pub slippage_threshold_bps: i64,           // Configured threshold
    pub deviation_bps: i64,                    // Computed deviation
}
```

**Off-Chain Processing**:

Indexers and monitoring systems listen for `YieldSlippageWarning` events and can:
- Alert administrators immediately
- Trigger manual review workflows
- Log audit trails for compliance
- Implement custom business logic (e.g., hold funds pending approval)

## API Reference

### Public View Functions

#### `get_yield_slippage_threshold(env: Env) -> i64`

Returns the configured threshold (in basis points). Returns `0` if detection is disabled.

**Authorization**: None (read-only)

**Example**:
```
threshold = get_yield_slippage_threshold()
// Returns: 150  (if configured) or 0 (if disabled)
```

#### `get_investor_yield_slippage(env: Env, investor: Address) -> (i64, i64, i64)`

Computes and returns a tuple of:
1. Expected yield (base)
2. Actual yield (investor-specific)
3. Deviation (absolute difference)

Useful for off-chain preview of whether a claim would trigger a warning.

**Authorization**: None (read-only)

**Example**:
```
(expected, actual, deviation) = get_investor_yield_slippage(investor)
// Returns: (800, 1200, 400)
// Indicates a 4% deviation from the expected 8% base yield
```

### Entrypoint Changes

#### `claim_investor_payout(env: Env, investor: Address)`

Modified to call `detect_yield_slippage` before emitting the standard [`InvestorPayoutClaimed`](../escrow/src/lib.rs) event.

**Behavior**:
- If threshold is 0 or unset: skip slippage check (no warning emitted)
- If threshold > 0 and deviation ≤ threshold: emit only `InvestorPayoutClaimed`
- If threshold > 0 and deviation > threshold: emit both `YieldSlippageWarning` and `InvestorPayoutClaimed`

**Idempotency**: Second and subsequent claims do not re-emit events (claim state is already marked).

## Error Handling

New error codes:

| Code | Name | Context |
|------|------|---------|
| 162 | `YieldSlippageDetected` | Reserved for future use; current implementation emits events only |
| 163 | `YieldSlippageThresholdOutOfRange` | [`init`](../escrow/src/lib.rs) rejected a threshold outside `0..=10_000` |

## Usage Scenarios

### Scenario 1: Disabled Detection (Default)

```rust
// Initialize without threshold (or threshold = None)
client.init(
    &admin, "INVOICE_001", &sme, &1_000_000, &800, &0, &token, &None, &treasury,
    &None, &None, &None, &None, &None, &None, &None  // No threshold
);

// Later, when investor claims: no slippage check, no warning emitted
```

### Scenario 2: Conservative Slippage Monitoring

```rust
// Initialize with tight threshold (1% = 100 bps)
client.init(
    &admin, "INVOICE_001", &sme, &1_000_000, &800, &0, &token, &None, &treasury,
    &None, &None, &None, &None, &None, &None, &Some(100)  // 1% threshold
);

// If investor has tiered yield that deviates > 1%, warning emitted
```

### Scenario 3: With Tiered Yields

```rust
// Initialize with base yield 800 bps, tiers offering up to 1500 bps
let mut tiers = Vec::new(&env);
tiers.push_back(YieldTier { min_lock_secs: 0, yield_bps: 1200 });
tiers.push_back(YieldTier { min_lock_secs: 86400, yield_bps: 1500 });

client.init(
    &admin, "INVOICE_TIERED", &sme, &1_000_000, &800, &0, &token, &None, &treasury,
    &Some(tiers), &None, &None, &None, &None, &None, &Some(200)  // 2% threshold
);

// Investor who commits for 24h+ gets 1500 bps (7% deviation from 800 bps)
// 7% > 2% threshold ⟹ YieldSlippageWarning emitted at claim
```

## Testing

Comprehensive test suite in [`escrow/src/tests/yield_slippage.rs`](../escrow/src/tests/yield_slippage.rs):

### Initialization & Configuration Tests

- `init_accepts_zero_threshold`: Zero threshold disables detection
- `init_accepts_minimum_threshold`: Minimum non-zero threshold (1 bps) is valid
- `init_accepts_threshold_just_below_max`: 9_999 bps is valid
- `init_accepts_max_threshold`: Maximum threshold (10_000 bps = 100%) is valid
- `init_rejects_threshold_above_max`: Values > 10_000 bps are rejected with error 163
- `get_threshold_returns_configured_value`: Query function returns set value
- `get_threshold_returns_zero_when_unconfigured`: Unset threshold defaults to 0
- `get_investor_yield_slippage_returns_correct_values`: Slippage computation is correct

### Claim Behavior Tests

- `claim_with_disabled_slippage_detection`: Threshold = 0 skips checks (no event)
- `claim_with_matching_yields_no_warning`: Zero deviation never warns
- `claim_with_deviation_just_below_threshold_no_warning`: (199 <= 200) ⟹ No warning
- `claim_with_deviation_equal_to_threshold_no_warning`: (200 == 200) ⟹ No warning
- `claim_with_deviation_just_above_threshold_emits_warning`: (201 > 200) ⟹ Warning
- `claim_with_minimum_threshold_and_slight_deviation_emits_warning`: (2 > 1) ⟹ Warning at minimum
- `claim_with_maximum_threshold_no_warning`: 10_000 bps threshold never warns
- `claim_with_excessive_slippage_emits_warning`: Large deviation always warns
- `claim_idempotency_no_duplicate_warnings`: Repeated claims don't re-emit events

### Key Test Boundaries

| Test | Threshold | Deviation | Expected |
|------|-----------|-----------|----------|
| below | 200 | 199 | ✓ No warning |
| at | 200 | 200 | ✓ No warning |
| above | 200 | 201 | ✓ Warning |
| minimum | 1 | 2 | ✓ Warning |
| maximum | 10_000 | 9_200 | ✓ No warning |

Run tests:
```bash
cd escrow
cargo test yield_slippage --lib
```

For coverage report:
```bash
cargo llvm-cov --package karis_ky_escrow --lib yield_slippage
```

## Integration Checklist

- [x] Data key variant `YieldSlippageThreshold` added
- [x] Error codes 162–163 reserved and documented
- [x] Event type `YieldSlippageWarning` defined with all fields
- [x] `init` signature updated with optional threshold parameter
- [x] Validation in `init` to enforce `0..=10_000` bps range
- [x] Storage and persistence of threshold
- [x] Slippage detection logic in `claim_investor_payout`
- [x] Helper function `detect_yield_slippage` (private)
- [x] Public view function `get_yield_slippage_threshold`
- [x] Public view function `get_investor_yield_slippage`
- [x] Unit tests for all scenarios
- [x] Documentation

## Future Enhancements

1. **Admin Approval Workflow**: Require explicit admin approval before claims above threshold complete (currently events-only).
2. **Dynamic Threshold Updates**: Allow admin to adjust threshold post-initialization (currently immutable).
3. **Slippage History**: Track and query historical slippage events per investor.
4. **Configurable Actions**: Support halting claims, refunding, or redirecting payouts based on deviation severity.

## References

- **ADR-005**: [Tiered Yield](./adr/ADR-005-tiered-yield.md) (background on yield mechanisms)
- **Event Schema**: [EVENT_SCHEMA.md](./EVENT_SCHEMA.md)
- **Security Checklist**: [escrow-security-checklist.md](./escrow-security-checklist.md)
- **Pro-Rata Settlement**: [escrow-pro-rata.md](./escrow-pro-rata.md)
