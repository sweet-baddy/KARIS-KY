// Simulation entrypoint tests: verify dry-run behavior without state mutations.
//
// These tests verify that:
// 1. Simulation entrypoints perform the same guard checks as actual operations
// 2. Simulation results match what actual operations would produce
// 3. No state changes occur (idempotence)
// 4. No authorization required

use karis_ky_escrow::{LiquifactEscrow, LiquifactEscrowClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env};

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

fn default_init(client: &LiquifactEscrowClient<'_>, env: &Env, admin: &Address, sme: &Address) {
    let (token, treasury) = free_addresses(env);
    client.init(
        admin,
        &soroban_sdk::String::from_str(env, "INV_SIM_001"),
        sme,
        &100_000_000_000i128,
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
}

const TARGET: i128 = 100_000_000_000i128;

// ============================================================================
// Test: simulate_fund behavior and idempotence
// ============================================================================

/// Test that simulate_fund returns correct projected state without persisting changes.
#[test]
fn test_simulate_fund_partial() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    let amount = TARGET / 2;

    // Simulate the funding
    let simulated = client.simulate_fund(&investor, &amount);

    // Verify projected state
    assert_eq!(simulated.funded_amount, amount);
    assert_eq!(simulated.status, 0); // Not yet funded
    assert_eq!(simulated.amount, TARGET);

    // Verify idempotence: second simulation gives same result
    let simulated2 = client.simulate_fund(&investor, &amount);
    assert_eq!(simulated, simulated2);

    // Verify no persistence: actually fund and check it persists now
    let actual = client.fund(&investor, &amount);
    assert_eq!(actual.funded_amount, simulated.funded_amount);
}

/// Test that simulate_fund detects non-positive amounts.
#[test]
fn test_simulate_fund_zero_amount_fails() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.simulate_fund(&investor, &0i128);
    }));
    assert!(result.is_err(), "simulate_fund should reject zero amount");
}

/// Test that simulate_fund reaches funded status at target.
#[test]
fn test_simulate_fund_reaches_funded() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);

    // Actually fund first investor
    let amount1 = TARGET * 60 / 100;
    client.fund(&investor1, &amount1);

    // Simulate second funding: reaches target
    let amount2 = TARGET * 40 / 100;
    let sim2 = client.simulate_fund(&investor2, &amount2);
    assert_eq!(sim2.funded_amount, TARGET);
    assert_eq!(sim2.status, 1); // Funded!
}

/// Test that simulate_fund is idempotent: same result on repeated calls.
#[test]
fn test_simulate_fund_idempotent() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    let amount = 25_000_000_000i128;

    let sim1 = client.simulate_fund(&investor, &amount);
    let sim2 = client.simulate_fund(&investor, &amount);
    let sim3 = client.simulate_fund(&investor, &amount);

    assert_eq!(sim1, sim2);
    assert_eq!(sim2, sim3);
}

/// Test that simulate_fund works without any authorization.
#[test]
fn test_simulate_fund_no_auth_required() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // Clear all auths
    env.mock_auths(&[]);

    let investor = Address::generate(&env);
    let amount = 50_000_000_000i128;

    // Should succeed without auth
    let simulated = client.simulate_fund(&investor, &amount);
    assert_eq!(simulated.funded_amount, amount);
}

// ============================================================================
// Test: simulate_settle behavior and guards
// ============================================================================

/// Test that simulate_settle returns status=2 without persisting.
#[test]
fn test_simulate_settle_success() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);

    // Simulate settlement
    let simulated = client.simulate_settle();

    assert_eq!(simulated.status, 2); // Settled
    assert_eq!(simulated.funded_amount, TARGET);

    // Verify persistence didn't happen
    let current = client.get_escrow();
    assert_eq!(current.status, 1); // Still funded, not settled
}

/// Test that simulate_settle rejects un-funded escrows.
#[test]
fn test_simulate_settle_requires_funded() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // Try to settle without funding
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.simulate_settle();
    }));
    assert!(
        result.is_err(),
        "simulate_settle should fail for un-funded escrow"
    );
}

/// Test that simulate_settle respects maturity.
#[test]
fn test_simulate_settle_maturity_not_reached() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    let (token, treasury) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SIM_MATURE"),
        &sme,
        &TARGET,
        &800i64,
        &99999u64, // Far in the future (beyond current ledger time of 12345)
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

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);

    // Try to settle before maturity
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.simulate_settle();
    }));
    assert!(
        result.is_err(),
        "simulate_settle should fail before maturity"
    );
}

