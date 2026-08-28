# karis-ky Escrow — Demo Video Series

> **Contract version:** `SCHEMA_VERSION = 6`  
> **Stellar CLI:** v22+  
> **Last updated:** 2026-07-25

This series walks through every stage of an invoice escrow lifecycle using the
`stellar` CLI against a local standalone validator. Each page contains:

- A **recording script** — the exact shell commands and narration cues used
  during the screen recording.
- A **transcript** — a written summary of what each step does, what the output
  looks like, and what to watch for.

The four demos build on each other sequentially. Run them in order to follow a
complete escrow from zero to settlement.

---

## Demos

| # | Demo | Duration | What it covers |
|---|------|----------|----------------|
| [01](01-deploy-contract.md) | Deploy the contract | < 5 min | Build WASM, start local node, create identities, deploy token, deploy escrow contract |
| [02](02-initialize-escrow.md) | Initialize an escrow | < 5 min | Call `init`, verify stored state with `get_escrow` and `get_version` |
| [03](03-fund-as-investor.md) | Fund as investor | < 5 min | Two investors split the funding target, read contribution state, verify snapshot |
| [04](04-settle-as-sme.md) | Settle as SME | < 5 min | SME calls `settle`, investor claims payout, read final state |

---

## Prerequisites

Before running any demo:

```bash
# Rust stable + WASM target
rustup update stable
rustup target add wasm32v1-none

# Stellar CLI v22+
cargo install --locked stellar-cli --features opt
stellar --version   # must print 22.x.x or higher

# Docker (for the local standalone validator)
docker --version
```

See [escrow-sim-stellar-cli.md](../escrow-sim-stellar-cli.md) for a complete
prerequisite reference and extended command examples.

---

## Environment variables used across demos

Each demo exports the same named shell variables. You can source the setup
commands from Demo 01 and keep them in your shell session for Demos 02–04.

| Variable | Set in | Description |
|----------|--------|-------------|
| `CONTRACT_ID` | Demo 01 | Deployed escrow contract address (`C...`) |
| `TOKEN_ID` | Demo 01 | Test SEP-41 token address (`C...`) |
| `ADMIN` | Demo 01 | Admin keypair address (`G...`) |
| `SME` | Demo 01 | SME (invoice issuer) keypair address (`G...`) |
| `INVESTOR1` | Demo 01 | First investor keypair address (`G...`) |
| `INVESTOR2` | Demo 01 | Second investor keypair address (`G...`) |
| `TREASURY` | Demo 01 | Treasury keypair address (`G...`) |

---

## Staying up to date

The demos track the **deployed WASM schema version**. When `SCHEMA_VERSION`
changes, the following items may need updating:

1. Arg list for `init` if `InvoiceEscrow` fields change.
2. Expected output shapes in transcript sections.
3. The version badge at the top of this page.
4. Any `get_version` expected output in the transcripts.

See [OPERATOR_RUNBOOK.md](../OPERATOR_RUNBOOK.md) for the upgrade and redeploy
decision tree. See the schema version changelog in the project
[README](../../README.md#schema-version-changelog-datakey-version) for a full
history of breaking vs. additive changes.

---

## Related docs

- [escrow-sim-stellar-cli.md](../escrow-sim-stellar-cli.md) — full CLI recipe
  reference for all entrypoints (extends what the demos cover)
- [escrow-lifecycle.md](../escrow-lifecycle.md) — state machine diagram and
  status transitions
- [OPERATOR_RUNBOOK.md](../OPERATOR_RUNBOOK.md) — build, deploy, upgrade, and
  rollback procedures
- [escrow-error-messages.md](../escrow-error-messages.md) — typed `EscrowError`
  code reference
- [adr/](../adr/) — design decisions behind the contract behaviour shown in the
  demos
