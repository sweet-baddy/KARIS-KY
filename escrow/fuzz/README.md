# Escrow Contract Fuzzing

This directory contains fuzzing harnesses for the karis-ky escrow contract, using [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) (libfuzzer) to generate random operation sequences and verify contract invariants.

## Overview

Fuzzing generates random input data and feeds it to the contract under test, automatically exploring edge cases and finding violations of stated invariants. This catches issues that deterministic unit tests might miss.

## Setup

### Install cargo-fuzz

```bash
cargo install cargo-fuzz
```

### Initialize the fuzz harness (if not already set up)

```bash
cd escrow
cargo fuzz init
```

## Fuzz Targets

### 1. escrow_funding_operations

**Location:** `fuzz/fuzz_targets/escrow_funding_operations.rs`

**Purpose:** Generate random funding sequences and verify funding invariants.

**Invariants Tested:**
- ✅ Funded amount never exceeds funding target during open status
- ✅ Funded amount never exceeds invoice amount
- ✅ Yield bps always in valid range [0, 10000]
- ✅ Status advances correctly when threshold is reached
- ✅ Structural properties immutable (amounts, addresses)
- ✅ Status always in valid range [0, 4]

**Run:**
```bash
cd escrow
cargo +nightly fuzz run escrow_funding_operations
```

**Example findings:**
- Detects integer overflow scenarios
- Finds edge cases in status transitions
- Validates yield range enforcement

### 2. escrow_settlement_flow

**Location:** `fuzz/fuzz_targets/escrow_settlement_flow.rs`

**Purpose:** Generate random settlement sequences respecting time locks.

**Invariants Tested:**
- ✅ Funding snapshot exists once status is funded
- ✅ Status is 2 (settled) after settlement
- ✅ Escrow data immutable post-settlement
- ✅ Cannot re-settle an already settled escrow
- ✅ Status is monotonically increasing (forward-only)
- ✅ Maturity time lock respected

**Run:**
```bash
cd escrow
cargo +nightly fuzz run escrow_settlement_flow
```

**Example findings:**
- Detects time-lock bypass attempts
- Validates snapshot immutability
- Finds double-settlement issues

### 3. escrow_yield_calculations

**Location:** `fuzz/fuzz_targets/escrow_yield_calculations.rs`

**Purpose:** Generate random investor contributions and yield calculations.

**Invariants Tested:**
- ✅ Yield bps in valid range [0, 10000]
- ✅ Snapshot total principal >= funded amount
- ✅ Effective yield <= base yield (no unexpected enhancements)
- ✅ Effective yield always in valid range
- ✅ Claim lock timestamps are in past or at settlement
- ✅ Yield unchanged during execution
- ✅ Final status >= 2 (settled or beyond)
- ✅ Amounts immutable post-settlement

**Run:**
```bash
cd escrow
cargo +nightly fuzz run escrow_yield_calculations
```

**Example findings:**
- Detects yield calculation overflows
- Validates pro-rata share calculations
- Finds claim-lock bypass attempts

## Running All Fuzz Targets

```bash
cd escrow

# Run all fuzz targets with default (infinite) time limit
cargo +nightly fuzz run --all

# Run a specific target for 60 seconds
cargo +nightly fuzz run escrow_funding_operations -- -max_len=10000 -timeout=60

# Run with a custom seed corpus
cargo +nightly fuzz run escrow_funding_operations -- corpus/

# Run with specific verbosity
cargo +nightly fuzz run escrow_funding_operations -- -verbosity=2
```

## Corpus

Corpus files are stored in `fuzz/corpus/<target_name>/`. These are the inputs that triggered interesting behavior or bugs. They are automatically saved and replayed for regression testing.

```bash
# View corpus files
ls -la fuzz/corpus/escrow_funding_operations/

# Manually test a specific corpus input
cargo +nightly fuzz run escrow_funding_operations -- fuzz/corpus/escrow_funding_operations/interesting_input
```

## Reproducing Failures

