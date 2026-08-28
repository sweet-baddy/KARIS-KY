# Feature Flags: Operator's Handbook

Practical procedures for toggling features without redeployment.

---

## Quick Start: Toggle a Feature

### Enable a feature on one instance

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- set_feature_flag \
  --flag_name "tiered_yield_enabled" \
  --enabled true \
  --reason "Enabling tiered yield for this escrow"
```

### Enable across multiple instances (batch)

```bash
#!/bin/bash
# enable_feature_batch.sh

FEATURE="tiered_yield_enabled"
INSTANCES="CAAA... CBBB... CCCC..."

for INSTANCE in $INSTANCES; do
  stellar contract invoke \
    --id $INSTANCE \
    --source $ADMIN_SECRET \
    --network mainnet \
    -- set_feature_flag \
    --flag_name $FEATURE \
    --enabled true \
    --reason "Batch enable: $FEATURE for governance approval"
  
  echo "✓ Enabled $FEATURE on $INSTANCE"
done
```

### Check current state

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- get_feature_flags | jq '.'
```

### View change history

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- get_feature_flag_history \
  --limit 20 | jq '.changes | sort_by(.timestamp) | reverse[]'
```

---

## Common Operations

### Disable a feature (emergency response)

**When:** Bug discovered, need immediate action

**How:**

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- set_feature_flag \
  --flag_name "problematic_feature" \
  --enabled false \
  --reason "EMERGENCY: Bug found; disabling until fix verified"

# Verify
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- get_feature_flags | jq '.problematic_feature'
```

**Time:** < 1 minute (no redeploy needed)

---

### Re-enable a feature

**When:** After fix is verified and deployed

**How:**

```bash
# On testnet first:
stellar contract invoke \
  --id <TESTNET_ID> \
  --source $TESTNET_ADMIN \
  --network testnet \
  -- set_feature_flag \
  --flag_name "fixed_feature" \
  --enabled true \
  --reason "Testing: feature re-enable after fix"

# Smoke test on testnet
# Then on mainnet:
stellar contract invoke \
  --id <MAINNET_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- set_feature_flag \
  --flag_name "fixed_feature" \
  --enabled true \
  --reason "Fix verified; re-enabling feature"
```

---

### Gradual rollout (canary pattern)

**Goal:** Enable feature for subset first, then all

**Steps:**

```bash
# Step 1: Enable on canary instances
echo "Enabling on canary instances..."
for CANARY in CANARY_1 CANARY_2; do
  stellar contract invoke \
    --id $CANARY \
    --source $ADMIN_SECRET \
    --network mainnet \
    -- set_feature_flag \
    --flag_name "new_feature" \
    --enabled true \
    --reason "Canary rollout: phase 1"
done

# Step 2: Monitor (1h, 24h, 72h)
sleep 1800  # Wait 30 min
stellar contract invoke --id CANARY_1 -- get_feature_flag_history | head -5

# Step 3: Enable on production (after monitoring)
echo "Enabling on production instances..."
for PROD in PROD_1 PROD_2 PROD_3; do
  stellar contract invoke \
    --id $PROD \
    --source $ADMIN_SECRET \
    --network mainnet \
    -- set_feature_flag \
    --flag_name "new_feature" \
    --enabled true \
    --reason "Production rollout: canary monitoring complete"
done

echo "✓ Gradual rollout complete"
```

---

### Get feature status across all instances

```bash
#!/bin/bash
# feature_status_report.sh

echo "Feature Status Report"
echo "===================="
echo ""

INSTANCES=$(cat all_instances.txt)

for INSTANCE in $INSTANCES; do
  echo "Instance: $INSTANCE"
  stellar contract invoke \
    --id $INSTANCE \
    --source $ADMIN_SECRET \
    --network mainnet \
    -- get_feature_flags | jq '.'
  echo ""
done
```

---

## Operational Scenarios

### Scenario 1: Investor reports error with tiered yields

**Diagnosis:**

```bash
# Check: Is tiered_yield feature enabled?
stellar contract invoke \
  --id <INVESTOR_ESCROW> \
  --source $ADMIN_SECRET \
  -- get_feature_flags | grep tiered_yield
# If false → Feature disabled, offer workaround
# If true → Feature enabled, investigate logic

# Get history
stellar contract invoke \
  --id <INVESTOR_ESCROW> \
  --source $ADMIN_SECRET \
  -- get_feature_flag_history | grep tiered_yield
```

**Response options:**

