# Escrow Contract Fuzzing Guide

## Quick Start

### Prerequisites

Ensure you have the Rust nightly toolchain and cargo-fuzz installed:

```bash
rustup install nightly
cargo install cargo-fuzz
```

### Running the Fuzz Targets

From the `escrow/` directory:

```bash
# Run a single fuzz target indefinitely
cd escrow
cargo +nightly fuzz run escrow_funding_operations

# Run all fuzz targets (each indefinitely)
cargo +nightly fuzz run --all

# Run with a time limit (60 seconds)
cargo +nightly fuzz run escrow_funding_operations -- -max_total_time=60

# Run with multiple workers (faster exploration)
cargo +nightly fuzz run escrow_funding_operations -- -jobs=4 -workers=4

# Run with input size limit and timeout
cargo +nightly fuzz run escrow_funding_operations -- -max_len=10240 -timeout=5
```

## Fuzz Targets Overview

### Coverage mapping

The table below maps each fuzz target to the source code regions it exercises,
the specific invariants it guards, and the escrow test module that provides
the closest deterministic counterpart.

| Target | Primary source region | Invariants guarded | Deterministic counterpart |
|--------|-----------------------|--------------------|---------------------------|
| `escrow_funding_operations` | `fund_impl`, `simple_fund`, `UniqueFunderCount` logic | Funded ≤ target, status gate, yield bps range, funder count monotonicity, immutable fields | `escrow/src/tests/funding.rs` |
| `escrow_settlement_flow` | `settle`, `FundingCloseSnapshot` write, maturity gate | Snapshot exists post-fund, status forward-only, data immutable post-settle, no re-settle, status monotonic | `escrow/src/tests/settlement.rs` |
| `escrow_yield_calculations` | `claim_investor_payout`, `compute_investor_payout`, `get_investor_yield_bps`, `InvestorClaimNotBefore` | Yield bps range, snapshot ≥ funded, claim lock in past, no double-claim, amounts immutable post-settle | `escrow/src/tests/properties.rs`, `escrow/src/tests/tokenomics.rs` |

### Code-level invariant coverage

#### `escrow_funding_operations` — 6 invariants

| # | Invariant | Contract code path | Error triggered on violation |
|---|-----------|--------------------|------------------------------|
| 1 | `funded_amount <= funding_target` when `status == 0` | `fund_impl` status check before `checked_add` | `INVARIANT VIOLATION` panic |
| 2 | `funded_amount <= amount` always | `fund_impl` amount cap | `INVARIANT VIOLATION` panic |
| 3 | `yield_bps ∈ [0, 10 000]` | `init` `yield_bps` range check | `INVARIANT VIOLATION` panic |
| 4 | `status == 1` iff `funded_amount >= funding_target` | `fund_impl` status transition | `INVARIANT VIOLATION` panic |
| 5 | `funded_amount ≤ invoice_amount` (final state) | Final escrow read | `INVARIANT VIOLATION` panic |
| 6 | `amount`, `admin`, `sme_address` immutable; `status ∈ [0, 4]` | `init` write-once fields | `INVARIANT VIOLATION` panic |

#### `escrow_settlement_flow` — 5 invariants

| # | Invariant | Contract code path | Error triggered on violation |
|---|-----------|--------------------|------------------------------|
| 1 | `FundingCloseSnapshot` exists once `status == 1` | First fund reaching target triggers snapshot write | `INVARIANT VIOLATION` panic |
| 2 | `status == 2` after successful `settle` | `settle` status write | `INVARIANT VIOLATION` panic |
| 3 | `amount`, `funded_amount`, `yield_bps` immutable post-settle | `settle` does not modify these fields | `INVARIANT VIOLATION` panic |
| 4 | Second `settle` call does not change `status` | `settle` `status == 1` guard | `INVARIANT VIOLATION` panic |
| 5 | Status never decreases during execution | Monotonicity check on every path | `INVARIANT VIOLATION` panic |

#### `escrow_yield_calculations` — 9 invariants

| # | Invariant | Contract code path | Error triggered on violation |
|---|-----------|--------------------|------------------------------|
| 1 | `yield_bps ∈ [0, 10 000]` | `init` validation; checked before contract call | `INVARIANT VIOLATION` panic |
| 2 | `snapshot.total_principal >= funded_amount` | `FundingCloseSnapshot` write includes overfunding | `INVARIANT VIOLATION` panic |
| 3 | `effective_yield ≤ base yield` for base funders | `get_investor_yield_bps` fallback to base | `INVARIANT VIOLATION` panic |
| 4 | `effective_yield ∈ [0, 10 000]` per investor | `InvestorEffectiveYield` range | `INVARIANT VIOLATION` panic |
| 5 | `claim_not_before ≤ current_time` at settlement | `InvestorClaimNotBefore` set to past/now on base fund | `INVARIANT VIOLATION` panic |
| 6 | Double-claim is idempotent (status unchanged) | `InvestorClaimed` idempotency guard | `INVARIANT VIOLATION` panic |
| 7 | `yield_bps` unchanged across execution | No entrypoint modifies `yield_bps` post-init | `INVARIANT VIOLATION` panic |
| 8 | Final `status >= 2` after successful settle | `settle` status transition | `INVARIANT VIOLATION` panic |
| 9 | `amount` and `funded_amount` unchanged post-settle | Immutability of funded state | `INVARIANT VIOLATION` panic |

