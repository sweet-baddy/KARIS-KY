//! Tests for the embedded build metadata entrypoint.
//!
//! Verifies that the values returned by [`LiquifactEscrow::get_build_metadata`]
//! match the source-of-truth constants in `escrow/src/lib.rs`. These tests
//! run on the host target (`cargo test`), but `escrow/build.rs` runs for
//! every target — so the embedded values are available in test mode too.

use soroban_sdk::Env;

use crate::{
    LiquifactEscrow, LiquifactEscrowClient, CONTRACT_INTERFACE_VERSION, SCHEMA_VERSION,
};

#[test]
fn get_build_metadata_reports_source_constants() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let meta = client.get_build_metadata();

    // The contract must report the same schema/interface versions as the
    // source constants. If this fails, either build.rs or lib.rs has drifted.
    assert_eq!(
        meta.schema_version, SCHEMA_VERSION,
        "embedded schema_version must match lib.rs SCHEMA_VERSION constant"
    );
    assert_eq!(
        meta.interface_version, CONTRACT_INTERFACE_VERSION,
        "embedded interface_version must match lib.rs CONTRACT_INTERFACE_VERSION constant"
    );
}

#[test]
fn get_build_metadata_available_without_init() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    // No init() called — metadata should still be available because the
    // values are compile-time constants, not stored state.
    let meta = client.get_build_metadata();

    assert!(meta.schema_version > 0, "schema_version must be positive");
    assert!(
        meta.interface_version > 0,
        "interface_version must be positive"
    );
    assert!(!meta.git_commit.is_empty(), "git_commit must not be empty");
    assert!(!meta.pkg_version.is_empty(), "pkg_version must not be empty");
    assert!(
        !meta.build_timestamp.is_empty(),
        "build_timestamp must not be empty"
    );
}

#[test]
fn get_build_metadata_is_deterministic() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let first = client.get_build_metadata();
    let second = client.get_build_metadata();
    let third = client.get_build_metadata();

    assert_eq!(first, second);
    assert_eq!(second, third);
}

#[test]
fn get_build_metadata_consistent_with_get_interface_version() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let meta = client.get_build_metadata();
    let interface_version = client.get_interface_version();

    assert_eq!(
        meta.interface_version, interface_version,
        "get_build_metadata().interface_version must match get_interface_version()"
    );
}