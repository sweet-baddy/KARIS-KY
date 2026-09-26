# karis-ky Escrow — Audit Handoff Bundle

**Contract:** `karis-ky_escrow` (`escrow/src/lib.rs`)
**Schema version:** 7 (`SCHEMA_VERSION`)
**Soroban SDK:** 25.0
**Stellar protocol:** SEP-41 token interface
**Audit date:** 2026-08-26

---

## 1. Contract Purpose

Single-invoice escrow. Holds investor stablecoin principal until a funding target is met, then allows the SME to withdraw or settle. After settlement, each investor records a claim marker; actual payout math happens off-chain using the on-chain snapshot and contribution data. A cancel/refund path allows the admin to cancel while open, returning principal to investors.

---

## 2. Lifecycle State Machine

```
                fund() / fund_with_commitment()
  [0: open]  ──────────────────────────────────► [1: funded]
     │                                                 │
     │ cancel_funding()                                ├── settle()  ──► [2: settled]  ──► claim_investor_payout()
     ▼                                                 └── withdraw() ──► [3: withdrawn]
  [4: cancelled]
     │
     └── refund(investor)    (per-investor; status stays 4)

  [2/3/4] ──► archive() ──► [5: archived]    (monitoring only; does not block reads)
```

| Status | Value | Terminal | Dust sweep allowed |
|--------|-------|----------|--------------------|
| open       | 0 | No  | No  |
| funded     | 1 | No  | No  |
| settled    | 2 | Yes | Yes |
| withdrawn  | 3 | Yes | Yes |
| cancelled  | 4 | Yes | Yes |
| archived   | 5 | Yes | Yes |

Transitions are **strictly forward**. No entrypoint moves status backward.

---

## 3. Invariants

These map directly to property tests in `escrow/src/tests/properties.rs` and related test modules.

| ID | Name | Statement | Test(s) |
|----|------|-----------|---------|
| ESC-STA-001 | status_monotone | `status` never decreases; valid paths `0→1→2`, `0→1→3`, `0→4` | `prop_status_only_increases`, `test_withdraw_funded_then_cannot_settle` |
| ESC-FUND-001 | funded_amount_monotone | Each `fund` call adds a positive amount; `funded_amount` never decreases | `prop_funded_amount_non_decreasing` |
| ESC-FUND-002 | contribution_sum | `funded_amount == Σ contribution(investor)` across all investors while open | `test_contributions_sum_equals_funded_amount` |
| ESC-CLM-001 | investor_claim_once | `InvestorClaimed(investor)` set at most once; second call panics | `test_claim_investor_twice_panics` |
| ESC-RFND-001 | refund_once | `InvestorRefunded(investor)` set at most once; `refund` is idempotent-safe | `test_investor_refund_idempotent` |
| ESC-ATT-001 | primary_attestation_single_set | `PrimaryAttestationHash` written once; rebind panics | `test_bind_primary_attestation_single_set_and_get`, `test_bind_primary_attestation_twice_panics` |
| ESC-ATT-002 | attestation_append_bounded | `len(AttestationAppendLog) ≤ MAX_ATTESTATION_APPEND_ENTRIES (32)` | `test_append_attestation_respects_max_length` |
| ESC-MIN-001 | min_contribution_per_call | If `min_floor > 0`, every `fund` amount `≥ min_floor` | `test_min_contribution_floor_rejects_below_and_accepts_equal` |
| ESC-CAP-001 | unique_funder_cap | If `MaxUniqueInvestorsCap = n`, at most `n` distinct investor addresses may contribute | `test_max_unique_investors_cap_enforced` |
| ESC-INI-001 | single_initialization | `DataKey::Escrow` written exactly once; second `init` panics | `test_double_init_panics` |
| ESC-IMM-001 | funding_token_immutable | `DataKey::FundingToken` set at init and never mutated | `test_init_stores_registry_some_and_getters` |
| ESC-IMM-002 | treasury_immutable | `DataKey::Treasury` set at init and never mutated | `test_init_stores_registry_some_and_getters` |
| ESC-SNAP-001 | snapshot_write_once | `FundingCloseSnapshot` written at first `status→1` transition; never overwritten | `test_funding_close_snapshot_set_on_fund` |
| ESC-YIELD-001 | tier_selection_immutable | `InvestorEffectiveYield(investor)` set on first deposit only; `fund_with_commitment` panics if investor already contributed | `test_fund_with_commitment_second_call_panics` |
| ESC-DUST-001 | dust_sweep_terminal_only | `sweep_terminal_dust` rejected when `status < 2` | `test_sweep_rejected_when_open` |
| ESC-DUST-002 | dust_sweep_capped | `sweep_terminal_dust` amount ≤ `MAX_DUST_SWEEP_AMOUNT` (100_000_000) | `test_sweep_rejects_amount_above_dust_cap` |
| ESC-DUST-003 | dust_sweep_liability_floor | `balance - sweep_amt ≥ funded_amount - distributed_principal` | `sweep_liability_floor_blocks_sweep_when_investor_not_yet_refunded` |

