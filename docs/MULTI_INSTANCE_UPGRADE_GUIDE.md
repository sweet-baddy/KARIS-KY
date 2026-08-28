# Multi-Instance Escrow Upgrade Guide

> **Scope:** Operating karis-ky escrow contract upgrades across multiple live instances on Stellar/Soroban mainnet and testnet.

This guide covers safe upgrade procedures for additive (WASM-only) and breaking (redeploy) changes, with worked examples, rollback protocols, and monitoring checklists.

---

## Overview: Two upgrade paths

| Path | When to use | Risk | Reversibility |
|------|------------|------|----------------|
| **Additive WASM upgrade** | New keys only; no struct layout changes | Low | High — revert old WASM hash anytime |
| **Redeploy (breaking change)** | `InvoiceEscrow` struct or stored type layout changed | Medium | Medium — requires investor migration, new contract ID |

**Decision:** If any `#[contracttype]` stored struct changes XDR shape, **redeploy**. Otherwise, **additive upgrade**.

---

## Part 1: Pre-Upgrade Validation

### 1.1 Pre-flight checklist (complete all items)

#### Build and verification

- [ ] Rust 1.70+ installed
- [ ] `wasm32v1-none` target available: `rustup target add wasm32v1-none`
- [ ] Format passes: `cargo fmt --all -- --check`
- [ ] Linter passes: `cargo clippy -p karis-ky_escrow -- -D warnings`
- [ ] Full test suite passes: `cargo test -p karis-ky_escrow`
- [ ] Coverage meets gate (≥95%): `cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow`
- [ ] WASM artifact exists: `ls -lh target/wasm32v1-none/release/karis-ky_escrow.wasm`

#### Contract security review

- [ ] `admin` is a multisig or governance contract (not an EOA)
- [ ] `funding_token` is SEP-41 compliant (no fee-on-transfer, no rebasing)
- [ ] `treasury` address is controlled by karis-ky governance
- [ ] `invoice_id` matches off-chain invoice slug validation
- [ ] `maturity` uses ledger timestamp seconds (not wall-clock oracle)
- [ ] All live escrow instances documented with contract IDs and invoice IDs
- [ ] No active legal holds on instances to be upgraded (or plan clear timing)

### 1.2 Inventory of live instances

Create a **spreadsheet or JSON** with all instances to upgrade:

```json
{
  "instances": [
    {
      "environment": "mainnet",
      "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
      "invoice_id": "INV-2024-001",
      "status": "funded",
      "admin": "GADMIN...",
      "schema_version": 5,
      "notes": "Active investor payout in progress"
    },
    {
      "environment": "testnet",
      "contract_id": "CBBBB...",
      "invoice_id": "INV-TEST-001",
      "status": "open",
      "admin": "GADMIN...",
      "schema_version": 5,
      "notes": "Smoke test instance"
    }
  ]
}
```

**Verify before proceeding:**
```bash
# For each instance, call get_version to confirm stored schema version matches inventory
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- get_version
```

### 1.3 Determine upgrade path

**Ask these questions in order:**

1. **Did any `#[contracttype]` struct layout change?**
   - Check: `InvoiceEscrow`, `SmeCollateralCommitment`, `FundingCloseSnapshot`, `YieldTier`, `EscrowTemplate`, or any other stored type.
   - Tool: `git diff HEAD~1 escrow/src/lib.rs | grep -A10 '#\[contracttype\]'`
   - **If YES** → **REDEPLOY path** (skip to Part 3)
   - **If NO** → Continue

2. **Did any existing `DataKey` variant change or rename?**
   - **If YES** → **REDEPLOY path**
   - **If NO** → Continue

3. **Are all changes purely additive?** (new keys, new functions, bug fixes in logic)
   - **If YES** → **ADDITIVE WASM UPGRADE** (use Part 2)
   - **If NO** → Review with governance before proceeding

---

## Part 2: Additive WASM Upgrade (zero-downtime)

Use this path when only new `DataKey` variants are added and no stored struct layouts changed.

