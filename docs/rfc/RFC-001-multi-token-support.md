# RFC-001: Multi-Token Support

**Status:** DRAFT  
**Author:** Platform Team (@karis-ky)  
**Date Proposed:** 2026-07-27  
**Target Release:** v2.0 (Q1 2027)  
**Related:** Issue #847, [Token Integration Checklist](../ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)

---

## Summary

This RFC proposes extending the escrow contract to support settlement in multiple Stellar stablecoins (USDC, EURC, etc.) beyond the currently hardcoded funding token. This enables international settlements and reduces operational friction for users who hold different stablecoin assets across regions.

---

## Motivation

**Problem Statement:**

Currently, each escrow instance is bound to a single funding token at initialization. This constrains workflows:

1. **Single-currency constraint:** A seller in the EU must accept USDC even if they prefer EURC
2. **Conversion friction:** Integrators must manage token swaps outside the contract
3. **Market limitations:** Cannot offer escrows in currencies matching customer portfolios
4. **Operational overhead:** Need separate contract deployments per token instead of configuration

**Impact:**
- Limits addressable market (developing regions prefer local stablecoins)
- Increases operational cost for integrators
- Creates arbitrage friction vs. competitors supporting multi-token

**Why now:**
- Stellar ecosystem expanding (EURC launch Q3 2026, others pending)
- Customer requests accumulating (3+ partners asked for EURC support in past month)
- Storage schema evolution (v6→v7) provides natural migration point
- No technical blockers identified

**Use Cases:**

1. **International invoicing:** Seller in Mexico receives USDC, settles in USDM (Mexican stablecoin)
2. **Regional preference:** European integrators prefer EURC for compliance/tax reasons
3. **Portfolio matching:** Investor holds EURC; settles in same asset (no slippage)
4. **Hedging:** SME denominated in EUR; settles in EURC rather than USDC

**Success Metric:** 
- 3+ distinct settlement tokens active on mainnet within 3 months of launch
- Multi-token settlement > 25% of total escrow volume by Q2 2027

---

## Design

### Overview

**High-level approach:**

Escrows will be initialized with an **optional allowlist** of settlement tokens (in addition to the funding token). When `settle()` is called, the SME specifies a settlement token from the list. The contract verifies the token is allowlisted, calculates pro-rata payouts, and transfers settlement proceeds in the chosen token.

**Backward compatibility:** Escrows created before this feature continue to settle only in the funding token. No migration required.

### Detailed Design

**Component 1: Storage Schema (v6 → v7)**

New `DataKey` variants:

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// Immutable list of allowlisted settlement tokens (beyond funding token).
    /// Absent ⇒ only funding token is valid for settlement.
    /// Set once at init; never modified.
    SettlementTokenAllowlist,
    
    /// Per-token total principal received (for accounting/auditing).
    /// Absent ⇒ 0. Incremented on each successful fund() call.
    SettlementTokenFunded(Address),  // token address as key
}
```

**Migration path:**
- Old instances (v6) lack `SettlementTokenAllowlist` → default to funding token only
- New instances (v7) optionally set allowlist at `init()`
- No `migrate()` call required; reads default to `None`

**Component 2: Entrypoint Changes**

**`init()` signature update:**

```rust
pub fn init(
    env: Env,
    invoice_id: String,
    admin: Address,
    sme_address: Address,
    amount: i128,
    funding_target: i128,
    funding_token: Address,  // Funding asset (must be SEP-41)
    maturity: u64,
    yield_bps: i64,
    treasury: Address,
    yield_tiers: Option<Vec<YieldTier>>,
    settlement_token_allowlist: Option<Vec<Address>>,  // NEW
) -> Result<EscrowSummary, EscrowError>
```

**`settle()` signature update:**

```rust
pub fn settle(
    env: Env,
    settlement_token: Option<Address>,  // NEW: None ⇒ use funding_token
) -> Result<EscrowSettled, EscrowError>
```

**Validation rules:**

- If `settlement_token_allowlist` is `Some(list)` and non-empty:
  - Deny `settle()` if requested token not in list → error code 42 (UnsupportedSettlementToken)
- If `settlement_token_allowlist` is `None` or empty:
  - Only `settlement_token=None` (i.e., funding token) is allowed
- Token must be a valid SEP-41 contract (post-transfer balance check)

**Component 3: Token Validation**

Add to `external_calls.rs`:

```rust
/// Verify a token is SEP-41-compliant by querying its metadata.
pub fn validate_settlement_token(
    env: &Env,
    token: Address,
) -> Result<TokenMetadata, EscrowError> {
    // Call token's `decimals()` and `name()` to verify responsiveness
    // Cache result in instance storage to avoid repeat queries
    // Return error code 41 if token is unresponsive or malformed
}

