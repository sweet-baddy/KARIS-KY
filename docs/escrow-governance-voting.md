# Token-Based Governance for Schema Upgrades

**Status:** Design (RFC-005 DRAFT)  
**Purpose:** Community voting on smart contract schema version upgrades  
**Applies to:** Schema version transitions (v6 → v7, v7 → v8, etc.)

---

## Overview

Token holders vote on schema version upgrades before deployment. This provides:

- **Transparency:** Community knows upcoming upgrades + timeline
- **Alignment:** 51%+ token holders must approve schema changes
- **Safety:** Voting period allows integrators to plan + upgrade
- **Auditability:** Vote history logged on-chain
- **Bypass:** Admin can emergency-override (fallback, logged)

---

## Governance Model

### Actors

| Role | Responsibility | Examples |
|------|---|---|
| **Platform** | Submits upgrade proposals | "Schema v6 → v7: off-chain storage support" |
| **Token Holders** | Vote approve/reject | Investors, integrators, community |
| **Governance Contract** | Manages proposals + votes | Soroban smart contract |
| **Escrow Contract** | Executes approved upgrades | Calls `update_current_contract_wasm()` |

### Proposal Lifecycle

```
DRAFT (off-chain discussion)
  ↓ (platform decides to propose)
SUBMITTED (on-chain proposal created, voting begins)
  ↓ (7-day voting period)
VOTING (token holders vote yes/no)
  ↓ (voting deadline passes)
APPROVED (51%+ yes votes) or REJECTED (< 51% yes)
  ↓ (if APPROVED)
EXECUTED (anyone calls execute_proposal, WASM upgrades)
  ↓
CLOSED (archived)
```

**Alternative paths:**
- SUBMITTED → CANCELLED (proposer or governance can cancel anytime)
- APPROVED → CANCELLED (before execution)

### Voting Mechanics

**Token-Weighted Voting:**
```
Voting Power = Token Balance at Proposal Submission Time

Example (1M total tokens distributed):
  Holder A: 100K tokens → 100K votes
  Holder B: 50K tokens → 50K votes
  Holder C: 10K tokens → 10K votes

Approval Threshold: 51% of participating voters
  yes_votes ≥ 0.51 × (yes_votes + no_votes)
```

**Snapshot-Based Voting Power:**
- Voting power determined **at proposal submission** (immutable)
- Prevents vote buying (can't acquire tokens during voting period to swing vote)
- Balances checked against governance token contract

**Example Vote:**
```
Proposal: Schema v6 → v7
Voting Period: 7 days (604,800 seconds)

Day 1: 100K tokens vote YES
Day 3: 80K tokens vote NO
Day 5: 20K tokens vote YES (total YES: 120K, NO: 80K)
Day 7: Voting closes

Result:
  YES: 120K
  NO: 80K
  Participation: 200K / 1M = 20% (low, but vote is clear)
  Approval: 120K / (120K + 80K) = 60% → APPROVED ✓
```

---

## Contract Interfaces

### Governance Contract

```rust
pub struct UpgradeProposal {
    pub id: u64,
    pub proposal_type: String,  // "WasmUpgrade" | "Migration" | "BreakingSchema"
    pub escrow_address: Address,
    pub new_schema_version: u32,
    pub old_schema_version: u32,
    pub wasm_hash: BytesN<32>,  // (if WasmUpgrade)
    pub description: String,
    pub proposer: Address,
    pub submission_timestamp: u64,
    pub voting_deadline: u64,
    pub status: String,         // SUBMITTED, VOTING, APPROVED, REJECTED, EXECUTED, CANCELLED
    pub yes_votes: i128,
    pub no_votes: i128,
    pub total_participants: u32,
}

pub fn submit_upgrade_proposal(
    env: Env,
    escrow_address: Address,
    old_version: u32,
    new_version: u32,
    wasm_hash: BytesN<32>,
    proposal_type: String,
    description: String,
) -> Result<u64, GovernanceError>;

pub fn vote(
    env: Env,
    proposal_id: u64,
    vote: bool,  // true = yes, false = no
) -> Result<(), GovernanceError>;

pub fn finalize_vote(
    env: Env,
    proposal_id: u64,
) -> Result<(), GovernanceError>;

pub fn execute_proposal(
    env: Env,
    proposal_id: u64,
) -> Result<(), GovernanceError>;

pub fn get_proposal(
    env: Env,
    proposal_id: u64,
) -> Result<UpgradeProposal, GovernanceError>;

pub fn list_proposals(
    env: Env,
    status_filter: String,  // "APPROVED", "REJECTED", "EXECUTED", etc.
    limit: u32,
) -> Result<Vec<u64>, GovernanceError>;
```

### Escrow Contract

```rust
pub fn upgrade_via_governance(
    env: Env,
    governance_proposal_id: u64,
    new_wasm_hash: BytesN<32>,
) -> Result<(), EscrowError>;

pub fn upgrade_admin_fallback(
    env: Env,
    new_wasm_hash: BytesN<32>,
    reason: String,  // Logged for audit trail
) -> Result<(), EscrowError>;  // Admin-only, governance unavailable

pub fn get_governance_contract(
    env: Env,
) -> Result<Address, EscrowError>;

pub fn set_governance_contract(
    env: Env,
    governance_address: Address,
) -> Result<(), EscrowError>;  // Admin-only, init or update
```

---

## Data Model

### On-Chain Storage (Governance Contract)

```rust
pub enum GovernanceDataKey {
    /// Current proposal counter (used for ID assignment)
    ProposalCount,
    
    /// Proposal details (map by proposal ID)
    Proposal(u64),
    
    /// Per-voter tracking (who voted on which proposals)
    VoterRecord(Address, u64),  // (voter_address, proposal_id) → true if voted
    
    /// Voting parameters
    VotingPeriodSecs,
    ApprovalThresholdBps,       // e.g., 5100 = 51%
    
    /// Token contract reference (immutable)
    TokenContract,
    
    /// Platform proposal submitter (who can submit new proposals)
    PlatformAddress,
    
    /// Archive of executed proposals (immutable log)
    ExecutedProposalLog(u64),   // proposal_id → execution_timestamp
}
```

### On-Chain Storage (Escrow Contract)

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// Governance contract address (optional)
    /// Immutable after init. If absent, use admin-only upgrades.
    GovernanceContract,
    
    /// Latest governance proposal ID executed (for auditing)
    LastGovernanceProposalId,
    
    /// Timestamp of last governance upgrade (for audit trail)
    LastGovernanceUpgradeAt,
}
```

---

## Workflow Examples

### Example 1: Standard Governance Upgrade

**Scenario:** Deploy schema v7 (off-chain storage support)

```
T=0 (Monday):
  RFC-004 published (2-week community discussion)

