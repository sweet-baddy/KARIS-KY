# RFC-005: Token-Based Governance for Schema Version Upgrades

**Status:** DRAFT  
**Author:** Platform Governance (@karis-ky)  
**Date Proposed:** 2026-07-27  
**Target Release:** v2.2 (Q3 2027)  
**Related:** ADR-007 (Storage Evolution), OPERATOR_RUNBOOK.md, Issue #1567

---

## Summary

This RFC proposes a token-based governance mechanism that requires token holders to vote on schema version upgrades (migration from SCHEMA_VERSION N → N+1) before deployment. This introduces transparency, community alignment, and procedural safety gates while preserving operator flexibility for emergency patches and additive-only changes.

---

## Motivation

**Problem Statement:**

Today, schema version upgrades are admin-gated (single address or multisig). This model has gaps:

1. **Lack of transparency:** Community doesn't know schema changes are planned until deployed
2. **No stakeholder alignment:** Token holders (investors, integrators) have no voice in upgrades
3. **Risk concentration:** Single multisig can upgrade without consensus (key loss = permanent freeze)
4. **Breaking changes deployed without notice:** Investors unaware of new requirements
5. **No emergency override:** If governance is compromised, no rapid response path

**Impact:**
- Token holders trust is eroded (governance feels centralized)
- Upgrades can disrupt large integrations (backwards-incompatible changes)
- Regulatory concern (no audit trail for schema changes)
- Community adoption suffers (platforms need decentralization credibility)

**Why now:**
- Schema v6 → v7 planned (off-chain storage, RFC-004)
- Platform maturing (need stronger governance narrative)
- Community expanding (3+ institutional integrators asking about governance)
- Token launch expected Q3 2026 (natural point to introduce voting)

**Use Cases:**

1. **Community alignment:** Before v2.2 ships, 51%+ token holders vote "yes" on schema changes
2. **Transparency audit:** Community can see all pending upgrades + vote history
3. **Emergency override:** If governance multisig compromised, community can vote to replace admin
4. **Integrator planning:** Integrators get 7-day notice + community vote deadline for planning
5. **Regulatory compliance:** Audit trail of who voted, when, on what schema changes

**Success Metric:**
- Token holder participation ≥ 30% on first governance vote
- 95%+ voter approval for planned upgrades
- Zero emergency admin changes (rely on governance instead)
- Upgrade notices published 2 weeks before vote deadline

---

## Design

### Overview

**High-level approach:**

1. **Governance contract** (new, DAO-style)
   - Holds upgrade proposal queue
   - Manages token holder votes
   - Executes approved upgrades (calls escrow contract upgrade)

2. **Escrow contract changes (minimal)**
   - Accept upgrade proposal ID from governance contract
   - Verify proposal is approved + ready to execute
   - Proceed with upgrade (or redeploy, depending on change type)

3. **Token-weighted voting**
   - 1 token = 1 vote
   - Voting power snapshot at proposal creation
   - 7-day voting period (configurable)
   - 51%+ approval threshold (configurable)

4. **Proposal lifecycle**
   - **DRAFT:** Platform proposes upgrade (off-chain discussion)
   - **SUBMITTED:** Governance contract receives proposal + vote period starts
   - **VOTING:** Token holders vote (7 days)
   - **APPROVED:** Vote passes (51%+ yes votes) → ready to execute
   - **REJECTED:** Vote fails → proposal archived
   - **EXECUTED:** Escrow upgrades to new WASM
   - **CANCELLED:** Admin/governance can cancel anytime

### Detailed Design

**Component 1: Governance Contract Interface**

