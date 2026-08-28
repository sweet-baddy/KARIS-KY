# Issue: Implement Multi-Instance Upgrade Coordinator for Batch Redeployment

**Type:** Feature
**Status:** Backlog, ready for review and assignment
**Priority:** High
**Related:** [Multi-Instance Escrow Upgrade Guide](../MULTI_INSTANCE_UPGRADE_GUIDE.md), [Operator Runbook](../OPERATOR_RUNBOOK.md), [Upgrade Checklist](../UPGRADE_CHECKLIST.md), [Escrow State Export/Import](../escrow-state-export-import.md)

## Description

Implement a coordinator for safely redeploying a batch of live escrow instances
when a breaking storage-schema change requires new contract IDs. The coordinator
must turn the current manual, per-instance procedure into a resumable workflow
that inventories instances, captures state and initialization data, deploys and
initializes replacements, restores eligible state, produces an old-to-new
contract mapping, and reports failures without losing progress.

This is an operational migration tool and workflow, not an in-place contract
upgrade and not the token-governance authorization mechanism proposed by
RFC-005. It must respect the existing rule that stored XDR/layout changes and
other unsupported migration paths require redeployment, while additive-only
changes remain eligible for a WASM upgrade path.

## Current Behavior

The repository documents a redeploy procedure, but operators must manually loop
over instances and separately perform snapshots, deployment, initialization,
state restoration, integration updates, investor notifications, and monitoring.
The guide recommends sequential execution to simplify rollback, but provides no
coordinator that persists progress or enforces consistent validation across the
batch.

## Steps to Reproduce

1. Prepare an inventory containing at least two live escrow instances, including
   one funded instance and one instance with invalid or incomplete metadata.
2. Build and upload a breaking-change WASM artifact.
3. Follow the current redeploy instructions manually for each instance: capture
   state, deploy a new contract, initialize it, restore eligible investor data,
   and append the old/new IDs to a migration log.
4. Allow the second instance to fail during deployment, initialization, state
   restoration, or an external integration update.
5. Resume the procedure from the existing documentation.
6. Observe that the operator must determine manually which instances completed,
   which new IDs are valid, which state was restored, and which notifications
   were delivered. Re-running the loop can duplicate deployments or produce
   ambiguous mappings.

## Expected Behavior

A single coordinator run should accept a validated batch manifest and produce a
persistent migration run with one record per source instance. It should:

- validate the artifact, network, schema transition, instance inventory, and
  required initialization data before making changes;
- capture a tamper-evident snapshot/export for every source instance and refuse
  to proceed with an incomplete preflight;
- process instances deterministically, with bounded concurrency or sequential
  execution selected explicitly by configuration;
- deploy and initialize exactly one replacement per source instance, recording
  transaction IDs and resulting contract IDs after each successful step;
- restore only supported state, verify balances, escrow metadata, investor data,
  schema version, and relevant state hashes after restoration;
- checkpoint every completed stage so an interrupted run can resume without
  duplicating deployments or state writes;
- isolate failed instances, preserve successful migrations, and provide a
  retry or operator-approved rollback action;
- emit a machine-readable migration manifest suitable for indexers, APIs, UIs,
  support, and investor communications; and
- perform final batch health checks and report whether the run is complete,
  partially complete, or rolled back.

## Actual Behavior

There is no batch coordinator, durable run state, idempotency key, standard
manifest schema, partial-failure report, or automated post-redeploy verification.
The available procedure relies on shell loops and manually maintained logs.
External integration updates and investor notifications are also separate from
the deployment loop, so the system cannot distinguish an on-chain success from
a fully completed migration.

## Proposed Solution

Add a coordinator under the repository's operational tooling, reusing the
existing Soroban CLI/deployment scripts and state export/import interfaces.
The implementation language and packaging should follow the existing scripts
and backend conventions; do not duplicate contract business logic in the tool.

### 1. Manifest and preflight

Define a versioned input manifest containing the environment/network, release
and WASM hash, source schema version, target schema version, migration reason,
operator approval, and one entry per instance. Each instance entry should
include the old contract ID, invoice ID, initialization parameters or a secure
reference to them, current status, and restoration policy.

Preflight must validate:

- network and environment are explicit and cannot silently target mainnet;
- the WASM artifact exists and its hash matches the manifest;
- every source contract is reachable and reports the expected schema/version;
- required snapshots and initialization parameters are available;
- the change is classified as redeploy-required rather than additive-only;
- duplicate source IDs and already-migrated entries are rejected or handled
  idempotently; and
- the operator has configured credentials, destination services, and a rollback
  policy.

### 2. Durable state machine

Persist a run ID, manifest hash, timestamps, and per-instance state transitions:
`PENDING`, `SNAPSHOTTED`, `DEPLOYED`, `INITIALIZED`, `RESTORED`, `VERIFIED`,
`NOTIFIED`, `FAILED`, and `ROLLED_BACK`. Store transaction hashes, contract IDs,
error details, retry counts, and artifact hashes. State transitions must be
atomic enough to support restart and must never mark a step complete before its
verification succeeds.

### 3. Execution and recovery

