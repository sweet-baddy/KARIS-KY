# Feature #220: Interactive Contract REPL / Debugger CLI Tool

## Overview

Interactive CLI tool to call contract methods, inspect state, and debug issues without writing test code.

## Design

### 1. Architecture

```
repl-cli/
  ├─ Cargo.toml
  ├─ src/
  │   ├─ main.rs              # REPL loop + command router
  │   ├─ commands.rs          # Command handlers (call, get, state, etc.)
  │   ├─ network.rs           # Network provider abstraction
  │   ├─ state_inspector.rs   # DataKey deserialization
  │   ├─ transaction.rs       # Transaction builder
  │   └─ serialization.rs     # JSON/Binary serialization
  └─ README.md
```

### 2. Command Categories

#### 2.1 Contract Interaction Commands

**`call <method> [args]`** — Invoke a contract entrypoint

```
> call fund --investor GBXYZ --amount 1000
Transaction: TxHash...
Result: Ok(())
Events:
  - InvestorFundedEvt { investor: GBXYZ, amount: 1000 }

> call settle --sme-address GABC
Transaction: TxHash...
Result: Ok(())

> call compute_investor_payout --investor GBXYZ
Result: 1050 (i128)
```

**`dry-run <method> [args]`** — Simulate without submitting

```
> dry-run fund --investor GBXYZ --amount 1000
Simulation result:
  Status: would succeed
  Gas estimate: 2,500,000
  State changes:
    - InvestorContribution(GBXYZ): 0 -> 1000
    - Escrow.funded_amount: 0 -> 1000
```

#### 2.2 State Inspection Commands

**`get <key> [index]`** — Read a specific DataKey

```
> get Escrow
InvoiceEscrow {
  invoice_id: "INV_001",
  admin: GADMIN...,
  sme_address: GASME...,
  amount: 100_000_000,
  funded_amount: 95_000_000,
  status: 1 (funded),
  ...
}

> get InvestorContribution GBXYZ
1_000_000 (i128)

> get FundingCloseSnapshot
FundingCloseSnapshot {
  total_principal: 95_000_000,
  target: 100_000_000,
  closed_at: 1690000000,
  closed_ledger: 12345,
}

> get InvestorEffectiveYield GBXYZ
Some(600) (basis points)
```

**`state [address]`** — Show full escrow state or per-investor state

```
> state
Escrow state:
  Status: 1 (funded)
  Funded: 95M / 100M
  Maturity: 2024-01-01 (1672531200)
  Yield: 500 bps
  Admin: GADMIN...
  SME: GASME...

Legal hold: false
Treasury: GTREASURY...

> state GBXYZ
Investor state (GBXYZ):
  Contribution: 1_000_000
  Effective yield: 600 bps
  Claim lock until: 2024-02-01 (1675209600)
  Claimed: false
```

**`history [limit]`** — Show recent event history (if indexer connected)

```
> history 10
[T12350] InvestorFundedEvt { investor: GBXYZ, amount: 500_000 }
[T12351] InvoiceEscrowFundedEvt { total_funded: 50_000_000, target: 100_000_000 }
[T12352] SettlementStartedEvt { maturity_reached: true, status: 2 }
```

#### 2.3 Debugging Commands

**`trace [level]`** — Set trace level (off, error, warn, info, debug, trace)

```
> trace info
Trace level set to INFO
Next calls will emit trace events...

> call fund --investor GBXYZ --amount 1000
Traces:
  [StorageRead] Escrow -> InvoiceEscrow { ... }
  [StorageWrite] InvestorContribution(GBXYZ) 0 -> 1000
  [StateChange] 0 (open) -> 1 (funded)
```

**`breakpoint <condition>`** — Set condition-based breakpoint

```
> breakpoint funded_amount > 50_000_000
Breakpoint set. Will pause before state transition if condition met.

> call fund --investor GBXYZ --amount 1000
[BREAKPOINT HIT] funded_amount = 50_001_000 > 50_000_000
State snapshot saved to breakpoint_001.json

Continue? [y/n] y
```

**`snapshot [name]`** — Save/restore contract state snapshot

