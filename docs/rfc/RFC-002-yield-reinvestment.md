# RFC-002: Yield Reinvestment

**Status:** IMPLEMENTED  
**Author:** Product Team (@karis-ky)  
**Date Proposed:** 2026-07-27  
**Target Release:** v1.5 (Q4 2026)  

This RFC is now implemented via the reinvestment lifecycle enforced by the escrow contract and is tracked as part of FEAT-004.
**Related:** ADR-005 (Tiered Yield), Issue #892, RFC-003 (Registry Integration)

---

## Summary

This RFC proposes an optional **yield reinvestment feature** that allows investors to automatically contribute their accrued yield into a subsequent funding round, compounding their position. This increases investor engagement, improves escrow funding velocity, and creates a natural upgrade path for returning customers.

---

## Motivation

**Problem Statement:**

After settlement, investors receive their pro-rata payout but accrued yield is distributed separately. Currently, investors must:

1. Receive yield proceeds (e.g., 500 USDC yield)
2. Manually decide to re-invest in a new escrow
3. Make a separate call to `fund()` for the new round

This friction causes **yield leakage**: many investors don't reinvest, even if they want to. Opportunities:

- **Compound growth:** Enable investors to grow positions across multiple rounds
- **Reduced churn:** Automatic reinvestment increases customer lifetime value
- **Platform stickiness:** Easier for investors to scale exposure over time

**Impact:**
- ~30% of investors currently don't reinvest after first round (estimated from integrator feedback)
- Reinvestors have 2.5x higher lifetime value (internal data)
- Competitors increasingly offer auto-compound features

**Why now:**
- Customer requests accumulating (5+ integrators asked in Q3 2026)
- ADR-005 (Tiered Yield) already defines per-investor yield tracking
- No new storage assumptions required
- Natural fit with yield distribution entrypoint

**Use Cases:**

1. **Automated scaling:** Investor 1 contributes 1000 USDC to round 1, earns 50 USDC, automatically re-invests (1050 USDC to round 2)
2. **Opt-in compounding:** Investor wants yield to compound but only in invoices from trusted sellers
3. **Portfolio rebalancing:** Investor who diversifies across 3 concurrent escrows with reinvestment enabled

**Success Metric:**
- 40%+ of investors enable reinvestment on first escrow
- Reinvestment volume (principal + compounded yield) > 15% of total funded amount within 6 months of launch
- Average investor lifetime value increases by 20%

---

## Design

### Overview

**High-level approach:**

Investors will specify a **reinvestment preference** (`reinvest_yield: bool`) at fund time or during settlement. If enabled:

1. After settlement, investor's accrued yield is automatically reserved
2. On next funding round by same SME, yield is pre-approved for contribution
3. Investor may accept/reject the reinvestment offer (1-week window)
4. If accepted, yield + any manual contribution are combined into single deposit
5. If rejected or expired, yield is transferred to investor normally

**Backward compatibility:** Feature is opt-in. Existing investors continue to receive yield payouts directly.

### Detailed Design

**Component 1: Storage Schema**

New `DataKey` variants:

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// Per-investor reinvestment preference (opt-in).
    /// Persistent. Absent ⇒ false (reinvestment disabled).
    InvestorReinvestmentEnabled(Address),
    
    /// Accrued yield reserved for reinvestment (not yet claimed).
    /// Persistent. Absent ⇒ 0. Incremented at settlement.
    InvestorYieldReserve(Address),
    
    /// Ledger timestamp when reserved yield expires (7 days after settlement).
    /// Persistent. Absent ⇒ not reserved. Cleared when investor claims or reinvests.
    InvestorYieldReserveExpiresAt(Address),
}
```

**Data flow:**

```
Settlement occurs
    ↓
Read InvestorReinvestmentEnabled(investor)
    ↓
If true:
    → Reserve yield in InvestorYieldReserve(investor)
    → Set InvestorYieldReserveExpiresAt(investor) = now + 7 days
    → Emit YieldReservedForReinvestment event
Else:
    → Transfer yield to investor (existing behavior)
