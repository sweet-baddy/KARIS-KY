# RFC-004: Off-Chain Storage with Merkle Proof Verification

**Status:** DRAFT  
**Author:** Platform Architecture (@karis-ky)  
**Date Proposed:** 2026-07-27  
**Target Release:** v2.1 (Q2 2027)  
**Related:** Issue #1234, ADR-006 (Token Safety), ADR-007 (Storage Key Evolution)

---

## Summary

This RFC proposes an optional off-chain storage system for large escrows that store bulky data (investor records, historical contributions, collateral metadata) off-chain with on-chain Merkle root verification. This unbounds instance storage growth while maintaining cryptographic proof integrity, enabling escrows with 10,000+ investors without hitting Soroban's per-contract storage limits.

---

## Motivation

**Problem Statement:**

Today, all per-investor data lives on-chain:
- `InvestorContribution(Address)` in persistent storage
- `InvestorEffectiveYield(Address)` in persistent storage  
- `InvestorClaimNotBefore(Address)` in persistent storage
- `InvestorClaimed(Address)` in persistent storage

**Current constraints:**

1. **Large investor counts scale linearly in storage cost**
   - 1,000 investors → ~50 KB of persistent keys
   - 10,000 investors → ~500 KB of persistent keys
   - 100,000 investors → ~5 MB (effectively unbounded on Soroban)

2. **Storage fees accumulate**
   - Each persistent key has TTL rent
   - 10,000 investors = 10,000 independent TTL ledgers to extend
   - Operational overhead: `bump_ttl` must list all addresses

3. **Business model friction**
   - Large invoice pools (B2B network effects) incentivize 10K+ investors
   - But current design caps practical investor cardinality around 1K–2K
   - Competitor platforms (centralized) have no such limits

4. **Scalability ceiling**
   - Soroban instance storage ~1 MB soft limit (not hard, but expensive)
   - Per-address persistent keys → O(n) cost where n = investor count
   - Cannot efficiently query "top 100 investors by contribution"

**Impact:**
- Late-stage invoicing platforms request higher investor caps (3+ customers)
- Current architecture is limiting factor for platform growth
- No technical blocker (off-chain data is feasible), just design choice

**Why now:**
- Storage schema v6 already uses persistent keys (addressable issue)
- Indexer ecosystem mature (off-chain data sources trusted)
- Merkle tree libraries available (soroban-merkle ecosystem emerging)
- v2.1 roadmap allows breaking storage changes

**Use Cases:**

1. **Large invoice pool:** 50K+ investors contributing to a single invoice escrow
2. **Institutional investor:** Single address with 1000+ transactions (need efficient claiming)
3. **Portfolio analytics:** Query "what's my return across 100 invoices?" without O(n) contract calls
4. **Compliance audit:** Replay investor history from merkle proofs + off-chain ledger

**Success Metric:**
- Escrows with 10,000+ investors deployable without hitting storage costs
- TTL extension cost < 10% of payout cost (vs. 30% today)
- Query time for "investor contribution" < 100ms (using merkle proof verification)

---

## Design

### Overview

**High-level approach:**

For escrows with `investor_count > 1000` (configurable), store investor data off-chain:

1. **Off-chain ledger** (e.g., S3, IPFS, centralized DB)
   - Immutable record: `{ investor_address, contribution, yield_bps, claim_timestamp, claimed }`
   - Organized in temporal order (sorted by contribution timestamp)

2. **Merkle tree** (off-chain computation)
   - Compute SHA-256 Merkle tree over investor records
   - Root hash = cryptographic commitment to all records
   - Proof = O(log n) path from leaf to root

3. **On-chain root** (Soroban storage)
   - Store only the Merkle root (`DataKey::InvestorDataMerkleRoot`)
   - Verify claims via `verify_investor_claim(proof, investor, contribution, claim_ts)`
   - Proof size: ~32 bytes * log₂(n) ≈ 320 bytes for 1M investors

4. **Fallback to on-chain** (for small escrows)
   - Escrows with < 1000 investors keep current design (persistent keys)
   - No migration burden
   - Gradual adoption

### Detailed Design

**Component 1: Data Model — Off-Chain Storage Schema**

**Investor Record (off-chain):**

```json
{
  "investor": "GXXXXXX...",
  "contribution": 1000000,
  "contribution_timestamp": 1722090119,
  "effective_yield_bps": 500,
  "claim_lock_timestamp": 1725000000,
  "claimed": false,
  "claim_timestamp": null
}
```

**Off-Chain Ledger Structure:**

