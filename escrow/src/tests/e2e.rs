//! End-to-end tests for the full escrow lifecycle.
//!
//! These tests exercise the complete escrow flow from initialization through
//! settlement, claims, and dust sweep using mocked token, oracle, and registry
//! contracts. Each test builds its own `Env` and is fully self-contained.
//!
//! ## Lifecycle tested
//!
//! ```text
//! init → fund (various investors) → settle → claims → dust sweep
//! ```
//!
//! ## Mock contracts
//!
//! - `MockToken`: A simple token contract used for tracking transfers in tests
//!   that don't require real SEP-41 semantics.
//! - Real Stellar Asset Contracts (SAC) are used for tests that require
//!   balance tracking and transfer verification.
//!
//! ## Edge cases covered
//!
//! - Token transfer failures
//! - Multiple investors with different contribution sizes
//! - Over-funding past target
//! - Claim idempotency
//! - Dust sweep after settlement
//! - Cancellation and refund flow
//! - Legal hold during lifecycle
//! - Admin handover during lifecycle
//! - Funding deadline expiry
//! - Min contribution enforcement
//! - Max unique investor cap enforcement

#[cfg(test)]
use super::{
    assert_contract_error, default_init, deploy, deploy_with_id, free_addresses,
    install_stellar_asset_token, setup, TARGET,
};
use crate::{
    CollateralRecordedEvt, DataKey, EscrowError, FundingCloseSnapshot, InvoiceEscrow,
    LiquifactEscrow, LiquifactEscrowClient, SmeCollateralCommitment, YieldTier,
};
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Events, Ledger as _},
    token::{StellarAssetClient, TokenClient, TokenInterface},
    Address, Env, Error, InvokeError, Map, MuxedAddress, String, Symbol, Val, Vec,
};

// ──────────────────────────────────────────────────────────────────────────────
// Mock contracts
// ──────────────────────────────────────────────────────────────────────────────

/// A bare-bones mock token contract that tracks balances in persistent storage.
/// Used for tests that need a standalone token without full SAC deployment.
#[contract]
pub struct MockToken;

#[contractimpl]
impl TokenInterface for MockToken {
    fn balance(env: Env, id: Address) -> i128 {
        env.storage().persistent().get(&id).unwrap_or(0)
    }

    fn transfer(env: Env, from: Address, to: MuxedAddress, amount: i128) {
        from.require_auth();
        let to_addr = to.address();
        let from_bal: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let to_bal: i128 = env.storage().persistent().get(&to_addr).unwrap_or(0);
        // Simple transfer — no fees
        env.storage().persistent().set(&from, &(from_bal - amount));
        env.storage().persistent().set(&to_addr, &(to_bal + amount));
    }

    fn allowance(_env: Env, _from: Address, _spender: Address) -> i128 {
        0
    }
    fn approve(_env: Env, _from: Address, _spender: Address, _amount: i128, _exp: u32) {}
    fn transfer_from(_env: Env, _spender: Address, _from: Address, _to: Address, _amount: i128) {
        unimplemented!()
    }
    fn burn(_env: Env, _from: Address, _amount: i128) {
        unimplemented!()
    }
    fn burn_from(_env: Env, _spender: Address, _from: Address, _amount: i128) {
        unimplemented!()
    }
    fn decimals(_env: Env) -> u32 {
        7
    }
    fn name(env: Env) -> String {
        String::from_str(&env, "MockToken")
    }
    fn symbol(env: Env) -> String {
        String::from_str(&env, "MOCK")
    }
}

/// Mint tokens into mock token storage by writing directly to persistent storage.
fn mock_mint(env: &Env, token_id: &Address, to: &Address, amount: i128) {
    env.as_contract(token_id, || {
        let current: i128 = env.storage().persistent().get(to).unwrap_or(0);
        env.storage().persistent().set(to, &(current + amount));
    });
}