```rust
// Governance contract (separate deployment, platform-operated initially)

pub enum UpgradeProposal {
    /// WASM upgrade (additive-only, safe)
    WasmUpgrade {
        escrow_address: Address,
        new_wasm_hash: BytesN<32>,
        description: String,
        schema_version_old: u32,
        schema_version_new: u32,
    },
    /// Migration (breaking change, redeploy likely needed)
    Migration {
        escrow_address: Address,
        from_version: u32,
        to_version: u32,
        migration_plan: String,  // Operator notes on migration steps
    },
    /// Schema breaking (redeploy with new init)
    BreakingSchema {
        escrow_address: Address,
        old_schema_version: u32,
        new_schema_version: u32,
        reason: String,
    },
}

pub struct GovernanceProposal {
    id: u64,
    proposal_type: UpgradeProposal,
    proposer: Address,
    submission_timestamp: u64,
    voting_deadline: u64,
    yes_votes: i128,
    no_votes: i128,
    token_holders_voted: u32,
    status: String,  // DRAFT, VOTING, APPROVED, REJECTED, EXECUTED, CANCELLED
}

pub trait GovernanceContract {
    /// Submit proposal for community vote
    pub fn submit_upgrade_proposal(
        env: Env,
        proposal_type: UpgradeProposal,
        description: String,
        voting_period_secs: u64,  // e.g. 7 days = 604800 secs
    ) -> Result<u64, GovernanceError>;  // returns proposal ID
    
    /// Vote on a proposal (token-weighted)
    pub fn vote(
        env: Env,
        proposal_id: u64,
        vote: bool,  // true = yes, false = no
    ) -> Result<(), GovernanceError>;
    
    /// Finalize vote (anyone can call after deadline)
    pub fn finalize_vote(
        env: Env,
        proposal_id: u64,
    ) -> Result<(), GovernanceError>;
    
    /// Execute approved upgrade proposal
    pub fn execute_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Result<(), GovernanceError>;
    
    /// Cancel proposal (proposer or governance can call)
    pub fn cancel_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Result<(), GovernanceError>;
    
    /// Query proposal details
    pub fn get_proposal(
        env: Env,
        proposal_id: u64,
    ) -> Result<GovernanceProposal, GovernanceError>;
    
    /// List all active proposals
    pub fn list_active_proposals(
        env: Env,
        limit: u32,
    ) -> Result<Vec<u64>, GovernanceError>;
}
```

**Component 2: Escrow Contract Integration**

New entrypoint (replaces admin-only `upgrade_wasm`):

```rust
pub fn upgrade_via_governance(
    env: Env,
    governance_proposal_id: u64,
    new_wasm_hash: BytesN<32>,
) -> Result<(), EscrowError> {
    // 1. Query governance contract for proposal details
    let governance = env.current_contract_id();  // or configured governance address
    let proposal = governance_contract.get_proposal(env.clone(), governance_proposal_id)?;
    
    // 2. Verify proposal is APPROVED + ready to execute
    if proposal.status != "APPROVED" {
        return Err(EscrowError::UpgradeNotApproved);  // error 60
    }
    
    if proposal.voting_deadline > env.ledger().timestamp() {
        return Err(EscrowError::UpgradeVotingStillActive);  // error 61
    }
    
    // 3. Verify proposal targets this escrow
    if proposal.escrow_address != env.current_contract_address() {
        return Err(EscrowError::UpgradeWrongEscrow);  // error 62
    }
    
    // 4. Verify schema version progression
    let current_version = Self::get_version(env.clone())?;
    let (expected_old, expected_new) = extract_versions_from_proposal(&proposal)?;
    if current_version != expected_old {
        return Err(EscrowError::UpgradeVersionMismatch);  // error 63
    }
    
    // 5. Verify WASM hash matches proposal
    if new_wasm_hash != proposal.wasm_hash {
        return Err(EscrowError::UpgradeWasmHashMismatch);  // error 64
    }
    
    // 6. Execute WASM upgrade
    env.deployer().update_current_contract_wasm(new_wasm_hash);
    
    // 7. Mark proposal as EXECUTED in governance contract
    governance_contract.mark_executed(env.clone(), governance_proposal_id)?;
    
    // 8. Emit event
    env.events().publish(
        ("upgrade_executed", env.current_contract_address()),
        (governance_proposal_id, new_wasm_hash, expected_new),
    );
    
    Ok(())
}
```

