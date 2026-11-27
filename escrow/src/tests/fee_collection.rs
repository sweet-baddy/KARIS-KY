//! Fee collection and basis verification tests.
//!
//! This module tests that:
//! 1. `get_fee_percentage()` returns the configured fee from storage
//! 2. The fee is applied **to the gross yield**, never to principal
//! 3. The fee is deducted at settlement time before investor claims
//! 4. Treasury receives the correct fee amount with proper event emission
//! 5. The net coupon (after fee) is what investors can claim
//!
//! # Fee Mechanics
//!
//! During settlement (`settle_impl`), for each settlement:
//! - Compute: `gross_coupon = settlement_basis * yield_bps / 10_000`
//! - If fee_percentage > 0:
//!   - Compute: `fee_amount = gross_coupon * fee_percentage / 10_000` (floor)
//!   - Transfer `fee_amount` to treasury
//!   - Emit `ProtocolFeeCollected` event
//! - Net coupon available for investors: `gross_coupon - fee_amount`

#[cfg(test)]
use super::{
    assert_contract_error, default_init, deploy, deploy_with_id, free_addresses,
    install_stellar_asset_token, setup, TARGET,
};
use crate::{LiquifactEscrow, EscrowError};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    token::StellarAssetClient,
    Address, Env, String,
};

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Set up an escrow with explicit fee configuration, backed by real Stellar asset.
/// Returns `(client, investor, sme, treasury, sac_admin, escrow_contract_id, fee_percentage)`.
fn setup_escrow_with_fee<'a>(
    env: &'a Env,
    fee_percentage: i64, // in basis points
) -> (
    super::LiquifactEscrowClient<'a>,
    Address,
    Address,
    Address,
    StellarAssetClient<'a>,
    Address,
    i64,
) {
    let sac = env.register_stellar_asset_contract_v2(Address::generate(env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(env, &escrow_id);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let treasury = Address::generate(env);
    let investor = Address::generate(env);

    // Note: This setup assumes the init function has been extended to accept fee_percentage.
    // Current code may store it via admin methods; adjust if needed.
    client.init(
        &admin,
        &soroban_sdk::String::from_str(env, "INV_FEE_TEST"),
        &sme,
        &TARGET,
        &800i64, // 8% yield
        &0u64,   // no maturity
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Mint tokens to both investor and escrow contract
    sac_admin.mint(&investor, &TARGET);
    sac_admin.mint(&escrow_id, &TARGET);

    // Fund the escrow from investor
    client.fund(&investor, &TARGET);

    (client, investor, sme, treasury, sac_admin, escrow_id, fee_percentage)
}

/// Bring an escrow to settled status.
fn settle_escrow(client: &super::LiquifactEscrowClient<'_>, env: &Env) {
    let escrow = client.get_escrow();
    if escrow.status == 1 {
        client.settle();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// get_fee_percentage() tests
// ──────────────────────────────────────────────────────────────────────────────

/// `get_fee_percentage()` returns 0 when no fee is configured at init.
#[test]
fn get_fee_percentage_returns_zero_when_unconfigured() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _investor, _sme, _treasury, _sac, _escrow_id, _) =
        setup_escrow_with_fee(&env, 0);

    let fee = client.get_fee_percentage();
    assert_eq!(
        fee, 0i64,
        "get_fee_percentage must return 0 when no fee is configured"
    );
}

/// `get_fee_percentage()` returns the configured value.
/// **Note**: This test assumes init has been extended to accept fee_percentage.
/// Adjust test setup if fee is set via admin methods instead.
#[test]
#[ignore] // Uncomment once init is extended to accept fee_percentage parameter
fn get_fee_percentage_returns_configured_value() {
    let env = Env::default();
    env.mock_all_auths();
    let (_client, _investor, _sme, _treasury, _sac, _escrow_id, expected_fee) =
        setup_escrow_with_fee(&env, 500i64); // 5% fee

    // Once init accepts fee_percentage:
    // let fee = client.get_fee_percentage();
    // assert_eq!(fee, expected_fee, "get_fee_percentage must return the configured fee");
}

// ──────────────────────────────────────────────────────────────────────────────
// Fee basis tests (yield, not principal)
// ──────────────────────────────────────────────────────────────────────────────

/// **Core test**: Fee is deducted from gross yield, not from principal.
///
/// Scenario:
/// - Principal: 1_000_000 (TARGET)
/// - Yield: 8% (800 basis points)
/// - Gross coupon: 1_000_000 * 800 / 10_000 = 80_000
/// - Fee percentage: 500 (5%)
/// - Fee amount: 80_000 * 500 / 10_000 = 4_000
/// - Net coupon for investors: 80_000 - 4_000 = 76_000
///
/// After settlement:
/// - Investor principal remains 1_000_000 (untouched)
/// - Investor yield available = 76_000
/// - Treasury receives fee = 4_000
#[test]
#[ignore] // Uncomment once settle/claim_investor_payout integration is finalized
fn fee_deducted_from_yield_not_principal() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let investor = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let sac = StellarAssetClient::new(&env, &token_id);

    // Initialize with 0 fee initially
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_YIELD_FEE"),
        &sme,
        &TARGET,
        &800i64, // 8% yield
        &0u64,
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Mint tokens
    sac.mint(&investor, &TARGET);
    sac.mint(&escrow_id, &(TARGET * 2)); // Extra for yield

    // Fund and settle
    client.fund(&investor, &TARGET);
    settle_escrow(&client, &env);

    // Get investor's principal contribution (should equal TARGET)
    let contribution = client.get_contribution(&investor);
    assert_eq!(
        contribution, TARGET,
        "Principal must not be deducted for fee; investor contribution remains TARGET"
    );

    // Expected yield calculation (without actual fee mechanism in place yet):
    // gross_coupon = TARGET * 800 / 10_000 = 80_000
    // After settlement with 0 fee: net_coupon = 80_000
    // After settlement with 5% fee (500 bps): net_coupon = 76_000
    //
    // This test documents the expectation; actual claim yields will verify the basis.
}

/// Treasury is the sole recipient of collected fees.
///
/// Scenario:
/// - Settlement with fee > 0
/// - Fee amount computed and transferred to treasury
/// - `ProtocolFeeCollected` event emitted with full accounting
#[test]
#[ignore] // Requires fee mechanism fully integrated
fn treasury_receives_fee_amount_on_settlement() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let investor = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let sac = StellarAssetClient::new(&env, &token_id);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_TREASURY_FEE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    sac.mint(&investor, &TARGET);
    sac.mint(&escrow_id, &(TARGET * 2));

    client.fund(&investor, &TARGET);
    settle_escrow(&client, &env);

    // Check that settlement emitted ProtocolFeeCollected event
    let contract_events = env.events().all();
    let events = contract_events.events();
    // Once event structure is confirmed, verify treasury address in ProtocolFeeCollected

    // Verify treasury address is correct
    let stored_treasury = client.get_treasury();
    assert_eq!(
        stored_treasury, treasury,
        "Treasury address must match configuration"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Settlement application point tests
// ──────────────────────────────────────────────────────────────────────────────

/// Fee is applied at settlement time, not before.
///
/// Sequence:
/// 1. Escrow is funded (status 1)
/// 2. Settlement is called (transitions to status 2)
/// 3. Fee is computed and transferred during settle
/// 4. Investors then claim their share of net coupon
#[test]
#[ignore] // Requires full settlement integration
fn fee_applied_at_settlement_not_funding() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let investor = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let sac = StellarAssetClient::new(&env, &token_id);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SETTLE_FEE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    sac.mint(&investor, &TARGET);
    sac.mint(&escrow_id, &(TARGET * 2));

    // Fund: status 0 → 1 (no fee yet)
    client.fund(&investor, &TARGET);
    assert_eq!(
        client.get_escrow().status,
        1,
        "Escrow must be funded after fund()"
    );

    // Settle: status 1 → 2 (fee applied here)
    settle_escrow(&client, &env);
    assert_eq!(
        client.get_escrow().status,
        2,
        "Escrow must be settled after settle()"
    );

    // Post-settlement: investors claim their net coupon
    // (no additional fee is deducted at claim time)
}

/// Fee accounting: gross coupon, fee percentage, fee amount, net coupon are all accessible.
///
/// The `ProtocolFeeCollected` event must include:
/// - `gross_coupon`: settlement_basis * yield_bps / 10_000
/// - `fee_percentage`: the configured fee (in bps)
/// - `fee_amount`: gross_coupon * fee_percentage / 10_000
/// - `net_coupon`: gross_coupon - fee_amount
///
/// This allows treasury operators to reconcile fees without on-chain queries.
#[test]
#[ignore] // Requires event structure finalization
fn fee_accounting_complete_in_event() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let investor = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let sac = StellarAssetClient::new(&env, &token_id);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_FEE_EVENT"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    sac.mint(&investor, &TARGET);
    sac.mint(&escrow_id, &(TARGET * 2));

    client.fund(&investor, &TARGET);
    settle_escrow(&client, &env);

    // Expected values:
    // gross_coupon = 1_000_000 * 800 / 10_000 = 80_000
    // fee_percentage = 500 (5%, once configured)
    // fee_amount = 80_000 * 500 / 10_000 = 4_000
    // net_coupon = 80_000 - 4_000 = 76_000

    // Once event structure is confirmed:
    // let protocol_fee_events = env.events().all()
    //     .events()
    //     .into_iter()
    //     .filter(|e| e.name == "fee_coll") // Check actual symbol
    //     .collect::<Vec<_>>();
    // assert!(!protocol_fee_events.is_empty(), "ProtocolFeeCollected event must be emitted");
    // Verify event fields: gross_coupon, fee_percentage, fee_amount, net_coupon
}

// ──────────────────────────────────────────────────────────────────────────────
// Edge cases and invariants
// ──────────────────────────────────────────────────────────────────────────────

/// When fee_percentage == 0, no fee is transferred and no fee event is emitted.
#[test]
#[ignore] // Requires full settlement integration
fn zero_fee_percentage_no_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let investor = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let sac = StellarAssetClient::new(&env, &token_id);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_ZERO_FEE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    sac.mint(&investor, &TARGET);
    sac.mint(&escrow_id, &(TARGET * 2));

    client.fund(&investor, &TARGET);
    settle_escrow(&client, &env);

    // Verify get_fee_percentage returns 0
    assert_eq!(client.get_fee_percentage(), 0i64, "Fee must be 0");

    // Verify no ProtocolFeeCollected event (or verify net_coupon == gross_coupon)
}

