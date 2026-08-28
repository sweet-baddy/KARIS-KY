# RFC-NNN: Title (Use Imperative Mood)

**Status:** DRAFT | DISCUSSION | ACCEPTED | IMPLEMENTED | CLOSED  
**Author:** Your Name (@github_handle)  
**Date Proposed:** YYYY-MM-DD  
**Target Release:** vX.Y or TBD  
**Related:** ADR-NNN, Issue #NNN, RFC-NNN (if applicable)

---

## Summary

One paragraph: What is this RFC about? Why does it matter?

Example:
> This RFC proposes support for multiple settlement tokens (USDC, EURC, etc.) 
> to enable escrows denominated in different stablecoins. Currently only USDC 
> is supported, limiting market reach. Multi-token support increases platform 
> flexibility for international settlements.

---

## Motivation

**Problem Statement:** What is the gap or limitation today?

- Current constraint or limitation
- Impact on users or operators
- Why now? (timeline, external factors)

**Use Cases:**

1. **Primary use case** — Who benefits and how?
2. **Secondary use case** — Additional beneficiaries

**Success Metric:** How do we measure if this solves the problem?
- e.g., "3+ new asset types registered in first month"
- e.g., "Settlement slippage < 1% for all tokens"

---

## Design

### Overview

**High-level approach:** Summarize the solution in 1–2 paragraphs.

### Detailed Design

**Component 1: Storage Schema**
- What storage keys are added/modified?
- Any migration burden?
- TTL or growth implications?

**Component 2: Entrypoints**
- What new functions are needed?
- Auth boundaries?
- Backward compatibility?

**Component 3: Token Validation**
- How do we verify a token is safe to settle?
- Allowlist, registry lookup, or dynamic?
- Error codes for invalid tokens?

### Data Model

If applicable, include a diagram (Mermaid or ASCII):

```
Escrow ──→ FundingToken (SEP-41)
       ├→ SettlementTokens (NEW)
       │   ├─ USDC
       │   ├─ EURC
       │   └─ [others]
       └─ Treasury
```

### Examples

**Example 1: Investor funds USDC, SME settles in EURC**
```
1. Investor calls fund(usdc_amount=1000, token=USDC)
2. Contract verifies USDC in allowlist
3. Stores contribution under InvestorContribution(investor_addr, USDC)
4. SME calls settle(settlement_token=EURC)
5. Contract converts contributions to EURC at spot rate
6. Transfers to SME's EURC account
```

**Example 2: Escrow configured with `allowed_settlement_tokens=[USDC, EURC]`**
```
init(
  funding_token: USDC,
  settlement_tokens: [USDC, EURC],  // NEW
  treasury: treasury_addr,
)
// SME may settle in either token
```

---

## Alternatives Considered

### Alternative 1: Single Hardcoded Settlement Token

**Approach:** Require all escrows to settle in contract deployer's chosen token.

**Pros:**
- No token validation logic needed
- Simpler storage schema
- Easier testing

**Cons:**
- Inflexible for multi-currency workflows
- Requires new contract deployment per token
- Higher operational cost

**Tradeoff:** Less flexible, simpler to build. Rejected because users need flexibility.

---

### Alternative 2: Dynamic Token Registry Lookup

**Approach:** Query a registry contract to validate settlement tokens at runtime.

**Pros:**
- Highly extensible (add tokens without upgrading escrow)
- Centralized token governance

**Cons:**
- Extra host function call on every settle (gas cost)
- Registry contract becomes critical dependency
- Potential race conditions if registry updates mid-transaction

**Tradeoff:** More flexible but higher gas/latency. Choose Alternative 1 (allowlist) for v1.

---

### Alternative 3: Investor Chooses Settlement Token at Fund Time

**Approach:** Each investor specifies desired settlement token on first deposit.

**Pros:**
- Per-investor choice and flexibility

**Cons:**
- Complex state tracking (per-investor settlement preferences)
- Settlement phase must consolidate multiple tokens
- Tax/accounting complexity for multi-token payouts

**Tradeoff:** Too complex for v1. Defer to v2 if requested.

---

## Implementation

