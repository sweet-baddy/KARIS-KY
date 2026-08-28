//! Tests for [`LiquifactEscrow::init_from_template`], [`LiquifactEscrow::register_template`],
//! and [`LiquifactEscrow::get_template`].

use crate::{EscrowTemplate, LiquifactEscrow, LiquifactEscrowClient};
use soroban_sdk::{testutils::{Address as _, Ledger as _}, Address, Env, String};

// ── helpers ──────────────────────────────────────────────────────────────────

fn deploy(env: &Env) -> LiquifactEscrowClient<'_> {
    let id = env.register(LiquifactEscrow, ());
    LiquifactEscrowClient::new(env, &id)
}

fn setup(env: &Env) -> (LiquifactEscrowClient<'_>, Address, Address, Address, Address) {
    let mut info = env.ledger().get();
    info.timestamp = 100_000;
    env.ledger().set(info);
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let token = Address::generate(env);
    let treasury = Address::generate(env);
    (client, admin, sme, token, treasury)
}

// ── built-in template resolution ─────────────────────────────────────────────

#[test]
fn test_get_template_fast() {
    let env = Env::default();
    let client = deploy(&env);
    let tmpl = client
        .get_template(&String::from_str(&env, "fast"))
        .expect("fast template must exist");
    assert_eq!(tmpl.yield_bps, 200);
    assert_eq!(tmpl.maturity_secs, 259_200);
    assert!(tmpl.min_contribution.is_none());
    assert!(tmpl.max_unique_investors.is_none());
    assert!(tmpl.max_per_investor.is_none());
    assert!(tmpl.yield_tiers.is_none());
}

#[test]
fn test_get_template_standard() {
    let env = Env::default();
    let client = deploy(&env);
    let tmpl = client
        .get_template(&String::from_str(&env, "standard"))
        .expect("standard template must exist");
    assert_eq!(tmpl.yield_bps, 500);
    assert_eq!(tmpl.maturity_secs, 1_209_600);
}

#[test]
fn test_get_template_conservative() {
    let env = Env::default();
    let client = deploy(&env);
    let tmpl = client
        .get_template(&String::from_str(&env, "conservative"))
        .expect("conservative template must exist");
    assert_eq!(tmpl.yield_bps, 300);
    assert_eq!(tmpl.maturity_secs, 2_592_000);
}

#[test]
fn test_get_template_unknown_returns_none() {
    let env = Env::default();
    let client = deploy(&env);
    // An unregistered custom name should return None.
    let result = client.get_template(&String::from_str(&env, "nonexistent"));
    assert!(result.is_none());
}

// ── init_from_template: built-ins ────────────────────────────────────────────

#[test]
fn test_init_from_template_fast() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    let escrow = client.init_from_template(
        &String::from_str(&env, "fast"),
        &String::from_str(&env, "INV_FAST"),
        &admin,
        &sme,
        &1_000_000i128,
        &token,
        &None,
        &treasury,
    );

    assert_eq!(escrow.yield_bps, 200);
    // maturity should be now (100_000) + 3 days (259_200) = 359_200
    assert_eq!(escrow.maturity, 359_200u64);
    assert_eq!(escrow.amount, 1_000_000i128);
    assert_eq!(escrow.status, 0);
}

#[test]
fn test_init_from_template_standard() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    let escrow = client.init_from_template(
        &String::from_str(&env, "standard"),
        &String::from_str(&env, "INV_STD"),
        &admin,
        &sme,
        &5_000_000i128,
        &token,
        &None,
        &treasury,
    );

    assert_eq!(escrow.yield_bps, 500);
    assert_eq!(escrow.maturity, 100_000 + 1_209_600);
    assert_eq!(escrow.amount, 5_000_000i128);
}

#[test]
fn test_init_from_template_conservative() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    let escrow = client.init_from_template(
        &String::from_str(&env, "conservative"),
        &String::from_str(&env, "INV_CON"),
        &admin,
        &sme,
        &10_000_000i128,
        &token,
        &None,
        &treasury,
    );

    assert_eq!(escrow.yield_bps, 300);
    assert_eq!(escrow.maturity, 100_000 + 2_592_000);
    assert_eq!(escrow.amount, 10_000_000i128);
}

// ── init_from_template: registry hint is forwarded ───────────────────────────

#[test]
fn test_init_from_template_with_registry() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);
    let registry = Address::generate(&env);

    client.init_from_template(
        &String::from_str(&env, "fast"),
        &String::from_str(&env, "INV_REG"),
        &admin,
        &sme,
        &1_000_000i128,
        &token,
        &Some(registry.clone()),
        &treasury,
    );

    let stored_registry = client.get_registry_ref();
    assert_eq!(stored_registry, Some(registry));
}

// ── register_template + custom template lookup ───────────────────────────────

