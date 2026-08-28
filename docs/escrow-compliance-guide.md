# Escrow Compliance Officer Guide

This guide is for compliance officers responsible for monitoring escrows, interpreting legal holds,
managing sanctions integration, and reviewing audit logs. It assumes familiarity with the
karis-ky escrow contract and Soroban event indexing.

---

## Table of Contents

1. [Legal Hold: When and How to Trigger](#1-legal-hold-when-and-how-to-trigger)
2. [Snapshot Export for Compliance](#2-snapshot-export-for-compliance)
3. [Sanctions Integration Setup](#3-sanctions-integration-setup)
4. [Audit Log Interpretation](#4-audit-log-interpretation)
5. [Archival of Closed Escrows](#5-archival-of-closed-escrows)
6. [Settlement Notifier Monitoring](#6-settlement-notifier-monitoring)
7. [Registry Discovery](#7-registry-discovery)

---

## 1. Legal Hold: When and How to Trigger

### When to trigger a legal hold

A legal hold should be applied when:

- **Regulatory action**: A court order, regulatory directive, or law enforcement request requires freezing funds.
- **Sanctions screening**: A participant (SME, investor, or admin) appears on an updated sanctions list (OFAC, UN, EU, etc.).
- **Fraud investigation**: Internal risk review identifies suspicious funding patterns or anomalous activity.
- **Dispute resolution**: A legal dispute between SME and investors requires pausing all risk-bearing operations.

### What a legal hold blocks

| Operation | Blocked? | Notes |
|-----------|----------|-------|
| `fund()` / `fund_with_commitment()` | **Yes** | New investments are rejected |
| `settle()` | **Yes** | SME cannot finalize settlement |
| `withdraw()` | **Yes** | SME cannot pull liquidity |
| `claim_investor_payout()` | **Yes** | Investors cannot claim payouts |
| `cancel_funding()` | **Yes** | Admin cannot cancel funding |
| `sweep_terminal_dust()` | **Yes** | Treasury cannot sweep dust |
| `get_escrow()` / getters | **No** | Read-only access always works |
| `propose_admin()` / `accept_admin()` | **No** | Admin rotation is the recovery path |

### How to trigger a legal hold

The admin (governed multisig or DAO contract) calls:

```
set_legal_hold(env, active: true)
```

Or with the two-phase clear delay (recommended for production):

```
set_legal_hold(env, active: true)
```

### How to clear a legal hold

**Direct clear** (no delay configured):
```
set_legal_hold(env, active: false)
```

**Two-phase clear** (delay configured at init via `legal_hold_clear_delay`):
1. `request_clear_legal_hold()` — schedules a clearable-at timestamp
2. Wait until `env.ledger().timestamp() >= clearable_at`
3. `set_legal_hold(env, active: false)` — completes the clear

### Monitoring legal holds

Indexers should monitor the `LegalHoldChanged` event:

```
event LegalHoldChanged {
    name: "hold_chg",
    invoice_id: Symbol,
    active: u32  // 1 = enabled, 0 = cleared
}
```

**Alert thresholds**:
- Hold duration > 7 days: escalate to compliance lead
- Hold duration > 30 days: escalate to legal counsel
- Any hold on an escrow with `funded_amount > $X`: immediate escalation

### Recovery from lost admin key during a hold

If the admin key is lost while a hold is active:
1. Governance executes `propose_admin(new_admin)` using remaining multisig signers
2. New admin calls `accept_admin()`
3. New admin clears the hold

**Invariant**: A hold is always clearable by whoever holds `InvoiceEscrow::admin`.

---

## 2. Snapshot Export for Compliance

### What to export

Use the `get_escrow_summary()` entrypoint to capture a complete escrow state snapshot:

| Field | Purpose |
|-------|---------|
| `escrow.invoice_id` | Invoice identifier for cross-referencing |
| `escrow.status` | Current lifecycle status (0-5) |
| `escrow.funded_amount` | Total principal contributed |
| `escrow.sme_address` | Beneficiary receiving funds |
| `escrow.admin` | Admin controlling compliance gates |
| `legal_hold` | Whether a hold is active |
| `funding_close_snapshot` | Pro-rata denominator for payout verification |
| `unique_funder_count` | Number of distinct investors |
| `sme_collateral_commitment` | SME-reported collateral metadata |

### Export frequency

| Escrow status | Recommended frequency |
|---------------|----------------------|
| Open (0) / Funded (1) | Daily |
| Settled (2) / Withdrawn (3) | Weekly for 90 days, then monthly |
| Cancelled (4) | At cancellation and after all refunds |
| Archived (5) | On-demand only |

### Export automation

Set up an indexer or scheduled task that:
1. Calls `get_escrow_summary()` for each active escrow
2. Calls `get_contribution(addr)` for each known investor
3. Stores the snapshot in an immutable compliance archive (e.g., IPFS or append-only database)
4. Hashes the export bundle and optionally records it via `bind_primary_attestation_hash()`

---

## 3. Sanctions Integration Setup

### Architecture

```
External sanctions list (OFAC, UN, EU)
        │
        ▼
Compliance screening service
        │
        ├─ Match found ──► Trigger legal hold + block address
        │
        └─ No match ────► Continue monitoring
```

### On-chain enforcement options

**Option A: Investor allowlist (recommended for production)**

1. Enable the allowlist gate:
   ```
   set_investors_allowlisted(env, active: true)
   ```
2. Pre-screen investors against sanctions lists before allowlisting
3. Add screened investors in batches (up to 32 per call):
   ```
   set_investors_allowlisted(env, investors: Vec<Address>, allowed: true)
   ```
4. Remove sanctioned investors:
   ```
   set_investors_allowlisted(env, investors: Vec<Address>, allowed: false)
   ```

**Option B: Legal hold (emergency response)**

For urgent sanctions matches on already-funded escrows:
1. Apply legal hold immediately: `set_legal_hold(env, active: true)`
2. Follow your jurisdiction's asset freeze procedures
3. Coordinate with legal counsel before clearing

### Screening integration checklist

- [ ] Connect sanctions list API (OFAC SDN, UN Consolidated, EU Consolidated)
- [ ] Index all `EscrowInitialized` events to discover new escrows
- [ ] Screen `sme_address`, `admin`, and all investor addresses on each new escrow
- [ ] Re-screen on every `EscrowFunded` event (new investor joins)
- [ ] Maintain an audit trail of all screening decisions
- [ ] Configure alerting for positive matches (Slack, email, PagerDuty)
- [ ] Run a testnet drill: detect a test address, apply hold, verify blocks, clear hold, verify recovery

### Address monitoring events

| Event to monitor | When | Action |
|-----------------|------|--------|
| `EscrowInitialized` | New escrow created | Screen all parties |
| `EscrowFunded` | New investor contributes | Screen investor |
| `AdminProposedEvent` | Admin handover initiated | Screen proposed admin |
| `BeneficiaryRotated` | SME address changed | Screen new SME |

---

## 4. Audit Log Interpretation

### On-chain audit trail components

The escrow contract provides several audit mechanisms:

#### Primary Attestation Hash (`DataKey::PrimaryAttestationHash`)

A single 32-byte digest (e.g., SHA-256) bound once by the admin. Typically represents:
- A legal document bundle
- A KYC/KYB verification package
- An off-chain compliance report

**Read**: `get_primary_attestation_hash()` → `Option<BytesN<32>>`

**Interpretation**:
- `None`: No attestation has been bound yet
- `Some(hash)`: A document bundle was recorded. Verify the hash against your document store.

#### Attestation Append Log (`DataKey::AttestationAppendLog`)

An append-only chain of up to 32 digests, each representing an audit event:

**Read**: `get_attestation_append_log()` → `Vec<BytesN<32>>`

**Interpretation example**:

| Index | Digest (first 8 hex chars) | Meaning |
|-------|---------------------------|---------|
| 0 | `a1b2c3d4...` | Initial KYC bundle for SME |
| 1 | `e5f6a7b8...` | Investor KYC batch #1 |
| 2 | `c9d0e1f2...` | Quarterly compliance review |
| 3 | `34567890...` | Sanctions re-screening report |

**Revocation**: `get_attestation_revoked(index)` indicates whether an entry has been superseded.

#### Event-based audit trail

The complete event stream provides a cryptographically verifiable audit trail:

| Event | Audit significance |
|-------|-------------------|
| `EscrowInitialized` | Creation timestamp, bound addresses |
| `EscrowFunded` | Each investor contribution with timestamp |
| `LegalHoldChanged` | Every hold activation and clearance |
| `LegalHoldClearRequested` | Scheduled hold clearance |
| `EscrowSettled` | Settlement finalization with timestamp |
| `SmeWithdrew` | SME liquidity withdrawal |
| `InvestorPayoutClaimed` | Investor claim markers |
| `FundingCancelled` | Admin cancellation |
| `InvestorRefundedEvt` | Each investor refund |
| `EscrowArchived` | Archival with prior status |
| `SettlementNotifierInvoked` | External system notification |
| `CollateralRecordedEvt` | SME collateral metadata updates |
| `BeneficiaryRotated` | SME address changes |
| `AdminProposedEvent` / `AdminTransferredEvent` | Admin key rotation |

### Building an audit report

To generate a compliance audit report for a specific escrow:

1. Call `get_escrow_summary()` for current state
2. Query all events filtered by `invoice_id`
3. For each investor, call `get_contribution(address)` and `is_investor_claimed(address)`
4. Call `get_primary_attestation_hash()` and `get_attestation_append_log()`
5. Cross-reference event timestamps with off-chain compliance records
6. Verify that no risk-bearing operation occurred during active legal holds

### Example: Detecting a hold violation

```python
# Pseudocode for hold violation detection
events = indexer.query_events(invoice_id=escrow_id, order="asc")
hold_active = False
hold_start = None

for event in events:
    if event.type == "LegalHoldChanged":
        hold_active = (event.active == 1)
        hold_start = event.timestamp if hold_active else None
    
    if hold_active and event.type in ["EscrowFunded", "EscrowSettled", "SmeWithdrew",
                                       "InvestorPayoutClaimed", "FundingCancelled",
                                       "TreasuryDustSwept"]:
        alert(f"POTENTIAL VIOLATION: {event.type} at {event.timestamp} "
              f"while hold was active (applied at {hold_start})")
```

---

## 5. Archival of Closed Escrows

### When to archive

Archive an escrow when:
- All investor claims have been processed (settled escrows)
- All SME funds have been withdrawn (withdrawn escrows)
- All investor refunds have been completed (cancelled escrows)
- The escrow is at least 90 days past its terminal transition
- No pending disputes or investigations

### How to archive

The admin calls:

```
archive_escrow(env) → InvoiceEscrow
```

**Preconditions**:
- Escrow must be in a terminal state: settled (2), withdrawn (3), or cancelled (4)
- Admin authorization required

**Post-archive**:
- Status becomes 5 (archived)
- All read-only operations continue to work
- `sweep_terminal_dust()` still permitted
- Indexers should exclude status-5 escrows from active monitoring dashboards

### Archival event

```
event EscrowArchived {
    name: "esc_arch",
    invoice_id: Symbol,
    prior_status: u32,
    archived_at_ledger_timestamp: u64
}
```

---

## 6. Settlement Notifier Monitoring

### What the notifier does

When configured at `init` via `settlement_notifier_contract`, the settlement notifier is an external
contract that receives settlement details. Use cases include:
- Updating an off-chain ledger or accounting system
- Triggering payment processing
- Recording settlement in a registry

### How it works

1. `settle()` finalizes settlement and emits `EscrowSettled` — the notifier is **not** called inline
2. An off-chain relayer or indexer watches for `EscrowSettled` and calls `notify_settlement()`
3. `notify_settlement()` invokes the configured notifier contract with settlement details
4. Graceful failure: a broken notifier never blocks settlement; `notify_settlement()` can be retried independently

### Monitoring the notifier

Watch for the `SettlementNotifierInvoked` event:

```
event SettlementNotifierInvoked {
    name: "notify_ok",
    invoice_id: Symbol,
    notifier_contract: Address,
    funded_amount: i128,
    yield_bps: i64,
    settled_at_ledger_timestamp: u64
}
```

If this event is missing after an `EscrowSettled` event, the notifier may have been misconfigured
or the `settle()` call was a retry that didn't re-invoke the notifier.

---

## 7. Registry Discovery

### Finding active escrows

Use `get_registry_listing()` to retrieve discovery metadata for any escrow:

```
get_registry_listing() → RegistryListing {
    escrow_address: Address,
    invoice_id: Symbol,
    sme_address: Address,
    created_at: u64,
    status: u32,
    funding_target: i128
}
```

### Filtering for active monitoring

Off-chain indexers should:
- Query all known escrow addresses
- Filter out status 5 (archived) for active monitoring
- Include status 5 only in historical/audit queries

---

## Quick Reference Card

| Situation | Action | Entrypoint |
|-----------|--------|------------|
| Sanctions match on SME | Apply legal hold | `set_legal_hold(true)` |
| Sanctions match on investor | Remove from allowlist + hold | `set_investors_allowlisted` + `set_legal_hold(true)` |
| Court order received | Apply legal hold | `set_legal_hold(true)` |
| Investigation complete, no violation | Clear legal hold | `set_legal_hold(false)` or two-phase clear |
| Quarterly audit | Export snapshot | `get_escrow_summary()` |
| Close inactive escrow | Archive it | `archive_escrow()` |
| Verify KYC on file | Check attestation | `get_primary_attestation_hash()` |
| Settlement notification missed | Retry notifier | `notify_settlement()` |
| Discover all escrows | Query registry listing | `get_registry_listing()` |

---

## Related Documents

- [Legal Hold Security Reference](escrow-legal-hold.md)
- [ADR-004: Legal Hold](adr/ADR-004-legal-hold.md)
- [Escrow Lifecycle](escrow-lifecycle.md)
- [Operator Runbook](OPERATOR_RUNBOOK.md)
- [Event Schema](EVENT_SCHEMA.md)
- [Escrow Snapshots](escrow-snapshot.md)
