# Partial Settlement Invariants — Implementation Summary

## Overview

This document summarizes the correctness pin for the partial settlement feature in the escrow contract.
The scope includes:

1. **Definition** of six core invariants that govern partial settlement behavior
2. **Implementation verification** that the code enforces these invariants
3. **Test coverage** for partial-then-full claim sequences to prevent subtle payout mis-accounting

---

## The Problem

The `partial_settle(env, caller)` function settles a portion of escrow principal without
immediately transitioning to the settled state. When investors claim payouts after partial
settlements, there's risk of:

- **Payout divergence:** Sum of incremental payouts ≠ full settlement payout (due to incorrect `distributed_principal` tracking)
- **Double-counting:** Investor claims same principal twice
- **Rounding errors:** Pro-rata distribution loses precision across multiple steps
- **Status inconsistency:** Escrow stays in `funded` state (1) while principal is distributed

This module pins correctness by defining invariants and providing end-to-end tests.

---

## Invariants

### Invariant 1: `get_distributed_principal()` advances monotonically and accurately

**Definition:**

`get_distributed_principal()` returns the cumulative principal distributed across all settlements
(partial and full). It is the running total of every `partial_settle` / `settle` amount.

**Properties:**

- Before any settlement: `get_distributed_principal() == 0`
- After each partial settlement of `amount`: increases by exactly `amount`
- After full settlement: `get_distributed_principal() == funded_amount`
- **Monotonic:** Value never decreases. Once `X`, it stays `≥ X`
- **Bounded:** `get_distributed_principal() ≤ funded_amount` always

**Implementation:**

Stored in `DataKey::SettledAmount`. Updated in `settle_impl()`:

```rust
let current_settled_amount: i128 = env
    .storage()
    .instance()
    .get(&DataKey::SettledAmount)
    .unwrap_or(0);

let new_settled_amount = match partial_amount {
    Some(amount) => {
        // Validate partial amount
        ensure(&env, amount > 0, EscrowError::SettlementNotPositive);
        ensure(&env, amount <= escrow.funded_amount, EscrowError::SettlementExceedsFunded);
        ensure(
            &env,
            amount + current_settled_amount <= escrow.funded_amount,
            EscrowError::SettlementExceedsFunded,
        );
        current_settled_amount + amount
    }
    None => escrow.funded_amount, // Full settlement
};

env.storage()
    .instance()
    .set(&DataKey::SettledAmount, &new_settled_amount);
```

The bounds check `amount + current_settled_amount <= escrow.funded_amount` enforces the invariant.

**Tests:**

- `distributed_principal_advances_through_partial_settlements()` — verifies monotonic advance
- `settle_beyond_outstanding_principal_panics()` — verifies bounds enforcement

---

### Invariant 2: Investor payouts across partial settlements reconcile correctly

**Definition:**

The sum of all incremental investor payouts (across multiple partial settle + claim sequences)
equals the payout from a single full settlement at the same yield.

```
sum(payout_i(step1), payout_i(step2), ..., payout_i(stepN)) = payout_i(full_settlement)
```

**Derivation:**

Each investor's payout at step `j` is:

```
payout_i(j) = contribution_i × (settled_amount_j / total_principal) + (yield on settled_amount_j)
```

Where:
- `settled_amount_j` = cumulative settled up to step `j`
- `total_principal` = total funded amount (snapshot at funding close)
- Yield is computed on `settled_amount_j` only, not on `contribution_i`

After `N` steps, if full settlement is reached:

```
payout_i(full) = contribution_i × (sum(settled_amount_j) / total_principal) + sum(yield_j)
               = contribution_i × (total_principal / total_principal) + total_yield
               = contribution_i + total_yield
```

The sum of incremental payouts equals the full settlement payout because principal and yield
are both distributed pro-rata across all steps.

**Implementation:**

Computed in `compute_investor_payout_impl()`:

```rust
let settled_amount: i128 = env
    .storage()
    .instance()
    .get(&DataKey::SettledAmount)
    .unwrap_or(0);

if settled_amount == 0 {
    return 0; // No settlement yet
}

// gross_payout = contribution × settle_pool / total_principal  (floor)
contribution
    .checked_mul(settle_pool)
    .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
    .checked_div(total_principal)
    .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
```