/// Check token is in allowlist.
pub fn check_token_allowlisted(
    env: &Env,
    token: Address,
    allowlist: Vec<Address>,
) -> Result<(), EscrowError> {
    // If token not in list, return error 42 (UnsupportedSettlementToken)
    Ok(())
}
```

**Component 4: Settlement Logic Update**

When `settle()` is called with `settlement_token`:

1. Read escrow status (must be FUNDED)
2. Validate settlement token (if specified):
   - Is it in allowlist? (error 42 if not)
   - Is it a valid SEP-41? (error 41 if not)
3. Calculate payout (unchanged; denominated in funding token)
4. **NEW:** Convert payout to settlement token at spot rate (if different)
5. Transfer to SME (via `external_calls::transfer_token_from()`)
6. Record settlement token in `SettlementTokenFunded`
7. Emit `EscrowSettled` with settlement token recorded

**Spot Rate Handling:**

Use **Stellar Price Feed** if available (requires oracle contract); alternatively:
- Require SME to specify exchange rate at settlement time
- Or: Only allow settlement in same token (no conversion needed)

For v1, use **no conversion** — settlement token must be same as funding token or we reject (error 42). v2 will add oracle-based conversion.

### Data Model

```
InvoiceEscrow {
    invoice_id: Symbol,
    admin: Address,
    sme_address: Address,
    amount: i128,
    funding_target: i128,
    funded_amount: i128,
    yield_bps: i64,
    maturity: u64,
    status: u32,
    // NEW:
    settlement_token: Address,  // Token used at settlement (recorded after settle())
}

DataKey::SettlementTokenAllowlist = vec![USDC, EURC, ...]
DataKey::SettlementTokenFunded(EURC) = 5_000_000  // cents
```

### Examples

**Example 1: Dual-currency escrow (USDC funding, USDC or EURC settlement)**

```rust
// Init: allow USDC or EURC for settlement
init(
    ...,
    funding_token: USDC,
    settlement_token_allowlist: Some(vec![USDC, EURC]),
)
// Investor funds 1000 USDC
fund(amount: 1_000_000_000, token: USDC)

// SME settles in EURC (no conversion; assume 1:1 for now)
settle(settlement_token: Some(EURC))
// → Transfers ~1000 EURC to SME
```

**Example 2: Single-token escrow (backward compat)**

```rust
// Init: no settlement allowlist specified
init(
    ...,
    funding_token: USDC,
    settlement_token_allowlist: None,
)

