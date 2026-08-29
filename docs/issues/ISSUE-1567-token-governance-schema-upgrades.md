# Issue #1567: Add Token Governance for Schema Upgrades (RFC-005)

**Type:** Feature
**Status:** Backlog, ready for review and assignment
**Priority:** High
**Target release:** v2.2 (proposed)
**Related:** [RFC-005: Token-Based Governance for Schema Version Upgrades](../rfc/RFC-005-token-governance-schema-upgrades.md), [ADR-007: Storage Key Evolution](../adr/ADR-007-storage-key-evolution.md)

## Description

Replace the current admin-only schema/WASM upgrade authorization with an opt-in,
on-chain governance flow. Token holders must be able to review and approve a
schema version upgrade before it is executed, while the platform retains a
controlled emergency admin fallback for critical security incidents or an
unavailable governance contract.

The feature covers a governance contract and the minimum escrow integration
needed to verify and execute approved proposals. It must support transitions
from schema version `N` to `N+1`, including additive WASM upgrades and breaking
migrations or redeploy-required changes. Existing escrows that have no
configured governance contract must remain backward-compatible with the current
admin-only behavior.

## Current Behavior

Schema metadata is stored under `DataKey::Version`, and the deployed contract
currently declares `SCHEMA_VERSION = 7`. Upgrade authorization is not connected
to token-holder voting. The contract exposes an admin-controlled WASM upgrade
path, while `migrate` validates supported version transitions and reports when
no migration path exists.

As a result, a platform administrator or configured admin authority can change
the deployed WASM without an on-chain proposal, token-weighted vote, voting
deadline, or governance audit trail. Integrators and token holders cannot
reliably discover or approve a planned schema change before execution.

## Steps to Reproduce

1. Deploy an escrow contract and initialize it with the current schema version.
2. Confirm the stored value returned by `get_version` matches the deployed
   `SCHEMA_VERSION`.
3. Have an authorized admin invoke the existing WASM upgrade entrypoint with a
   new WASM hash.
4. Observe that the upgrade succeeds without creating a governance proposal,
   recording a token snapshot, collecting votes, waiting for a deadline, or
   checking an approval threshold.
5. Query the escrow state and event stream; observe there is no governance
   proposal ID or token-holder decision associated with the upgrade.

A negative test also demonstrates the missing feature: a token holder cannot
submit an upgrade proposal or vote through the escrow contract because no
governance contract integration or public proposal lifecycle exists.

## Expected Behavior

For an escrow configured with a governance contract:

- A platform-authorized proposer can submit an upgrade proposal containing the
  target escrow, old and new schema versions, upgrade type, migration plan or
  description, and the new WASM hash.
- Proposal submission records an immutable token-balance snapshot and starts a
  configurable voting period, seven days by default.
- Each token counts as one vote. An address can vote only once per proposal.
- Anyone can finalize a proposal after its deadline. The proposal is approved
  only when the configured threshold is met, 51% of participating yes/no votes
  by default.
- Execution is rejected while voting is active, for a rejected/cancelled/
  already-executed proposal, for the wrong escrow, for a schema-version
  mismatch, or when the supplied WASM hash differs from the proposal.
- A valid approved proposal can be executed once, updates the WASM, records the
  proposal as executed, and emits an auditable upgrade event.
- Failed proposals can be resubmitted as new proposals.
- Existing escrows without a governance address continue using the documented
  admin fallback path.

## Actual Behavior

No governance proposal, token snapshot, vote, deadline, approval calculation,
proposal execution guard, or proposal-linked upgrade event is currently
available. Admin authorization is the only upgrade authorization path. The
current implementation also has no governance-specific error codes for these
failure cases.

## Proposed Solution

Implement the RFC-005 design in the following slices:

1. **Governance contract**
   - Store proposals and their lifecycle: `SUBMITTED`/`VOTING`, `APPROVED`,
     `REJECTED`, `EXECUTED`, and `CANCELLED`.
   - Implement proposal submission, snapshot-based token-weighted voting,
     finalization, cancellation, execution, and read APIs.
   - Prevent duplicate votes and double execution.
   - Make voting period, approval threshold, and emergency policy configurable
     through governance-controlled parameters.

