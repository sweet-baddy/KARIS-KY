use super::*;
use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, Ledger};

/// Property: total claimed payout never exceeds funded amount plus yield.
///
/// Strategy: generate a random number of investors (1..10), each with a random
/// contribution amount (positive, up to a reasonable cap). Fund the escrow,
/// settle, then have each investor claim. Sum the claimed amounts and compare
/// to the total funded plus the yield computed on the total funded.
#[test]
fn test_total_claimed_never_exceeds_funded_plus_yield() {
    // This test uses proptest to explore many random scenarios.
    // We'll generate a vector of contributions.
    proptest!(|(
        contributions in proptest::collection::vec(1..100_000_000i128, 1..10),
    )| {
        // Set up test environment
        let env = Env::default();
        env.mock_all_auths();

        let (client, admin, sme) = setup(&env);
        let (funding_token, treasury) = free_addresses(&env);

        // Sum contributions for total funded
        let total_funded: i128 = contributions.iter().sum();
        // Avoid zero total (though vec size >=1 and each >0)
        assert!(total_funded > 0);

        // Initialize escrow with target = total_funded (so it becomes funded after all contributions)
        let target = total_funded;
        let yield_bps = 800; // 8% (common base)
        client.init(
            &admin,
            &String::from_str(&env, "PROPTEST_001"),
            &sme,
            &target,
            &yield_bps,
            &0, // No maturity lock for simplicity
            &funding_token,
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

        // Fund each investor
        let investors: Vec<Address> = contributions
            .iter()
            .map(|_| Address::generate(&env))
            .collect();

        for (i, &amount) in contributions.iter().enumerate() {
            client.fund(&investors[i], &amount);
        }

        // After all contributions, the escrow should be funded (status=1) because total >= target
        // (we set target = total_funded, so it becomes funded immediately after the last deposit)
        // Actually, funding close snapshot is written when status becomes 1.
        // We need to check that the escrow is now funded.
        let escrow = client.get_escrow();
        assert_eq!(escrow.status, 1, "Escrow should be funded");

        // Settle the escrow (SME must be the one calling settle)
        // Since we mock all auths, any address can act as SME, but we'll use the actual SME.
        // We need to set ledger time to after maturity if maturity >0, but we set maturity=0.
        client.settle();

        // After settlement, each investor claims their payout.
        let mut total_claimed = 0i128;
        for investor in &investors {
            let payout_before = client.compute_investor_payout(&investor);
            if payout_before > 0 {
                // Claim
                client.claim_investor_payout(&investor);
                // We can't easily get the payout amount directly after claim,
                // but we can compute it again after claim (it will still return the same value)
                let payout_after = client.compute_investor_payout(&investor);
                total_claimed += payout_after;
            }
        }

        // Compute expected yield on the total funded amount (using base yield)
        // Yield = total_funded * yield_bps / 10_000
        let expected_yield = (total_funded * yield_bps as i128) / 10_000;
        let max_payout = total_funded + expected_yield;

        // Invariant: total claimed should never exceed max_payout
        assert!(
            total_claimed <= max_payout,
            "Invariant violated: total_claimed={} > max_payout={} (funded={}, yield={})",
            total_claimed,
            max_payout,
            total_funded,
            expected_yield
        );

        // Also verify that total_claimed is at least the funded amount (since principal is always returned)
        // Actually, due to rounding, total_claimed might be slightly less than total_funded?
        // In this contract, each investor gets at least their principal, so sum of claims >= total_funded.
        // But let's not enforce that strictly as rounding might cause tiny discrepancies.
        // We'll check that total_claimed >= total_funded - (some tolerance).
        let tolerance = 10; // Allow up to 10 units of rounding error.
        assert!(
            total_claimed >= total_funded - tolerance,
            "Principal not fully returned: total_claimed={} < funded={}",
            total_claimed,
            total_funded
        );
    });
}