# On-Chain Feature Flags and Runtime Configuration

Enable/disable contract features dynamically without redeployment.

---

## Overview: Feature Flags Architecture

Feature flags allow operators to toggle contract capabilities on or off without changing WASM code or redeploying instances.

### Benefits

- ✓ Zero-downtime feature toggles (no upgrade needed)
- ✓ Gradual rollout (enable for subset of instances or users)
- ✓ Emergency disable (block risky features if bugs discovered)
- ✓ A/B testing (compare old vs. new behavior)
- ✓ Backwards compatibility (old instances continue working)

### Implementation strategy

```
Feature flag stored as boolean or enum in contract storage
  ├─ Enabled by default (conservative)
  ├─ Admin-gated (governance controls)
  ├─ Checked at entrypoint (early exit if disabled)
  └─ Audit logged (who changed, when, why)
```

---

## Part 1: Feature Flag Storage

### 1.1 DataKey for feature flags

```rust
// In escrow/src/lib.rs
pub enum DataKey {
    // ... existing keys ...
    
    // Feature flags (v6+)
    FeatureFlags,                           // Contains bitmask or struct
    FeatureFlagHistory,                     // Audit log
}

#[derive(Clone)]
pub struct FeatureFlags {
    pub tiered_yield_enabled: bool,         // v5+ feature
    pub attestation_enabled: bool,          // v4+ feature
    pub investor_caps_enabled: bool,        // v3+ feature
    pub funding_snapshot_enabled: bool,     // v3+ feature
    pub collateral_metadata_enabled: bool,  // v6+ feature
    pub legal_hold_grace_period_enabled: bool,  // v7+ feature (future)
}

impl FeatureFlags {
    pub fn all_enabled() -> Self {
        Self {
            tiered_yield_enabled: true,
            attestation_enabled: true,
            investor_caps_enabled: true,
            funding_snapshot_enabled: true,
            collateral_metadata_enabled: true,
            legal_hold_grace_period_enabled: true,
        }
    }
    
    pub fn all_disabled() -> Self {
        Self {
            tiered_yield_enabled: false,
            attestation_enabled: false,
            investor_caps_enabled: false,
            funding_snapshot_enabled: false,
            collateral_metadata_enabled: false,
            legal_hold_grace_period_enabled: false,
        }
    }
}
```

### 1.2 Audit log for flag changes

```rust
#[derive(Clone)]
pub struct FeatureFlagChange {
    pub timestamp: u64,
    pub admin: Address,
    pub flag_name: String,
    pub old_value: bool,
    pub new_value: bool,
    pub reason: String,  // Optional: governance note
}

pub struct FeatureFlagHistory {
    pub changes: Vec<FeatureFlagChange>,
    pub max_entries: u32,  // Bounded at 100 entries
}
```

---

## Part 2: Admin Entrypoints for Feature Management

### 2.1 Set individual feature flag

```bash
# Admin-only entrypoint
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- set_feature_flag \
  --flag_name "tiered_yield_enabled" \
  --enabled true \
  --reason "Enabling tiered yield for v6+ instances"
```

**Pseudo-code:**
```rust
pub fn set_feature_flag(
    env: Env,
    flag_name: String,
    enabled: bool,
    reason: String,
) -> Result<(), EscrowError> {
    // Require admin auth
    let admin = Self::load_escrow_require_admin(env.clone());
    
    // Get current flags
    let mut flags = env.storage().instance()
        .get::<_, FeatureFlags>(&DataKey::FeatureFlags)
        .unwrap_or(FeatureFlags::all_enabled());
    
    // Get old value
    let old_value = match flag_name.as_str() {
        "tiered_yield_enabled" => flags.tiered_yield_enabled,
        "attestation_enabled" => flags.attestation_enabled,
        // ... match other flags ...
        _ => return Err(EscrowError::InvalidFeatureFlagName),
    };
    
    // Update flag
    match flag_name.as_str() {
        "tiered_yield_enabled" => flags.tiered_yield_enabled = enabled,
        "attestation_enabled" => flags.attestation_enabled = enabled,
        // ... match other flags ...
        _ => return Err(EscrowError::InvalidFeatureFlagName),
    };
    
    // Store updated flags
    env.storage().instance().set(&DataKey::FeatureFlags, &flags);
    
    // Log change
    Self::log_feature_flag_change(
        env.clone(),
        admin,
        flag_name.clone(),
        old_value,
        enabled,
        reason,
    );
    
    // Emit event
    env.events().publish(
        ("feature_flag_changed", flag_name.clone(), enabled),
        (),
    );
    
    Ok(())
}
```

