# SME Collateral Metadata Audit Trail Guide

This guide explains how risk teams and off-chain auditors can query, interpret, and verify SME collateral commitment metadata recorded on-chain through the karis-ky escrow contract.

---

## Critical Disclaimer

> **⚠️ THIS IS METADATA ONLY.** The `record_sme_collateral_commitment` entrypoint and stored `SmeCollateralCommitment` data are **not** proof of asset custody, collateral control, lien perfection, or enforceable on-chain claims. They are SME-reported metadata written to the ledger for auditing and off-chain risk review.
>
> This contract does **not**:
> - Transfer or move tokens
> - Freeze or lock balances
> - Verify SME custody or ownership
> - Enforce collateral restrictions
> - Prevent any settlement or withdrawal flows
>
> Treat all indexed records as **reported metadata** and verify custody and control through separate, independent channels.

---

## Record Format

### On-Chain Storage

Collateral commitments are stored under a single key per escrow instance:

```rust
// In escrow/src/lib.rs
pub enum DataKey {
    // ...
    SmeCollateralPledge,  // Stores SmeCollateralCommitment (single, replaces on re-record)
    // ...
}

#[contracttype]
pub struct SmeCollateralCommitment {
    pub asset: Symbol,        // Off-chain asset label (e.g., "USDC", "GOLD", "COMMODITY_X")
    pub amount: i128,         // Reported amount (in base units; not transferred)
    pub recorded_at: u64,     // Soroban ledger timestamp (seconds since epoch)
}
```

**Key properties:**
- **Single record per escrow:** Each escrow stores only one commitment. Calling `record_sme_collateral_commitment` replaces any prior record.
- **Ledger timestamp:** Uses Soroban validator-observed ledger time, not wall-clock time. Skew between testnet/mainnet is normal.
- **Immutable after write:** Once recorded, the data is immutable on-chain (but can be replaced by calling `record_sme_collateral_commitment` again with new values).

### Event Emission

Every successful call emits a `CollateralRecordedEvt`:

```rust
#[contractevent]
pub struct CollateralRecordedEvt {
    #[topic]
    pub name: Symbol,                // Fixed: "coll_rec"
    #[topic]
    pub invoice_id: Symbol,          // Invoice identifier for filtering
    pub amount: i128,                // Newly recorded amount
    pub prior_amount: i128,          // Previous amount (0 if first record)
}
```

**Event semantics:**
- Emitted after successful validation and storage write
- Captures replacement semantics: `prior_amount` allows auditors to track changes
- Indexed by `name` and `invoice_id` for efficient filtering in off-chain indexers

---

## Querying Historical Records

### Using the Escrow Client (TypeScript)

```typescript
import { EscrowClient } from "@karis-ky/escrow-sdk";

async function auditCollateralRecord(
    client: EscrowClient,
    invoiceId: string
): Promise<void> {
    try {
        // Fetch current collateral commitment
        const commitment = await client.getSmeCollateralCommitment();

        if (!commitment) {
            console.log("No collateral record for this escrow");
            return;
        }

        console.log("Current collateral commitment:");
        console.log(`  Asset: ${commitment.asset}`);
        console.log(`  Amount: ${commitment.amount}`);
        console.log(`  Recorded at: ${new Date(commitment.recorded_at * 1000).toISOString()}`);
    } catch (error) {
        console.error("Failed to fetch collateral record:", error);
    }
}

// Usage
const client = new EscrowClient({
    rpcUrl: "https://soroban-testnet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
    contractId: "CXYZ...",
});

await auditCollateralRecord(client, "INVOICE-2026-001");
```

### Using the Rust SDK

```rust
use soroban_sdk::{Address, Env, Symbol};
use karis_ky_escrow::{LiquifactEscrow, SmeCollateralCommitment};

fn audit_collateral_record(env: &Env, contract_id: &str) {
    let client = LiquifactEscrow::client(env, &contract_id);

    // Fetch current commitment
    match client.try_get_sme_collateral_commitment() {
        Ok(Some(commitment)) => {
            println!("Current collateral commitment:");
            println!("  Asset: {}", commitment.asset);
            println!("  Amount: {}", commitment.amount);
            println!("  Recorded at: {}", commitment.recorded_at);
        }
        Ok(None) => {
            println!("No collateral record for this escrow");
        }
        Err(e) => {
            eprintln!("Failed to fetch record: {:?}", e);
        }
    }
}
```