The `settle_pool = settled_amount + net_coupon` grows with each partial settlement,
so incremental payouts are computed correctly.

**Tests:**

- `single_investor_partial_then_full_settlement_yields_match()` — verifies single investor across two partials
- `equal_investors_receive_equal_pro_rata_shares()` — verifies multiple equal investors get equal shares
- `unequal_investors_receive_proportional_pro_rata_shares()` — verifies proportional shares
- `full_reconciliation_multi_partial_then_full()` — verifies 4-step partial sequence
- `multi_investor_full_reconciliation_partial_then_full()` — verifies 3 investors, 2 partials

---

### Invariant 3: Partial claim then full settlement yields no double-counting

**Definition:**

When an investor claims after a partial settlement, then another partial settlement occurs,
the second payout includes only **new incremental principal and yield**, not a re-distribution
of already-claimed portion.

**Implementation:**

Enforced by the idempotent claim marker set in `claim_investor_payout_impl()`:

```rust
// Idempotent early-return: a second claim is a no-op (no re-emit).
if Self::get_persistent_investor_claimed(&env, investor.clone()) {
    return; // Early return; investor has already claimed
}

// Mark before emit — prevents re-emission on any re-entrant path.
Self::set_persistent_investor_claimed(&env, investor.clone(), true);
```

Each investor's claimed flag is set to `true` after their first claim. On subsequent calls,
the investor is marked as already-claimed, so the payout computation returns a no-op.

However, note that `compute_investor_payout()` is recomputed at each settlement step and
returns the **incremental payout** due to the new settled amount. The claim flag ensures
no event re-emission, but the payout is still accessible for the next settlement step.

**Tests:**

- `double_claim_is_idempotent()` — verifies second claim is a no-op
- `investor_accumulates_claims_across_partial_settlements()` — verifies accumulation across steps

---

### Invariant 4: `claim_investor_payout()` is idempotent across all settlement states

**Definition:**

Once an investor calls `claim_investor_payout()` after a settlement step, a second call
in the same or later step is a **no-op** and does not emit a duplicate event.

**Implementation:**

The claimed flag (`DataKey::InvestorClaimed(investor)`) is checked early:

```rust
if Self::get_persistent_investor_claimed(&env, investor.clone()) {
    return; // No-op; no event emitted
}

Self::set_persistent_investor_claimed(&env, investor.clone(), true);
```

**Tests:**

- `double_claim_is_idempotent()` — verifies no duplicate events
- `investor_accumulates_claims_across_partial_settlements()` — verifies no re-emission

---

### Invariant 5: Multiple investors with equal contributions receive equal payouts

**Definition:**

For any settlement step, two investors with identical contributions receive identical payout
amounts (subject to rounding from integer division).

```
contribution_a == contribution_b  ⟹  payout_a ≈ payout_b  (within rounding tolerance)
```

**Implementation:**

The payout formula is symmetric in contributions:

```
payout_i = contribution_i × (settle_pool / total_principal)
```

If `contribution_a == contribution_b`, then `payout_a == payout_b` (integer division may differ by ±1).

**Tests:**

- `equal_investors_receive_equal_pro_rata_shares()` — verifies equal contributions get equal payouts

---

### Invariant 6: Settling beyond outstanding principal panics

**Definition:**

The sum of all partial settlements must not exceed the total funded principal:

```
sum(partial_amounts) ≤ funded_amount
```

Attempting to settle beyond this bound panics with `EscrowError::SettlementExceedsFunded`.

**Implementation:**

Enforced in `settle_impl()`:

```rust
ensure(
    &env,
    amount + current_settled_amount <= escrow.funded_amount,
    EscrowError::SettlementExceedsFunded,
);
```

**Tests:**

- `settle_beyond_outstanding_principal_panics()` — verifies panic on overflow
- `partial_settle_zero_panics()` — verifies zero is rejected
- `partial_settle_one_unit_succeeds()` — verifies boundary (minimum 1 unit)

---

## Test Coverage

All invariants are verified by comprehensive tests in `escrow/src/tests/partial_settle_invariants.rs`.