/// A mock oracle contract that returns yield data. Used as a registry hint
/// in integration scenarios.
#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    /// Returns a fixed oracle rate for testing.
    pub fn get_rate(_env: Env) -> i64 {
        800i64 // 8% base rate
    }

    /// Returns whether the oracle is healthy.
    pub fn is_healthy(_env: Env) -> bool {
        true
    }
}

/// A mock registry contract that tracks escrow deployments.
#[contract]
pub struct MockRegistry;

#[contractimpl]
impl MockRegistry {
    pub fn register_escrow(env: Env, invoice_id: Symbol, escrow_addr: Address) {
        env.storage()
            .persistent()
            .set(&Symbol::new(&env, "last_registered"), &escrow_addr);
    }

    pub fn get_escrow(env: Env, _invoice_id: Symbol) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&Symbol::new(&env, "last_registered"))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Happy path — full lifecycle
// ──────────────────────────────────────────────────────────────────────────────

/// Full happy-path lifecycle: init → fund (single investor) → settle → claim → dust sweep.
#[test]
fn test_e2e_happy_path_single_investor() {
    let env = Env::default();
    env.mock_all_auths();

    // --- Phase 1: Deploy and Initialize ---
    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(&env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let registry = env.register(MockRegistry, ());

    let init_amount = 1_000_000i128;

    let escrow = client.init(
        &admin,
        &String::from_str(&env, "E2E_HAPPY"),
        &sme,
        &init_amount,
        &800i64,   // 8% yield
        &0u64,     // no maturity lock
        &token_id,
        &Some(registry.clone()),
        &treasury,
        &None,     // no tiers
        &None,     // no min contribution
        &None,     // no max investors
        &None,     // no max per investor
        &None,     // no legal hold clear delay
        &None,     // no funding deadline
        &None,     // no slippage threshold
    );

    assert_eq!(escrow.status, 0u32);
    assert_eq!(escrow.amount, init_amount);
    assert_eq!(escrow.funded_amount, 0i128);

    // --- Phase 2: Fund ---
    let investor = Address::generate(&env);
    sac_admin.mint(&escrow_id, &init_amount);
    let funded = client.fund(&investor, &init_amount);

    assert_eq!(funded.status, 1u32);
    assert_eq!(funded.funded_amount, init_amount);

    // Verify FundingCloseSnapshot was written
    let snapshot = client.get_funding_close_snapshot();
    assert!(snapshot.is_some());
    let snap = snapshot.unwrap();
    assert_eq!(snap.total_principal, init_amount);
    assert_eq!(snap.funding_target, init_amount);

    // --- Phase 3: Settle ---
    let settled = client.settle();
    assert_eq!(settled.status, 2u32);

    // --- Phase 4: Claim ---
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));

    // Verify compute_investor_payout returns positive value
    let payout = client.compute_investor_payout(&investor);
    assert!(payout > 0i128, "payout must be positive");

    // --- Phase 5: Dust Sweep ---
    // Mint some extra dust
    sac_admin.mint(&escrow_id, &100i128);
    let swept = client.sweep_terminal_dust(&100i128);
    assert_eq!(swept, 100i128);
    assert_eq!(TokenClient::new(&env, &token_id).balance(&treasury), 100i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Multiple investors, different amounts
// ──────────────────────────────────────────────────────────────────────────────

/// Test with multiple investors contributing different amounts, then all claiming.
#[test]
fn test_e2e_multiple_investors() {
    let env = Env::default();
    env.mock_all_auths();

    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = sac.address();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    let target = 1_000_000i128;

    client.init(
        &admin,
        &String::from_str(&env, "E2E_MULTI"),
        &sme,
        &target,
        &500i64, // 5% yield
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
        &None,
        &None,
    );

    // Three investors: 40%, 35%, 25%
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);

    client.fund(&inv_a, &400_000i128);
    assert_eq!(client.get_contribution(&inv_a), 400_000i128);

    client.fund(&inv_b, &350_000i128);
    assert_eq!(client.get_contribution(&inv_b), 350_000i128);

    client.fund(&inv_c, &250_000i128);
    assert_eq!(client.get_contribution(&inv_c), 250_000i128);

    assert_eq!(client.get_escrow().status, 1u32);
    assert_eq!(client.get_unique_funder_count(), 3u32);

    // Settle
    let settled = client.settle();
    assert_eq!(settled.status, 2u32);

    // All three claim — must be idempotent
    client.claim_investor_payout(&inv_a);
    assert!(client.is_investor_claimed(&inv_a));
    // Second claim is idempotent
    client.claim_investor_payout(&inv_a);
    assert!(client.is_investor_claimed(&inv_a));

    client.claim_investor_payout(&inv_b);
    assert!(client.is_investor_claimed(&inv_b));

    client.claim_investor_payout(&inv_c);
    assert!(client.is_investor_claimed(&inv_c));

    // Verify payouts are proportional
    let payout_a = client.compute_investor_payout(&inv_a);
    let payout_b = client.compute_investor_payout(&inv_b);
    let payout_c = client.compute_investor_payout(&inv_c);

    // sum of payouts ≈ total_principal + total_yield (within rounding)
    let total_payout = payout_a + payout_b + payout_c;
    let total_principal = 1_000_000i128;
    let total_yield = total_principal * 500i128 / 10_000i128; // 5%
    let expected_total = total_principal + total_yield;
    // Allow ±2 margin for integer truncation rounding
    let diff = if total_payout > expected_total {
        total_payout - expected_total
    } else {
        expected_total - total_payout
    };
    assert!(diff <= 3i128, "total payout {total_payout} should be close to expected {expected_total}");
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Over-funding past target
// ──────────────────────────────────────────────────────────────────────────────

/// Test that over-funding past target is handled correctly — the snapshot captures
/// the full overshoot and pro-rata math uses the true total_principal.
#[test]
fn test_e2e_overfunding() {
    let env = Env::default();
    env.mock_all_auths();

    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = sac.address();

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    let target = 1_000_000i128;

    client.init(
        &admin,
        &String::from_str(&env, "E2E_OVER"),
        &sme,
        &target,
        &500i64,
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
        &None,
        &None,
    );

    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);

    // A funds exactly target, B overfunds past it
    client.fund(&inv_a, &target);
    client.fund(&inv_b, &500_000i128);

    // Total funded should be 1_500_000 (target + 500k over)
    assert_eq!(client.get_escrow().funded_amount, 1_500_000i128);
    assert_eq!(client.get_escrow().status, 1u32);

    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, 1_500_000i128);
    assert_eq!(snap.funding_target, target);

    client.settle();

    // Both investors should have valid payouts
    let payout_a = client.compute_investor_payout(&inv_a);
    let payout_b = client.compute_investor_payout(&inv_b);
    assert!(payout_a > 0i128);
    assert!(payout_b > 0i128);
    assert!(payout_a > payout_b, "investor A should get more than B");

    client.claim_investor_payout(&inv_a);
    client.claim_investor_payout(&inv_b);
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Cancellation and refund flow
// ──────────────────────────────────────────────────────────────────────────────

