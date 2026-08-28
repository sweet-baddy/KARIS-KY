# Investor Quick-Start Guide

**karis-ky** lets you fund tokenized invoices on Stellar and earn a fixed yield
when the invoice is settled. This guide walks you through the full lifecycle —
from picking an escrow to collecting your payout — in plain language.

---

## Table of contents

1. [What you need before you start](#1-what-you-need-before-you-start)
2. [Step 1 — Select an escrow](#step-1--select-an-escrow)
3. [Step 2 — Understand your yield](#step-2--understand-your-yield)
4. [Step 3 — Fund the escrow](#step-3--fund-the-escrow)
5. [Step 4 — Wait for settlement](#step-4--wait-for-settlement)
6. [Step 5 — Claim your payout](#step-5--claim-your-payout)
7. [Cancellation and refunds](#cancellation-and-refunds)
8. [FAQ](#faq)

---

## 1. What you need before you start

| Requirement | Details |
|-------------|---------|
| Stellar account | A funded Stellar address (`G...`) on the correct network (testnet or mainnet) |
| Funding token balance | The escrow's bound SEP-41 token (typically a stablecoin, e.g. USDC) |
| Contract address | The deployed `LiquifactEscrow` contract id for the invoice you want to fund |
| Allowlist approval _(if required)_ | Some escrows require admin pre-approval; check `is_allowlist_active` |

To check whether an escrow exists and is open for funding:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_escrow_summary
```

Look for `"status": 0` (open). If status is anything other than `0`, the escrow
is no longer accepting new deposits.

---

## Step 1 — Select an escrow

Each escrow is a separate contract instance that represents one invoice. The key
fields to check before committing capital are:

```
InvoiceEscrow {
  invoice_id       – unique identifier for the invoice
  amount           – original invoice face value
  funding_target   – how much is needed to close funding
  funded_amount    – how much has been deposited so far
  yield_bps        – base annual yield in basis points (500 = 5.00%)
  maturity         – earliest ledger timestamp when settlement is allowed
                     (0 means no time lock — can settle immediately when funded)
  status           – 0 = open, 1 = funded, 2 = settled, 3 = withdrawn, 4 = cancelled
}
```

**How to read the full state:**

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_escrow_summary
```

**What to check:**

- `status == 0` — still accepting deposits
- `funded_amount < funding_target` — room left to invest
- `maturity` — if > 0, settlement cannot happen before this ledger timestamp;
  use `docs/escrow-ledger-time.md` to convert to wall-clock time
- `is_allowlist_active` — if `true`, your address must be pre-approved
- `has_maturity_lock` — quick boolean summary of whether maturity is enforced

**Check the funding token:**

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_funding_token
```

Ensure you hold the returned token in your wallet before continuing.

---

## Step 2 — Understand your yield

### Base yield

Every escrow has a `yield_bps` set at deployment. This is expressed in
**basis points** (1 bps = 0.01%).

```
yield_bps = 500  →  5.00% annual yield
yield_bps = 850  →  8.50% annual yield
```

### Tiered yield (optional)

Some escrows offer a tiered yield ladder: commit to locking your payout claim
for a longer period and earn a higher rate on your first deposit. Tiers are
**immutable** — they are set at contract deployment and cannot change.

You can read the tier table off-chain through `get_escrow_summary` or look for
the `YieldTierTable` storage key. A typical ladder might look like:

| Min lock (seconds) | Yield (bps) |
|--------------------|-------------|
| 0                  | 500 (base)  |
| 2 592 000 (30d)    | 600         |
| 7 776 000 (90d)    | 750         |

> **Important:** tier selection happens on your **first** deposit only.
> Once your yield rate is locked in, it cannot be upgraded by depositing again.

### Payout formula

Your gross payout after settlement is calculated as:

```
coupon      = total_principal × effective_yield_bps / 10 000   (floor division)
settle_pool = total_principal + coupon
gross_payout = your_contribution × settle_pool / total_principal  (floor division)
```

Where `total_principal` is the `FundingCloseSnapshot.total_principal` — the
total amount recorded at the moment funding closed (immutable denominator).

**Example:**

| | |
|---|---|
| Funding target | 10 000 USDC |
| Total principal at close | 10 050 USDC (over-funded) |
| Your contribution | 1 000 USDC |
| Yield | 500 bps (5%) |

```
coupon       = 10 050 × 500 / 10 000 = 502 USDC
settle_pool  = 10 050 + 502 = 10 552 USDC
gross_payout = 1 000 × 10 552 / 10 050 ≈ 1 049 USDC  (floor)
```

You can get the authoritative on-chain figure at any time (no auth required):

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- compute_investor_payout \
  --investor <YOUR_STELLAR_ADDRESS>
```

---

## Step 3 — Fund the escrow

### Option A — Standard deposit (base yield)

Use `fund` for a straightforward deposit at the escrow's base yield. You can
top up your position later with additional calls to `fund`.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source <YOUR_SECRET_KEY> \
  -- fund \
  --investor <YOUR_STELLAR_ADDRESS> \
  --amount <AMOUNT_IN_BASE_UNITS>
```

**Amount is in token base units.** For USDC (7 decimals), 1 USDC = `10_000_000`.

### Option B — First deposit with commitment lock (tiered yield)

Use `fund_with_commitment` on your **first deposit only** to lock in a higher
tier yield. You commit to not claiming your payout until
`now + committed_lock_secs` has passed on the ledger.

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source <YOUR_SECRET_KEY> \
  -- fund_with_commitment \
  --investor <YOUR_STELLAR_ADDRESS> \
  --amount <AMOUNT_IN_BASE_UNITS> \
  --committed_lock_secs <LOCK_DURATION_IN_SECONDS>
```

> After your first tiered deposit, any follow-up deposits must use `fund`
> (not `fund_with_commitment`). Attempting a second `fund_with_commitment`
> call will fail with `TieredSecondDeposit` (error 108).

### Funding constraints to be aware of

| Constraint | Where to check | Error if violated |
|------------|---------------|-------------------|
| Minimum per deposit | `get_min_contribution_floor()` | `FundingBelowMinContribution` (101) |
| Max total from your address | `get_max_per_investor_cap()` | `InvestorContributionExceedsCap` (106) |
| Max unique investors | `get_max_unique_investors_cap()` | `UniqueInvestorCapReached` (107) |
| Allowlist | `is_investor_allowlisted(your_address)` | `InvestorNotAllowlisted` (104) |
| Funding deadline | `get_funding_deadline()` | `FundingDeadlinePassed` (164) |

### Confirm your deposit was recorded

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_contribution \
  --investor <YOUR_STELLAR_ADDRESS>
```

This should return your cumulative contributed amount. You can also check your
effective yield rate:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_investor_yield_bps \
  --investor <YOUR_STELLAR_ADDRESS>
```

---

## Step 4 — Wait for settlement

### What happens after funding closes

Once cumulative deposits reach `funding_target`, the escrow automatically
transitions to **status 1 (funded)**. No action is required from you at this
point.

A `FundingCloseSnapshot` is written at that moment and becomes the immutable
pro-rata denominator for all payout calculations. Over-funding (deposits that
pushed the total above the target) is included in `total_principal`, which can
only benefit your pro-rata share.

### The settlement trigger

Settlement is initiated by the **SME** (the invoice issuer), not investors.
The SME calls `settle()`, which requires:

1. `status == 1` (funded)
2. If `maturity > 0`: ledger timestamp ≥ `maturity`
3. No active legal hold

Once `settle()` succeeds, status transitions to **2 (settled)**.

### How to check if the escrow has settled

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_escrow
```

Look for `"status": 2`.

### How long will it take?

The timeline depends on two things:

1. **Maturity date** — if `has_maturity_lock` is `true`, settlement cannot
   happen before `maturity`. You can read the timestamp with `get_escrow` and
   convert ledger time to wall-clock time (see `docs/escrow-ledger-time.md`).
2. **SME action** — the SME must actively call `settle()` after maturity passes.
   Off-chain coordination and legal review may add additional time.

If the escrow has `maturity == 0` and reaches `status == 1`, the SME may settle
at any time. There is no minimum wait.

### Commitment locks

If you used `fund_with_commitment` with a non-zero lock, your payout claim is
additionally gated until `now >= InvestorClaimNotBefore`. Check your personal
gate:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_investor_claim_not_before \
  --investor <YOUR_STELLAR_ADDRESS>
```

`0` means no extra gate. Any non-zero value is a ledger timestamp — you cannot
claim before that timestamp, even if the escrow is already settled.

---

## Step 5 — Claim your payout

Once the escrow is settled (`status == 2`) and your commitment lock has expired
(if any), call `claim_investor_payout`:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source <YOUR_SECRET_KEY> \
  -- claim_investor_payout \
  --investor <YOUR_STELLAR_ADDRESS>
```

This marks your claim on-chain and emits an `InvestorPayoutClaimed` event.

> **Important:** this entrypoint records the claim as a ledger marker — it does
> **not** transfer tokens directly to your wallet. The actual payout distribution
> is handled off-chain by the karis-ky integration layer, which reads
> `InvestorPayoutClaimed` events and the `FundingCloseSnapshot` to compute and
> disburse each investor's gross payout. Confirm with the platform how and when
> the transfer will be completed.

### Verify your claim was recorded

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- is_investor_claimed \
  --investor <YOUR_STELLAR_ADDRESS>
```

Returns `true` once your claim is registered. Calling `claim_investor_payout`
again is a safe no-op — the contract is idempotent on this path.

### How much will you receive?

Use the on-chain view to get the exact gross payout figure before claiming:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- compute_investor_payout \
  --investor <YOUR_STELLAR_ADDRESS>
```

The figure uses floor (truncating) integer division. Any rounding residue across
all investors accumulates in the contract balance and is eventually swept by the
treasury via `sweep_terminal_dust` — it is not lost from the system.

---

## Cancellation and refunds

If the escrow is cancelled before reaching its funding target (admin calls
`cancel_funding()`), status transitions to **4 (cancelled)** and all investors
may reclaim their principal:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source <YOUR_SECRET_KEY> \
  -- refund \
  --investor <YOUR_STELLAR_ADDRESS>
```

You receive exactly your contributed amount back (no yield — the invoice was
never settled). You can verify your contribution before calling:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_contribution \
  --investor <YOUR_STELLAR_ADDRESS>
```

A second `refund` call will fail because your contribution is zeroed after the
first successful transfer.

---

## FAQ

**Q: Do I need to approve the contract to spend my tokens before calling `fund`?**

Yes. You must call `approve` (or `increase_allowance`) on the SEP-41 token
contract to allow the escrow contract to pull the funding amount. The contract
address is the contract id you are invoking.

---

**Q: Can I fund multiple times?**

Yes, with `fund`. You can top up your contribution as many times as you want
while the escrow is open (`status == 0`), as long as each deposit meets the
minimum contribution floor and your running total does not exceed
`max_per_investor_cap` (if configured).

If you used `fund_with_commitment` on your first deposit, all follow-up deposits
must use `fund`, not `fund_with_commitment`.

---

**Q: What happens if more than the target is deposited?**

Over-funding is allowed. If aggregate deposits exceed `funding_target`, the
excess is recorded in `FundingCloseSnapshot.total_principal`. Your pro-rata
share is calculated against that larger denominator, so your proportional
payout is slightly smaller — but the absolute yield applied to your contribution
is unchanged by over-funding.

---

**Q: What is a legal hold and what does it mean for me?**

A legal hold is a compliance gate that the admin can activate at any time. While
a hold is active, `fund`, `claim_investor_payout`, and `settle` are all blocked.
This is a governance safeguard; holds must be cleared by the admin (or a
successor admin) before the escrow resumes normal operation.

Check the current hold status:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_legal_hold
```

---

**Q: What if I miss the funding deadline?**

If `get_funding_deadline()` returns a non-zero timestamp and the current ledger
time has passed it, new deposits will be rejected with `FundingDeadlinePassed`
(error 164). Existing contributions are unaffected.

---

**Q: What does "basis points" (bps) mean?**

1 basis point = 0.01%. So:

| `yield_bps` | Annual yield |
|-------------|-------------|
| 100         | 1.00%       |
| 250         | 2.50%       |
| 500         | 5.00%       |
| 850         | 8.50%       |
| 1 000       | 10.00%      |

---

**Q: How do I know my money is safe while the escrow is open?**

Your deposited tokens are held by the escrow contract itself until either:
- The SME calls `withdraw` to pull funded liquidity (status → 3), or
- The escrow is cancelled and you call `refund` (status → 4).

The contract enforces SEP-41 balance-delta checks on every token transfer to
detect non-compliant tokens. Fee-on-transfer and rebasing tokens are explicitly
out of scope and will cause the transaction to fail at the transfer boundary
rather than silently draining funds.

---

**Q: Where can I find more technical details?**

| Topic | Document |
|-------|---------|
| State machine (all statuses and transitions) | [`docs/escrow-lifecycle.md`](escrow-lifecycle.md) |
| Pro-rata payout math | [`docs/escrow-pro-rata.md`](escrow-pro-rata.md) |
| Ledger time and maturity | [`docs/escrow-ledger-time.md`](escrow-ledger-time.md) |
| Tiered yield design | [`docs/adr/ADR-005-tiered-yield.md`](adr/ADR-005-tiered-yield.md) |
| Settlement flow design | [`docs/adr/ADR-003-settlement-flow.md`](adr/ADR-003-settlement-flow.md) |
| All read-only entrypoints | [`docs/escrow-read-api.md`](escrow-read-api.md) |
| Error code reference | [`docs/escrow-error-messages.md`](escrow-error-messages.md) |
| CLI simulation walkthrough | [`docs/escrow-sim-stellar-cli.md`](escrow-sim-stellar-cli.md) |
