# Protocol Fee Basis and Collection Mechanism

**Last updated**: 2024
**Status**: Documented and tested

## Overview

The Liquifact Escrow contract includes an optional protocol fee mechanism that is applied to yield, never to principal. This document clarifies the fee basis, application point, and accounting procedures to ensure transparent treasury reconciliation.

## Fee Basis: Yield Only

The protocol fee is deducted from the **gross yield coupon**, not from investor principal.

### Fee Formula

At settlement time, for a given settlement:

```
gross_coupon = settlement_basis * yield_bps / 10_000

if fee_percentage > 0:
  fee_amount = gross_coupon * fee_percentage / 10_000  (floor division)
  net_coupon = gross_coupon - fee_amount
else:
  net_coupon = gross_coupon
  fee_amount = 0
```

Where:
- `settlement_basis`: The amount of principal being settled (for full settlement, equals `funded_amount`; for partial settlement, equals the partial amount)
- `yield_bps`: The annual yield in basis points (from `InvoiceEscrow::yield_bps`)
- `fee_percentage`: The protocol fee in basis points (from `DataKey::FeePercentage`)
- `gross_coupon`: The full yield coupon before fee
- `fee_amount`: The portion going to treasury
- `net_coupon`: The portion available for investor distribution

### Example Calculation

```
Principal (funded_amount):         1,000,000 tokens
Yield rate (yield_bps):            800 bps (8.0%)
Settlement basis:                  1,000,000 tokens (full settlement)

Gross coupon = 1,000,000 * 800 / 10,000 = 80,000 tokens

Fee percentage:                    500 bps (5% of gross coupon)
Fee amount = 80,000 * 500 / 10,000 = 4,000 tokens

Net coupon = 80,000 - 4,000 = 76,000 tokens

Distribution to investors:         76,000 tokens
Transfer to treasury (fee):        4,000 tokens
```

**Key invariant**: Investor principal remains **1,000,000 tokens** (unchanged). The fee is entirely from yield.

## Application Point

The fee is applied **during settlement** (`settle_impl` entrypoint):

1. **Trigger**: When an escrow transitions from status 1 (funded) to status 2 (settled)
2. **Calculation**: Performed once at settlement; not recalculated at claim time
3. **Transfer**: The fee amount is immediately transferred to the treasury account via `transfer_funding_token_with_balance_checks`
4. **Event**: A `ProtocolFeeCollected` event is emitted with full accounting details
5. **Investor claim**: Later, when investors call `claim_investor_payout`, they receive their pro-rata share of the net coupon (after fee has already been deducted)

### Partial Settlements

For partial settlements (where the escrow settles less than the full funded amount):

```
Funded amount:          1,000,000 tokens
Partial settlement amt: 500,000 tokens (from the 1,000,000)

Gross coupon = 500,000 * 800 / 10,000 = 40,000 tokens
Fee = 40,000 * 500 / 10,000 = 2,000 tokens
Net coupon = 40,000 - 2,000 = 38,000 tokens

This settlement contributes 2,000 tokens to treasury.
Remaining unsettled principal (500,000) will incur a separate fee if settled later.
```

## Configuration and Immutability

- **Set at**: `init` time by the admin
- **Storage**: `DataKey::FeePercentage` (instance storage)
- **Mutation**: Immutable after initialization; cannot be changed by any subsequent call
- **Value**: In basis points (bps); 0 = no fee, 500 = 5%, 10,000 = 100%
- **Query**: `get_fee_percentage()` returns the configured value or 0 if unset

This immutability prevents surprise fee increases mid-escrow-lifecycle and simplifies escrow accounting.

## Treasury Accountability

### Direct Token Transfer

The fee amount is transferred to the treasury address in a **separate token transfer** that:
- Is independent of investor principal/yield distributions
- Uses the same SEP-41 funding token as the escrow
- Is transferred from the escrow contract to the treasury account
- May be swept to treasury via `sweep_terminal_dust` if the escrow enters a terminal state before the transfer occurs

### Event Emission

Each settlement that incurs a fee emits a `ProtocolFeeCollected` event containing:

```rust
ProtocolFeeCollected {
  invoice_id: Symbol,
  gross_coupon: i128,              // Full yield before fee
  fee_percentage: i64,             // Fee rate in bps
  fee_amount: i128,                // Amount transferred to treasury
  net_coupon: i128,                // Remaining yield for investors
}
```

This event allows off-chain systems (dashboards, reconciliation scripts, audit tools) to:
- Verify fee calculations without on-chain queries
- Reconcile treasury receipts against escrow settlements
- Detect mismatches in accounting
- Generate auditable fee reports

### No Fee Event When Fee is Zero

If `fee_percentage == 0` or `gross_coupon == 0`, the `ProtocolFeeCollected` event is **not emitted** and **no transfer occurs**. The settlement proceeds with the full coupon available for investors.

## Investor Impact

Investors see the fee effect only on yield:

1. **Principal**: Always remains exactly what they contributed (never touched)
2. **Yield before settlement**: May be quoted as full gross coupon in communications
3. **Yield after settlement**: The claim amount equals `(contribution * net_coupon) / total_principal`
4. **Transparency**: The fee basis and amount are publicly available via events and read-only entrypoints

## Relationship to Other Mechanisms

### Settlement Status vs. Fee Collection

- Escrow reaches **funded** (status 1): No fee collected yet
- Escrow transitions to **settled** (status 2): Fee is collected and transferred during this transition
- Investors **claim** (after settlement): They receive their pro-rata share of the already-reduced net coupon