T+14 days (Monday):
  Platform calls governance.submit_upgrade_proposal(
    escrow_address,
    old_version: 6,
    new_version: 7,
    wasm_hash: 0xabc123...,
    proposal_type: "WasmUpgrade",
    description: "Add off-chain storage support (RFC-004)"
  )
  → Returns proposal_id = 42
  → Voting begins
  → Event: ProposalSubmitted { id: 42, deadline: T+21 days }

T+21 days (Monday):
  On-chain voting closes
  Vote result: 1.2M YES, 800K NO (60% approval, 30% participation)
  Status: APPROVED ✓

T+22 days (Tuesday):
  Anyone calls governance.finalize_vote(42)
  → Status changes to APPROVED (finalization is idempotent)

T+23 days (Wednesday):
  Anyone calls governance.execute_proposal(42)
  → Governance calls escrow.upgrade_via_governance(42, 0xabc123...)
  → Escrow verifies proposal details + approval
  → Escrow calls env.deployer().update_current_contract_wasm(0xabc123...)
  → WASM bytecode updates
  → Status: EXECUTED
  → Event: UpgradeExecuted { proposal_id: 42, schema_v7 }

T+24 days (Thursday):
  All Soroban nodes running new WASM
  New escrows can opt into MERKLE storage mode
  Existing escrows continue on-chain (backward compat)
```

### Example 2: Emergency Admin Override

**Scenario:** Critical security bug in v7, need immediate patch

```
Discovery: Platform finds token transfer bug in v7
Timeline: Need fix ASAP (voting would take 7 days)
Decision: Use admin emergency override

Process:
  Platform calls escrow.upgrade_admin_fallback(
    new_wasm_hash: 0xdef456...,
    reason: "Emergency security patch: fix token transfer overflow"
  )
  → Requires admin auth
  → Logs event: EmergencyAdminUpgrade { reason, timestamp }
  → WASM updates immediately
  → Status: PATCH_DEPLOYED

Post-Patch:
  Community review: Why was admin override used?
  Platform publishes post-mortem: Root cause analysis + fixes
  Next cycle: Governance proposal for v8 (formal upgrade + vote)
```

### Example 3: Failed Vote (Resubmission)

**Scenario:** Community votes down initial proposal

```
Proposal 1 submitted (v6 → v7)
Community concerns: "Off-chain storage too complex, need more time"
Vote: 40% YES, 60% NO → REJECTED

Response:
  Platform publishes detailed implementation guide
  Addresses concerns in updated RFC
  Waits 2 weeks for community feedback

Proposal 2 submitted (same v6 → v7, with updated description)
  Vote: 75% YES → APPROVED ✓
  Execute as normal
```

---

## Governance Parameters

### Default Configuration

```
voting_period_secs: 604800        // 7 days
approval_threshold_bps: 5100      // 51%+
min_participation_bps: 0           // No quorum (just 51% of voters)
emergency_override_allowed: true   // Admin can bypass voting
```

### Parameter Updates (via Governance)

Parameters can be updated via governance meta-proposal:

```
Proposal Type: GovernanceParamUpdate {
  voting_period_secs: Some(1209600),    // Change to 14 days
  approval_threshold_bps: Some(6000),   // Change to 60%+
}
```

**Governance vote on parameter changes:**
- Same voting mechanism (51%+ approval)
- Takes effect after execution (one-cycle delay)
- Example: Change voting period from 7 to 14 days (next proposal uses new period)

---

## Audit Trail

### Events Emitted

```rust
// On proposal submission
event ProposalSubmitted {
    id: u64,
    proposer: Address,
    escrow_address: Address,
    old_schema: u32,
    new_schema: u32,
    deadline: u64,
    description: String,
}

