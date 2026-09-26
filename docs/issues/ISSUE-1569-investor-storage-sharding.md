# Issue: Implement Sharding for High-Cardinality Escrow Investor Sets

**Type:** Feature
**Status:** Backlog, ready for review and assignment
**Priority:** High
**Related:** [Escrow Investor Sharding Architecture](../../ESCROW_SHARDING_DESIGN.md), [Escrow Sharding Implementation Plan](../../ESCROW_SHARDING_IMPLEMENTATION_PLAN.md), [Sharding Summary](../../ESCROW_SHARDING_SUMMARY.md), [Escrow State Export/Import](../escrow-state-export-import.md), [ADR-009: Per-Investor Persistent Storage](../adr/ADR-009-per-investor-persistent-storage.md)

## Description

Implement production investor-data sharding for escrows whose investor set is
large enough that a single contract's persistent storage, TTL maintenance, or
transaction/resource budget becomes impractical. The primary escrow should
remain the authority for escrow-level state while investor-specific records are
distributed across shard contracts and accessed through deterministic routing.

The target workload is 10,000 or more investors, with a design path that can
scale further through a bounded number of shards. Existing low-cardinality
escrows must remain compatible and must not be forced into sharding. Sharding
must preserve funding, settlement, pro-rata payout, claim, refund, allowlist,
yield, and investor-count semantics; it is a storage and execution boundary,
not a change to financial behavior.

The repository contains a design, implementation roadmap, and a proof-of-
concept `escrow/src/sharding.rs`, but the production contract does not wire
those types and operations into `lib.rs`. The proof of concept still uses a
placeholder hash and pseudocode for shard creation and cross-contract calls.

## Current Behavior

Investor-specific values such as contributions, effective yield, claim timing,
claim markers, allowlist membership, and refund markers are stored under
persistent address-keyed entries in the primary escrow. Storage footprint and
TTL work grow with investor cardinality. `MaxUniqueInvestorsCap` can reject new
investors, but it does not distribute records across contracts or provide a
capacity expansion mechanism.

Current settlement and claim paths operate against the primary escrow's local
state. There is no production shard registry, shard contract API, routing
integration, shard-level authorization, cross-shard aggregation, or migration
entrypoint for moving an existing investor set.

## Steps to Reproduce

1. Deploy and initialize an escrow with a high or unset
   `MaxUniqueInvestorsCap`.
2. Fund the escrow from a large number of distinct investor addresses, using
   the existing funding path and retaining the investor list.
3. Continue until the primary contract approaches its storage, TTL, or
   transaction/resource budget, or until the configured unique-investor cap is
   reached.
4. Attempt to add another investor or run settlement and investor claim flows.
5. Observe that the primary contract remains the only storage location; there
   is no automatic shard creation or routing, and operations eventually fail,
   become operationally expensive, or require a manual cap/redeployment
   workaround.
6. Inspect `escrow/src/sharding.rs` and observe that its routing test can run,
   but `ensure_shard_exists` and aggregation are placeholders and no public
   escrow entrypoint invokes them.

For a deterministic lower-cost reproduction, run the existing cardinality and
cap tests with a fixture configured near its investor limit, then verify that
no shard registry entries or shard contract addresses are created as new
investors are funded.

## Expected Behavior

- An escrow configured for sharding routes each investor deterministically to
  exactly one shard using a stable hash and routing configuration.
- The first investor assigned to a shard creates that shard within the allowed
  shard limit; later investors reuse it.
- The primary escrow owns the registry and aggregate totals. Shards accept
  calls only from their primary escrow and store investor-specific records.
- Repeated funding by the same investor reaches the same shard and preserves
  the existing contribution, yield, claim, allowlist, and refund semantics.
- Funding updates primary aggregates exactly once and keeps aggregate totals
  consistent with shard totals.
- Settlement verifies or computes aggregate contributions and investor counts
  across all registered shards within documented resource limits.
- Claims, refunds, and investor reads route to the correct shard and cannot
  read or mutate another investor's record.
- Existing escrows with sharding disabled continue to use the current local
  storage path without changed behavior.
- An existing escrow can opt into sharding only through an explicit,
  authorization-protected migration process that preserves all supported
  investor state and blocks conflicting fund/settle/claim operations while it
  runs.

## Actual Behavior

All investor records remain in the primary escrow. High-cardinality sets have
no shard-backed capacity path, no shard-aware settlement or claim logic, and no
safe migration procedure. The current proof-of-concept module is not wired
into the contract, returns a predictable placeholder hash rather than hashing
the address, and does not create or call real shard contracts.

## Proposed Solution

Implement the architecture in stages, validating each boundary before enabling
it for production escrows.

### 1. Shard model and configuration

Add versioned shard configuration and registry storage to the primary escrow,
including a shard count or routing capacity, immutable hash seed, shard WASM or
trusted shard template reference, per-shard addresses, and aggregate counters.
Define explicit limits for maximum shards, target investors per shard, and
settlement fan-out. Decide whether shard count is fixed at initialization or
whether the routing scheme supports expansion without changing existing
assignments; existing design guidance favors immutable assignment and no
rebalancing.

Replace the proof-of-concept placeholder hash with a deterministic Soroban-
compatible hash over the canonical investor address bytes and test its range,
determinism, and distribution. Reject zero or invalid shard configuration
rather than silently routing to shard zero.

### 2. Shard contract and authorization

Create the minimal shard contract and typed client interface for recording and
reading investor data, aggregate queries, claims, refunds, and deactivation.
Each shard must bind itself to one primary escrow and shard ID. Shard mutating
entrypoints must authenticate the primary; investor-facing operations must
validate the caller and target investor as appropriate.

