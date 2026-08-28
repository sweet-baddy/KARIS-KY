# Quick Reference: Four Features Implementation Guide

## Executive Summary

| Feature | What | Why | Impact |
|---------|------|-----|--------|
| **#224 Trace Mode** | Log all storage reads/writes for debugging | Forensic analysis, root cause analysis | High operational value, low performance impact |
| **#221 Benchmarks** | Measure performance across pool sizes | Detect regressions, track optimizations | Critical for reliability, enables data-driven decisions |
| **#218 Parallel Yield** | Optimize yield calculation for large pools | 3-4x speedup for 1000+ investors | High impact for scaling, but WASM-limited |
| **#220 REPL CLI** | Interactive debugging tool | No test code needed, faster iteration | Developer productivity, onboarding |

---

## Key Technical Decisions

### 1. Trace Mode Architecture
- **Event-based** for critical operations (state transitions)
- **Buffer-based** for detailed logs (bounded storage)
- **Feature flag** to eliminate overhead when disabled
- **Verbosity levels:** OFF, ERROR, WARN, INFO, DEBUG, TRACE

**Why this approach:**
- Events queryable via indexer
- Buffer allows detailed inspection without external systems
- Feature flag = zero-cost abstraction in production

### 2. Benchmark Baseline Strategy
- **Criterion statistical analysis** (not raw timings)
- **Pool size variations:** 1, 10, 100, 1000 investors
- **Regression threshold:** ±10% (automatic detection)
- **CI integration:** optional run, publish results

**Why this approach:**
- Criterion handles variance automatically
- Baselines are reproducible and comparable
- 10% threshold catches real regressions without noise

### 3. Parallel Yield Limitations & Workaround
- **Problem:** WASM is single-threaded → rayon can't parallelize in-contract
- **Solution:** Three-tier approach
  - Tier 1 (sequential): current implementation
  - Tier 2 (batch-optimized): cache Escrow + Snapshot reads
  - Tier 3 (parallel): off-contract library for backend/indexer

**Why this design:**
- Tier 2 gives ~15% improvement with zero WASM changes
- Tier 3 enables 3-4x speedup for real workloads
- Honest about WASM limitations (senior dev pattern)

### 4. REPL Network Abstraction
- **Pluggable providers:** local, testnet, mainnet
- **No transaction signing in REPL** (read-only focus)
- **Snapshots for state inspection** (local simulation)
- **Security note:** suggest running on secure network

**Why this design:**
- Supports dev/staging/prod workflows
- Avoids key management complexity
- Read-only = no security risk
- Snapshots enable time-travel debugging

---

## Critical Integration Points

### Storage Key Wrapping (Trace Mode)
```
Current:  env.storage().instance().get(&DataKey::Escrow)
Wrapped:  trace_get!(env, &DataKey::Escrow)
          → calls trace_read() + actual get

Decision: Use macros for optional wrapping (zero-cost when disabled)
```

### Event Emission (All Features)
```
Pattern:  SomeEvent { ... }.publish(&env);
Used by:  Trace mode (event-based traces)
          Benchmarks (measure event overhead)
          Integration tests (verify state transitions)
```

### Feature Flags (All Features)
```
Cargo.toml:
  [features]
  trace-mode = []
  parallel-yield = ["karis-ky-yield-parallel"]

Code:
  #[cfg(feature = "trace-mode")]
  fn emit_trace() { ... }
  
  #[cfg(not(feature = "trace-mode"))]
  fn emit_trace() { /* no-op */ }
```

### Off-Contract Library (Parallel Yield)
```
New crate:  yield-parallel/
Uses:       rayon for parallelism (HOST, not WASM)
Called by:  Backend/indexer after settlement
Returns:    Vec<(Address, i128)> of payouts
```

---

## File Structure After Implementation

