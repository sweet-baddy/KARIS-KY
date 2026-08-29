//! Interactive REPL CLI for karis-ky escrow contract inspection (MVP)
//!
//! This is a minimal viable product supporting four key commands:
//! - `get_escrow`: Fetch current escrow state
//! - `get_version`: Fetch schema version
//! - `is_dispute_paused`: Check if dispute pause is active
//! - `export_state`: Export complete state snapshot
//!
//! All output is pretty-printed JSON for easy parsing and display.
//!
//! Usage:
//!   escrow-repl --network <network> --contract <contract-id>
//!
//! Example:
//!   escrow-repl --network testnet --contract CBXYZ...
//!   escrow> get_escrow
//!   escrow> export_state | jq .

use clap::Parser;
use rustyline::DefaultEditor;
use serde_json::json;
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "escrow-repl")]
#[command(about = "Interactive REPL for karis-ky escrow contract inspection", long_about = None)]
struct Args {
    /// Network name (local, testnet, mainnet, or custom RPC URL)
    #[arg(long, default_value = "testnet")]
    network: String,

    /// Contract ID (Soroban contract address)
    #[arg(long)]
    contract: Option<String>,

    /// Optional RPC endpoint (overrides network default)
    #[arg(long)]
    rpc_url: Option<String>,
}

/// Command enum for REPL commands
#[derive(Debug)]
enum ReplCommand {
    /// Fetch current escrow state
    GetEscrow,
    /// Fetch schema version
    GetVersion,
    /// Check if dispute pause is active
    IsDisputePaused,
    /// Export complete state snapshot
    ExportState,
    /// Show help
    Help { topic: Option<String> },
    /// Exit REPL
    Quit,
    /// Unknown command
    Unknown(String),
}

impl ReplCommand {
    fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        match parts.first().copied() {
            Some("get_escrow") | Some("get-escrow") => ReplCommand::GetEscrow,
            Some("get_version") | Some("get-version") => ReplCommand::GetVersion,
            Some("is_dispute_paused") | Some("is-dispute-paused") => ReplCommand::IsDisputePaused,
            Some("export_state") | Some("export-state") => ReplCommand::ExportState,
            Some("help") => {
                let topic = parts.get(1).map(|s| s.to_string());
                ReplCommand::Help { topic }
            }
            Some("quit") | Some("exit") => ReplCommand::Quit,
            Some("") => return ReplCommand::Unknown("".to_string()),
            Some(cmd) => ReplCommand::Unknown(cmd.to_string()),
            None => ReplCommand::Unknown("".to_string()),
        }
    }
}

/// REPL context holding network and contract info
struct ReplContext {
    network: String,
    rpc_url: String,
    contract_id: String,
    mock_mode: bool, // For testing/demo without actual RPC
}

impl ReplContext {
    fn new(args: &Args) -> Self {
        let rpc_url = args.rpc_url.clone().unwrap_or_else(|| {
            match args.network.as_str() {
                "local" => "http://localhost:8000/soroban/rpc".to_string(),
                "testnet" => "https://soroban-testnet.stellar.org".to_string(),
                "mainnet" => "https://soroban-mainnet.stellar.org".to_string(),
                custom => custom.to_string(),
            }
        });

        let contract_id = args.contract.clone().unwrap_or_else(|| "unknown".to_string());
        let mock_mode = args.contract.is_none();

        Self {
            network: args.network.clone(),
            rpc_url,
            contract_id,
            mock_mode,
        }
    }

    /// Execute a REPL command and return the output
    async fn execute(&self, cmd: ReplCommand) -> Result<String, String> {
        match cmd {
            ReplCommand::GetEscrow => self.cmd_get_escrow().await,
            ReplCommand::GetVersion => self.cmd_get_version().await,
            ReplCommand::IsDisputePaused => self.cmd_is_dispute_paused().await,
            ReplCommand::ExportState => self.cmd_export_state().await,
            ReplCommand::Help { topic } => Ok(self.cmd_help(topic)),
            ReplCommand::Quit => Err("QUIT".to_string()),
            ReplCommand::Unknown(cmd) => Err(format!(
                "Unknown command: '{}'. Type 'help' for available commands.",
                cmd
            )),
        }
    }

    /// Simulate get_escrow (mock data for demo; real implementation would call Soroban RPC)
    async fn cmd_get_escrow(&self) -> Result<String, String> {
        if self.mock_mode {
            let mock_data = json!({
                "invoice_id": "INV_DEMO_001",
                "admin": "GADMIN...",
                "sme_address": "GASME...",
                "amount": 100_000_000i64,
                "funded_amount": 95_000_000i64,
                "yield_bps": 500i64,
                "status": 1,
                "status_label": "funded",
                "maturity": 1700000000u64,
                "created_at": 1690000000u64,
                "updated_at": 1690001000u64,
            });
            Ok(serde_json::to_string_pretty(&mock_data).unwrap())
        } else {
            // TODO: Real implementation would invoke contract via Soroban RPC
            Err("get_escrow not connected to live RPC yet. Use --rpc-url to override.".to_string())
        }
    }

    /// Simulate get_version (mock data for demo)
    async fn cmd_get_version(&self) -> Result<String, String> {
        if self.mock_mode {
            let mock_data = json!({
                "schema_version": 7u32,
                "contract_version": "0.1.0",
                "build_timestamp": "2026-08-29T09:15:05Z",
            });
            Ok(serde_json::to_string_pretty(&mock_data).unwrap())
        } else {
            Err("get_version not connected to live RPC yet. Use --rpc-url to override.".to_string())
        }
    }

