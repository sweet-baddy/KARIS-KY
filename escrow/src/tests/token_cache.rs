//! Tests for token metadata caching functionality.
//!
//! Verifies that:
//! - Token metadata is cached at escrow initialization
//! - Cache can be explicitly revalidated by admin
//! - Fund operations correctly use cached metadata
//! - Cache timestamps enable staleness detection

#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::*, Address, Env, String};

    use crate::{LiquifactEscrow, TokenMetadataCache};

    use super::super::{
        default_init, deploy, free_addresses, install_stellar_asset_token, setup, TARGET,
    };

    #[test]
    fn test_token_cache_initialized_at_init() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        // Initialize escrow
        client.init(
            &admin,
            &String::from_str(&env, "INV001"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        // Verify token cache was written
        let cache = LiquifactEscrow::get_token_metadata_cache(env.clone());
        assert!(cache.is_some(), "Token cache should be written at init");

        let cache = cache.unwrap();
        assert_eq!(cache.decimals, 7, "Token decimals should be cached (Stellar default)");
        assert!(
            cache.cached_at_ledger_timestamp > 0,
            "Cache timestamp should be set"
        );
        assert!(
            cache.cached_at_ledger_sequence > 0,
            "Cache sequence should be set"
        );
    }

    #[test]
    fn test_cache_decimals_match_token_contract() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        // Initialize
        client.init(
            &admin,
            &String::from_str(&env, "INV002"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        // Get cached decimals
        let cache = LiquifactEscrow::get_token_metadata_cache(env.clone()).unwrap();

        // Get decimals directly from token
        let direct_decimals = token.token.decimals();

        // They should match
        assert_eq!(
            cache.decimals, direct_decimals,
            "Cached decimals should match token contract"
        );
    }

    #[test]
    fn test_get_token_metadata_cache_returns_option() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let (token, treasury) = free_addresses(&env);

        // Before init, cache should not exist
        let cache = LiquifactEscrow::get_token_metadata_cache(env.clone());
        assert!(cache.is_none(), "Cache should not exist before init");

        // After init with real token
        let real_token = install_stellar_asset_token(&env);
        client.init(
            &admin,
            &String::from_str(&env, "INV003"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
            &real_token.id,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let cache = LiquifactEscrow::get_token_metadata_cache(env.clone());
        assert!(cache.is_some(), "Cache should exist after init");
    }

    #[test]
    fn test_revalidate_token_cache_updates_timestamps() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        // Initialize
        client.init(
            &admin,
            &String::from_str(&env, "INV004"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        let cache_before = LiquifactEscrow::get_token_metadata_cache(env.clone()).unwrap();

        // Advance ledger
        let mut ledger = env.ledger().get();
        ledger.timestamp += 1000;
        ledger.sequence_number += 10;
        env.ledger().set(ledger);

        // Revalidate cache
        let cache_after = LiquifactEscrow::revalidate_token_cache(env.clone());

        // Timestamps should be updated
        assert!(
            cache_after.cached_at_ledger_timestamp > cache_before.cached_at_ledger_timestamp,
            "Timestamp should be updated after revalidation"
        );
        assert!(
            cache_after.cached_at_ledger_sequence > cache_before.cached_at_ledger_sequence,
            "Sequence should be updated after revalidation"
        );

        // Decimals should be same
        assert_eq!(
            cache_after.decimals, cache_before.decimals,
            "Decimals should not change on revalidation"
        );
    }

    #[test]
    fn test_revalidate_requires_admin_auth() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        client.init(
            &admin,
            &String::from_str(&env, "INV005"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        // Non-admin attempt should fail
        env.mock_auths(&[]);
        let non_admin = Address::generate(&env);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            LiquifactEscrow::revalidate_token_cache(env.clone())
        }));

        // Should panic (require_auth fails)
        assert!(result.is_err(), "Non-admin revalidation should fail");

        // Admin should succeed
        env.mock_all_auths();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            LiquifactEscrow::revalidate_token_cache(env.clone())
        }));

        assert!(result.is_ok(), "Admin revalidation should succeed");
    }

    #[test]
    fn test_cache_available_during_fund_operations() {
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
            &String::from_str(&env, "INV006"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        let cache_init = LiquifactEscrow::get_token_metadata_cache(env.clone()).unwrap();

        // Fund
        client.fund(&investor, &100);

        // Cache should still be present and unchanged
        let cache_after_fund = LiquifactEscrow::get_token_metadata_cache(env.clone()).unwrap();

        assert_eq!(
            cache_init.decimals, cache_after_fund.decimals,
            "Decimals unchanged after fund"
        );
        assert_eq!(
            cache_init.cached_at_ledger_timestamp,
            cache_after_fund.cached_at_ledger_timestamp,
            "Timestamp unchanged after fund"
        );
        assert_eq!(
            cache_init.cached_at_ledger_sequence,
            cache_after_fund.cached_at_ledger_sequence,
            "Sequence unchanged after fund"
        );
    }

    #[test]
    fn test_cache_persists_across_multiple_fund_calls() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        // Initialize
        client.init(
            &admin,
            &String::from_str(&env, "INV007"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        let cache_init = LiquifactEscrow::get_token_metadata_cache(env.clone()).unwrap();

        // Multiple investors funding
        for i in 0..5 {
            let investor = Address::generate(&env);
            token.stellar.mint(&investor, &1_000_000_000i128);
            client.fund(&investor, &100);

            // Cache should remain stable
            let cache = LiquifactEscrow::get_token_metadata_cache(env.clone()).unwrap();
            assert_eq!(
                cache.decimals, cache_init.decimals,
                "Cache decimals stable across fund calls (iteration {})",
                i
            );
        }
    }

    #[test]
    fn test_cache_struct_has_all_fields() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        client.init(
            &admin,
            &String::from_str(&env, "INV008"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        let cache = LiquifactEscrow::get_token_metadata_cache(env.clone()).unwrap();

        // Verify all fields are present and reasonable
        assert!(cache.decimals > 0, "Decimals should be positive");
        assert!(
            cache.decimals <= 18,
            "Decimals within reasonable range (0-18)"
        );
        assert!(
            cache.cached_at_ledger_timestamp > 0,
            "Timestamp should be positive"
        );
        assert!(
            cache.cached_at_ledger_sequence > 0,
            "Sequence should be positive"
        );
    }

    #[test]
    fn test_cache_clone_debug_partialeq() {
        let cache1 = TokenMetadataCache {
            decimals: 7,
            cached_at_ledger_timestamp: 1000,
            cached_at_ledger_sequence: 100,
        };

        // Clone works
        let cache2 = cache1.clone();
        assert_eq!(cache1, cache2, "Clone should be equal to original");

        // Debug works
        let debug_str = format!("{:?}", cache1);
        assert!(debug_str.contains("decimals"), "Debug output should contain decimals");
        assert!(
            debug_str.contains("cached_at_ledger_timestamp"),
            "Debug output should contain timestamp"
        );

        // PartialEq works
        let cache3 = TokenMetadataCache {
            decimals: 7,
            cached_at_ledger_timestamp: 1000,
            cached_at_ledger_sequence: 100,
        };
        assert_eq!(cache1, cache3, "Equal caches should be equal");

        let cache4 = TokenMetadataCache {
            decimals: 8, // different
            cached_at_ledger_timestamp: 1000,
            cached_at_ledger_sequence: 100,
        };
        assert_ne!(cache1, cache4, "Different caches should not be equal");
    }

    #[test]
    fn test_revalidate_after_settlement() {
        let env = Env::default();
        env.budget().reset_unlimited();
        let (client, admin, sme) = setup(&env);
        let token = install_stellar_asset_token(&env);
        let treasury = Address::generate(&env);

        let investor = Address::generate(&env);
        token.stellar.mint(&investor, &1_000_000_000i128);

        // Initialize and fund
        client.init(
            &admin,
            &String::from_str(&env, "INV009"),
            &sme,
            &TARGET,
            &800i64,
            &1000u64,
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

        client.fund(&investor, &TARGET as i128);

        // Advance to maturity and settle
        let mut ledger = env.ledger().get();
        ledger.timestamp = 2001; // past maturity (1000)
        env.ledger().set(ledger);

        client.settle();

        // Revalidate should still work after settlement
        let cache = LiquifactEscrow::revalidate_token_cache(env.clone());
        assert!(cache.decimals > 0, "Cache should be valid after settlement");
    }
}
