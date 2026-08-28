// Snapshot tests for escrow contract state lifecycle.
//
// These tests capture the complete escrow state at key transitions:
// - init: fresh escrow created
// - first_fund: first funding deposit
// - funding_closed: escrow reaches funded status
// - settlement: escrow settled and ready for withdrawal
//
// Run with: cargo test --test snapshots --features testutils -- --nocapture
// Update snapshots with: cargo insta review

use karis_ky_escrow::{LiquifactEscrow, LiquifactEscrowClient};
use serde::Serialize;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env};
use std::string::String as StdString;

/// Serializable escrow snapshot for deterministic comparison.
/// Uses debug format for addresses and symbols to capture the contract state deterministically.
#[derive(Debug, Clone, Serialize, PartialEq)]
struct EscrowSnapshot {
    pub invoice_id: StdString,
    pub admin_debug: StdString,
    pub sme_address_debug: StdString,
    pub amount: i128,
    pub funding_target: i128,
    pub funded_amount: i128,
    pub yield_bps: i64,
    pub maturity: u64,
    pub status: u32,
}

impl EscrowSnapshot {
    /// Capture the current escrow state into a serializable snapshot.
    fn from_escrow(escrow: &karis_ky_escrow::InvoiceEscrow) -> Self {
        Self {
            invoice_id: format!("{:?}", escrow.invoice_id),
            admin_debug: format!("{:?}", escrow.admin),
            sme_address_debug: format!("{:?}", escrow.sme_address),
            amount: escrow.amount,
            funding_target: escrow.funding_target,
            funded_amount: escrow.funded_amount,
            yield_bps: escrow.yield_bps,
            maturity: escrow.maturity,
            status: escrow.status,
        }
    }
}

// Test helpers
fn deploy(env: &Env) -> LiquifactEscrowClient<'_> {
    let id = env.register(LiquifactEscrow, ());
    LiquifactEscrowClient::new(env, &id)
}

fn setup(env: &Env) -> (LiquifactEscrowClient<'_>, Address, Address) {
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 12345;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    (client, admin, sme)
}

fn free_addresses(env: &Env) -> (Address, Address) {
    (Address::generate(env), Address::generate(env))
}

/// Scenario: init creates a fresh escrow with default values.
///
/// Verifies:
/// - Escrow is in open status (0)
/// - Funded amount is zero
/// - All target/amount fields are set correctly
#[test]
fn snapshot_init_fresh_escrow() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    let (token, treasury) = free_addresses(&env);

    let escrow = client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SNAP_001"),
        &sme,
        &100_000_000_000i128,
        &800i64,
        &1000u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let snapshot = EscrowSnapshot::from_escrow(&escrow);

    // Snapshot assertion: captures current state for future regression detection.
    insta::assert_json_snapshot!(snapshot);
}

/// Scenario: first fund records initial investor contribution.
///
/// Verifies:
/// - Funded amount reflects deposit
/// - Escrow remains in open status (0) before threshold
/// - Storage persists between fund calls
#[test]
fn snapshot_first_fund_partial() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    let target = 100_000_000_000i128;
    let first_deposit = target / 2;

    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SNAP_002"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor1 = Address::generate(&env);
    let after_first_fund = client.fund(&investor1, &first_deposit);

    let snapshot = EscrowSnapshot::from_escrow(&after_first_fund);

    insta::assert_json_snapshot!(snapshot);
}

/// Scenario: funding_closed reaches target and transitions to funded status.
///
/// Verifies:
/// - Status transitions to 1 (funded) when target reached
/// - Funded amount matches or exceeds target
/// - State is stable after reaching funded
#[test]
fn snapshot_funding_closed_transition() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    let target = 100_000_000_000i128;

    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SNAP_003"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);

    // First deposit: 60% of target
    client.fund(&investor1, &(target * 60 / 100));

    // Second deposit: remaining 40% (reaches target)
    let funded_escrow = client.fund(&investor2, &(target * 40 / 100));

    let snapshot = EscrowSnapshot::from_escrow(&funded_escrow);

    insta::assert_json_snapshot!(snapshot);
}

/// Scenario: settlement marks escrow as settled (status 2).
///
/// Verifies:
/// - Status transitions to 2 after settle() call
/// - Funded amount persists through settlement
/// - All fields remain consistent
#[test]
fn snapshot_settlement_state() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    let target = 100_000_000_000i128;

    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SNAP_004"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund to target
    let investor = Address::generate(&env);
    client.fund(&investor, &target);

    // Settle the escrow
    let settled_escrow = client.settle();

    let snapshot = EscrowSnapshot::from_escrow(&settled_escrow);

    insta::assert_json_snapshot!(snapshot);
}

/// Scenario: complete lifecycle from init → fund → fund
///
/// Captures escrow state through funding workflow and detects any
/// unintended changes to state transitions or field values.
#[test]
fn snapshot_complete_lifecycle() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    let target = 50_000_000_000i128;

    let (token, treasury) = free_addresses(&env);

    // Step 1: Init
    let initial = client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SNAP_LIFECYCLE"),
        &sme,
        &target,
        &1200i64, // Higher yield
        &0u64,    // Maturity at block 0 (immediate)
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    insta::assert_json_snapshot!(EscrowSnapshot::from_escrow(&initial));

    // Step 2: Partial fund
    let investor1 = Address::generate(&env);
    let after_partial = client.fund(&investor1, &(target / 3));

    insta::assert_json_snapshot!(EscrowSnapshot::from_escrow(&after_partial));

    // Step 3: Complete funding
    let investor2 = Address::generate(&env);
    let after_full = client.fund(&investor2, &(target * 2 / 3));

    insta::assert_json_snapshot!(EscrowSnapshot::from_escrow(&after_full));
}

/// Scenario: multiple investors with varying contributions.
///
/// Verifies escrow handles multiple concurrent funders correctly
/// and accumulates contributions properly.
#[test]
fn snapshot_multi_investor_funding() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    let target = 100_000_000_000i128;

    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SNAP_MULTI"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    let investor_c = Address::generate(&env);

    // Three investors, three tranches
    client.fund(&investor_a, &(target / 3));
    client.fund(&investor_b, &(target / 3));
    let final_state = client.fund(&investor_c, &(target / 3));

    let snapshot = EscrowSnapshot::from_escrow(&final_state);

    insta::assert_json_snapshot!(snapshot);
}