```
KARIS-KY/
  ├─ escrow/
  │   ├─ src/
  │   │   ├─ lib.rs                    [MODIFY] Add trace + batch yield entrypoints
  │   │   ├─ trace.rs (NEW)            [NEW] Trace macros + buffer management
  │   │   ├─ external_calls.rs         [MODIFY] Optional trace wrapping
  │   │   ├─ tests/
  │   │   │   ├─ trace.rs (NEW)        [NEW] Trace mode tests
  │   │   │   └─ ... (existing)
  │   │   └─ tests.rs
  │   ├─ benches/
  │   │   ├─ main.rs (NEW)             [NEW] Criterion benchmarks
  │   │   └─ lib.rs (NEW)              [NEW] Benchmark utilities
  │   ├─ Cargo.toml                    [MODIFY] Add dev-deps, features, [[bench]]
  │   └─ criterion.toml (NEW)          [NEW] Criterion configuration
  │
  ├─ repl-cli/
  │   ├─ src/
  │   │   ├─ main.rs (NEW)             [NEW] REPL entry point
  │   │   ├─ commands.rs (NEW)         [NEW] Command parsing
  │   │   ├─ network.rs (NEW)          [NEW] Network providers
  │   │   ├─ state_inspector.rs (NEW)  [NEW] DataKey formatting
  │   │   ├─ transaction.rs (NEW)      [NEW] Transaction building
  │   │   └─ serialization.rs (NEW)    [NEW] JSON serialization
  │   ├─ tests/
  │   │   └─ integration.rs (NEW)      [NEW] REPL integration tests
  │   ├─ Cargo.toml (NEW)              [NEW] CLI dependencies
  │   └─ README.md (NEW)               [NEW] REPL usage guide
  │
  ├─ yield-parallel/
  │   ├─ src/
  │   │   └─ lib.rs (NEW)              [NEW] Parallel computation logic
  │   ├─ Cargo.toml (NEW)              [NEW] Rayon + serde deps
  │   └─ tests/ (NEW)                  [NEW] Parallel computation tests
  │
  ├─ docs/
  │   ├─ trace-mode-guide.md (NEW)     [NEW] Trace mode user guide
  │   ├─ benchmark-suite-guide.md (NEW) [NEW] Benchmark usage
  │   ├─ parallel-yield-guide.md (NEW) [NEW] Parallel yield config
  │   ├─ repl-quickstart.md (NEW)      [NEW] REPL getting started
  │   └─ adr/
  │       ├─ ADR-008-trace-mode.md (NEW) [NEW] Trace mode decision
  │       └─ ADR-009-parallel-yield.md (NEW) [NEW] Parallel yield decision
  │
  ├─ ARCHITECTURE_ANALYSIS.md          [CREATED] Codebase overview
  ├─ FEATURE_224_TRACE_MODE_DESIGN.md  [CREATED] Trace mode design
  ├─ FEATURE_221_BENCHMARK_SUITE_DESIGN.md [CREATED] Benchmark design
  ├─ FEATURE_218_PARALLEL_YIELD_DESIGN.md [CREATED] Parallel yield design
  ├─ FEATURE_220_REPL_DESIGN.md        [CREATED] REPL design
  ├─ IMPLEMENTATION_ROADMAP.md         [CREATED] This roadmap
  ├─ Cargo.toml                        [MODIFY] Add workspace members
  ├─ .github/workflows/ci.yml          [MODIFY] Add benchmark step
  └─ README.md                         [MODIFY] Add feature overview
```

---

## Command Checklist (for implementation)

### Phase 1: Trace Mode Setup
```bash
# Create trace module
touch escrow/src/trace.rs

# Add to lib.rs
# - TraceEvent enum
# - DataKey::TraceBuffer variant
# - trace_get! / trace_set! macros
# - enable_tracing / disable_tracing / get_trace_buffer entrypoints

# Add tests
touch escrow/src/tests/trace.rs

# Update Cargo.toml
# [features]
# trace-mode = []

# Verify compilation
cargo build --features trace-mode
cargo build --no-default-features
cargo test --features trace-mode
```