2. **Escrow integration**
   - Add a governance-only upgrade entrypoint such as
     `upgrade_via_governance(proposal_id, new_wasm_hash)`.
   - Verify proposal status, completed deadline, target escrow, expected old/new
     schema versions, and WASM hash before calling the deployer.
   - Mark the proposal executed and emit the proposal ID, WASM hash, and new
     schema version in the upgrade event.
   - Add governance errors corresponding to the RFC-005 cases (currently
     proposed as codes 60-64), without changing existing public error meanings.

3. **Configuration and compatibility**
   - Add an optional immutable governance contract address and documented
     default voting parameters to escrow configuration/storage.
   - Preserve admin-only upgrades when governance is not configured or when the
     documented emergency fallback is invoked; every fallback must emit an
     auditable event and be covered by policy documentation.
   - Define whether an upgrade is an in-place additive change, a migration, or a
     redeploy, and reject unsupported transitions rather than silently changing
     `DataKey::Version`.

4. **Documentation and deployment**
   - Update the operator runbook, upgrade checklist, governance guide, and
     deployment materials.
   - Document the token contract prerequisite, governance contract address,
     emergency procedure, proposal timeline, and indexer/event requirements.
   - Reconcile the stale schema-version statement in ADR-007 with the current
     source-of-truth value before marking the issue complete.

## Environment Context

- **Repository:** `KARIS-KY` Soroban escrow contracts
- **Platform:** Stellar Soroban smart contracts
- **Language/toolchain:** Rust, Cargo, Soroban SDK
- **Current schema constant:** `escrow/src/lib.rs` declares `SCHEMA_VERSION = 7`
- **Storage authority:** `DataKey::Version` in escrow instance storage
- **Existing upgrade surface:** admin WASM upgrade and `migrate` entrypoint
- **Governance status:** RFC-005 is draft; no governance contract integration is
  currently present
- **Prerequisites:** an audited governance token contract with a balance query
  interface, a deployed/configurable governance contract, and an authorization
  mechanism allowing governance to call escrow upgrades

## Acceptance Criteria

- [ ] A governance contract accepts valid upgrade proposals from an authorized
      platform proposer and stores all target/version/hash metadata.
- [ ] Proposal token voting power is snapshotted at submission and cannot change
      during the vote; one token equals one vote.
- [ ] Each address can vote at most once per proposal, and vote totals are
      deterministic.
- [ ] The default voting period is seven days and the default approval threshold
      is 51% of participating yes/no votes; both are configurable as specified
      by RFC-005.
- [ ] Anyone can finalize after the deadline, and finalization produces the
      correct `APPROVED` or `REJECTED` result, including zero-vote and tie cases.
- [ ] Escrow rejects governance execution before the deadline, without approval,
      after cancellation/execution, for the wrong escrow, or for a version/hash
      mismatch.
- [ ] An approved proposal executes exactly once and calls the WASM upgrade path
      with the hash committed in the proposal.
- [ ] Existing escrows without governance configuration retain their documented
      admin-only upgrade behavior.
- [ ] Emergency admin upgrades remain available only under the documented
      fallback policy and emit an event identifying the emergency action.
- [ ] Proposal submission, voting, finalization, cancellation, governance
      execution, and emergency fallback emit queryable audit events.
- [ ] Governance failure cases return stable, documented error codes without
      reusing existing unrelated escrow error meanings.
- [ ] Unit tests cover voting snapshots, duplicate votes, threshold math,
      deadlines, lifecycle transitions, authorization, and replay protection.
- [ ] Integration tests cover submit -> vote -> finalize -> execute for both an
      additive upgrade and a migration/redeploy-required transition, plus failed
      and cancelled proposals.
- [ ] Testnet rollout and rollback procedures are documented, and the relevant
      governance and escrow code maintains at least 95% coverage.
- [ ] Security review/audit finds no unresolved critical or high-severity issues.
- [ ] RFC-005, ADR-007, the operator runbook, and upgrade documentation agree on
      the supported schema-version and governance behavior.

## Assignment Notes

Do not assign implementation until the token contract interface, governance
contract ownership, emergency fallback authority, and migration-versus-redeploy
policy are confirmed. The implementation estimate in RFC-005 is 14-20 engineer
days across governance, escrow integration, tests, documentation, and audit
preparation.