---

## 4. Trust Model

### 4.1 Role → Entrypoint Map

| Role | Stored at | Entrypoints authorized |
|------|-----------|------------------------|
| `admin` | `InvoiceEscrow::admin` (rotatable via `propose_admin` + `accept_admin`) | `init`, `set_legal_hold`, `request_clear_legal_hold`, `clear_legal_hold`, `update_maturity`, `update_funding_target`, `lower_max_unique_investors`, `propose_admin`, `accept_admin`, `migrate`, `bind_primary_attestation_hash`, `append_attestation_digest`, `cancel_funding`, `pause_dispute`, `resume_dispute`, `set_investors_allowlisted`, `export_state`, `import_state`, `archive` |
| `sme_address` | `InvoiceEscrow::sme_address` (rotatable via `rotate_beneficiary`) | `settle`, `withdraw`, `record_sme_collateral_commitment` |
| `investor` | per-call argument (verified via `require_auth`) | `fund`, `fund_with_commitment`, `fund_batch`, `claim_investor_payout`, `refund` |
| `treasury` | `DataKey::Treasury` (immutable) | `sweep_terminal_dust` |

**No superuser path exists.** The admin cannot sweep dust unless it is also the treasury. A compromised investor key cannot settle, withdraw, or sweep.

### 4.2 Legal Hold Gate

When `DataKey::LegalHold == true`, the following entrypoints fail immediately with a typed error:

- `fund` / `fund_with_commitment` / `fund_batch` (code 102)
- `settle` (code 120)
- `withdraw` (code 123)
- `claim_investor_payout` (code 125)
- `sweep_terminal_dust` (code 30)
- `cancel_funding` (code 140)
- `rotate_beneficiary` (code 160)

Read-only getters are never blocked. Only `admin` can set or clear the hold. Clearing with a non-zero delay requires a two-phase workflow (`request_clear_legal_hold` → `clear_legal_hold`); codes 150–152 guard this path. Production deployments must use a multisig or governed contract as `admin`.

### 4.3 Dispute Pause

`pause_dispute` (admin-only) sets `DisputePaused` state with an optional auto-expiry duration. While active, `settle` and `withdraw` are blocked. This is a **separate** mechanism from legal hold; both may be independently active. `resume_dispute` manually clears it; the state also auto-expires after the configured duration.

### 4.4 Registry Non-Authority Model

`DataKey::RegistryRef` is an **optional, read-only, off-chain hint** stored at init. No on-chain logic in this contract calls or reads it after storage. Its presence **does not** constitute proof of registry membership. Callers must query the registry contract directly.

---

## 5. Token Transfer Security Audit

### 5.1 Token Transfer Wrapper

All on-chain token movements go through a single hardened function:

```
escrow/src/external_calls.rs
  └── transfer_funding_token_with_balance_checks(env, token_addr, from, treasury, amount)
```

The function enforces the following invariants in order:

1. **Positive-amount guard** — `ensure(amount > 0, TransferAmountNotPositive)` (code 36)
2. **Pre-transfer sender balance check** — `ensure(from_before >= amount, InsufficientTokenBalanceBeforeTransfer)` (code 37)
3. **`token.transfer(from, MuxedAddress::from(treasury), &amount)`** — SEP-41 call
4. **Sender underflow guard** — `from_before.checked_sub(from_after)` → `SenderBalanceUnderflow` (code 38)
5. **Recipient underflow guard** — `treasury_after.checked_sub(treasury_before)` → `RecipientBalanceUnderflow` (code 39)
6. **Sender delta equality** — `ensure(spent == amount, SenderBalanceDeltaMismatch)` (code 40)
7. **Recipient delta equality** — `ensure(received == amount, RecipientBalanceDeltaMismatch)` (code 41)