### 2.1 Testnet staging (Day 0)

#### Stage the upgrade on testnet mirror

```bash
export STELLAR_NETWORK=testnet
export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
export SOURCE_SECRET=S...          # Testnet deployer secret
export LIQUIFACT_ADMIN_ADDRESS=G...

# Step 1: Build release WASM
cargo build --target wasm32v1-none --release -p karis-ky_escrow

# Step 2: Upload to testnet
TESTNET_WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  | grep -oP 'Upload result: \K.*')

echo "Testnet WASM hash: $TESTNET_WASM_HASH"
```

#### Smoke test on testnet instance

```bash
# Verify new WASM does not break existing escrow reads
TESTNET_CONTRACT_ID=C...  # from your testnet instance inventory

# Call get_version — should return current schema version
stellar contract invoke \
  --id $TESTNET_CONTRACT_ID \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- get_version

# Call get_escrow — should read full state without errors
stellar contract invoke \
  --id $TESTNET_CONTRACT_ID \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- get_escrow
```

### 2.2 Mainnet deployment (Day 1+)

#### Step 1: Upload WASM to mainnet

```bash
export STELLAR_NETWORK=mainnet
export SOROBAN_RPC_URL=https://soroban-mainnet.stellar.org
export SOURCE_SECRET=S...          # Mainnet deployer secret
export LIQUIFACT_ADMIN_ADDRESS=G...

MAINNET_WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  | grep -oP 'Upload result: \K.*')

echo "Mainnet WASM hash: $MAINNET_WASM_HASH"

# Save this hash for the upgrade invocation
echo "$MAINNET_WASM_HASH" > wasm_hash.txt
```

#### Step 2: Activate legal hold (optional but recommended)

For each **funded or in-flight** escrow instance, activate legal hold to block concurrent operations during upgrade:

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- set_legal_hold --active true
  echo "Legal hold set on $CONTRACT_ID"
done
```

#### Step 3: Deploy new WASM (invoke upgrade entrypoint)

**Note:** The current karis-ky escrow contract does **not** expose an admin-gated upgrade entrypoint. See Part 3 (Redeploy) for the alternative approach, or implement an `upgrade()` entrypoint first.

If an `upgrade()` entrypoint exists:

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- upgrade --new_wasm_hash $MAINNET_WASM_HASH

  echo "Upgraded $CONTRACT_ID to $MAINNET_WASM_HASH"
done
```

#### Step 4: Verify upgrades

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  echo "Checking $CONTRACT_ID..."
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- get_version
done
```

All should return the same schema version (no changes in additive upgrade).

#### Step 5: Clear legal hold

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- set_legal_hold --active false
  echo "Legal hold cleared on $CONTRACT_ID"
done
```

#### Post-upgrade monitoring (24–72 hours)

- [ ] Monitor all instances for errors in logs
- [ ] Verify no settlement/payout transactions are blocked
- [ ] Confirm investor claims succeed if any post-settlement
- [ ] Run snapshot audit: query all instances and compare state hashes

---

## Part 3: Redeploy (breaking changes)

Use this path when `InvoiceEscrow` struct or any stored `#[contracttype]` layout changes.

### 3.1 Pre-redeploy planning

#### Identify breaking change

```bash
# Diff the contract type changes
git diff HEAD~1 escrow/src/lib.rs | grep -A20 '#\[contracttype\]'

# Example breaking change: adding a required field to InvoiceEscrow
```

#### Prepare migration playbook

For each instance, document:

1. **Current state snapshot** (call `get_escrow`)
2. **All investor contributions** (enumerate via indexer or query)
3. **New contract ID** (post-deployment)
4. **Off-chain state sync** (how to notify indexer, UI, API)

```bash
# Step 1: Snapshot all instances
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- get_escrow > snapshot_${CONTRACT_ID}.json
done
```

### 3.2 Testnet redeploy (Day 0)

