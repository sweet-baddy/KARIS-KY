# Canary Deployment Strategy for Escrow Contracts

Staged rollout procedure for deploying new WASM to a subset of escrow instances before full production rollout.

---

## Overview

Canary deployments allow operators to test new contract versions on a small set of production-like escrows (`canary: true` instances) before rolling out to all instances. This reduces risk of widespread failures.

### Two-stage deployment model

```
Stage 1: Canary (5-10% of instances)
  ├─ CanaryOperator deploys to canary escrows only
  ├─ Monitor 24-72 hours
  └─ Measure: errors, performance, investor behavior

Stage 2: Production (100% of instances)
  ├─ Governance approval after canary success
  ├─ Admin deploys to all remaining instances
  └─ Full rollout with monitoring
```

---

## Part 1: Instance Classification and Setup

### 1.1 Escrow instance canary flag

Each instance has an `is_canary: bool` attribute set at init or via admin entrypoint.

#### Schema definition (contract storage)

```rust
// In escrow/src/lib.rs
#[derive(Clone)]
pub struct InvoiceEscrow {
    pub admin: Address,
    pub sme_address: Address,
    pub amount: i128,
    pub funded_amount: i128,
    pub yield_bps: u32,
    pub maturity: u64,
    pub invoice_id: String,
    pub status: u8,
    pub is_canary: bool,  // ← NEW: canary flag
    // ... other fields
}
```

#### Setting canary flag at init

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network mainnet \
  -- init \
  --admin $ADMIN \
  --invoice_id INV-CANARY-001 \
  --sme_address $SME \
  --amount 10000000000 \
  --yield_bps 800 \
  --maturity 0 \
  --funding_token $TOKEN \
  --registry null \
  --treasury $TREASURY \
  --is_canary true
```

#### Updating canary flag (admin entrypoint)

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network mainnet \
  -- set_canary_flag --is_canary true
```

---

### 1.2 Instance inventory with canary classification

Maintain inventory distinguishing canary vs. production instances:

```json
{
  "network": "mainnet",
  "deployment_metadata": {
    "wasm_hash": "abc123...",
    "deployment_date": "2024-07-27",
    "stage": "canary"
  },
  "canary_instances": [
    {
      "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4",
      "invoice_id": "INV-CANARY-001",
      "is_canary": true,
      "status": "open",
      "investor_count": 5,
      "funded_amount": "0",
      "purpose": "smoke test escrow (small investor base)"
    },
    {
      "contract_id": "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBC4",
      "invoice_id": "INV-CANARY-002",
      "is_canary": true,
      "status": "funded",
      "investor_count": 15,
      "funded_amount": "5000000000",
      "purpose": "funded escrow with active claims"
    }
  ],
  "production_instances": [
    {
      "contract_id": "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC4",
      "invoice_id": "INV-PROD-001",
      "is_canary": false,
      "status": "funded",
      "investor_count": 50
    }
    // ... more production instances
  ]
}
```

### 1.3 Canary instance selection criteria

Choose canary instances that represent realistic usage:

- **Mix of statuses:** open, funded, and settled escrows
- **Various investor counts:** 0-10, 10-50 to catch scaling issues
- **Different token types:** if using multiple tokens
- **Known investors:** can notify of canary testing
- **Non-critical:** if canary fails, no major business impact

**Example:** 2-3 escrows covering ~5-10% of total investor base

---

## Part 2: CanaryOperator Role

### 2.1 Role definition and permissions

#### CanaryOperator role

| Action | CanaryOperator | Admin | Governance |
|--------|---|---|---|
| Deploy to canary instances only | ✓ | — | — |
| Deploy to production instances | ✗ | ✓ | Approves |
| Promote canary to production | ✗ | ✓ | Approves |
| Set/clear legal hold on canary | ✓ | — | — |
| Query all instances | ✓ | ✓ | — |
| Emergency rollback (canary) | ✓ | — | — |
| Emergency rollback (production) | ✗ | ✓ | Governance consult |

#### Access control in contract

```rust
// Pseudo-code for access control
fn require_canary_operator(env: Env) {
    let caller = env.invoker();
    let canary_ops_set = env.storage().instance()
        .get::<_, Set<Address>>(&DataKey::CanaryOperators)
        .unwrap_or_default();
    
    if !canary_ops_set.contains(&caller) {
        fail(&env, EscrowError::UnauthorizedCanaryOperator);
    }
}

fn upgrade_to_wasm(env: Env, new_wasm_hash: BytesN<32>, is_canary_only: bool) {
    if is_canary_only {
        // CanaryOperator can invoke for canary instances only
        require_canary_operator(env.clone());
    } else {
        // Admin-only for production instances
        let admin = Self::load_escrow_require_admin(env.clone());
    }
    
    env.deployer().update_current_contract_wasm(new_wasm_hash);
}
```

#### Key management

```bash
# Designate CanaryOperator address (governance-controlled)
CANARY_OPERATOR_ADDRESS=G...  # Non-multisig hot wallet acceptable for canary only

# Store in contract (admin-only)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- set_canary_operators \
  --operators '[G..., G...]'
```

