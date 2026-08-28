//! Real-time yield slippage detection tests for the karis-ky escrow contract.
//!
//! Covers the yield slippage anomaly detection feature (#232):
//! - Configuration of `yield_slippage_threshold` at initialization
//! - Real-time slippage checks during investor claim
//! - Event emission when deviation exceeds threshold
//! - Edge cases: threshold bounds, disabled detection, zero threshold

#[cfg(test)]
use super::{default_init, setup, TARGET};
use crate::{EscrowError, LiquifactEscrow, YieldTier};
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, String, Vec,
};

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Initialize an escrow with a yield slippage threshold.
fn init_with_slippage_threshold(
    client: &super::LiquifactEscrowClient<'_>,
    env: &Env,
    admin: &Address,
    sme: &Address,
    yield_bps: i64,
    threshold_bps: i64,
) {
    client.init(
        &admin,
        &soroban_sdk::String::from_str(env, "INV_SLIP"),
        &sme,
        &TARGET,
        &yield_bps,
        &0u64,
        &Address::generate(env),
        &None,
        &Address::generate(env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &Some(threshold_bps),
        &None,
        &None,
    );
}

/// Initialize an escrow with yield tiers and a slippage threshold.
fn init_with_tiers_and_slippage(
    client: &super::LiquifactEscrowClient<'_>,
    env: &Env,
    admin: &Address,
    sme: &Address,
    base_yield_bps: i64,
    tiers: Vec<YieldTier>,
    threshold_bps: i64,
) {
    client.init(
        &admin,
        &soroban_sdk::String::from_str(env, "TIER_SLIP"),
        &sme,
        &TARGET,
        &base_yield_bps,
        &0u64,
        &Address::generate(env),
        &None,
        &Address::generate(env),
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &Some(threshold_bps),
        &None,
        &None,
    );
}

/// Fund an escrow, settle it, and return the investor address.
fn fund_settle_and_return_investor(
    client: &super::LiquifactEscrowClient<'_>,
    env: &Env,
) -> Address {
    let investor = Address::generate(env);
    client.fund(&investor, &TARGET);
    client.settle();
    investor
}

// ──────────────────────────────────────────────────────────────────────────────
// Init validation
// ──────────────────────────────────────────────────────────────────────────────

/// Threshold must be within valid range (0..=10_000 bps).
#[test]
#[should_panic]
fn init_rejects_threshold_above_max() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    // 10_001 bps exceeds the valid range [0..=10_000]
    init_with_slippage_threshold(&client, &env, &admin, &sme, 800i64, 10_001i64);
}

/// Threshold can be zero (disables slippage detection).
#[test]
fn init_accepts_zero_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    // Zero threshold is valid and disables detection
    init_with_slippage_threshold(&client, &env, &admin, &sme, 800i64, 0i64);

    let threshold = client.get_yield_slippage_threshold();
    assert_eq!(threshold, 0i64, "zero threshold should disable detection");
}

/// Threshold can be up to 10_000 bps (100%).
#[test]
fn init_accepts_max_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    // 10_000 bps is the maximum valid threshold (100%)
    init_with_slippage_threshold(&client, &env, &admin, &sme, 800i64, 10_000i64);

    let threshold = client.get_yield_slippage_threshold();
    assert_eq!(threshold, 10_000i64, "max threshold should be stored");
}

// ──────────────────────────────────────────────────────────────────────────────
// Slippage query function
// ──────────────────────────────────────────────────────────────────────────────

/// `get_yield_slippage_threshold()` returns configured threshold.
#[test]
fn get_threshold_returns_configured_value() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    init_with_slippage_threshold(&client, &env, &admin, &sme, 800i64, 150i64);

    let threshold = client.get_yield_slippage_threshold();
    assert_eq!(threshold, 150i64, "threshold should return configured value");
}

/// `get_yield_slippage_threshold()` returns 0 when not configured.
#[test]
fn get_threshold_returns_zero_when_unconfigured() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    // Initialize without explicit threshold (uses default None)
    default_init(&client, &env, &admin, &sme);

    let threshold = client.get_yield_slippage_threshold();
    assert_eq!(threshold, 0i64, "unconfigured threshold should default to 0");
}

/// `get_investor_yield_slippage()` computes expected, actual, and deviation.
#[test]
fn get_investor_yield_slippage_returns_correct_values() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    init_with_slippage_threshold(&client, &env, &admin, &sme, 800i64, 150i64);

    let investor = Address::generate(&env);
    // When investor has no deposit yet, effective yield is the base yield
    let (expected, actual, deviation) = client.get_investor_yield_slippage(&investor);

    assert_eq!(expected, 800i64, "expected yield should be base yield");
    assert_eq!(actual, 800i64, "actual yield should fall back to base");
    assert_eq!(deviation, 0i64, "deviation should be zero when yields match");
}

// ──────────────────────────────────────────────────────────────────────────────
// Slippage detection during claim
// ──────────────────────────────────────────────────────────────────────────────