```bash
export STELLAR_NETWORK=testnet
export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
export SOURCE_SECRET=S...          # Testnet deployer secret

# Step 1: Build new WASM
cargo build --target wasm32v1-none --release -p karis-ky_escrow

# Step 2: Upload to testnet
TESTNET_WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  | grep -oP 'Upload result: \K.*')

# Step 3: Deploy new instance
NEW_TESTNET_CONTRACT=$(stellar contract deploy \
  --wasm-hash $TESTNET_WASM_HASH \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK)

echo "New testnet instance: $NEW_TESTNET_CONTRACT"

# Step 4: Init with same parameters as old instance
stellar contract invoke \
  --id $NEW_TESTNET_CONTRACT \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- init \
  --admin $LIQUIFACT_ADMIN_ADDRESS \
  --invoice_id INV-TEST-001 \
  --sme_address G... \
  --amount 10000000000 \
  --yield_bps 800 \
  --maturity 0 \
  --funding_token C... \
  --registry null \
  --treasury G... \
  --yield_tiers null \
  --min_contribution null \
  --max_unique_investors null

# Step 5: Verify new instance works
stellar contract invoke \
  --id $NEW_TESTNET_CONTRACT \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- get_version
```

### 3.3 Mainnet redeploy (Day 1+)

#### Step 1: Upload new WASM

```bash
export STELLAR_NETWORK=mainnet
export SOROBAN_RPC_URL=https://soroban-mainnet.stellar.org
export SOURCE_SECRET=S...          # Mainnet deployer secret

MAINNET_WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  | grep -oP 'Upload result: \K.*')

echo "Mainnet WASM hash: $MAINNET_WASM_HASH"
```

#### Step 2: For each instance, deploy → init → migrate

```bash
# For each old instance:
OLD_CONTRACT_ID=C...
INVOICE_ID=INV-2024-001
# ... retrieve init params from snapshot ...

# Deploy new instance
NEW_CONTRACT_ID=$(stellar contract deploy \
  --wasm-hash $MAINNET_WASM_HASH \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK)

echo "New contract: $NEW_CONTRACT_ID (replacing $OLD_CONTRACT_ID)"

# Init new instance with same parameters
stellar contract invoke \
  --id $NEW_CONTRACT_ID \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- init \
  --admin $ADMIN \
  --invoice_id $INVOICE_ID \
  --sme_address $SME \
  --amount $AMOUNT \
  --yield_bps $YIELD_BPS \
  --maturity $MATURITY \
  --funding_token $FUNDING_TOKEN \
  --registry $REGISTRY \
  --treasury $TREASURY \
  --yield_tiers null \
  --min_contribution null \
  --max_unique_investors null

# If escrow was already funded, restore investor contributions
# (off-chain process — depends on indexer or manual entry)
```

#### Step 3: Notify integrators

Update all external systems with new contract IDs:

```json
{
  "migration_date": "2024-07-28T12:00:00Z",
  "migrations": [
    {
      "invoice_id": "INV-2024-001",
      "old_contract": "CAAAA...",
      "new_contract": "CBBBB...",
      "reason": "Storage layout upgrade to schema v6"
    }
  ]
}
```

Send to:
- Indexer/blockchain listener (update all pointers)
- API consumers (update contract ID in discovery)
- UI (clear old contract caches)
- Investors (optional: notify of new address for claims)

---

## Part 4: Rollback Procedure

### 4.1 Additive WASM upgrade rollback

If the new WASM has a bug after deployment, **revert to the old WASM hash:**

```bash
export STELLAR_NETWORK=mainnet
export SOROBAN_RPC_URL=https://soroban-mainnet.stellar.org
export SOURCE_SECRET=S...

# Retrieve old WASM hash from git tag or deployment record
OLD_WASM_HASH=<previous_release_wasm_hash>

# For each instance, invoke upgrade with old hash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- upgrade --new_wasm_hash $OLD_WASM_HASH

  echo "Rolled back $CONTRACT_ID to $OLD_WASM_HASH"
done

# Verify
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- get_escrow
done
```

**Worked example:**

