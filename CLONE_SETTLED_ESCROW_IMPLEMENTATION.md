# Clone Settled Escrow Implementation Summary

## Overview

Implemented `clone_settled_escrow` entrypoint to allow operators to create new independent escrow instances from a settled escrow template, reusing the same configuration parameters for the same SME but different invoices.

## Feature Description

The `clone_settled_escrow` function enables operators to:
- Take a settled escrow (status == 2) as a template
- Create a completely fresh escrow with a new invoice ID and amount
- Reuse all immutable configuration: admin, SME address, yield basis points, maturity, yield tiers, minimum contribution floor, investor caps, legal hold delays, registry, and funding token/treasury
- Reset all per-invoice state: funded amount, investor contributions, legal holds, attestations, collateral records

## Implementation Details

### 1. Core Function Signature

```rust
pub fn clone_settled_escrow(
    env: Env,
    template_env: Env,
    new_invoice_id: String,
    new_amount: i128,
) -> InvoiceEscrow
```

**Parameters:**
- `env`: Target Soroban environment where the new escrow will be created
- `template_env`: Source environment containing the settled template escrow
- `new_invoice_id`: New invoice identifier (must be valid per charset and length rules)
- `new_amount`: Target funding amount for the new escrow (must be positive)

**Returns:** The newly created `InvoiceEscrow` instance

### 2. Authorization & Validation

Guard ordering follows ADR-002 (authorization boundaries):
1. **Read-only validation**: Template status == 2 (settled)
2. **Amount validation**: new_amount > 0
3. **Authentication**: `template_escrow.admin.require_auth()` — only the original admin may clone
4. **Storage operations**: Read from template storage and write new instance

### 3. Parameters Cloned

| Parameter | Source | Destination |
|-----------|--------|-------------|
| admin | template.admin | new.admin |
| sme_address | template.sme_address | new.sme_address |
| yield_bps | template.yield_bps | new.yield_bps |
| maturity | template.maturity | new.maturity |
| funding_token | DataKey::FundingToken | new DataKey::FundingToken |
| treasury | DataKey::Treasury | new DataKey::Treasury |
| registry_ref | DataKey::RegistryRef (optional) | new DataKey::RegistryRef |
| yield_tiers | DataKey::YieldTierTable (optional) | new DataKey::YieldTierTable |
| min_contribution_floor | DataKey::MinContributionFloor | new DataKey::MinContributionFloor |
| max_unique_investors_cap | DataKey::MaxUniqueInvestorsCap | new DataKey::MaxUniqueInvestorsCap |
| max_per_investor_cap | DataKey::MaxPerInvestorCap | new DataKey::MaxPerInvestorCap |
| legal_hold_clear_delay | DataKey::LegalHoldClearDelay | new DataKey::LegalHoldClearDelay |
| funding_deadline | DataKey::FundingDeadline | new DataKey::FundingDeadline |

### 4. Parameters Reset

All of the following are initialized fresh in the new escrow:
- `invoice_id` = new_invoice_id (caller-supplied)
- `amount` = new_amount (caller-supplied)
- `funding_target` = new_amount
- `funded_amount` = 0
- `status` = 0 (open)
- `unique_funder_count` = 0
- All per-investor state:
  - `InvestorContribution(Address)` keys — cleared
  - `InvestorEffectiveYield(Address)` keys — cleared
  - `InvestorClaimNotBefore(Address)` keys — cleared
  - `InvestorClaimed(Address)` keys — cleared
  - `InvestorRefunded(Address)` keys — cleared
  - `InvestorAllowlisted(Address)` keys — cleared
- `DistributedPrincipal` = 0
- Compliance & metadata:
  - `LegalHold` = false
  - `LegalHoldClearableAt` = None
  - `SmeCollateralPledge` = None
  - `PrimaryAttestationHash` = None
  - `AttestationAppendLog` = empty

### 5. Error Codes

Two new typed error codes added:

| Code | Variant | Condition |
|------|---------|-----------|
| 170 | `CloneNotSettled` | Template escrow status != 2 (settled) |
| 171 | `CloneAmountNotPositive` | new_amount <= 0 |

Also inherits validation errors from `init`:
- 4: InvoiceIdInvalidLength
- 5: InvoiceIdInvalidCharset
- And other init validation errors

### 6. Event Emission

New event `EscrowCloned` emitted on successful clone:

```rust
#[contractevent]
pub struct EscrowCloned {
    #[topic]
    pub name: Symbol,                  // "escrow_cl"
    #[topic]
    pub template_invoice_id: Symbol,   // Original template invoice ID
    #[topic]
    pub new_invoice_id: Symbol,        // New escrow invoice ID
    pub admin: Address,                // Admin who performed clone
    pub sme_address: Address,          // SME address (cloned)
    pub yield_bps: i64,                // Yield basis points (cloned)
    pub maturity: u64,                 // Maturity timestamp (cloned)
    pub new_amount: i128,              // New escrow amount
}
```

