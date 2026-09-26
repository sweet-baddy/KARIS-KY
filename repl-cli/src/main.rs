//! Interactive REPL CLI for karis-ky escrow contract inspection (MVP)
//!
//! This is a minimal viable product supporting four key commands:
//! - `get_escrow`: Fetch current escrow state
//! - `get_version`: Fetch schema version
//! - `is_dispute_paused`: Check if dispute pause is active
//! - `get_attestation_log`: Fetch attestation digests
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

/// Built-in network presets mapping to their default Soroban RPC endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Network {
    Local,
    Testnet,
    Mainnet,
}

impl Network {
    /// Parse a network preset name, returning a helpful error for unknown values.
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "local" => Ok(Network::Local),
            "testnet" => Ok(Network::Testnet),
            "mainnet" => Ok(Network::Mainnet),
            other => Err(format!(
                "Unknown network '{}'. Valid networks are: testnet, mainnet, local.\n\
                 Use --rpc-url <URL> to target a custom RPC endpoint.",
                other
            )),
        }
    }

    /// Default RPC endpoint for this network preset.
    fn rpc_url(&self) -> &'static str {
        match self {
            Network::Local => "http://localhost:8000",
            Network::Testnet => "https://soroban-testnet.stellar.org",
            Network::Mainnet => "https://soroban-mainnet.stellar.org",
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Network::Local => "local",
            Network::Testnet => "testnet",
            Network::Mainnet => "mainnet",
        }
    }
}

#[derive(Parser)]
#[command(name = "escrow-repl")]
#[command(about = "Interactive REPL for karis-ky escrow contract inspection", long_about = None)]
struct Args {
    /// Network preset (testnet, mainnet, or local)
    #[arg(long, default_value = "testnet")]
    network: String,

    /// Contract ID (Soroban contract address)
    #[arg(long)]
    contract: Option<String>,

    /// Optional RPC endpoint (overrides the network preset)
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
    /// Fetch attestation digests
    GetAttestationLog,
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
            Some("get_attestation_log") | Some("get-attestation-log") => {
                ReplCommand::GetAttestationLog
            }
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
    fn new(args: &Args) -> Result<Self, String> {
        let network = Network::parse(&args.network)?;

        // `--rpc-url` overrides the preset's default endpoint.
        let rpc_url = args
            .rpc_url
            .clone()
            .unwrap_or_else(|| network.rpc_url().to_string());

        let contract_id = args.contract.clone().unwrap_or_else(|| "unknown".to_string());
        let mock_mode = args.contract.is_none();

        Ok(Self {
            network: network.as_str().to_string(),
            rpc_url,
            contract_id,
            mock_mode,
        })
    }

    /// Execute a REPL command and return the output
    async fn execute(&self, cmd: ReplCommand) -> Result<String, String> {
        match cmd {
            ReplCommand::GetEscrow => self.cmd_get_escrow().await,
            ReplCommand::GetVersion => self.cmd_get_version().await,
            ReplCommand::IsDisputePaused => self.cmd_is_dispute_paused().await,
            ReplCommand::GetAttestationLog => self.cmd_get_attestation_log().await,
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

    /// Simulate get_attestation_log (mock data for demo)
    async fn cmd_get_attestation_log(&self) -> Result<String, String> {
        if self.mock_mode {
            Ok(serde_json::to_string_pretty(&json!([])).unwrap())
        } else {
            Err("get_attestation_log not connected to live RPC yet. Use --rpc-url to override."
                .to_string())
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
                     Returns: InvoiceEscrow with all fields\n\
                     Example: get_escrow"
                        .to_string()
                }
                "get_version" => {
                    "get_version — Fetch the contract schema version\n\
                     Returns: schema_version, contract_version, build_timestamp\n\
                     Example: get_version"
                        .to_string()
                }
                "is_dispute_paused" => {
                    "is_dispute_paused — Check if dispute pause is active\n\
                     Returns: is_paused, pause_reason, pause_ticket_id, paused_at, resumes_at\n\
                     Example: is_dispute_paused"
                        .to_string()
                }
                "get_attestation_log" => {
                    "get_attestation_log — Fetch attestation digests in insertion order\n\
                     Returns: JSON array of 32-byte digests encoded as hex\n\
                     Example: escrow> get_attestation_log"
                        .to_string()
                }
                "export_state" => {
                    "export_state — Export complete state snapshot\n\
                     Returns: Full contract state as JSON\n\
                     Example: export_state | jq ."
                        .to_string()
                }
                _ => format!("No help available for '{}'", t),
            },
            None => {
                "Available commands:\n\
                 get_escrow         — Fetch current escrow state\n\
                 get_version        — Fetch schema version\n\
                 is_dispute_paused  — Check if dispute pause is active\n\
                 get_legal_hold     — Check if legal hold is active\n\
                 export_state       — Export complete state snapshot\n\
                 help [command]     — Show help for a command\n\
                 quit / exit        — Exit the REPL"
                    .to_string()
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let ctx = match ReplContext::new(&args) {
        Ok(ctx) => ctx,
        Err(err) => {
            eprintln!("Error: {}", err);
            std::process::exit(2);
        }
    };

    println!("escrow-repl — network: {}", ctx.network);
    println!("RPC endpoint: {}", ctx.rpc_url);
    println!("Contract: {}", ctx.contract_id);
    if ctx.mock_mode {
        println!("Running in mock mode (no --contract supplied).");
    }
    println!("Type 'help' for available commands, 'quit' to exit.\n");

    let mut rl = match DefaultEditor::new() {
        Ok(rl) => rl,
        Err(err) => {
            eprintln!("Failed to initialize REPL: {}", err);
            std::process::exit(1);
        }
    };

    loop {
        match rl.readline("escrow> ") {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                let cmd = ReplCommand::parse(&line);
                match ctx.execute(cmd).await {
                    Ok(output) => println!("{}", output),
                    Err(err) if err == "QUIT" => break,
                    Err(err) => eprintln!("Error: {}", err),
                }
            }
            Err(_) => break,
        }
    }
}
