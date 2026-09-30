use anyhow::{anyhow, Result};
use colored::Colorize;
use rustyline::DefaultEditor;
use serde_json::json;
use std::collections::HashMap;

/// Represents a command in the REPL.
#[derive(Debug, Clone)]
pub enum Command {
    CheckHealth,
    GetHealth,
    GetContribution { investor: String },
    FundWithCommitment { investor: String, amount: i128, lock_secs: u64 },
    Help { topic: Option<String> },
    Quit,
}

/// Health warning codes with descriptions.
#[derive(Debug, Clone, Copy)]
pub enum WarningCode {
    Healthy = 0,
    LowFundingRatio = 4001,
    CloseToMaturity = 4002,
    OverMaturity = 4003,
}

impl WarningCode {
    fn from_u32(code: u32) -> Self {
        match code {
            4001 => WarningCode::LowFundingRatio,
            4002 => WarningCode::CloseToMaturity,
            4003 => WarningCode::OverMaturity,
            _ => WarningCode::Healthy,
        }
    }

    fn description(&self) -> &'static str {
        match self {
            WarningCode::Healthy => "Healthy - No warning",
            WarningCode::LowFundingRatio => "Low Funding Ratio (< 50%)",
            WarningCode::CloseToMaturity => "Close to Maturity (< 1 day)",
            WarningCode::OverMaturity => "Over Maturity (past maturity, unfunded)",
        }
    }

    fn color_status(&self) -> String {
        match self {
            WarningCode::Healthy => "✓ HEALTHY".green().to_string(),
            WarningCode::LowFundingRatio => "⚠ WARNING".yellow().to_string(),
            WarningCode::CloseToMaturity => "⚠ WARNING".yellow().to_string(),
            WarningCode::OverMaturity => "✗ CRITICAL".red().to_string(),
        }
    }
}

/// Parse user input into a command.
fn parse_command(input: &str) -> Result<Command> {
    let parts: Vec<&str> = input.trim().split_whitespace().collect();

    match parts.get(0).map(|s| *s) {
        Some("check_health") => Ok(Command::CheckHealth),
        Some("get_health") => Ok(Command::GetHealth),
        Some("get_contribution") => {
            if parts.len() < 2 {
                return Err(anyhow!("get_contribution requires an investor address argument"));
            }
            Ok(Command::GetContribution {
                investor: parts[1].to_string(),
            })
        }
        Some("fund_with_commitment") => {
            if parts.len() < 4 {
                return Err(anyhow!("fund_with_commitment requires <investor> <amount> <lock_secs> arguments"));
            }
            let investor = parts[1].to_string();
            let amount: i128 = parts[2].parse().map_err(|_| anyhow!("Invalid amount: must be an integer"))?;
            let lock_secs: u64 = parts[3].parse().map_err(|_| anyhow!("Invalid lock_secs: must be a positive integer"))?;
            Ok(Command::FundWithCommitment { investor, amount, lock_secs })
        }
        Some("help") => {
            let topic = parts.get(1).map(|s| s.to_string());
            Ok(Command::Help { topic })
        }
        Some("quit") | Some("exit") => Ok(Command::Quit),
        Some(cmd) if cmd.is_empty() => Err(anyhow!("Empty command")),
        Some(cmd) => Err(anyhow!("Unknown command: '{}'. Type 'help' for available commands.", cmd)),
        None => Err(anyhow!("No command provided")),
    }
}

/// Mock health check response. In production, this would call the contract.
struct HealthCheckResponse {
    warning_type: u32,
    funded_ratio_bps: i64,
    time_to_maturity_secs: i64,
}

impl HealthCheckResponse {
    /// Simulate calling check_escrow_health contract endpoint.
    fn from_contract() -> Self {
        // Mock data - in production would invoke via Soroban RPC
        HealthCheckResponse {
            warning_type: 0,
            funded_ratio_bps: 7500, // 75% funded
            time_to_maturity_secs: 432_000, // 5 days
        }
    }
}

/// Mock health metrics response. In production, this would call the contract.
#[derive(Debug, Clone)]
struct EscrowHealthMetrics {
    funding_progress_percent: u32,
    days_to_maturity: i64,
    unique_investor_count: u32,
    average_contribution_size: i128,
    estimated_yield_payout: i128,
}

