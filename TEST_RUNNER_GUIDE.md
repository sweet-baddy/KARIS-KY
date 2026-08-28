# Test Runner Guide

## New Test Modules

Four new test modules have been added to `escrow/src/tests/`:

### 1. Tokenomics Tests (`tokenomics.rs`)
**9 property-based tests** verifying yield distribution across various tokenomics scenarios.

```bash
# Run all tokenomics tests
cargo test --lib tokenomics -- --nocapture

# Run specific test
cargo test --lib tokenomics::prop_single_investor_yield_not_created_or_destroyed

# Run all proptest scenarios (takes longer)
cargo test --lib tokenomics prop_ -- --nocapture
```

**Key tests:**
- `prop_single_investor_yield_not_created_or_destroyed` — Yield conservation
- `prop_equal_contributions_equal_payouts` — Pro-rata invariant
- `prop_sum_of_payouts_bounded_by_settle_pool` — Rounding bounds
- `prop_tiered_yield_increases_investor_return` — Yield tiers
- `prop_zero_yield_equals_principal` — Deflation scenario
- `prop_high_yield_inflation_scenario` — Extreme yield (50% APY)
- `prop_overfunding_snapshot_uses_actual_funded_amount` — Snapshot correctness
- `prop_varying_contributions_maintain_pro_rata_ratio` — Pro-rata with varying amounts
- `test_yield_lifecycle_complete` — End-to-end lifecycle

### 2. Upgrade Compatibility Tests (`upgrade_compat.rs`)
**9 integration tests** verifying schema upgrades v1→v2→v3→v4→v5→v6.

```bash
# Run all upgrade tests
cargo test --lib upgrade_compat -- --nocapture

# Run specific version migration test
cargo test --lib upgrade_compat::test_schema_v1_to_v2_additive_investor_yield_keys

# Run full upgrade matrix
cargo test --lib upgrade_compat::test_full_version_upgrade_matrix
```

**Key tests:**
- `test_schema_v1_to_v2_additive_investor_yield_keys` — v2 backward compat
- `test_schema_v2_to_v3_additive_snapshot_and_caps` — v3 snapshot
- `test_schema_v3_to_v4_additive_attestation_keys` — v4 attestations
- `test_schema_v4_to_v5_tiered_yield_and_registry` — v5 yield tiers
- `test_schema_v5_to_v6_persistent_storage_requires_redeploy` — v6 persistent storage
- `test_migrate_error_codes_are_typed_and_consistent` — Error codes (90/91/92)
- `test_migrate_requires_admin_auth_before_version_checks` — Auth boundary
- `test_full_version_upgrade_matrix` — Complete v1→v6 path
- `test_old_and_new_instances_coexist` — Gradual rollout

### 3. DOS Attack Surface Tests (`dos_analysis.rs`)
**10 runtime bounds enforcement tests** verifying DOS protection.

```bash
# Run all DOS tests
cargo test --lib dos_analysis -- --nocapture

# Run bound verification tests
cargo test --lib dos_analysis test_fund_batch_enforces_size_limit
cargo test --lib dos_analysis test_attestation_append_enforces_log_capacity
```

**Key tests:**
- `test_fund_batch_has_bounded_iteration` — MAX_FUND_BATCH verification
- `test_attestation_append_log_has_bounded_capacity` — MAX_ATTESTATION_APPEND_ENTRIES verification
- `test_dust_sweep_has_bounded_amount` — MAX_DUST_SWEEP_AMOUNT verification
- `test_fund_batch_enforces_size_limit` — Rejects >50 entries
- `test_fund_batch_accepts_max_entries` — Accepts exactly 50
- `test_attestation_append_enforces_log_capacity` — Rejects >32 entries
- `test_allowlist_batch_enforces_size_limit` — Rejects oversized allowlist
- `test_per_investor_storage_cardinality_bounded_by_cap` — Enforces unique investor cap
- `test_per_investor_storage_no_unbounded_enumeration` — No O(n) enumeration
- `test_dust_sweep_enforces_amount_limit` — Amount bound verification

### 4. Secure RNG Audit Tests (`secure_rng.rs`)
**8 tests** auditing RNG usage and secure random patterns.

```bash
# Run all RNG tests
cargo test --lib secure_rng -- --nocapture

# Run PRNG verification
cargo test --lib secure_rng test_soroban_prng

# Run distribution property test
cargo test --lib secure_rng prop_soroban_prng_byte_distribution
```