### Via Soroban RPC (Low-Level)

```bash
#!/usr/bin/env bash

# Query the collateral record directly via RPC

CONTRACT_ID="CXYZ..."
SOROBAN_RPC_URL="https://soroban-testnet.stellar.org"

# Construct the ledger entry key for SmeCollateralPledge
# (XDR encoding required; normally handled by SDK)

curl -s -X POST "$SOROBAN_RPC_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getLedgerEntries",
    "params": {
      "keys": [
        "AAAAAA..." // XDR-encoded ledger key for DataKey::SmeCollateralPledge
      ]
    }
  }' | jq .
```

---

## Event-Based Audit Trail

Off-chain indexers track `CollateralRecordedEvt` to maintain a complete audit trail:

### TypeScript Indexer Example

```typescript
import { Soroban, SorobanEvent } from "@stellar/stellar-sdk";

async function indexCollateralEvents(
    contractId: string,
    startLedger: number = 0
): Promise<CollateralAuditEntry[]> {
    const auditTrail: CollateralAuditEntry[] = [];

    // Fetch events from network
    const events = await fetchSorobanEvents({
        contractId,
        topic: ["coll_rec"],  // Event name
        startLedger,
    });

    for (const event of events) {
        const entry: CollateralAuditEntry = {
            invoiceId: event.topics[1],
            amount: event.data.amount,
            priorAmount: event.data.prior_amount,
            recordedAt: event.ledger_timestamp,
            txHash: event.tx_hash,
            ledgerSeq: event.ledger_sequence,
        };
        auditTrail.push(entry);
    }

    return auditTrail;
}

interface CollateralAuditEntry {
    invoiceId: string;
    amount: string;  // i128 as string
    priorAmount: string;
    recordedAt: number;
    txHash: string;
    ledgerSeq: number;
}

// Usage
const trail = await indexCollateralEvents("CXYZ...", 12345);
for (const entry of trail) {
    console.log(`Invoice ${entry.invoiceId}:`);
    console.log(`  ${entry.priorAmount} → ${entry.amount} (at ledger ${entry.ledgerSeq})`);
    console.log(`  Tx: ${entry.txHash}`);
}
```

### Historical Query Pattern

```typescript
/**
 * Reconstruct the collateral commitment state at a specific ledger.
 * Replays all CollateralRecordedEvt up to that point.
 */
async function reconstructCollateralStateAtLedger(
    contractId: string,
    targetLedger: number
): Promise<SmeCollateralCommitment | null> {
    const events = await fetchSorobanEvents({
        contractId,
        topic: ["coll_rec"],
        endLedger: targetLedger,
    });

    if (events.length === 0) {
        return null;
    }

    // Use the last event up to targetLedger
    const lastEvent = events[events.length - 1];

    return {
        asset: lastEvent.topics[1],
        amount: lastEvent.data.amount,
        recorded_at: lastEvent.ledger_timestamp,
    };
}
```

---

## Validation & Verification

### Checklist: Is This Record Trustworthy?

Before relying on a collateral record, verify:

- [ ] **Authorized signer:** The transaction was signed by the correct SME address (linked to the invoice)
- [ ] **Contract integrity:** The contract at `contract_id` matches the known karis-ky escrow deployment
- [ ] **Event authenticity:** The `CollateralRecordedEvt` was emitted by the contract (check event source in ledger)
- [ ] **Escrow existence:** The escrow has been initialized (check `get_escrow` returns data)
- [ ] **On-chain record read:** Query the contract directly using `get_sme_collateral_commitment()` to confirm the current state

### Example Verification Script