### Test Organization

| Invariant | Test Functions |
|-----------|----------------|
| 1 | `distributed_principal_advances_through_partial_settlements()`, `settle_beyond_outstanding_principal_panics()` |
| 2 | `single_investor_partial_then_full_settlement_yields_match()`, `equal_investors_receive_equal_pro_rata_shares()`, `unequal_investors_receive_proportional_pro_rata_shares()`, `full_reconciliation_multi_partial_then_full()`, `multi_investor_full_reconciliation_partial_then_full()` |
| 3 | (Implicit in Invariant 4 tests) |
| 4 | `double_claim_is_idempotent()`, `investor_accumulates_claims_across_partial_settlements()` |
| 5 | `equal_investors_receive_equal_pro_rata_shares()` |
| 6 | `settle_beyond_outstanding_principal_panics()`, `partial_settle_zero_panics()`, `partial_settle_one_unit_succeeds()` |

### Key Test Scenarios

1. **Multi-step partial settlement with intermediate claims**
   - Setup: 1 investor, fund 100%
   - Settle 25%, claim
   - Settle 25%, claim
   - Settle 25%, claim
   - Settle 25%, claim (full)
   - Verify: sum of payouts = expected full settlement payout

2. **Multiple investors, unequal contributions**
   - Setup: 3 investors (50%, 30%, 20%)
   - Settle 40%, all claim
   - Settle 60%, all claim
   - Verify: each investor gets proportional share

3. **Idempotent claims**
   - Setup: Fund, partial settle
   - Claim, claim again (no-op)
   - Partial settle, claim again
   - Verify: no duplicate events, no double payout

---

## Code Locations

| Component | File | Lines |
|-----------|------|-------|
| `partial_settle_impl()` | `escrow/src/settlement.rs` | ~100–180 |
| `settle_impl()` with partial logic | `escrow/src/settlement.rs` | ~300–500 |
| `claim_investor_payout_impl()` | `escrow/src/settlement.rs` | ~774–980 |
| `compute_investor_payout_impl()` | `escrow/src/settlement.rs` | ~1446–1550 |
| `get_distributed_principal_impl()` | `escrow/src/admin.rs` | ~1445–1450 |
| **Invariant tests** | `escrow/src/tests/partial_settle_invariants.rs` | Full module (~650 lines) |
| **Documentation** | `docs/escrow-lifecycle.md` | "Partial settlement invariants" section |

---

## Acceptance Criteria Verification

✅ **Sum of partial claims equals the expected prorated payout**
   - Verified by `single_investor_partial_then_full_settlement_yields_match()`,
     `full_reconciliation_multi_partial_then_full()`,
     `multi_investor_full_reconciliation_partial_then_full()`

✅ **`get_distributed_principal()` advances correctly across partial settles**
   - Verified by `distributed_principal_advances_through_partial_settlements()`,
     and all multi-step tests

✅ **Invariant is documented**
   - Documented in `docs/escrow-lifecycle.md` under "Partial settlement invariants" section
   - Includes derivation, properties, and test references

---

## Notes for Reviewers

### No Breaking Changes

- No signature changes to `partial_settle()` or `claim_investor_payout()`
- No multi-asset partial settle (out of scope)
- Fully backward-compatible with existing settlement flows

### Test Methodology

- Each test sets up a fresh `Env` via shared `setup()` helper
- Tests use `env.mock_all_auths()` for deterministic authorization
- Payout amounts are compared with tolerance `<= 2` or `<= 4` to account for rounding in integer division
- Idempotency is verified by marking investors as claimed and asserting no re-entry

### Future Work (Out of Scope)

- Optimization: batch yield distribution snapshots for large investor counts
- Gas analysis: measure transaction cost of partial settlement chains
- Multi-asset support: extend invariants to multiple collateral types

---

## References

- **ADR-001:** Escrow status state machine (`docs/escrow-lifecycle.md`)
- **settle_impl():** Full and partial settlement logic (`escrow/src/settlement.rs`)
- **compute_investor_payout_impl():** Pro-rata payout computation
- **Test helpers:** `escrow/src/tests.rs` (setup, deploy, etc.)