---

### 1. **escrow_funding_operations**

Tests funding invariants with random investor deposits.

**Invariants:**
- Funded amount ≤ funding target (in open status)
- Funded amount ≤ invoice amount
- Yield bps ∈ [0, 10000]
- Status advances when funded ≥ target
- Immutable fields remain unchanged

**Typical inputs:**
- Invoice amounts (1..1 billion)
- Funding targets
- Multiple investor deposits
- Yield rates

**Quick run:**
```bash
cargo +nightly fuzz run escrow_funding_operations -- -max_total_time=30
```

**Expected result:** ✅ All invariants pass (no crashes)

---

### 2. **escrow_settlement_flow**

Tests settlement operations and time-lock enforcement.

**Invariants:**
- Snapshot exists post-funding
- Status = 2 after settlement
- Escrow data immutable post-settlement
- Cannot re-settle
- Status monotonically increasing

**Typical inputs:**
- Maturity offsets
- Time advancement steps
- Early settlement attempts
- Multiple investor funds

**Quick run:**
```bash
cargo +nightly fuzz run escrow_settlement_flow -- -max_total_time=30
```

**Expected result:** ✅ All invariants pass, time-lock respected

---

### 3. **escrow_yield_calculations**

Tests yield computation and investor payouts.

**Invariants:**
- Yield bps ∈ [0, 10000]
- Snapshot total ≥ funded amount
- Effective yield ≤ base yield
- Claim locks in past/at settlement
- Yield immutable post-settlement
- No double-claims

**Typical inputs:**
- Base yields (0..10000 bps)
- Investor contributions
- Settlement claims
- Multiple investors

**Quick run:**
```bash
cargo +nightly fuzz run escrow_yield_calculations -- -max_total_time=30
```

**Expected result:** ✅ All invariants pass, yields valid

---

## Corpus Files

The fuzzer builds a corpus of interesting inputs in `fuzz/corpus/`:

```bash
# View corpus for a target
ls -la escrow/fuzz/corpus/escrow_funding_operations/

# Corpus automatically replayed on each run (regression testing)
# To manually add a corpus entry:
cp my_interesting_input escrow/fuzz/corpus/escrow_funding_operations/
```

---

## Known Crash Corpus and Edge Cases

### Corpus structure

```
escrow/
└── fuzz/
    ├── corpus/
    │   ├── escrow_funding_operations/
    │   │   └── <sha256-named inputs>   ← interesting paths found during fuzzing
    │   ├── escrow_settlement_flow/
    │   │   └── <sha256-named inputs>
    │   └── escrow_yield_calculations/
    │       └── <sha256-named inputs>
    └── artifacts/
        ├── escrow_funding_operations/
        │   └── crash-<sha256>          ← inputs that triggered invariant violations
        ├── escrow_settlement_flow/
        │   └── crash-<sha256>
        └── escrow_yield_calculations/
            └── crash-<sha256>
```

Corpus entries are **deterministic regression seeds** — they are replayed
automatically on every subsequent fuzzing run to guard against regressions. If
a fix removes an invariant violation, the crash artifact can be promoted to a
corpus seed to ensure the fixed edge case is always exercised.

### Seeding the corpus for faster coverage

The fuzz targets accept structured `Arbitrary` input, so hand-crafted seeds
can steer the fuzzer toward known interesting regions immediately. Add seeds
to the appropriate `fuzz/corpus/<target>/` directory before running.

#### Recommended seed values — `escrow_funding_operations`

| Seed intent | `invoice_amount` | `funding_target` | `yield_bps` | `investor_amounts` | Why |
|-------------|-----------------|-----------------|------------|-------------------|-----|
| Exact-target single-investor | `1_000_000` | `1_000_000` | `0` | `[1_000_000]` | Boundary: funded == target on first deposit |
| Over-fund (target < amount) | `1_000_000_000` | `500_000_000` | `10_000` | `[600_000_000]` | Tests overfunding cap at max yield |
| Zero yield | `10_000_000` | `10_000_000` | `0` | `[5_000_000, 5_000_000]` | Floor yield path |
| Max unique investors (10) | `100_000_000` | `100_000_000` | `800` | `[10_000_000] × 10` | Funder count boundary |
| Minimum amounts | `1` | `1` | `1` | `[1]` | 1-stroop edge case |
| Invoice amount = 1 | `1` | `1` | `0` | `[1, 1]` | Second deposit after funded; must be rejected |

