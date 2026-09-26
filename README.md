# karis-ky Escrow Contracts

Soroban smart contracts for karis-ky, the invoice liquidity network on Stellar.
This repository contains the `escrow` contract that holds investor funds for
tokenized invoices until settlement and is maintained under the `karis-ky` project name.

---

## FAQ

**How do I run the tests?**
Run `cargo test` from the repository root to execute the full workspace test suite, or `cargo test -p karis-ky_escrow` to target the escrow crate only. CI mirrors these commands, so a green local run is a good pre-push signal.
See also: [Quick start](#quick-start) and [`escrow/`](escrow/).

**What is the current schema version?**
The authoritative version is the `SCHEMA_VERSION` constant in `escrow/src/lib.rs`, stored on-chain under `DataKey::Version` at `init`. The current value is `7`; production instances should match the deployed WASM.
See also: [Schema version changelog](#schema-version-changelog-datakeyversion).

**Which tokens are supported?**
The escrow binds a single funding token at `init` via the `FundingToken` key, so any SEP-41 / Soroban-compatible token contract can be used. The token is fixed for the lifetime of the escrow and cannot be changed after initialization.
See also: [Escrow init parameters](docs/escrow-init-parameters.md).

**How do I perform a legal hold?**
Without a configured guardian, the admin calls `set_legal_hold` to activate or clear a compliance hold. With a guardian, the admin calls `propose_legal_hold` and the guardian calls `confirm_legal_hold` to activate; the admin still clears the hold. Legal hold is distinct from a dispute pause and is coordinated per the operator runbook.
See also: [`docs/OPERATOR_RUNBOOK.md`](docs/OPERATOR_RUNBOOK.md).

**What happens if the admin key is lost?**
Admin-gated operations (legal hold, dispute pause, attestations, cloning) become unavailable, so admin keys should be held in governance multisig / custody. Recovery requires redeploying or re-initializing under a new admin per the runbook; there is no on-chain key-recovery path.
See also: [Release runbook: build, deploy, verify](#release-runbook-build-deploy-verify).

**What is a dispute pause and how does it differ from a legal hold?**
`pause_dispute` temporarily freezes an escrow due to a dispute, and `resume_dispute` (or auto-expiration) lifts it; `is_dispute_paused` / `get_dispute_pause` report the state. It is a separate mechanism from `set_legal_hold`, which is a compliance control.
See also: [Escrow contract — public entrypoints](#escrow-contract--public-entrypoints).

**How is the maturity date enforced?**
`settle` requires SME auth and enforces the invoice maturity date, so a funded escrow cannot be settled before maturity. Maturity is set at `init` and is part of the stored `InvoiceEscrow`.
See also: [Escrow init parameters](docs/escrow-init-parameters.md).

**How do yield tiers work?**
`fund_with_commitment` records the first deposit with an optional lock period and selects a tiered yield from the `YieldTierTable`. Per-investor effective yield is stored under `InvestorEffectiveYield` and used at claim time.
See also: [Escrow fund parameters](docs/escrow-fund-parameters.md).

**How do I migrate an existing escrow?**
Call `migrate(from_version)`, which currently emits typed errors on all paths (codes 90–92) and has no silent migration path to version 6. Additive-key upgrades need no `migrate` call, but struct-layout changes require a redeploy or an explicit migration implementation.
See also: [`migrate` entrypoint — typed error semantics](#migrate-entrypoint--typed-error-semantics).

**How do I set up the REPL / CLI?**
Source `scripts/local-env.sh` to spin up a local Soroban validator, identities, test token, and a deployed contract in one command. Then use the Stellar CLI (or the TypeScript SDK) to interact with the deployed contract.
See also: [Local development (one-command)](#local-development-one-command) and [TypeScript SDK](#typescript-sdk).

---

## Prerequisites

- Rust 1.70+ (stable)
- `wasm32v1-none` target for WASM builds: `rustup target add wasm32v1-none`
- Soroban / Stellar CLI (optional — for deployment and contract interaction)

For local development and CI, Rust alone is sufficient.

### SDK Examples

Common integration patterns are demonstrated in [`examples/basic_workflow.rs`](examples/basic_workflow.rs):
init, fund, settle, claim, tiered yield, oracle settlement, and NFT minting workflows.

---

## Quick start

```bash
cargo build
cargo test
```

### Local development (one-command)

```bash
source scripts/local-env.sh
```

This sets up a complete local Soroban environment — validator, identities,
test token, and deployed contract — ready for development. See
[scripts/local-env.sh](scripts/local-env.sh) for details.

### TypeScript SDK

```bash
cd sdk-ts
npm install
npm run build
npm run example
```

See [`sdk-ts/`](sdk-ts/) for the typed client wrapper, contract types, and example usage.

---

## Schema version changelog (`DataKey::Version`)

The `SCHEMA_VERSION` constant in `escrow/src/lib.rs` is stored on-chain under
`DataKey::Version` at [`init`] and is the authoritative version for upgrade
decisions. All production instances should have this value match the deployed
WASM.

| Version | Description | Upgrade path |
|---------|-------------|--------------|
| 1 | Initial schema (`InvoiceEscrow` v1, basic funding / settle) | N/A |
| 2 | Added per-investor yield keys (`InvestorEffectiveYield`, `InvestorClaimNotBefore`) | Additive keys — no `migrate` call required for read compatibility |
| 3 | Added `FundingCloseSnapshot`, `MinContributionFloor`, `MaxUniqueInvestorsCap`, `UniqueFunderCount` | Additive keys — old instances return `None` / `0` defaults |
| 4 | Added attestation API (`PrimaryAttestationHash`, `AttestationAppendLog`) | Additive keys — no `migrate` call required |
| 5 | Added `YieldTierTable` (`fund_with_commitment`), `RegistryRef`, `Treasury`; tightened `InvoiceEscrow` layout | **Redeploy required** if `InvoiceEscrow` struct layout differs from stored XDR |
| 6 | Moved per-investor keys to persistent storage to bound instance footprint and decouple per-address TTL | **Redeploy required** — prior instances must be redeployed to pick up new storage locations |
| 7 | Added `DisputePaused` state for temporary dispute resolution (separate from legal hold) | Additive keys — no `migrate` call required |

> **Current:** `SCHEMA_VERSION = 8`

---

## Storage-only upgrade policy (additive fields)

**Compatible without redeploy** when you only:

- Add **new** `DataKey` variants and/or new `#[contracttype]` structs stored
  under **new** keys.
- Read new keys with `.get(...).unwrap_or(default)` so missing keys behave as
  "unset" on old deployments.

**Requires new deployment or explicit migration** when you:

- Change the layout or XDR shape of an existing stored type (e.g. add a
  required field to `InvoiceEscrow` without a migration that rewrites
  `DataKey::Escrow`).
- Rename or change the XDR shape of an existing `DataKey` variant used in
  production.

### `migrate` entrypoint — typed error semantics

`LiquifactEscrow::migrate(from_version)` emits typed [`EscrowError`](docs/escrow-error-messages.md)
codes in all current cases. There is **no silent migration path** from any prior version to
version 6. Callers must not assume it will do bookkeeping work:

| Condition | Typed error (code) |
|-----------|-------------------|
| `stored != from_version` | `MigrationVersionMismatch` (90) |
| `from_version >= SCHEMA_VERSION` | `AlreadyCurrentSchemaVersion` (91) |
| Any `from_version < SCHEMA_VERSION` | `NoMigrationPath` (92) |

See [`docs/escrow-error-messages.md`](docs/escrow-error-messages.md) for the full reference.

To add a real migration path (e.g. rewrite `DataKey::Escrow` after a struct
field change), implement the transformation inside `migrate` before the final
typed error and update `DataKey::Version`.

### `DataKey` naming convention

| Rule | Example |
|------|---------|
| PascalCase enum variant | `DataKey::FundingToken` |
| Per-address variants use tuple form | `DataKey::InvestorContribution(Address)` |
| New variants must be additive (no rename of existing) | — |

### Compatibility test plan (short)

1. Deploy version _N_; exercise `init`, `fund`, `settle`.
2. Deploy version _N+1_ with only new optional keys; repeat flows; assert old
   instances still readable.
3. If `InvoiceEscrow` changes, add a migration test **or** document mandatory
   redeploy.

See [`docs/OPERATOR_RUNBOOK.md`](docs/OPERATOR_RUNBOOK.md) for the full
redeploy-vs-upgrade decision tree and Stellar/Soroban CLI examples.

---

## Release runbook: build, deploy, verify

**Who may deploy production:** only addresses and keys owned by karis-ky
governance (multisig / custody). Treat contract admin and deployer secrets as
**highly sensitive**.

See [`docs/OPERATOR_RUNBOOK.md`](docs/OPERATOR_RUNBOOK.md) for the step-by-step
runbook including pre-flight checklists, rollback protocol, and legal hold
coordination.

### Environment variables (example)

| Variable | Purpose |
|----------|---------|
| `STELLAR_NETWORK` | e.g. `testnet` / `mainnet` / custom network passphrase |
| `SOROBAN_RPC_URL` | Soroban RPC endpoint |
| `SOURCE_SECRET` | Funding / deployer Stellar secret key (`S...`) |
| `LIQUIFACT_ADMIN_ADDRESS` | Initial admin intended to control holds and funding target |

Exact CLI flags change between Soroban releases; always cross-check the
[Stellar Soroban docs](https://developers.stellar.org/docs/tools/soroban-cli/stellar-cli)
for your installed `stellar` CLI version.

### Build WASM

```bash
rustup target add wasm32v1-none
cargo build --target wasm32v1-none --release -p karis-ky_escrow
# Artifact (typical):
# target/wasm32v1-none/release/karis-ky_escrow.wasm
```

### Lint

```bash
# Escrow crate only (mirrors CI)
cargo clippy -p karis-ky_escrow -- -D warnings

# Entire workspace
cargo clippy --all-targets -- -D warnings
```

---

## Escrow contract — public entrypoints

| Entrypoint | Description |
|------------|-------------|
| `init` | Create an invoice escrow; binds funding token, treasury, optional registry. See [parameter reference](docs/escrow-init-parameters.md). |
| `fund` | Record investor principal; marks escrow funded when target is met. See [fund parameters reference](docs/escrow-fund-parameters.md). |
| `fund_with_commitment` | First deposit with optional lock period; selects tiered yield. See [fund parameters reference](docs/escrow-fund-parameters.md). |
| `settle` | Mark a funded escrow as settled (SME auth required; maturity enforced). |
| `clone_settled_escrow` | Clone a settled escrow template to create a new independent escrow with the same parameters (admin auth required). |
| `withdraw` | SME pulls funded liquidity (accounting record). |
| `claim_investor_payout` | Investor records a payout claim after settlement. |
| `sweep_terminal_dust` | Treasury sweeps rounding residue from a terminal escrow. |
| `migrate` | Schema version gate — **typed errors on all paths** in the current release (codes 90–92). |
| `set_legal_hold` | Admin activates/clears compliance hold. |
| `pause_dispute` | Admin temporarily freezes escrow due to dispute (separate from legal hold). |
| `resume_dispute` | Admin manually resumes a paused escrow (or wait for auto-expiration). |
| `is_dispute_paused` | Check if dispute pause is currently active. |
| `get_dispute_pause` | Retrieve active dispute pause state (ticket, timestamps). |
| `bind_primary_attestation_hash` | Admin sets a single-write 32-byte digest. |
| `append_attestation_digest` | Admin appends to bounded audit log. |
| `record_sme_collateral_commitment` | ⚠️ **Metadata only — not proof of custody.** SME records collateral pledge. See [`docs/escrow-sme-collateral.md`](docs/escrow-sme-collateral.md). |
| `get_escrow` | Read current escrow state. |
| `get_version` | Read stored `DataKey::Version`. |

---

## Storage guardrails

The escrow stores per-investor contribution entries inside the contract
instance. That map is intentionally bounded.

- Supported investor cardinality: configured via `max_unique_investors` at
  `init` (optional cap); no hard-coded global max since investor cardinality
  is escrow-specific.
- Attestation append log: bounded at `MAX_ATTESTATION_APPEND_ENTRIES = 32`.
- Dust sweep: capped at `MAX_DUST_SWEEP_AMOUNT = 100_000_000` base units per
  call.

---

## Test organization

Escrow tests are organized by feature area under
[`escrow/src/test/`](escrow/src/test):

| File | Coverage area |
|------|--------------|
| `init.rs` | Initialization, invoice-id validation, getters, init-shaped baselines |
| `funding.rs` | Funding, contribution accounting, snapshots, tier selection |
| `settlement.rs` | Settlement, withdrawal, investor claims, maturity boundaries, dust sweep |
| `admin.rs` | Admin-governed state changes, legal hold, migration guards, collateral metadata |
| `integration.rs` | External token-wrapper assumptions, metadata-only integration checks |
| `properties.rs` | Proptest-based invariants |

Shared helpers live in [`escrow/src/test.rs`](escrow/src/test.rs). Each test
creates its own fresh `Env` so feature modules do not rely on hidden
cross-test state.

### Snapshot regression tests

[`escrow/tests/snapshots.rs`](escrow/tests/snapshots.rs) contains snapshot tests
that verify contract state structure and transitions using the `insta` crate.
These tests compare serialized contract state against stored `.snap` files
committed to the repository.

**When snapshot files update:**

Snapshot files are automatically committed to the repository when the escrow
contract's `InvoiceEscrow` struct layout changes (new fields, field type changes,
etc.). This ensures the repository history tracks state structure evolution
alongside WASM deployments.

**To update snapshots locally:**

```bash
# Run snapshot tests and accept all changes
cargo insta test --review
```

Alternatively, use the non-interactive accept mode:

```bash
# Run snapshot tests with auto-accept (writes new .snap files)
INSTA_FORCE_ACCEPT=true cargo test --test snapshots
```

Then commit the updated `.snap` files to git.

**CI verification:**

The CI pipeline runs snapshot tests with `INSTA_FORCE_ACCEPT=false` (the default),
which causes the build to fail if:
- A snapshot test produces output that differs from the committed `.snap` file.
- A new `.snap` file was created but not committed to git.

This prevents stale or uncommitted snapshots from masking accidental state structure changes.

---

## Architecture Decision Records

Core design decisions are captured in [`docs/adr/`](docs/adr/):

| ADR | Decision |
|-----|---------|
| [ADR-001](docs/adr/ADR-001-state-model.md) | Escrow state model (`status` 0–3, forward-only transitions) |
| [ADR-002](docs/adr/ADR-002-auth-boundaries.md) | Authorization boundaries per role (admin, SME, investor, treasury) |
| [ADR-003](docs/adr/ADR-003-settlement-flow.md) | Two-phase settlement flow and funding-close snapshot |
| [ADR-004](docs/adr/ADR-004-legal-hold.md) | Legal / compliance hold mechanism |
| [ADR-005](docs/adr/ADR-005-tiered-yield.md) | Optional tiered yield and per-investor commitment locks |
| [ADR-006](docs/adr/ADR-006-dust-sweep-and-token-safety.md) | Treasury dust sweep and SEP-41 token safety wrapper |

---

## Token integration security checklist

See [`docs/ESCROW_TOKEN_INTEGRATION_CHECKLIST.md`](docs/ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)
for supported token assumptions, explicit unsupported token warnings, and the
integration-layer responsibilities required when this contract interacts with
external token contracts.

---

MIT

## Handsoff notes

<!-- handsoff-issue-608 -->
- #608: Issue 120: Add property test: `export_state` + `import_state` is identity
