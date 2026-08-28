// State snapshot tests: create_state_snapshot and revert_to_snapshot functionality.
//
// Tests cover:
// - Snapshot creation with valid names, metadata capture, and event emission
// - Snapshot revert restoring escrow state
// - Error handling (invalid names, not found, unauthorized, etc.)
// - Edge cases (empty names, too-long names, non-alphanumeric characters)
// - Multiple snapshots and overwrites
// - State consistency across revert

use crate::String;
use soroban_sdk::Address;

#[test]
fn test_create_snapshot_success() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let snapshot_name = String::from_str(&env, "backup_1");
    client.create_state_snapshot(&snapshot_name);

    // Verify event was emitted
    let events = env.events().all();
    assert!(!events.is_empty(), "No events emitted");
}

#[test]
fn test_create_snapshot_captures_escrow_state() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &50_000_000_000i128);

    let snapshot_name = String::from_str(&env, "funded");
    client.create_state_snapshot(&snapshot_name);

    let escrow = client.get_escrow();
    assert_eq!(escrow.funded_amount, 50_000_000_000i128);
    assert_eq!(escrow.status, 0); // open
}

#[test]
#[should_panic]
fn test_create_snapshot_invalid_name_empty() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let empty_name = String::from_str(&env, "");
    client.create_state_snapshot(&empty_name);
}

#[test]
#[should_panic]
fn test_create_snapshot_invalid_name_too_long() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let long_name = String::from_str(&env, "this_is_a_very_long_snapshot_name_exceeding_limit");
    client.create_state_snapshot(&long_name);
}

#[test]
#[should_panic]
fn test_create_snapshot_invalid_name_bad_chars() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let bad_name = String::from_str(&env, "backup-1");
    client.create_state_snapshot(&bad_name);
}

#[test]
#[should_panic]
fn test_create_snapshot_unauthorized() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let unauthorized = Address::generate(&env);
    env.as_contract(&unauthorized, || {
        let snapshot_name = String::from_str(&env, "backup");
        client.create_state_snapshot(&snapshot_name);
    });
}

#[test]
#[should_panic]
fn test_create_snapshot_not_initialized() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let client = super::deploy(&env);

    let snapshot_name = String::from_str(&env, "backup");
    client.create_state_snapshot(&snapshot_name);
}

#[test]
fn test_revert_snapshot_success() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    client.fund(&investor, &50_000_000_000i128);
    assert_eq!(client.get_escrow().funded_amount, 50_000_000_000i128);

    let snapshot_name = String::from_str(&env, "pre_fund");
    client.create_state_snapshot(&snapshot_name);

    // Modify state by funding more
    let investor2 = Address::generate(&env);
    client.fund(&investor2, &30_000_000_000i128);
    assert_eq!(client.get_escrow().funded_amount, 80_000_000_000i128);

    // Revert to snapshot
    client.revert_to_snapshot(&snapshot_name);
    assert_eq!(client.get_escrow().funded_amount, 50_000_000_000i128);
}

#[test]
#[should_panic]
fn test_revert_snapshot_not_found() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let missing_snapshot = String::from_str(&env, "nonexistent");
    client.revert_to_snapshot(&missing_snapshot);
}

#[test]
#[should_panic]
fn test_revert_snapshot_unauthorized() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let snapshot_name = String::from_str(&env, "backup");
    client.create_state_snapshot(&snapshot_name);

    let unauthorized = Address::generate(&env);
    env.as_contract(&unauthorized, || {
        client.revert_to_snapshot(&snapshot_name);
    });
}

#[test]
#[should_panic]
fn test_revert_snapshot_not_initialized() {
    let env = soroban_sdk::Env::default();
    env.mock_all_auths();
    let client = super::deploy(&env);

    let snapshot_name = String::from_str(&env, "backup");
    client.revert_to_snapshot(&snapshot_name);
}

#[test]
#[should_panic]
fn test_revert_snapshot_invalid_name() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let bad_name = String::from_str(&env, "backup@");
    client.revert_to_snapshot(&bad_name);
}

#[test]
fn test_multiple_snapshots() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let snapshot1 = String::from_str(&env, "snap1");
    let snapshot2 = String::from_str(&env, "snap2");

    client.create_state_snapshot(&snapshot1);

    let investor = Address::generate(&env);
    client.fund(&investor, &50_000_000_000i128);

    client.create_state_snapshot(&snapshot2);

    let investor2 = Address::generate(&env);
    client.fund(&investor2, &30_000_000_000i128);

    // Revert to snapshot2
    client.revert_to_snapshot(&snapshot2);
    assert_eq!(client.get_escrow().funded_amount, 50_000_000_000i128);

    // Revert to snapshot1
    client.revert_to_snapshot(&snapshot1);
    assert_eq!(client.get_escrow().funded_amount, 0);
}

#[test]
fn test_snapshot_preserves_all_escrow_fields() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);

    let (token, treasury) = super::free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "COMPLEX"),
        &sme,
        &1_000_000_000i128,
        &750i64,
        &100_000u64,
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

    let snapshot_name = String::from_str(&env, "complex");
    client.create_state_snapshot(&snapshot_name);

    let escrow_before = client.get_escrow();
    assert_eq!(escrow_before.amount, 1_000_000_000i128);
    assert_eq!(escrow_before.yield_bps, 750i64);
    assert_eq!(escrow_before.maturity, 100_000u64);
    assert_eq!(escrow_before.funding_target, 1_000_000_000i128);

    // Revert (no-op in this case)
    client.revert_to_snapshot(&snapshot_name);

    let escrow_after = client.get_escrow();
    assert_eq!(escrow_after.amount, escrow_before.amount);
    assert_eq!(escrow_after.yield_bps, escrow_before.yield_bps);
    assert_eq!(escrow_after.maturity, escrow_before.maturity);
    assert_eq!(escrow_after.funding_target, escrow_before.funding_target);
}

#[test]
fn test_snapshot_name_alphanumeric_and_underscore() {
    let env = soroban_sdk::Env::default();
    let (client, admin, sme) = super::setup(&env);
    super::default_init(&client, &env, &admin, &sme);

    let valid_names = vec![
        "snap_1",
        "snapshot_backup_2",
        "v1_2_3",
        "UPPERCASE",
        "lowercase",
        "Mixed_Case_123",
    ];

    for name_str in valid_names {
        let name = String::from_str(&env, name_str);
        client.create_state_snapshot(&name);
    }
}