#### Recommended seed values — `escrow_settlement_flow`

| Seed intent | `funding_target` | `maturity_offset` | `advance_time_steps` | Why |
|-------------|-----------------|------------------|----------------------|-----|
| Settle at exact maturity | `1_000_000` | `3_600` | `1` | One 1h step lands exactly at maturity |
| Early settle attempt | `1_000_000` | `7_200` | `0` | Zero time advance; maturity not reached |
| Long maturity (far future) | `500_000_000` | `31_536_000` | `255` | Max advance still before maturity (255 × 3600 < 1y) |
| Minimum target, one investor | `1` | `1` | `1` | 1-stroop boundary |

#### Recommended seed values — `escrow_yield_calculations`

| Seed intent | `yield_bps` | `invoice_amount` | `investor_contributions` | Why |
|-------------|------------|-----------------|-------------------------|-----|
| Max yield, single investor | `10_000` | `10_000_000_000` | `[8_000_000_000]` | 100% yield on near-full funding |
| Zero yield | `0` | `1_000_000_000` | `[800_000_000]` | No coupon; pro-rata principal only |
| Many investors (max 10) | `500` | `100_000_000` | `[10_000_000] × 10` | All 10 investors claim simultaneously |
| One-stroop contribution | `800` | `1_000_000` | `[1]` | Minimum contribution; rounding edge |
| Overfund past 80% target | `800` | `10_000_000_000` | `[9_000_000_000]` | Snapshot total > funded_amount |

### Known edge cases identified during development

The following edge cases were found during property-based testing and have
corresponding deterministic regression tests. Add them as corpus seeds if
the fuzz target has not already converged on them:

| Edge case | Relevant fuzz target | Deterministic test |
|-----------|---------------------|-------------------|
| `funded_amount` overflow via `checked_add` on max `i128` contributions | `escrow_funding_operations` | `escrow/src/tests/funding.rs` — overflow guards |
| Settlement called before any investors fund (status never reaches 1) | `escrow_settlement_flow` | `escrow/src/tests/settlement.rs::settle_not_funded_panics` |
| `claim_not_before` in the future when `committed_lock_secs > 0` | `escrow_yield_calculations` | `escrow/src/tests/settlement.rs` — claim lock tests |
| Yield snapshot total == funded amount (no overfunding, exact target) | `escrow_yield_calculations` | `escrow/src/tests/properties.rs::prop_status_settle_transition` |
| `settle` with `maturity == 0` (no time gate) | `escrow_settlement_flow` | `escrow/src/tests/settlement.rs` — zero maturity |
| `status` field unchanged after failed settle (maturity not reached) | `escrow_settlement_flow` | `escrow/src/tests/settlement.rs` — maturity boundary |
| `funded_amount = 0` at init; first fund transitions to status 1 immediately if target = 0 | `escrow_funding_operations` | `escrow/src/tests/init.rs` |

### Corpus minimization

After extended fuzzing runs the corpus may accumulate redundant entries that
exercise the same code paths. Minimize periodically to keep replay time short:

```bash
# Minimize corpus — keeps only entries that contribute unique coverage
cd escrow
cargo +nightly fuzz cmin escrow_funding_operations \
  fuzz/corpus/escrow_funding_operations/ \
  /tmp/minimized_corpus/

# Replace with minimized set
rm -rf fuzz/corpus/escrow_funding_operations/
mv /tmp/minimized_corpus/ fuzz/corpus/escrow_funding_operations/
```

Run the same for each target. Commit the minimized corpus so CI replay stays
fast.

## Reproducing Crashes

If fuzzing finds a crash:

```bash
# List crash artifacts
ls escrow/fuzz/artifacts/escrow_funding_operations/

# Reproduce the crash
cargo +nightly fuzz run escrow_funding_operations -- \
  escrow/fuzz/artifacts/escrow_funding_operations/crash-abc123
```

The crash input will be printed with the invariant violation message, enabling debugging.

## Advanced Fuzzing

### Parallel Fuzzing (Faster)

```bash
# Use 4 workers in parallel for faster coverage
cargo +nightly fuzz run escrow_funding_operations -- \
  -jobs=4 -workers=4 -max_total_time=300
```

### Custom Input Sizes

```bash
# Limit to 5KB inputs (faster execution)
cargo +nightly fuzz run escrow_funding_operations -- -max_len=5120

# Allow larger inputs (slower but better coverage)
cargo +nightly fuzz run escrow_funding_operations -- -max_len=50000
```