```bash
# Option A: Feature temporarily disabled for investigation
stellar contract invoke \
  --id <INVESTOR_ESCROW> \
  --source $ADMIN_SECRET \
  -- set_feature_flag \
  --flag_name "tiered_yield_enabled" \
  --enabled false \
  --reason "Investor issue INV-1234: investigating tier calculation; temporarily disabled"

# Option B: Per-instance override
stellar contract invoke \
  --id <INVESTOR_ESCROW> \
  --source $ADMIN_SECRET \
  -- set_instance_feature_override \
  --flag_name "tiered_yield_enabled" \
  --enabled false \
  --reason "Investor INV-1234: using legacy tier logic"

# Option C: Re-enable with fix
stellar contract invoke \
  --id <INVESTOR_ESCROW> \
  --source $ADMIN_SECRET \
  -- set_feature_flag \
  --flag_name "tiered_yield_enabled" \
  --enabled true \
  --reason "Fix deployed; re-enabling tiered yield"
```

---

### Scenario 2: Deprecate old attestation API

**Timeline:**

```
T+0: Announce deprecation
  └─ Notify all users (email, API docs)
  └─ Document migration path

T+30 days: Set deprecation flag in contract
  └─ Begin warning all attestation calls

T+60 days: Disable feature (auto-scheduled)
  └─ Contract blocks new attestation operations
  └─ Users have migrated to new API

T+2 versions: Remove code
  └─ Next major release removes old code entirely
```

**Implementation:**

```bash
# Day 1: Announce (off-chain)
# Day 30: Enable deprecation warning
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  -- deprecate_feature \
  --flag_name "old_attestation_v4" \
  --disable_at 1730000000 \
  --removal_version 8 \
  --reason "Replaced by Attestation API v5; migration docs at [link]"

# Day 60: Auto-disable (happens at scheduled timestamp)
# Users see: Feature 'old_attestation_v4' disabled as of 2024-10-27

# Next release: Remove code, no flag check needed
```

---

### Scenario 3: A/B test new yield calculation

**Setup:**

```bash
# Canary set A: Old calculation (baseline)
for CANARY in CANARY_OLD_{1..3}; do
  stellar contract invoke \
    --id $CANARY \
    --source $ADMIN_SECRET \
    -- set_feature_flag \
    --flag_name "new_yield_calculation_v7" \
    --enabled false \
    --reason "A/B test Group A: old yield calculation"
done

# Canary set B: New calculation (variant)
for CANARY in CANARY_NEW_{1..3}; do
  stellar contract invoke \
    --id $CANARY \
    --source $ADMIN_SECRET \
    -- set_feature_flag \
    --flag_name "new_yield_calculation_v7" \
    --enabled true \
    --reason "A/B test Group B: new yield calculation"
done
```

**Monitor over 7 days:**

```bash
for CANARY in CANARY_OLD_1 CANARY_NEW_1; do
  echo "=== $CANARY ==="
  stellar contract invoke \
    --id $CANARY \
    --source $ADMIN_SECRET \
    -- get_escrow | jq '{funded: .funded_amount, status: .status}'
done
```

**Results analysis:**

| Metric | Old (Group A) | New (Group B) | Winner |
|--------|---------------|---------------|--------|
| Avg yield % | 8.0% | 8.5% | New (higher) |
| Settlement success | 99.9% | 99.8% | Old (more stable) |
| Investor complaints | 0 | 1 | Old |
| Gas usage | 125k | 128k | Old (cheaper) |

**Decision:** Keep old calculation (more stable + lower gas)

---

## Checklist: Feature Toggle Operations

### Before toggling

- [ ] Understand what the feature does
- [ ] Know which entrypoints are affected
- [ ] Verify feature is safe to toggle (no mid-transaction risk)
- [ ] Prepare rollback plan (how to revert if needed)
- [ ] Have all contract IDs documented
- [ ] Test on testnet first
- [ ] Notify governance (if required)
- [ ] Prepare communication for investors (if user-facing)

### During toggle

- [ ] Run `get_feature_flags()` before change (baseline)
- [ ] Execute flag change with clear reason
- [ ] Verify change: `get_feature_flags()` shows new state
- [ ] Record transaction hash for audit trail
- [ ] Notify ops team (Slack, incident system)

### After toggle

- [ ] Monitor logs for errors (1h checkpoint)
- [ ] Query 3+ instances to confirm change applied
- [ ] Check health metrics (gas, errors, investor operations)
- [ ] Update documentation with timestamp
- [ ] Prepare incident report (if issue occurred)
- [ ] Schedule follow-up review (48h post-toggle)

---

## Troubleshooting

### Feature change didn't take effect

**Check:**

```bash
# Verify state after change
stellar contract invoke --id <ID> -- get_feature_flags

# Check if admin signature was accepted
# Check if transaction was submitted to correct network
stellar contract invoke --id <ID> -- get_feature_flag_history
```

