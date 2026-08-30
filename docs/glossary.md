# karis-ky Escrow — Glossary

Cross-team reference (legal, product, engineering) for terms used in the
on-chain contract, off-chain indexers, and user-facing copy.

All status values refer to `InvoiceEscrow.status` stored under
`DataKey::Escrow` in `escrow/src/lib.rs`.

---

## Invoice lifecycle status codes

| `status` | Name | Meaning | Allowed next status |
|----------|------|---------|---------------------|
| `0` | **Open** | Escrow initialised; accepting investor funding. | `1` (funded) |
| `1` | **Funded** | `funded_amount >= funding_target`; SME may withdraw or settle. | `2` (settled) or `3` (withdrawn) |
| `2` | **Settled** | SME called `settle` after maturity; investors may record payout claims. | — terminal |
| `3` | **Withdrawn** | SME called `withdraw`; liquidity pulled. No further settlement possible. | — terminal |

Transitions are **strictly forward**. No entrypoint moves `status` backward.
`settle` and `withdraw` are mutually exclusive paths from `funded` (both
require `status == 1`). See [ADR-001](adr/ADR-001-state-model.md).

**Terminal states:** `settled` (2) and `withdrawn` (3). Only terminal escrows
may have dust swept by the treasury (`sweep_terminal_dust`).

---

## Roles

### Admin
The Stellar address supplied at `init`. Controls governance-level operations:
setting/clearing legal hold, binding attestation hashes, updating maturity or
funding target (open state only), and transferring admin to a new address.

Production deployments should use a **multisig or governed contract** as admin
so that legal holds cannot be used for indefinite fund lock without off-chain
governance recovery. See [ADR-004](adr/ADR-004-legal-hold.md).

### SME (Small and Medium Enterprise)
The invoice originator. Receives stablecoin liquidity when the escrow is
funded (`withdraw`) and is responsible for calling `settle` after the invoice
matures. The SME address is set at `init` and cannot be changed.

### Investor
Any Stellar address that contributes principal via `fund` or
`fund_with_commitment`. Investors record a payout claim via
`claim_investor_payout` after the escrow is settled. Each investor's
contribution is stored under `DataKey::InvestorContribution(Address)`.

### Treasury
The protocol treasury address bound immutably at `init`. The only recipient of
`sweep_terminal_dust`. Must authorise dust-sweep calls. Cannot be changed after
deploy.

---

## Core terms

### Invoice escrow
A single deployed instance of the `LiquifactEscrow` contract that holds
investor funds for one tokenised invoice until settlement. Each instance binds
exactly one funding token, one SME, one treasury, and one admin.

### Invoice ID (`invoice_id`)
An ASCII alphanumeric + underscore identifier (max 32 chars) supplied at `init`
and stored as a Soroban `Symbol`. Must align with off-chain invoice slugs so
indexers remain unambiguous.

