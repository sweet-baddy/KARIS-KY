# Issue: Add Fee Entrypoint and Deduct Protocol Fee at Settlement

**Type:** Feature
**Status:** Backlog, ready for review and assignment
**Priority:** High
**Related:** [ADR-003: Settlement Flow](../adr/ADR-003-settlement-flow.md), [Pro-rata Payout Mathematics](../escrow-pro-rata.md), [Escrow Data Model](../escrow-data-model.md), [Escrow Events](../escrow-events.md)

## Description

Add a protocol-fee configuration entrypoint and make settlement apply the
configured fee deterministically. The fee must be calculated in token base
units, deducted from the settlement yield/coupon at settlement time, transferred
to the configured treasury, and reflected in every payout calculation and
settlement snapshot that investors or integrations use.

The fee configuration must have an explicit unit and range, be protected by the
correct authority, and follow a clear mutability policy. The recommended model
is basis points (`0..=10_000`) over the gross yield coupon, with the fee
configuration set or changed by the admin before settlement and frozen once
settlement begins. Existing escrows with no configured fee must remain
backward-compatible with a zero-fee default.

The repository contains partial fee plumbing: `get_fee_percentage`,
`ProtocolFeeCollected`, and fee-related settlement/payout calculations are
present in `escrow/src/lib.rs`, but no complete public configuration path or
consistent tested storage model is present. The current fee references need to
be completed and reviewed as one accounting change.

## Current Behavior

The escrow already stores a configured treasury and computes investor payout
values from principal plus yield. Settlement changes the escrow to settled and
emits settlement events, while investor claims use the settlement state and
pro-rata math.

There are references to a `FeePercentage` storage key, a
`get_fee_percentage` getter, and a `ProtocolFeeCollected` event. However, no
`DataKey::FeePercentage` declaration, initialization path, or admin setter was
found, and no focused fee tests establish the intended behavior. The settlement
code attempts to transfer a fee and the payout code attempts to subtract it,
but the partial implementation is not a complete usable fee entrypoint.

## Steps to Reproduce

1. Build or type-check the escrow workspace.
2. Inspect the `FeePercentage` references in `escrow/src/lib.rs` and attempt to
   configure a nonzero protocol fee through the public contract API.
3. Observe that there is no fee configuration entrypoint or documented init
   parameter that an operator can invoke.
4. If the incomplete fee code is compiled, fund an escrow, configure a
   settlement treasury, and call `settle()`.
5. Compare the settlement amount, treasury balance, investor payout result, and
   emitted events with a zero-fee settlement.
6. Observe that fee behavior cannot be configured or reliably asserted, and
   that the payout path contains an inconsistent `gross_coupon` reference even
   though the local coupon calculation is named `coupon`.

A regression fixture should use a principal and yield that produce a fee larger
than zero after integer division, for example a 5% yield and a 10% protocol fee,
then verify both the treasury transfer and investor net payout.

## Expected Behavior

- An authorized admin can set the protocol fee through a documented entrypoint
  such as `set_fee_percentage(fee_bps)` before settlement.
- The entrypoint rejects negative values, values above `10_000` basis points,
  and calls after settlement has started. It emits a configuration audit event.
- The configured fee is readable through `get_fee_percentage`; absent or legacy
  configuration means zero fee.
- At settlement, the contract computes gross coupon using the existing checked
  yield math, computes `fee = floor(gross_coupon * fee_bps / 10_000)`, and
  computes `net_coupon = gross_coupon - fee`.
- The fee is transferred exactly once to the immutable configured treasury. If
  the transfer fails, settlement fails atomically and does not mark the escrow
  settled or emit a successful settlement event.
- Investor payout calculations use the net settlement pool, so the protocol fee
  cannot be paid to investors and cannot reduce principal.
- Partial settlement, automatic yield distribution snapshots, reinvestment,
  dust accounting, and read APIs use the same fee policy and do not double-charge
  or double-count the fee.
- Rounding is deterministic: intermediate multiplication uses checked arithmetic
  and the final fee amount is floored in token base units. Any remainder stays
  in the contract and follows the documented terminal-dust policy.
- A zero-fee configuration produces the same results and events as the legacy
  behavior, apart from any explicitly versioned metadata.

## Actual Behavior

There is no usable public fee configuration flow. `FeePercentage` is referenced
without a complete declared/configured storage contract, and no focused tests
cover range validation, authorization, settlement transfer, rounding, or
partial settlement. The payout implementation also refers to `gross_coupon`
where the local variable is `coupon`, making the current fee path internally
inconsistent.

Consequently, operators cannot reliably set a protocol fee, investors cannot
rely on a documented net-payout calculation, and integrations have no stable
configuration or audit event contract for fee collection.

## Proposed Solution

### 1. Define fee configuration

Add `DataKey::FeePercentage` as an instance key storing fee basis points and
initialize it to zero for backward compatibility. Add a public admin entrypoint,
for example:

```rust
pub fn set_fee_percentage(env: Env, fee_bps: i64)
```

