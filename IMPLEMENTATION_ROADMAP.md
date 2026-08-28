# Implementation Roadmap: Four Major Features

**Overall Timeline:** 4-6 weeks (estimated)  
**Phase Structure:** Sequential delivery with parallel testing

---

## Phase 1: Trace Mode (#224) - Weeks 1-2

### Deliverables
1. Add `TraceEvent` enum and `DataKey::TraceBuffer` storage
2. Implement trace wrapper macros (optional optimization)
3. Add trace level management entrypoints
4. Integration tests for trace collection
5. Feature flag compilation

### Files to Modify/Create
```
escrow/src/lib.rs                 # Add TraceEvent, trace entrypoints, DataKey variants
escrow/src/trace.rs (NEW)         # Trace macro implementations, buffer management
escrow/src/tests/trace.rs (NEW)   # Unit + integration tests for trace mode
Cargo.toml                         # Add trace-mode feature flag
```

### Acceptance Criteria
- [ ] Trace events emit correctly via Soroban event system
- [ ] Trace buffer persists and retrieves correctly
- [ ] Feature flag compilation works (no-op when disabled)
- [ ] Zero overhead when trace level = OFF
- [ ] Tests pass with 95%+ coverage

### Estimated Effort
- Implementation: 20 hours
- Testing: 8 hours
- Documentation: 4 hours

---

## Phase 2: Benchmark Suite (#221) - Weeks 2-3

### Deliverables
1. Create `escrow/benches/` directory structure
2. Implement criterion benchmarks for fund/settle/claim
3. Add benchmark utilities (state generators)
4. Establish performance baselines
5. CI integration for benchmark runs

### Files to Modify/Create
```
escrow/benches/main.rs (NEW)      # Main benchmark entry point
escrow/benches/lib.rs (NEW)       # Shared benchmark utilities
escrow/benches/criterion.toml (NEW) # Criterion configuration
Cargo.toml                         # Add criterion dev-dependency, [[bench]] section
.github/workflows/ci.yml           # Add benchmark step
```

### Acceptance Criteria
- [ ] All 5 benchmark groups run successfully
- [ ] Baseline results stored and retrieved correctly
- [ ] Criterion HTML reports generate cleanly
- [ ] CI integration works without flakiness
- [ ] Baselines established for fund/settle/claim (10/100/1000 investor scenarios)

### Estimated Effort
- Implementation: 24 hours
- Baseline establishment: 8 hours
- CI/CD integration: 6 hours

---

## Phase 3: Parallel Yield (#218) - Weeks 3-4

### Deliverables
1. Implement `compute_investor_payouts_batch()` in-contract
2. Create separate `karis-ky-yield-parallel` library (with rayon)
3. Add benchmark comparisons (sequential vs batch vs parallel)
4. Feature flag for parallel mode
5. Documentation on WASM limitations

### Files to Modify/Create
```
escrow/src/lib.rs                 # Add compute_investor_payouts_batch() entrypoint
yield-parallel/Cargo.toml (NEW)   # New library crate
yield-parallel/src/lib.rs (NEW)   # Parallel computation logic using rayon
escrow/Cargo.toml                 # Add parallel-yield feature, optional yield-parallel dependency
escrow/benches/parallel.rs (NEW)  # Parallel vs sequential benchmarks
```

### Acceptance Criteria
- [ ] Batch optimization shows ~10-15% improvement over sequential
- [ ] Off-contract parallel library compiles and tests pass
- [ ] Parallel library demonstrates 3-4x speedup on 4-core machine
- [ ] WASM incompatibility documented with fallback notes
- [ ] Feature flag works correctly

### Estimated Effort
- In-contract batch: 12 hours
- Off-contract library: 16 hours
- Benchmarking: 8 hours

---

## Phase 4: REPL CLI Tool (#220) - Weeks 4-5

### Deliverables
1. Create `repl-cli/` binary target in workspace
2. Implement command parser and REPL loop
3. Add network provider abstraction (local/testnet/mainnet)
4. State inspection and formatting
5. Integration tests against local testnet

### Files to Modify/Create
```
repl-cli/Cargo.toml (NEW)         # New binary crate
repl-cli/src/main.rs (NEW)        # REPL loop entry point
repl-cli/src/commands.rs (NEW)    # Command parsing and routing
repl-cli/src/network.rs (NEW)     # Network provider abstraction
repl-cli/src/state_inspector.rs (NEW) # DataKey deserialization/formatting
repl-cli/src/transaction.rs (NEW) # Transaction building helpers
repl-cli/tests/integration.rs (NEW) # REPL integration tests
Cargo.toml                         # Add repl-cli to workspace
```

### Acceptance Criteria
- [ ] REPL starts and accepts commands
- [ ] `call` command invokes contract methods
- [ ] `get` command reads DataKeys correctly
- [ ] `state` command formats escrow/investor state nicely
- [ ] Network switching works (local/testnet/mainnet)
- [ ] Help system is comprehensive
- [ ] Integration tests pass against local testnet

### Estimated Effort
- Core REPL: 20 hours
- Commands implementation: 24 hours
- Testing: 12 hours

---

## Phase 5: Integration Testing - Week 5

### Deliverables
1. Full test suite with all features enabled
2. Trace mode on realistic transaction flows
3. Benchmark regression detection
4. REPL interaction tests
5. Cross-feature integration verification