```
Day 1, 14:00 UTC: Deploy WASM hash ABC123 to instances [C001, C002]
Day 1, 15:30 UTC: Bug detected in C001 settlement logic
Day 1, 16:00 UTC: Revert C001 and C002 to WASM hash 789DEF (previous release)
Day 1, 16:15 UTC: Verify get_escrow returns data correctly on both instances
Day 2, 09:00 UTC: Fix deployed, redeploy ABC456 after code review
```

### 4.2 Redeploy rollback

If the new instance has a fatal flaw:

**Option A (investor migration already in progress):**
- Pause investor onboarding to new contract
- Complete claims/settlements on new contract
- Route future funding to old contract if still operational

**Option B (emergency):**
- Activate legal hold on new instance to block operations
- Notify investors of delay
- Coordinate off-chain resolution

```bash
# Emergency hold: freeze all operations on new instance
stellar contract invoke \
  --id $NEW_CONTRACT_ID \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- set_legal_hold --active true

echo "New contract $NEW_CONTRACT_ID frozen. Investigating..."
```

---

## Part 5: Monitoring During and After Upgrade

### 5.1 Real-time monitoring checklist

**Before upgrade window:**
- [ ] Disable automatic investor funding (pause integrator)
- [ ] Enable enhanced logging on contract invocations
- [ ] Brief support team on rollback procedures
- [ ] Confirm admin multisig is online and responsive

**During upgrade (per instance):**
- [ ] Check `get_version` returns expected value
- [ ] Check `get_escrow` reads without errors
- [ ] Spot-check 2–3 random investor contribution queries (via indexer)
- [ ] Monitor RPC error rates (should be 0)

**Post-upgrade window (24–72 hours):**
- [ ] Run hourly state snapshots and diff for anomalies
- [ ] Monitor settlement and withdrawal transactions
- [ ] Confirm investor claim payouts succeed
- [ ] Audit all attestation digests for data integrity

### 5.2 Query templates for monitoring

#### Per-instance health check script

```bash
#!/bin/bash
# health_check.sh — call after every instance upgrade

CONTRACT_ID=$1
STELLAR_NETWORK=$2
SOURCE_SECRET=$3

echo "=== Health Check: $CONTRACT_ID ==="

# Check 1: Version matches expectation
VERSION=$(stellar contract invoke \
  --id $CONTRACT_ID \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- get_version)
echo "Schema version: $VERSION"
[ "$VERSION" -eq 6 ] && echo "✓ Version OK" || echo "✗ Version mismatch!"

# Check 2: Escrow state is readable
ESCROW=$(stellar contract invoke \
  --id $CONTRACT_ID \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
ation overhead. Save old contract IDs in archive for compliance.

**Q: What if get_version returns a different schema on two instances?**

A: This indicates instances were at different versions pre-upgrade. Before proceeding:
1. Document current version on each
2. Determine if both need upgrade or only one
3. Run separate upgrade workflows per version batch

**Q: Can I call migrate() after an additive WASM upgrade?**

A: **No.** In the current release (v6), `migrate()` fails with typed errors on all paths. Do not call it for additive upgrades. It is only valid when you implement a real storage-rewriting migration path.

**Q: How do I know if a change is "additive" vs. "breaking"?**

A: Use this rule:
- **Additive:** Only new `DataKey` variants, new functions, logic changes. Old instances can read with `.get(...).unwrap_or(default)`.
- **Breaking:** Any change to `InvoiceEscrow`, `SmeCollateralCommitment`, or other stored `#[contracttype]` struct. XDR shape changes require redeploy.

Code review: `git diff HEAD~1 escrow/src/lib.rs | grep -A15 '#\[contracttype\]'`

**Q: What if the upgrade fails mid-invocation?**

A: Soroban aborts on contract error (panic or typed error). The state is **not** modified. Simply retry the invocation with the same parameters. No partial state is stored.

**Q: Can old WASM read new schema keys?**

A: No. If you deploy an old WASM against new schema keys (redeploy with new init), the old WASM will not know about them and will default them to `None` / `0` when read. This is why redeploy is necessary for breaking changes — the old WASM cannot decode new stored types.