**Component 3: Data Model**

New storage keys for escrow:

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// Optional governance contract address (for upgrade voting).
    /// Immutable after init. Absent ⇒ upgrades via admin-only (backward compat).
    GovernanceContract,
    
    /// Minimum token holder approval percentage for upgrades (e.g., 51).
    /// Default: 51 (51% quorum + approval).
    UpgradeApprovalThreshold,
    
    /// Minimum voting period in seconds (e.g., 604800 = 7 days).
    UpgradeVotingPeriod,
}
```

**Component 4: Proposal Workflow**

```
Timeline for schema upgrade (v6 → v7):

T=0 (Monday):
  Platform publishes RFC-004 Off-Chain Storage
  Community discussion begins (2-week period)

T+14 days (Monday):
  Platform submits governance proposal "Schema v7 Upgrade"
  Voting begins (7 days)
  Proposal details published on-chain:
    - old schema: 6
    - new schema: 7
    - WASM hash: 0x1234...
    - migration plan: "See RFC-004, off-chain storage mode"

T+21 days (Monday):
  Voting closes
  Result: 72% yes votes → APPROVED ✓

T+22 days (Tuesday):
  Anyone calls execute_proposal(proposal_id)
  Governance contract verifies 51%+ approval
  Calls escrow.upgrade_via_governance(proposal_id, wasm_hash)
  Escrow upgrades WASM + version
  Event published: UpgradeExecuted { proposal_id, schema_v7 }
  All nodes see new WASM bytecode

T+23 days (Wednesday):
  Indexers update to v7 schema
  New escrows can opt into off-chain merkle storage
  Existing escrows continue with on-chain storage
```

### Examples

**Example 1: Additive-only upgrade (v6 → v7 with new DataKey)**

```rust
// Proposal submitted with:
proposal_type: UpgradeProposal::WasmUpgrade {
    escrow_address: current_escrow,
    new_wasm_hash: 0xabc123...,
    description: "Add InvestorDataMerkleRoot for off-chain storage",
    schema_version_old: 6,
    schema_version_new: 7,
}

// Voting (7 days)
// yes_votes: 15_000_000 tokens
// no_votes: 5_000_000 tokens
// Approval: 75% → APPROVED

// Execution
// call upgrade_via_governance(proposal_id, 0xabc123...)
// → WASM updates
// → DataKey::Version stays 6 (additive, no migration)
// → New escrows can set InvestorDataMerkleRoot at init
```

**Example 2: Breaking schema change (v5 → v6 with per-investor key changes)**

```rust
proposal_type: UpgradeProposal::BreakingSchema {
    escrow_address: current_escrow,
    old_schema_version: 5,
    new_schema_version: 6,
    reason: "Per-investor keys moved to persistent storage (ADR-007)",
}

// Voting (7 days)
// yes_votes: 18_000_000 tokens (90%)
// → APPROVED

// Execution
// call upgrade_via_governance(proposal_id, new_wasm_hash)
// → WASM updates
// → Operator then calls migrate(5) on existing instances
// → Existing escrows rewritten to v6 storage layout
```

**Example 3: Emergency upgrade (no voting, admin-only fallback)**

```
Scenario: Critical security bug discovered in v6
Timeline: Need to patch ASAP (voting too slow)

Option A: Emergency proposal (expedited)
  - Platform proposes emergency patch
  - Voting period: 24 hours (vs 7 days)
  - If 70%+ approval: execute immediately
  - If vote fails: fallback to admin upgrade

Option B: Admin-only override (if governance contract unavailable)
  - If governance contract down, admin can still upgrade
  - Must be documented + logged
  - Community review post-mortem