Require the current admin authorization, validate `0 <= fee_bps <= 10_000`,
and reject updates once the escrow is settled or a partial settlement has
started. Decide and document whether the value is immutable after init or may
be updated while status is open; do not allow an update that changes the fee
basis between partial settlement steps.

Emit a versioned event containing the invoice ID, actor, previous fee, new fee,
and ledger timestamp. Preserve the existing getter and document the basis-point
unit in the SDK and read API.

### 2. Centralize settlement accounting

Create one checked helper for gross coupon, fee amount, net coupon, and settle
pool calculation. Use it from full settlement, partial settlement, investor
payout computation, automatic yield distribution, and reinvestment paths. Fix
the current `coupon`/`gross_coupon` naming mismatch rather than duplicating
slightly different fee formulas.

Apply the fee only to yield/coupon, never to principal. For each partial
settlement, define whether the fee is assessed on the newly settled portion and
record cumulative fee state if needed for auditability. Ensure a full settlement
after partial settlements cannot charge an earlier portion again.

### 3. Transfer and events

Use the existing balance-checked funding-token transfer helper to send the fee
to `DataKey::Treasury`. Require a configured treasury, fail closed on an
insufficient balance or token transfer error, and rely on transaction atomicity
so no settled state survives a failed fee transfer.

Emit `ProtocolFeeCollected` once per fee transfer with gross coupon, fee basis
points, fee amount, and net coupon. Extend settlement/receipt/indexer schemas
as needed so consumers can distinguish gross yield, protocol fee, net yield,
principal, and any terminal dust.

### 4. Tests and documentation

Add unit and integration tests for setter authorization and validation, zero
fee, maximum fee, rounding, treasury transfer, insufficient balance, full and
partial settlement, repeated settlement rejection, payout equality with the
net pool, and legacy instances. Add property tests for principal conservation
and fee bounds. Update the pro-rata math, data model, read API, event schema,
operator runbook, and SDK documentation.

## Environment Context

- **Repository:** `KARIS-KY` Soroban escrow contracts
- **Platform:** Stellar Soroban
- **Language/toolchain:** Rust, Cargo, Soroban SDK, SEP-41 token interface
- **Settlement actor:** configured SME; fee configuration actor should be the
  current admin unless governance later supersedes it
- **Fee unit:** proposed basis points, where `10_000` means 100%
- **Fee base:** proposed gross yield coupon only; principal must remain intact
- **Destination:** immutable `DataKey::Treasury` address
- **Current partial implementation:** fee getter, event, and settlement/payout
  references exist, but configuration/storage/tests are incomplete
- **Compatibility:** missing fee configuration defaults to zero for existing
  escrows
- **Accounting constraints:** checked `i128` arithmetic, token base-unit
  flooring, atomic settlement, partial-settlement idempotency, and audit events

## Acceptance Criteria

- [ ] `DataKey::FeePercentage` is declared, documented, initialized with a
      backward-compatible zero default, and exposed through `get_fee_percentage`.
- [ ] An admin-only fee entrypoint validates `0..=10_000` basis points and emits
      an auditable configuration event containing old and new values.
- [ ] Fee changes are rejected after settlement or partial settlement begins,
      according to the documented immutability policy.
- [ ] The fee is calculated as `floor(gross_coupon * fee_bps / 10_000)` using
      checked arithmetic and is applied to coupon only, never principal.
- [ ] Net coupon and investor payout calculations use one shared accounting
      helper with no inconsistent duplicate formulas or undefined variables.
- [ ] Full settlement transfers the exact fee once to the configured treasury
      and emits one `ProtocolFeeCollected` event with gross, fee, and net values.
- [ ] A failed treasury/token transfer leaves the escrow funded, does not mark
      settlement complete, and emits no successful fee or settlement event.
- [ ] Zero-fee and legacy escrows preserve prior settlement and payout results.
- [ ] Partial settlement applies the fee to the newly settled portion exactly
      once and produces correct cumulative fee and payout accounting.
- [ ] Automatic yield distribution, reinvestment, dust sweeping, settlement
      receipts, and read APIs all agree on gross yield, fee, net yield, and
      principal values.
- [ ] Tests cover authorization, invalid ranges, zero/max fee, rounding,
      overflow, insufficient treasury-transfer balance, and repeated calls.
- [ ] Integration tests verify investor net payouts and treasury balances for
      full and partial settlement scenarios, including overfunding.
- [ ] Property tests prove principal conservation, nonnegative bounded fees,
      and `net_coupon <= gross_coupon` for valid configurations.
- [ ] Event, read API, operator, pro-rata, and SDK documentation describe the
      fee unit, timing, destination, rounding, and legacy default.
- [ ] Security review finds no unresolved high-severity issue involving admin
      fee changes, treasury substitution, double charging, or principal loss.

## Assignment Notes

Before assignment, confirm whether the fee is set at initialization or by a
pre-settlement admin entrypoint, whether governance approval is required for
fee changes, the treatment of partial settlement and terminal dust, and the
required indexer/SDK receipt fields. The first implementation milestone should
be the fee storage/configuration contract and a shared accounting helper,
followed by settlement transfer and payout integration.
