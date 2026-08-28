# Feature #221: Benchmark Suite for Performance Tracking

## Overview

Implement comprehensive benchmarks for key operations (fund, settle, claim) across different pool sizes to detect performance regressions over versions.

## Design

### 1. Benchmark Structure

```
escrow/benches/
  ├─ criterion.toml              # Criterion configuration
  ├─ lib.rs                       # Shared benchmark utilities
  └─ main.rs                      # Benchmark suite entry point
      ├─ bench_fund
      ├─ bench_settle
      ├─ bench_claim_payout
      ├─ bench_compute_payout
      └─ bench_sweep_dust
```

### 2. Key Benchmarks

#### 2.1 Fund Operation

**Benchmark:** `bench_fund_single_investor`
- Setup: Fresh escrow, funding target = 1000
- Scenarios:
  - 1 investor, 1000 units (completes funding)
  - 1 investor, 500 units (partial)

**Benchmark:** `bench_fund_bulk_sequential`
- Setup: Escrow, funding target = 10000
- Scenarios:
  - 10 investors × 1000 units each
  - 100 investors × 100 units each
  - 1000 investors × 10 units each
- Measure: Per-investor overhead (time per fund call)

**Benchmark:** `bench_fund_with_commitment`
- Setup: Escrow with tiered yield, funding target = 1000
- Scenarios:
  - Fund with commitment lock (1 investor)
  - Multiple investors with different tiers (100)

#### 2.2 Settle Operation

**Benchmark:** `bench_settle_simple`
- Setup: Funded escrow, past maturity
- Scenarios:
  - Immediate settle (no prior state)
  - Settle after long-running campaign

**Benchmark:** `bench_settle_with_snapshot`
- Setup: Large investor pool (1000+)
- Verify: Snapshot creation doesn't add significant overhead

#### 2.3 Claim Payout Operation

**Benchmark:** `bench_claim_single`
- Setup: Settled escrow, 1 investor
- Measure: Single payout computation time

**Benchmark:** `bench_claim_bulk_sequential`
- Setup: Settled escrow
- Scenarios:
  - 10 investors claiming sequentially
  - 100 investors claiming sequentially
  - 1000 investors claiming sequentially
- Measure: Per-investor claim overhead

**Benchmark:** `bench_compute_payout_direct`
- Setup: Pre-computed investor set
- Scenarios:
  - 1 investor with various contribution levels
  - 100 investors, equal contribution
  - 1000 investors, unequal contributions (realistic distribution)
- Measure: Core yield calculation (compute_investor_payout)

#### 2.4 Sweep Dust Operation

**Benchmark:** `bench_sweep_terminal_dust`
- Setup: Terminal escrow (settled/withdrawn)
- Scenarios:
  - Sweep 1M units (typical)
  - Sweep MAX_DUST_SWEEP_AMOUNT
  - Sweep with large investor pool (no impact expected)

### 3. Metrics Collected

Per benchmark:
- **Execution time:** Mean, median, stddev
- **Throughput:** Operations per second
- **Memory peak:** Heap allocations
- **Storage reads:** Via env counters (if available)
- **Storage writes:** Via env counters (if available)

### 4. Baseline Targets (Realistic)

| Operation | Scenario | Target | Ceiling |
|-----------|----------|--------|---------|
| fund | 1 investor | 5ms | 10ms |
| fund | 100 investors (bulk) | 500ms | 1s |
| fund | 1000 investors (bulk) | 5s | 10s |
| settle | Funded escrow | 10ms | 20ms |
| claim | 1 investor | 3ms | 5ms |
| claim | 100 investors | 300ms | 500ms |
| claim | 1000 investors | 3s | 5s |
| compute_payout | 1 investor | 1ms | 2ms |
| compute_payout | 1000 investors | 1s | 2s |
| sweep_dust | Terminal | 5ms | 10ms |

### 5. Regression Detection

**Strategy:** Criterion automatic baseline comparison

1. First run establishes baseline (stored in `target/criterion/`)
2. Subsequent runs compare against baseline
3. Flag if:
   - Mean time increases > 10%
   - Median increases > 8%
   - Stddev increases dramatically (>20%)

**CI Integration:**
```bash
cargo bench --bench main -- --verbose
# CI fails if regression detected
```

### 6. Implementation Plan

#### 6.1 Benchmark Utilities (lib.rs)