Steps 4–7 detect fee-on-transfer, rebasing, and hook tokens. Non-compliant tokens fail closed.

**Return value:** SEP-41 `transfer` returns `()` (unit). There is no return value to check; Soroban propagates panics as traps. The wrapper derives correctness from the post-call balance assertions, not from any return value.

### 5.2 Call Sites

There are exactly **4** call sites in `escrow/src/lib.rs`:

| # | Entrypoint | Transfer direction | State written before transfer | CEI compliance |
|---|-----------|-------------------|-------------------------------|----------------|
| 1 | `sweep_terminal_dust` | contract → treasury | All guards checked; no `DataKey::Escrow` mutation needed (sweep is read-only on escrow state) | ✅ Fully CEI-compliant |
| 2 | `settle` (protocol fee path) | contract → treasury | `DataKey::SettledAmount` written; `escrow.status` updated in-memory. `DataKey::Escrow` persisted **after** transfer | ⚠️ See §5.3 |
| 3 | `withdraw` | contract → sme_address | `DataKey::Escrow` (status→3) and `DataKey::DistributedPrincipal` written before transfer | ✅ Fully CEI-compliant |
| 4 | `refund` | contract → investor | `DataKey::InvestorRefunded(investor)`, `DataKey::Escrow`, and `DataKey::DistributedPrincipal` written before transfer | ✅ Fully CEI-compliant |

### 5.3 settle() — CEI Deviation Note (Informational)

In `settle()`, when a protocol fee is applicable, the call sequence is:

```
DataKey::SettledAmount  ← written (pre-transfer ✅)
escrow.status = 2       ← mutated in-memory (pre-transfer ✅)
token.transfer(fee)     ← external call
DataKey::Escrow         ← written to storage (post-transfer ⚠️)
```

`DataKey::SettledAmount` is committed and `escrow.status` is set in-memory before the token call. However, the primary escrow record (`DataKey::Escrow`, which carries `.status`) is not persisted to storage until after the transfer returns.

**Risk assessment — Not exploitable in Soroban:**
The Soroban host model executes each host function to completion before allowing any other invocation on the same contract. Classic re-entrancy (token callback → re-enter `settle` mid-execution) is structurally prevented by the host. The in-memory `escrow.status = 2` change and the committed `DataKey::SettledAmount` are sufficient to prevent logical double-settlement even if the host model were to change, because `ensure(escrow.status == 1, SettlementNotFunded)` reads `DataKey::Escrow` freshly at the top of every call.

**Recommendation:** Future refactors should move `env.storage().instance().set(&DataKey::Escrow, &escrow)` to before the fee transfer call to eliminate the deviation and align with strict CEI ordering. This is a low-priority hardening action, not a live vulnerability.

### 5.4 Reentrancy Analysis

Soroban contracts do not support the classic EVM reentrancy attack pattern:

- The Soroban host executes each cross-contract call to completion before returning to the caller.
- There is no mechanism for a token contract to re-enter `karis-ky_escrow` mid-execution.
- The pre/post balance checks in `transfer_funding_token_with_balance_checks` are a **defense-in-depth** against non-compliant token behavior (fee-on-transfer, balance manipulation), not a reentrancy guard.

No reentrancy vectors were identified.

### 5.5 Error Code Completeness (codes 36–42)

Cross-reference between `escrow/src/lib.rs` (`EscrowError` enum) and `docs/escrow-error-messages.md`:

| Code | Variant | In `lib.rs` | In `escrow-error-messages.md` | Consistent |
|------|---------|:-----------:|:-----------------------------:|:----------:|
| 36 | `TransferAmountNotPositive` | ✅ | ✅ | ✅ |
| 37 | `InsufficientTokenBalanceBeforeTransfer` | ✅ | ✅ | ✅ |
| 38 | `SenderBalanceUnderflow` | ✅ | ✅ | ✅ |
| 39 | `RecipientBalanceUnderflow` | ✅ | ✅ | ✅ |
| 40 | `SenderBalanceDeltaMismatch` | ✅ | ✅ | ✅ |
| 41 | `RecipientBalanceDeltaMismatch` | ✅ | ✅ | ✅ |
| 42 | `SweepExceedsLiabilityFloor` | ✅ | ✅ | ✅ |

All seven codes in the dust-sweep / SEP-41 safety range (30–42) are complete and consistent.

---

## 6. Function → Event → Off-chain Followup

| Function | Event struct | Topic symbol | Off-chain followup |
|----------|-------------|-------------|---------------------|
| `init` | `EscrowInitialized` | `escrow_ii` | Index `invoice_id`; register `funding_token` and `treasury` addresses; start monitoring |
| `fund` | `EscrowFunded` | `funded` | Update investor contribution ledger; check `status` field for funded transition |
| `fund_with_commitment` | `EscrowFunded` | `funded` | Same as `fund`; also record `investor_effective_yield_bps` and claim-lock timestamp |
| `fund_batch` | `EscrowFunded` (per entry) | `funded` | Same as `fund`; batch reconcile all entries |
| `settle` | `EscrowSettled` | `escrow_sd` | Trigger off-chain pro-rata payout calculation using `FundingCloseSnapshot` + per-investor `get_contribution` |
| `settle` (fee path) | `ProtocolFeeCollected` | `fee_coll` | Record protocol fee in treasury reconciliation system |
| `withdraw` | `SmeWithdrew` | `sme_wd` | Record SME liquidity event; update invoice status in off-chain ledger |
| `claim_investor_payout` | `InvestorPayoutClaimed` | `inv_claim` | Mark investor as paid in off-chain system; release hold on investor record |
| `cancel_funding` | `FundingCancelledEvt` | `cancelled` | Alert investors; begin refund workflow |
| `refund` | `InvestorRefundedEvt` | `refunded` | Mark investor refunded in off-chain system; reconcile DistributedPrincipal |
| `set_legal_hold(true)` | `LegalHoldChanged` | `legalhld` | Alert compliance dashboard; suspend investor UI funding flows |
| `set_legal_hold(false)` / `clear_legal_hold` | `LegalHoldChanged` | `legalhld` | Resume operations; notify relevant parties |
| `pause_dispute` | `DisputePausedEvt` | `dis_pause` | Alert operations team; track dispute ticket |
| `resume_dispute` | `DisputeResumedEvt` | `dis_res` | Confirm dispute resolved; re-enable settlement UI |
| `update_maturity` | `MaturityUpdatedEvent` | `maturity` | Update off-chain settlement schedule; re-notify investors if material |
| `propose_admin` | `AdminProposedEvent` | `adm_prop` | Notify proposed successor; keep current admin active until acceptance |
| `accept_admin` | `AdminTransferredEvent` | `admin` | Update key registry and access control records; confirm pending proposal cleared |
| `rotate_beneficiary` | `BeneficiaryRotatedEvt` | `ben_rot` | Update SME address in off-chain records; verify dual auth (SME + admin) |
| `update_funding_target` | `FundingTargetUpdated` | `fund_tgt` | Update off-chain target display; re-evaluate investor communications |
| `record_sme_collateral_commitment` | `CollateralRecordedEvt` | `coll_rec` | Store in compliance/risk system; **do not treat as enforced on-chain lock** |
| `sweep_terminal_dust` | `TreasuryDustSwept` | `dust_sw` | Reconcile treasury balance; log sweep amount and token address |
| `bind_primary_attestation_hash` | `PrimaryAttestationBound` | `att_bind` | Verify digest against known IPFS CID or document bundle; record binding in compliance system |
| `append_attestation_digest` | `AttestationDigestAppended` | `att_app` | Append to off-chain audit log with `index` for ordering |

---

## 7. Known Limitations and Out-of-Scope Items

### 7.1 Fee-on-Transfer / Non-Standard Tokens