```

**Component 2: Entrypoint Changes**

**`fund()` / `fund_with_commitment()` signature update:**

```rust
pub fn fund(
    env: Env,
    amount: i128,
    reinvest_yield: Option<bool>,  // NEW: default None = no preference change
) -> Result<InvestorContributed, EscrowError>
```

**Settlement flow update:**

When `settle()` or `withdraw()` occurs:

1. For each investor with `InvestorReinvestmentEnabled == true`:
   - Calculate accrued yield
   - Reserve yield in `InvestorYieldReserve(investor)`
   - Emit `YieldReservedForReinvestment(investor, amount, expires_at)`
2. For investors with reinvestment disabled (or not set):
   - Transfer yield to investor account (existing behavior)

**New entrypoint: `accept_yield_reinvestment()`**

```rust
pub fn accept_yield_reinvestment(
    env: Env,
    reserved_investor: Address,
) -> Result<ReinvestmentAccepted, EscrowError>
```

When called by investor within 7-day window:
- Move `InvestorYieldReserve` into investor's contribution for next escrow with same SME
- Clear expiration timer
- Emit `ReinvestmentAccepted(investor, yield_amount, contributed_to_escrow_id)`

**New entrypoint: `claim_reserved_yield()`**

```rust
pub fn claim_reserved_yield(
    env: Env,
) -> Result<YieldClaimed, EscrowError>
```

When called by investor (or automatically at expiration):
- Transfer `InvestorYieldReserve` to investor account
- Clear expiration timer
- Emit `YieldClaimedFromReserve(investor, amount)`

**Component 3: Reinvestment Matching Logic**

When new escrow `init()` is called for same SME:

1. Scan for other escrows by same SME with active reinvestment reserves
2. For each investor with non-expired reserve:
   - Pre-populate contribution from reserved yield
   - Notify investor: "Your $Y yield is ready to invest in new invoice (expires in 7 days)"
3. Investor may accept/reject via `accept_yield_reinvestment()`

**Matching rules:**
- Reinvestment only available if: same SME + investor has `InvestorReinvestmentEnabled` + reserve not expired
- Investor may combine reserved yield + new manual contribution in single `fund()` call
- Multiple active reserves cannot be consolidated (reinvest one at a time)

### Examples

**Example 1: Simple opt-in reinvestment**

```
T=0 days: Investor 1 calls fund(amount: 1000, reinvest_yield: true)
          → Sets InvestorReinvestmentEnabled(investor1) = true

T=30 days: Settlement occurs on invoice. Yield = 50 USDC
           → Reserved in InvestorYieldReserve(investor1) = 50
           → Expires in 7 days (T=37)

T=32 days: Same SME creates new invoice 2. Contract detects reinvestment reserves.
           → Notifies investor1: "Your $50 yield is ready. Accept reinvestment?"

T=34 days: Investor1 calls accept_yield_reinvestment()
           → Yield moved into contribution for invoice 2
           → Investor1 now has 50 USDC pre-approved for invoice 2

T=35 days: Investor1 calls fund(amount: 950, reinvest_yield: true)
           → 50 USDC (reserved) + 950 USDC (new) = 1000 USDC contribution
```

**Example 2: Opt-out during fund time**

```
T=0 days: Investor 2 calls fund(amount: 2000, reinvest_yield: false)
          → InvestorReinvestmentEnabled(investor2) = false

T=30 days: Settlement. Yield = 100 USDC.
           → InvestorReinvestmentEnabled(investor2) = false
           → Yield transferred directly to investor2 account (standard payout)
           → No reserve created
```

**Example 3: Reserve expires, investor claims**

```
T=32 days: Investor3 has reserve of 75 USDC expiring at T=39.

T=38 days: Investor3 calls claim_reserved_yield()
           → Transfer 75 USDC to investor3 account
           → Clear reserve

T=40 days: Reserve naturally expires. Cron job (off-chain):
           → Transfers any unclaimed reserves to investors
