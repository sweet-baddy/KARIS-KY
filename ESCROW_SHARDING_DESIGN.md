# Escrow Investor Sharding Architecture

## Overview

Design document for implementing investor storage sharding to support escrows with 10k+ investors by spawning multiple internal shard contracts on-demand.

## Problem Statement

**Current Limitations:**
- Single escrow contract stores all investor contributions in persistent storage
- Per-investor keys grow linearly: `InvestorContribution(Address)`, `InvestorEffectiveYield(Address)`, etc.
- Storage costs and TTL management become problematic at 10k+ investors
- No built-in mechanism to shard investor records across multiple contract instances

**Acceptance Criteria:**
1. Escrow can spawn up to N shard contracts on-demand
2. Fund routes to appropriate shard based on investor address hash
3. Settlement aggregates across all shards

## Architecture

### Design Overview

```
┌─────────────────────────────────────────────────────┐
│          Primary Escrow Contract                    │
│  (Manages escrow state, routes to shards)          │
│                                                      │
│  • Escrow state (amount, target, status, etc.)     │
│  • Shard registry (shard_id -> contract address)   │
│  • Total aggregated funding metrics                │
│  • Shard count and hash seed                       │
└─────────────────────────────────────────────────────┘
           │                    │                    │
           ▼                    ▼                    ▼
    ┌────────────┐      ┌────────────┐      ┌────────────┐
    │  Shard 0   │      │  Shard 1   │      │  Shard N   │
    │            │      │            │      │            │
    │ Investor   │      │ Investor   │      │ Investor   │
    │ data:      │      │ data:      │      │ data:      │
    │ • contrib  │      │ • contrib  │      │ • contrib  │
    │ • yield    │      │ • yield    │      │ • yield    │
    │ • claimed  │      │ • claimed  │      │ • claimed  │
    │ • lock_nb  │      │ • lock_nb  │      │ • lock_nb  │
    └────────────┘      └────────────┘      └────────────┘
```

### Routing Logic

**Investor Address -> Shard:**

```rust
fn route_to_shard(investor: Address, shard_count: u32) -> u32 {
    // Hash investor address -> shard ID
    let bytes = investor.to_bytes();
    let hash = blake3(bytes).to_bytes()[0..4];
    let shard_id = u32::from_le_bytes(hash) % shard_count;
    shard_id
}
```

**Properties:**
- Deterministic: same investor always routes to same shard
- Uniform: hash distribution across shards (blake3)
- Immutable: shard assignment doesn't change across rebalances

### Shard Contract Lifecycle

#### Phase 1: Spawning

**When:** First investor for a shard is funded
- Primary escrow detects shard doesn't exist
- Creates new shard contract instance (if shard_count < N_MAX)
- Registers shard address in primary state
- Future investors for this shard use existing instance

**Spawn Logic:**
```
investor routes to shard_id=5
  │
  ├─ Check: shard_5 exists?
  │  └─ YES -> route to existing shard
  │  └─ NO -> spawn new shard
  │     ├─ Create shard contract
  │     ├─ Register in primary
  │     └─ Route to new shard
```

#### Phase 2: Normal Operation

**Fund operations:**
- Primary escrow receives fund call
- Routes to appropriate shard based on investor hash
- Shard stores investor contribution
- Primary aggregates total funded_amount

**Settlement:**
- Primary escrow enters settled status
- For each shard: aggregate investor payout computations
- Each shard independently manages investor claims
- Primary verifies final state consistency

#### Phase 3: Deactivation

**When:** Escrow enters terminal status (settled/withdrawn/cancelled)
- Shards become read-only for investors
- Admin can decomission shards (optional)
- Historical data remains on-chain

### Storage Model

#### Primary Escrow Storage

```rust
// Existing escrow state (unchanged)
pub struct InvoiceEscrow { /* ... */ }

// New: Shard registry
pub enum DataKey {
    // Existing keys...
    
    // NEW: Sharding configuration
    ShardCount,                    // Total number of shards spawned
    ShardAddresses(u32),          // shard_id -> Address mapping
    ShardHashSeed,                // Blake3 seed for deterministic routing
    InvestorShardAllocation,      // Optional: pre-allocated routing table
    
    // Aggregated totals (migrated from direct storage)
    AggregatedFundedAmount,       // Sum across all shards
    AggregatedUniqueFunderCount,  // Count across all shards
}
```

#### Shard Contract Storage