---

### 2.2 CanaryOperator operational constraints

**CanaryOperator MUST:**
- [ ] Only invoke upgrade on instances where `is_canary == true`
- [ ] Never invoke upgrade on production instances (`is_canary == false`)
- [ ] Document all canary deployments in deployment log
- [ ] Monitor canary instances continuously (1h, 24h, 72h checkpoints)
- [ ] Report results to governance within 72 hours
- [ ] Never promote canary to production (admin-only)
- [ ] Escalate any anomaly immediately (do not hide issues)

**CanaryOperator CANNOT:**
- ✗ Upgrade production instances
- ✗ Remove canary flag from instances
- ✗ Approve or promote to production
- ✗ Change canary instance selection

---

## Part 3: Canary Deployment Procedure

### 3.1 Pre-canary checklist

Before deploying to canary escrows:

```
Pre-canary validation:
  ✓ Build verification (all CI gates pass)
  ✓ Testnet staging (smoke tests on testnet mirror)
  ✓ Code review by security team
  ✓ Governance approval for canary (Stage 1)
  ✓ Canary instances identified and documented
  ✓ CanaryOperator on-call and ready
  ✓ Monitoring dashboard prepared
  ✓ Rollback plan documented
```

### 3.2 Canary deployment execution

#### Step 1: Upload WASM to mainnet

```bash
export STELLAR_NETWORK=mainnet
export SOROBAN_RPC_URL=https://soroban-mainnet.stellar.org
export CANARY_OPERATOR_SECRET=S...

# Build and upload
cargo build --target wasm32v1-none --release -p karis-ky_escrow

CANARY_WASM_HASH=$(stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $CANARY_OPERATOR_SECRET \
  --network $STELLAR_NETWORK \
  | grep -oP 'Upload result: \K.*')

echo "WASM hash: $CANARY_WASM_HASH"
echo "$CANARY_WASM_HASH" > canary_wasm_hash.txt
```

#### Step 2: Activate legal hold on canary instances

```bash
CANARY_INSTANCES=(
  "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"
  "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBC4"
)

for CID in "${CANARY_INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network $STELLAR_NETWORK \
    -- set_legal_hold --active true
  echo "✓ Legal hold set: $CID"
done
```

#### Step 3: Verify canary flag is set

```bash
for CID in "${CANARY_INSTANCES[@]}"; do
  ESCROW=$(stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network $STELLAR_NETWORK \
    -- get_escrow)
  
  IS_CANARY=$(echo $ESCROW | jq -r '.is_canary')
  if [ "$IS_CANARY" != "true" ]; then
    echo "✗ ERROR: $CID is not marked as canary!"
    exit 1
  fi
done
echo "✓ All canary flags verified"
```

#### Step 4: Deploy to canary instances only

```bash
CANARY_WASM_HASH=$(cat canary_wasm_hash.txt)

for CID in "${CANARY_INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network $STELLAR_NETWORK \
    -- upgrade --new_wasm_hash $CANARY_WASM_HASH
  
  echo "✓ Deployed to canary: $CID"
done
```

#### Step 5: Verify canary deployments

```bash
for CID in "${CANARY_INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network $STELLAR_NETWORK \
    -- get_version
  
  stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network $STELLAR_NETWORK \
    -- get_escrow | jq '.status'
done
```

#### Step 6: Clear legal hold on canary

```bash
for CID in "${CANARY_INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network $STELLAR_NETWORK \
    -- set_legal_hold --active false
  echo "✓ Legal hold cleared: $CID"
done
```

---

### 3.3 Canary monitoring window (72 hours)

Monitor canary instances continuously over 3 days:

#### 1-hour checks

```bash
#!/bin/bash
# canary_health_1h.sh

CANARY_INSTANCES=(
  "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"
  "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBC4"
)

echo "=== CANARY 1-HOUR CHECK ===" > canary_report_1h.txt
echo "Timestamp: $(date -u)" >> canary_report_1h.txt

for CID in "${CANARY_INSTANCES[@]}"; do
  echo "" >> canary_report_1h.txt
  echo "Instance: $CID" >> canary_report_1h.txt
  
  # Get version
  VERSION=$(stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network mainnet \
    -- get_version)
  echo "Version: $VERSION" >> canary_report_1h.txt
  
  # Get escrow state
  ESCROW=$(stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network mainnet \
    -- get_escrow)
  
  echo "Status: $(echo $ESCROW | jq '.status')" >> canary_report_1h.txt
  echo "Funded: $(echo $ESCROW | jq '.funded_amount')" >> canary_report_1h.txt
  echo "Legal hold: $(echo $ESCROW | jq '.legal_hold_active')" >> canary_report_1h.txt
done

cat canary_report_1h.txt
```

#### 24-hour and 72-hour checks

- [ ] All instances responding to RPC calls without errors
- [ ] Schema version correct across all canary instances
- [ ] State reads return consistent data
- [ ] No unusual error patterns in contract events
- [ ] Investor funding/claims working (if any transactions)
- [ ] Performance metrics normal (gas usage, latency)