#[test]
fn test_register_and_use_custom_template() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    // The escrow must be initialised before register_template can be called
    // (it calls load_escrow_require_admin internally).
    client.init_from_template(
        &String::from_str(&env, "fast"),
        &String::from_str(&env, "INV_SEED"),
        &admin,
        &sme,
        &1_000i128,
        &token,
        &None,
        &treasury,
    );

    // This escrow is already init'd; create a fresh one to test the template.
    // Register the template on the first client (which has admin set) so we
    // can verify the DataKey is stored and read back via get_template.
    let custom = EscrowTemplate {
        yield_bps: 750,
        maturity_secs: 604_800, // 7 days
        min_contribution: Some(500i128),
        max_unique_investors: Some(50u32),
        max_per_investor: None,
        legal_hold_clear_delay: None,
        funding_deadline_secs: None,
        yield_tiers: None,
    };
    client.register_template(&String::from_str(&env, "weekly_high"), &custom);

    let fetched = client
        .get_template(&String::from_str(&env, "weekly_high"))
        .expect("custom template must be present after registration");
    assert_eq!(fetched.yield_bps, 750);
    assert_eq!(fetched.maturity_secs, 604_800);
    assert_eq!(fetched.min_contribution, Some(500i128));
    assert_eq!(fetched.max_unique_investors, Some(50u32));
}

#[test]
#[should_panic(expected = "cannot override built-in template")]
fn test_register_template_cannot_override_builtin_fast() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    client.init_from_template(
        &String::from_str(&env, "fast"),
        &String::from_str(&env, "INV_B"),
        &admin,
        &sme,
        &1_000i128,
        &token,
        &None,
        &treasury,
    );

    let tmpl = EscrowTemplate {
        yield_bps: 9_999,
        maturity_secs: 1,
        min_contribution: None,
        max_unique_investors: None,
        max_per_investor: None,
        legal_hold_clear_delay: None,
        funding_deadline_secs: None,
        yield_tiers: None,
    };
    // Must panic — built-in names are protected.
    client.register_template(&String::from_str(&env, "fast"), &tmpl);
}

#[test]
#[should_panic(expected = "unknown template")]
fn test_init_from_template_unknown_panics() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    client.init_from_template(
        &String::from_str(&env, "no_such_template"),
        &String::from_str(&env, "INV_X"),
        &admin,
        &sme,
        &1_000i128,
        &token,
        &None,
        &treasury,
    );
}

// ── init_from_template reuses init validation ─────────────────────────────────

#[test]
#[should_panic]
fn test_init_from_template_rejects_zero_amount() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    // amount == 0 is rejected by init's AmountMustBePositive guard.
    client.init_from_template(
        &String::from_str(&env, "fast"),
        &String::from_str(&env, "INV_Z"),
        &admin,
        &sme,
        &0i128,
        &token,
        &None,
        &treasury,
    );
}

#[test]
#[should_panic]
fn test_init_from_template_rejects_already_initialized() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    client.init_from_template(
        &String::from_str(&env, "fast"),
        &String::from_str(&env, "INV_DUP"),
        &admin,
        &sme,
        &1_000i128,
        &token,
        &None,
        &treasury,
    );

    // Second init on the same contract must fail.
    client.init_from_template(
        &String::from_str(&env, "fast"),
        &String::from_str(&env, "INV_DUP2"),
        &admin,
        &sme,
        &1_000i128,
        &token,
        &None,
        &treasury,
    );
}

// ── template with maturity_secs == 0 produces no maturity lock ───────────────

#[test]
fn test_register_template_no_maturity_lock() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = setup(&env);

    // First init the escrow so register_template can auth against it.
    // We'll use a fresh escrow contract for the actual no-lock check.
    let client2 = {
        let id = env.register(LiquifactEscrow, ());
        LiquifactEscrowClient::new(&env, &id)
    };

    // Register a zero-maturity custom template on the second contract.
    // Must init it first so the admin check passes.
    client2.init_from_template(
        &String::from_str(&env, "standard"),
        &String::from_str(&env, "SEED2"),
        &admin,
        &sme,
        &1_000i128,
        &token,
        &None,
        &treasury,
    );

    let no_lock = EscrowTemplate {
        yield_bps: 100,
        maturity_secs: 0,
        min_contribution: None,
        max_unique_investors: None,
        max_per_investor: None,
        legal_hold_clear_delay: None,
        funding_deadline_secs: None,
        yield_tiers: None,
    };
    client2.register_template(&String::from_str(&env, "instant"), &no_lock);

    let fetched = client2
        .get_template(&String::from_str(&env, "instant"))
        .unwrap();
    assert_eq!(fetched.maturity_secs, 0);

    // Deploy a fresh contract and initialise it from the zero-maturity template
    // to verify that maturity_secs == 0 produces no maturity lock.
    // (client2 was initialised from "standard" with a 14-day maturity, so its
    // has_maturity_lock() would return true — we need a separate instance.)
    let client3 = {
        let id = env.register(LiquifactEscrow, ());
        LiquifactEscrowClient::new(&env, &id)
    };
    client3.init(
        &admin,
        &String::from_str(&env, "NOLOCK"),
        &sme,
        &1_000i128,
        &100i64, // yield_bps from the template
        &0u64,   // maturity == 0  ← the key assertion
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None, // yield_slippage_threshold
        &None, // yield_token
        &None, // oracle_contract
        &None, // nft_contract
    );
    assert!(!client3.has_maturity_lock());
}