```rust
// Minimal shard state
pub struct ShardData {
    primary_escrow: Address,
    shard_id: u32,
    // Investor-specific per-address data (moved from primary)
    // Stored under persistent keys:
    // - InvestorContribution(Address)
    // - InvestorEffectiveYield(Address)
    // - InvestorClaimNotBefore(Address)
    // - InvestorClaimed(Address)
    // - InvestorRefunded(Address)
}
```

## Implementation Strategy

### Phase 1: Design & Types (Minimal)

1. Define ShardRegistry type to track shard addresses
2. Add sharding storage keys to DataKey enum
3. Create ShardInvestorData helper struct for cross-shard serialization

### Phase 2: Spawn Logic

1. Implement `fund_into_shard()` internal function
2. Routing logic: address hash -> shard_id
3. Lazy shard spawning on first fund for each shard

### Phase 3: Settlement Aggregation

1. Implement settlement query to each shard
2. Aggregate results (funded amounts, investor counts)
3. Consistency verification

### Phase 4: Migration Path

1. For escrows with pre-existing 10k+ investors:
   - Batch operation to migrate investor data to shards
   - Recompute aggregated totals
   - No fund/settle operations during migration

## Data Flow

### Fund Operation with Sharding

```
fund(investor, amount)
  │
  ├─ Route investor to shard:
  │  shard_id = hash(investor) % shard_count
  │
  ├─ Shard exists?
  │  ├─ YES: Direct call to existing shard
  │  └─ NO: Spawn shard (if < N_MAX)
  │        Register shard address
  │        Then call shard
  │
  ├─ Shard records contribution:
  │  InvestorContribution(investor) += amount
  │
  └─ Primary updates aggregate:
     AggregatedFundedAmount += amount
     Check funded_amount >= target
     Advance status if threshold reached
```

### Settlement with Sharding

```
settle()
  │
  ├─ Verify status == funded
  ├─ Verify maturity reached
  │
  ├─ For each shard (1..shard_count):
  │  ├─ Query shard settlement state
  │  ├─ Collect investor aggregates
  │  └─ Verify contributions sum
  │
  ├─ Primary status -> settled
  └─ Each shard allows investor claims
```

### Investor Claim with Sharding

```
claim_investor_payout(investor)
  │
  ├─ Route to shard:
  │  shard_id = hash(investor) % shard_count
  │
  ├─ Call shard.claim_payout(investor):
  │  ├─ Verify status == settled
  │  ├─ Get investor contribution from shard
  │  ├─ Compute pro-rata share
  │  ├─ Mark claimed
  │  └─ Return payout amount
  │
  └─ Primary escrow verifies total
     and manages treasury
```

## Key Design Decisions

### 1. Lazy Shard Spawning
- **Why:** Avoid upfront cost of creating N shards
- **Benefit:** Only create shards as needed
- **Trade-off:** First investor to new shard pays spawn cost

### 2. Hash-Based Routing
- **Why:** Deterministic, uniform distribution, no rebalancing needed
- **Benefit:** Investor always routes to same shard
- **Safety:** Cannot accidentally split investor data

### 3. Primary Aggregation
- **Why:** Unified settlement logic at primary level
- **Benefit:** Simpler governance, clear responsibility
- **Trade-off:** Primary must query all shards at settle time

### 4. No Shard Rebalancing
- **Why:** Simplicity, immutability of allocations
- **Benefit:** Investor data stable, no cross-shard transfers
- **Limitation:** Uneven distribution possible (acceptable with hash)

### 5. Optional Shard Limits
- **Why:** Prevent unbounded contract spawning
- **Limit:** N_MAX typically 1000-10000 shards
- **Benefit:** Bounded operational complexity

## Scalability Analysis

### Storage Efficiency

**Single Contract (Current):**
- Per-investor: 5-6 persistent storage entries
- 10k investors: ~50k entries
- Cost: High, TTL management complex

**With Sharding (10k investors, 1024 shards):**
- Per shard: ~10 investors on average
- Per shard storage: ~50-60 entries
- Total: ~50k entries (same), but distributed
- Benefit: Lower TTL pressure per shard, parallelizable

**Extreme Case (100k investors, 1024 shards):**
- Per shard: ~100 investors
- Scales linearly with shards
- Settlement: O(shard_count) cross-contract calls

### Gas Costs