**Q: Should I test the upgrade procedure on mainnet first?**

A: **No.** Always use testnet as the final staging environment before mainnet. Testnet mirrors the same Soroban runtime and RPC behavior, minimizing surprises.

---

## Part 8: Worked example — additive upgrade

**Scenario:** Shipping a new yield-tiering feature (new `DataKey` variants, no struct changes).

### Timeline

```
Day 0, 09:00 UTC
  ├─ Build new WASM with new yield keys
  ├─ Upload to testnet, run smoke tests
  └─ ✓ get_version returns 6, get_escrow reads OK

Day 0, 14:00 UTC
  ├─ Code review approved
  ├─ All pre-flight checks pass
  └─ ✓ Instance inventory complete: [C-INV-001, C-INV-002, C-INV-003]

Day 1, 08:00 UTC (upgrade window)
  ├─ Upload WASM to mainnet → hash ABC123
  ├─ Activate legal hold on all 3 instances
  ├─ Invoke upgrade entrypoint for each instance
  ├─ Verify get_version and get_escrow on each
  ├─ Clear legal hold on all 3
  └─ ✓ All instances now running new WASM

Day 1, 20:00 UTC (post-upgrade monitoring)
  ├─ Run health_check.sh against all instances
  ├─ Query investor claims (if any settled) — all succeed
  └─ ✓ No errors in logs

Day 3, 09:00 UTC
  └─ ✓ Final audit: 72-hour post-upgrade window clear
```

### Commands executed

```bash
#!/bin/bash
# upgrade_additive.sh — additive WASM upgrade script

set -e

STELLAR_NETWORK=mainnet
SOROBAN_RPC_URL=https://soroban-mainnet.stellar.org
SOURCE_SECRET=S...
INSTANCES=(
  "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"
  "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBC4"
  "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC4"
)

echo "Building new WASM..."
cargo build --target wasm32v1-none --release -p karis-ky_escrow

echo "Uploading to mainnet..."
WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  | grep -oP 'Upload result: \K.*')

echo "WASM hash: $WASM_HASH"

echo "Setting legal hold on all instances..."
for CID in "${INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- set_legal_hold --active true
  echo "  ✓ Legal hold set: $CID"
done

echo ""
echo "Upgrading instances..."
for CID in "${INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- upgrade --new_wasm_hash $WASM_HASH
  echo "  ✓ Upgraded: $CID"
done

echo ""
echo "Verifying upgrades..."
for CID in "${INSTANCES[@]}"; do
  VERSION=$(stellar contract invoke \
    --id $CID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- get_version)
  echo "  ✓ $CID version: $VERSION"
done

echo ""
echo "Clearing legal hold..."
for CID in "${INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- set_legal_hold --active false
  echo "  ✓ Legal hold cleared: $CID"
done

echo ""
echo "✓ Upgrade complete. Begin monitoring."
```

---

## Part 9: Worked example — redeploy (breaking change)

**Scenario:** Moving per-investor keys to persistent storage (requires new deployment, like v5→v6 transition).

### Pre-migration snapshot

```bash
#!/bin/bash
# snapshot_pre_redeploy.sh

OLD_INSTANCES=(
  "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"
  "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBC4"
)

for CID in "${OLD_INSTANCES[@]}"; do
  echo "Snapshotting $CID..."
  stellar contract invoke \
    --id $CID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- get_escrow > escrow_snapshot_${CID}.json
    
  echo "  funded_amount: $(jq '.funded_amount' escrow_snapshot_${CID}.json)"
  echo "  status: $(jq '.status' escrow_snapshot_${CID}.json)"
done
```

### Redeploy sequence