// Settlement must use funding token
settle(settlement_token: None)  // Default to USDC
```

---

## Alternatives Considered

### Alternative 1: Dynamic Token Registry Lookup (Rejected)

**Approach:** Query a registry contract to validate settlement tokens at runtime.

**Pros:**
- Highly extensible (add tokens without upgrading escrow)
- Centralized governance

**Cons:**
- Extra host function call on `settle()` (gas cost ~1000 XLM)
- Registry becomes critical dependency
- Potential race conditions (registry changes mid-transaction)

**Decision:** Rejected for v1. Use allowlist (configured at init) for simplicity. Upgrade to registry lookup in v2 if operational overhead justifies.

---

### Alternative 2: Investor Chooses Settlement Token (Rejected)

**Approach:** Each investor specifies settlement token on first deposit.

**Pros:**
- Per-investor flexibility

**Cons:**
- Complex state tracking (per-investor settlement preferences)
- Settlement consolidation nightmare (how to merge multiple tokens?)
- Tax complexity (multi-token payouts)
- Storage bloat (per-investor preferences)

**Decision:** Rejected. Too complex for v1. Defer to future RFC if customers request.

---

### Alternative 3: Oracle-Based Conversion at Settlement (Deferred to v2)

**Approach:** Query price oracle to convert between any two tokens at settlement.

**Pros:**
- Maximum flexibility (any token pair)

**Cons:**
- Oracle dependency (latency, cost, attack surface)
- Slippage risk (market can move between quote and settlement)
- Requires sophisticated yield adjustments

**Decision:** Deferred to v2. v1 launches with same-token settlement only (no conversion).

---

## Implementation

### Effort Estimate

| Component | Estimate | Notes |
|-----------|----------|-------|
| Schema v6 → v7 migration | 3 days | New DataKey variants, read compatibility |
| Entrypoint updates (`init`, `settle`, `withdraw`) | 3 days | Validation + settlement routing |
| Token validation layer | 2 days | SEP-41 compliance checks |
| Unit tests (token allowlist, settlement routing) | 3 days | Parametrized over token pairs |
| Integration tests (funding → settlement flow) | 3 days | Multi-token end-to-end |
| Documentation + audit prep | 2 days | ADR, security checklist, API docs |
| **Total** | **16 days** | ~3-week sprint |

### Milestones

**Week 1:** Storage schema + allowlist validation
- [ ] New DataKey variants defined + read/write helpers
- [ ] `validate_settlement_token()` function
- [ ] Unit tests for token allowlist logic

**Week 2:** Entrypoint integration + settlement routing
- [ ] `init()` accepts settlement allowlist
- [ ] `settle()` accepts settlement token parameter
- [ ] Settlement token recorded in escrow state
- [ ] Error code 42 (UnsupportedSettlementToken) tests

**Week 3:** Integration tests + audit prep
- [ ] End-to-end funding → settlement flows (multiple token pairs)
- [ ] Backward compatibility tests (v6 instances settle in funding token only)
- [ ] Error handling (invalid tokens, unlisted tokens, validation failures)
- [ ] ADR draft + security checklist

### Blockers

- [ ] No known blockers as of 2026-07-27
- [ ] Assumes Stellar node supports `name()` and `decimals()` queries (confirmed via testnet)

### Implementation Notes

- Use `Vec::contains()` for allowlist checks (O(n) but n ≤ 10 typically)
- Cache token metadata in instance storage to avoid repeat queries
- Maintain strict separation: funding token vs. settlement token (different data paths)
- Error code 42 for "unsupported settlement token" (new)

---

## Acceptance Criteria

- [ ] `settle()` accepts optional settlement token parameter
- [ ] Settlement token must be in allowlist or error code 42 is returned
- [ ] If allowlist is empty/None, only funding token is valid for settlement
- [ ] Pro-rata payout calculated correctly (denominated in funding token)
- [ ] Settlement token recorded in `SettlementTokenFunded` for accounting
- [ ] Backward compatible: old v6 instances default to funding token settlement
- [ ] Token validation fails with typed error codes (41 for malformed, 42 for unlisted)
- [ ] 95%+ code coverage maintained (unit + integration tests)
- [ ] ADR-009 drafted and linked
- [ ] Documentation updated: signature changes, data model, error codes
- [ ] Zero security findings from audit

---

## Rollout Plan

### Phase 1: Testnet Beta (Week 1–2)

- Deploy to testnet with 3 test tokens (mock USDC, mock EURC, mock other)
- Run parametrized tests (all token pair combinations)
- Collect metrics: settlement latency, gas cost per token
- Solicit team feedback

**Success criteria:** All integration tests pass; no regressions on existing escrows.

### Phase 2: Early Partner Testing (Week 3)

- Release `v2.0-beta` to 2–3 early partners
- Partners run closed testing on testnet
- Collect UX feedback + operational concerns
- Refine error messages based on partner feedback

**Success criteria:** Partners confirm settlement flows work; no show-stoppers.

### Phase 3: Mainnet Release (Week 4)

- Audit completion + fixes applied
- Release `v2.0` to mainnet Friday afternoon (UTC)
- Monitor settlement volume + error rates over weekend
- Rollback plan: revert to `v1.4` if error rate > 1%

**Success criteria:** Settlement success rate ≥ 99.9% for 48 hours.

### Monitoring

**Key metrics:**
- Settlement success rate (target: 99.9%)
- Per-token settlement volume (track adoption)
- Average settlement gas cost per token
- Error rate by code (42 = unsupported token, etc.)

**Dashboards:**
- Grafana: settlement latency + error rates (real-time)
- BigQuery: historical settlement data by token (daily rollup)

**Alerts:**
- Settlement error rate > 1% → page on-call
- Token validation failures increasing → investigate

---

## References

- **Stellar SEP-41 Token Interface:** https://stellar.org/protocol/core/cap-0046-06
- **GitHub Issue #847:** "Support EURC settlement" (karis-ky/escrow-contracts#847)
- **Token Integration Checklist:** [`docs/ESCROW_TOKEN_INTEGRATION_CHECKLIST.md`](../ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)
- **ADR-006 (Token Safety):** [`docs/adr/ADR-006-dust-sweep-and-token-safety.md`](../adr/ADR-006-dust-sweep-and-token-safety.md)
- **Related RFC:** RFC-002 (Yield Reinvestment) — may need adjustment for multi-token

---

## Decision

**Owner:** Platform Lead  
**Status:** DRAFT (awaiting team feedback)  
**Decision date:** TBD (target: 2026-08-10)

---

## Revision History

| Date | Status | Change |
|------|--------|--------|
| 2026-07-27 | DRAFT | Initial proposal |
| — | DISCUSSION | Awaiting team feedback (3+ reviewers) |
| — | ACCEPTED | Decision made by platform lead |
| — | IMPLEMENTED | Feature shipped |