### Merge Corpus Files

```bash
# Combine corpus from multiple runs
cargo +nightly fuzz cmin escrow_funding_operations \
  fuzz/corpus/escrow_funding_operations/ \
  /path/to/other/corpus/
```

## Integration with CI

Add fuzzing to your CI pipeline:

```yaml
# .github/workflows/fuzz.yml
name: Fuzz
on: [pull_request, push]

jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz

      - name: Fuzz escrow_funding_operations
        run: cd escrow && cargo +nightly fuzz run escrow_funding_operations -- -max_total_time=60

      - name: Fuzz escrow_settlement_flow
        run: cd escrow && cargo +nightly fuzz run escrow_settlement_flow -- -max_total_time=60

      - name: Fuzz escrow_yield_calculations
        run: cd escrow && cargo +nightly fuzz run escrow_yield_calculations -- -max_total_time=60
```

## Interpreting Fuzz Output

```
#0      READ units: 0
#1024   READ units: 1  L: 32/1024 MS: 4 ShuffleBytes-
#2048   READ units: 2  L: 64/1024 MS: 3 CopyPart-
...
#1000000 READ units: 15 L: 256/1024 MS: 5 MutationDispatch-
```

Key fields:
- `#N` — Number of iterations
- `READ units` — Corpus size
- `L: A/B` — Input length / max length
- `MS` — Mutation strategy applied
- `rss` — Memory usage
- `time` — Elapsed time
- `units/s` — Iterations per second

## Debugging a Failure

When an invariant fails:

```
thread 'main' panicked at 'INVARIANT VIOLATION: funded_amount (999999999) > funding_target (500000000) in open status'
```

1. **Note the input:** Fuzzer saves to `fuzz/artifacts/escrow_funding_operations/crash-*`
2. **Reproduce locally:**
   ```bash
   cargo +nightly fuzz run escrow_funding_operations -- \
     escrow/fuzz/artifacts/escrow_funding_operations/crash-abc123
   ```
3. **Add test case:** Extract the input and add to `escrow/src/tests/` for permanent regression testing
4. **Fix the bug:** Modify the contract to respect the invariant
5. **Verify:** Re-run the fuzz target to confirm fix

## Performance Tuning

| Issue | Solution |
|-------|----------|
| Too slow | Reduce `-max_len`, use `-jobs=4` |
| Too much memory | Reduce `-max_len`, limit corpus size |
| Not finding bugs | Increase `-max_total_time`, add corpus seeds |
| Corpus too large | Use `cargo +nightly fuzz cmin` to minimize |

## Expected Runtime

On a modern laptop:

| Target | 30 seconds | Coverage |
|--------|------------|----------|
| escrow_funding_operations | ~5-10K iterations | Good |
| escrow_settlement_flow | ~2-5K iterations | Good |
| escrow_yield_calculations | ~3-7K iterations | Good |

Longer runs (hours) on continuous integration servers find more edge cases.

## Troubleshooting

### "Found an interesting crash"

This means an invariant violation was found. See "Reproducing Crashes" above.

### "Compiler errors"

Ensure Soroban SDK and dependencies are compatible:

```bash
cd escrow
cargo update
cargo +nightly fuzz run escrow_funding_operations
```

### "Timeout (exit code 77)"

An input takes too long. Either:

1. Reduce input size: `cargo +nightly fuzz run escrow_funding_operations -- -max_len=5120`
2. Increase timeout: `cargo +nightly fuzz run escrow_funding_operations -- -timeout=10`
3. Optimize contract code

### "Out of memory"

The fuzzer is using too much memory:

```bash
# Reduce input size and run single-threaded
cargo +nightly fuzz run escrow_funding_operations -- -max_len=5120 -jobs=1
```

## Files

```
escrow/
├── fuzz/
│   ├── Cargo.toml                          # Fuzz harness crate config
│   ├── README.md                           # Detailed fuzz documentation
│   ├── fuzz_targets/
│   │   ├── escrow_funding_operations.rs    # Funding invariant tests
│   │   ├── escrow_settlement_flow.rs       # Settlement flow tests
│   │   └── escrow_yield_calculations.rs    # Yield calculation tests
│   ├── corpus/                              # Interesting inputs (auto-populated)
│   │   ├── escrow_funding_operations/
│   │   ├── escrow_settlement_flow/
│   │   └── escrow_yield_calculations/
│   ├── artifacts/                           # Crashes (auto-populated)
│   │   ├── escrow_funding_operations/
│   │   ├── escrow_settlement_flow/
│   │   └── escrow_yield_calculations/
│   └── .gitignore
```

## See Also

- `escrow/fuzz/README.md` — Detailed fuzz harness documentation
- `escrow/src/lib.rs` — Contract implementation
- `docs/adr/` — Architecture decision records