```
ledger.jsonl (line-delimited JSON, immutable append-only):
{ "investor": "GXXX", "contribution": 1000000, ... }
{ "investor": "GYYY", "contribution": 500000, ... }
{ "investor": "GZZZ", "contribution": 1500000, ... }
...
```

Stored in:
- **IPFS** (decentralized, content-addressed, immutable)
- **S3 + Merkle commitment** (centralized, queryable, audit trail)
- **Arweave** (permanent, timestamped, permaweb)

**Merkle Tree (off-chain computation):**

```
                   Root: H(...)
                   /          \
             H1: H(...)    H2: H(...)
             /      \      /        \
         L1 L2    L3 L4  L5 L6    L7 L8
        [R1][R2] [R3][R4][R5][R6][R7][R8]
```

- Leaves: `keccak256(abi.encode(investor_record))` for each record
- Nodes: `sha256(left || right)`
- Root: Commitment to all records

**Merkle Proof (for verification):**

For leaf L3 (investor 3):
```
Proof = [L4, H1, H2]  // 3 sibling hashes
Verify: 
  1. leaf_hash = keccak256(investor_record)
  2. h3_h4 = sha256(L3 || L4)
  3. h1 = sha256(h3_h4 || H1)
  4. root = sha256(h1 || H2)
  5. assert(root == stored_root)
```

**Component 2: Storage Schema (v6 → v7)**

New `DataKey` variants:

```rust
pub enum DataKey {
    // ... existing keys ...
    
    /// Mode for large escrows (per-investor data storage strategy).
    /// DRAFT = on-chain persistent keys (current, default)
    /// MERKLE = off-chain + on-chain merkle root verification (new)
    InvestorStorageMode,  // Enum: DRAFT | MERKLE
    
    /// Merkle root of investor data tree (only when StorageMode == MERKLE).
    /// Immutable once set. Size: 32 bytes (SHA-256).
    /// Absent ⇒ not using merkle verification (storage mode is DRAFT).
    InvestorDataMerkleRoot,
    
    /// URL/hash pointing to off-chain investor ledger (IPFS, S3, Arweave).
    /// Size: ~100 bytes. Immutable after init.
    /// Absent ⇒ investor data is on-chain (storage mode is DRAFT).
    InvestorDataSource,
    
    /// Timestamp when merkle root was last updated (for audit trail).
    /// Absent ⇒ never updated.
    MerkleRootUpdatedAt,
}
```

**Migration Strategy:**

- **On-chain (DRAFT mode):** Existing behavior, no change
- **Off-chain (MERKLE mode):** New escrows with `use_merkle_storage: true` at init
- **No mandatory migration:** v6 escrows continue using on-chain mode indefinitely

**Component 3: Entrypoint Changes**

**`init()` signature update:**

```rust
pub fn init(
    env: Env,
    invoice_id: String,
    ...,
    max_unique_investors: Option<u32>,
    use_merkle_storage: Option<bool>,  // NEW: default false for backward compat
    investor_data_source: Option<String>,  // NEW: URL/IPFS hash if merkle mode
    investor_data_merkle_root: Option<BytesN<32>>,  // NEW: initial root if merkle mode
) -> Result<EscrowSummary, EscrowError>
```

**`claim_investor_payout()` update (merkle verification):**

```rust
pub fn claim_investor_payout(
    env: Env,
    investor_contribution: i128,      // Claimed amount (from off-chain proof)
    merkle_proof: Option<Vec<BytesN<32>>>,  // NEW: proof if storage mode == MERKLE
) -> Result<InvestorPayoutClaimed, EscrowError>
```

**New entrypoint: `verify_investor_claim_merkle()`:**

```rust
pub fn verify_investor_claim_merkle(
    env: Env,
    investor: Address,
    contribution: i128,
    effective_yield_bps: i64,
    claim_lock_timestamp: u64,
    merkle_proof: Vec<BytesN<32>>,
) -> Result<bool, EscrowError>
```

**Validation logic:**

```rust
fn verify_merkle_claim(
    env: &Env,
    investor: Address,
    contribution: i128,
    effective_yield_bps: i64,
    claim_lock_timestamp: u64,
    merkle_proof: Vec<BytesN<32>>,
) -> Result<bool, EscrowError> {
    // 1. Get stored merkle root
    let merkle_root = env.storage().instance()
        .get(&DataKey::InvestorDataMerkleRoot)?;
    
    // 2. Construct investor record hash
    let record = InvestorRecord {
        investor: investor.clone(),
        contribution,
        effective_yield_bps,
        claim_lock_timestamp,
    };
    let leaf_hash = keccak256(abi.encode(&record));
    
    // 3. Verify merkle path
    let computed_root = merkle_verify(leaf_hash, merkle_proof)?;
    
    // 4. Compare roots
    if computed_root == merkle_root {
        Ok(true)
    } else {
        Err(EscrowError::InvalidMerkleProof)  // error code 44
    }
}
```