```typescript
import { EscrowClient, EscrowErrorCode } from "@karis-ky/escrow-sdk";
import { stellar } from "@stellar/stellar-sdk";

async function verifyCollateralRecord(
    client: EscrowClient,
    invoiceId: string,
    smeAddress: string
): Promise<{ valid: boolean; errors: string[] }> {
    const errors: string[] = [];

    // 1. Verify contract exists
    try {
        const version = await client.getVersion();
        console.log(`✓ Contract deployed at version ${version}`);
    } catch (e) {
        errors.push("Contract not found or not initialized");
        return { valid: false, errors };
    }

    // 2. Verify escrow exists
    try {
        const escrow = await client.getEscrow();
        console.log(`✓ Escrow initialized for invoice ${escrow.invoice_id}`);
    } catch (e) {
        errors.push(`Escrow not initialized: ${e}`);
        return { valid: false, errors };
    }

    // 3. Verify SME address matches
    try {
        const escrow = await client.getEscrow();
        if (escrow.sme_address !== smeAddress) {
            errors.push(
                `SME mismatch: contract has ${escrow.sme_address}, expected ${smeAddress}`
            );
        } else {
            console.log(`✓ SME address verified: ${smeAddress}`);
        }
    } catch (e) {
        errors.push(`Failed to read escrow: ${e}`);
    }

    // 4. Fetch collateral record
    let commitment: any;
    try {
        commitment = await client.getSmeCollateralCommitment();
        if (commitment) {
            console.log(`✓ Collateral record found:`);
            console.log(`  Asset: ${commitment.asset}`);
            console.log(`  Amount: ${commitment.amount}`);
            console.log(`  Recorded at: ${commitment.recorded_at}`);
        } else {
            console.log("⚠ No collateral record on-chain");
        }
    } catch (e) {
        errors.push(`Failed to fetch collateral: ${e}`);
    }

    // 5. Verify timestamp is reasonable (not in future)
    if (commitment && commitment.recorded_at) {
        const now = Math.floor(Date.now() / 1000);
        if (commitment.recorded_at > now) {
            errors.push(`Recorded timestamp is in the future (${commitment.recorded_at} > ${now})`);
        } else {
            console.log(`✓ Timestamp is valid`);
        }
    }

    return {
        valid: errors.length === 0,
        errors,
    };
}

// Usage
const client = new EscrowClient({
    rpcUrl: "https://soroban-testnet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
    contractId: "CXYZ...",
});

const result = await verifyCollateralRecord(
    client,
    "INVOICE-2026-001",
    "GA..." // SME address
);

if (result.valid) {
    console.log("✓ Record is valid");
} else {
    console.log("✗ Record validation failed:");
    for (const error of result.errors) {
        console.log(`  - ${error}`);
    }
}
```

---

## Audit Workflow

### Step 1: Identify Invoices with Collateral Records

```typescript
async function findInvoicesWithCollateral(
    indexerDb: Database,
    escrowContractIds: string[]
): Promise<InvoiceCollateralRecord[]> {
    const records: InvoiceCollateralRecord[] = [];

    for (const contractId of escrowContractIds) {
        const client = new EscrowClient({
            contractId,
            rpcUrl: rpcUrl,
            networkPassphrase,
        });

        try {
            const collateral = await client.getSmeCollateralCommitment();
            if (collateral) {
                const escrow = await client.getEscrow();
                records.push({
                    invoiceId: escrow.invoice_id,
                    contractId,
                    smeAddress: escrow.sme_address,
                    collateral,
                });
            }
        } catch (e) {
            console.warn(`Failed to check ${contractId}: ${e}`);
        }
    }

    return records;
}
```

### Step 2: Verify Custody Independently

