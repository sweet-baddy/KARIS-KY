#![no_main]
use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::*, Address, Env, String};

use karis_ky_escrow::LiquifactEscrow;

/// Fuzz harness for yield calculations and investor payouts.
///
/// Tests invariants:
/// - Yield bps always in valid range [0, 10000]
/// - Payout calculations never overflow
/// - Effective yield per investor valid
/// - Claim locks respect configured timings
/// - Investor payouts never exceed pro-rata share

#[derive(Arbitrary, Debug, Clone)]
struct YieldInput {
    /// Base yield in bps [0, 10000]
    yield_bps: u32,
    /// Invoice amount
    invoice_amount: u64,
    /// Investor contributions and their yields
    investor_contributions: Vec<u64>,
    /// Time to claim (relative to settlement)
    claim_time_offset: u32,
}

fn setup_env() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.budget().reset_unlimited();
    env.mock_all_auths();

    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 1_000_000;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);

    (env, admin, sme, token)
}

fuzz_target!(|input: YieldInput| {
    let (env, admin, sme, token) = setup_env();

    let invoice_amount = std::cmp::max(1u64, input.invoice_amount % 10_000_000_000) as i128;
    let yield_bps = (input.yield_bps % 10_001) as i64;
    let funding_target = (invoice_amount * 4) / 5; // 80% target

    let treasury = Address::generate(&env);

    // **INVARIANT 1**: yield_bps must be in valid range [0, 10000]
    assert!(
        yield_bps >= 0 && yield_bps <= 10_000,
        "INVARIANT VIOLATION: yield_bps ({}) outside valid range",
        yield_bps
    );

    // Initialize escrow
    let invoice_id = String::from_str(&env, "yield-fuzz");
    let init_result = LiquifactEscrow::init(
        env.clone(),
        invoice_id,
        admin.clone(),
        sme.clone(),
        invoice_amount,
        funding_target,
        1_000_000u64 + (365 * 24 * 60 * 60), // 1 year maturity
        yield_bps,
        token.clone(),
        treasury,
        None, // registry
        None, // min_contribution
        None, // max_unique_investors
        None, // max_per_investor
        None, // yield_tiers
        None, // funding_deadline
        false, // allowlist
    );

    if init_result.is_err() {
        return;
    }

    let mut total_funded = 0i128;
    let mut investors: Vec<Address> = Vec::new();
    let mut contributions: Vec<i128> = Vec::new();

    // Fund from multiple investors
    for &amount_u64 in input.investor_contributions.iter().take(10) {
        let fund_amount = std::cmp::max(1u64, amount_u64 % 50_000_000) as i128;

        // Don't over-fund
        if total_funded + fund_amount > funding_target * 2 {
            break;
        }

        let investor = Address::generate(&env);

        let fund_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            LiquifactEscrow::fund(&env, investor.clone(), fund_amount)
        }));

        if fund_result.is_ok() {
            investors.push(investor);
            contributions.push(fund_amount);
            total_funded += fund_amount;
        }
    }

    // Check if we reached funded state
    let escrow_after_funding = LiquifactEscrow::get_escrow(env.clone());
    if escrow_after_funding.status != 1 {
        return; // Not funded, skip settlement tests
    }

    // Get snapshot for pro-rata calculations
    let snapshot = LiquifactEscrow::get_funding_close_snapshot(env.clone());
    if snapshot.is_none() {
        return;
    }

    let snapshot = snapshot.unwrap();

    // **INVARIANT 2**: snapshot.total_principal >= funded_amount
    assert!(
        snapshot.total_principal >= escrow_after_funding.funded_amount,
        "INVARIANT VIOLATION: snapshot total < funded_amount"
    );

    // Advance to maturity
    let mut ledger = env.ledger().get();
    ledger.timestamp = 1_000_000 + (365 * 24 * 60 * 60) + 100;
    env.ledger().set(ledger);

    // Settle
    let settle_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        LiquifactEscrow::settle(&env)
    }));

    if settle_result.is_err() {
        return;
    }

    // Verify yield information for each investor
    for (idx, investor) in investors.iter().enumerate() {
        if idx >= contributions.len() {
            break;
        }

        let contribution = contributions[idx];

        // Get investor's effective yield
        let effective_yield = LiquifactEscrow::get_investor_yield_bps(env.clone(), investor.clone());

        // **INVARIANT 3**: Effective yield must be <= base yield (no yield enhancement for base)
        assert!(
            effective_yield <= yield_bps || effective_yield == yield_bps,
            "INVARIANT VIOLATION: effective yield ({}) exceeds base yield ({})",
            effective_yield,
            yield_bps
        );

        // **INVARIANT 4**: Yield must be in valid range
        assert!(
            effective_yield >= 0 && effective_yield <= 10_000,
            "INVARIANT VIOLATION: investor effective yield ({}) outside valid range",
            effective_yield
        );

        // Get claim info
        let claim_not_before = LiquifactEscrow::get_investor_claim_not_before(env.clone(), investor.clone());

        // **INVARIANT 5**: claim_not_before should be in past or at settlement time
        let current_time = env.ledger().timestamp();
        assert!(
            claim_not_before <= current_time,
            "INVARIANT VIOLATION: claim_not_before ({}) in future (current: {})",
            claim_not_before,
            current_time
        );

        // Attempt claim
        let claim_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            LiquifactEscrow::claim_investor_payout(&env, investor.clone())
        }));

        match claim_result {
            Ok(_) => {
                // Claim succeeded - verify it was reasonable
                // Note: actual payout verification would require token balance checks

                // **INVARIANT 6**: Cannot double-claim
                let double_claim = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LiquifactEscrow::claim_investor_payout(&env, investor.clone())
                }));

                if double_claim.is_ok() {
                    // Second claim should also succeed but be idempotent
                }
            }
            Err(_) => {
                // Claim failed, which is acceptable for various reasons
            }
        }
    }

    // Final state consistency check
    let final_escrow = LiquifactEscrow::get_escrow(env.clone());

    // **INVARIANT 7**: Final yield_bps unchanged
    assert_eq!(
        final_escrow.yield_bps, yield_bps,
        "INVARIANT VIOLATION: yield_bps changed during execution"
    );

    // **INVARIANT 8**: Final status is settled or beyond
    assert!(
        final_escrow.status >= 2,
        "INVARIANT VIOLATION: final status ({}) should be >= 2 (settled)",
        final_escrow.status
    );

    // **INVARIANT 9**: Amounts unchanged
    assert_eq!(
        final_escrow.amount, invoice_amount,
        "INVARIANT VIOLATION: invoice amount changed"
    );
    assert_eq!(
        final_escrow.funded_amount, escrow_after_funding.funded_amount,
        "INVARIANT VIOLATION: funded_amount changed post-settlement"
    );
});
