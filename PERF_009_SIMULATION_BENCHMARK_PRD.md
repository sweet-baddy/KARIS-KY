# PRD: PERF-009 — `simulation.rs` Cost Baseline for Settle + Claim Cycle (20 Investors)

**Issue ID:** PERF-009
**Category:** Performance / Observability
**Status:** Specified — ready for assignment
**Author:** Escrow maintainers (spec expanded from backlog stub)
**Related:** [`escrow/src/tests/health_and_events.rs:299`](escrow/src/tests/health_and_events.rs) (`test_bucketing_cost_baseline_many_investors`, the precedent this PRD follows), [`FEATURE_221_BENCHMARK_SUITE_DESIGN.md`](FEATURE_221_BENCHMARK_SUITE_DESIGN.md) (broader, not-yet-implemented `criterion`-based benchmark suite — out of scope here, see "Relationship to Feature #221" below)

---

## 1. Problem Statement

`escrow/tests/simulation.rs` exercises the **dry-run** entrypoints (`simulate_fund`, `simulate_settle`) for correctness — no state mutation, idempotence, no-auth-required. It does not measure **resource cost** of the real state-mutating lifecycle.

There is currently no CI signal for the cost of the most common real-money path: N investors fund an escrow, it settles, and each investor claims their payout. A regression here (e.g., an accidental per-investor loop added inside `settle`, or a shared list that every `claim_investor_payout` call now scans) would not be caught by any existing test — all current cost-baseline tests (`test_cost_baseline_init*`, `test_cost_baseline_fund_*`, `test_cost_baseline_settle`, `test_bucketing_cost_baseline_many_investors`) measure `init`/`fund` individually, never the `settle` → `claim × N` sequence together.

This PRD defines exactly one new test. It does not propose new production code, new infrastructure, or a general benchmarking framework.

## 2. Goals

- Establish a CI-enforced upper-bound cost baseline (CPU instructions, and memory bytes if cheaply available) for:
  1. `settle(None)` alone, on a fully-funded 20-investor escrow.
  2. The full cycle of 20 sequential `claim_investor_payout` calls after settlement.
  3. The derived average cost per claim (phase-2 delta / 20).
- Fail CI loudly, with the measured value printed in the assertion message, if either phase regresses past its bound — so a future PR that accidentally makes `settle` or `claim_investor_payout` scale with investor count is caught before merge.
- Keep the test fast (sub-second) and deterministic (no wall-clock timing, no flakiness).

## 3. Non-Goals

- **Not** building a `criterion`-based `escrow/benches/` suite, wall-clock timing, or historical trend tracking/HTML reports. That is the larger, separately-scoped effort in `FEATURE_221_BENCHMARK_SUITE_DESIGN.md`, which is a design document only — `escrow/benches/` does not exist yet in this repo. This PRD does not depend on it and should not be blocked by it.
- **Not** fixing any performance problem — no baseline exists yet, so there is nothing confirmed to fix. If this work uncovers an actual regression or an unexpectedly expensive path, that becomes a separate follow-up ticket, not part of this one.
- **Not** covering `fund_batch`, `fund_with_commitment` tiered-yield claim paths, delegate claims (`claim_investor_payout_as_delegate`), or partial settlement (`settle(Some(partial_amount))`). Single-token, non-tiered, full-settlement path only, matching the issue title exactly ("settle + claim cycle for 20 investors").
- **Not** parameterizing investor count as a matrix (5/20/100/1000). One fixed size (20), per the issue title. A follow-up can extend to a matrix if this baseline proves useful.

## 4. Background: Why 20 Investors, and Why This Pattern

- **Investor count (20):** Specified directly by the issue title. It's a representative small-to-mid funding round: large enough to expose linear-vs-quadratic scaling in the claim loop (a quadratic bug is invisible at N=1), small enough to keep the test fast and not confusable with a stress/load test. It stays well under `MAX_FUND_BATCH = 50` (`escrow/src/lib.rs:178`) and far under `MAX_INVESTOR_HISTORY_ENTRIES = 128` (`lib.rs:161`), so the test measures the steady-state cost path, not a boundary/cap condition.
- **Measurement mechanism:** `Env::budget()` deltas (`cpu_insns()`), the same mechanism already established in `test_bucketing_cost_baseline_many_investors` (`escrow/src/tests/health_and_events.rs:328-347`). No new tooling or dependency is introduced.
- **Loose bounds, not exact-value assertions:** Per the existing convention (`health_and_events.rs:338-341`: *"This is a cost-baseline test, not a hard assertion; it documents the resource profile for future optimization comparisons."*) — assert generous upper bounds so the test is a regression tripwire, not something that breaks on every minor const-cost drift from an unrelated `soroban-sdk` version bump.