    /// Simulate is_dispute_paused (mock data for demo)
    async fn cmd_is_dispute_paused(&self) -> Result<String, String> {
        if self.mock_mode {
            let mock_data = json!({
                "is_paused": false,
                "pause_reason": null,
                "pause_ticket_id": null,
                "paused_at": null,
                "resumes_at": null,
            });
            Ok(serde_json::to_string_pretty(&mock_data).unwrap())
        } else {
            Err(
                "is_dispute_paused not connected to live RPC yet. Use --rpc-url to override."
                    .to_string(),
            )
        }
    }

    /// Simulate export_state (mock data for demo)
    async fn cmd_export_state(&self) -> Result<String, String> {
        if self.mock_mode {
            let mock_data = json!({
                "schema_version": 7u32,
                "escrow": {
                    "invoice_id": "INV_DEMO_001",
                    "admin": "GADMIN...",
                    "sme_address": "GASME...",
                    "amount": 100_000_000i64,
                    "funded_amount": 95_000_000i64,
                    "yield_bps": 500i64,
                    "status": 1,
                },
                "funding_token": "TOKEN...",
                "treasury": "GTREASURY...",
                "legal_hold": false,
                "unique_funder_count": 42u32,
                "funding_close_snapshot": {
                    "total_principal": 95_000_000i64,
                    "target": 100_000_000i64,
                    "closed_at": 1690001000u64,
                    "closed_ledger": 12345u32,
                },
            });
            Ok(serde_json::to_string_pretty(&mock_data).unwrap())
        } else {
            Err("export_state not connected to live RPC yet. Use --rpc-url to override.".to_string())
        }
    }

    /// Generate help text
    fn cmd_help(&self, topic: Option<String>) -> String {
        match topic {
            Some(t) => match t.as_str() {
                "get_escrow" => {
                    "get_escrow — Fetch the current escrow state\n\
                     Returns: InvoiceEscrow with all escrow metadata\n\
                     Example: escrow> get_escrow"
                        .to_string()
                }
                "get_version" => {
                    "get_version — Fetch the contract schema version\n\
                     Returns: version, build metadata\n\
                     Example: escrow> get_version"
                        .to_string()
                }
                "is_dispute_paused" => {
                    "is_dispute_paused — Check if a dispute pause is currently active\n\
                     Returns: pause status, ticket ID, expiry timestamp\n\
                     Example: escrow> is_dispute_paused"
                        .to_string()
                }
                "export_state" => {
                    "export_state — Export complete escrow state snapshot\n\
                     Useful for backup, migration, or audit\n\
                     Returns: EscrowSnapshot with all storage keys\n\
                     Example: escrow> export_state | jq . | less"
                        .to_string()
                }
                _ => format!(
                    "Unknown help topic: '{}'. Available topics: get_escrow, get_version, is_dispute_paused, export_state",
                    t
                ),
            },
            None => {
                "karis-ky Escrow REPL v1.0\n\n\
                 Available commands:\n\
                   get_escrow       — Fetch current escrow state\n\
                   get_version      — Fetch contract schema version\n\
                   is_dispute_paused — Check if dispute pause is active\n\
                   export_state     — Export complete state snapshot\n\
                   help [command]   — Show this help or detailed command help\n\
                   quit / exit      — Exit REPL\n\n\
                 Examples:\n\
                   escrow> get_escrow\n\
                   escrow> export_state | jq .\n\
                   escrow> help export_state\n\
                 \n\
                 Note: Currently in DEMO MODE. To connect to a live contract, use --contract <address>"
                    .to_string()
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("karis-ky Escrow REPL v1.0");
    println!("Type 'help' for command list\n");

    if args.contract.is_none() {
        println!(
            "⚠️  Demo mode: No contract ID specified. Use --contract <id> to connect to live contract.\n"
        );
    }

    let context = ReplContext::new(&args);
    println!("Network: {}", context.network);
    println!("RPC: {}", context.rpc_url);
    println!("Contract: {}\n", context.contract_id);

    let mut editor = DefaultEditor::new()?;
    let prompt = "escrow> ";

    loop {
        match editor.readline(prompt) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }

                editor.add_history_entry(&line)?;

                let cmd = ReplCommand::parse(&line);
                match context.execute(cmd).await {
                    Ok(output) => println!("{}\n", output),
                    Err(e) if e == "QUIT" => {
                        println!("Goodbye!");
                        break;
                    }
                    Err(e) => eprintln!("❌ Error: {}\n", e),
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("\nInterrupted");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("\nGoodbye!");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_get_escrow() {
        let cmd = ReplCommand::parse("get_escrow");
        assert!(matches!(cmd, ReplCommand::GetEscrow));
    }

    #[test]
    fn test_parse_get_version() {
        let cmd = ReplCommand::parse("get_version");
        assert!(matches!(cmd, ReplCommand::GetVersion));
    }

    #[test]
    fn test_parse_export_state() {
        let cmd = ReplCommand::parse("export_state");
        assert!(matches!(cmd, ReplCommand::ExportState));
    }

    #[test]
    fn test_parse_quit() {
        let cmd = ReplCommand::parse("quit");
        assert!(matches!(cmd, ReplCommand::Quit));
    }

    #[test]
    fn test_parse_hyphenated_commands() {
        let cmd = ReplCommand::parse("get-escrow");
        assert!(matches!(cmd, ReplCommand::GetEscrow));
    }

    #[test]
    fn test_parse_unknown() {
        let cmd = ReplCommand::parse("foobar");
        assert!(matches!(cmd, ReplCommand::Unknown(_)));
    }
}
