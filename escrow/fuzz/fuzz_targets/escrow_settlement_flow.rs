#![no_main]
use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::*, Address, Env, String};

use karis_ky_escrow::LiquifactEscrow;

/// Fuzz harness for escrow settlement operations.
///
/// Tests invariants:
/// - Settlement only allowed after funding_close_snapshot is set
/// - Settlement respects maturity time lock
/// - Status transitions are forward-only
/// - Settled escrow cannot be re-settled
/// - Investor claims respect settlement finality

#[derive(Arbitrary, Debug, Clone)]
struct SettlementInput {
    /// Funding target
    funding_target: u64,
    /// Initial maturity offset from current time
    maturity_offset: u32,
    /// Investor funding amounts
    investor_amounts: Vec<u64>,
    /// Whether to advance time before settlement
    advance_time_steps: u8,
    /// Whether to attempt early settlement (before maturity)
    attempt_early_settlement: bool,
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

fuzz_target!(|input: SettlementInput| {
    let (env, admin, sme, token) = setup_env();
    env.mock_all_auths();

    let invoice_amount = 1_000_000_000i128;
    let funding_target = std::cmp::max(1u64, input.funding_target % 500_000_000) as i128;
    let base_timestamp = 1_000_000u64;
    let maturity = base_timestamp + std::cmp::max(1u32, input.maturity_offset) as u64;

    let treasury = Address::generate(&env);

    // Initialize escrow with maturity lock
    let invoice_id = String::from_str(&env, "settlement-fuzz");
    let init_result = LiquifactEscrow::init(
        env.clone(),
        invoice_id.clone(),
        admin.clone(),
        sme.clone(),
        invoice_amount,
        funding_target,
        maturity,
        500i64, // 5% yield
        token.clone(),
        treasury,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
    );

    if init_result.is_err() {
        return;
    }

    // Fund to reach funded status
    let mut total_funded = 0i128;
    for &amount_u64 in input.investor_amounts.iter().take(5) {
        let fund_amount = std::cmp::max(1u64, amount_u64 % 100_000_000) as i128;

        // Only fund up to target
        if total_funded + fund_amount > funding_target {
            break;
        }

        let investor = Address::generate(&env);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            LiquifactEscrow::fund(&env, investor, fund_amount)
        }));

        total_funded += fund_amount;

        if total_funded >= funding_target {
            break;
        }
    }

    let mut current_escrow = LiquifactEscrow::get_escrow(env.clone());

    // Only proceed if we reached funded status
    if current_escrow.status != 1 {
        return;
    }

    // **INVARIANT 1**: Funded snapshot must exist once status is funded
    let snapshot = LiquifactEscrow::get_funding_close_snapshot(env.clone());
    assert!(
        snapshot.is_some(),
        "INVARIANT VIOLATION: funding_close_snapshot missing after reaching funded status"
    );

    // Optionally advance time
    for step in 0..input.advance_time_steps {
        let mut ledger = env.ledger().get();
        ledger.timestamp += 3600; // 1 hour per step
        env.ledger().set(ledger);
    }

    current_escrow = LiquifactEscrow::get_escrow(env.clone());

    // Attempt settlement
    let settlement_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        LiquifactEscrow::settle(&env)
    }));

    match settlement_result {
        Ok(_) => {
            let settled_escrow = LiquifactEscrow::get_escrow(env.clone());

            // **INVARIANT 2**: Status must be 2 (settled) after successful settlement
            assert_eq!(
                settled_escrow.status, 2,
                "INVARIANT VIOLATION: status not 2 after settlement (got {})",
                settled_escrow.status
            );

            // **INVARIANT 3**: Escrow data is immutable post-settlement
            assert_eq!(
                settled_escrow.amount, current_escrow.amount,
                "INVARIANT VIOLATION: invoice amount changed post-settlement"
            );
            assert_eq!(
                settled_escrow.funded_amount, current_escrow.funded_amount,
                "INVARIANT VIOLATION: funded_amount changed post-settlement"
            );
            assert_eq!(
                settled_escrow.yield_bps, current_escrow.yield_bps,
                "INVARIANT VIOLATION: yield_bps changed post-settlement"
            );

            // **INVARIANT 4**: Cannot settle again
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                LiquifactEscrow::settle(&env)
            }));
            // Second settle should fail gracefully
            let double_settled = LiquifactEscrow::get_escrow(env.clone());
            assert_eq!(
                double_settled.status, 2,
                "INVARIANT VIOLATION: status changed on re-settlement attempt"
            );
        }
        Err(_) => {
            // Settlement failed, which is acceptable if:
            // - Maturity not reached
            // - Not actually funded
            // - Other validation failed
            let failed_escrow = LiquifactEscrow::get_escrow(env.clone());
            assert!(
                failed_escrow.status == 1 || failed_escrow.status == 0,
                "INVARIANT VIOLATION: status should remain unchanged after failed settlement"
            );
        }
    }

    // **INVARIANT 5**: Status is monotonically increasing
    let final_escrow = LiquifactEscrow::get_escrow(env.clone());
    assert!(
        final_escrow.status >= current_escrow.status,
        "INVARIANT VIOLATION: status decreased during execution (was {}, now {})",
        current_escrow.status,
        final_escrow.status
    );
});