```typescript
/**
 * Cross-reference on-chain collateral record with off-chain custody verification.
 * This is performed by risk/legal teams outside the contract.
 */
async function verifyCustodyOffChain(
    invoiceId: string,
    smeAddress: string,
    reportedAsset: string,
    reportedAmount: string,
    custodySystemDb: Database
): Promise<CustodyVerification> {
    // Query the custody system (e.g., bank account, deposit registry, vault provider)
    const custodyRecord = await custodySystemDb.query({
        invoiceId,
        smeAddress,
        assetType: reportedAsset,
    });

    if (!custodyRecord) {
        return {
            verified: false,
            reason: "No custody record found in external system",
        };
    }

    // Verify amount matches (within tolerance)
    const TOLERANCE = 0.01;  // 1% tolerance for rounding
    const custodyAmount = parseFloat(custodyRecord.amount);
    const reportedAmountNum = parseFloat(reportedAmount);
    const diff = Math.abs(custodyAmount - reportedAmountNum) / custodyAmount;

    if (diff > TOLERANCE) {
        return {
            verified: false,
            reason: `Amount mismatch: ${custodyAmount} (custody) vs ${reportedAmountNum} (on-chain)`,
        };
    }

    return {
        verified: true,
        custodyRecord,
    };
}
```

### Step 3: Generate Audit Report

```typescript
interface AuditReport {
    invoiceId: string;
    contractId: string;
    onChainRecord: SmeCollateralCommitment;
    offChainVerification: CustodyVerification;
    riskAssessment: {
        trustLevel: "high" | "medium" | "low" | "unverified";
        notes: string;
        flags: string[];
    };
}

async function generateAuditReport(
    invoiceId: string,
    contractId: string,
    riskTeamNotes: string = ""
): Promise<AuditReport> {
    const client = new EscrowClient({ contractId, rpcUrl, networkPassphrase });

    const escrow = await client.getEscrow();
    const onChainRecord = await client.getSmeCollateralCommitment();

    if (!onChainRecord) {
        return {
            invoiceId,
            contractId,
            onChainRecord: null,
            offChainVerification: { verified: false, reason: "No on-chain record" },
            riskAssessment: {
                trustLevel: "unverified",
                notes: "No collateral record on-chain",
                flags: ["NO_COLLATERAL_RECORD"],
            },
        };
    }

    const custody = await verifyCustodyOffChain(
        invoiceId,
        escrow.sme_address,
        onChainRecord.asset,
        onChainRecord.amount,
        custodyDb
    );

    const trustLevel = custody.verified ? "high" : "low";
    const flags: string[] = [];

    if (!custody.verified) {
        flags.push("CUSTODY_UNVERIFIED");
    }

    // Check for stale records (>30 days)
    const now = Math.floor(Date.now() / 1000);
    if (now - onChainRecord.recorded_at > 30 * 86400) {
        flags.push("STALE_RECORD");
    }

    return {
        invoiceId,
        contractId,
        onChainRecord,
        offChainVerification: custody,
        riskAssessment: {
            trustLevel,
            notes: riskTeamNotes || "Audit completed",
            flags,
        },
    };
}
```

---

## Example Audit Script

Save as `scripts/audit-collateral.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# karis-ky Collateral Metadata Audit Script
# Queries on-chain collateral records and generates risk report.
#
# Usage:
#   bash scripts/audit-collateral.sh \
#     --contract CXYZ... \
#     --rpc-url https://soroban-testnet.stellar.org \
#     --network testnet \
#     --output report.json

CONTRACT_ID=""
RPC_URL=""
NETWORK=""
OUTPUT_FILE="collateral-audit-report.json"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --contract) CONTRACT_ID="$2"; shift 2 ;;
    --rpc-url) RPC_URL="$2"; shift 2 ;;
    --network) NETWORK="$2"; shift 2 ;;
    --output) OUTPUT_FILE="$2"; shift 2 ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

if [ -z "$CONTRACT_ID" ] || [ -z "$RPC_URL" ]; then
  echo "Usage: $0 --contract <id> --rpc-url <url> --network <network> [--output <file>]"
  exit 1
fi

echo "Auditing collateral record for $CONTRACT_ID..."

# Use stellar CLI to query contract state
COLLATERAL_RECORD=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --operation get_sme_collateral_commitment \
  --rpc-url "$RPC_URL" \
  --network "$NETWORK" 2>/dev/null || echo "{}")

ESCROW=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --operation get_escrow \
  --rpc-url "$RPC_URL" \
  --network "$NETWORK" 2>/dev/null || echo "{}")

# Write report
cat > "$OUTPUT_FILE" <<EOF
{
  "audit_timestamp": $(date +%s),
  "contract_id": "$CONTRACT_ID",
  "network": "$NETWORK",
  "collateral_record": $COLLATERAL_RECORD,
  "escrow": $ESCROW,
  "notes": "On-chain collateral record is metadata only. Verify custody independently."
}
EOF

echo "✓ Audit report written to $OUTPUT_FILE"
```

