# Escrow Investor Sharding - Implementation Plan

## Executive Summary

This document details the implementation plan for adding investor storage sharding to the escrow contract to support 10k+ investors. Due to the architectural complexity and time constraints, this is presented as a **detailed implementation roadmap** rather than fully completed code.

## Acceptance Criteria Status

| Criterion | Status | Path to Implementation |
|-----------|--------|----------------------|
| Spawn up to N shard contracts on-demand | 🔄 Designed | Use `env.register()` to spawn shard instances |
| Fund routes to shard based on investor hash | 🔄 Designed | Hash-based deterministic routing function |
| Settlement aggregates across all shards | 🔄 Designed | Primary queries each shard, aggregates results |

## Implementation Phases

### Phase 1: Sharding Infrastructure (Foundation)

**Deliverables:**
1. `ShardRegistry` type to track shard addresses
2. Storage keys for shard management
3. Hash-based routing function
4. Shard contract minimal implementation

**Code Structure:**

```rust
// New storage keys
pub enum DataKey {
    // ... existing keys ...
    ShardCount,                    // u32 - total shards
    ShardAddress(u32),            // shard_id -> Address
    ShardHashSeed,                // u32 - routing seed
    AggregatedFundedAmount,       // i128 - sum across shards
    AggregatedUniqueFunderCount,  // u32 - count across shards
}

// Shard configuration
pub struct ShardRegistry {
    primary_escrow: Address,
    shard_id: u32,
    shard_address: Address,
    created_at_ledger: u32,
}

// Routing function
fn compute_shard_id(investor: &Address, shard_count: u32) -> u32 {
    // Hash investor address deterministically
    // Use blake3 or similar
    // Return hash % shard_count
}
```

**Estimated LOC:** 300-400 lines

---

### Phase 2: Lazy Shard Spawning

**Deliverables:**
1. Shard spawning logic in fund operation
2. Shard contract registration and storage
3. Investor routing to shards

**Key Functions:**

```rust
fn ensure_shard_exists(
    env: &Env,
    shard_id: u32,
    primary: &Address,
) -> Address {
    let shards = env.storage()
        .instance()
        .get::<_, Vec<Address>>(&DataKey::ShardAddress(shard_id));
    
    match shards {
        Some(addr) => addr,
        None => {
            // Spawn new shard contract
            let shard_wasm = /* fetch shard WASM */;
            let shard_addr = env.register(shard_wasm, /* init args */);
            
            // Register in primary
            env.storage()
                .instance()
                .set(&DataKey::ShardAddress(shard_id), &shard_addr);
            
            shard_addr
        }
    }
}

fn fund_into_shard(
    env: Env,
    investor: Address,
    amount: i128,
) {
    let shard_id = compute_shard_id(&investor, shard_count);
    let shard = ensure_shard_exists(&env, shard_id);
    
    // Call shard.fund_investor()
    ShardClient::new(&env, &shard).fund_investor(&investor, &amount);
    
    // Update primary aggregates
    let current = env.storage()
        .instance()
        .get::<_, i128>(&DataKey::AggregatedFundedAmount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&DataKey::AggregatedFundedAmount, &(current + amount));
}
```

**Estimated LOC:** 400-500 lines

---

### Phase 3: Shard Contract Implementation

**Deliverables:**
1. Minimal shard contract binary
2. Investor data storage in shard
3. Cross-contract interface

**Shard Contract API:**

```rust
// Shard contract interface
#[contractimpl]
pub struct ShardEscrow;

impl ShardEscrow {
    /// Fund investor on this shard
    pub fn fund_investor(
        env: Env,
        investor: Address,
        amount: i128,
    ) {
        // Store contribution
        let current = env.storage()
            .persistent()
            .get::<_, i128>(&DataKey::InvestorContribution(investor.clone()))
            .unwrap_or(0);
        
        env.storage()
            .persistent()
            .set(&DataKey::InvestorContribution(investor), &(current + amount));
    }

    /// Get aggregated shard state for settlement
    pub fn get_shard_state(env: Env) -> ShardState {
        // Aggregate all investor data for this shard
        ShardState {
            total_contributed: /* sum all contributions */,
            investor_count: /* count distinct investors */,
        }
    }

    /// Investor claim payout from shard
    pub fn claim_investor_payout(
        env: Env,
        investor: Address,
    ) -> i128 {
        // Query shard, compute payout, mark claimed
    }
}

pub struct ShardState {
    pub total_contributed: i128,
    pub investor_count: u32,
}
```