**Likely causes:**

- Transaction not submitted (command error)
- Submitted to wrong network (testnet vs. mainnet)
- Admin key doesn't have authority
- Contract requires different signature format

---

### Feature disabled but investors still see error

**Diagnosis:**

```bash
# Confirm feature is actually disabled
stellar contract invoke --id <ID> -- get_feature_flags | grep feature_name

# Check: Is error coming from different code path?
# (Feature disable only blocks that entrypoint; other errors unrelated)
```

**Resolution:**

- If feature still enabled: toggle again
- If feature disabled: error from different source; investigate elsewhere
- If toggle successful but still errors: clear investor cache or wait for block confirmation

---

### Need to toggle back immediately

```bash
# Emergency: Revert previous flag change
stellar contract invoke \
  --id <ID> \
  --source $ADMIN_SECRET \
  -- set_feature_flag \
  --flag_name "problematic_feature" \
  --enabled true \
  --reason "Reverting: emergency rollback - flag disabled prematurely"

# Verify
stellar contract invoke --id <ID> -- get_feature_flags | grep problematic
```

**Time:** < 30 seconds (instant revert)

---

## Monitoring Dashboard

Track feature flag state across all instances:

```bash
#!/bin/bash
# feature_dashboard.sh (run every 1h)

REPORT_FILE="feature_status_$(date +%Y%m%d_%H%M%S).json"

{
  echo "{"
  echo '  "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",'
  echo '  "instances": ['
  
  FIRST=true
  for INSTANCE in $(cat all_instances.txt); do
    if [ "$FIRST" = false ]; then echo ","; fi
    FIRST=false
    
    echo "    {"
    echo '      "id": "'$INSTANCE'",'
    stellar contract invoke \
      --id $INSTANCE \
      --source $SECRET \
      --network mainnet \
      -- get_feature_flags | jq -c '.' | sed 's/^/      "flags": /'
    echo "    }"
  done
  
  echo "  ]"
  echo "}"
} > $REPORT_FILE

echo "✓ Report saved: $REPORT_FILE"
```

---

## Feature Flag Runbook

| Action | Command | Time | Risk |
|--------|---------|------|------|
| Enable feature | `set_feature_flag --enabled true` | < 1 min | Low |
| Disable feature | `set_feature_flag --enabled false` | < 1 min | Low |
| Check state | `get_feature_flags` | < 10 sec | None |
| View history | `get_feature_flag_history` | < 10 sec | None |
| Per-instance override | `set_instance_feature_override` | < 1 min | Low |
| Deprecate feature | `deprecate_feature` | < 1 min | Low |

**All operations:**
- Require admin key (multisig)
- Logged with reason and timestamp
- Reversible (can toggle back immediately)
- Zero redeploy needed

---

## FAQ

**Q: Can I toggle a feature mid-transaction?**
A: No. Feature checks happen at entrypoint; in-flight transactions see old state. Disable happens for next transaction.

**Q: What if an investor is funding when I disable a feature?**
A: If feature is gated on `fund()`, the disable takes effect on the next call. Current in-flight transaction completes with old logic.

**Q: Can I set per-investor feature flags?**
A: Not directly. But you can use per-instance overrides (set at init time) to customize behavior per escrow.

**Q: What happens if I disable all features?**
A: Escrow still functions (fund, settle, claim work). But advanced features (yields, caps, attestations) blocked.

**Q: Do feature flags survive upgrades?**
A: Yes. Feature flag state stored independently of WASM. Upgrades don't change flags (unless code explicitly resets them).

**Q: Can I schedule a flag change for the future?**
A: Not directly in v6. Deploy code with deprecation timestamp to auto-disable (see deprecate_feature example).

---

## Best Practices

✓ **Test on testnet first** — Always toggle features on testnet 24h before mainnet

✓ **Document with reason** — Every toggle should explain why (governance decision, emergency, A/B test)

✓ **Monitor after toggle** — Check health metrics (error rate, gas, investor complaints) for 24h post-toggle

✓ **Version-control config** — Keep feature flag policies in git for audit trail

✓ **Notify stakeholders** — If user-facing feature, notify investors/integrators before toggle

✓ **Use governance approval** — For critical features, require governance vote before toggling

✓ **Prepare rollback plan** — Always know how to revert (just toggle back)

---

## References

- **FEATURE_FLAGS_AND_RUNTIME_CONFIG.md** — Architecture and implementation
- **OPERATOR_RUNBOOK.md** — General deployment procedures
- **CANARY_DEPLOYMENT_STRATEGY.md** — Staged rollout procedures
