# Code Locations - Automatic Yield Distribution Implementation

## Quick Reference - Line Numbers

### Data Structures and Enums

**YieldDistributionSnapshot Type**
- File: `escrow/src/lib.rs`
- Lines: 769-782
- Doc: Immutable per-investor yield snapshot at settlement
- Fields: payout_amount, captured_at_ledger_timestamp, captured_at_ledger_sequence

**DataKey Enum Additions**
- File: `escrow/src/lib.rs`
- Line 575: `YieldDistributionShare(Address)` - Per-investor persistent yield storage
- Line 578: `YieldAutoDistributionEnabled` - Feature flag

### Event Types

**YieldDistributionSnapshotCreated**
- File: `escrow/src/lib.rs`
- Lines: 1064-1072
- Emitted: At settlement when auto-distribution enabled
- Fields: invoice_id, settled_amount, investor_count, timestamp

**AutoDistributedYieldClaimed**
- File: `escrow/src/lib.rs`
- Lines: 1083-1091
- Emitted: When investor claims pre-computed yield
- Fields: invoice_id, investor, payout_amount

**YieldAutoDistributionEnabled**
- File: `escrow/src/lib.rs`
- Lines: 1096-1102
- Emitted: When admin enables feature
- Fields: invoice_id, enabled, timestamp

**YieldAutoDistributionDisabled**
- File: `escrow/src/lib.rs`
- Lines: 1104-1110
- Emitted: When admin disables feature
- Fields: invoice_id, timestamp

### Helper Functions (Private)

**get_persistent_yield_distribution_share**
- File: `escrow/src/lib.rs`
- Lines: 2896-2903
- Returns: `Option<YieldDistributionSnapshot>`
- Purpose: Retrieve pre-computed yield for investor

**set_persistent_yield_distribution_share**
- File: `escrow/src/lib.rs`
- Lines: 2904-2912
- Parameters: `(env, investor, snapshot)`
- Purpose: Store pre-computed yield for investor

**is_yield_auto_distribution_enabled (private)**
- File: `escrow/src/lib.rs`
- Lines: 2913-2920
- Returns: `bool`
- Purpose: Check if feature is enabled

### Public Entrypoints

**enable_yield_auto_distribution**
- File: `escrow/src/lib.rs`
- Lines: 3455-3468
- Auth: `escrow.admin.require_auth()`
- Effect: Sets `YieldAutoDistributionEnabled = true`
- Emits: `YieldAutoDistributionEnabled` event

**disable_yield_auto_distribution**
- File: `escrow/src/lib.rs`
- Lines: 3480-3495
- Auth: `escrow.admin.require_auth()`
- Effect: Sets `YieldAutoDistributionEnabled = false`
- Emits: `YieldAutoDistributionDisabled` event

**is_yield_auto_distribution_enabled (public query)**
- File: `escrow/src/lib.rs`
- Lines: 3500-3504
- Returns: `bool`
- Auth: None (read-only)
- Purpose: Check current feature status

### Modified Core Functions

**settle()**
- File: `escrow/src/lib.rs`
- Settlement snapshot computation: Lines 4794-4846
- Logic:
  1. Check if auto-distribution enabled AND full settlement
  2. Get FundingCloseSnapshot for pro-rata denominator
  3. Compute net coupon (after protocol fees)
  4. Calculate settlement pool
  5. Emit YieldDistributionSnapshotCreated event
  6. Store investor count and metadata for future enumeration

**claim_investor_payout()**
- File: `escrow/src/lib.rs`
- Pre-computed yield check: Lines 5214-5232
- Logic:
  1. Mark investor as claimed
  2. Check for pre-computed yield in YieldDistributionShare
  3. If found: Use payout_amount, emit AutoDistributedYieldClaimed
  4. If not found: Fallback to compute_investor_payout()
  5. Continue with reinvestment and health check logic
- Maintains: All existing guards, idempotency, authorization

### Test Suite

**New Test Module**
- File: `escrow/src/tests/yield_distribution.rs`
- Size: 487 lines
- Tests: 13 comprehensive tests

**Test Imports**
- File: `escrow/src/tests.rs`
- Line: Added `mod yield_distribution;` to module tree