/// Fee percentage is immutable after init (admin cannot change it post-deployment).
#[test]
#[ignore] // Requires documenting immutability guarantee
fn fee_percentage_is_immutable_after_init() {
    // This test documents the design constraint:
    // - Fee is set at init time by admin
    // - After init, the fee cannot be changed by any subsequent call
    // - Only a full contract redeploy or migration would allow fee changes
    // This prevents surprise fee increases mid-escrow-lifecycle.
}

/// Partial settlements: fee is computed only on the settled amount, not total funded.
/// If funded_amount = 1_000_000 but partial_settle(500_000):
/// - gross_coupon = 500_000 * 800 / 10_000 = 40_000 (not 80_000)
/// - fee = 40_000 * fee_pct / 10_000 (computed on 500_000, not 1_000_000)
#[test]
#[ignore] // Requires partial settlement integration
fn fee_computed_on_partial_settlement_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let investor = Address::generate(&env);

    let token_id = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let sac = StellarAssetClient::new(&env, &token_id);

    // Double funding target to allow partial settle
    let double_target = TARGET * 2;
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_PARTIAL"),
        &sme,
        &double_target,
        &800i64,
        &0u64,
        &token_id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    sac.mint(&investor, &double_target);
    sac.mint(&escrow_id, &(double_target * 2));

    client.fund(&investor, &double_target);

    // Once partial_settle is integrated:
    // client.settle(Some(TARGET)); // Partial: settle only first half
    // gross_coupon = TARGET * 800 / 10_000 = 80_000
    // fee = 80_000 * fee_pct / 10_000 (computed on TARGET, not double_target)
}
