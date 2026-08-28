# ✅ Token-Based Governance for Schema Upgrades — Complete Design

**Date:** July 27, 2026  
**Status:** RFC-005 DRAFT + Design Document Complete

---

## Problem & Solution

### Problem
Schema version upgrades are admin-gated (single multisig):
- Community has no transparency (don't know upgrades are planned)
- Token holders have no voice (centralized decision-making)
- Breaking changes deployed without notice
- No audit trail of who approved what changes
- Emergency overrides can't be overridden if compromised

### Solution
**Token-based governance voting on schema upgrades:**
- Submit proposals on-chain (7-day voting period)
- Token holders vote (1 token = 1 vote)
- 51%+ approval required for upgrades
- Full audit trail (events logged on-chain)
- Emergency admin override (fallback, logged)

---

## Deliverables

### 1. RFC-005: Token-Based Governance (591 lines)
- **Status:** DRAFT
- **Path:** `docs/rfc/RFC-005-token-governance-schema-upgrades.md`
- **Sections:**
  - Problem statement: lack of community alignment on upgrades
  - High-level governance design
  - Governance contract interface
  - Escrow integration changes
  - Proposal lifecycle + workflow
  - 3 alternatives considered + rationale
  - Effort: 14–20 days
  - Acceptance criteria (13 items)
  - 3-phase rollout (testnet → token testing → mainnet)
  - Security model + threat analysis

### 2. Technical Design Document (528 lines)
- **Path:** `docs/escrow-governance-voting.md`
- **Sections:**
  - Governance model overview
  - Actors (platform, token holders, governance contract, escrow)
  - Proposal lifecycle (submitted → voting → approved → executed → closed)
  - Token-weighted voting mechanics
  - Snapshot-based voting (prevents vote buying)
  - Governance contract interface
  - Escrow contract integration
  - Data model (on-chain storage keys)
  - Workflow examples (standard vote, emergency override, failed vote)
  - Governance parameters (configurable voting period, approval threshold)
  - Audit trail (events, query capabilities)
  - Backward compatibility (opt-in, existing escrows unaffected)
  - Security threat analysis + assumptions
  - Comparison with other governance models

---

## Key Design Decisions

### 1. Token-Weighted Voting

```
Voting Power = Token Balance at Proposal Submission

Prevents vote buying:
  - Voting power determined at proposal creation (immutable snapshot)
  - Can't acquire tokens during voting period to swing vote
  - Snapshot checked against governance token contract
```

### 2. Approval Threshold

```
Default: 51% (51% + 1 of participating voters)
Updatable via governance

Example:
  YES votes: 1.2M tokens
  NO votes: 800K tokens
  Approval: 1.2M / (1.2M + 800K) = 60% → APPROVED ✓
```

### 3. Voting Period

```
Default: 7 days (604,800 seconds)
Updatable via governance

Timeline:
  T=0: Proposal submitted (voting begins)
  T+7 days: Voting closes
  T+8 days: Anyone can execute if approved
  T+9 days: WASM upgrades, schema version changes
```

### 4. Proposal Types

```
✓ WasmUpgrade: Additive-only (new DataKey variant)
✓ Migration: Breaking change (requires migrate() call)
✓ BreakingSchema: Redeploy (struct layout change)
```

### 5. Emergency Override

```
Admin can bypass voting (fallback if governance contract down)
  → Logged event: EmergencyAdminUpgrade { reason }
  → Community sees reason on-chain
  → Used only for security patches (norm enforcement)
```

### 6. Backward Compatibility

```
✓ Existing escrows (no governance): Continue with admin-only
✓ New escrows: Can opt into governance at init
✓ Optional conversion: Admin can enable governance anytime
```

---

## Governance Workflow

### Standard Upgrade Path

```
T+0 days (Proposal Submitted):
  Platform calls governance.submit_upgrade_proposal()
  → Proposal ID assigned
  → Voting deadline set (T+7 days)
  → Event: ProposalSubmitted { id, deadline }

T+0-7 days (Voting Period):
  Token holders call governance.vote(proposal_id, yes/no)
  → Voting power = token balance at T+0
  → No re-voting allowed
  → Vote counts accumulate

T+7 days (Voting Closes):
  Anyone calls governance.finalize_vote(proposal_id)
  → Approval calculated: yes / (yes + no)
  → Status: APPROVED (if >= 51%) or REJECTED (if < 51%)

T+8+ days (Execute Approved Upgrade):
  Anyone calls governance.execute_proposal(proposal_id)
  → Governance verifies APPROVED status
  → Calls escrow.upgrade_via_governance(proposal_id, wasm_hash)
  → Escrow verifies schema versions + WASM hash
  → env.deployer().update_current_contract_wasm(wasm_hash)
  → Event: UpgradeExecuted { proposal_id, schema_v7 }

T+9+ days (Deployed):
  All nodes running new WASM
  New escrows can use new features
  Old escrows continue (backward compat)
```

### Emergency Override Path

```
Discovery: Critical security bug
Timeline: Need fix in hours (voting too slow)

Platform calls escrow.upgrade_admin_fallback(wasm_hash, reason)
  → Requires admin auth
  → WASM upgrades immediately
  → Event: EmergencyAdminUpgrade { reason, timestamp }

Post-incident:
  Community review: Why was override used?
  Post-mortem: Root cause analysis
  Next upgrade: Formal governance vote (builds trust)
```

---

## Data Model

### Governance Contract Storage

```
ProposalCount: u64                    // Current proposal ID counter
Proposal(u64): UpgradeProposal        // Proposal details by ID
VoterRecord(addr, id): bool           // Who voted on what
VotingPeriodSecs: u64                 // Default: 604800 (7 days)
ApprovalThresholdBps: u32             // Default: 5100 (51%)
TokenContract: Address                // Immutable reference
PlatformAddress: Address              // Who can submit proposals
ExecutedProposalLog(u64): u64         // Timestamp of execution
```

### Escrow Contract Storage

```
GovernanceContract: Address           // Optional governance address
LastGovernanceProposalId: u64         // Last approved proposal ID
LastGovernanceUpgradeAt: u64          // Timestamp of last upgrade
```

---

## Performance Metrics

### Governance Overhead

```
Submission: ~100 ms (write proposal to storage)
Voting: ~50 ms per vote (read + update vote counts)
Finalization: ~500 ms (calculate approval %)
Execution: ~200 ms (verify + call WASM upgrade)

Total time from submission to upgraded WASM: ~7-10 days
(Mostly waiting for voting period, not computation)
```

### Storage Cost

```
Per proposal: ~2 KB (proposal details + vote counts)
Per vote: ~50 bytes (voter address + vote flag)

Example (1M votes on one proposal):
  Proposal: 2 KB
  Votes: 50 MB (mostly temporary, cleared after finalization)
```

---

## Governance Parameters (Configurable)

### Default Configuration

```
voting_period_secs: 604800            // 7 days
approval_threshold_bps: 5100          // 51%+ approval
min_participation_bps: 0               // No quorum (vote passes if 51%+ of voters yes)
emergency_override_allowed: true       // Admin can bypass voting
proposal_submission_fee: 0             // Free submission (v2.2)
```

### Updating Parameters

Via governance meta-proposal:
```
Proposal Type: GovernanceParamUpdate {
  voting_period_secs: Some(1209600),   // Change to 14 days
  approval_threshold_bps: Some(6000),  // Change to 60%+
}

Vote on parameter change, same 7-day process
If approved, takes effect immediately after execution
```

---

## Audit Trail & Transparency

### Events Emitted

```
ProposalSubmitted(id, proposer, escrow, old_schema, new_schema, deadline)
VoteCast(proposal_id, voter, vote_yes/no, voting_power)
VoteFinalized(proposal_id, yes_votes, no_votes, approval%)
UpgradeExecuted(proposal_id, escrow, schema_v7, wasm_hash, executor, timestamp)
ProposalCancelled(proposal_id, cancelled_by, reason)
EmergencyAdminUpgrade(escrow, admin, wasm_hash, reason, timestamp)
```

### Query Capabilities

Community can query on-chain:
```
get_proposal(id)                    → Full proposal details
list_proposals("EXECUTED", limit)   → All executed upgrades
list_proposals("VOTING", limit)     → Active votes
list_proposals("APPROVED", limit)   → Pending executions
```

**Example audit report:**
```
All Schema Upgrades (governance-approved):
├─ v5 → v6: 2026-06-15, 85% approval, EXECUTED
├─ v6 → v7: 2026-07-27, 60% approval, EXECUTED
└─ v7 → v8: 2026-09-30, VOTING (deadline 2026-10-07)

Emergency Admin Overrides:
├─ 2026-06-01: Security patch (token overflow)
└─ 2026-08-15: Hotfix (precision rounding)
```

---

## Security Model

### Threat Analysis

| Threat | Mitigation | Cost to Execute |
|---|---|---|
| Vote buying | Snapshot-based voting power | Buy 26%+ tokens on market |
| Double voting | Contract prevents re-voting | Logic enforcement |
| Governance exploit | Audited code + bug bounty | Depends on vulnerabilities |
| Admin abuse | Logged event (community fork if needed) | Admin key compromise + trust loss |

### Assumptions

✓ Token contract is secure (audited)  
✓ Governance contract code is correct (will audit)  
✓ Community monitors votes (engaged token holders)  
✓ Platform uses emergency override responsibly (norm enforcement)  

---

## Implementation Roadmap

| Phase | Duration | Components |
|---|---|---|
| **Week 1** | 5–7 days | Governance contract (voting, approval calc, execution) |
| **Week 2** | 1–2 days | Escrow integration (upgrade_via_governance entrypoint) |
| **Week 3** | 3–4 days | Tests (voting logic, integration, edge cases) |
| **Week 4** | 2–3 days | Documentation + audit prep |
| **Total** | 14–20 days | ~3-week sprint |

---

## Use Cases Enabled

✓ **Community alignment** — 51%+ token holders must approve schema changes  
✓ **Transparency** — All proposals + votes logged on-chain  
✓ **Planning horizon** — 7-day notice before upgrades  
✓ **Integrator coordination** — Integrators time their updates to governance timeline  
✓ **Emergency response** — Admin can patch security bugs, logged for review  
✓ **Governance evolution** — Parameters updatable via governance (voting on voting!)  

---

## Backward Compatibility

### Existing Escrows (No Change)

Escrows deployed before governance launches:
- Continue with admin-only upgrades (default)
- No governance contract address set
- Backward fully compatible

### Opt-In Governance

New escrows can enable governance:
1. Set `governance_address` at init
2. All future upgrades require governance votes
3. Can opt-back-out if needed (set to zero)

---

## Next Steps

1. **RFC Discussion** (starting 2026-07-27)
   - Solicit feedback from 3–5 reviewers
   - Address concerns on voting mechanics
   - Target approval: 2026-08-10

2. **Implementation** (if approved)
   - Build governance contract (testnet first)
   - Integrate with escrow contract
   - Set up off-chain proposal publishing

3. **Testing** (Q1 2027)
   - Testnet: Submit dummy proposals + test votes
   - Partner testing: Real community feedback
   - Iterate on UX (voting interface)

4. **Mainnet** (Q2 2027, synchronized with token launch)
   - Deploy governance contract
   - First binding proposal: Schema v7 upgrade (RFC-004)
   - Monitor participation + sentiment

---

## Files Generated

| File | Lines | Purpose |
|---|---|---|
| `docs/rfc/RFC-005-token-governance-schema-upgrades.md` | 591 | RFC proposal (DRAFT) |
| `docs/escrow-governance-voting.md` | 528 | Technical governance design |
| **Total** | **1,119** | Complete governance design |

---

## Summary

✅ **RFC-005 DRAFT** — Token-based governance for schema upgrades  
✅ **Technical design** — Complete voting mechanism + integration  
✅ **Transparency model** — On-chain audit trail + query capabilities  
✅ **Security analysis** — Threat model + mitigations  
✅ **Backward compatibility** — Existing escrows unaffected, opt-in for new ones  
✅ **Implementation roadmap** — 3-week sprint to mainnet  

**Next:** RFC discussion (3–5 reviewers, 1 week), then approval for v2.2 implementation.