**Individual Tests (by category)**

Feature Control:
- Line ~40-50: `enable_auto_distribution_sets_flag()`
- Line ~60-70: `disable_auto_distribution_clears_flag()`
- Line ~75-85: `auto_distribution_defaults_to_disabled()`

Settlement:
- Line ~90-110: `settlement_with_auto_dist_disabled_no_snapshot()`
- Line ~115-165: `settlement_with_auto_dist_enabled_creates_snapshot()`

Claims:
- Line ~170-215: `claim_with_auto_dist_enabled_emits_event()`
- Line ~220-265: `claim_with_auto_dist_disabled_uses_on_demand()`

Idempotency:
- Line ~270-305: `auto_dist_claim_is_idempotent()`

Multi-Investor:
- Line ~310-360: `multi_investor_auto_dist_all_claim()`

Backwards Compatibility:
- Line ~365-385: `default_escrow_has_auto_dist_disabled()`
- Line ~390-425: `auto_dist_feature_backward_compatible()`

Authorization:
- Line ~430-450: `enable_auto_dist_requires_admin()` (should_panic)
- Line ~460-480: `disable_auto_dist_requires_admin()` (should_panic)

### Documentation Files

**Implementation Guide**
- File: `YIELD_DISTRIBUTION_IMPLEMENTATION.md`
- Coverage: Architecture, features, acceptance criteria, security, performance

**Completion Summary**
- File: `AUTOMATIC_YIELD_DISTRIBUTION_COMPLETION.md`
- Coverage: Full summary, acceptance criteria verification, testing, future work

**Code Locations Reference**
- File: `CODE_LOCATIONS_YIELD_DISTRIBUTION.md` (this file)
- Coverage: Line numbers and quick reference

## Key Integration Points

### Settlement Flow
```
settle() {
  [... existing logic ...]
  if is_yield_auto_distribution_enabled && is_full_settlement {
    // Lines 4794-4846: NEW yield distribution snapshot
    compute snapshot metadata
    emit YieldDistributionSnapshotCreated
  }
  [... publish settlement events ...]
}
```

### Claim Flow
```
claim_investor_payout() {
  [... existing guards ...]
  // Lines 5214-5232: NEW pre-computed yield check
  if let Some(yield_dist) = get_persistent_yield_distribution_share(investor) {
    use yield_dist.payout_amount
    emit AutoDistributedYieldClaimed
  } else {
    use compute_investor_payout() // fallback
  }
  [... continue with reinvestment ...]
}
```

### Admin Control
```
Admin can:
1. enable_yield_auto_distribution() → sets flag, emits event
2. disable_yield_auto_distribution() → clears flag, emits event
3. is_yield_auto_distribution_enabled() → check status
```

## Schema Impact

**SCHEMA_VERSION**: Still 9 (no change needed)
- New DataKey variants are additive
- No migration required
- Gracefully ignored on old instances

**Storage Changes**:
- Add: `DataKey::YieldDistributionShare(Address)` - persistent, per-investor
- Add: `DataKey::YieldAutoDistributionEnabled` - instance, single flag
- No removed or modified keys

## Backwards Compatibility

✅ All additions are opt-in
✅ Default behavior unchanged (feature disabled)
✅ Existing code paths unaffected
✅ Fallback logic for missing pre-computed yields
✅ Full compatibility with:
   - Tiered yields
   - Commitment locks
   - Reinvestment
   - Delegation
   - All existing features

## Testing Requirements

Run tests with:
```bash
cargo test yield_distribution --lib -- --nocapture
```

All 13 tests should pass:
- Feature control: 3 tests
- Settlement: 2 tests
- Claims: 2 tests
- Idempotency: 1 test
- Multi-investor: 1 test
- Backwards compat: 2 tests
- Authorization: 2 tests

## Deployment Considerations

1. **No Migration Path Required**: Feature is additive only
2. **No Config Changes**: Works with existing init parameters
3. **Enable at Will**: Admins can enable per-escrow as needed
4. **Zero Breaking Changes**: Existing deployments continue unchanged

---

**Last Updated**: 2024-07-29
**Implementation Status**: Complete ✅
**Code Review Status**: Ready for review