### Yield Tiering

If the escrow uses tiered yield (different investors get different yields), the fee is applied **per tier/investor**:

```
Each investor's gross_coupon = contribution * investor_yield_bps / 10_000
Each investor's fee = gross_coupon * fee_percentage / 10_000
Each investor's net_coupon = gross_coupon - fee
```

The fee percentage is uniform across all investors, but the absolute fee amount varies per investor based on their effective yield.

### Legal Hold and Disputes

- If a legal hold is active, settlement is blocked (no fees can be collected)
- If a dispute pause is active, settlement is blocked (no fees can be collected)
- Once a legal hold is cleared and settlement proceeds, the fee mechanism applies normally

## Validation and Invariants

### Pre-Settlement Checks

Before applying the fee, the contract ensures:

1. Escrow status is 1 (funded)
2. No legal hold is active
3. No dispute pause is active
4. Maturity (if set) has been reached
5. `fee_percentage` is non-negative and ≤ 10,000 bps

### Post-Settlement Guarantees

After a settlement:

1. `fee_amount > 0` ⟹ Treasury received the tokens (or transaction reverted)
2. `net_coupon ≤ gross_coupon` (always; fee is never negative)
3. `fee_amount = gross_coupon - net_coupon` (identity; event allows verification)
4. Principal accounts remain unchanged
5. Escrow status transitions to 2 (settled) for full settlement, or remains 1 for partial

### Overflow Protection

All arithmetic uses checked operations (`checked_mul`, `checked_div`) to prevent integer overflow. If overflow is detected during fee calculation, the settlement reverts with `EscrowError::ComputePayoutArithmeticOverflow`.

## Treasury Operations

### Sweep Terminal Dust

After settlement, any remaining contract balance (not used for investor claims) can be swept to treasury via `sweep_terminal_dust`:

```
max_sweep_amount = min(requested_amount, MAX_DUST_SWEEP_AMOUNT, contract_balance)
treasury_receives = max_sweep_amount
```

This includes any unclaimed yield or fee residuals from rounding.

### Off-Chain Reconciliation

The treasury operator can verify fees using:

1. **Event log**: Query all `ProtocolFeeCollected` events
2. **Calculate total expected**: Sum all `fee_amount` values from events
3. **Verify on-chain**: Call `get_treasury()` and verify token balance via SEP-41 `balance(treasury)`

## Configuration Example

### At Init

```rust
// Fee percentage: 500 bps (5%)
// This is an example; actual init parameters may vary.
let fee_percentage_bps = 500i64;

client.init(
  &admin,
  &invoice_id,
  &sme,
  &amount,
  &yield_bps,     // e.g., 800 (8%)
  &maturity,
  &funding_token,
  &registry,
  &treasury,
  /* other params */
);

// After init, the fee is locked in. Query with:
let configured_fee = client.get_fee_percentage(); // Returns 500
```

### Reading Current Fee

```rust
let fee = client.get_fee_percentage(); // Returns configured value or 0
```

## Related Entrypoints and Events

### Entrypoints

- `get_fee_percentage()`: Read the configured fee (read-only, no auth required)
- `settle()`: Apply fees and transition to settled (SME-authorized)
- `claim_investor_payout()`: Claim pro-rata share of net coupon (investor-authorized)

### Events

- `ProtocolFeeCollected`: Emitted at settlement when fee > 0
- `EscrowSettled`: Emitted when settlement completes (includes gross coupon)
- `ProtocolFeeCollected`: Contains full accounting (gross, fee %, fee amount, net)

## Testing and Verification

### Test Coverage

The `fee_collection.rs` module provides tests for:

1. `get_fee_percentage()` returns configured value
2. Fee is deducted from yield, not principal
3. Treasury receives the correct amount
4. Event emission with full accounting
5. Partial settlement fee calculation
6. Zero fee behavior (no event, no transfer)
7. Immutability of fee percentage after init

### Manual Verification

To verify fee behavior in a deployed escrow:

```
1. Query escrow: get_escrow() → funded_amount, yield_bps, status
2. Query fee: get_fee_percentage() → fee_bps
3. Calculate expected fee:
   gross_coupon = funded_amount * yield_bps / 10_000
   fee_amount = gross_coupon * fee_bps / 10_000
4. Query events: Look for ProtocolFeeCollected
5. Verify event fields match calculation
6. Query treasury balance before and after settlement
```

## Glossary

- **Principal**: The amount an investor contributes to the escrow (immutable once recorded)
- **Yield**: The return on principal over the escrow term (annual percentage, specified in basis points)
- **Gross coupon**: The full yield payout before fee deduction
- **Net coupon**: The yield payout after fee deduction; what investors receive
- **Fee**: The protocol fee, expressed in basis points (bps), applied to gross coupon
- **Basis points**: 1 bps = 0.01% (10,000 bps = 100%)
- **Settlement basis**: The amount of principal on which yield is calculated (full or partial amount)
- **Treasury**: The recipient of protocol fees

## References

- `escrow/src/settlement.rs`: `settle_impl()` - fee calculation and transfer
- `escrow/src/admin.rs`: `get_fee_percentage_impl()` - fee query
- `escrow/src/contract.rs`: `get_fee_percentage()` - public API
- `escrow/src/storage.rs`: `DataKey::FeePercentage` - storage key
- `escrow/src/tests/fee_collection.rs`: Comprehensive fee tests
