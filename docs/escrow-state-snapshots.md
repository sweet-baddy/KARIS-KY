# State Snapshot Recovery Guide

**Purpose:** Emergency recovery and rollback procedures for the karis-ky escrow contract.

## Quick Reference

| Operation | Entrypoint | Auth | Returns |
|-----------|-----------|------|---------|
| Create snapshot | `create_state_snapshot(name: String)` | Admin | None (emits event) |
| Revert to snapshot | `revert_to_snapshot(name: String)` | Admin | None (emits event) |
| List snapshots | Query events `StateSnapshotCreated` | None | Event history |
| Query current state | `get_escrow()` | None | Current `InvoiceEscrow` |

## Workflow: Creating a Snapshot

### Step 1: Prepare
```bash
# Verify current escrow state
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  --source <ADMIN_SECRET> \
  -- get_escrow
```

Output:
```json
{
  "admin": "G...",
  "sme_address": "G...",
  "invoice_id": "INV_001",
  "status": 1,  // 0=open, 1=funded, 2=settled, 3=withdrawn, 4=cancelled
  "funded_amount": 100_000_000_000,
  "amount": 100_000_000_000,
  "yield_bps": 800,
  "maturity": 1700000000,
  "funding_target": 100_000_000_000
}
```

### Step 2: Create Snapshot
```bash
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  --source <ADMIN_SECRET> \
  -- create_state_snapshot \
  --name "before_settlement"
```

Expected output: No error, event is emitted. Query the ledger to confirm:

```bash
soroban events --network testnet --start-ledger <LEDGER_NUM>
```

Look for `StateSnapshotCreated` event with:
- `snapshot_name`: `"before_settlement"`
- `created_at_ledger_timestamp`: Unix timestamp when snapshot was taken
- `created_by`: Admin address
- `escrow_snapshot`: Full escrow state at that moment

### Step 3: Verify
```bash
# Confirm the snapshot was stored by attempting to revert (will succeed)
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  --source <ADMIN_SECRET> \
  -- revert_to_snapshot \
  --name "before_settlement"

# Query state to confirm it reverted
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  --source <ADMIN_SECRET> \
  -- get_escrow
```

The escrow state should match what it was before the snapshot was created.

## Workflow: Reverting to a Snapshot

### When to Revert

**Legitimate scenarios:**
- Contract bug discovered and fixed off-chain (e.g., incorrect yield calculation in settlement).
- Admin policy violation (e.g., governance vote to reverse an invalid settlement).
- Data corruption from external misconfiguration.

**NOT legitimate:**
- Investor disputes over payout amounts (conduct audit instead).
- Disagreement with governance decision (do not revert without new governance approval).
- Testing/experimentation on production (use testnet).

### Step 1: Governance Approval (If Required)
Consult your operations policy. Depending on the escrow setup:
- **Multisig admin:** Obtain signatures from required signers.
- **Governance DAO:** Submit proposal and wait for vote approval.
- **Single admin:** Operator must document the decision and rationale.

### Step 2: Notify Stakeholders
- **SME:** Alert the SME that a state revert may occur and what it means for their position.
- **Investors:** Post a notice that the contract state will be rolled back to a specific ledger timestamp.
- **Indexers/Off-chain systems:** Pause automated indexing and alert your data team.

### Step 3: Backup Off-Chain State
```bash
# Export current investor records (if you maintain an off-chain ledger)
pg_dump escrow_db > /backup/escrow_db_pre_revert_$(date +%s).sql

# Export any pending investor claims or payouts
SELECT investor_address, claim_amount, claim_timestamp FROM investor_claims 
  WHERE claim_timestamp > '2024-07-28 00:00:00' 
  ORDER BY investor_address;
```

### Step 4: Execute Revert

```bash
# Confirm the snapshot exists (check event history)
soroban events --network testnet

# Revert to the snapshot
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  --source <ADMIN_SECRET> \
  -- revert_to_snapshot \
  --name "before_settlement"

# Verify the revert event was emitted
soroban events --network testnet --start-ledger <LEDGER_NUM> | grep StateSnapshotReverted
```

### Step 5: Verify State Consistency

```bash
# Check current escrow state
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  -- get_escrow

# Expected: matches the snapshot state

# Check that per-investor data is unchanged
# (investor contributions remain in persistent storage)
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  -- get_contribution \
  --investor <INVESTOR_ADDRESS>

# Expected: same as before revert (not rolled back)
```

### Step 6: Reconcile Off-Chain Records

**IMPORTANT:** Snapshots **DO NOT** revert per-investor persistent entries (contributions, yields, claims).

```
Scenario: Investor A funded 50M before snapshot. After snapshot was taken, 
Investor A funded another 30M, then settlement occurred.

After revert:
- On-chain `escrow.funded_amount` = 50M (reverted)
- On-chain `get_contribution(A)` = 80M (NOT reverted, still has both deposits)
- Off-chain records show A should be entitled to payout based on 80M contribution

Reconciliation:
- WRONG: Use 50M as the pro-rata denominator (inconsistent with storage)
- RIGHT: Audit the timing of A's deposits, manually adjust payout to account for 
         the second 30M deposit that occurred after the snapshot.
```

### Step 7: Document and Audit