---

## Part 4: Canary to Production Promotion

### 4.1 Promotion decision gate

**After 72-hour canary monitoring period, assess:**

| Metric | Threshold | Status |
|--------|-----------|--------|
| Canary error rate | < 0.1% | ✓/✗ |
| Investor complaints | 0 | ✓/✗ |
| Settlement success | 100% | ✓/✗ |
| RPC availability | > 99.5% | ✓/✗ |
| Performance (gas) | Baseline ±10% | ✓/✗ |

**Promotion criteria:** All metrics pass → Ready for production rollout

### 4.2 Governance approval for production

```
Canary success report → Governance review → Vote → Approval
```

Governance vote must include:
- [ ] Canary monitoring report (72 hours)
- [ ] Error logs and incidents (if any)
- [ ] Performance metrics
- [ ] Investor impact assessment
- [ ] Rollback plan for production

### 4.3 Production deployment (admin-only)

Once governance approves, Admin deploys to remaining instances:

```bash
export ADMIN_SECRET=S...  # Multisig key

# Upload same WASM (already on-chain, same hash)
PROD_WASM_HASH=$(cat canary_wasm_hash.txt)

# Get all non-canary production instances
PROD_INSTANCES=(
  "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC4"
  "CDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD4"
  # ... more production instances (filtered: is_canary == false)
)

# Deploy to production (same procedure as canary)
for CID in "${PROD_INSTANCES[@]}"; do
  # Activate legal hold
  # Verify is_canary == false
  # Deploy upgrade
  # Verify deployment
  # Clear legal hold
done
```

---

## Part 5: Canary Rollback

### 5.1 Emergency rollback (canary only)

If canary deployment has critical bugs:

```bash
# CanaryOperator can rollback canary instances immediately
OLD_WASM_HASH=<previous_working_hash>

for CID in "${CANARY_INSTANCES[@]}"; do
  stellar contract invoke \
    --id $CID \
    --source $CANARY_OPERATOR_SECRET \
    --network $STELLAR_NETWORK \
    -- upgrade --new_wasm_hash $OLD_WASM_HASH
  
  echo "✓ Rolled back: $CID"
done
```

### 5.2 Rollback decision criteria

Rollback immediately if:
- [ ] Canary instances panicking (XDR decode error)
- [ ] Settlement transactions failing for all instances
- [ ] Investor claims blocked unexpectedly
- [ ] Critical state corruption detected
- [ ] RPC errors > 5% sustained

Do NOT rollback if:
- [ ] Single investor claims an edge case (not reproducible)
- [ ] Performance is 10-15% slower (within margin)
- [ ] Non-critical new keys missing defaults (expected)

### 5.3 Post-rollback investigation

After rollback:
- [ ] Document root cause
- [ ] Update code review
- [ ] Re-test on testnet
- [ ] Schedule re-canary deployment
- [ ] Notify governance of delay

---

## Part 6: Canary Monitoring Dashboard

Create a dedicated dashboard for real-time canary metrics:

```json
{
  "canary_deployment": {
    "wasm_hash": "abc123...",
    "deployment_time": "2024-07-27T12:00:00Z",
    "status": "monitoring_72h",
    "instances": [
      {
        "contract_id": "CAAAA...",
        "is_canary": true,
        "deployed_at": "2024-07-27T12:15:00Z",
        "version": 6,
        "escrow_status": 1,
        "legal_hold_active": false,
        "rpc_error_rate": 0.05,
        "last_check": "2024-07-27T13:00:00Z",
        "health": "OK"
      }
    ],
    "aggregated_metrics": {
      "error_rate": 0.07,
      "error_count": 1,
      "settlement_success": 100,
      "investor_complaints": 0,
      "gas_usage_vs_baseline": "+5%"
    }
  }
}
```

---

## Part 7: Canary Deployment Checklist

Use this for every canary deployment:

```
PRE-CANARY PHASE
  ✓ CI passes (build, test, lint, coverage)
  ✓ Security code review approved
  ✓ Testnet staging successful
  ✓ Governance approves canary stage
  ✓ Canary instances documented
  ✓ CanaryOperator on-call
  ✓ Monitoring dashboard ready
  ✓ Rollback plan documented

CANARY DEPLOYMENT
  ✓ WASM uploaded to mainnet
  ✓ Legal hold activated on canary instances
  ✓ Canary flag verified (is_canary == true)
  ✓ Upgrade invocation succeeds
  ✓ Version query returns expected schema
  ✓ State reads return no errors
  ✓ Legal hold cleared

CANARY MONITORING (72 hours)
  ✓ 1-hour check: all instances healthy
  ✓ 24-hour check: no unusual errors
  ✓ 72-hour check: metrics within thresholds
  ✓ Error rate < 0.1%
  ✓ No investor complaints
  ✓ Settlement success 100%
  ✓ All promotion criteria met

POST-CANARY
  ✓ Report sent to governance
  ✓ Governance votes for production
  ✓ Admin begins production rollout
  ✓ Full documentation archived
```