When fuzzing finds a failure, it saves the input to a crash file. To reproduce:

```bash
# The crash file is typically in fuzz/artifacts/<target_name>/
ls fuzz/artifacts/escrow_funding_operations/

# Run with the crashing input to reproduce
cargo +nightly fuzz run escrow_funding_operations -- fuzz/artifacts/escrow_funding_operations/crash-abc123
```

## Configuration

Key parameters in each fuzz target (via libfuzzer command-line):

| Flag | Purpose | Default |
|------|---------|---------|
| `-max_len=N` | Maximum input size in bytes | ~16MB |
| `-timeout=N` | Timeout per input in seconds | No limit |
| `-max_total_time=N` | Total fuzzing time in seconds | No limit |
| `-jobs=N` | Parallel job count | 1 |
| `-workers=N` | Number of worker processes (with `-jobs`) | CPU count |
| `-verbosity=N` | Verbosity level (0-2) | 1 |

Example: Run for 5 minutes across 4 workers, max 10KB inputs:

```bash
cargo +nightly fuzz run escrow_funding_operations -- \
  -max_len=10240 \
  -max_total_time=300 \
  -jobs=4 \
  -workers=4
```

## Expected Output

Running a fuzz target produces output like:

```
#0      READ units: 0
#1024   READ units: 1
#2048   READ units: 2
...
#1000000 READ units: 5  L: 256/512  MS: 4 ShuffleBytes-
[...]
  rss: 123 MB  |  time: 45s  |  now: 2000000 units, 44K/s
```

Key columns:
- `#N` — execution count
- `READ units` — corpus size
- `L` — input length / max length
- `MS` — mutation strategy applied
- `rss` — resident memory
- `time` — elapsed time
- `units/s` — fuzzing speed

A successful run will continue indefinitely until you stop it (`Ctrl+C`), finding new code paths and corpus entries.

## Invariant Violations

When an invariant is violated, the fuzzer will panic with a clear message:

```
INVARIANT VIOLATION: funded_amount (999999999) > funding_target (500000000) in open status
```

The reproducer input is saved to `fuzz/artifacts/<target_name>/crash-*`, allowing deterministic reproduction.

## CI Integration

The fuzzing harnesses can be integrated into CI by running with short time limits:

```bash
# In CI: fuzz for 30 seconds each
for target in escrow_funding_operations escrow_settlement_flow escrow_yield_calculations; do
  cargo +nightly fuzz run $target -- -max_total_time=30 || exit 1
done
```

## Best Practices

1. **Start simple:** Begin with broad input ranges, then narrow after understanding behavior
2. **Assert early:** Invariant checks should fail fast and provide context
3. **Corpus maintenance:** Keep interesting inputs for regression testing
4. **Resource limits:** Set `-max_len` and `-timeout` to prevent runaway processes
5. **Parallelization:** Use `-jobs=N` for faster exploration on multi-core systems
6. **Incremental runs:** Run regularly to build corpus and catch new edge cases

## Troubleshooting

### "Found an interesting crash"

The fuzzer discovered an input that triggers undefined behavior or assertion failure. Reproduce with:

```bash
cargo +nightly fuzz run <target> -- fuzz/artifacts/<target>/crash-*
```

### "Exited with code 77" (timeout)

An input triggers very slow execution. Reduce `-max_len` or increase `-timeout` as needed.

### Memory issues

The fuzzer is consuming too much memory. Reduce `-max_len` or run single-threaded (`-jobs=1`).

### "No libfuzzer"

Missing nightly Rust toolchain:

```bash
rustup install nightly
```

## References

- [cargo-fuzz Book](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [Libfuzzer Documentation](https://llvm.org/docs/LibFuzzer/)
- [Arbitrary Crate](https://docs.rs/arbitrary/)

## See Also

- `../README.md` — Escrow contract overview
- `../docs/escrow-security-checklist.md` — Security considerations
- `../docs/escrow-data-model.md` — Data model invariants