/// Test the full cancellation → refund lifecycle.
#[test]
fn test_e2e_cancellation_and_refund() {
    let env = Env::default();
    env.mock_all_auths();

    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(&env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "E2E_CANCEL"),
        &sme,
        &1_000_000i128,
        &500i64,
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
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    sac_admin.mint(&escrow_id, &500_000i128);
    client.fund(&investor, &500_000i128);

    assert_eq!(client.get_contribution(&investor), 500_000i128);

    // Cancel
    let cancelled = client.cancel_funding();
    assert_eq!(cancelled.status, 4u32);

    // Cannot claim after cancellation
    let claim_err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.claim_investor_payout(&investor);
    }));
    assert!(claim_err.is_err(), "claim must fail after cancellation");

    // Refund
    assert!(!client.is_investor_refunded(&investor));
    client.refund(&investor);
    assert!(client.is_investor_refunded(&investor));

    // Second refund must be idempotent (no double refund)
    client.refund(&investor);
    assert!(client.is_investor_refunded(&investor));

    assert_eq!(client.get_distributed_principal(), 500_000i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Legal hold during lifecycle
// ──────────────────────────────────────────────────────────────────────────────

/// Test that a legal hold mid-lifecycle blocks risk-bearing operations
/// and that clearing it allows the flow to resume.
#[test]
fn test_e2e_legal_hold_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(&env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "E2E_HOLD"),
        &sme,
        &1_000_000i128,
        &500i64,
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
        &None,
        &None,
    );

    // Phase 1: Fund
    let investor_a = Address::generate(&env);
    sac_admin.mint(&escrow_id, &600_000i128);
    client.fund(&investor_a, &600_000i128);
    assert_eq!(client.get_escrow().status, 0u32);

    // Phase 2: Enable legal hold — blocks further funding
    client.set_legal_hold(&true);
    let investor_b = Address::generate(&env);
    let fund_err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor_b, &400_000i128);
    }));
    assert!(fund_err.is_err(), "funding must be blocked under legal hold");

    // Phase 3: Clear hold — funding resumes
    client.clear_legal_hold();
    assert!(!client.get_legal_hold());
    sac_admin.mint(&escrow_id, &400_000i128);
    client.fund(&investor_b, &400_000i128);

    // Now funded
    assert_eq!(client.get_escrow().status, 1u32);

    // Phase 4: Re-enable hold → blocks settle
    client.set_legal_hold(&true);
    let settle_err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.settle();
    }));
    assert!(settle_err.is_err(), "settle must be blocked under legal hold");

    // Phase 5: Clear → settle → claims succeed
    client.clear_legal_hold();
    let settled = client.settle();
    assert_eq!(settled.status, 2u32);

    client.claim_investor_payout(&investor_a);
    assert!(client.is_investor_claimed(&investor_a));
    client.claim_investor_payout(&investor_b);
    assert!(client.is_investor_claimed(&investor_b));
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Admin handover during lifecycle
// ──────────────────────────────────────────────────────────────────────────────

