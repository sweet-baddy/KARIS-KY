//! Comprehensive tests for state inconsistency detection.
//!
//! These tests validate that [`LiquifactEscrow::detect_state_inconsistencies`]
//! correctly identifies logical invariant violations and emits the appropriate event.

#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::*, vec, Address, Env, String, Symbol};

    use crate::{
        EscrowError, LiquifactEscrow, StateInconsistencyReport, StateInconsistenciesDetected,
        SCHEMA_VERSION,
    };

    use super::super::{
        default_init, deploy, free_addresses, install_stellar_asset_token, setup, TARGET,
    };

    #[test]
    fn test_valid_escrow_no_inconsistencies() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &TARGET,
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

        let report = client.detect_state_inconsistencies();

        assert_eq!(report.funded_exceeds_target_stale, false);
        assert_eq!(report.funded_positive_status_open, false);
        assert_eq!(report.funded_zero_status_advanced, false);
        assert_eq!(report.funders_exist_status_open, false);
        assert_eq!(report.no_funders_advanced_status, false);
        assert_eq!(report.snapshot_exists_not_funded, false);
        assert_eq!(report.snapshot_missing_post_funded, false);
        assert_eq!(report.settled_before_maturity_lock, false);
        assert_eq!(report.invalid_funding_amounts, false);
        assert_eq!(report.invalid_status_value, false);
    }

    #[test]
    fn test_inconsistency_funded_exceeds_target_stale() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        let investor = Address::generate(&env);
        token.stellar.mint(&investor, &1_000_000_000i128);

        // Initialize with low target
        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &1000,  // amount
            &500,   // funding_target (set low)
            &(365 * 24 * 60 * 60),
            &token.id,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Fund with amount > target (600 > 500)
        client.fund(&investor, &600);

        // Check that we have the inconsistency
        let report = client.detect_state_inconsistencies();
        assert_eq!(report.funded_exceeds_target_stale, true);
    }

    #[test]
    fn test_inconsistency_funded_positive_status_open() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        let investor = Address::generate(&env);
        token.stellar.mint(&investor, &1_000_000_000i128);

        // Initialize with high target
        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &1000,  // amount
            &900,   // funding_target (high)
            &(365 * 24 * 60 * 60),
            &token.id,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Fund with small amount
        client.fund(&investor, &100);

        // Check: funded_amount > 0 but status == 0 (open)
        let report = client.detect_state_inconsistencies();
        assert_eq!(report.funded_positive_status_open, true);
    }

    #[test]
    fn test_inconsistency_funders_exist_status_open() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        let investor = Address::generate(&env);
        token.stellar.mint(&investor, &1_000_000_000i128);

        // Initialize
        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &1000,  // amount
            &900,   // funding_target
            &(365 * 24 * 60 * 60),
            &token.id,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Fund - increments unique_funder_count
        client.fund(&investor, &100);

        // Check: unique_funder_count > 0 but status == 0 (open)
        let report = client.detect_state_inconsistencies();
        assert_eq!(report.funders_exist_status_open, true);
    }

    #[test]
    fn test_multiple_inconsistencies_detected() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        let investor = Address::generate(&env);
        token.stellar.mint(&investor, &1_000_000_000i128);

        // Initialize with high target
        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &1000,  // amount
            &900,   // funding_target
            &(365 * 24 * 60 * 60),
            &token.id,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Fund partially - triggers multiple inconsistencies
        client.fund(&investor, &100);

        let report = client.detect_state_inconsistencies();

        // We should have at least 2 inconsistencies:
        // 1. funded_positive_status_open
        // 2. funders_exist_status_open
        assert_eq!(report.funded_positive_status_open, true);
        assert_eq!(report.funders_exist_status_open, true);
    }

    #[test]
    fn test_all_inconsistency_flags_false_on_valid_state() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &TARGET,
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

        let report = client.detect_state_inconsistencies();

        // Verify all flags are false for a valid initialized escrow
        assert_eq!(report.funded_exceeds_target_stale, false);
        assert_eq!(report.funded_positive_status_open, false);
        assert_eq!(report.funded_zero_status_advanced, false);
        assert_eq!(report.funders_exist_status_open, false);
        assert_eq!(report.no_funders_advanced_status, false);
        assert_eq!(report.snapshot_exists_not_funded, false);
        assert_eq!(report.snapshot_missing_post_funded, false);
        assert_eq!(report.settled_before_maturity_lock, false);
        assert_eq!(report.invalid_sme_address, false);
        assert_eq!(report.invalid_admin_address, false);
        assert_eq!(report.invalid_funding_amounts, false);
    }

    #[test]
    fn test_inconsistency_detection_is_readonly() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &TARGET,
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

        // Call detect_state_inconsistencies multiple times
        let report1 = client.detect_state_inconsistencies();
        let report2 = client.detect_state_inconsistencies();

        // Reports should be identical
        assert_eq!(report1.funded_exceeds_target_stale, report2.funded_exceeds_target_stale);
        assert_eq!(report1.funded_positive_status_open, report2.funded_positive_status_open);
        assert_eq!(report1.funded_zero_status_advanced, report2.funded_zero_status_advanced);
        assert_eq!(report1.funders_exist_status_open, report2.funders_exist_status_open);
        assert_eq!(report1.no_funders_advanced_status, report2.no_funders_advanced_status);
        assert_eq!(report1.snapshot_exists_not_funded, report2.snapshot_exists_not_funded);
        assert_eq!(report1.snapshot_missing_post_funded, report2.snapshot_missing_post_funded);
        assert_eq!(report1.settled_before_maturity_lock, report2.settled_before_maturity_lock);
        assert_eq!(report1.invalid_funding_amounts, report2.invalid_funding_amounts);
        assert_eq!(report1.invalid_status_value, report2.invalid_status_value);
    }

    #[test]
    fn test_funded_exceeds_with_exact_match() {
        // When funding exactly matches target, there should be no inconsistency
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        let investor = Address::generate(&env);
        token.stellar.mint(&investor, &1_000_000_000i128);

        // Initialize with 500 target
        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &1000,
            &500,   // funding_target
            &(365 * 24 * 60 * 60),
            &token.id,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        // Fund exactly to target (should auto-advance to funded)
        client.fund(&investor, &500);

        let report = client.detect_state_inconsistencies();
        // Status should be advanced to 1 (funded), so funded_exceeds_target_stale
        // should be false
        assert_eq!(report.funded_exceeds_target_stale, false);
    }
}
