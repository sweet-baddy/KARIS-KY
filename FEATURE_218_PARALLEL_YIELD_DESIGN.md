# Feature #218: Parallel Yield Calculation for Large Pools

## Overview

Optimize yield calculation to parallelize when investor pool > 1000, reducing computation time for large escrows.

## Design

### 1. Architecture Decision: Host vs. WASM Parallelism

**Issue:** Soroban contracts compile to WASM, which runs in a single-threaded sandbox. True parallelization (rayon) is not supported.

**Solution:** Three-tier approach

**Tier 1: Sequential (current, always available)**
```rust
pub fn compute_investor_payout(env: Env, investor: Address) -> i128 {
    // Current implementation (unchanged)
}
```

**Tier 2: Batch-optimized (in-contract, no parallelism)**
```rust
pub fn compute_investor_payouts_batch(
    env: Env,
    investors: Vec<Address>,
) -> Vec<i128> {
    // Optimize storage access patterns (batch reads)
    // Reduce function call overhead
}
```

**Tier 3: Host-side parallel (off-contract, for indexer/backend)**
- Implement `compute_payouts_parallel` in a separate Rust library (not compiled to WASM)
- Called by off-chain indexer/backend to compute payouts for settlement events
- Uses rayon for parallelism across multiple cores

### 2. In-Contract Batch Optimization

**Problem with current approach:**
```rust
for investor in investors {
    let payout = compute_investor_payout(env.clone(), investor);
    // Makes N separate calls, each:
    // - Clones env
    // - Reads Escrow once (wasteful, same data)
    // - Reads FundingCloseSnapshot once (wasteful, same data)
    // - Reads InvestorEffectiveYield + InvestorContribution (necessary)
}
```

**Optimized batch approach:**

```rust
pub fn compute_investor_payouts_batch(
    env: Env,
    investors: Vec<Address>,
) -> Vec<i128> {
    // Cache shared reads (avoid N redundant reads)
    let escrow = Self::get_escrow(env.clone());
    let snapshot = env
        .storage()
        .instance()
        .get::<DataKey, FundingCloseSnapshot>(&DataKey::FundingCloseSnapshot);
    
    if let Some(snap) = snapshot {
        investors.iter().map(|investor| {
            Self::compute_investor_payout_cached(
                env.clone(),
                investor.clone(),
                &escrow,
                &snap,
            )
        }).collect()
    } else {
        vec![0; investors.len()]
    }
}

// Helper: payout computation with cached escrow/snapshot
fn compute_investor_payout_cached(
    env: Env,
    investor: Address,
    escrow: &InvoiceEscrow,
    snap: &FundingCloseSnapshot,
) -> i128 {
    let contribution = Self::get_persistent_investor_contribution(&env, investor.clone());
    if contribution == 0 {
        return 0;
    }
    
    let total_principal = snap.total_principal;
    if total_principal <= 0 {
        return 0;
    }
    
    let effective_yield_bps = Self::get_persistent_investor_effective_yield(&env, investor)
        .unwrap_or(escrow.yield_bps);
    
    let coupon = total_principal
        .checked_mul(effective_yield_bps as i128)
        .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
        .checked_div(10_000)
        .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));
    
    let settle_pool = total_principal
        .checked_add(coupon)
        .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));
    
    contribution
        .checked_mul(settle_pool)
        .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
        .checked_div(total_principal)
        .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
}
```

**Benefits:**
- Single Escrow read (instead of N)
- Single FundingCloseSnapshot read (instead of N)
- Still N reads for InvestorContribution (necessary per investor)
- Still N reads for InvestorEffectiveYield (necessary per investor)

**Expected speedup:** ~10-15% for large pools (Escrow + Snapshot reads are small, but called N times).

### 3. Off-Contract Parallel Library

**New crate:** `karis-ky-yield-parallel` (separate from contract)

```toml
[package]
name = "karis_ky_yield_parallel"
version = "0.1.0"

[dependencies]
rayon = "1.7"
serde = { version = "1.0", features = ["derive"] }

[features]
default = ["parallel"]
parallel = ["rayon"]
```

**Implementation:**