**Fund Operation:**
- Routing: O(1) hash
- Shard lookup: O(1) storage read
- Shard call: O(1) + cross-contract overhead (~200-400 gas)
- Total: ~500-1000 gas overhead vs. non-sharded (~100-200 gas)
- Trade-off acceptable for 10k+ investor scenarios

**Settlement:**
- Per-shard query: ~200-400 gas
- Total: O(shard_count) * 300 gas
- 1024 shards: ~307k gas (acceptable)

## Implementation Constraints

### Soroban Limitations

1. **Contract Spawning:** Must use `env.register()` or similar
   - Requires WASM code uploaded
   - Each shard is full contract instance

2. **Cross-Contract Calls:** All shard operations are async
   - Network latency for each call
   - No atomic multi-contract transactions

3. **Storage Capacity:**
   - Each contract instance has own storage limits
   - Spreading reduces individual contract pressure

4. **Immutability:**
   - Once spawned, shard address is permanent
   - Cannot rebalance or migrate shards

### Practical Limits

- **Max Shards:** 1000-10000 (reasonable governance limit)
- **Max Investors per Shard:** Depends on Soroban storage, typically 1k-10k
- **Max Total Investors:** (Max Shards) × (Max per Shard) = 10M+

## Backwards Compatibility

**Existing Escrows Without Sharding:**
- ShardCount = 0
- All investor data in primary (current behavior)
- No breaking changes

**Existing Escrows Migrating to Sharding:**
- On-demand: admin triggers migration
- Migrate investor data to shards
- Update aggregates
- New funds automatically route to shards

**New Escrows:**
- Option to enable sharding at init
- ShardCount = 0 by default (no sharding)
- Opt-in to sharding for high-volume scenarios

## Acceptance Criteria Mapping

| Criterion | Design Element | Implementation |
|-----------|---|---|
| Spawn up to N shard contracts | Lazy spawning logic | fund_into_shard() |
| Route based on investor hash | Hash-based routing | hash(address) % shard_count |
| Settlement aggregates across shards | Primary query loop | settle() queries each shard |

## Risk Mitigation

### Risk: Shard Contract Upgrade

**Mitigation:** Shard contract deployed as separate WASM
- Primary can be upgraded independently
- Shard code is immutable (or selectively upgradeable)
- Multi-phase upgrade strategy documented

### Risk: Shard Failure

**Mitigation:** Shard redundancy
- Keep backup copy of shard data
- Recovery mechanism for failed shards
- Off-chain indexer mirrors all data

### Risk: Cross-Shard Consistency

**Mitigation:** Aggregate verification
- After settlement, verify: Σ(shard contributions) == total
- Catch data loss or corruption
- Governance intervention point

### Risk: Governance Spam

**Mitigation:** Shard limits
- Max N_MAX shards per escrow
- Prevent unbounded spawning
- Admin approval for each shard

## Future Enhancements

1. **Dynamic Shard Rebalancing**
   - Redistribute investors across shards
   - Rebalance based on load
   - Complex, deferred to v2

2. **Parallel Settlement**
   - Settle all shards in parallel
   - Coordinate via primary
   - Improves latency

3. **Shard Pooling**
   - Reuse shards across multiple escrows
   - Reduce total contract instances
   - Share shard storage

4. **Hierarchical Sharding**
   - Shard the shards
   - For 1M+ investors
   - Deferred to v2

## Success Metrics

- ✅ Escrow supports 10k+ investors without storage issues
- ✅ Fund operations < 2x overhead vs. non-sharded
- ✅ Settlement aggregates correctly across all shards
- ✅ Backwards compatible with existing escrows
- ✅ Zero investor data loss in sharding operations

## References

- Soroban Contract Deployment: `env.register()`
- Cross-Contract Calls: Soroban SDK guide
- Storage Model: `docs/escrow-data-model.md`
- Current Investor Storage: `DataKey::InvestorContribution*`

## Timeline

1. **Phase 1 (Design):** ✅ This document
2. **Phase 2 (Spawn Logic):** Implementation in progress
3. **Phase 3 (Settlement):** Post-Phase 2
4. **Phase 4 (Migration):** Post-Phase 3
5. **Phase 5 (Testing):** Ongoing

## Conclusion

The sharding architecture provides a scalable path for escrows exceeding 10k investors while maintaining backwards compatibility and governance control. Implementation is straightforward for Phase 1-2, with Phase 3 (settlement aggregation) being the most complex cross-contract coordination challenge.