### Funding token
The SEP-41-compliant stablecoin contract bound at `init` under
`DataKey::FundingToken`. Immutable after deploy. Fee-on-transfer, rebasing, and
hook tokens are **explicitly out of scope** — see
[Token integration security](#token-integration-security) below.

### Funding target
`InvoiceEscrow.funding_target` — the principal amount (in token base units)
that must be reached for the escrow to transition from open → funded. Defaults
to `amount` at `init`; may be updated by admin while status is open.

### Funded amount
`InvoiceEscrow.funded_amount` — running total of principal credited across all
`fund` / `fund_with_commitment` calls. Incremented with `checked_add` to
prevent overflow.

### Maturity
`InvoiceEscrow.maturity` — a ledger timestamp (Unix seconds, validator-observed)
before which `settle` is blocked. `0` means no maturity gate. Enforced as
`ledger.timestamp() >= maturity` (inclusive). Updatable by admin while open.

### Yield (basis points)
`InvoiceEscrow.yield_bps` — the base annualised yield for this invoice in
integer basis points (`0–10 000`). Used as the floor for tiered yield
selection. Coupon arithmetic is performed **off-chain**; the contract stores
the rate only.

### Tiered yield / commitment lock
Optional ladder of `YieldTier { min_lock_secs, yield_bps }` set at `init` and
stored immutably under `DataKey::YieldTierTable`. An investor who calls
`fund_with_commitment` on their **first** deposit selects the best matching
tier; their effective rate is stored under
`DataKey::InvestorEffectiveYield(Address)`. Follow-on deposits must use `fund`.
If `committed_lock_secs > 0`, `DataKey::InvestorClaimNotBefore(Address)` is
set to `ledger.timestamp() + committed_lock_secs`, gating `claim_investor_payout`.
See [ADR-005](adr/ADR-005-tiered-yield.md).

### Funding-close snapshot
`FundingCloseSnapshot` written once, atomically, on the first transition to
`status == 1`. Contains `total_principal` (including overfunding past target),
`funding_target`, ledger timestamp, and sequence. **Immutable** thereafter.
Off-chain pro-rata share for an investor:
`get_contribution(addr) / snapshot.total_principal`. See
[ADR-003](adr/ADR-003-settlement-flow.md).

### Payout claim
An idempotency marker (`DataKey::InvestorClaimed(Address) = true`) set when an
investor calls `claim_investor_payout` after `status == 2`. The contract does
**not** transfer tokens; the integration layer handles actual payout using the
snapshot and contribution data.

### Legal / compliance hold
A boolean flag (`DataKey::LegalHold`) set by admin via `set_legal_hold`. While
active it blocks `settle`, `withdraw`, `claim_investor_payout`, and
`sweep_terminal_dust`. Cleared by the same admin path. See
[ADR-004](adr/ADR-004-legal-hold.md).

### Collateral commitment
`SmeCollateralCommitment` — a **ledger record only**. The SME may call
`record_sme_collateral_commitment` to log an asset symbol, amount, and
timestamp. This does **not** custody collateral, freeze tokens, or trigger
liquidation. It is metadata for transparency and indexing.

### Attestation
Two complementary audit mechanisms, both admin-only:

- **Primary attestation hash** (`DataKey::PrimaryAttestationHash`): a single
  32-byte digest (e.g. SHA-256 of a KYC/legal bundle). Single-set; cannot be
  overwritten.
- **Attestation append log** (`DataKey::AttestationAppendLog`): a bounded
  append-only list of digests (max `MAX_ATTESTATION_APPEND_ENTRIES = 32`) for
  versioned or incremental updates.

### Terminal dust sweep
`sweep_terminal_dust` moves at most `MAX_DUST_SWEEP_AMOUNT = 100 000 000` base
units of the funding token from the contract to the treasury per call. Only
permitted in terminal states (`status == 2` or `3`), blocked by legal hold,
and requires treasury auth. Intended for rounding residue — not for settling
live liabilities.

### Schema version
`SCHEMA_VERSION` (currently `7`) written to `DataKey::Version` at `init`. Used
to gate the `migrate` entrypoint. See the
[schema version changelog](../README.md#schema-version-changelog-datakeyversion).

### Dispute pause
A **temporary, bounded freeze** of escrow operations triggered by an admin
calling `pause_dispute(ticket_id, duration_secs)`. While active it blocks
`fund`, `settle`, `withdraw`, and `claim_investor_payout` with typed errors
(`DisputePausedBlocks*`). Unlike legal / compliance hold, a dispute pause is
**time-bounded** — it automatically expires when the ledger timestamp reaches
`expires_at_ledger_timestamp`, without requiring a separate admin call.

Dispute pause state is stored under `DataKey::DisputePaused` as a
`DisputePauseState` struct:

| Field | Type | Purpose |
|-------|------|---------|
| `ticket_id` | `String` | Support/dispute ticket reference (audit trail) |
| `paused_at_ledger_timestamp` | `u64` | Ledger timestamp when pause was activated |
| `expires_at_ledger_timestamp` | `u64` | Ledger timestamp at which auto-expiration occurs |

**Maximum duration:** `MAX_DISPUTE_PAUSE_DURATION_SECS = 1 209 600` (14 days).
Attempting to set a longer duration fails with
`EscrowError::DisputePauseDurationExceedsMax` (code 181). This cap ensures
disputes escalate to governance within a bounded operational window, consistent
with standard invoice-finance dispute SLAs (3–15 days).

**Auto-expiration** is checked lazily on each blocked operation: if
`ledger.timestamp() >= expires_at_ledger_timestamp`, the pause is considered
inactive even though the storage entry is not yet removed. Operators may call
`resume_dispute` to clear it explicitly before it expires.

**Interaction with legal hold:** Legal hold and dispute pause are independent
mechanisms. A legal hold may be active while a dispute pause is also active.
`resume_dispute` succeeds even when a legal hold is active; `clear_legal_hold`
succeeds even when a dispute pause is active. They do not unset each other.

Relevant entrypoints: `pause_dispute`, `resume_dispute`, `is_dispute_paused`,
`get_dispute_pause`. See [`docs/DEPLOYER_SECURITY.md`](DEPLOYER_SECURITY.md)
for operational guidance and [`docs/state-machine.md`](state-machine.md) for
the dispute-pause overlay on the state diagram.

### Circuit breaker
An operational **safety pattern** that halts a subset of escrow operations
when an anomalous condition is detected, protecting investor funds while
allowing the root cause to be investigated without permanently blocking the
contract.

In the karis-ky escrow, the circuit breaker concept is implemented via two
complementary mechanisms:

| Mechanism | Scope | Auto-reset | Auth to set | Auth to clear |
|-----------|-------|-----------|-------------|---------------|
| **Legal hold** (`DataKey::LegalHold`) | Permanent freeze of settle, withdraw, claim, dust sweep | No — manual admin clear | Admin | Admin (with optional `LegalHoldClearDelay`) |
| **Dispute pause** (`DataKey::DisputePaused`) | Temporary freeze of fund, settle, withdraw, claim | Yes — auto-expires at `expires_at_ledger_timestamp` | Admin | Admin (`resume_dispute`) or time-based |

An off-chain integrator layer may implement an additional circuit-breaker by
halting new invoice origination when on-chain `check_escrow_health()` returns
warning codes `4003` (over-maturity with underfunding) or when dispute-pause
activity crosses a configured threshold across multiple escrow instances.

**Circuit breaker is not a single contract primitive** — it is the combination
of governance policy, the typed error codes on blocked paths, and the admin's
ability to pause, hold, and resume operations independently of the escrow's
lifecycle state machine.

### Compaction
In the karis-ky escrow, "compaction" refers to two related but distinct
storage-reduction operations:

#### Delta snapshot compaction
The escrow's **delta-encoded snapshot** system (ADR-009) stores each state
change as a `SnapshotDelta` appended to a chain. As the chain grows, reading
or reconstructing historical state requires walking all prior deltas. Compaction
collapses the accumulated delta chain into a new `FullSnapshot` baseline and
resets the chain head, capping reconstruction cost at O(1) reads again.

Compaction is triggered operationally (future enhancement) when the delta
count exceeds a configured threshold — typically 50 deltas in a chain — and
is performed by the admin entrypoint. After compaction:

- The new `FullSnapshot` replaces the baseline for all future delta
  reconstruction.
- Old `SnapshotDelta(id)` entries remain on-chain for audit but are no longer
  needed for state reads.
- Off-chain indexers must be aware of the new baseline to correctly reconstruct
  historical state.

#### Soroban ledger entry archival (TTL / rent compaction)
Soroban charges "rent" for ledger entries based on their time-to-live (TTL).
Entries whose TTL reaches zero are **archived** — removed from hot state and
moved to cold storage. Archived entries cannot be read by contracts until
explicitly restored.

For escrow instances with long-lived funding periods or deferred claims, TTL
expiry is a live risk. The contract mitigates this via:

- **`bump_ttl` entrypoint** — permissionless; extends TTL for instance storage
  (escrow state, legal hold, allowlist flag, snapshot) and persistent storage
  (per-investor contribution, yield, claim-not-before). Any caller may invoke
  it without auth.
- **Named constants** — `INSTANCE_TTL_MIN_EXTENSION_LEDGERS` and
  `PERSISTENT_TTL_MIN_EXTENSION_LEDGERS` (both ≈ 1 h at 1 ledger/sec) define
  the minimum extension horizon.
- **Monotonic guarantee** — TTL extension never shortens an existing TTL.

Operators should include `bump_ttl` calls in monitoring scripts for any escrow
with a maturity more than 24 h in the future, and before any admin operation on
an escrow that has not been touched recently. Failure to extend TTL before
archival means state reads will panic until entries are restored through the
Soroban archive-restore mechanism.

See [`docs/escrow-gas-storage-notes.md`](escrow-gas-storage-notes.md) for the
full TTL semantics reference and [`docs/adr/ADR-009-delta-encoded-snapshots.md`](adr/ADR-009-delta-encoded-snapshots.md)
for the delta compaction design.

---

## Token integration security

The following assumptions apply to the funding token. Violations cause
safe-failure panics at the balance-check boundary in `external_calls.rs`.

| Assumption | Detail |
|------------|--------|
| SEP-41 compliant | Standard `transfer` semantics; sender decreases and recipient increases by exactly `amount`. |
| No fee-on-transfer | Post-transfer balance deltas must equal the requested amount on both sides. |
| No rebasing | Token balances must not change outside of explicit transfers. |
| No hook / callback tokens | Soroban does not allow classic EVM-style re-entrancy, but adversarial token logic is still out of scope. |

See [`docs/ESCROW_TOKEN_INTEGRATION_CHECKLIST.md`](ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)
for the full integration checklist.

---

## Out of scope

- **Token economics / coupon calculation** — yield values are stored as integer
  basis points; all coupon arithmetic is off-chain.
- **Collateral enforcement** — `record_sme_collateral_commitment` is a metadata
  record only; no on-chain liquidation is triggered.
- **Registry authority** — `DataKey::RegistryRef` is a discoverability hint for
  indexers; it is not an on-chain authority. Query the registry contract
  directly to verify membership.
- **Sybil resistance** — `UniqueFunderCount` / `MaxUniqueInvestorsCap` limit
  distinct chain accounts, not real-world persons.
- **Wall-clock time** — maturity and claim locks use validator-observed ledger
  timestamps (`Env::ledger().timestamp()`), not an external oracle.

---

## Related documents

| Document | Purpose |
|----------|---------|
| [ADR-001](adr/ADR-001-state-model.md) | Escrow state model and status transitions |
| [ADR-002](adr/ADR-002-auth-boundaries.md) | Authorization boundaries per role |
| [ADR-003](adr/ADR-003-settlement-flow.md) | Two-phase settlement flow and funding-close snapshot |
| [ADR-004](adr/ADR-004-legal-hold.md) | Legal / compliance hold mechanism |
| [ADR-005](adr/ADR-005-tiered-yield.md) | Optional tiered yield and commitment locks |
| [ADR-006](adr/ADR-006-dust-sweep-and-token-safety.md) | Treasury dust sweep and SEP-41 token safety |
| [ADR-009 (delta snapshots)](adr/ADR-009-delta-encoded-snapshots.md) | Delta-encoded snapshots and compaction design |
| [OPERATOR_RUNBOOK.md](OPERATOR_RUNBOOK.md) | Deploy, upgrade, rollback, and post-mortem procedures |
| [DEPLOYER_SECURITY.md](DEPLOYER_SECURITY.md) | Dispute pause and legal hold operational guidance |
| [ESCROW_TOKEN_INTEGRATION_CHECKLIST.md](ESCROW_TOKEN_INTEGRATION_CHECKLIST.md) | Token integration requirements and warnings |
| [EVENT_SCHEMA.md](EVENT_SCHEMA.md) | On-chain event definitions |
| [escrow-gas-storage-notes.md](escrow-gas-storage-notes.md) | TTL semantics, bump_ttl, and storage rent mitigation |
| [state-machine.md](state-machine.md) | State diagram with legal hold and dispute pause overlays |