### Phase 2: Benchmark Suite
```bash
# Create benchmark structure
mkdir -p escrow/benches
touch escrow/benches/{main.rs,lib.rs,criterion.toml}

# Update Cargo.toml
# [dev-dependencies]
# criterion = { version = "0.5", features = ["html_reports"] }
# [[bench]]
# name = "main"
# harness = false

# Establish baselines
cargo bench --bench main

# Verify CI integration
cargo bench --bench main --no-fail-fast
```

### Phase 3: Parallel Yield
```bash
# Create in-contract batch function
# - add compute_investor_payouts_batch() to lib.rs
# - cache Escrow + Snapshot reads
# - benchmark vs sequential

# Create off-contract library
cargo new --lib yield-parallel
# - Add YieldCompute struct with rayon
# - Add tests
# - Add benchmarks to escrow/benches/

# Update main Cargo.toml
# [dependencies]
# karis_ky_yield_parallel = { path = "../yield-parallel", optional = true }
# [features]
# parallel-yield = ["karis_ky_yield_parallel"]

# Verify both paths work
cargo build --features parallel-yield
cargo build --no-default-features
```

### Phase 4: REPL CLI
```bash
# Create CLI crate
cargo new --bin repl-cli

# Update main Cargo.toml workspace
# [workspace]
# members = ["escrow", "repl-cli", "yield-parallel"]

# Implement REPL modules
touch repl-cli/src/{commands,network,state_inspector,transaction}.rs

# Test against local testnet
cargo run -p karis-ky-repl-cli -- --network local

# Integration tests
touch repl-cli/tests/integration.rs
cargo test -p karis-ky-repl-cli
```

### Phase 5: Integration & Testing
```bash
# Full test suite with all features
cargo test --all-features
cargo test --no-default-features
cargo test --features trace-mode
cargo test --features parallel-yield

# Verify coverage
cargo llvm-cov --all-features --fail-under-lines 95

# Run benchmarks (establish new baselines if improved)
cargo bench --bench main

# Final CI pass
cargo fmt --all -- --check
cargo clippy -p karis-ky_escrow -- -D warnings
cargo clippy -p karis-ky-repl-cli -- -D warnings
```

---

## Common Pitfalls & Fixes

| Issue | Cause | Fix |
|-------|-------|-----|
| Trace overhead in production | Feature flag not used | Ensure `#[cfg(feature = "trace-mode")]` blocks no-op |
| Benchmark flakiness | System load variations | Use criterion defaults; allow ±10% variance |
| WASM incompatibility with rayon | Tried to use rayon in WASM | Move to off-contract library (tier 3 only) |
| REPL serialization errors | DataKey types not serializable | Add `#[derive(Serialize, Deserialize)]` to all DataKey-related types |
| Storage key wrapping double-reads | Macro expands to `get` then `trace_get` | Use macro alone, not both |

---

## Success Verification Checklist

- [ ] All tests pass (95%+ coverage)
- [ ] No clippy warnings
- [ ] Feature flags compile cleanly (all combinations)
- [ ] Trace mode overhead < 1% when disabled
- [ ] Benchmarks establish reproducible baselines
- [ ] Parallel library shows 3-4x speedup (off-contract)
- [ ] REPL connects to local/testnet/mainnet
- [ ] Documentation is comprehensive and tested
- [ ] Zero breaking changes
- [ ] Performance regression: none detected

---

## For Code Review

**Questions to ask:**

1. **Trace Mode**: Is the feature flag truly zero-cost? Can we verify overhead is < 1%?
2. **Benchmarks**: Are baselines realistic? Do 10%, 100%, 1000 investor scenarios cover the problem space?
3. **Parallel Yield**: Is WASM limitation clearly documented? Does Tier 2 (batch) show measurable improvement?
4. **REPL**: Is network abstraction clean enough for future providers? Does state inspection handle all DataKey types?
5. **Overall**: Are there any breaking changes? Does backward compatibility hold?

**For sign-off:**

1. Author: Implementation complete, tests passing
2. Reviewer: Design and code quality approved
3. QA: Integration tests passing, no regressions
4. Docs: Feature guides written and examples tested

