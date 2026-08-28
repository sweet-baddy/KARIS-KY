# Escrow Token Integration Security Checklist

This checklist describes the supported assumptions and explicit unsupported token behaviors for integrations that use the karis-ky escrow contract with cross-contract token assets.

## Supported token assumptions

- Amounts are recorded in the escrow contract as raw smallest units using `i128`.
  - Integration layers must convert external human-readable amounts into smallest units before calling `fund`.
  - Do not rely on asset decimals inside the escrow contract; the contract stores integer amounts only.
- The escrow contract does not itself perform token transfers or custody assets.
  - `record_sme_collateral_commitment` stores SME-reported metadata only and does not lock assets, verify custody, or create an enforceable on-chain claim.
  - Token movement must be handled separately by the integration layer.
- The contract uses strong signer authorization for state changes (`require_auth(...)` for admin, SME, and investor roles).
- Token asset identity should be established by token contract ID or audited registry, not by symbol alone.

## Integration-layer responsibilities

- Validate the token contract before use:
  - confirm the contract ID or hash is expected and audited
  - confirm the token contract is not paused, frozen, or blacklisted
  - confirm the token implements standard transfer semantics without hidden fees
- Normalize decimals outside the contract:
  - convert human-facing amounts into the token’s smallest unit
  - reject tokens with nonstandard decimals or dynamic fractional behavior
- Protect against malicious tokens:
  - do not integrate with fee-on-transfer or deflationary transfer tokens
  - do not integrate with tokens that have reentrant hooks or unexpected callback behavior
  - do not assume token contract invariants beyond the audited interface
- Use separate transfer preflight logic or atomic transfer flows to ensure on-chain escrow state matches actual token movement.

## Explicit unsupported token behavior warnings

The escrow contract and its documented assumptions do not support direct integration with the following token behaviors:

- Fee-on-transfer or deflationary tokens
- Paused, frozen, or blacklisted token contracts
- Nonstandard transfer semantics or callback-based reentrancy
- Dynamic decimals, fractional units outside integer smallest-unit semantics
- Malicious token contracts that alter balances in unexpected ways or change transfer metadata

## Terminal dust sweep (`sweep_terminal_dust`)

- The escrow uses [`escrow/src/external_calls.rs`](../escrow/src/external_calls.rs) to assert **exact** sender/recipient balance deltas for the configured **funding** token.
- Integrations must still treat **fee-on-transfer** and other non-standard tokens as **unsupported**; such tokens can cause the sweep to panic when deltas do not match `amount`.

## Custody reconciliation (`verify_asset_custody`)

- Admins or off-chain schedulers can call `verify_asset_custody()` to compare the escrow contract's current funding-token balance with the recorded `funded_amount`.
- The entrypoint returns a signed discrepancy (`contract_balance - recorded_funded_amount`) and emits an `AssetCustodyVerified` event for auditing.
- Reconcile the returned discrepancy with custody statements, bridge transfer logs, and any expected transfers before settlement, withdrawal, or any dust sweep.
- Treat non-zero discrepancies as a signal to pause downstream actions until the balance mismatch is investigated.

## Why this matters

Because the contract only records numeric state and collateral metadata (aside from the guarded dust sweep transfer path), token integration security is enforced by the surrounding application or bridge logic.

- The escrow contract is safe for algebraic accounting of on-chain amounts.
- The integration layer must reject unsupported token patterns before calling escrow entrypoints.
- The collateral commitment record is not an on-chain asset lock and should not be treated as proof of custody; see [`escrow-sme-collateral.md`](escrow-sme-collateral.md).

---

## Security audit findings (2026-08-26, schema v7)

A full audit of all token transfer call sites in `external_calls.rs` and `lib.rs` was completed.
Full report: [`docs/audit-handoff-escrow.md`](audit-handoff-escrow.md).

### Call sites audited

All on-chain token movement goes through one function:
`external_calls::transfer_funding_token_with_balance_checks`. There are exactly **4** call sites:

| Entrypoint | Transfer direction | CEI compliant |
|-----------|-------------------|:-------------:|
| `sweep_terminal_dust` | contract → treasury | ✅ |
| `settle` (protocol fee path) | contract → treasury | ⚠️ informational — see below |
| `withdraw` | contract → sme_address | ✅ |
| `refund` | contract → investor | ✅ |

### Findings

| ID | Severity | Finding |
|----|----------|---------|
| AUDIT-001 | Informational | `settle()` writes `DataKey::Escrow` to persistent storage after the fee transfer. `DataKey::SettledAmount` and the in-memory `escrow.status = 2` are both set before the call. Not exploitable in the Soroban host model (host functions run to completion with no mid-execution re-entry). Recommendation: move the `DataKey::Escrow` write before the fee transfer in a future refactor for strict CEI alignment. |
| AUDIT-002 | None | Error codes 36–42 are complete and consistent between `EscrowError` in `lib.rs` and the canonical reference in `escrow-error-messages.md`. No gaps. |
| AUDIT-003 | None | SEP-41 `transfer` returns `()`. There is no return value to miss. Correctness is enforced entirely by the post-call balance assertions inside the wrapper. |
| AUDIT-004 | None | Reentrancy is structurally prevented by the Soroban host model. Pre/post balance checks are defense-in-depth, not a reentrancy guard. |
| AUDIT-005 | None | No `transfer_from` calls exist anywhere in this contract. |
| AUDIT-006 | None | 22 dedicated external-calls tests cover fee-on-transfer mock rejection, zero/negative/insufficient-amount guards, large amounts, and sequential transfer invariants. |

### Token integration requirements (summary)

- ✅ Use standard SEP-41 tokens only (USDC, USDT, EURC, or equivalent).
- ❌ Do not use fee-on-transfer, rebasing, hook, paused, or blacklisted tokens — the wrapper will reject them with typed errors (codes 36–41) but they must be excluded before deployment.
- ✅ Confirm the token contract ID is audited and allowlisted by governance before calling `init`.
- ✅ Operators must ensure the contract holds sufficient token balance before calling `settle` (fee path), `withdraw`, or `refund`; insufficient balance produces a typed error (code 37).