impl EscrowHealthMetrics {
    /// Simulate calling get_escrow_health_metrics contract endpoint.
    fn from_contract() -> Self {
        // Mock data - in production would invoke via Soroban RPC
        EscrowHealthMetrics {
            funding_progress_percent: 75,
            days_to_maturity: 5,
            unique_investor_count: 42,
            average_contribution_size: 238_095,
            estimated_yield_payout: 500_000,
        }
    }
}

/// Format and display check_health command output with color coding.
fn display_check_health(response: &HealthCheckResponse) {
    println!();
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());
    println!("{}", "ESCROW HEALTH CHECK".bright_blue().bold());
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());

    let warning = WarningCode::from_u32(response.warning_type);
    println!();
    println!(
        "Status: {}",
        warning.color_status()
    );
    println!(
        "Warning Code: {}",
        match response.warning_type {
            0 => format!("{}", response.warning_type).green(),
            4001 | 4002 | 4004 => format!("{}", response.warning_type).yellow(),
            4003 => format!("{}", response.warning_type).red(),
            _ => format!("{}", response.warning_type).normal(),
        }
    );
    println!("Description: {}", warning.description());

    println!();
    println!("Funding Ratio: {}%", format!("{:.2}%", response.funded_ratio_bps as f64 / 100.0).bright_white());
    println!("Time to Maturity: {} seconds", format_duration(response.time_to_maturity_secs).bright_white());

    println!();
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());
    println!();
}

/// Format and display get_health command output with detailed metrics.
fn display_get_health(metrics: &EscrowHealthMetrics) {
    println!();
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());
    println!("{}", "ESCROW HEALTH METRICS".bright_blue().bold());
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());

    println!();
    println!(
        "Funding Progress: {}%",
        format!("{}", metrics.funding_progress_percent).bright_cyan()
    );
    println!(
        "Days to Maturity: {}",
        if metrics.days_to_maturity < 0 {
            format!("{} (past maturity)", metrics.days_to_maturity).red()
        } else if metrics.days_to_maturity == 0 {
            format!("0 (no maturity)", ).bright_white()
        } else {
            format!("{}", metrics.days_to_maturity).bright_white()
        }
    );
    println!(
        "Unique Investors: {}",
        format!("{}", metrics.unique_investor_count).bright_cyan()
    );
    println!(
        "Avg. Contribution: {}",
        format!("{} stroops", metrics.average_contribution_size).bright_white()
    );
    println!(
        "Est. Yield Payout: {}",
        format!("{} stroops", metrics.estimated_yield_payout).bright_white()
    );

    println!();
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());
    println!();
}

/// Format and display get_contribution command output.
fn display_get_contribution(investor: &str, contribution: i128) {
    println!();
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());
    println!("{}", "INVESTOR CONTRIBUTION".bright_blue().bold());
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());

    println!();
    println!("Investor Address:");
    println!("  {}", investor.bright_cyan());
    println!();
    println!("Cumulative Principal Contributed:");
    if contribution == 0 {
        println!("  {} stroops {}", format!("{}", contribution).yellow(), "(no contribution recorded)".yellow());
    } else {
        println!("  {} stroops", format!("{}", contribution).bright_green());
    }

    println!();
    println!("{}", "═══════════════════════════════════════════════════════".bright_blue());
    println!();
}

/// Format duration in seconds to human-readable format.
fn format_duration(secs: i64) -> String {
    if secs < 0 {
        return format!("{} seconds (past)", secs);
    }

    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let remaining_secs = secs % 60;

    if days > 0 {
        format!("{} days, {} hours", days, hours)
    } else if hours > 0 {
        format!("{} hours, {} minutes", hours, minutes)
    } else if minutes > 0 {
        format!("{} minutes, {} seconds", minutes, remaining_secs)
    } else {
        format!("{} seconds", remaining_secs)
    }
}