Support a dry-run/preflight mode and an execution mode. Default to conservative
sequential processing; allow bounded concurrency only when explicitly enabled.
Add retry handling for transient RPC failures with limits and backoff, while
classifying deterministic contract errors as operator failures.

On restart, resume from the last durable checkpoint. If a replacement contract
was created before the coordinator stopped, discover it from the recorded
transaction or idempotency key instead of deploying a second replacement.
Provide per-instance retry and a batch-level pause/resume command. Do not
silently delete or overwrite old instances; legal holds or retirement actions
must be explicit and policy-controlled.

### 4. State restoration and verification

Use the existing export/import or documented migration mechanisms to restore
only state that is supported by the target schema. Verify, at minimum, target
initialization, schema version, escrow metadata, investor contributions and
claim-related state where applicable, token balances, and state hashes against
the source snapshot. Record fields that cannot be restored and require explicit
operator acknowledgement.

### 5. Outputs and integrations

Generate a versioned JSON migration manifest mapping each old contract to its
new contract, with status, schema versions, snapshot references, transaction
hashes, timestamps, and failure details. Provide a dry-run diff and a final
batch report. Integrations should be pluggable or delivered as an explicit
post-run step for indexer/API/UI updates and notifications; a failed notification
must not be reported as a successful full migration.

Document rollback separately for pre-restoration, post-restoration, and
partially completed batches. The coordinator must not claim that an on-chain
redeploy can be reversed when investor or external-system state has already
moved; instead it should pause the affected instance and require operator
resolution.

## Environment Context

- **Repository:** `KARIS-KY` Soroban escrow contracts
- **Platform:** Stellar Soroban
- **Current tooling:** Rust/Cargo contract workspace, shell deployment scripts,
  and documented Stellar CLI operations
- **Relevant workflow:** breaking schema changes require a new contract ID and
  state restoration; additive-only changes should not enter this workflow
- **Existing documentation:** `docs/MULTI_INSTANCE_UPGRADE_GUIDE.md` describes
  manual inventory, snapshot, deploy, initialize, restore, notification,
  rollback, and health-check steps
- **Deployment environments:** testnet, staging, and mainnet; mainnet must
  require an explicit confirmation gate
- **Operational constraints:** RPC failures, transaction submission delays,
  Soroban resource limits, funded/in-flight escrows, investor data integrity,
  indexer/API cache updates, and legal-hold coordination
- **Prerequisites:** stable state export/import format, deployer credentials,
  access to the source instance inventory/indexer, and an agreed policy for
  unsupported or non-restorable state

## Acceptance Criteria

- [ ] A versioned batch manifest schema is documented and validated before any
      on-chain mutation.
- [ ] Preflight validates network, artifact hash, schema transition, source
      instances, required initialization data, credentials, and duplicate IDs.
- [ ] Dry-run mode produces a complete planned action list and identifies
      missing, invalid, or non-restorable data without submitting transactions.
- [ ] Each run and instance has durable status, timestamps, transaction hashes,
      contract IDs, artifact hashes, retry counts, and error details.
- [ ] The coordinator snapshots every source instance before deployment and
      blocks execution when required snapshots are missing or invalid.
- [ ] The default execution mode is deterministic and sequential; any parallel
      mode has explicit bounded concurrency and documented rate-limit behavior.
- [ ] A successful instance progresses through deploy, initialize, restore,
      verify, and notify states only after each step succeeds.
- [ ] Restarting a run resumes from checkpoints and does not create duplicate
      replacement contracts or duplicate state restoration.
- [ ] Transient RPC failures are retried within configured limits; deterministic
      failures isolate the instance and produce actionable diagnostics.
- [ ] Per-instance retry and batch pause/resume are supported without discarding
      completed migrations.
- [ ] Restoration verifies target schema/version, escrow metadata, supported
      investor state, token balances, and required state hashes against snapshots.
- [ ] Unsupported state is reported explicitly and cannot be silently dropped.
- [ ] The coordinator generates a versioned old-to-new migration manifest and a
      final report with complete, partial, failed, and rolled-back statuses.
- [ ] Integration updates and notifications are explicit, auditable steps; a
      notification failure cannot be hidden behind deployment success.
- [ ] Testnet execution covers at least one multi-instance batch with funded,
      empty, and intentionally failing instances.
- [ ] Tests cover idempotent restart, duplicate detection, partial failure,
      retry limits, bounded concurrency, snapshot mismatch, and rollback gates.
- [ ] Mainnet execution has an explicit confirmation gate, credential safety
      checks, and documented pause/rollback procedures.
- [ ] The multi-instance guide, operator runbook, upgrade checklist, and state
      migration documentation describe the coordinator workflow and manifest.

## Assignment Notes

Before assignment, confirm the owner of the state export/import contract,
the supported restoration boundary for funded escrows, the integration targets
(indexer/API/UI), the desired coordinator runtime, and whether external
notifications are in scope for the first milestone. A suggested first milestone
is manifest validation, durable checkpoints, sequential testnet execution, and
machine-readable reporting; bounded parallelism and automated integration
updates can follow once recovery behavior is proven.