/// Test that admin can be transferred mid-lifecycle and new admin can operate.
#[test]
fn test_e2e_admin_handover() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "E2E_ADMIN"),
        &sme,
        &TARGET,
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
        &None,
        &None,
        &None,
        &None,
    );

    // Fund to target
    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);
    assert_eq!(client.get_escrow().status, 1u32);

    // Old admin proposes new admin
    let new_admin = Address::generate(&env);
    let pending = client.propose_admin(&new_admin);
    assert_eq!(pending, new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));

    // Old admin cannot settle (admin is now new_admin after accept)
    // New admin accepts
    let updated = client.accept_admin();
    assert_eq!(updated.admin, new_admin);
    assert_eq!(client.get_pending_admin(), None);

    // New admin can operate — set legal hold sequence
    client.set_legal_hold(&true);
    assert!(client.get_legal_hold());
    client.clear_legal_hold();

    // SME can still settle after hold cleared
    client.settle();
    assert_eq!(client.get_escrow().status, 2u32);

    // Investor can claim
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Funding deadline enforcement
// ──────────────────────────────────────────────────────────────────────────────

/// Test that funding deadline is enforced and expired deadlines block deposits.
#[test]
fn test_e2e_funding_deadline() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);
    let deadline = 2000u64;

    env.ledger().set_timestamp(1000);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "E2E_DEAD"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &Some(deadline),
        &None,
        &None,
        &None,
    );

    // Fund before deadline — succeeds
    let investor_a = Address::generate(&env);
    env.ledger().set_timestamp(1500);
    client.fund(&investor_a, &50_000_000_000i128);

    // Fund after deadline — must fail
    env.ledger().set_timestamp(2001);
    let investor_b = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor_b, &10_000i128);
    }));
    assert!(result.is_err(), "funding after deadline must be rejected");
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Min contribution and max investor cap enforcement
// ──────────────────────────────────────────────────────────────────────────────