```bash
#!/bin/bash
# redeploy_sequence.sh

set -e

STELLAR_NETWORK=mainnet
SOROBAN_RPC_URL=https://soroban-mainnet.stellar.org
SOURCE_SECRET=S...

# Map of old → new instances to deploy
declare -A MIGRATION_MAP=(
  ["CAAA..."]="INV-2024-001"
  ["CBBB..."]="INV-2024-002"
)

echo "Building new WASM..."
cargo build --target wasm32v1-none --release -p karis-ky_escrow

echo "Uploading to mainnet..."
WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  | grep -oP 'Upload result: \K.*')

echo "WASM hash: $WASM_HASH"
echo ""

# Deploy new instances
for OLD_CID in "${!MIGRATION_MAP[@]}"; do
  INVOICE_ID="${MIGRATION_MAP[$OLD_CID]}"
  
  echo "Creating new instance for $INVOICE_ID..."
  
  # Load snapshot
  SNAPSHOT=$(cat escrow_snapshot_${OLD_CID}.json)
  
  # Extract init parameters
  ADMIN=$(echo $SNAPSHOT | jq -r '.admin')
  SME=$(echo $SNAPSHOT | jq -r '.sme_address')
  AMOUNT=$(echo $SNAPSHOT | jq -r '.amount')
  YIELD=$(echo $SNAPSHOT | jq -r '.yield_bps')
  MATURITY=$(echo $SNAPSHOT | jq -r '.maturity')
  TOKEN=$(echo $SNAPSHOT | jq -r '.funding_token')
  REGISTRY=$(echo $SNAPSHOT | jq -r '.registry')
  TREASURY=$(echo $SNAPSHOT | jq -r '.treasury')
  
  # Deploy new instance
  NEW_CID=$(stellar contract deploy \
    --wasm-hash $WASM_HASH \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK)
  
  echo "  New contract ID: $NEW_CID"
  
  # Init
  stellar contract invoke \
    --id $NEW_CID \
    --source $SOURCE_SECRET \
    --network $STELLAR_NETWORK \
    -- init \
    --admin $ADMIN \
    --invoice_id $INVOICE_ID \
    --sme_address $SME \
    --amount $AMOUNT \
    --yield_bps $YIELD \
    --maturity $MATURITY \
    --funding_token $TOKEN \
    --registry $REGISTRY \
    --treasury $TREASURY \
    --yield_tiers null \
    --min_contribution null \
    --max_unique_investors null
  
  echo "  ✓ Initialized: $NEW_CID"
  echo "  OLD → NEW: $OLD_CID → $NEW_CID" >> migration_log.txt
  echo ""
done

echo "✓ Redeploy complete. Begin investor migration and integration updates."
```

### Post-redeploy integration updates

```bash
#!/bin/bash
# update_integrations.sh

# 1. Notify indexer of new contract IDs
cat > migration_manifest.json << 'EOF'
{
  "redeploy_date": "2024-07-28T08:00:00Z",
  "reason": "Per-investor key persistent storage migration (v5→v6)",
  "migrations": [
    {
      "invoice_id": "INV-2024-001",
      "old_contract": "CAAA...",
      "new_contract": "CXXX...",
      "reason": "Storage layout change"
    },
    {
      "invoice_id": "INV-2024-002",
      "old_contract": "CBBB...",
      "new_contract": "CYYY...",
      "reason": "Storage layout change"
    }
  ]
}
EOF

# 2. Update API service
curl -X POST https://api.karis-ky.dev/admin/migration \
  -H "Authorization: Bearer $API_TOKEN" \
  -H "Content-Type: application/json" \
  -d @migration_manifest.json

# 3. Update UI caches
# (Implementation: depends on your UI architecture)

# 4. Notify support / legal (off-chain)
mail -s "Escrow instances redeployed (v5→v6)" ops@karis-ky.dev < migration_log.txt

echo "✓ Integrations updated."
```

---

## Part 10: Emergency procedures

### Legal hold activation (block all operations)

```bash
#!/bin/bash
# emergency_hold.sh — block escrow from operating

CONTRACT_ID=$1

stellar contract invoke \
  --id $CONTRACT_ID \
  --source $SOURCE_SECRET \
  --network mainnet \
  -- set_legal_hold --active true

echo "✗ Legal hold ACTIVE on $CONTRACT_ID"
echo "  All settlement, withdrawal, and claim operations blocked."
echo "  Admin only: clear via set_legal_hold --active false after investigation."
```