**Component 4: Off-Chain Integration Points**

**At escrow init:**

```rust
pub fn init(..., use_merkle_storage: bool, investor_data_source: String, merkle_root: BytesN<32>) {
    // Validate merkle root is not empty (if merkle mode enabled)
    if use_merkle_storage {
        assert!(!merkle_root.is_zero(), "merkle root cannot be zero");
        env.storage().instance().set(&DataKey::InvestorStorageMode, &"MERKLE");
        env.storage().instance().set(&DataKey::InvestorDataMerkleRoot, &merkle_root);
        env.storage().instance().set(&DataKey::InvestorDataSource, &investor_data_source);
    } else {
        env.storage().instance().set(&DataKey::InvestorStorageMode, &"DRAFT");
    }
}
```

**At claim time:**

```rust
pub fn claim_investor_payout(env: Env, investor_contribution: i128, merkle_proof: Option<Vec<...>>) {
    let mode = env.storage().instance().get(&DataKey::InvestorStorageMode).unwrap_or("DRAFT");
    
    match mode {
        "DRAFT" => {
            // Current on-chain verification
            let stored_contribution = env.storage().persistent()
                .get(&DataKey::InvestorContribution(caller.clone()))
                .unwrap_or(0);
            assert!(investor_contribution == stored_contribution);
        }
        "MERKLE" => {
            // Merkle proof verification
            let proof = merkle_proof.ok_or(EscrowError::MissingMerkleProof)?;
            verify_merkle_claim(env, caller.clone(), investor_contribution, proof)?;
        }
    }
}
```

### Examples

**Example 1: Large escrow with 50K investors (MERKLE mode)**

```rust
// Init with merkle root (root computed off-chain)
init(
    invoice_id: "INV-MEGA-001",
    ...,
    max_unique_investors: Some(50_000),
    use_merkle_storage: true,
    investor_data_source: "ipfs://QmABC123...",  // IPFS CID
    investor_data_merkle_root: merkle_root,      // 32 bytes
)

// On-chain storage needed:
// - Merkle root: 32 bytes
// - Source URL: ~100 bytes
// Total: ~300 bytes vs. 50K * 50 bytes = ~2.5 MB with on-chain mode
```

**Example 2: Small escrow with 100 investors (DRAFT mode, backward compat)**

```rust
// Old behavior, no merkle proof needed
init(
    ...,
    use_merkle_storage: false,  // or omitted (default)
)

// Per-investor data stored on-chain as before
// No change to claiming logic
```

**Example 3: Investor claims with merkle proof**

```
// Off-chain: Integrate submits proof with claim request
POST /claim
{
  "investor": "GXXX...",
  "contribution": 1000000,
  "effective_yield_bps": 500,
  "claim_lock_timestamp": 1725000000,
  "merkle_proof": [
    "0x1234...",  // sibling 1 (32 bytes)
    "0x5678...",  // sibling 2 (32 bytes)
    "0xabcd...",  // sibling 3 (32 bytes)
    // log2(50000) ≈ 16 siblings for 50K investors
  ]
}

// On-chain: Contract verifies proof against stored root
claim_investor_payout(
    env,
    1000000,
    Some(merkle_proof),
)
// → Computes leaf hash from (investor, contribution, yield, lock_ts)
// → Verifies proof against stored merkle root
// → Transfer payout if proof valid
```

### Data Model Diagram

```
Single Escrow Instance:

DRAFT Mode (current):
├─ InvoiceEscrow (on-chain)
└─ InvestorContribution(addr) × 1000 (persistent keys, on-chain)
   └─ TTL rent cost: O(n)

MERKLE Mode (new, for n > 1000):
├─ InvoiceEscrow (on-chain)
├─ InvestorDataMerkleRoot (on-chain, 32 bytes)
├─ InvestorDataSource (on-chain, 100 bytes)
└─ Off-Chain Ledger (IPFS/S3/Arweave)
    └─ 50K investor records
    └─ No per-record TTL rent
```

---

## Alternatives Considered

### Alternative 1: Increase On-Chain Storage (Rejected)

**Approach:** Just live with large on-chain footprint; optimize storage reads.

**Pros:**
- Simplest (no new entrypoints)
- Custody-verifiable (all data on-chain)

**Cons:**
- Storage rent scales with investor count (O(n) cost)
- TTL extension becomes operationally complex (list 50K addresses)
- Hits Soroban limits around 10K investors
- Storage reads are still slow (must iterate through all keys)