---

## Common Audit Scenarios

### Scenario 1: Verify No Collateral Record

```typescript
const commitment = await client.getSmeCollateralCommitment();
if (commitment === null) {
    console.log("✓ No collateral record (expected for this invoice)");
} else {
    console.log("✗ Unexpected collateral record:", commitment);
}
```

### Scenario 2: Track Collateral Record Changes

```typescript
/**
 * Monitor for changes to collateral commitment over time.
 * Useful for detecting SME adjustments or disputes.
 */
async function monitorCollateralChanges(
    contractId: string,
    pollIntervalSeconds: number = 3600
) {
    const client = new EscrowClient({ contractId, rpcUrl, networkPassphrase });
    let lastRecord = await client.getSmeCollateralCommitment();

    setInterval(async () => {
        const currentRecord = await client.getSmeCollateralCommitment();

        if (JSON.stringify(lastRecord) !== JSON.stringify(currentRecord)) {
            console.log("Collateral record changed:");
            console.log("  Before:", lastRecord);
            console.log("  After:", currentRecord);

            // Alert risk team, audit log, etc.
            logAuditEvent("COLLATERAL_CHANGED", {
                contractId,
                before: lastRecord,
                after: currentRecord,
            });
        }

        lastRecord = currentRecord;
    }, pollIntervalSeconds * 1000);
}
```

### Scenario 3: Audit Trail for Multi-Party Ledger

```typescript
/**
 * For complex multi-party settlements, reconstruct the complete audit trail
 * of collateral commitments across all invoices.
 */
async function generateMultiInvoiceAuditTrail(
    invoiceIds: string[],
    contractIds: string[]
): Promise<AuditTrail[]> {
    const trail: AuditTrail[] = [];

    for (let i = 0; i < invoiceIds.length; i++) {
        const client = new EscrowClient({
            contractId: contractIds[i],
            rpcUrl,
            networkPassphrase,
        });

        const escrow = await client.getEscrow();
        const collateral = await client.getSmeCollateralCommitment();

        trail.push({
            invoiceId: invoiceIds[i],
            contractId: contractIds[i],
            smeAddress: escrow.sme_address,
            collateralAsset: collateral?.asset || "NONE",
            collateralAmount: collateral?.amount || "0",
            recordedAt: collateral?.recorded_at || 0,
            escrowStatus: escrow.status,
            fundedAmount: escrow.funded_amount,
        });
    }

    return trail;
}
```

---

## Important Caveats

- **Metadata only:** This data is not enforced on-chain. Settlement, withdrawal, and claims proceed regardless of collateral commitments.
- **Self-reported:** Only the SME can submit these records. There is no external verification mechanism in the contract.
- **Immutable (until replaced):** Once recorded, the data cannot be changed on-chain except by calling `record_sme_collateral_commitment()` again.
- **No token movement:** Calling this entrypoint does not transfer or reserve any assets.
- **Risk team responsibility:** Off-chain audit, custody verification, and enforcement are the responsibility of risk teams and governance.

---

## Summary

1. **Query the record** using `get_sme_collateral_commitment()` or via RPC.
2. **Verify SME identity** against the escrow's `sme_address`.
3. **Cross-reference custody** using off-chain systems (bank statements, registries, vault providers).
4. **Index events** to maintain a complete historical audit trail.
5. **Generate reports** for risk team review and compliance workflows.
6. **Treat as metadata:** This record is self-reported and not proof of actual custody or control.

Use this data as one input to a broader risk assessment, not as a standalone custody proof.