### Immediate rollback (additive WASM only)

```bash
#!/bin/bash
# emergency_rollback.sh

# Use only if new WASM has critical bug
# Requires old WASM hash to be known and uploaded

OLD_WASM_HASH=$1
INSTANCES=$2

for CID in $INSTANCES; do
  stellar contract invoke \
    --id $CID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- upgrade --new_wasm_hash $OLD_WASM_HASH
  echo "✓ Rolled back $CID to $OLD_WASM_HASH"
done
```

### Investigation checklist

If an instance behaves abnormally post-upgrade:

1. [ ] Activate legal hold immediately
2. [ ] Run health check script to dump full state
3. [ ] Compare `get_escrow` output to pre-upgrade snapshot
4. [ ] Check RPC logs for network errors
5. [ ] If state mismatch: do **not** clear legal hold; escalate to security team
6. [ ] If RPC error: wait 5 min, retry once
7. [ ] If clear root cause: proceed with rollback or investigation fix

---

## Appendix A: Configuration template for multi-instance upgrades

```json
{
  "upgrade": {
    "type": "additive",
    "version_from": 5,
    "version_to": 6,
    "wasm_hash_testnet": "abc123...",
    "wasm_hash_mainnet": "def456...",
    "testnet_instances": [
      {
        "contract_id": "C...",
        "invoice_id": "TEST-001",
        "status": "smoke_test"
      }
    ],
    "mainnet_instances": [
      {
        "contract_id": "C...",
        "invoice_id": "INV-2024-001",
        "status": "funded",
        "investors": 42,
        "funded_amount": "5000000000"
      }
    ],
    "rollback_plan": "Revert WASM hash to abc123... if critical bug detected",
    "monitoring_window_hours": 72,
    "approval_required": true,
    "approved_by": "governance-multisig"
  }
}
```

---

## Appendix B: Monitoring dashboard queries

### Per-instance query: investor contributions

```bash
# Query via indexer or enumerate off-chain
# Example: fetch all contributions for a contract

curl -X GET "https://indexer.karis-ky.dev/escrow/CAAAA.../contributions" \
  -H "Accept: application/json"

# Expected response:
# {
#   "contract_id": "CAAAA...",
#   "contributions": [
#     { "investor": "G...", "amount": "100000000", "tier": 2 },
#     { "investor": "G...", "amount": "200000000", "tier": 3 }
#   ]
# }
```

### Per-instance query: settlement log

```bash
# Check if any settlements/claims are in flight

curl -X GET "https://indexer.karis-ky.dev/escrow/CAAAA.../settlements?since=<timestamp>" \
  -H "Accept: application/json"
```

### Batch health report

```bash
#!/bin/bash
# Generate health report for all instances

echo "=== Multi-Instance Upgrade Health Report ===" > health_report.txt
echo "Generated: $(date -u)" >> health_report.txt
echo "" >> health_report.txt

for CID in "${INSTANCES[@]}"; do
  echo "Instance: $CID" >> health_report.txt
  stellar contract invoke \
    --id $CID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- get_version >> health_report.txt 2>&1
  echo "" >> health_report.txt
done

echo "Report saved to health_report.txt"
```

---

## Appendix C: Version compatibility matrix (detailed)

| Old WASM | Old schema | New WASM | New schema | Action | Outcome |
|----------|-----------|----------|-----------|--------|---------|
| v5 | 5 | v6 | 6 | WASM upgrade (if additive) | v6 reads v5 data as-is; new keys → defaults |
| v5 | 5 | v6 | 6 | Redeploy (if breaking) | New instance; investor data re-recorded |
| v4 | 4 | v6 | 6 | Must redeploy | No migration path for 4→6; redeploy |
| v6 | 6 | v5 | 5 | **Not supported** | Never downgrade to older WASM |
| v6 | 6 | v6 | 6 | Revert to same hash | Legal hold block still present; manual clear |

---

## Appendix D: Access control and approval workflow

### Pre-upgrade checklist (governance)

