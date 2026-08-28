# KARIS-KY Escrow Contract: Architecture Analysis & Implementation Plan

**Date:** 2026-07-27  
**Scope:** Four major features (#224, #221, #220, #218)  
**Status:** Design Phase  

## 1. Codebase Overview

### 1.1 Core Contract Structure

**Contract:** `karis_ky_escrow` (Soroban smart contract)
- **Location:** `/workspaces/KARIS-KY/escrow/src/lib.rs` (122KB, ~3000 lines)
- **Language:** Rust (Soroban SDK 25.0)
- **Schema Version:** 6 (current)
- **Entrypoints:** 19+ public methods
- **Storage:** Instance + Persistent keys

### 1.2 Key Data Types

**InvoiceEscrow State Machine** (stored at `DataKey::Escrow`):
```
status: u32
  0 = Open (accepting funding)
  1 = Funded (target met, waiting for settlement)
  2 = Settled (maturity reached, investors can claim)
  3 = Withdrawn (SME pulled liquidity)
  4 = Cancelled (investors may refund)
```

**Core Fields:**
- `invoice_id: Symbol` — immutable invoice identifier
- `admin: Address` — governance/compliance role
- `sme_address: Address` — business entity receiving liquidity
- `amount: i128` — original invoice amount
- `funding_target: i128` — target to trigger settlement readiness
- `funded_amount: i128` — cumulative investor contributions
- `yield_bps: i64` — base yield (basis points, 0-10000)
- `maturity: u64` — Unix timestamp after which settle is allowed

**Storage Keys (DataKey enum):**
- `Escrow` — main contract state
- `Version` — schema version (u32)
- `FundingToken` — SEP-41 token address
- `Treasury` — treasury address for dust sweep
- `RegistryRef` — optional registry hint
- `FundingCloseSnapshot` — immutable pro-rata snapshot when status→1
- `InvestorContribution(Address)` — **persistent** per-investor principal
- `InvestorEffectiveYield(Address)` — **persistent** tiered yield override
- `InvestorClaimNotBefore(Address)` — **persistent** commitment lock timestamp
- `InvestorAllowlisted(Address)` — **persistent** allowlist flag
- `LegalHold` — bool, blocks settle/withdraw/claim/refund
- `YieldTierTable` — optional Vec<YieldTier> (immutable after init)
- `AttestationAppendLog` — Vec<[u8; 32]> (max 32 entries)
- `SmeCollateralCommitment` — metadata-only collateral record

### 1.3 Critical Entrypoints for Feature Integration

| Method | Auth | Storage Impact | Key for Features |
|--------|------|-----------------|------------------|
| `init` | None | Writes Escrow + Version + FundingToken + Treasury | Initialization point |
| `fund` | Investor | Increments InvestorContribution; status→1 trigger | **High iteration cost** |
| `fund_with_commitment` | Investor | Same + sets InvestorEffectiveYield + InvestorClaimNotBefore | Tiered yield entry |
| `settle` | SME (maturity gate) | status→2; triggers investor claim window | Settlement gate |
| `claim_investor_payout` | Investor | Calls `compute_investor_payout` per investor | **Yield bottleneck** |
| `compute_investor_payout` | None (read-only) | Reads: Contribution, EffectiveYield, FundingCloseSnapshot, Escrow | **Parallelizable** |
| `sweep_terminal_dust` | Treasury | Moves dust to treasury (terminal status only) | Post-settlement cleanup |
| `withdraw` | SME | status→3; moves liquidity to SME | Terminal transition |

### 1.4 Yield Calculation Flow (Current)

```
compute_investor_payout(investor):
  1. Get contribution = get_persistent_investor_contribution(investor) [storage read]
  2. If 0, return 0
  3. Get snapshot = FundingCloseSnapshot [storage read]
  4. Get effective_yield_bps = get_persistent_investor_effective_yield(investor)
     OR escrow.yield_bps [storage reads]
  5. coupon = total_principal × effective_yield_bps / 10_000 [arithmetic]
  6. settle_pool = total_principal + coupon
  7. gross_payout = contribution × settle_pool / total_principal [arithmetic]
  8. Return gross_payout

Caller (claim_investor_payout):
  - Calls compute_investor_payout ONCE per investor
  - Currently sequential (no parallelization)
```

**Bottleneck:** Step 1 & 4 are storage I/O. With 1000+ investors, ~2000 storage reads occur sequentially.

### 1.5 Test Structure

```
escrow/src/tests/
  ├─ init.rs           (initialization, validation, getters)
  ├─ funding.rs        (contribution accounting, tier selection)
  ├─ settlement.rs     (settle, withdraw, claim, dust sweep)
  ├─ admin.rs          (legal hold, migration, collateral metadata)
  ├─ integration.rs    (end-to-end flows with external tokens)
  ├─ properties.rs     (proptest-based invariants)
  ├─ attestations.rs   (audit log append)
  ├─ legal_hold.rs     (compliance holds)
  ├─ cap_validation.rs (investor cap enforcement)
  ├─ external_calls.rs (token wrapper safety)
  └─ coverage.rs       (comprehensive coverage + regression tests)

Test Baseline:
  - 95%+ line coverage (enforced in CI)
  - Proptest regressions in proptest-regressions/
  - CI: cargo fmt, clippy, build, test, coverage check
```

### 1.6 Event Emission Pattern

Events are published via `EscrowInitialized`, `InvoiceEscrow`, etc., using Soroban's `#[contractevent]` macro. Example:

```rust
#[contractevent]
pub struct InvestorPayoutClaimed {
    pub investor: Address,
    pub payout: i128,
}
```

---

## 2. Feature Requirements & Integration Points

### 2.1 #224 - Contract Debugger Trace Mode

**Goal:** Emit detailed trace events for each operation (read/write) for forensic analysis.

**Requirements:**
- Log all storage reads and writes with keys, old/new values
- Capture function entry/exit with args, return values
- Track state transitions (status 0→1→2→3)
- Include timestamp, gas cost (if available), error codes
- Bounded capacity (e.g., last 1000 traces)
- Verbosity levels: OFF, ERROR, WARN, INFO, DEBUG, TRACE
- Must not impact production performance when disabled

**Integration Points:**
- Wrap `env.storage().instance().get()` / `.set()` calls
- Intercept entrypoint entry/exit in `#[contractimpl]` block
- Emit trace events via Soroban event system or bounded buffer
- Add feature flag `trace-mode` for compile-time control

**Data Structure:**
```rust
#[contractevent]
pub enum TraceEvent {
    StorageRead { key_type: String, key: String, value: String, ts: u64 },
    StorageWrite { key_type: String, old_value: String, new_value: String, ts: u64 },
    FunctionEnter { fn_name: String, args_hash: [u8; 32], ts: u64 },
    FunctionExit { fn_name: String, result_hash: [u8; 32], ts: u64 },
    StateTransition { from: u32, to: u32, reason: String, ts: u64 },
}
```

---

### 2.2 #221 - Benchmark Suite for Performance Tracking

**Goal:** Benchmark key operations (fund, settle, claim) and track regression over versions.

**Requirements:**
- Measure: fund(), settle(), claim_investor_payout() execution time
- Vary pool sizes: 10, 100, 1000, 5000 investors
- Track memory footprint (storage footprint)
- Compare across versions (baseline vs. optimized)
- Detect regressions (e.g., 10% slowdown triggers alert)
- CI integration: optional benchmark run, publish results

**Structure:**
- Create `escrow/benches/` directory
- Use `criterion` crate for statistical analysis
- Implement generators for realistic escrow states
- Track: wall-time, heap allocations, storage I/O count

**Key Benchmarks:**
1. `bench_fund_single` — 1 investor funding
2. `bench_fund_bulk` — 1000 investors in sequence
3. `bench_settle_ready` — settle call on pre-funded escrow
4. `bench_claim_payout_single` — single investor claim
5. `bench_claim_payout_bulk` — 1000 investors claiming sequentially

---

### 2.3 #218 - Parallel Yield Calculation for Large Pools

**Goal:** Parallelize yield calculation if pool > 1000.

**Requirements:**
- Detect pool size at claim time
- Use rayon crate for work-stealing parallelism if pool > 1000
- Threshold: configurable (default 1000)
- Fallback to sequential if rayon unavailable
- Add feature flag `parallel-yield`
- Benchmark speedup curves (2x, 4x, 8x cores)

**Implementation:**
```rust
pub fn compute_investor_payout_parallel(
    env: Env,
    investors: Vec<Address>,
) -> Result<Vec<i128>, EscrowError> {
    if investors.len() <= PARALLEL_THRESHOLD {
        // Sequential path
        investors.iter().map(|inv| compute_investor_payout(env, inv)).collect()
    } else {
        // Parallel path (via rayon)
        investors.par_iter()
            .map(|inv| compute_investor_payout(env, inv.clone()))
            .collect()
    }
}
```

**Note:** Soroban contracts run in WASM, so true parallelism requires host-side parallelization or async execution. Rayon may not work in WASM context. Alternative: provide parallel helper in off-chain SDK/indexer layer.

---

### 2.4 #220 - Interactive Contract REPL / Debugger CLI Tool

**Goal:** Interactive CLI to call contract methods, inspect state, debug without test code.

**Requirements:**
- Separate CLI binary (e.g., `repl-cli/`)
- Commands: `call <method> <args>`, `get <key>`, `state`, `history`, `breakpoint`, `step`
- Connect to local test env, testnet, mainnet
- Transaction dry-run mode
- State snapshot/restore
- Inspect any DataKey (with proper serialization)

**Structure:**
- New binary target in workspace: `repl-cli`
- Dependencies: `soroban-sdk`, `clap` (CLI), `serde` (JSON serialization)
- Interactive loop with readline support
- Network provider abstraction (local/testnet/mainnet)

---

## 3. Implementation Strategy

### 3.1 Dependency Additions

**For Benchmarks:**
- `criterion = "0.5"` (dev-dependency)

**For Parallel Yield:**
- `rayon = "1.7"` (conditional feature)

**For REPL CLI:**
- `clap = { version = "4.4", features = ["derive"] }`
- `rustyline = "14.0"`
- `serde_json = "1.0"`
- `tokio = { version = "1.0", features = ["full"] }`

**For Trace Mode:**
- No new deps; use built-in Soroban event system

### 3.2 Phased Rollout

**Phase 1: Task #1 - Analysis (CURRENT)**
- Document architecture, data flow, integration points ✓

**Phase 2: Task #2 - Trace Mode**
- Add TraceEvent enum + feature flag
- Wrap key storage calls
- Integration tests with trace collection

**Phase 3: Task #3 - Benchmark Suite**
- Create benches/ directory
- Implement criterion benchmarks
- Generate realistic escrow states for testing
- CI integration

**Phase 4: Task #4 - Parallel Yield**
- Add parallel feature flag + rayon dependency
- Implement `compute_investor_payout_parallel`
- Benchmark speedup curves
- Document WASM limitations

**Phase 5: Task #5 - REPL CLI**
- Create repl-cli/ binary target
- Implement command parser + REPL loop
- Add state inspection + transaction building
- Test against local + testnet environments

**Phase 6: Task #6 - Integration Testing**
- Run full test suite with all features
- Trace mode on real flows
- Benchmark regression detection
- REPL interaction tests

**Phase 7: Task #7 - Documentation**
- Trace mode user guide
- Benchmark suite runbook
- REPL command reference
- Deployment notes

---

## 4. Key Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Trace overhead impacts testnet | High | Feature flag + off-by-default in production |
| Rayon incompatible with WASM | High | Fallback to sequential; move parallelism to indexer |
| Benchmark flakiness on CI | Medium | Use criterion's statistical filters; allow variance |
| REPL security (allows arbitrary reads) | Medium | Documentation warnings; suggest audit log access control |
| Storage key serialization in REPL | Medium | Implement Debug derive for all DataKey variants |

---

## 5. Next Steps

1. ✓ Complete codebase analysis (this document)
2. → Begin Task #2: Trace mode infrastructure
3. → Implement benchmark harness
4. → Add parallel yield (with WASM fallback)
5. → Build REPL CLI tool
6. → Full integration testing
7. → Complete documentation