```

---

## Alternatives Considered

### Alternative 1: Automatic Reinvestment (Rejected)

**Approach:** Yield automatically reinvested into next escrow by same SME without investor interaction.

**Pros:**
- Maximum friction reduction
- Highest reinvestment rate

**Cons:**
- Investors may not notice their yield is locked in next escrow
- Surprise capital commitment (could cause tax/accounting issues)
- Regulatory risk (auto-renewal / auto-debit requirements)
- Hard to opt-out or modify

**Decision:** Rejected for v1. Use opt-in + 7-day acceptance window (v1.5). Upgrade to automatic in v2 if customer feedback supports.

---

### Alternative 2: Yield Compounding at Maturity (Rejected)

**Approach:** Reinvest yield immediately at settlement, without investor choice.

**Pros:**
- Simpler state management

**Cons:**
- No investor control (forced reinvestment)
- Complicates tax reporting (investor can't easily see when reinvestment happens)
- Reduces flexibility (investor might need yield for other purposes)

**Decision:** Rejected. Investors need explicit control.

---

### Alternative 3: Cross-SME Reinvestment (Deferred)

**Approach:** Yield can be reinvested into any SME's escrow, not just same SME.

**Pros:**
- Investor portfolio diversification

**Cons:**
- More complex matching (which SME to choose?)
- Loses investor relationship cohesion (return customers for same SME)
- Storage overhead (track preferred SME list per investor)

**Decision:** Deferred to v2. v1 supports same-SME only.

---

## Implementation

### Effort Estimate

| Component | Estimate | Notes |
|-----------|----------|-------|
| Storage schema (3 new DataKey variants) | 1–2 days | Persistent keys, TTL handling |
| Settlement flow update (yield reservation) | 2–3 days | Modify settlement logic + emit events |
| Acceptance/claim entrypoints | 2–3 days | `accept_yield_reinvestment()`, `claim_reserved_yield()` |
| Reinvestment matching logic | 2–3 days | Detect active reserves during `fund()` |
| Unit tests (reinvestment state, expiration) | 2–3 days | Parametrized over timeframes |
| Integration tests (full flow) | 2–3 days | Fund → settle → reinvest → fund again |
| Documentation + audit prep | 1–2 days | ADR, API docs, security checklist |
| **Total** | **12–18 days** | ~3-week sprint |

### Milestones

**Week 1:** Storage schema + settlement flow
- [ ] New DataKey variants defined
- [ ] Settlement logic reserves yield when reinvestment enabled
- [ ] TTL/expiration handling for reserves
- [ ] Unit tests for reservation logic

**Week 2:** Entrypoints + matching logic
- [ ] `accept_yield_reinvestment()` entrypoint
- [ ] `claim_reserved_yield()` entrypoint
- [ ] Reinvestment matching during `fund()`
- [ ] Event emission (`YieldReservedForReinvestment`, etc.)

**Week 3:** Integration tests + audit prep
- [ ] End-to-end: fund → settle → reinvest → fund
- [ ] Expiration handling (auto-claim after 7 days)
- [ ] Error handling (invalid investor, expired reserve, etc.)
- [ ] ADR draft + security checklist

### Blockers

- [ ] None identified as of 2026-07-27
- Assumes investors check for reinvestment opportunities (requires UX notification from integrators)

### Implementation Notes

- Use `Env::ledger().timestamp() + 7 * 86400` for expiration (7 days in seconds)
- Reinvestment reserve is **separate** from normal payout; don't combine both
- Error code 43: "Reinvestment offer expired" (new)
- Maintain strict per-investor tracking; don't aggregate reserves

---

## Acceptance Criteria

- [ ] Investor may set `InvestorReinvestmentEnabled` at fund time
- [ ] Settlement reserves yield in `InvestorYieldReserve` if reinvestment enabled
- [ ] Reserved yield has 7-day expiration (from settlement date)
- [ ] Investor may accept/reject reinvestment within window
- [ ] Expired reserves are automatically transferred to investor
- [ ] Reinvestment matches investor with next escrow by same SME
- [ ] Combined contribution (reserved + manual) handled correctly in pro-rata calc
- [ ] All state transitions emit appropriate events
- [ ] Error code 43 returned for expired reinvestment offers
- [ ] 95%+ code coverage maintained
- [ ] ADR-010 drafted and linked
- [ ] Documentation updated: new entrypoints, data model, events
- [ ] Zero security findings from audit

---

## Rollout Plan

### Phase 1: Testnet (Week 1–2)

- Deploy to testnet
- Run parametrized tests (multiple investors, SMEs, expiration scenarios)
- Verify yield calculation matches ADR-005 spec
- Solicit team feedback on UX/event schema

**Success criteria:** All tests pass; no regressions on existing settlement logic.

### Phase 2: Early Integration Testing (Week 3)

- Release `v1.5-beta` to 2–3 integrators
- Partners test reinvestment flow on testnet
- Collect feedback on: UI clarity, notification timing, error messages
- Refine event payloads based on partner needs

**Success criteria:** Partners successfully implement reinvestment acceptance flow; no critical UX gaps.

### Phase 3: Mainnet (Week 4)

- Audit completion + fixes applied
- Release `v1.5` alongside RFC-001 (Multi-Token Support)
- Monitor reinvestment acceptance rates + expiration auto-claim success
- Rollback plan: disable reinvestment feature flag if error rate > 1%

**Success criteria:** Reinvestment acceptance rate ≥ 30%; auto-claim success rate ≥ 99%.

### Monitoring

**Key metrics:**
- Reinvestment acceptance rate (% of investors who accept within 7 days)
- Average reinvestment amount vs. new contributions
- Expiration auto-claim success rate
- Lifetime value of reinvesting vs. non-reinvesting cohorts

**Dashboards:**
- Grafana: reinvestment acceptance rates (real-time)
- Cohort analysis: reinvestor LTV vs. control group (weekly)

---

## References

- **ADR-005 (Tiered Yield):** [`docs/adr/ADR-005-tiered-yield.md`](../adr/ADR-005-tiered-yield.md)
- **GitHub Issue #892:** "Enable yield auto-reinvestment" (karis-ky/escrow-contracts#892)
- **Related RFC:** RFC-001 (Multi-Token) — may need coordination on settlement flows
- **Related RFC:** RFC-003 (Registry) — reinvestment matching could benefit from registry lookup

---

## Decision

**Owner:** Product Lead  
**Status:** DISCUSSION (awaiting feedback from 3+ reviewers)  
**Decision date:** TBD (target: 2026-08-10)

---

## Revision History

| Date | Status | Change |
|------|--------|--------|
| 2026-07-27 | DRAFT | Initial proposal |
| — | DISCUSSION | Team review period (3+ reviewers) |
| — | ACCEPTED | Decision made by product lead |
| — | IMPLEMENTED | Feature shipped |