```
> snapshot save pre-settlement
Snapshot saved: pre-settlement.json
Escrow state captured at ledger 12350

> call settle --sme-address GASME
...

> snapshot restore pre-settlement
State restored from pre-settlement.json
(Local simulation only; contract state unchanged)

> snapshot list
Snapshots:
  - pre-settlement (12350)
  - funded-state (12300)
```

#### 2.4 Network Commands

**`network [list|switch|add]`** — Manage network profiles

```
> network list
Networks:
  * local    (http://localhost:8000/soroban/rpc)
  testnet   (https://soroban-testnet.stellar.org)
  mainnet   (https://soroban-mainnet.stellar.org)

> network switch testnet
Connected to testnet
RPC: https://soroban-testnet.stellar.org

> network add staging https://staging.example.com/soroban/rpc
Network 'staging' added
```

**`info`** — Show current network, contract, and connection info

```
> info
Current network: testnet
Contract address: CBXYZ...
Contract version: 6
Admin: GADMIN...
Connected: true
RPC latency: 142ms
```

#### 2.5 REPL Control Commands

**`help [command]`** — Show help

```
> help
REPL commands:
  call <method> [args]     - Invoke contract method
  dry-run <method>         - Simulate without submitting
  get <key>                - Read DataKey
  state [addr]             - Show escrow state
  history [limit]          - Show event history
  trace <level>            - Set trace level
  breakpoint <cond>        - Set breakpoint
  snapshot <cmd>           - Manage snapshots
  network <cmd>            - Manage network profiles
  help [cmd]               - Show help
  quit                     - Exit REPL

> help call
call <method> [args] - Invoke a contract entrypoint

Examples:
  call fund --investor GBXYZ --amount 1000
  call settle --sme-address GASME
  call compute_investor_payout --investor GBXYZ
```

**`quit`** / `exit` / `Ctrl+D` — Exit REPL

### 3. Implementation

#### 3.1 Dependencies (Cargo.toml)

```toml
[package]
name = "karis-ky-repl-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.4", features = ["derive"] }
rustyline = "14.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
soroban-sdk = "25.0"
soroban-client = "0.10"  # hypothetical; may use stellar-sdk

[dev-dependencies]
```

#### 3.2 Main REPL Loop (main.rs)

```rust
use rustyline::DefaultEditor;
use clap::Parser;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "local")]
    network: String,
    
    #[arg(long)]
    contract: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    let mut editor = DefaultEditor::new()?;
    let mut repl = ReplContext::new(&args.network).await?;
    
    println!("karis-ky Escrow REPL v1.0");
    println!("Type 'help' for command list");
    println!("");
    
    loop {
        let readline = editor.readline("escrow> ");
        
        match readline {
            Ok(line) => {
                editor.add_history_entry(&line)?;
                
                match repl.execute(&line).await {
                    Ok(output) => println!("{}", output),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("Interrupted");
                break;
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}
```

#### 3.3 Command Router (commands.rs)

```rust
pub enum Command {
    Call { method: String, args: Vec<String> },
    DryRun { method: String, args: Vec<String> },
    Get { key: String, index: Option<String> },
    State { address: Option<String> },
    History { limit: Option<usize> },
    Trace { level: String },
    Breakpoint { condition: String },
    Snapshot { subcommand: SnapshotCmd },
    Network { subcommand: NetworkCmd },
    Info,
    Help { topic: Option<String> },
    Quit,
}

impl Command {
    pub fn parse(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        
        match parts.get(0).map(|s| *s) {
            Some("call") => {
                let method = parts.get(1).ok_or("Missing method name")?;
                let args = parts[2..].to_vec();
                Ok(Command::Call {
                    method: method.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                })
            }
            Some("get") => {
                let key = parts.get(1).ok_or("Missing key name")?;
                let index = parts.get(2).map(|s| s.to_string());
                Ok(Command::Get {
                    key: key.to_string(),
                    index,
                })
            }
            Some("state") => {
                let address = parts.get(1).map(|s| s.to_string());
                Ok(Command::State { address })
            }
            _ => Err("Unknown command".into()),
        }
    }
}
```

#### 3.4 Network Abstraction (network.rs)