1. **Code review:** karis-ky security team signs off on WASM diff
2. **Impact analysis:** Legal, ops, and tech confirm no breaking changes to investor rights
3. **Coverage audit:** `cargo llvm-cov` >= 95% on new code paths
4. **Multisig approval:** Governance multisig votes to proceed
5. **Testnet sign-off:** Ops team confirms testnet upgrade succeeds

### Role responsibilities

| Role | Responsibility |
|------|-----------------|
| **Governance** | Approve upgrade type (additive vs. redeploy); authorize admin key usage |
| **Security** | Code review WASM diff; audit typed error handling |
| **Ops** | Execute upgrade steps; monitor 72 hours; rollback if needed |
| **Indexer team** | Update contract ID pointers; backfill investor data if redeploy |
| **Legal** | Confirm compliance with investor agreements (esp. redeploy) |

---

## Summary

This guide provides operators with:

✓ **Two clear upgrade paths** (additive vs. redeploy) with go/no-go criteria
✓ **Testnet staging** before mainnet to reduce surprises
✓ **Legal hold coordination** to block concurrent operations during upgrade
✓ **Rollback procedures** with worked examples for both paths
✓ **Monitoring templates** to verify success post-upgrade
✓ **Checklists** for each upgrade type to ensure no steps missed
✓ **Emergency procedures** for rapid response if issues arise

For questions or escalations, contact the karis-ky ops team or governance multisig.
  -- get_escrow)
echo "Escrow status: $(echo $ESCROW | jq -r '.status')"
echo "Funded amount: $(echo $ESCROW | jq -r '.funded_amount')"

# Check 3: Legal hold status
HOLD=$(echo $ESCROW | jq -r '.legal_hold_active')
echo "Legal hold: $HOLD"

echo ""
```

#### Batch health check

```bash
#!/bin/bash
# batch_health_check.sh — run all instances

for instance in instances.json; do
  CONTRACT_ID=$(jq -r '.instances[].contract_id' $instance)
  ENV=$(jq -r '.instances[].environment' $instance)
  
  if [ "$ENV" = "mainnet" ]; then
    NETWORK="mainnet"
    RPC="https://soroban-mainnet.stellar.org"
  else
    NETWORK="testnet"
    RPC="https://soroban-testnet.stellar.org"
  fi
  
  bash health_check.sh $CONTRACT_ID $NETWORK $SOURCE_SECRET
done
```

---

## Part 6: Checklist by upgrade type

### Additive WASM upgrade checklist

- [ ] Pre-flight build/test/lint all pass
- [ ] Instance inventory complete and verified (get_version matches)
- [ ] No breaking struct changes (code review)
- [ ] Testnet deployment successful
- [ ] Legal hold activated (if funded instances)
- [ ] WASM uploaded to mainnet
- [ ] Upgrade invocation succeeds for all instances
- [ ] get_version returns expected schema
- [ ] get_escrow reads without errors (spot check)
- [ ] Legal hold cleared
- [ ] Investor funding re-enabled
- [ ] 72-hour post-upgrade monitoring: no errors
- [ ] Health check script passes for all instances

### Redeploy checklist

- [ ] Pre-flight build/test/lint all pass
- [ ] Breaking change confirmed (code review)
- [ ] Instance inventory with init parameters captured
- [ ] Investor snapshots taken (current contributions)
- [ ] Testnet redeploy + init successful
- [ ] New testnet instance smoke tests pass
- [ ] All integrators notified of migration
- [ ] Old instances set to legal hold (optional)
- [ ] New instances deployed on mainnet
- [ ] New instances initialized with same parameters
- [ ] Investor contributions re-recorded in new instances (if already funded)
- [ ] All external systems updated with new contract IDs
- [ ] Investors notified (if applicable)
- [ ] 72-hour post-upgrade monitoring: state hashes match snapshots

---

## Part 7: FAQ and troubleshooting

**Q: Can I upgrade multiple instances in parallel?**

A: Yes, but sequentially per instance to simplify rollback. Parallel uploads are fine; sequential invocations reduce coordin