### Effort Estimate

| Component | Estimate | Notes |
|-----------|----------|-------|
| Storage schema migration | 3–4 days | Schema v7 with new DataKey variants |
| Entrypoint changes | 2–3 days | `init`, `settle`, `withdraw` updates |
| Token validation layer | 2–3 days | Allowlist checks + error codes |
| Testing (unit + integration) | 3–4 days | Parametrized tests over token pairs |
| Audit readiness | 2–3 days | Documentation, review prep |
| **Total** | **12–17 days** | ~3 week sprint |

### Milestones

1. **Week 1:** Storage schema + validation layer PR
2. **Week 2:** Entrypoint integration + unit tests
3. **Week 3:** Integration tests + audit prep
4. **Week 4:** Review + refinement

### Blockers

- [ ] No known blockers
- [ ] Stellar RPC response time for token metadata (TBD if using registry)
- [ ] Audit slot availability (assume Q4 2026)

### Implementation Notes

- Use `Option<Vec<Address>>` for settlement token allowlist (None = only funding token)
- Add typed error code `42` for "unsupported settlement token"
- Maintain backward compatibility: old escrows continue to settle in funding token only

---

## Acceptance Criteria

- [ ] All settlement tokens in allowlist are verified before `settle()` processes
- [ ] Investor contributions tracked per token (not aggregated)
- [ ] SME can select any allowlisted token at settlement time
- [ ] Pro-rata payout calculated correctly across token allocations
- [ ] Token validation fails with error code 42 for unlisted tokens
- [ ] Schema migration tested (old instances continue to work)
- [ ] Documentation updated: entrypoint signatures, data model, error codes
- [ ] 95%+ code coverage maintained
- [ ] Zero known security issues from audit

---

## Rollout Plan

### Phase 1: Internal Testing (Week 1–2)
- Deploy to testnet with 2–3 test tokens
- Run integration tests for edge cases
- Team feedback + refinement

### Phase 2: Beta Release (Week 3)
- Release `v1.5-beta` on testnet
- Share with early partners (2–3 integrators)
- Collect feedback on UX, gas costs, token list

### Phase 3: Production (Week 4)
- Audit completion and fixes
- Deploy to mainnet on Friday (rollback support Monday)
- Monitor settlement volume + error rates for 1 week
- Publish migration guide for operators

### Monitoring

**Metrics to watch:**
- Settlement success rate (target: 99.9%)
- Token validation error frequency
- Pro-rata payout accuracy (sample audit)
- Gas cost per settlement (track over time)

**Rollback plan:** If error rate > 1% or critical bug found, revert to v1.4 and disable multi-token feature.

---

## References

- **Stellar SEP-41 Standard:** https://stellar.org/protocol/core/cap-0046-06 (token interface)
- **Issue #1234:** "Support EURC settlement alongside USDC" (GitHub)
- **Related ADR:** None yet (will become ADR-NNN if accepted)
- **Token Integration Checklist:** [`docs/ESCROW_TOKEN_INTEGRATION_CHECKLIST.md`](../ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)

---

## Decision

**Owner:** Governance Lead  
**Decision:** Accepted / Rejected / Deferred  
**Rationale:** [Why did we choose this path over alternatives?]  
**Date Decided:** YYYY-MM-DD

---

## Timeline

| Date | Event | Status |
|------|-------|--------|
| 2026-07-27 | RFC proposed (DRAFT) | ✓ |
| 2026-08-03 | Review period ends (DISCUSSION) | — |
| 2026-08-10 | Decision made (ACCEPTED) | — |
| 2026-09-01 | Implementation complete (IMPLEMENTED) | — |
| 2026-10-01 | Shipped to production (CLOSED) | — |

---

## Appendix: FAQ

**Q: Can we support wrapped tokens?**  
A: Out of scope for v1. Defer to RFC-NNN-wrapped-tokens if needed.

**Q: What about token pairs with different decimals?**  
A: Use Stellar's `amount` field normalization. Token metadata includes decimals.

**Q: How does this interact with tiered yield (ADR-005)?**  
A: Yield is denominated in funding token; settlement token conversions happen post-calculation.