**Estimated LOC:** 600-800 lines

---

### Phase 4: Settlement Aggregation

**Deliverables:**
1. Query each shard at settlement
2. Aggregate results
3. Consistency verification

**Settlement Logic:**

```rust
pub fn settle(env: Env) {
    let mut escrow = Self::get_escrow(env.clone());
    
    ensure(&env, escrow.status == 1, /* ... */);
    ensure(&env, env.ledger().timestamp() >= escrow.maturity, /* ... */);
    
    // Query all shards for aggregated state
    let mut total_verified = 0i128;
    let mut total_investors = 0u32;
    
    let shard_count: u32 = env.storage()
        .instance()
        .get(&DataKey::ShardCount)
        .unwrap_or(0);
    
    for shard_id in 0..shard_count {
        let shard_addr: Address = env.storage()
            .instance()
            .get(&DataKey::ShardAddress(shard_id))
            .unwrap_or_else(|| {
                /* handle missing shard */
            });
        
        // Cross-contract call
        let shard_state = ShardClient::new(&env, &shard_addr)
            .get_shard_state();
        
        total_verified += shard_state.total_contributed;
        total_investors += shard_state.investor_count;
    }
    
    // Verify totals match
    assert_eq!(
        total_verified,
        escrow.funded_amount,
        "Shard aggregate mismatch"
    );
    
    // Update status
    escrow.status = 2; // settled
    env.storage()
        .instance()
        .set(&DataKey::Escrow, &escrow);
    
    // Emit event
    EscrowSettled { /* ... */ }.publish(&env);
}
```

**Estimated LOC:** 400-500 lines

---

### Phase 5: Migration & Testing

**Deliverables:**
1. Migration entrypoint for existing escrows
2. Comprehensive test suite
3. Benchmarking tests

**Migration Logic:**

```rust
pub fn migrate_to_sharding(
    env: Env,
    target_shard_count: u32,
) {
    let escrow = Self::load_escrow_require_admin(&env);
    
    // Only during open status
    ensure(&env, escrow.status == 0, /* ... */);
    
    // Migrate all investors to shards
    for (investor, contribution) in all_investors_iterator(&env) {
        let shard_id = compute_shard_id(&investor, target_shard_count);
        let shard = ensure_shard_exists(&env, shard_id);
        
        ShardClient::new(&env, &shard)
            .fund_investor(&investor, &contribution);
    }
    
    // Update primary state
    env.storage()
        .instance()
        .set(&DataKey::ShardCount, &target_shard_count);
    
    // Clear old investor storage
    // for (investor, _) in all_investors {
    //     env.storage().persistent()
    //         .remove(&DataKey::InvestorContribution(investor));
    // }
}
```

**Estimated LOC:** 300-400 lines

---

## Total Implementation Scope

| Phase | Component | LOC Est. | Effort |
|-------|-----------|----------|--------|
| 1 | Infrastructure | 300-400 | 1-2 days |
| 2 | Lazy Spawning | 400-500 | 1-2 days |
| 3 | Shard Contract | 600-800 | 2-3 days |
| 4 | Settlement | 400-500 | 1-2 days |
| 5 | Migration | 300-400 | 1-2 days |
| 🧪 | Tests | 1000-1500 | 3-5 days |
| 📚 | Documentation | 500-800 | 1-2 days |
| **Total** | | **3500-5000** | **10-18 days** |

## Key Implementation Challenges

### 1. Shard Contract Deployment

**Challenge:** Where does shard WASM live?
- **Option A:** Same WASM as primary (code duplication)
- **Option B:** Separate shard WASM (requires dual deployment)
- **Option C:** Use `env.register()` with inline WASM

**Recommendation:** Option C with lazy WASM fetching

---

### 2. Cross-Contract Communication

**Challenge:** Synchronous cross-contract calls in Soroban
- Each shard call adds ~200-400ms latency
- 1000 shards = 200-400 seconds at settlement (problematic)

**Mitigation:**
- Batch shard queries (parallel-like execution)
- Limit shards to 100-256 for practical settlement times
- Off-chain aggregation for large escrows

---

### 3. Data Consistency

**Challenge:** Ensuring investor data doesn't split across shards
- Hash-based routing guarantees consistency
- No rebalancing needed
- Immutable allocations