```markdown
## Revert Log Entry

**Date:** 2024-07-28 14:30:00 UTC  
**Contract ID:** CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAU5HZ
**Snapshot Name:** `before_settlement`  
**Snapshot Timestamp:** 2024-07-28 13:00:00 UTC (Ledger 123456789)  
**Revert Reason:** Settlement calculation bug (double-counted yield)  
**Admin Authorization:** Multisig approval from [signer1, signer2, signer3]  
**Governance:** Approved by [vote ID or decision reference]  

**Affected Stakeholders:**
- SME: Alice (GAAAA...)
- Investors: 42 addresses, total funded = 100M

**Pre-Revert State:**
- funded_amount: 150M (over-funded due to bug)
- status: settled

**Post-Revert State:**
- funded_amount: 100M
- status: funded

**Reconciliation Steps Taken:**
1. Re-ran settlement math off-chain with corrected yield calc
2. Verified no investor contribution was lost (all still in persistent storage)
3. Issued revised payout schedule to all investors
4. SME will re-initiate withdrawal after corrected settlement

**Risk Mitigation:**
- Added test case for double-yield scenario
- Code review required for all yield-related changes
```

## Error Reference

| Error Code | Error Name | Cause | Resolution |
|-----------|-----------|-------|-----------|
| 170 | `InvalidSnapshotName` | Name is empty, too long (>32 chars), or contains disallowed characters | Use 1–32 alphanumeric + `_` |
| 171 | `SnapshotStorageCapacityReached` | More than 16 snapshots exist | Delete unused snapshots (manual cleanup of storage if needed) or contact chain validator |
| 172 | `SnapshotNotFound` | Snapshot name does not exist | Check snapshot name spelling; list all snapshots in event history |

## Risks and Mitigations

### Risk 1: Partial Revert Inconsistency

**Problem:** Investor contributions are NOT reverted when escrow state is rolled back.

**Example:**
- Snapshot taken: `funded_amount = 50M, investor A contribution = 50M`
- After snapshot: `funded_amount += 30M (investor B), investor B contribution = 30M`
- Revert: `funded_amount = 50M` but `investor B contribution = 30M` still exists

**Impact:** Off-chain pro-rata calculations may be inconsistent.

**Mitigation:**
- Always audit investor contributions before claiming payouts after a revert.
- Use `get_contribution(investor)` to verify individual balances.
- Recalculate pro-rata shares manually if total_principal was reverted.

### Risk 2: Admin Key Compromise

**Problem:** A compromised admin key can repeatedly revert the contract, undoing legitimate investor claims.

**Mitigation:**
- Use a multisig admin address so no single key can unilaterally revert.
- Require governance vote approval before revert.
- Monitor contract for suspicious revert events.
- Rotate admin keys regularly.

### Risk 3: Operator Error

**Problem:** Operator reverts to the wrong snapshot or snapshot is stale.

**Mitigation:**
- Always verify snapshot metadata (timestamp, name) before revert.
- Test revert procedures in testnet first.
- Require approval from a second operator.
- Document every revert with full context and rationale.

## Frequently Asked Questions

### Q: Will investors' contributions be restored if I revert?
**A:** No. Only the main `InvoiceEscrow` struct is reverted. Per-investor persistent entries (contributions, yields, claims) remain unchanged. You must audit and manually reconcile off-chain records.

### Q: Can I revert a reverted state (revert, then revert back)?
**A:** Yes. Snapshots are independent. You can revert to any existing named snapshot. However, be aware that reverting back will again leave investor-side data unchanged, potentially creating complex reconciliation scenarios. Avoid repeated reverts if possible.

### Q: What if I accidentally deleted a snapshot or it expired?
**A:** Snapshots are stored in instance storage and will not expire (instance TTL is managed by the ledger). If you need an older historical state that was never snapshotted, you will need to:
1. Check event logs (all `EscrowFunded`, `EscrowSettled` events are immutable and on-chain).
2. Reconstruct the state off-chain from the event log.
3. Consider taking regular snapshots (e.g., daily) as a best practice.

### Q: Can I snapshot during a legal hold?
**A:** Yes. Snapshots are admin-only and bypass legal holds (legal holds only block funding, settlement, and claims). However, reverting to a snapshot taken before a hold was activated will remove the hold if the snapshot state has `legal_hold = false`.

### Q: Do snapshot and revert operations cost more gas?
**A:** Snapshot creation requires writing two storage entries (metadata + state). Revert requires reading and writing the escrow state. Both are O(1) operations. In Soroban, storage ops dominate CPU cost, but snapshot operations are not prohibitively expensive. Compare to a full contract redeploy (which requires uploading WASM), snapshots are much cheaper.

## Support and Escalation

If you encounter issues during snapshot/revert:
1. **Verify admin authorization:** Ensure your admin key/multisig has required signatures.
2. **Check network:** Verify you are targeting the correct network (testnet vs mainnet).
3. **Review events:** Query `soroban events` to see if the operation succeeded.
4. **Contact governance:** If a revert requires approval or if you suspect a bug, escalate to karis-ky governance team.

## References

- [ADR-008: State Snapshots](adr/ADR-008-state-snapshots.md) — Architecture Decision Record
- [ADR-002: Auth Boundaries](adr/ADR-002-auth-boundaries.md) — Role-based authorization
- [OPERATOR_RUNBOOK.md](OPERATOR_RUNBOOK.md) — Full operational procedures
- `escrow/src/lib.rs` — Implementation source code