// On vote
event VoteCast {
    proposal_id: u64,
    voter: Address,
    vote: bool,
    voting_power: i128,
}

// On vote finalization
event VoteFinalized {
    proposal_id: u64,
    yes_votes: i128,
    no_votes: i128,
    status: String,  // APPROVED or REJECTED
    approval_percentage: u32,  // bps
}

// On upgrade execution
event UpgradeExecuted {
    proposal_id: u64,
    escrow_address: Address,
    old_schema: u32,
    new_schema: u32,
    wasm_hash: BytesN<32>,
    executed_by: Address,
    timestamp: u64,
}

// On emergency admin override
event EmergencyAdminUpgrade {
    escrow_address: Address,
    admin: Address,
    wasm_hash: BytesN<32>,
    reason: String,
    timestamp: u64,
}

// On proposal cancellation
event ProposalCancelled {
    proposal_id: u64,
    cancelled_by: Address,
    reason: String,
}
```

### Query Capabilities

**Community can query:**
```
governance.get_proposal(id) → Full proposal details
governance.list_proposals("EXECUTED", limit=100) → All executed upgrades
governance.list_proposals("APPROVED", limit=50) → Pending executions
governance.list_proposals("VOTING", limit=20) → Active votes

For each proposal:
  - Submission time + deadline
  - Vote counts (yes/no)
  - Approval %
  - WASM hash (can verify against deployed code)
```

**Audit report example:**
```
All Schema Upgrades (governance-approved):
├─ Proposal 1: v5 → v6 (2026-06-15, 85% approved, EXECUTED)
├─ Proposal 2: v6 → v7 (2026-07-27, 60% approved, EXECUTED)
└─ Proposal 3: v7 → v8 (2026-09-30, VOTING, deadline 2026-10-07)

Emergency Admin Overrides:
├─ 2026-06-01: Security patch v5.1 (token overflow fix)
└─ 2026-08-15: Hotfix v7.1 (precision rounding bug)
```

---

## Backward Compatibility

### Existing Escrows (No Governance)

Escrows deployed before governance launches continue:
- Upgrades via admin-only (default behavior, backward compat)
- No governance contract address set
- `upgrade_via_governance()` fails with "governance not configured"

### Opt-In Governance

To enable governance for an escrow:
1. Admin calls `set_governance_contract(governance_address)`
2. Future upgrades require governance votes
3. Can be changed back to admin-only (governance address set to zero)

---

## Security Model

### Threat Analysis

| Threat | Mitigation | Cost to Execute |
|---|---|---|
| Vote buying (acquire 26%+ tokens) | Snapshot-based voting power (fixed at proposal time) | Cost of buying 26% on secondary market |
| Double voting | Contract tracks voted_flag per voter per proposal | Contract logic enforcement |
| Governance contract exploit | Audited code + bug bounty | Depends on vulnerabilities |
| Admin keysteal + emergency override | Logged event (community can see + fork if needed) | Requires admin compromise + community trust loss |
| Spam proposals | Proposal submission fee (optional, v2.3+) | Cost threshold TBD |

### Assumptions

✓ Token contract is secure (has been audited)  
✓ Governance contract code is correct (will be audited)  
✓ Community monitors votes (engaged token holders)  
✓ Platform won't abuse emergency override (operational norm)  

---

## Comparison: Governance Models

| Model | Pros | Cons | Used By |
|---|---|---|---|
| **Admin-only** (today) | Fast, simple | Centralized, no community input | v1–v6 |
| **Token governance** (RFC-005) | Decentralized, transparent, aligned | Slower (~7 days), complexity | v7+ (proposed) |
| **Timelock only** | Faster than voting (notice period) | No real veto | Compound, Aave (emergency) |
| **Veto voting** (future) | Fast default, community override | Burden on token holders to veto | Governance DAO (Gnosis) |

---

## Migration from Admin-Only

### Timeline

**Phase 1 (v2.1):** Deploy governance contract (testnet)  
**Phase 2 (v2.2):** Enable governance on new escrows (mainnet)  
**Phase 3 (v2.3):** Migrate existing escrows to governance (optional)  

### Existing Escrow Upgrade Path

For escrows deployed under admin-only model:

```
Option A: Keep Admin-Only (Forever)
  → No governance_contract set
  → Upgrades via admin-only
  → Backward compatible

Option B: Opt Into Governance
  → Admin calls set_governance_contract(governance_addr)
  → Future upgrades require governance votes
  → Can opt-back-out if needed

Option C: New Escrow Under Governance
  → Initialize with governance_address at init
  → All upgrades require votes from day 1
```

---

## References

- **RFC-005:** Token-Based Governance for Schema Upgrades
- **ADR-007:** Storage Key Evolution (schema versioning)
- **OPERATOR_RUNBOOK:** Redeploy vs. On-Chain Upgrade (current process)
- **Compound Governance:** https://compound.finance/docs/governance
- **Aave Governance:** https://aave.com/