### 2.2 Get current feature flags

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- get_feature_flags
```

**Returns:**
```json
{
  "tiered_yield_enabled": true,
  "attestation_enabled": true,
  "investor_caps_enabled": true,
  "funding_snapshot_enabled": true,
  "collateral_metadata_enabled": false,
  "legal_hold_grace_period_enabled": false
}
```

### 2.3 Get feature flag history

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- get_feature_flag_history \
  --limit 10
```

**Returns:**
```json
{
  "changes": [
    {
      "timestamp": 1722084000,
      "admin": "GADMIN...",
      "flag_name": "tiered_yield_enabled",
      "old_value": false,
      "new_value": true,
      "reason": "Governance vote 2024-07-27: Enable tiered yields for all instances"
    },
    {
      "timestamp": 1722087600,
      "admin": "GADMIN...",
      "flag_name": "collateral_metadata_enabled",
      "old_value": true,
      "new_value": false,
      "reason": "Emergency: collateral metadata parsing error detected in mainnet"
    }
  ]
}
```

---

## Part 3: Using Feature Flags in Entrypoints

### 3.1 Tiered yield feature gate

```rust
pub fn fund_with_commitment(
    env: Env,
    investor: Address,
    amount: i128,
    tier: u32,
    lock_period_secs: u64,
) -> Result<(), EscrowError> {
    // Check: is tiered yield enabled?
    let flags = env.storage().instance()
        .get::<_, FeatureFlags>(&DataKey::FeatureFlags)
        .unwrap_or(FeatureFlags::all_enabled());
    
    if !flags.tiered_yield_enabled {
        // Feature disabled: reject with typed error
        return Err(EscrowError::FeatureDisabled); // Error code: 200
    }
    
    // Proceed with tiered yield logic
    // ... rest of function ...
}
```

### 3.2 Attestation feature gate

```rust
pub fn bind_primary_attestation_hash(
    env: Env,
    hash: BytesN<32>,
) -> Result<(), EscrowError> {
    let flags = env.storage().instance()
        .get::<_, FeatureFlags>(&DataKey::FeatureFlags)
        .unwrap_or(FeatureFlags::all_enabled());
    
    if !flags.attestation_enabled {
        return Err(EscrowError::FeatureDisabled);
    }
    
    // Proceed with attestation logic
}
```

### 3.3 Investor caps feature gate

```rust
pub fn fund(
    env: Env,
    investor: Address,
    amount: i128,
) -> Result<(), EscrowError> {
    let escrow = Self::get_escrow(env.clone());
    
    // Check caps only if feature enabled
    let flags = env.storage().instance()
        .get::<_, FeatureFlags>(&DataKey::FeatureFlags)
        .unwrap_or(FeatureFlags::all_enabled());
    
    if flags.investor_caps_enabled {
        // Enforce caps
        if let Some(max_cap) = escrow.max_unique_investors {
            let current_count = Self::get_unique_funder_count(env.clone());
            if current_count >= max_cap {
                return Err(EscrowError::MaxInvestorsReached);
            }
        }
    }
    // else: caps disabled, accept funding without cap checks
    
    // Proceed with funding
}
```

---

## Part 4: Per-Instance Feature Configuration

### 4.1 Instance-specific flags

Some instances may have different feature requirements:

```rust
#[derive(Clone)]
pub struct InvoiceEscrow {
    // ... existing fields ...
    pub custom_feature_overrides: Option<FeatureFlags>,  // Per-instance overrides
}
```

**Usage:**
```rust
// Check: do we have instance-specific overrides?
let flags = if let Some(overrides) = escrow.custom_feature_overrides {
    // Use instance-specific flags
    overrides
} else {
    // Fall back to contract-wide flags
    env.storage().instance()
        .get::<_, FeatureFlags>(&DataKey::FeatureFlags)
        .unwrap_or(FeatureFlags::all_enabled())
};
```

### 4.2 Set instance-specific override

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- set_instance_feature_override \
  --flag_name "tiered_yield_enabled" \
  --enabled false \
  --reason "This escrow uses v5 tier config; disable new yield logic"
```

---

## Part 5: Feature Deprecation Policy

### 5.1 Feature lifecycle

```
ENABLED (default)
  ↓
DEPRECATED (warn users, set expiry date)
  ↓
DISABLED (end date reached, flag turns off)
  ↓
REMOVED (future version: feature code deleted)
```

### 5.2 Deprecation flag structure

```rust
pub struct DeprecatedFeature {
    pub flag_name: String,
    pub deprecated_at: u64,        // Timestamp
    pub disable_at: u64,           // Auto-disable date
    pub removal_version: u32,      // Version where code is removed
    pub deprecation_reason: String,
}
```

### 5.3 Deprecation announcement

```bash
# Admin announces deprecation
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $ADMIN_SECRET \
  --network mainnet \
  -- deprecate_feature \
  --flag_name "old_attestation_v4" \
  --disable_at 1730000000 \
  --removal_version 8 \
  --reason "Replaced by new attestation API in v7; will be removed in v8"