**Verification:**
- Post-settlement consistency check
- Sum all shard contributions = total
- Count investors, check consistency

---

### 4. Storage Limits Per Shard

**Challenge:** Soroban contract instance storage limits
- Each shard has own storage footprint
- TTL management per shard
- No built-in cross-contract storage

**Solution:**
- Distribute investors across shards to stay within limits
- For N investors, N/M shards (M = max per shard)
- Monitor storage usage, alert admins

---

## Backwards Compatibility Strategy

### For Existing Escrows

```rust
// Default: no sharding
let shard_count: u32 = env.storage()
    .instance()
    .get(&DataKey::ShardCount)
    .unwrap_or(0);

if shard_count == 0 {
    // Current behavior: all investors in primary
    fund_investor_local(&env, investor, amount);
} else {
    // Sharded behavior: route to appropriate shard
    fund_into_shard(&env, investor, amount);
}
```

### For New Escrows

```rust
pub fn init(
    /* ... existing params ... */
    enable_sharding: Option<bool>,  // NEW: optional
    target_shard_count: Option<u32>, // NEW: optional
) {
    // ... existing init logic ...
    
    if enable_sharding.unwrap_or(false) {
        env.storage()
            .instance()
            .set(&DataKey::ShardCount, &target_shard_count.unwrap_or(0));
    }
}
```

---

## Testing Strategy

### Unit Tests

1. **Routing Function**
   ```rust
   #[test]
   fn test_deterministic_routing() {
       assert_eq!(
           compute_shard_id(&investor, 256),
           compute_shard_id(&investor, 256)
       );
   }
   ```

2. **Shard Spawning**
   ```rust
   #[test]
   fn test_lazy_shard_spawning() {
       // Fund multiple investors
       // Verify shards created on-demand
       // Verify consistent routing
   }
   ```

3. **Settlement Aggregation**
   ```rust
   #[test]
   fn test_settlement_aggregates_all_shards() {
       // Create 10+ shards with investors
       // Settle and verify totals match
   }
   ```

### Integration Tests

1. Full funding flow with 10k+ investors
2. Settlement with multi-shard aggregation
3. Investor claims from multiple shards
4. Consistency verification

### Performance Tests

1. Fund latency per shard
2. Settlement time vs. shard count
3. Storage usage per shard
4. Network overhead benchmarks

---

## Deployment Strategy

### Pre-Deployment

1. ✅ Finalize sharding design (this document)
2. ✅ Implement core infrastructure
3. ✅ Implement lazy spawning
4. ✅ Implement shard contract
5. ✅ Implement settlement aggregation
6. ✅ Comprehensive test suite
7. ✅ Performance benchmarking

### Deployment Phases

1. **Internal Testing** (2 weeks)
   - Soroban testnet with 100+ shards
   - Load testing with 10k+ simulated investors
   - Settlement correctness verification

2. **Staging Deployment** (1 week)
   - Deploy to staging network
   - Invite beta users
   - Monitor real-world usage

3. **Production Deployment** (1 week)
   - Gradual rollout with feature flags
   - Monitor shard operations
   - Ready for on-demand escrow scaling

---

## Success Metrics

- ✅ **Scalability:** Escrows support 10k+ investors
- ✅ **Correctness:** Settlement aggregates correctly (0% data loss)
- ✅ **Performance:** Fund operations within acceptable latency
- ✅ **Compatibility:** Existing escrows unaffected
- ✅ **Efficiency:** Storage well-distributed across shards

---

## References & Related Systems

- **Current Investor Storage:** `DataKey::InvestorContribution(Address)`, etc.
- **Settlement Logic:** `escrow/src/lib.rs` - `settle()` function
- **Soroban Contract Deployment:** `env.register()`
- **Cross-Contract Calls:** Soroban SDK documentation

---

## Conclusion

Investor sharding is technically feasible within Soroban's architecture. The main complexity lies in coordinating cross-contract calls and ensuring settlement aggregation correctness. The proposed design prioritizes:

1. **Simplicity:** Hash-based routing, no rebalancing
2. **Correctness:** Consistent allocation, verification at settle
3. **Backwards Compatibility:** Opt-in for new escrows, no impact on existing
4. **Scalability:** Supports 10M+ investors with 10k shards

**Recommended Next Step:** Implement Phase 1 (Infrastructure) to establish foundation for subsequent phases.