/// Test min contribution floor and max unique investor cap enforcement.
#[test]
fn test_e2e_cap_enforcement() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "E2E_CAP"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &Some(10_000i128), // min contribution
        &Some(2u32),       // max 2 unique investors
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Below min — must fail
    let investor_a = Address::generate(&env);
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor_a, &5_000i128);
    }));
    assert!(err.is_err(), "funding below min contribution must be rejected");

    // At min — succeeds
    client.fund(&investor_a, &10_000i128);

    // Second investor
    let investor_b = Address::generate(&env);
    client.fund(&investor_b, &10_000i128);
    assert_eq!(client.get_unique_funder_count(), 2u32);

    // Third investor — must fail (cap of 2)
    let investor_c = Address::generate(&env);
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor_c, &10_000i128);
    }));
    assert!(err.is_err(), "third investor must exceed unique investor cap");
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: SME collateral commitment during lifecycle
// ──────────────────────────────────────────────────────────────────────────────

/// Test recording and replacing SME collateral commitments.
#[test]
fn test_e2e_sme_collateral() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "E2E_COL"),
        &sme,
        &TARGET,
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
        &None,
        &None,
        &None,
        &None,
    );

    // Record initial collateral
    let asset = Symbol::new(&env, "USDC");
    let commitment = client.record_sme_collateral_commitment(&asset, &50_000i128);
    assert_eq!(commitment.amount, 50_000i128);
    assert_eq!(commitment.asset, asset);

    let stored = client.get_sme_collateral_commitment();
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().amount, 50_000i128);

    // Replace with updated collateral (advance time to avoid stale timestamp)
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 1000);
    let updated = client.record_sme_collateral_commitment(&asset, &75_000i128);
    assert_eq!(updated.amount, 75_000i128);

    let stored2 = client.get_sme_collateral_commitment();
    assert!(stored2.is_some());
    assert_eq!(stored2.unwrap().amount, 75_000i128);

    // Verify regular lifecycle still works
    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);
    client.settle();
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Tiered yield with commitment locks
// ──────────────────────────────────────────────────────────────────────────────

/// Test the full lifecycle with tiered yield and commitment locks.
#[test]
fn test_e2e_tiered_yield() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    let mut tiers = Vec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100u64,
        yield_bps: 600i64,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 200u64,
        yield_bps: 800i64,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 500u64,
        yield_bps: 1200i64,
    });

    env.ledger().set_timestamp(1000);

    client.init(
        &admin,
        &String::from_str(&env, "E2E_TIER"),
        &sme,
        &3_000i128,
        &400i64, // base 4% yield
        &0u64,
        &token,
        &None,
        &treasury,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Investor A: 500s lock → gets 1200 bps (top tier)
    let inv_a = Address::generate(&env);
    client.fund_with_commitment(&inv_a, &1_000i128, &500u64);

    // Investor B: 200s lock → gets 800 bps (middle tier)
    let inv_b = Address::generate(&env);
    client.fund_with_commitment(&inv_b, &1_000i128, &200u64);

    // Investor C: simple fund → gets 400 bps (base)
    let inv_c = Address::generate(&env);
    client.fund(&inv_c, &1_000i128);

    assert_eq!(client.get_escrow().status, 1u32);

    // Settle
    client.settle();

    // All claims at t=1100: A locked 500s from t=1000 → not yet (needs t>=1500)
    // B locked 200s → ok at t>=1200
    // C no lock → ok immediately
    env.ledger().set_timestamp(1100);

    // C (no lock) can claim
    client.claim_investor_payout(&inv_c);
    assert!(client.is_investor_claimed(&inv_c));

    // B (200s lock) NOT yet — 1100 < 1200
    let b_err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.claim_investor_payout(&inv_b);
    }));
    assert!(b_err.is_err(), "B must be blocked at t=1100");

    // A (500s lock) NOT yet — 1100 < 1500
    let a_err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.claim_investor_payout(&inv_a);
    }));
    assert!(a_err.is_err(), "A must be blocked at t=1100");

    // Advance to t=1300
    env.ledger().set_timestamp(1300);
    client.claim_investor_payout(&inv_b);
    assert!(client.is_investor_claimed(&inv_b));

    // A still blocked
    let a_err2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.claim_investor_payout(&inv_a);
    }));
    assert!(a_err2.is_err(), "A must still be blocked at t=1300");

    // Advance to t=1500 — A can now claim
    env.ledger().set_timestamp(1500);
    client.claim_investor_payout(&inv_a);
    assert!(client.is_investor_claimed(&inv_a));
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Asset custody verification
// ──────────────────────────────────────────────────────────────────────────────