/// When slippage detection is disabled (threshold = 0), claim succeeds without warning.
#[test]
fn claim_with_disabled_slippage_detection() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    // Initialize with zero threshold (disabled)
    init_with_slippage_threshold(&client, &env, &admin, &sme, 800i64, 0i64);

    let investor = fund_settle_and_return_investor(&client, &env);
    client.claim_investor_payout(&investor);

    // Should succeed without emitting a slippage warning
    let events = env.events().all();
    let slippage_events: Vec<_> = events
        .events()
        .iter()
        .filter(|evt| evt.topics.get(0).map_or(false, |t| {
            t.to_string().contains("yield_slip")
        }))
        .collect();
    assert_eq!(
        slippage_events.len(),
        0,
        "no slippage warning should be emitted when detection is disabled"
    );
}

/// When actual yield matches expected, no slippage warning is emitted.
#[test]
fn claim_with_matching_yields_no_warning() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    // Base yield 800 bps, threshold 150 bps
    init_with_slippage_threshold(&client, &env, &admin, &sme, 800i64, 150i64);

    let investor = fund_settle_and_return_investor(&client, &env);
    client.claim_investor_payout(&investor);

    // Actual yield equals base yield (no tier deviation), so no warning
    let events = env.events().all();
    let slippage_events: Vec<_> = events
        .events()
        .iter()
        .filter(|evt| evt.topics.get(0).map_or(false, |t| {
            t.to_string().contains("yield_slip")
        }))
        .collect();
    assert_eq!(
        slippage_events.len(),
        0,
        "no warning when deviation is 0 (matches base yield)"
    );
}

/// When deviation exceeds threshold, a slippage warning is emitted.
#[test]
fn claim_with_excessive_slippage_emits_warning() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    let base_yield = 800i64;
    let threshold = 100i64;
    let mut tiers = Vec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 0u64,
        yield_bps: 1200i64, // Deviation of 400 bps > threshold of 100 bps
    });

    init_with_tiers_and_slippage(&client, &env, &admin, &sme, base_yield, tiers, threshold);

    let investor = Address::generate(&env);
    // Fund with commitment to get the higher yield
    client.fund_with_commitment(&investor, &TARGET, &0u64);
    client.settle();
    client.claim_investor_payout(&investor);

    // Should emit a slippage warning event
    let events = env.events().all();
    let slippage_events: Vec<_> = events
        .events()
        .iter()
        .filter(|evt| evt.topics.get(0).map_or(false, |t| {
            t.to_string().contains("yield_slip")
        }))
        .collect();

    assert_eq!(
        slippage_events.len(),
        1,
        "exactly one slippage warning should be emitted when deviation exceeds threshold"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Slippage detection edge cases
// ──────────────────────────────────────────────────────────────────────────────

/// When deviation exactly equals threshold, warning is **not** emitted (requires > threshold).
#[test]
fn claim_with_deviation_equal_to_threshold_no_warning() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    let base_yield = 800i64;
    let threshold = 200i64;
    let mut tiers = Vec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 0u64,
        yield_bps: 1000i64, // Deviation of exactly 200 bps == threshold
    });

    init_with_tiers_and_slippage(&client, &env, &admin, &sme, base_yield, tiers, threshold);

    let investor = Address::generate(&env);
    client.fund_with_commitment(&investor, &TARGET, &0u64);
    client.settle();
    client.claim_investor_payout(&investor);

    let events = env.events().all();
    let slippage_events: Vec<_> = events
        .events()
        .iter()
        .filter(|evt| evt.topics.get(0).map_or(false, |t| {
            t.to_string().contains("yield_slip")
        }))
        .collect();

    assert_eq!(
        slippage_events.len(),
        0,
        "no warning when deviation equals threshold (requires > threshold)"
    );
}

/// When deviation is just above threshold, warning is emitted.
#[test]
fn claim_with_deviation_above_threshold_emits_warning() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    let base_yield = 800i64;
    let threshold = 200i64;
    let mut tiers = Vec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 0u64,
        yield_bps: 1001i64, // Deviation of 201 bps > threshold of 200 bps
    });

    init_with_tiers_and_slippage(&client, &env, &admin, &sme, base_yield, tiers, threshold);

    let investor = Address::generate(&env);
    client.fund_with_commitment(&investor, &TARGET, &0u64);
    client.settle();
    client.claim_investor_payout(&investor);

    let events = env.events().all();
    let slippage_events: Vec<_> = events
        .events()
        .iter()
        .filter(|evt| evt.topics.get(0).map_or(false, |t| {
            t.to_string().contains("yield_slip")
        }))
        .collect();

    assert_eq!(
        slippage_events.len(),
        1,
        "warning should be emitted when deviation just exceeds threshold"
    );
}

/// Idempotency: second claim does not re-emit warning.
#[test]
fn claim_idempotency_no_duplicate_warnings() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    let base_yield = 800i64;
    let threshold = 100i64;
    let mut tiers = Vec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 0u64,
        yield_bps: 1200i64, // Deviation of 400 bps > threshold
    });

    init_with_tiers_and_slippage(&client, &env, &admin, &sme, base_yield, tiers, threshold);

    let investor = Address::generate(&env);
    client.fund_with_commitment(&investor, &TARGET, &0u64);
    client.settle();

    // First claim
    client.claim_investor_payout(&investor);
    let events_after_first = env.events().all().events().len();

    // Second claim (should be idempotent)
    client.claim_investor_payout(&investor);
    let events_after_second = env.events().all().events().len();

    // No new events should be emitted on the second claim
    assert_eq!(
        events_after_first, events_after_second,
        "second claim should not re-emit events"
    );
}