**Decision:** Rejected. Doesn't solve scalability problem.

---

### Alternative 2: Centralized Database + Oracle (Rejected)

**Approach:** Store all data in centralized DB (e.g., Postgres); use oracle to attest balances at settlement time.

**Pros:**
- Query performance is instant (SQL indexes)
- Flexible schema (can add fields later)

**Cons:**
- Oracle dependency (centralized trust, latency, cost)
- No immutable audit trail (data can be rewritten)
- Breaks principle of "contract is source of truth"
- Single point of failure

**Decision:** Rejected. Merkle tree preserves immutability + decentralization.

---

### Alternative 3: Smart Contract Rollups (Deferred)

**Approach:** Use a rollup/scaling layer (e.g., Stellar Anchor Platform) to handle investor accounting.

**Pros:**
- Scaling solution for high throughput
- Natural for multi-escrow portfolio operations

**Cons:**
- Requires new ecosystem (anchor platform not yet mature)
- Adds deployment complexity
- Merkle proof solution works immediately

**Decision:** Deferred to v3. Use merkle proofs in v2.1 as interim scaling measure.

---

### Alternative 4: Per-Escrow Contract Instances (Rejected)

**Approach:** Deploy separate escrow instance for every 100 investors.

**Pros:**
- No storage bloat on single contract

**Cons:**
- 500× more contract deployments for 50K investors
- Discovery nightmare (which contract holds which investor?)
- Settlement coordination nightmare (settle 500 contracts?)
- Massive operational overhead

**Decision:** Rejected. Merkle proof is superior.

---

## Implementation

### Effort Estimate

| Component | Estimate | Notes |
|-----------|----------|-------|
| Storage schema + DataKey variants | 1–2 days | 3 new keys, instance storage only |
| Merkle verification logic (on-chain) | 2–3 days | keccak256 + tree verification |
| Claim entrypoint update | 1–2 days | Conditional logic (DRAFT vs MERKLE) |
| Init with merkle parameters | 1 day | Validation + root storage |
| Unit tests (merkle verification) | 2–3 days | Test vectors, boundary cases |
| Integration tests (full flow) | 2–3 days | Off-chain proof generation + verification |
| Off-chain proof generation (Python/JS) | 3–5 days | Merkle tree builder, IPFS integration |
| Documentation + audit prep | 2–3 days | Security model, integration guide |
| **Total** | **14–22 days** | ~3–4 week sprint |

### Milestones

**Week 1:** Storage schema + merkle verification logic
- [ ] New DataKey variants defined
- [ ] Merkle proof verification function (on-chain)
- [ ] Unit tests for tree verification

**Week 2:** Entrypoint integration + init changes
- [ ] `init()` accepts merkle root + source URL
- [ ] `claim_investor_payout()` routes to merkle verification
- [ ] Storage mode conditional logic working
- [ ] Error code 44 (InvalidMerkleProof) working

**Week 3:** Off-chain tooling + integration tests
- [ ] Off-chain proof generator (Python/JS)
- [ ] IPFS integration (upload ledger, get CID)
- [ ] End-to-end: generate ledger → compute merkle tree → init escrow → claim → verify
- [ ] Backward compatibility tests (DRAFT mode escrows unaffected)

**Week 4:** Audit + documentation
- [ ] Security analysis (merkle proof soundness, hash function choices)
- [ ] Escrow-merkle-verification.md (integration guide for integrators)
- [ ] ADR-011 draft (if warranted)

### Blockers

- [ ] None identified as of 2026-07-27
- Assumes `keccak256` available in Soroban (standard hash function, should be available)
- Assumes off-chain storage (IPFS, S3, Arweave) operator-maintained (not contract-maintained)

### Implementation Notes

- Use `BytesN<32>` for merkle hashes (fixed size, no serialization overhead)
- Merkle tree structure: **balanced binary tree** (simplest verification)
- Hash function: `keccak256` (Ethereum-compatible, battle-tested)
- Proof format: `Vec<BytesN<32>>` (array of sibling hashes)
- Proof size: ~32 bytes × log₂(n) ≈ 320 bytes for 1M investors
- Off-chain storage: IPFS recommended (immutable, content-addressed, decentralized)

---

## Acceptance Criteria