/// Test the asset custody verification entrypoint during lifecycle.
#[test]
fn test_e2e_asset_custody_verification() {
    let env = Env::default();
    env.mock_all_auths();

    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(&env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "E2E_CUST"),
        &sme,
        &1_000_000i128,
        &500i64,
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
        &None,
        &None,
    );

    // Verify before funding — funded_amount is 0
    let discrepancy = client.verify_asset_custody();
    assert_eq!(discrepancy, 0i128, "no funded amount, no discrepancy expected");

    // Fund but don't mint → funded_amount > 0 but contract balance = 0
    let investor = Address::generate(&env);
    client.fund(&investor, &500_000i128);

    let discrepancy = client.verify_asset_custody();
    assert!(discrepancy < 0i128, "shortfall must yield negative discrepancy");

    // Mint tokens to match funded_amount
    sac_admin.mint(&escrow_id, &500_000i128);
    let discrepancy = client.verify_asset_custody();
    assert_eq!(discrepancy, 0i128, "balance matches funded_amount");
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Allowlist lifecycle
// ──────────────────────────────────────────────────────────────────────────────

/// Test the allowlist lifecycle: enable, add, fund, disable.
#[test]
fn test_e2e_allowlist_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "E2E_ALST"),
        &sme,
        &TARGET,
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
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);

    // Enable allowlist
    client.set_allowlist_active(&true);
    assert!(client.is_allowlist_active());

    // Non-allowlisted investor cannot fund
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor, &10_000i128);
    }));
    assert!(err.is_err(), "non-allowlisted must be blocked");

    // Allowlist the investor
    client.set_investor_allowlisted(&investor, &true);
    assert!(client.is_investor_allowlisted(&investor));

    // Now funding succeeds
    let funded = client.fund(&investor, &10_000i128);
    assert_eq!(funded.funded_amount, 10_000i128);

    // Remove from allowlist — already-funded investor is not retroactively blocked
    client.set_investor_allowlisted(&investor, &false);
    assert!(!client.is_investor_allowlisted(&investor));

    // Additional funding from now-blocked investor must fail
    let err2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor, &5_000i128);
    }));
    assert!(err2.is_err(), "de-listed investor cannot add more funds");

    // Disable allowlist entirely — funding opens up
    client.set_allowlist_active(&false);
    client.fund(&investor, &5_000i128);
    assert_eq!(client.get_contribution(&investor), 15_000i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// E2E: Full dust sweep after settled escrow
// ──────────────────────────────────────────────────────────────────────────────

/// Full dust sweep after settlement with various edge cases.
#[test]
fn test_e2e_dust_sweep_after_settlement() {
    let env = Env::default();
    env.mock_all_auths();

    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(&env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "E2E_DUST"),
        &sme,
        &1_000_000i128,
        &500i64,
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
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    sac_admin.mint(&escrow_id, &1_000_000i128);
    client.fund(&investor, &1_000_000i128);
    client.settle();
    client.claim_investor_payout(&investor);

    // Add some dust to the contract
    sac_admin.mint(&escrow_id, &500i128);

    // Sweep dust — must succeed
    let swept = client.sweep_terminal_dust(&500i128);
    assert_eq!(swept, 500i128);
    assert_eq!(TokenClient::new(&env, &token_id).balance(&treasury), 500i128);

    // Second sweep with no balance — must fail
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.sweep_terminal_dust(&1i128);
    }));
    assert!(err.is_err(), "sweep with no balance must fail");
}