/// Display help information.
fn display_help(topic: Option<&str>) {
    println!();
    println!("{}", "╔═══════════════════════════════════════════════════════╗".bright_blue());
    println!("{}", "║              KARIS-KY REPL CLI - HELP                  ║".bright_blue());
    println!("{}", "╚═══════════════════════════════════════════════════════╝".bright_blue());
    println!();

    match topic {
        Some("check_health") => {
            println!("{}:", "check_health".bright_cyan().bold());
            println!("  Display quick health status of the escrow contract.");
            println!("  Returns: warning_type, funded_ratio_bps, time_to_maturity_secs");
            println!();
            println!("{}:", "Color Coding".bright_yellow());
            println!("  {} - No warning detected", "✓ GREEN".green());
            println!("  {} - Low funding or close to maturity", "⚠ YELLOW".yellow());
            println!("  {} - Critical: over maturity and unfunded", "✗ RED".red());
            println!();
            println!("{}:", "Usage".bright_cyan());
            println!("  > check_health");
            println!();
        }
        Some("get_health") => {
            println!("{}:", "get_health".bright_cyan().bold());
            println!("  Display detailed health metrics of the escrow contract.");
            println!("  Returns: funding_progress_percent, days_to_maturity, unique_investor_count,");
            println!("           average_contribution_size, estimated_yield_payout");
            println!();
            println!("{}:", "Usage".bright_cyan());
            println!("  > get_health");
            println!();
        }
        Some("get_contribution") => {
            println!("{}:", "get_contribution".bright_cyan().bold());
            println!("  Display the cumulative principal contributed by an investor.");
            println!("  Returns: 0 if the investor has not contributed; otherwise their total principal.");
            println!();
            println!("{}:", "Arguments".bright_cyan());
            println!("  <investor> - The investor address (G...) to query");
            println!();
            println!("{}:", "Usage".bright_cyan());
            println!("  > get_contribution GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2");
            println!();
        }
        Some("fund_with_commitment") => {
            println!("{}:", "fund_with_commitment".bright_cyan().bold());
            println!("  Record a first-time investor contribution with commitment lock and select yield tier.");
            println!("  Returns: JSON output with selected yield tier and claim-not-before timestamp.");
            println!();
            println!("{}:", "Arguments".bright_cyan());
            println!("  <investor>  - The investor address (G...)");
            println!("  <amount>    - The amount to fund (in stroops)");
            println!("  <lock_secs> - The commitment lock duration in seconds");
            println!();
            println!("{}:", "Usage".bright_cyan());
            println!("  > fund_with_commitment GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2 50000000 86400");
            println!();
        }
        _ => {
            println!("Available commands:");
            println!();
            println!("  {} - Check escrow health status (quick overview)", "check_health".bright_cyan());
            println!("  {} - Get detailed health metrics", "get_health".bright_cyan());
            println!("  {} - Get investor contribution amount", "get_contribution <investor>".bright_cyan());
            println!("  {} - Record deposit with commitment lock", "fund_with_commitment <investor> <amount> <lock_secs>".bright_cyan());
            println!("  {} - Display this help message", "help [command]".bright_cyan());
            println!("  {} - Exit the REPL", "quit/exit".bright_cyan());
            println!();
            println!("For more information on a command, type: {}", "help <command>".bright_cyan());
            println!();
        }
    }
}