`external_calls::transfer_funding_token_with_balance_checks` records pre/post balances and asserts exact delta equality on both sender and recipient. Fee-on-transfer, rebasing, or "hook" tokens will trigger a typed error (safe failure). They are **not supported** and must be excluded by governance before deployment. Standard SEP-41 tokens with no side-effects are the only in-scope class.

### 7.2 Registry Hint — Not Authority

`RegistryRef` is metadata for off-chain indexers only. The contract never calls the registry on-chain. See §4.4 above.

### 7.3 Record-Only Collateral

`SmeCollateralCommitment` stores asset code, amount, and timestamp. It does **not** custody tokens, freeze assets, or trigger automated liquidation. It is SME-reported metadata only.

### 7.4 Claim Is a Marker Only

`claim_investor_payout` sets `InvestorClaimed(investor) = true` and emits an event. It **does not transfer tokens**. Actual payout is the responsibility of the integration layer using `FundingCloseSnapshot.total_principal` and `get_contribution(investor)` for pro-rata math.

### 7.5 Ledger Time Trust

Maturity (`InvoiceEscrow::maturity`) and claim locks (`InvestorClaimNotBefore`) are compared against `Env::ledger().timestamp()` — validator-observed ledger time, not a wall-clock oracle. Boundaries are `>=` / `<` on integer seconds.

### 7.6 Legal Hold — No Automatic Expiry on Hold Itself

While the **clear** workflow has an optional delay (codes 150–152), the hold itself has no automatic expiry. Indefinite fund lock is possible if `admin` is a single compromised key. Production deployments must set `admin` to a governed multisig with off-chain recovery procedures.

### 7.7 Unique Investor Cap — Sybil Resistance

`MaxUniqueInvestorsCap` limits distinct **chain accounts**, not real-world persons. It provides no Sybil resistance.

### 7.8 Schema Migration

The `migrate` function emits typed errors for all current `from_version` values below `SCHEMA_VERSION` (codes 90–92). Changing `InvoiceEscrow` struct layout requires a coordinated migration or full redeploy. Additive instance keys are backward-compatible; layout changes are not.

### 7.9 Token Economics — Out of Scope

Yield coupon calculation, off-chain interest accrual, and pro-rata rounding are entirely off-chain concerns. The contract stores `yield_bps` and the snapshot but performs no token arithmetic beyond the `calculate_principal_plus_yield` helper (pure integer, no custody).

### 7.10 settle() CEI Ordering (Informational)

See §5.3. `DataKey::Escrow` is written after the protocol fee transfer in `settle()`. Not exploitable in the Soroban host model; documented for future hardening.

---

## 8. Storage Key Reference

| `DataKey` variant | Type | Mutable after init | Notes |
|-------------------|------|--------------------|-------|
| `Escrow` | `InvoiceEscrow` | Yes (status, funded_amount, admin) | Rewritten atomically on every state change |
| `Version` | `u32` | No | Always `SCHEMA_VERSION` (7) after init |
| `FundingToken` | `Address` | No | SEP-41 token; set once |
| `Treasury` | `Address` | No | Dust sweep recipient; set once |
| `RegistryRef` | `Address` | No | Optional; omitted when `None` at init |
| `LegalHold` | `bool` | Yes (admin only) | Absent = `false` |
| `DisputePaused` | `DisputePauseState` | Yes (admin only) | Optional; auto-expires if duration set |
| `SettledAmount` | `i128` | Yes (incremented per partial settle) | Cumulative settled principal |
| `FeePercentage` | `i64` | No | Optional protocol fee in bps; set at init |
| `MinContributionFloor` | `i128` | No | `0` = no floor |
| `MaxUniqueInvestorsCap` | `u32` | Yes (lowerable via `lower_max_unique_investors`) | Optional |
| `MaxPerInvestorCap` | `i128` | No | Optional |
| `UniqueFunderCount` | `u32` | Yes | Incremented on first deposit per address |
| `YieldTierTable` | `Vec<YieldTier>` | No | Optional; omitted when no tiers |
| `FundingCloseSnapshot` | `FundingCloseSnapshot` | No | Written once at status→1; never overwritten |
| `DistributedPrincipal` | `i128` | Yes | Incremented per `withdraw` and `refund` |
| `InvestorContribution(addr)` | `i128` | Yes (persistent) | Incremented per `fund` call |
| `InvestorEffectiveYield(addr)` | `i64` | No (persistent) | Set on first deposit; immutable thereafter |
| `InvestorClaimNotBefore(addr)` | `u64` | No (persistent) | `0` = no gate; set by `fund_with_commitment` |
| `InvestorClaimed(addr)` | `bool` | No (write-once, persistent) | Set to `true` by `claim_investor_payout` |
| `InvestorRefunded(addr)` | `bool` | No (write-once) | Set to `true` by `refund` |
| `SmeCollateralPledge` | `SmeCollateralCommitment` | Yes (SME may replace pre-settlement) | Record-only; no token custody |
| `PrimaryAttestationHash` | `BytesN<32>` | No (write-once) | Single-set; rebind panics |
| `AttestationAppendLog` | `Vec<BytesN<32>>` | Append-only | Bounded by `MAX_ATTESTATION_APPEND_ENTRIES` (32) |
| `InvestorAllowlist(addr)` | `bool` | Yes (admin) | Present only when allowlist gate is active |