### Test Coverage
- [ ] Trace mode captures all fund/settle/claim operations
- [ ] Benchmark suite runs without regression (within 10%)
- [ ] Parallel library produces identical results to sequential
- [ ] REPL can query trace buffer and state
- [ ] All 95%+ coverage maintained

### Estimated Effort
- Integration setup: 8 hours
- Testing: 16 hours
- Regression analysis: 4 hours

---

## Phase 6: Documentation - Week 6

### Deliverables
1. Trace mode user guide (setup, interpretation, examples)
2. Benchmark suite runbook (baseline usage, regression workflow)
3. Parallel yield configuration guide
4. REPL command reference and examples
5. Updated README with feature overview
6. Architecture Decision Record (ADR) for each feature

### Documents to Create
```
docs/trace-mode-guide.md          # How to enable, read, interpret traces
docs/benchmark-suite-guide.md     # Running benchmarks, reading results
docs/parallel-yield-guide.md      # Configuration, off-contract library usage
docs/repl-quickstart.md           # Getting started with CLI tool
docs/adr/ADR-008-trace-mode.md    # Architecture decision for tracing
docs/adr/ADR-009-parallel-yield.md # Architecture decision for parallelization
README.md                          # Update with feature overview section
```

### Acceptance Criteria
- [ ] Each feature has comprehensive user guide
- [ ] All code examples are tested and working
- [ ] Performance expectations documented
- [ ] Troubleshooting section for common issues
- [ ] ADRs follow existing format and conventions

### Estimated Effort
- Writing: 20 hours
- Review/examples: 8 hours

---

## Dependency Management

### New Dependencies to Add

**In `escrow/Cargo.toml`:**
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[features]
default = []
trace-mode = []
parallel-yield = ["karis-ky-yield-parallel"]

[dependencies]
karis-ky-yield-parallel = { path = "../yield-parallel", optional = true }
```

**In `repl-cli/Cargo.toml`:**
```toml
[dependencies]
clap = { version = "4.4", features = ["derive"] }
rustyline = "14.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
soroban-sdk = "25.0"

[dev-dependencies]
```

**In `yield-parallel/Cargo.toml`:**
```toml
[dependencies]
rayon = "1.7"
serde = { version = "1.0", features = ["derive"] }

[features]
default = ["parallel"]
parallel = ["rayon"]
```

### Cargo.lock Management
- Lock all new dependency versions (no `*` ranges)
- Include lockfile in PR for review
- Document any advisory bumps

---

## Testing & Quality Gates

### Pre-Commit Checks
```bash
cargo fmt --all -- --check
cargo clippy -p karis-ky_escrow -- -D warnings
cargo clippy -p karis-ky-repl-cli -- -D warnings
```

### CI Workflow
```bash
cargo build
cargo test
cargo bench --bench main --no-fail-fast
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
```

### Feature Flag Testing
```bash
# Test each feature flag combination
cargo test --features trace-mode
cargo test --features parallel-yield
cargo test --all-features
cargo test --no-default-features
```

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Trace mode overhead > 15% in DEBUG | Medium | Use feature flag; test OFF path |
| Benchmark flakiness in CI | Medium | Use criterion filters; allow variance |
| Rayon incompatible with WASM | High | Document limitation; provide fallback |
| REPL security (arbitrary reads) | Medium | Document warning; suggest access control |
| Storage key serialization issues | Medium | Add Debug derive; thorough serialization tests |

---

## Rollout Strategy

### Phase 1 (Week 1)
- Feature branch: `feature/trace-mode-#224`
- PR milestone: M1
- Gate: Coverage ≥ 95%, no warnings

### Phase 2 (Week 2)
- Feature branch: `feature/benchmark-#221`
- PR milestone: M2
- Gate: Baselines established, CI passing

### Phase 3 (Week 3)
- Feature branches: 
  - `feature/parallel-yield-#218` (in-contract)
  - `feature/yield-parallel-lib` (library)
- PR milestones: M3a, M3b
- Gate: Tests passing, documentation clear on WASM limitations

### Phase 4 (Week 4)
- Feature branch: `feature/repl-cli-#220`
- PR milestone: M4
- Gate: Integration tests pass against testnet

### Phase 5 (Week 5)
- Integration branch: `integration/all-features`
- Verify cross-feature compatibility
- Regression analysis

### Phase 6 (Week 6)
- Documentation branch: `docs/feature-guides`
- Final review and approval
- Release tag: `v0.2.0-alpha`

---

## Success Metrics

| Feature | Metric | Target |
|---------|--------|--------|
| #224 Trace Mode | Overhead when OFF | < 1% |
| #221 Benchmarks | Baseline regression detection | ±10% sensitivity |
| #218 Parallel | Speedup on 4-core | 3-4x vs sequential |
| #220 REPL | Command coverage | 15+ commands, all working |
| Overall | Test coverage | ≥ 95% |
| Overall | Zero breaking changes | 100% backward compatible |

---

## Sign-Off Checklist

### Per Feature
- [ ] Design document approved
- [ ] Implementation complete
- [ ] Tests pass (95%+ coverage)
- [ ] No clippy warnings
- [ ] Documentation written
- [ ] PR reviewed and approved

### Final Gate
- [ ] All features integrated
- [ ] Full test suite passes
- [ ] Benchmarks establish baselines
- [ ] No regressions detected
- [ ] Team sign-off