/// Test that simulate_settle is idempotent.
#[test]
fn test_simulate_settle_idempotent() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);

    let sim1 = client.simulate_settle();
    let sim2 = client.simulate_settle();
    let sim3 = client.simulate_settle();

    assert_eq!(sim1, sim2);
    assert_eq!(sim2, sim3);
}

/// Test that simulate_settle works without SME auth.
#[test]
fn test_simulate_settle_no_sme_auth_required() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);

    // Clear all auths
    env.mock_auths(&[]);

    // Should succeed without SME auth
    let simulated = client.simulate_settle();
    assert_eq!(simulated.status, 2);
}

// ============================================================================
// Test: simulate_claim_investor_payout behavior
// ============================================================================

/// Test that simulate_claim returns zero for non-participants.
#[test]
fn test_simulate_claim_non_participant() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    let non_participant = Address::generate(&env);

    // Fund from one investor
    client.fund(&investor, &TARGET);

    // Simulate claim for non-participant
    let payout = client.simulate_claim_investor_payout(&non_participant);
    assert_eq!(payout, 0, "non-participant should have zero payout");
}

/// Test that simulate_claim returns expected payout for valid investor.
#[test]
fn test_simulate_claim_valid_investor() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);

    // Two equal investors
    let half = TARGET / 2;
    client.fund(&investor1, &half);
    client.fund(&investor2, &half);

    // Settle to create snapshot
    client.settle();

    // Each should get approximately half the settlement pool
    let payout1 = client.simulate_claim_investor_payout(&investor1);
    let payout2 = client.simulate_claim_investor_payout(&investor2);

    assert!(payout1 > 0, "investor1 should have positive payout");
    assert!(payout2 > 0, "investor2 should have positive payout");
    // Due to rounding, they should be equal or very close
    assert!(
        (payout1 - payout2).abs() <= 1,
        "equal investors should have equal payouts"
    );
}

/// Test that simulate_claim works without investor auth.
#[test]
fn test_simulate_claim_no_auth_required() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);
    client.settle();

    // Clear all auths
    env.mock_auths(&[]);

    // Should succeed without auth
    let payout = client.simulate_claim_investor_payout(&investor);
    assert!(payout > 0);
}

/// Test that simulate_claim is idempotent.
#[test]
fn test_simulate_claim_idempotent() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);
    client.settle();

    let payout1 = client.simulate_claim_investor_payout(&investor);
    let payout2 = client.simulate_claim_investor_payout(&investor);
    let payout3 = client.simulate_claim_investor_payout(&investor);

    assert_eq!(payout1, payout2);
    assert_eq!(payout2, payout3);
}

// ============================================================================
// Integration tests: simulate vs actual operation consistency
// ============================================================================

/// Test that simulate_fund result matches actual fund operation (for the escrow state).
#[test]
fn test_simulate_fund_matches_actual() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    let amount = 30_000_000_000i128;

    // Simulate
    let simulated = client.simulate_fund(&investor, &amount);

    // Perform actual funding
    let actual = client.fund(&investor, &amount);

    // Escrow state should match
    assert_eq!(simulated.funded_amount, actual.funded_amount);
    assert_eq!(simulated.status, actual.status);
    assert_eq!(simulated.amount, actual.amount);
}

/// Test that simulate_settle result matches actual settle operation.
#[test]
fn test_simulate_settle_matches_actual() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);

    // Simulate
    let simulated = client.simulate_settle();

    // Perform actual settlement
    let actual = client.settle();

    // Escrow state should match
    assert_eq!(simulated.status, actual.status);
    assert_eq!(simulated.funded_amount, actual.funded_amount);
}

/// Test that simulate operations don't affect actual subsequent operations.
#[test]
fn test_simulate_does_not_affect_actual() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    let amount = 50_000_000_000i128;

    // Simulate many times
    for _ in 0..10 {
        let _sim = client.simulate_fund(&investor, &amount);
    }

    // Get state after simulations
    let state1 = client.get_escrow();

    // Now actually fund
    client.fund(&investor, &amount);
    let state2 = client.get_escrow();

    // Verify actual fund changed the state
    assert_eq!(
        state1.funded_amount, 0,
        "simulations should not change state"
    );
    assert_eq!(
        state2.funded_amount, amount,
        "actual fund should change state"
    );
}
