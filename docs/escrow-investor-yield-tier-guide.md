# Investor-Facing Yield Tier Selection Guide

This guide explains how to select a yield tier when making your first deposit into a karis-ky escrow contract, and what happens to your earnings after you commit.

---

## What Are Yield Tiers?

When you invest in a karis-ky invoice escrow, the contract can offer you a choice of **yield tiers** based on how long you're willing to lock your capital. Higher yields reward longer commitments.

Each tier has two properties:
- **Min lock duration** (seconds): the minimum time your capital is locked before you can claim your payout
- **Yield rate** (basis points): the annual percentage yield offered at that tier (e.g., 800 bps = 8% per annum)

The tier table is **immutable** after the contract is initialized—it does not change for the lifetime of the invoice.

---

## The First-Deposit Discipline

**Critical rule:** You select your tier only on your **first deposit** into an escrow. After that, you cannot change your tier, even if you add more capital later.

### Why?

This prevents investors from gaming the system by:
- Starting at a low tier to participate early
- Switching to a higher tier once the invoice is well-funded
- Claiming the higher yield while keeping the lower lock period

### How the Selection Works

When you make your first deposit using `fund_with_commitment()`, you tell the contract how long you're willing to lock your capital (`committed_lock_secs`). The contract automatically selects the **best matching tier**:

- It finds the tier with the highest yield whose `min_lock_secs ≤ your committed_lock_secs`
- If you commit to a lock period shorter than any tier's minimum, you get the base yield (tier 0)
- Your effective yield and claim lock are written to storage and become **immutable** for all future deposits from you

### Subsequent Deposits

After your first deposit, all follow-on deposits must use `fund()` (not `fund_with_commitment()`):

```rust
// First deposit: select tier
client.fund_with_commitment(&investor_address, &amount_1, &lock_secs)

// Follow-on deposits: no tier selection
client.fund(&investor_address, &amount_2)
```

Your tier and claim lock from the first deposit apply to all future payouts.

---

## Claim Timestamps

### What Is the Claim Lock?

When you select a tier with a lock period, the contract records a **claim-not-before timestamp**:

```
claim_not_before = ledger.timestamp() + committed_lock_secs
```

This timestamp is set **at the moment of your first deposit**, using the Soroban ledger time at that block.

### When Can You Claim?

You can only claim your payout **after** the escrow is settled **and** your claim-not-before timestamp has passed:

```
can_claim = (escrow.status == Settled) AND (ledger.timestamp() >= claim_not_before)
```

If you try to claim before your lock expires, the contract returns error code **128** (`InvestorCommitmentLockNotExpired`).

### Ledger Time Skew

The ledger timestamp is observed by Soroban validators and may vary slightly between networks (e.g., testnet vs. mainnet). Always treat claim boundaries as integer seconds:

- If your lock is 1 week (604,800 seconds) and you deposit at timestamp `T`, you can claim at `T + 604,800` or later
- The contract uses `>=` comparison, so claiming exactly at your lock expiration succeeds

---

## Scenario 1: Conservative Investor (Low Yield, No Lock)

**Invoice:** $100,000 USDC, 8% base yield, tiers available: none (base yield only)

**Your decision:** First deposit of $10,000 USDC with 0-second lock (`committed_lock_secs = 0`)

**What happens:**

1. Contract matches you to the base yield (0 bps tier, 0-second lock)
2. Your effective yield is 8% (800 bps)
3. Your claim-not-before timestamp is set to `now + 0`, effectively immediately after settlement
4. You can make follow-on deposits at any time using `fund()`, all under the same 8% yield
5. After settlement, you can claim your pro-rata share right away

**Rust SDK example:**

```rust
use soroban_sdk::Address;

let investor = Address::random(&env);
let amount = 10_000_0000000i128;  // 10,000 USDC (7 decimals)
let lock_secs = 0u64;             // No lock

// First deposit with tier selection
let escrow_after_fund = client.fund_with_commitment(&investor, &amount, &lock_secs);
assert_eq!(escrow_after_fund.investor_effective_yield_bps, 800);

// After settlement, claim immediately (no lock delay)
client.settle();
let claim_result = client.claim_investor_payout(&investor);
```

---

## Scenario 2: Balanced Investor (Mid-Tier Yield, 30-Day Lock)