Keep token custody and escrow-level status transitions in the primary unless a
security-reviewed design explicitly moves custody. Do not let a shard invent
payout totals independently of primary settlement state.

### 3. Primary routing integration

Update funding, investor reads, claims, refunds, allowlist operations, yield
lookups, and investor-count accounting to route through the registry. Lazily
spawn a shard on first use when permitted, register its address atomically
with the routing decision, and make retries idempotent.

For each operation, define behavior for a missing shard, stale registry entry,
failed cross-contract call, shard deactivation, and malformed aggregate data.
Preserve existing cap checks and ensure a contribution is not transferred or
counted twice when a cross-contract call fails or is retried.

### 4. Settlement and resource control

Aggregate shard totals and unique-investor counts with checked arithmetic and
consistency validation. Bound settlement fan-out and document behavior when
the number of shards cannot fit in one transaction, such as a staged
aggregation/checkpoint flow. Measure CPU, storage, and fee usage for realistic
shard counts rather than accepting the design estimate without benchmarks.

Claims must calculate the same pro-rata result as the unsharded path. Add
cross-shard invariants proving that the sum of shard contributions equals the
primary funded amount and that each investor maps to one shard.

### 5. Migration and rollout

Provide an authorization-protected migration path for existing escrows. Since
persistent address-keyed storage is not enumerable on-chain, require an
explicit, validated investor address list or an approved indexer/export source.
Snapshot the source state, pause conflicting operations, migrate in bounded
batches, verify every record and aggregate, and support restart from a durable
checkpoint. Never silently treat omitted investor records as zero.

Enable sharding behind an opt-in configuration or feature gate. Roll out on
local test environments, testnet, and staging before mainnet. Publish shard
registry and routing data to indexers and update SDK/API reads so clients can
resolve investor records without knowing the shard layout.

## Environment Context

- **Repository:** `KARIS-KY` Soroban escrow contracts
- **Platform:** Stellar Soroban
- **Language/toolchain:** Rust, Cargo, Soroban SDK, contract-to-contract calls
- **Current storage:** investor-specific persistent address-keyed records in the
  primary escrow
- **Current controls:** `MaxUniqueInvestorsCap` and `UniqueFunderCount` bound or
  measure cardinality but do not shard it
- **Current proof of concept:** `escrow/src/sharding.rs`, not integrated into
  `escrow/src/lib.rs`; shard creation and aggregation remain pseudocode
- **Target scale:** at least 10,000 investors, with documented tested limits
- **Compatibility constraint:** existing unsharded escrows and financial
  behavior must remain supported
- **Operational dependencies:** shard WASM/template availability, deployment
  authority, RPC/resource limits, indexer investor address lists, state export
  integrity, and a pause/migration policy for funded escrows

## Acceptance Criteria

- [ ] Sharding configuration validates nonzero bounds, target capacity, shard
      template/WASM identity, and an explicit opt-in state.
- [ ] The primary escrow stores a typed shard registry with primary binding,
      shard ID, creation metadata, and aggregate counters.
- [ ] Routing uses the canonical investor address bytes, a real deterministic
      Soroban-compatible hash, a stable seed, and a checked modulo operation.
- [ ] Routing tests prove determinism, range safety, stable assignment, and
      acceptable distribution across representative investor samples.
- [ ] A shard contract is deployable, initializes with its primary and shard ID,
      and rejects unauthorized mutating cross-contract calls.
- [ ] First use lazily creates and atomically registers a shard; retries cannot
      create duplicate registrations or duplicate investor funding.
- [ ] Funding routes new and repeat investors to the correct shard and preserves
      existing cap, contribution, yield, allowlist, and refund semantics.
- [ ] Investor reads, claims, and refunds route correctly and enforce investor
      authorization and settled-state rules.
- [ ] Primary aggregate funded amount and unique-investor count equal the checked
      sum of shard aggregates after every successful funding operation.
- [ ] Settlement verifies all registered shard aggregates and handles missing,
      failed, or resource-limited shard calls without partial state corruption.
- [ ] Pro-rata payouts and total distribution results match the unsharded path
      for equivalent fixtures.
- [ ] The implementation has a documented maximum shard count and a tested
      strategy for settlement when all shards cannot be queried in one transaction.
- [ ] Existing escrows with sharding disabled pass the existing test suite with
      unchanged externally observable behavior.
- [ ] Existing investor data can be migrated through an explicit address list or
      approved export in bounded, resumable batches with no silent omissions.
- [ ] Migration pauses conflicting operations, is idempotent, verifies source and
      target records, and supports a documented failure and rollback procedure.
- [ ] State export/import and indexer/API/SDK documentation includes shard
      registry, routing, investor lookup, and migration metadata.
- [ ] Tests cover authorization, duplicate funding, failed cross-contract calls,
      shard creation retries, aggregate mismatch, claim routing, cap behavior,
      migration restart, omitted investors, and high-cardinality load.
- [ ] Benchmarks demonstrate the target 10,000-investor workload within agreed
      storage, TTL, CPU, transaction-size, and fee limits.
- [ ] Testnet and staging rollout checks pass before the feature is enabled for
      mainnet, with monitoring and an explicit disable/pause procedure.
- [ ] Security review finds no unresolved critical or high-severity issue in
      shard authorization, routing, token accounting, migration, or aggregation.

## Assignment Notes

Before assignment, confirm shard custody boundaries, the source of the complete
investor address list for migration, the maximum supported shard fan-out per
transaction, the shard WASM deployment/versioning model, and the indexer/API
owners. A suggested first milestone is a production routing/registry contract
interface plus a standalone shard contract and benchmarked testnet prototype;
only then should existing funded escrows be considered for migration.
