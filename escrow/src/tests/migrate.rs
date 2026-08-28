use super::*;

// ============================================================================
// Migration Error Tests — Error Codes 90, 91, 92 (Issue #414)
// ============================================================================

/// Test: Error 90 — MigrationVersionMismatch
///
/// When `from_version` does not match the stored version, the contract
/// should panic with error code 90 (MigrationVersionMismatch).
#[test]
#[should_panic(expected = "90")]
fn test_migration_error_90_version_mismatch() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow (sets version to SCHEMA_VERSION)
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_ERR_90"),
        &admin,
        &100_000i128,
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
    );

    // Try to migrate with a from_version that doesn't match stored version
    // The stored version is SCHEMA_VERSION (e.g., 6)
    // Provide a different version (e.g., 1)
    client.migrate(&1u32);
}

/// Test: Error 91 — AlreadyCurrentSchemaVersion
///
/// When `from_version >= SCHEMA_VERSION`, the contract should panic
/// with error code 91 (AlreadyCurrentSchemaVersion).
#[test]
#[should_panic(expected = "91")]
fn test_migration_error_91_already_current() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow (sets version to SCHEMA_VERSION)
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_ERR_91"),
        &admin,
        &100_000i128,
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
    );

    // Get the current SCHEMA_VERSION
    let current_version = client.get_version();

    // Try to migrate with from_version >= current version
    // This should panic with error 91
    client.migrate(&current_version);
}

/// Test: Error 92 — NoMigrationPath
///
/// When `from_version < SCHEMA_VERSION` but no migration path is implemented,
/// the contract should panic with error code 92 (NoMigrationPath).
#[test]
#[should_panic(expected = "92")]
fn test_migration_error_92_no_migration_path() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow (sets version to SCHEMA_VERSION)
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_ERR_92"),
        &admin,
        &100_000i128,
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
    );

    // The stored version is SCHEMA_VERSION (e.g., 6)
    // Provide a from_version that is less than SCHEMA_VERSION
    // Since no migration path is implemented, this should panic with error 92
    client.migrate(&1u32);
}