**Invoice:** $100,000 USDC, 8% base yield
**Tiers:**
- Tier 1: min_lock_secs = 604,800 (7 days) → 9% yield (900 bps)
- Tier 2: min_lock_secs = 2,592,000 (30 days) → 10% yield (1000 bps)
- Tier 3: min_lock_secs = 7,776,000 (90 days) → 11% yield (1100 bps)

**Your decision:** First deposit of $20,000 USDC with 30-day lock (`committed_lock_secs = 2,592,000`)

**What happens:**

1. Contract checks tiers in order: does 2,592,000 >= 604,800? Yes. Does 2,592,000 >= 2,592,000? Yes. Does 2,592,000 >= 7,776,000? No.
2. Best matching tier is Tier 2 (30 days, 10% yield)
3. Your effective yield is 10% (1000 bps)
4. Your claim-not-before = `now + 2,592,000` (30 days from deposit timestamp)
5. Any follow-on deposits get 10% yield automatically
6. You can only claim 30 days after your first deposit, regardless of when the escrow settles

**Rust SDK example:**

```rust
use soroban_sdk::{Symbol, Vec as SorobanVec};

let investor = Address::random(&env);
let amount = 20_000_0000000i128;  // 20,000 USDC
let lock_secs = 2_592_000u64;     // 30 days

// Define tier table
let mut tiers = SorobanVec::new(&env);
tiers.push_back(YieldTier {
    min_lock_secs: 604_800,      // 7 days
    yield_bps: 900i64,           // 9%
});
tiers.push_back(YieldTier {
    min_lock_secs: 2_592_000,    // 30 days
    yield_bps: 1000i64,          // 10%
});
tiers.push_back(YieldTier {
    min_lock_secs: 7_776_000,    // 90 days
    yield_bps: 1100i64,          // 11%
});

// Initialize escrow with tiers
let client = EscrowClient::new(
    admin, invoice_id, sme, amount_cap, base_yield, maturity, token, treasury,
    Some(tiers),
);

// First deposit: select tier
let escrow_after_fund = client.fund_with_commitment(&investor, &amount, &lock_secs);
assert_eq!(escrow_after_fund.investor_effective_yield_bps, 1000);
assert_eq!(escrow_after_fund.tier_lock_secs, 2_592_000);

// Later: try to claim too early (fails with error 128)
client.settle();
let claim_too_early = client.try_claim_investor_payout(&investor);
// claim_too_early = Err(ContractError(128)) — InvestorCommitmentLockNotExpired

// After 30 days, claim succeeds
env.ledger().with_mut(|l| {
    l.timestamp = original_timestamp + 2_592_000;
});
let claim_result = client.claim_investor_payout(&investor);  // OK
```

---

## Scenario 3: Aggressive Investor (Maximum Yield, 90-Day Lock)

**Invoice:** $100,000 USDC, 8% base yield
**Tiers (same as Scenario 2):**
- Tier 1: min_lock_secs = 604,800 (7 days) → 9% yield
- Tier 2: min_lock_secs = 2,592,000 (30 days) → 10% yield
- Tier 3: min_lock_secs = 7,776,000 (90 days) → 11% yield

**Your decision:** First deposit of $50,000 USDC with 90-day lock (`committed_lock_secs = 7,776,000`)

**What happens:**

1. Contract finds the matching tier: Tier 3 (90 days, 11% yield) is the best fit
2. Your effective yield is 11% (1100 bps)
3. Your claim-not-before = `now + 7,776,000` (90 days from deposit timestamp)
4. You make follow-on deposits with no tier change, all earning 11%
5. Your payout is locked for 90 days after your first deposit

**Rust SDK example:**

```rust
use soroban_sdk::Address;

let investor = Address::random(&env);
let amount = 50_000_0000000i128;  // 50,000 USDC
let lock_secs = 7_776_000u64;     // 90 days

// First deposit: maximum tier
let escrow_after_fund = client.fund_with_commitment(&investor, &amount, &lock_secs);
assert_eq!(escrow_after_fund.investor_effective_yield_bps, 1100);

// Additional deposits: same 11% yield
let amount_2 = 10_000_0000000i128;
client.fund(&investor, &amount_2);

// After settlement, calculate payout (11% of pro-rata share)
client.settle();

// Check that claim is still locked
let claim_before_lock = client.try_claim_investor_payout(&investor);
// claim_before_lock = Err(ContractError(128))

// Wait 90 days, then claim
env.ledger().with_mut(|l| {
    l.timestamp = original_timestamp + 7_776_000;
});
let payout = client.claim_investor_payout(&investor);
```