```rust
use rayon::prelude::*;

/// Parallel investor payout computation (host-side only, not WASM)
pub struct YieldCompute {
    pub escrow_yield_bps: i64,
    pub total_principal: i128,
    pub investors: Vec<InvestorData>,
}

pub struct InvestorData {
    pub address: String,
    pub contribution: i128,
    pub effective_yield_bps: Option<i64>,
}

pub struct PayoutResult {
    pub investor: String,
    pub payout: i128,
}

impl YieldCompute {
    /// Sequential computation (fallback)
    pub fn compute_sequential(self) -> Vec<PayoutResult> {
        self.investors.into_iter().map(|inv| self.compute_payout(&inv)).collect()
    }
    
    /// Parallel computation (if pool > threshold)
    pub fn compute_parallel(self, threshold: usize) -> Vec<PayoutResult> {
        if self.investors.len() <= threshold {
            self.compute_sequential()
        } else {
            self.investors
                .into_par_iter()
                .map(|inv| self.compute_payout(&inv))
                .collect()
        }
    }
    
    fn compute_payout(&self, investor: &InvestorData) -> PayoutResult {
        if investor.contribution == 0 {
            return PayoutResult {
                investor: investor.address.clone(),
                payout: 0,
            };
        }
        
        let effective_yield = investor.effective_yield_bps.unwrap_or(self.escrow_yield_bps);
        let coupon = (self.total_principal as f64 * effective_yield as f64 / 10_000.0).floor() as i128;
        let settle_pool = self.total_principal + coupon;
        let payout = (investor.contribution as f64 * settle_pool as f64 / self.total_principal as f64).floor() as i128;
        
        PayoutResult {
            investor: investor.address.clone(),
            payout,
        }
    }
}
```

**Usage in backend/indexer:**

```rust
use karis_ky_yield_parallel::YieldCompute;

// After settlement, compute all payouts in parallel
let compute = YieldCompute {
    escrow_yield_bps: 500,
    total_principal: 10_000_000,
    investors: vec![...],
};

let payouts = compute.compute_parallel(1000);
// Uses rayon if investor count > 1000, otherwise sequential
```

### 4. Contract-Level Feature Flag

Add to `Cargo.toml`:

```toml
[features]
default = []
parallel-yield = []

[dev-dependencies]
karis_ky_yield_parallel = { path = "../../yield-parallel", optional = true }
```

### 5. In-Contract New Entrypoint (Optional)

```rust
pub fn compute_payouts_for_settlement(
    env: Env,
    settlement_addr: Address,
) -> Vec<(Address, i128)> {
    // Settlement-only: enumerate all investors and compute payouts
    // Requires escrow to be settled already
    
    let escrow = Self::get_escrow(env.clone());
    ensure(&env, escrow.status == 2, EscrowError::InvestorClaimNotSettled);
    
    // Fetch all unique investors from persistent storage
    // (requires enumeration capability - may not be available)
    
    // For now, this is NOT implemented in WASM
    // Indexer calls individual compute_investor_payout calls
    // OR uses off-contract parallel library
}
```

**Note:** Soroban persistent storage doesn't provide address enumeration, so we can't easily implement this in-contract. Instead, the indexer maintains the investor list and uses the parallel library.

### 6. Benchmarking

**New benchmarks in Feature #221:**

```rust
#[bench]
fn bench_compute_payouts_batch_vs_sequential(b: &mut Bencher) {
    let (env, client) = setup_escrow_env(100000, 1000);
    let investors = generate_investors(&env, 1000);
    fund_escrow_sequential(&client, &investors, 100);
    settle_escrow(&env, &client, &sme);
    
    b.iter(|| {
        client.compute_investor_payouts_batch(black_box(investors.clone()))
    });
}

#[bench]
fn bench_compute_payouts_parallel_offchain(b: &mut Bencher) {
    use karis_ky_yield_parallel::YieldCompute;
    
    let investor_data: Vec<InvestorData> = (0..1000)
        .map(|i| InvestorData {
            address: format!("investor_{}", i),
            contribution: 100,
            effective_yield_bps: None,
        })
        .collect();
    
    b.iter(|| {
        YieldCompute {
            escrow_yield_bps: 500,
            total_principal: 100_000,
            investors: black_box(investor_data.clone()),
        }.compute_parallel(1000)
    });
}
```

**Expected results:**
- Batch optimization: ~10-15% faster than sequential individual calls
- Off-contract parallel (on 4-core machine): ~3-3.5x faster than sequential
- Off-contract parallel (on 8-core machine): ~6-7x faster than sequential

### 7. Migration Notes

- **Schema version:** No change (no new storage keys)
- **Breaking changes:** None
- **Deprecations:** None
- **Feature flags:** Optional; existing code works without change

### 8. Limitations & Future Work

**Current limitations:**
1. WASM single-threaded execution prevents in-contract parallelism
2. No address enumeration in Soroban → indexer must track investors
3. Off-contract library requires backend/indexer support

**Future work:**
1. Soroban WASM multi-core support (if added)
2. Implement contract iterator for investor enumeration
3. Integration with Soroban indexer (Stellar RPC)