**Key tests:**
- `test_soroban_prng_available` — PRNG produces output
- `test_soroban_prng_not_reused` — Successive calls are distinct
- `prop_soroban_prng_byte_distribution` — Byte distribution uniformity
- `test_no_timestamp_based_randomness` — No predictable entropy
- `test_no_block_hash_entropy` — No block hash as seed
- `test_example_secure_rng_usage` — Correct usage patterns
- `test_commit_reveal_pattern_for_randomness` — High-stakes pattern
- `test_rng_audit_summary` — Audit documentation

---

## Running All New Tests

### Complete test suite for all four modules:

```bash
# Run all new tests
cargo test --lib tokenomics upgrade_compat dos_analysis secure_rng -- --nocapture

# Or run individually
cargo test --lib tokenomics
cargo test --lib upgrade_compat
cargo test --lib dos_analysis
cargo test --lib secure_rng
```

### Coverage-aware testing:

```bash
# Run with coverage (requires cargo-llvm-cov)
cargo install cargo-llvm-cov

# Test with coverage report
cargo llvm-cov \
  --features testutils \
  --fail-under-lines 95 \
  -- --test-threads=1

# Generate HTML report
cargo llvm-cov \
  --features testutils \
  --html \
  -- --test-threads=1
```

### Performance testing (proptest scenarios):

```bash
# Run proptests with more iterations (slower but more thorough)
PROPTEST_CASES=10000 cargo test --lib tokenomics prop_

# Run with specific seed for reproducibility
PROPTEST_RNG_SEED=12345 cargo test --lib tokenomics prop_
```

---

## CI Integration

The new tests are automatically run as part of the standard CI pipeline:

```bash
# This is what CI runs (from .github/workflows/ci.yml)
cargo fmt -p karis-ky_escrow -- --check
cargo clippy -p karis-ky_escrow -- -D warnings
cargo build
cargo test                                          # ← Runs all tests including new ones
cargo llvm-cov --features testutils --fail-under-lines 95
```

---

## Test Verification Checklist

Before committing test changes:

```bash
# 1. Format check
cargo fmt -p karis-ky_escrow -- --check

# 2. Lint check
cargo clippy -p karis-ky_escrow -- -D warnings

# 3. Build
cargo build

# 4. Run all tests
cargo test

# 5. Verify coverage
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only
```

---

## Test Maintenance

### Adding new tokenomics scenarios:
1. Add a new `prop_*` function in `tokenomics.rs`
2. Use `proptest!` macro with strategies from `gen_*` functions
3. Verify the invariant holds across the generated inputs
4. Document the property being tested in comments

### Adding upgrade compatibility tests:
1. Add a new schema version test in `upgrade_compat.rs`
2. Follow the pattern: init with old features, verify new features readable
3. Test both backward compat and error conditions
4. Update `test_full_version_upgrade_matrix` if new major version added

### Adding DOS tests:
1. Add bound constant verification tests
2. Test that oversized inputs are rejected
3. Test that exactly-at-limit inputs are accepted
4. Document cost analysis in module header

### Adding RNG tests:
1. Document the RNG use case and pattern
2. Add tests verifying the pattern is used correctly
3. Exclude any timestamp/block-hash entropy
4. Update `test_rng_audit_summary` with new findings

---

## Troubleshooting

### Test timeout
```bash
# Increase timeout for long-running proptests
cargo test --lib tokenomics -- --test-threads=1 --nocapture
```

### Out of memory
```bash
# Reduce proptest iterations
PROPTEST_CASES=100 cargo test --lib tokenomics prop_
```

### Flaky proptest
```bash
# Use deterministic seed
PROPTEST_RNG_SEED=12345 cargo test --lib tokenomics prop_
```

### Coverage not meeting threshold
```bash
# See which lines are uncovered
cargo llvm-cov --features testutils --html

# Open target/llvm-cov/html/index.html
```

---

## References

- **Module registration**: `escrow/src/tests.rs` (search for `mod tokenomics`, `mod upgrade_compat`, etc.)
- **Test templates**: Each module starts with documentation comments explaining the test purpose
- **Acceptance criteria**: See `IMPLEMENTATION_SUMMARY.md` for full requirements and verification
- **CI pipeline**: `.github/workflows/ci.yml`