---

## Common Mistakes to Avoid

### ❌ Calling `fund_with_commitment()` Twice

```rust
// First deposit: OK
client.fund_with_commitment(&investor, &amount_1, &lock_secs);

// Second deposit: PANIC (error 108: TieredSecondDeposit)
client.fund_with_commitment(&investor, &amount_2, &lock_secs);
```

**Fix:** Use `fund()` for all deposits after the first:

```rust
client.fund_with_commitment(&investor, &amount_1, &lock_secs);  // Tier selection
client.fund(&investor, &amount_2);                             // Follow-on, no tier change
```

### ❌ Claiming Before the Lock Expires

```rust
let claim_result = client.try_claim_investor_payout(&investor);
// If claim_not_before timestamp hasn't passed:
// claim_result = Err(ContractError(128)) — InvestorCommitmentLockNotExpired
```

**Fix:** Check that the ledger time has advanced past your lock:

```rust
let now = env.ledger().timestamp();
let can_claim = now >= investor_claim_not_before;
if can_claim {
    client.claim_investor_payout(&investor);
}
```

### ❌ Assuming Claim Lock Starts After Settlement

```rust
// WRONG: thinking the lock starts after settle
client.fund_with_commitment(&investor, &amount, &lock_secs);
env.ledger().with_mut(|l| {
    l.timestamp = original_timestamp + 10_000_000;  // Wait a long time
});
client.settle();

// The lock timestamp was set at fund_with_commitment, not at settle!
// Your claim may already be expired or expired soon.
```

**Right:** The lock starts immediately at your first deposit:

```rust
let deposit_timestamp = env.ledger().timestamp();

client.fund_with_commitment(&investor, &amount, &lock_secs);
// claim_not_before = deposit_timestamp + lock_secs (RIGHT NOW)

// Settle happens later
env.ledger().with_mut(|l| {
    l.timestamp = deposit_timestamp + lock_secs + some_offset;
});
client.settle();

// Now you can claim (both conditions met)
client.claim_investor_payout(&investor);
```

---

## Error Codes Reference

If your SDK or RPC call returns an error, check this table:

| Code | Error | Meaning | Recovery |
| ---: | --- | --- | --- |
| 108 | `TieredSecondDeposit` | You called `fund_with_commitment()` after your first deposit | Use `fund()` for follow-on deposits |
| 109 | `InvestorClaimTimeOverflow` | `timestamp + lock_secs` overflows u64 (extremely rare) | Reduce lock duration or contact support |
| 111 | `CommitmentLockExceedsMaturity` | Your lock extends past escrow maturity | Shorten your lock or ask for extended maturity |
| 128 | `InvestorCommitmentLockNotExpired` | You tried to claim before your lock expired | Wait until the lock timestamp passes |

---

## Frequently Asked Questions

**Q: Can I change my tier if the escrow operator adds new tiers?**
A: No. The tier table is immutable. Your tier is locked in at your first deposit. New tiers added in future escrows apply only to new invoices.

**Q: What if I deposit at lock_secs = 1 day, but the highest tier requires 90 days?**
A: You get the base yield only. The contract only upgrades your tier if your commitment meets or exceeds the tier's minimum. Committing to less than the lowest tier's minimum means no tier upgrade.

**Q: Does my lock restart if I make a follow-on deposit?**
A: No. Your lock is set once at your first deposit. Follow-on deposits do not extend the lock. They share the same claim-not-before timestamp.

**Q: What if the escrow settles before my lock expires?**
A: Settlement and locks are independent. You can claim your payout only when BOTH conditions are true:
1. Escrow is settled
2. Your lock timestamp has passed

If settlement happens early, you still wait for your lock.

**Q: Can the admin or SME change my tier after I deposit?**
A: No. Your tier and lock are written to storage when you first fund. No entrypoint can change them—not even admin operations. This is the fairness guarantee.

---

## Summary

- **First deposit only:** Tier selection happens once via `fund_with_commitment()` on your first deposit
- **Immutable forever:** Your tier, yield, and claim lock cannot change
- **Lock is immediate:** Your claim-not-before timestamp is set at deposit time, not settlement time
- **Subsequent deposits:** Use `fund()` to add capital without re-selecting a tier
- **Claim gates:** You can only claim when settled AND lock expired
- **No second chances:** Calling `fund_with_commitment()` twice returns error 108

Choose your tier wisely on your first deposit—it's permanent!