```rust
pub trait NetworkProvider {
    async fn call_contract(
        &self,
        method: &str,
        args: &[String],
    ) -> Result<String>;
    
    async fn get_data_key(&self, key: &str) -> Result<String>;
    
    async fn get_events(&self, limit: usize) -> Result<Vec<Event>>;
}

pub struct LocalProvider {
    rpc_url: String,
}

pub struct TestnetProvider {
    rpc_url: String,
}

pub struct MainnetProvider {
    rpc_url: String,
}
```

#### 3.5 State Inspector (state_inspector.rs)

```rust
use soroban_sdk::{Symbol, Address};

pub fn format_data_key(key: &str, value: &str) -> String {
    match key {
        "Escrow" => {
            // Parse and pretty-print InvoiceEscrow JSON
            format_escrow(value)
        }
        "InvestorContribution" => {
            // Parse and pretty-print as i128
            format_contribution(value)
        }
        "FundingCloseSnapshot" => {
            // Parse and pretty-print snapshot
            format_snapshot(value)
        }
        _ => value.to_string(),
    }
}

fn format_escrow(json: &str) -> String {
    // Pretty-print InvoiceEscrow
    serde_json::from_str::<serde_json::Value>(json)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
        .unwrap_or_default()
}
```

### 4. Usage Example Session

```
$ cargo run -p karis-ky-repl-cli -- --network local --contract CBXYZ...

karis-ky Escrow REPL v1.0
Type 'help' for command list

escrow> info
Current network: local (http://localhost:8000/soroban/rpc)
Contract: CBXYZ...
Schema version: 6
Status: 1 (funded)

escrow> state
Escrow state:
  Status: 1 (funded)
  Funded: 95M / 100M target
  Maturity: 2024-01-01
  Yield: 500 bps (base)

escrow> get Escrow
InvoiceEscrow {
  invoice_id: "INV_001",
  admin: GADMIN...,
  amount: 100_000_000,
  funded_amount: 95_000_000,
  ...
}

escrow> call compute_investor_payout --investor GBXYZ
Result: 1_050_000 (i128)

escrow> snapshot save pre-settle
Snapshot saved: pre-settle.json

escrow> call settle --sme-address GASME
Transaction: TxHash...
Result: Ok(())
Events: [SettlementStartedEvt, ...]

escrow> state
Escrow state:
  Status: 2 (settled)
  ...

escrow> call claim_investor_payout --investor GBXYZ
Transaction: TxHash...
Result: Ok(())

escrow> history 5
[T12350] InvestorFundedEvt { ... }
[T12351] FundedEvt { ... }
[T12352] SettlementStartedEvt { ... }
[T12353] InvestorPayoutClaimedEvt { investor: GBXYZ, payout: 1_050_000 }

escrow> quit
```

### 5. Testing

**Unit tests:**
```rust
#[test]
fn test_parse_call_command() {
    let cmd = Command::parse("call fund --investor GBXYZ --amount 1000").unwrap();
    assert!(matches!(cmd, Command::Call { ... }));
}

#[test]
fn test_format_escrow_state() {
    let json = r#"{"status": 1, "funded_amount": 95000000}"#;
    let formatted = format_escrow(json);
    assert!(formatted.contains("1 (funded)"));
}
```

**Integration tests (against local testnet):**
```rust
#[tokio::test]
async fn test_repl_fund_workflow() {
    let mut repl = ReplContext::new("local").await.unwrap();
    
    repl.execute("call fund --investor GBXYZ --amount 1000").await.ok();
    let output = repl.execute("get InvestorContribution GBXYZ").await.unwrap();
    
    assert!(output.contains("1000"));
}
```

### 6. Limitations & Future Work

**Current limitations:**
1. No transaction signing (requires external wallet/key management)
2. No gas estimation (requires Soroban RPC support)
3. Breakpoints are local simulation only
4. Limited to read operations and test methods (no key/auth required)

**Future work:**
1. Integrate with Stellar Albedo / WebAuthn for signing
2. Add transaction sequencing (multi-step workflows)
3. Support for ledger Nano signing
4. Export transaction history as CSV/JSON
5. Time-travel debugging (replay from snapshot)

### 7. Security Notes

- REPL allows **read** access to all contract storage (no confidentiality)
- Write operations require proper authorization (signature)
- Suggested: run REPL on secure network (localhost or VPN)
- Never commit snapshots with sensitive state to version control