```

---

## Alternatives Considered

### Alternative 1: Pure Token Governance (No Admin Fallback) (Rejected)

**Approach:** Only allow upgrades via governance; no admin override.

**Pros:**
- True decentralization
- Forces governance participation

**Cons:**
- If governance contract bugs out, no emergency fix path
- Security vulnerabilities can't be patched quickly enough
- Network could halt indefinitely

**Decision:** Rejected. Admin fallback needed for security patches.

---

### Alternative 2: Timelock Without Voting (Rejected)

**Approach:** Admin can upgrade, but must wait 48 hours (timelock) before execution.

**Pros:**
- Simple (no voting logic needed)
- Community has notice + time to respond
- Faster than voting

**Cons:**
- Community has no actual vote (false participation)
- If timelock compromised, no recourse
- Doesn't solve lack of transparency

**Decision:** Rejected. Voting provides real governance, not just notice.

---

### Alternative 3: Token Holders Can Veto (Deferred)

**Approach:** Admin upgrades by default, but token holders can vote to block.

**Pros:**
- Fast path (default is upgrade)
- Community can still say "no"

**Cons:**
- Burden on token holders to monitor + veto
- Hard to organize rapid veto (7 days needed?)
- Approval-based (v1) is stronger signal

**Decision:** Deferred to v2.3. Approval-based (v2.2) is stronger semantically.

---

## Implementation

### Effort Estimate

| Component | Estimate | Notes |
|-----------|----------|-------|
| Governance contract (DAO-style) | 5–7 days | Token-weighted voting, proposal queue, execution |
| Escrow integration (upgrade_via_governance) | 1–2 days | Proposal verification, WASM upgrade call |
| Storage keys + metadata | 1 day | GovernanceContract, thresholds, periods |
| Unit tests (governance + escrow integration) | 3–4 days | Voting logic, approval calculation, version checks |
| Integration tests (end-to-end vote → upgrade) | 2–3 days | Submit → vote → finalize → execute |
| Documentation + audit prep | 2–3 days | Governance model, operator runbook updates |
| **Total** | **14–20 days** | ~3-week sprint |

### Milestones

**Week 1:** Governance contract core
- [ ] Proposal submission + storage
- [ ] Vote collection (token-weighted)
- [ ] Vote finalization + approval calculation
- [ ] Unit tests for voting logic

**Week 2:** Escrow integration + testing
- [ ] Escrow upgrade_via_governance entrypoint
- [ ] Proposal verification (version checks, hash matching)
- [ ] Integration tests (submit → vote → execute)
- [ ] Error codes 60–64

**Week 3:** Documentation + deployment prep
- [ ] Governance model spec (ADR-012?)
- [ ] Operator runbook updates (when to use governance)
- [ ] Security audit prep
- [ ] Emergency upgrade policy documented

### Blockers

- [ ] None identified as of 2026-07-27
- Assumes token contract exists + exposed at known address (prerequisite)
- Assumes governance contract can call escrow upgrade entrypoint (auth delegation needed)

### Implementation Notes

- Use **time-based snapshots** for voting power (immutable at proposal creation)
- **Token-weighted voting:** balance at proposal creation time = voting power (prevents vote buying)
- **No vote delegation** (v2.2); add in v2.3 if requested
- **Emergency override:** admin can upgrade without governance if governance contract unavailable (documented escape hatch)
- Error codes: 60–64 for upgrade governance errors

---

## Acceptance Criteria

- [ ] Governance contract accepts upgrade proposals from platform address
- [ ] Token holders can vote (1 token = 1 vote, snapshot-based)
- [ ] Vote finalization calculates approval %  correctly
- [ ] Escrow verifies proposal status + approval before upgrading
- [ ] Escrow verifies WASM hash matches proposal + schema versions correct
- [ ] Upgrade executed only after voting deadline passes
- [ ] Failed votes (< 51%) can be resubmitted as new proposal
- [ ] Admin can upgrade without governance (fallback) if governance unavailable
- [ ] All upgrade governance calls emit events
- [ ] Error codes 60–64 returned for governance failures
- [ ] 95%+ code coverage maintained
- [ ] Token-weighted voting prevents vote buying (snapshot-based)
- [ ] Documentation: governance model + operator procedures
- [ ] Zero security findings from audit

---

## Rollout Plan

### Phase 1: Testnet (Week 1–2)

- Deploy governance contract + escrow integration
- Submit test proposals (dummy upgrade + dummy vote)
- Verify voting logic + approval calculation
- Test with synthetic token holders (various balances)

**Success criteria:** Voting works correctly, edge cases handled.

### Phase 2: Token Holder Testing (Week 3)

- Publish governance model on-chain
- Invite community feedback (1 week)
- Conduct test vote (non-binding, social signal)
- Iterate on UX (voting interface improvements)

**Success criteria:** Community comfortable with voting model, no security issues.

### Phase 3: Mainnet (Week 4+, timed with token launch)

- Deploy governance contract to mainnet
- Publish governance parameter FAQ
- First binding proposal: Schema v7 upgrade (RFC-004)
- Monitor vote participation + sentiment

**Success criteria:** First proposal achieves 30%+ participation, clear outcome.

### Monitoring

**Key metrics:**
- % token holders participating in votes
- Approval %% (target: 70%+ for consensus)
- Proposal submission → execution time
- Emergency override usage (should be rare)

---

## Governance Parameters

Parameters can be updated via governance (meta-vote):

```rust
pub struct GovernanceParams {
    pub voting_period_secs: u64,           // Default: 604800 (7 days)
    pub approval_threshold_bps: u32,       // Default: 5100 (51%)
    pub emergency_override_allowed: bool,  // Default: true
    pub emergency_voting_period_secs: u64, // Default: 86400 (24 hours)
}
```

Updates via governance meta-proposal:
```
proposal_type: GovernanceParamUpdate {
    new_voting_period: Some(1209600),  // 14 days
    new_threshold: Some(6000),         // 60% (higher bar for breaking changes)
}
```

---

## References

- **ADR-007:** Storage Key Evolution (docs/adr/ADR-007-storage-key-evolution.md)
- **OPERATOR_RUNBOOK:** Redeploy vs. On-Chain Upgrade
- **RFC-004:** Off-Chain Storage with Merkle Proofs (schema v7 use case)
- **Stellar Governance:** https://developers.stellar.org/docs/learn/fundamentals/stellar-data-model
- **DAO Voting Patterns:** https://snapshot.org (Ethereum DAO reference)

---

## Security Considerations

### Attack Vectors

| Attack | Mitigation | Cost to Execute |
|--------|-----------|---|
| Vote buying (acquire tokens to swing vote) | Snapshot-based voting power (fixed at proposal time) | Cost of buying 26%+ tokens on secondary market |
| Governance contract exploit | Standard audits + bug bounty | Depends on vulnerabilities found |
| Admin keysteal + emergency upgrade | Emergency upgrade logged + monitored; community can veto by forking | Requires admin key compromise + community acceptance |
| Double voting | Voting contract prevents re-vote per address | Contract logic enforcement |

### Assumptions

✓ **Token contract is secure** (audited, not compromised)  
✓ **Governance contract logic is correct** (audited)  
✓ **Community monitors votes** (engaged token holders)  
✓ **Admin key is reasonably secure** (standard operational practices)  

---

## Decision

**Owner:** Platform Governance Lead  
**Status:** DRAFT (awaiting team feedback)  
**Decision date:** TBD (target: 2026-08-10)

---

## Revision History

| Date | Status | Change |
|------|--------|--------|
| 2026-07-27 | DRAFT | Initial proposal |
| — | DISCUSSION | Awaiting team feedback (3+ reviewers) |
| — | ACCEPTED | Decision made by governance lead |
| — | IMPLEMENTED | Governance contract + escrow integration shipped |