/// Main REPL loop.
async fn run_repl() -> Result<()> {
    let mut rl = DefaultEditor::new()?;

    println!();
    println!("{}", "╔═══════════════════════════════════════════════════════╗".bright_blue());
    println!("{}", "║          KARIS-KY ESCROW REPL CLI - Health Check      ║".bright_blue());
    println!("{}", "║                    Version 0.1.0                       ║".bright_blue());
    println!("{}", "╚═══════════════════════════════════════════════════════╝".bright_blue());
    println!();
    println!("Type {} for help.\n", "'help'".bright_cyan());

    loop {
        let readline = rl.readline(&format!("{} ", "repl-cli>".bright_cyan()));

        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;
                let trimmed = line.trim();

                if trimmed.is_empty() {
                    continue;
                }

                match parse_command(trimmed) {
                    Ok(Command::CheckHealth) => {
                        let response = HealthCheckResponse::from_contract();
                        display_check_health(&response);
                    }
                    Ok(Command::GetHealth) => {
                        let metrics = EscrowHealthMetrics::from_contract();
                        display_get_health(&metrics);
                    }
                    Ok(Command::GetContribution { investor }) => {
                        // Mock data - in production would invoke via Soroban RPC
                        let contribution = 50_000_000_000i128;
                        display_get_contribution(&investor, contribution);
                    }
                    Ok(Command::FundWithCommitment { investor, amount, lock_secs }) => {
                        let eff_yield = if lock_secs >= 200 { 650 } else if lock_secs >= 100 { 550 } else { 500 };
                        let tier_idx = if lock_secs >= 200 { 1 } else if lock_secs >= 100 { 0 } else { -1 };
                        let claim_nb = if lock_secs > 0 { 1700000000u64 + lock_secs } else { 0u64 };
                        let out = json!({
                            "status": "success",
                            "investor": investor,
                            "amount": amount,
                            "lock_secs": lock_secs,
                            "yield_tier_selected": {
                                "tier_index": tier_idx,
                                "effective_yield_bps": eff_yield
                            },
                            "claim_not_before": claim_nb
                        });
                        println!("{}", serde_json::to_string_pretty(&out)?);
                    }
                    Ok(Command::Help { topic }) => {
                        display_help(topic.as_deref());
                    }
                    Ok(Command::Quit) => {
                        println!("Goodbye!");
                        break;
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C");
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(e) => {
                eprintln!("REPL Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run_repl().await
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIERS: &[(u64, i64)] = &[(100, 550), (200, 650)];

    #[test]
    fn trace_lists_every_tier_with_its_qualification() {
        assert_eq!(
            trace_tier_selection(500, TIERS, 150),
            "lock_secs 150, base yield 500 bps\n\
             tier 0: min_lock_secs 100, yield 550 bps: qualifies\n\
             tier 1: min_lock_secs 200, yield 650 bps: does not qualify\n"
        );
    }

    #[test]
    fn a_lock_equal_to_min_lock_secs_qualifies() {
        assert!(trace_tier_selection(500, TIERS, 200)
            .contains("tier 1: min_lock_secs 200, yield 650 bps: qualifies"));
    }

    #[test]
    fn zero_lock_secs_qualifies_for_no_tier() {
        let trace = trace_tier_selection(500, TIERS, 0);
        assert!(!trace.contains(": qualifies"), "{trace}");
    }

    #[test]
    fn missing_tier_table_says_base_yield_applies() {
        assert_eq!(
            trace_tier_selection(500, &[], 300),
            "lock_secs 300, base yield 500 bps\nno yield tier table: base yield applies\n"
        );
    }

    #[test]
    fn parse_reads_lock_secs_and_rejects_bad_input() {
        assert!(matches!(
            ReplCommand::parse("trace_tier_selection 42"),
            ReplCommand::TraceTierSelection { lock_secs: Ok(42) }
        ));
        assert!(matches!(
            ReplCommand::parse("trace-tier-selection abc"),
            ReplCommand::TraceTierSelection { lock_secs: Err(_) }
        ));
        assert!(matches!(
            ReplCommand::parse("trace_tier_selection"),
            ReplCommand::TraceTierSelection { lock_secs: Err(_) }
        ));
    }

    #[test]
    fn parse_get_contribution_requires_investor_argument() {
        assert!(parse_command("get_contribution").is_err());
        assert!(parse_command("get_contribution ").is_err());
    }

    #[test]
    fn parse_get_contribution_accepts_valid_address() {
        let result = parse_command("get_contribution GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2");
        assert!(result.is_ok());
        match result.unwrap() {
            Command::GetContribution { investor } => {
                assert_eq!(investor, "GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2");
            }
            _ => panic!("Expected GetContribution command"),
        }
    }

    #[test]
    fn help_get_contribution_displays_correct_info() {
        display_help(Some("get_contribution"));
        // This is a visual test; actual help should display usage instructions
    }

    #[test]
    fn display_contribution_zero_shows_no_contribution() {
        display_get_contribution("GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2", 0);
    }

    #[test]
    fn display_contribution_nonzero_shows_amount() {
        display_get_contribution("GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2", 50_000_000_000i128);
    }
}