```

---

## Part 6: Deployment Scenarios

### Scenario 1: Gradual rollout of tiered yield

```bash
# Day 1: Verify canary instances
stellar contract invoke --id CANARY_1 -- get_feature_flags

# Day 2: Enable on canary only
for CANARY in CANARY_1 CANARY_2; do
  stellar contract invoke \
    --id $CANARY \
    --source $ADMIN_SECRET \
    -- set_feature_flag \
    --flag_name "tiered_yield_enabled" \
    --enabled true \
    --reason "Canary testing: tiered yield rollout"
done

# Day 3-4: Monitor health
stellar contract invoke --id CANARY_1 -- get_feature_flag_history

# Day 5: Governance approves production
# Day 6: Enable on production
for PROD in PROD_1 PROD_2 PROD_3; do
  stellar contract invoke \
    --id $PROD \
    --source $ADMIN_SECRET \
    -- set_feature_flag \
    --flag_name "tiered_yield_enabled" \
    --enabled true \
    --reason "Governance vote 2024-07-28: Enable tiered yield production-wide"
done
```

### Scenario 2: Emergency disable

**Situation:** Bug discovered in collateral metadata

```bash
# Immediate: Disable across ALL instances
for INSTANCE in $(cat all_instances.txt); do
  stellar contract invoke \
    --id $INSTANCE \
    --source $ADMIN_SECRET \
    -- set_feature_flag \
    --flag_name "collateral_metadata_enabled" \
    --enabled false \
    --reason "EMERGENCY: XDR parse error; disabling until fix verified"
done

# Deploy fix, re-enable after validation
```

### Scenario 3: A/B testing

**Goal:** Compare old vs. new yield calculation

```bash
# Set A: Old logic (disabled)
stellar contract invoke \
  --id CANARY_OLD_LOGIC_1 \
  --source $ADMIN_SECRET \
  -- set_feature_flag \
  --flag_name "new_yield_calculation_v7" \
  --enabled false \
  --reason "A/B test: baseline (old yield logic)"

# Set B: New logic (enabled)
stellar contract invoke \
  --id CANARY_NEW_LOGIC_1 \
  --source $ADMIN_SECRET \
  -- set_feature_flag \
  --flag_name "new_yield_calculation_v7" \
  --enabled true \
  --reason "A/B test: new yield calculation"

# Monitor metrics for 7 days, compare winner
```

---

## Part 7: Feature Flag in Upgrade Workflow

### Avoiding redeploy via flags

**Traditional (requires redeploy):**
```
Bug found → Fix code → Redeploy WASM → New contract ID → Investor migration
```

**With feature flags (zero redeploy):**
```
Bug found → Disable flag instantly → Fix code → Re-enable → Gradual rollout
```

### Example: Emergency response

```bash
# Bug discovered in canary
# Instead of rolling back WASM:

stellar contract invoke \
  --id CANARY_INSTANCE \
  --source $ADMIN_SECRET \
  -- set_feature_flag \
  --flag_name "problematic_feature" \
  --enabled false \
  --reason "Canary incident: disabling until fix verified"

# Escrow continues, feature just disabled
# No storage loss, no investor re-recording
# Fix and re-enable when ready
```

---

## Part 8: Feature Flag Best Practices

### DO

✓ Start with feature disabled (safe default)
✓ Require governance approval for critical features
✓ Log all flag changes with reason
✓ Announce deprecations well in advance
✓ Test disable/enable in non-prod first
✓ Monitor metrics when toggling
✓ Use clear, descriptive flag names

### DON'T

✗ Use flags for configuration (use separate config)
✗ Hide critical fixes behind flags
✗ Disable without documenting reason
✗ Keep deprecated features after removal
✗ Deploy with all flags disabled
✗ Change flags frequently without tracking

---

## Part 9: Error Codes for Feature Flags

```
Code | Variant | Trigger | Action |
-----|---------|---------|--------|
200 | FeatureDisabled | Feature off | Governance enables or retry later |
201 | InvalidFeatureFlagName | Unknown flag | Check flag names |
202 | UnauthorizedFlagChange | Non-admin | Request governance |
203 | FeatureFlagHistoryFull | Audit log capped | Archive history |
```

---

## Summary

Feature flags enable:

✓ Zero-downtime toggles (no redeploy)
✓ Emergency disable (fast bug response)
✓ Gradual rollout (canary → production)
✓ A/B testing (compare implementations)
✓ Backwards compatibility (legacy instances)
✓ Governance control (who toggles what)
✓ Audit trail (change history logged)

Implement in v6+ to reduce redeployment burden and enable safer experimentation.