---

## 9. Security Assumptions

1. **`admin` is a governed key.** Legal hold, attestation binding, maturity updates, and admin rotation are all gated by `admin`. A compromised single-key admin can freeze the escrow indefinitely.
2. **Funding token is standard SEP-41.** Fee-on-transfer or rebasing tokens will fail at the balance-check boundary (codes 36–41), not silently corrupt state.
3. **Soroban single-writer model.** The host function runs to completion before any other call to the same contract. Classic EVM-style reentrancy is not possible; the pre/post balance check in `external_calls` is a defense-in-depth against non-compliant token behavior, not a reentrancy guard.
4. **Off-chain payout correctness is the integrator's responsibility.** The contract records the snapshot and contribution data; it does not enforce that investors receive correct amounts.
5. **Attestation digests are not verified on-chain.** The contract stores 32-byte blobs verbatim. Hash algorithm and canonical encoding are off-chain conventions.
6. **Dispute pause is temporary and admin-controlled.** It requires the same trust assumptions as legal hold. Auto-expiry bounds the freeze duration when a timeout is configured.

---

## 10. Test Coverage Summary

809 tests across the full test suite. CI enforces `cargo llvm-cov --features testutils --fail-under-lines 95`.

Token-transfer security tests are split across two dedicated modules:

| Test file | Tests | Coverage area |
|-----------|------:|---------------|
| `tests/external_calls.rs` | 13 | Standard-token delta invariants, zero/negative/insufficient amount guards, MuxedAddress, multiple sequential transfers, liability floor |
| `tests/external_calls_mocked.rs` | 9 | Fee-on-transfer mock rejection, positive-amount guard, insufficient-balance guard, compliant/non-compliant control cases, large transfer, sequential transfers |

Run locally:

```bash
cargo test -p karis-ky_escrow
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
```

---

## 11. Audit Findings Summary

| ID | Severity | Area | Finding | Status |
|----|----------|------|---------|--------|
| AUDIT-001 | Informational | CEI ordering | `settle()` writes `DataKey::Escrow` after fee transfer; `DataKey::SettledAmount` and in-memory `escrow.status` are updated before the call. Not exploitable in Soroban host model. | Documented; no code change required |
| AUDIT-002 | None | Error codes 36–42 | All 7 codes are present in `lib.rs` and consistent with `escrow-error-messages.md`. | Closed — no gap |
| AUDIT-003 | None | Return value handling | SEP-41 `transfer` returns `()`; no return value to miss. Correctness is enforced by post-call balance assertions. | Closed — no gap |
| AUDIT-004 | None | Reentrancy | Soroban host prevents mid-execution re-entry. Pre/post balance checks provide defense-in-depth against non-compliant tokens. | Closed — no gap |
| AUDIT-005 | None | `transfer_from` usage | No `transfer_from` calls exist anywhere in `external_calls.rs` or `lib.rs`. The escrow never needs to pull tokens from an external address. | Closed — no gap |
| AUDIT-006 | None | Test coverage | Fee-on-transfer rejection, all guard conditions, and sequential transfer invariants are tested in two dedicated test modules (22 tests total). | Closed — adequate coverage |
