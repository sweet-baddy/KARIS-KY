# karis-ky Escrow REPL CLI Tool

Interactive REPL (read-eval-print loop) CLI for inspecting karis-ky escrow contract state without writing test code.

## Features

**MVP supported commands:**
- `get_escrow` — Fetch current escrow state
- `get_version` — Fetch contract schema version  
- `is_dispute_paused` — Check if dispute pause is active
- `export_state` — Export complete state snapshot for backup or migration

**Output format:** Pretty-printed JSON for easy parsing and display.

## Installation

```bash
cd repl-cli
cargo build --release
```

Binary: `target/release/escrow-repl`

## Usage

### Demo mode (no contract required)

```bash
escrow-repl
```

Runs in **demo mode** with mock data for testing commands.

### Connect to testnet

```bash
escrow-repl --network testnet --contract CBXYZ123...
```

### Connect to local validator

```bash
escrow-repl --network local --contract CBXYZ123...
```

### Custom RPC endpoint

```bash
escrow-repl --rpc-url https://custom-rpc.example.com --contract CBXYZ123...
```

## Commands

### `get_escrow`

Fetch the current escrow state.

```
escrow> get_escrow
```

**Output:**
```json
{
  "invoice_id": "INV_DEMO_001",
  "admin": "GADMIN...",
  "sme_address": "GASME...",
  "amount": 100000000,
  "funded_amount": 95000000,
  "yield_bps": 500,
  "status": 1,
  "status_label": "funded",
  "maturity": 1700000000,
  "created_at": 1690000000,
  "updated_at": 1690001000
}
```

### `get_version`

Fetch contract schema version and build metadata.

```
escrow> get_version
```

**Output:**
```json
{
  "schema_version": 7,
  "contract_version": "0.1.0",
  "build_timestamp": "2026-08-29T09:15:05Z"
}
```

### `is_dispute_paused`

Check if a dispute pause is currently active (separate from legal hold).

```
escrow> is_dispute_paused
```

**Output:**
```json
{
  "is_paused": false,
  "pause_reason": null,
  "pause_ticket_id": null,
  "paused_at": null,
  "resumes_at": null
}
```

### `export_state`

Export complete escrow state snapshot (includes escrow, legal hold, funding snapshots, attestations, etc.).

Useful for backup, migration, or audit.

```
escrow> export_state
escrow> export_state | jq . | less
```

**Output:**
```json
{
  "schema_version": 7,
  "escrow": {
    "invoice_id": "INV_DEMO_001",
    "admin": "GADMIN...",
    "sme_address": "GASME...",
    "amount": 100000000,
    "funded_amount": 95000000,
    "yield_bps": 500,
    "status": 1
  },
  "funding_token": "TOKEN...",
  "treasury": "GTREASURY...",
  "legal_hold": false,
  "unique_funder_count": 42,
  "funding_close_snapshot": {
    "total_principal": 95000000,
    "target": 100000000,
    "closed_at": 1690001000,
    "closed_ledger": 12345
  }
}
```

## Examples

### Basic inspection

```bash
$ escrow-repl --network testnet --contract CBXYZ123...

karis-ky Escrow REPL v1.0
Type 'help' for command list

escrow> get_escrow
{
  "invoice_id": "INV_001",
  "status": 1,
  "status_label": "funded",
  ...
}

escrow> get_version
{
  "schema_version": 7,
  ...
}
```

### Export and process with jq

```bash
$ escrow-repl --network testnet --contract CBXYZ123...

escrow> export_state | jq '.escrow | {status, funded_amount, funding_target}'
{
  "status": 1,
  "funded_amount": 95000000,
  "funding_target": 100000000
}
```

### Save snapshot to file

```bash
escrow> export_state > escrow_snapshot.json
```

### Check pause status

```bash
escrow> is_dispute_paused
{
  "is_paused": true,
  "pause_ticket_id": "DISPUTE_123",
  "paused_at": 1690000000,
  "resumes_at": 1690003600
}
```

## Help

```
escrow> help
```

Show all available commands and usage examples.

```
escrow> help export_state
```

Show detailed help for a specific command.

## Architecture

- `main.rs` — REPL loop, command parser, network abstraction
- MVP implementation with demo mode (mock data)
- Future: Real Soroban RPC integration via stellar-sdk

## Status

**Current:** MVP with demo mode and command parsing  
**Next:** Real Soroban RPC integration for live contract inspection

## Limitations

- **Demo mode only:** Commands return mock data until Soroban RPC integration is added
- **Read-only:** Inspection only; no write operations (state mutations require `soroban contract invoke`)
- **Single network:** Use `--network` to switch (no persistent profile storage yet)

## Testing

```bash
cargo test --lib
```

Run unit tests for command parsing and help text generation.

## Future Enhancements

1. **Real RPC integration:** Call live contract via Soroban RPC
2. **Transaction support:** Invoke state-mutating entrypoints (fund, settle, etc.)
3. **Network profiles:** Persistent network configuration (.escrow-repl.toml)
4. **Snapshots:** Save and restore local snapshots for debugging
5. **Event history:** Query recent contract events
6. **Batch operations:** Multi-command scripts
7. **Interactive help:** Command completion and context-sensitive tips

## See Also

- [FEATURE_220_REPL_DESIGN.md](../FEATURE_220_REPL_DESIGN.md) — Full design specification
- [docs/escrow-sim-stellar-cli.md](../docs/escrow-sim-stellar-cli.md) — Stellar CLI recipes (reference)
- [escrow contract](../escrow) — Main contract implementation