- [ ] `InvestorStorageMode` switch works (DRAFT vs MERKLE)
- [ ] Merkle root stored immutably in instance storage
- [ ] Off-chain data source URL stored (IPFS CID, S3 URL, etc.)
- [ ] Merkle proof verification function is cryptographically sound
- [ ] `claim_investor_payout()` routes to merkle path when mode == MERKLE
- [ ] Backward compatibility: DRAFT mode escrows unchanged
- [ ] Proof verification returns error code 44 on invalid proof
- [ ] Proof size is O(log n) ≈ 320 bytes for 1M investors
- [ ] On-chain storage reduced to ~300 bytes (root + source) vs. ~5 MB for 100K investors
- [ ] Unit tests for merkle verification (boundary cases, invalid proofs)
- [ ] Integration tests (ledger generation → proof generation → verification)
- [ ] 95%+ code coverage maintained
- [ ] Off-chain tooling published (open-source merkle tree builder)
- [ ] Security analysis: no merkle tree attacks / proof collision risks
- [ ] Documentation: integration guide for integrators
- [ ] Zero security findings from audit

---

## Rollout Plan

### Phase 1: Testnet (Week 1–2)

- Deploy to testnet
- Test with synthetic 10K+ investor dataset
- Verify proof generation is correct (off-chain tool)
- Verify proof verification is correct (on-chain contract)
- Collect team feedback

**Success criteria:** All tests pass; merkle proofs verify correctly for 99%+ of cases.

### Phase 2: Early Partner Testing (Week 3)

- Release `v2.1-beta` to 1–2 early partners
- Partners test merkle integration on testnet
- Collect feedback on:
  - Proof generation latency (target < 100ms for 100K proofs)
  - IPFS upload experience
  - Claim experience (proof submission flow)
- Refine based on feedback

**Success criteria:** Partners successfully integrate; claim UX is smooth.

### Phase 3: Mainnet (Week 4)

- Audit completion + fixes applied
- Release v2.1 with merkle verification
- Publish off-chain tooling (open-source)
- New escrows can opt into merkle mode
- Support 10K+ investor escrows

**Success criteria:** First 10K+ investor escrow deploys on mainnet; claims work.

### Monitoring

**Key metrics:**
- % of new escrows using merkle mode
- Merkle proof verification success rate (target 99.9%)
- Claim latency (on-chain + off-chain proof fetch)
- IPFS availability (% of ledgers retrievable)

**Dashboards:**
- Grafana: merkle verification latency (real-time)
- BigQuery: escrow size distribution (investor count by mode)

**Alerts:**
- Merkle verification failure rate > 1% → investigate
- IPFS ledger retrieval failure > 5% → fallback to backup

---

## References

- **Merkle Tree Basics:** https://en.wikipedia.org/wiki/Merkle_tree
- **Merkle Proof Verification:** https://ethereum.org/en/developers/docs/data-structures-and-encoding/merkle-proofs/
- **Stellar Soroban Storage:** https://developers.stellar.org/docs/learn/storing-data
- **ADR-006 (Token Safety):** docs/adr/ADR-006-dust-sweep-and-token-safety.md
- **ADR-007 (Storage Evolution):** docs/adr/ADR-007-storage-key-evolution.md
- **Related Issue:** GitHub issue #1234 (Off-chain storage for large escrows)

---

## Security Considerations

### Merkle Tree Soundness

**Threat:** Attacker computes fraudulent merkle proof for non-existent investor.

**Mitigation:**
- Verify proof against stored root (cryptographic binding)
- Use collision-resistant hash (keccak256, 2^128 security)
- Proof verification is deterministic (no randomness to exploit)

**Assurance:** Proof is valid iff investor record exists in original off-chain ledger.

### Off-Chain Data Availability

**Threat:** IPFS goes down; investor can't claim (DOS).

**Mitigation:**
- Store on multiple providers (IPFS + S3 redundancy)
- Escrow admin responsible for maintaining data availability
- Fallback: on-chain claim without proof (if data truly lost)

**Assurance:** Investor can always claim on-chain by providing data (contract accepts any valid proof).

### Storage Migration

**Threat:** Upgrading from DRAFT → MERKLE mode loses on-chain contributor data.

**Mitigation:**
- No automatic migration; per-escrow opt-in
- DRAFT mode escrows can co-exist with MERKLE escrows indefinitely
- Migration is manual + documented (off-chain tool converts DRAFT → ledger → merkle tree)

**Assurance:** Backward compatibility preserved; no forced migration.

---

## Decision

**Owner:** Platform Architect  
**Status:** DRAFT (awaiting team feedback)  
**Decision date:** TBD (target: 2026-08-10)

---

## Revision History

| Date | Status | Change |
|------|--------|--------|
| 2026-07-27 | DRAFT | Initial proposal |
| — | DISCUSSION | Awaiting team feedback (3+ reviewers) |
| — | ACCEPTED | Decision made by platform architect |
| — | IMPLEMENTED | Feature shipped in v2.1 |