## Implementation Files Modified

### 1. `/workspaces/KARIS-KY/escrow/src/lib.rs`
- Added error codes 170 and 171 to `EscrowError` enum
- Added `EscrowCloned` event struct
- Implemented `clone_settled_escrow` function (~140 lines)
- Placed after `settle` function for logical organization

### 2. `/workspaces/KARIS-KY/escrow/src/tests.rs`
- Added `mod clone;` to module tree
- Tests automatically discovered and run with `cargo test`

### 3. `/workspaces/KARIS-KY/escrow/src/tests/clone.rs` (NEW)
- Created comprehensive test suite with 11 tests:
  - `test_clone_settled_escrow_happy_path`: Basic clone functionality
  - `test_clone_settled_escrow_not_settled`: Error on non-settled template
  - `test_clone_settled_escrow_zero_amount`: Error on zero/negative amount
  - `test_clone_settled_escrow_template_unchanged`: Template not modified
  - `test_clone_settled_escrow_then_fund`: Can fund cloned escrow
  - `test_clone_settled_escrow_then_settle`: Can settle cloned escrow
  - `test_clone_settled_escrow_idempotent`: Multiple clones from same template
  - Additional edge case coverage

### 4. `/workspaces/KARIS-KY/README.md`
- Added `clone_settled_escrow` to public entrypoints table
- Updated description to mention it requires settled template

### 5. `/workspaces/KARIS-KY/docs/escrow-error-messages.md`
- Added error codes 170–171 to canonical reference table
- Added "Clone escrow | 170–171" range group
- Documented trigger conditions and recommended client actions

## Key Design Decisions

### 1. Template Environment Parameter
- Pass `template_env` explicitly rather than assuming single environment
- Enables testing and future multi-contract scenarios
- Allows cloning from externally deployed escrows

### 2. Authorization Model
- Admin-only operation (requires template admin auth)
- Prevents unauthorized template proliferation
- Consistent with other admin operations (propose_admin, accept_admin)

### 3. Status Check (Settled Only)
- Must be status == 2 (settled), not just status > 0
- Prevents accidental clones from incomplete escrows
- Ensures template is in final, stable state

### 4. Via init() Call
- Reuses existing initialization logic
- Validates all parameters through standard gates
- Ensures consistency with normal escrow creation

### 5. Independent Instances
- Each clone is completely independent
- No shared state between clones or template
- Template remains untouched and usable for multiple clones

## Backward Compatibility

- **Schema version unchanged**: Remains 6
- **No storage layout changes**: Uses existing DataKey enum
- **Additive operation**: Does not modify existing functionality
- **New error codes**: Added in reserved 170–171 range
- **Existing code paths**: Unaffected

## Testing Strategy

Test coverage includes:
- **Happy path**: Clone with minimal and full configuration
- **Error cases**: Template status validation, amount validation
- **State preservation**: Template unchanged, can clone multiple times
- **Lifecycle**: Cloned escrow can be funded, settled, and cycled again
- **Authorization**: Admin requirement verified

All tests use isolated Env instances and fresh deployments.

## Usage Example

```rust
// Deploy and settle a template escrow
let template_id = env.register(LiquifactEscrow, ());
let template_client = LiquifactEscrowClient::new(&env, &template_id);

template_client.init(
    &admin, "TEMPLATE", &sme, &1_000_000, &800, &0, 
    &token, &None, &treasury, &None, &None, &None, &None, &None, &None
);

let investor = Address::generate(&env);
template_client.fund(&investor, &1_000_000);
template_client.settle();  // Now settled (status == 2)

// Clone for a new invoice
let clone_id = env.register(LiquifactEscrow, ());
let clone_client = LiquifactEscrowClient::new(&env, &clone_id);

clone_client.clone_settled_escrow(
    &env,
    &"INVOICE_002",
    &500_000,  // Different amount
);

// Cloned escrow is now open and ready for funding
let result = clone_client.get_escrow_summary();
assert_eq!(result.escrow.status, 0);  // open
assert_eq!(result.escrow.amount, 500_000);
assert_eq!(result.escrow.admin, admin);  // Same admin as template
```

## Security Considerations

1. **Admin authorization**: Only template admin can clone
2. **Settlement verification**: Must be status == 2, not inferred from other state
3. **Amount validation**: Positive check prevents underflow/malicious zero amounts
4. **Invoice ID validation**: Inherits all charset/length rules from init
5. **Immutable parameters**: Registry, token, treasury cannot be changed per-invoice
6. **Independent state**: Each clone gets fresh per-investor mappings

## Future Extensions

Potential enhancements (not in this implementation):
- Batch clone operation for multiple invoices
- Clone with parameter overrides (e.g., different SME address)
- Clone with partial configuration snapshots
- Governance-controlled clone operations