## 5. Detailed Requirements

### 5.1 Test placement

Add the new test as `test_cost_baseline_settle_and_claim_cycle_20_investors`.

Two placement options, in preference order:

1. **Preferred:** `escrow/src/tests/health_and_events.rs`, adjacent to `test_bucketing_cost_baseline_many_investors`, since that file already has `env.budget()` access and the unit-test harness (`#![cfg_attr(not(test), no_std)]` + `#[cfg(test)] extern crate std` at `escrow/src/lib.rs:1,113-114`) that `env.budget()` needs.
2. **Alternative:** `escrow/tests/simulation.rs`, if the team prefers colocating with the existing simulation-entrypoint tests for discoverability. This is an external integration-test binary (no `#[cfg(test)] extern crate std` guard needed there — it's already a separate `std`-enabled crate), so `env.budget()` is available the same way. If chosen, keep the existing `deploy`/`setup`/`free_addresses`/`default_init` helpers in that file, but note `default_init` (`simulation.rs:35-54`) must be checked against the current `init` signature (`lib.rs:1925-1946`) before reuse — the two have drifted in argument count in the past as `init` gained optional parameters, and a stale helper will fail to compile rather than fail at runtime.

Whichever file is chosen, this is the only new test — do not scaffold a new test file for one test.

### 5.2 Setup

- Fresh `Env`, `env.mock_all_auths()`.
- `init` an escrow with:
  - `amount` (funding target) evenly divisible by 20, e.g. `2_000_000_000i128` → `100_000_000` per investor.
  - `maturity` set so that after funding, advancing the ledger timestamp makes `settle` eligible (mirror the pattern in `escrow/src/tests/settlement.rs:616` `test_cost_baseline_settle` for the exact maturity/timestamp handling `settle` requires).
  - All other `init` parameters `None`/defaults — no yield tiers, no allowlist, no legal hold, no KYC gate. This isolates the measurement to the plain settle/claim path per the non-goals above.
- Fund with exactly 20 distinct `Address::generate(&env)` investors, equal contributions, using a loop matching `health_and_events.rs:331-334`.
- Advance `env.ledger()` timestamp/sequence past `maturity` (do this *before* starting the phase-1 budget snapshot — funding and time advancement are setup cost, not measured cost).

### 5.3 Measurement — Phase 1: `settle`

```rust
let budget_before = env.budget();
client.settle(&None);
let budget_after = env.budget();
let settle_cpu = budget_after.cpu_insns() - budget_before.cpu_insns();
```

Call site: `LiquifactEscrow::settle` at `escrow/src/lib.rs:5245`.

### 5.4 Measurement — Phase 2: 20 sequential claims

```rust
let budget_before = env.budget();
for investor in &investors {
    client.claim_investor_payout(investor);
}
let budget_after = env.budget();
let claim_cycle_cpu = budget_after.cpu_insns() - budget_before.cpu_insns();
let avg_cpu_per_claim = claim_cycle_cpu / 20;
```

Call site: `LiquifactEscrow::claim_investor_payout` at `escrow/src/lib.rs:5723`. Each call does one persistent read (`get_persistent_investor_contribution`, `lib.rs:5739`), an escrow status read, a settled-amount read, a lock-in-period check, and (per the existing implementation) a SEP-41 transfer via `external_calls`. Reminder: `Env::budget()` in the Soroban test harness typically tracks the currently-executing single-contract-call budget window per invocation rather than a persistent global counter — verify at implementation time whether cross-call accumulation actually captures each `claim_investor_payout` call's cost or resets on host-function boundaries; if it resets, sum per-call deltas inside the loop instead of taking one before/after snapshot around all 20 calls.

### 5.5 Assertions

- `settle_cpu > 0` (sanity — it should consume some CPU).
- `settle_cpu < <bound_settle>` — proposed starting bound: `50_000_000` (50M instructions), one order of magnitude above the observed value once measured, then tightened. **Exact bound must be calibrated against a real measured run before merge**; the number here is a placeholder ceiling, not a target.
- `claim_cycle_cpu > 0`.
- `claim_cycle_cpu < <bound_claim_cycle>` — proposed starting bound: `250_000_000` (250M instructions for 20 claims, i.e. ~12.5M/claim ceiling), calibrated the same way.
- Every assertion message must include the actual measured value, e.g.:
  ```rust
  assert!(
      settle_cpu < BOUND,
      "settle CPU should stay under {} instructions for a 20-investor escrow; actual: {}",
      BOUND, settle_cpu
  );
  ```
  This is required so a future recalibration doesn't require re-instrumenting the test to find the current number — per the acceptance criteria below.

### 5.6 Non-blocking on exact numbers

Because no baseline exists today, the concrete threshold constants in §5.5 are **not** final — they are illustrative starting points. The engineer implementing this ticket must:
1. Run the test once with the assertions loosened/removed (or with `println!`/test output) to capture the actual `settle_cpu`, `claim_cycle_cpu`, and `avg_cpu_per_claim` on current `main`.
2. Set the real thresholds at roughly 2–3× the observed value (matching the headroom style of the existing `500_000_000` bound for 100-investor funding in `health_and_events.rs:344`), not at the placeholder values above.
3. Record the observed baseline numbers in the test's doc comment for future reference, mirroring the comment style at `health_and_events.rs:300-301`.

## 6. Expected vs. Actual Behavior

| | Expected | Actual (today) |
|---|---|---|
| `settle` cost scaling | O(1) — single escrow-state read/write, no per-investor iteration (settlement snapshot math operates on the escrow record, not per-investor records) | Unknown — unmeasured. Should be confirmed, not assumed, since `settle` does write `FundingCloseSnapshot`/`SettledAmount` state that must be checked for any accidental per-investor coupling. |
| `claim_investor_payout` cost scaling across 20 calls | Linear in investor count — each call touches only its own investor's persistent keys | Unknown — unmeasured. Risk case: a future change routes claims through a shared structure (e.g. an audit/append log sized like `MAX_REINVESTMENT_AUDIT_ENTRIES`, `lib.rs:156`) that every claim scans or appends to, silently turning claims quadratic in investor count. |

The absence of a baseline is itself the actual-behavior gap this ticket closes: today there is no automated way to know if either of the above expectations holds.

## 7. Relationship to Feature #221

`FEATURE_221_BENCHMARK_SUITE_DESIGN.md` describes a much larger `criterion`-based wall-clock benchmark suite (`escrow/benches/`, CI integration, HTML reports, historical regression comparison across fund/settle/claim/compute_payout/sweep_dust at 1/100/1000-investor scales). That suite does not exist in the repo yet (`find escrow/benches` returns nothing). PERF-009 is intentionally a much smaller, immediately actionable slice: one `env.budget()`-based unit test, no new dependencies, no new CI job, following a pattern (`test_bucketing_cost_baseline_many_investors`) that already exists and already runs in the normal `cargo test` suite. If Feature #221 is eventually implemented, this test's coverage of the settle+claim cycle at N=20 can be superseded by (or feed calibration into) the `bench_settle_simple` / `bench_claim_bulk_sequential` benchmarks it defines — but PERF-009 should not wait on that larger effort.

## 8. Acceptance Criteria

- [ ] New test `test_cost_baseline_settle_and_claim_cycle_20_investors` added, in one of the two locations from §5.1.
- [ ] Test funds exactly 20 distinct investors to a fully-funded state, then calls `settle(None)`, then calls `claim_investor_payout` for all 20, per §5.2–§5.4.
- [ ] Test asserts an upper CPU-instruction bound on (a) `settle` alone and (b) the full 20-claim cycle, each with a calibrated (not placeholder) threshold per §5.6, and each assertion prints the actual measured value on failure.
- [ ] Test is deterministic — no wall-clock timing, no reliance on system load; only `Env::budget()` deltas.
- [ ] Test runs in the normal `cargo test` suite (no new CI job, no new dev-dependency).
- [ ] No production code (`escrow/src/lib.rs`, `escrow/src/validation.rs`, `escrow/src/external_calls.rs`) is modified as part of this ticket. If the calibration run in §5.6 surfaces an actual cost problem, file a separate follow-up ticket instead of fixing it inline here.
- [ ] Doc comment on the test records the observed baseline CPU numbers at time of writing, so a future contributor recalibrating the bound has a reference point.

## 9. Risks / Open Questions

- **`Env::budget()` semantics across multiple top-level contract calls in one test:** needs confirmation at implementation time (see note in §5.4) — if the budget doesn't accumulate across the 20 separate `claim_investor_payout` client calls the way it does within a single call, the measurement strategy must switch to per-call deltas summed in the loop.
- **Threshold flakiness across `soroban-sdk` versions:** CPU-instruction costs for host functions can shift on SDK upgrades. Because this is a loose upper-bound tripwire (not an exact match), minor drift shouldn't break it, but a major SDK bump may require recalibrating the constant — acceptable, same maintenance burden as the existing `test_bucketing_cost_baseline_many_investors` bound.
- **Test placement bikeshed (§5.1):** either location satisfies the acceptance criteria; pick based on reviewer preference for discoverability (grouped with other cost-baseline tests vs. grouped with other simulation-lifecycle tests).
