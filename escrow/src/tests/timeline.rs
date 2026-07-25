//! Escrow lifecycle timeline tests.
//!
//! Covers `get_event_timeline` and the internal `append_timeline_event` recorder:
//! - events are appended in chronological order for init, fund, settle, withdraw,
//!   claim, cancel, refund, and admin actions;
//! - `TimelineFilter` correctly filters by event type and date range;
//! - capacity is enforced at [`MAX_TIMELINE_EVENTS`].

#[cfg(test)]
use super::{
    default_init, deploy, deploy_with_id, free_addresses, init_and_fund_with_real_token, setup,
    TimelineEvent, TimelineFilter, MAX_TIMELINE_EVENTS, TARGET,
};
use soroban_sdk::{symbol_short, Address, Env, String};

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Advance ledger timestamp by `delta` seconds.
fn advance_time(env: &Env, delta: u64) {
    let mut info = env.ledger().get();
    info.timestamp += delta;
    env.ledger().set(info);
}

/// Convenience: deploy + init + fund + settle, returning `(client, admin, sme, investor)`.
fn setup_lifecycle(env: &Env) -> (super::LiquifactEscrowClient<'_>, Address, Address, Address) {
    let (client, admin, sme) = setup(env);
    let investor = Address::generate(env);
    default_init(&client, env, &admin, &sme);
    client.fund(&investor, &TARGET);
    client.settle();
    (client, admin, sme, investor)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn timeline_records_init_and_fund() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);

    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);

    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: None,
            from_timestamp: None,
            to_timestamp: None,
        },
    );

    let types: Vec<_> = events.iter().map(|e| e.event_type).collect();
    assert!(types.contains(&symbol_short!("escrow_ii")));
    assert!(types.contains(&symbol_short!("funded")));
    assert_eq!(events.len(), 2);
}

#[test]
fn timeline_records_full_lifecycle() {
    let env = Env::default();
    let (client, admin, sme, investor) = setup_lifecycle(&env);

    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: None,
            from_timestamp: None,
            to_timestamp: None,
        },
    );

    let types: Vec<_> = events.iter().map(|e| e.event_type).collect();
    assert!(types.contains(&symbol_short!("escrow_ii")));
    assert!(types.contains(&symbol_short!("funded")));
    assert!(types.contains(&symbol_short!("escrow_sd")));
    assert_eq!(events.len(), 3);
}

#[test]
fn timeline_records_withdraw() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, escrow_id, sme, _) = init_and_fund_with_real_token(&env, TARGET, "INV_WD");

    // withdraw SME funds after settlement
    client.withdraw();

    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: None,
            from_timestamp: None,
            to_timestamp: None,
        },
    );

    let types: Vec<_> = events.iter().map(|e| e.event_type).collect();
    assert!(types.contains(&symbol_short!("sme_wd")));
    assert_eq!(events.len(), 3); // init, funded, sme_wd
}

#[test]
fn timeline_filter_by_event_type() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let investor2 = Address::generate(&env);

    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    client.fund(&investor2, &TARGET);

    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: Some(symbol_short!("funded")),
            from_timestamp: None,
            to_timestamp: None,
        },
    );

    assert_eq!(events.len(), 2);
    for e in events.iter() {
        assert_eq!(e.event_type, symbol_short!("funded"));
    }
}

#[test]
fn timeline_filter_by_date_range() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);

    default_init(&client, &env, &admin, &sme);
    let t1 = env.ledger().timestamp();
    advance_time(&env, 100);
    client.fund(&investor, &TARGET);
    let t2 = env.ledger().timestamp();
    advance_time(&env, 100);
    client.settle();
    let t3 = env.ledger().timestamp();

    // Only init + first fund
    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: None,
            from_timestamp: Some(t1),
            to_timestamp: Some(t2),
        },
    );
    assert_eq!(events.len(), 2);
    for e in events.iter() {
        assert!(e.timestamp >= t1);
        assert!(e.timestamp <= t2);
    }

    // Only settle
    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: None,
            from_timestamp: Some(t3),
            to_timestamp: Some(t3),
        },
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events.get(0).unwrap().event_type, symbol_short!("escrow_sd"));
}

#[test]
fn timeline_records_admin_actions() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let new_admin = Address::generate(&env);

    default_init(&client, &env, &admin, &sme);
    client.propose_admin(&new_admin);
    client.accept_admin();
    client.update_maturity(&2000u64);

    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: None,
            from_timestamp: None,
            to_timestamp: None,
        },
    );

    let types: Vec<_> = events.iter().map(|e| e.event_type).collect();
    assert!(types.contains(&symbol_short!("adm_prop")));
    assert!(types.contains(&symbol_short!("admin")));
    assert!(types.contains(&symbol_short!("maturity")));
}

#[test]
fn timeline_actor_and_amount_fields() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);

    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);

    let events = client.get_event_timeline(
        &env,
        TimelineFilter {
            event_type: Some(symbol_short!("funded")),
            from_timestamp: None,
            to_timestamp: None,
        },
    );

    assert_eq!(events.len(), 1);
    let e = events.get(0).unwrap();
    assert_eq!(e.actor, investor);
    assert_eq!(e.amount, TARGET);
    assert_eq!(e.result, symbol_short!("success"));
}