```rust
use soroban_sdk::{Env, Address, Symbol, String};

/// Generate a fresh test environment with escrow initialized
pub fn setup_escrow_env(funding_target: i128, num_investors: usize) -> (Env, EscrowClient) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = create_mock_token(&env);
    let treasury = Address::generate(&env);
    
    let client = init_escrow(
        &env,
        admin,
        sme,
        token.clone(),
        treasury,
        funding_target,
        Symbol::new(&env, "TEST_INV"),
    );
    
    (env, client)
}

/// Generate N investor addresses
pub fn generate_investors(env: &Env, count: usize) -> Vec<Address> {
    (0..count)
        .map(|_| Address::generate(env))
        .collect()
}

/// Fund escrow with sequential investor deposits
pub fn fund_escrow_sequential(
    client: &EscrowClient,
    investors: &[Address],
    amount_per_investor: i128,
) {
    for investor in investors {
        client.fund(investor, amount_per_investor);
    }
}

/// Settle escrow and prepare for claims
pub fn settle_escrow(env: &Env, client: &EscrowClient, sme: &Address) {
    // Move time past maturity
    env.ledger().set_timestamp(env.ledger().timestamp() + 86400 * 365);
    client.settle(sme);
}
```

#### 6.2 Main Benchmark File (main.rs)

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use karis_ky_escrow_benches::*;

fn bench_fund_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("fund");
    
    group.bench_function("fund_1_investor", |b| {
        b.iter(|| {
            let (env, client) = setup_escrow_env(1000, 1);
            let investors = generate_investors(&env, 1);
            fund_escrow_sequential(&client, &investors, 1000);
        });
    });
    
    group.throughput(Throughput::Elements(1));
    group.bench_function("fund_100_investors", |b| {
        b.iter(|| {
            let (env, client) = setup_escrow_env(10000, 100);
            let investors = generate_investors(&env, 100);
            fund_escrow_sequential(&client, &investors, 100);
        });
    });
    
    group.throughput(Throughput::Elements(100));
    group.bench_function("fund_1000_investors", |b| {
        b.iter(|| {
            let (env, client) = setup_escrow_env(100000, 1000);
            let investors = generate_investors(&env, 1000);
            fund_escrow_sequential(&client, &investors, 100);
        });
    });
    
    group.throughput(Throughput::Elements(1000));
    group.finish();
}

fn bench_claim_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("claim");
    
    group.bench_function("claim_1_investor", |b| {
        b.iter(|| {
            let (env, client) = setup_escrow_env(1000, 1);
            let investors = generate_investors(&env, 1);
            fund_escrow_sequential(&client, &investors, 1000);
            settle_escrow(&env, &client, &black_box(Address::generate(&env)));
            client.claim_investor_payout(&investors[0]);
        });
    });
    
    group.throughput(Throughput::Elements(1));
    group.bench_function("claim_100_investors", |b| {
        b.iter(|| {
            let (env, client) = setup_escrow_env(10000, 100);
            let investors = generate_investors(&env, 100);
            fund_escrow_sequential(&client, &investors, 100);
            settle_escrow(&env, &client, &black_box(Address::generate(&env)));
            
            for investor in &investors {
                client.claim_investor_payout(investor);
            }
        });
    });
    
    group.throughput(Throughput::Elements(100));
    group.finish();
}

criterion_group!(benches, bench_fund_operations, bench_claim_operations);
criterion_main!(benches);
```

#### 6.3 Cargo.toml Updates

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "main"
harness = false
```

#### 6.4 criterion.toml

```toml
[measurement]
time_unit = "ms"
sample_size = 100
warm_up_time = 3
measurement_time = 5

[output_format]
verbose = true
```

## Usage

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench --bench main

# Run specific benchmark group
cargo bench --bench main -- fund

# Generate HTML report
cargo bench --bench main -- --verbose

# Compare against baseline
cargo bench --bench main -- --baseline main
```

### Interpreting Results

Example output:
```
fund/fund_100_investors        time:   [523.45 ms 531.23 ms 540.12 ms]
                                        change: [-2.3% +0.1% +2.5%] (within noise)
                                        baseline:   [537 ms]
                                        comparison: 1.02x slower
```

**Traffic lights:**
- 🟢 Green: Within noise (±5%)
- 🟡 Yellow: Slight regression (5-10%)
- 🔴 Red: Significant regression (>10%)

## CI Integration

Add to `.github/workflows/ci.yml`:

```yaml
- name: Run benchmarks
  run: cargo bench --bench main --no-fail-fast
  
- name: Upload benchmark results
  uses: actions/upload-artifact@v3
  if: always()
  with:
    name: benchmark-results
    path: target/criterion/
```

## Regression Workflow

1. **Developer makes optimization PR**
2. **Benchmark runs; baseline improves**
3. **CI publishes new baseline artifact**
4. **Future regressions compared against new baseline**
5. **Alert if regression > 10%**

## Notes

- Benchmarks use test environment (no real token/network)
- Times may vary between runs; criterion handles variance
- Storage overhead dominates (I/O >> computation)
- Parallelization benchmarks added in Feature #218

