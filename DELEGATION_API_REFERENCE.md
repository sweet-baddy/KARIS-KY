# Yield Claim Delegation - API Reference

## Public Entrypoints

### 1. `set_yield_claim_delegate`

Sets or overwrites an investor's yield claim delegation.

```rust
pub fn set_yield_claim_delegate(env: Env, investor: Address, delegate: Address)
```

**Parameters:**
- `investor`: Address of the investor (must sign)
- `delegate`: Address to delegate claim rights to

**Authorization:**
- Requires: `investor.require_auth()`
- Caller: Investor or their authorized representative

**Validation:**
- ✅ `delegate ≠ investor` (error 165)

**Side Effects:**
- Sets `DataKey::YieldClaimDelegate(investor)` → `delegate`
- Clears `DataKey::YieldClaimDelegateRevoked(investor)` → `false`
- Emits `YieldClaimDelegationSet` event

**Errors:**
```rust
EscrowError::DelegateAddressSameAsInvestor = 165
```

**Example:**
```rust
let investor = Address::generate(&env);
let multisig = Address::generate(&env);
client.set_yield_claim_delegate(&investor, &multisig);
```

---

### 2. `revoke_yield_claim_delegate`

Revokes an investor's yield claim delegation.

```rust
pub fn revoke_yield_claim_delegate(env: Env, investor: Address)
```

**Parameters:**
- `investor`: Address of the investor (must sign)

**Authorization:**
- Requires: `investor.require_auth()`
- Caller: Investor

**Validation:**
- ✅ Active delegation exists (error 168)
- ✅ Delegation not already revoked

**Side Effects:**
- Sets `DataKey::YieldClaimDelegateRevoked(investor)` → `true`
- Preserves `DataKey::YieldClaimDelegate(investor)` (for audit)
- Emits `YieldClaimDelegationRevoked` event

**Errors:**
```rust
EscrowError::NoActiveDelegation = 168
```

**Example:**
```rust
let investor = Address::generate(&env);
client.revoke_yield_claim_delegate(&investor);
```

---

### 3. `claim_investor_payout_as_delegate`

Delegate claims an investor's payout after settlement.

```rust
pub fn claim_investor_payout_as_delegate(
    env: Env,
    investor: Address,
    delegate: Address,
)
```

**Parameters:**
- `investor`: Address of the investor who contributed
- `delegate`: Address claiming on behalf of investor (must sign)

**Authorization:**
- Requires: `delegate.require_auth()`
- Caller: The delegated address

**Validation:**
- ✅ Escrow not under legal hold
- ✅ Investor has contribution > 0 (error 126)
- ✅ Delegation exists (error 166)
- ✅ Delegation not revoked (error 167)
- ✅ Provided delegate matches stored delegate (error 166)
- ✅ Escrow is settled (status = 2, error 127)
- ✅ Claim lock expired (error 128)

**Side Effects:**
- Sets `DataKey::InvestorClaimed(investor)` → `true` (if not already)
- Emits `InvestorPayoutClaimed` event (same as `claim_investor_payout`)

**Idempotency:**
- Multiple calls from same delegate are no-op (returns silently)
- Investor can also call `claim_investor_payout` directly (both set same flag)

**Errors:**
```rust
EscrowError::LegalHoldBlocksInvestorClaims = 125
EscrowError::NoContributionToClaim = 126
EscrowError::InvestorClaimNotSettled = 127
EscrowError::InvestorCommitmentLockNotExpired = 128
EscrowError::NoDelegationSet = 166
EscrowError::DelegationRevoked = 167
```

**Example:**
```rust
let investor = Address::generate(&env);
let delegate = Address::generate(&env);

// Investor sets delegation
env.mock_all_auths();
client.set_yield_claim_delegate(&investor, &delegate);

// Investor funds
client.fund(&investor, &5000);

// SME settles after maturity
client.settle();

// Delegate claims payout
env.mock_auths(&[MockAuth {
    address: delegate.clone(),
    invoke: MockInvoke::Contract(..),
    ..Default::default()
}]);
client.claim_investor_payout_as_delegate(&investor, &delegate);
```

---

## Public Query Functions

### 4. `get_yield_claim_delegate`

Reads the currently set delegate for an investor, if any.

```rust
pub fn get_yield_claim_delegate(env: Env, investor: Address) -> Option<Address>
```

**Returns:**
- `Some(address)`: If delegation is set (whether active or revoked)
- `None`: If no delegation has ever been set

**Authorization:**
- None (read-only)

**Example:**
```rust
let investor = Address::generate(&env);
match client.get_yield_claim_delegate(&investor) {
    Some(delegate) => println!("Delegated to: {}", delegate),
    None => println!("No delegation set"),
}
```

---

### 5. `is_yield_claim_delegate_revoked`

Checks if a delegation has been revoked.

```rust
pub fn is_yield_claim_delegate_revoked(env: Env, investor: Address) -> bool
```

**Returns:**
- `true`: If delegation exists and has been revoked
- `false`: If delegation never set, or active, or not yet revoked

**Authorization:**
- None (read-only)

**Example:**
```rust
let investor = Address::generate(&env);
if client.is_yield_claim_delegate_revoked(&investor) {
    println!("Delegation is revoked");
} else {
    println!("Delegation active or not set");
}
```

---

## Related Existing Functions

### `claim_investor_payout`

Unchanged. Investor can still claim directly (authorization: `investor.require_auth()`).

```rust
pub fn claim_investor_payout(env: Env, investor: Address)
```

**Behavior:**
- Investor claims their own payout (no delegation)
- Sets same `InvestorClaimed` flag as delegate claim
- Emits same `InvestorPayoutClaimed` event

---

### `compute_investor_payout`

Unchanged. Computes gross pro-rata payout (read-only).

```rust
pub fn compute_investor_payout(env: Env, investor: Address) -> i128
```

**Returns:**
- Pro-rata payout amount (same whether claimed by investor or delegate)
- Formula: See ADR-005 or `docs/escrow-pro-rata.md`

---

## Events

### `YieldClaimDelegationSet`

Emitted when delegation is set.

```rust
pub struct YieldClaimDelegationSet {
    #[topic]
    pub name: Symbol,          // symbol_short!("del_set")
    #[topic]
    pub investor: Address,     // Who delegated
    #[topic]
    pub delegate: Address,     // Delegated to whom
}
```

**Emitted by:** `set_yield_claim_delegate`

**Indexing:** Topics = (name, investor, delegate) ⇒ can filter by investor or delegate

---

### `YieldClaimDelegationRevoked`

Emitted when delegation is revoked.

```rust
pub struct YieldClaimDelegationRevoked {
    #[topic]
    pub name: Symbol,          // symbol_short!("del_rev")
    #[topic]
    pub investor: Address,     // Who revoked
}
```

**Emitted by:** `revoke_yield_claim_delegate`

**Indexing:** Topics = (name, investor) ⇒ can filter by investor

---

## Error Codes

### Delegation-Specific Errors

| Code | Enum | Meaning | Resolution |
|------|------|---------|-----------|
| 165 | `DelegateAddressSameAsInvestor` | Delegate = investor | Use different address |
| 166 | `NoDelegationSet` | No delegation exists (or wrong delegate) | Set delegation first |
| 167 | `DelegationRevoked` | Delegation was revoked | Investor must re-set |
| 168 | `NoActiveDelegation` | No active delegation to revoke | Set delegation first |

### Related Errors (Can Occur During Delegation Claim)

| Code | Enum | Meaning | Resolution |
|------|------|---------|-----------|
| 125 | `LegalHoldBlocksInvestorClaims` | Legal hold active | Wait for admin to clear |
| 126 | `NoContributionToClaim` | Investor never contributed | Fund first |
| 127 | `InvestorClaimNotSettled` | Escrow not settled yet | Wait for settlement |
| 128 | `InvestorCommitmentLockNotExpired` | Claim lock still active | Wait for lock to expire |

---

## State Machine

### Investor Delegation States

```
┌─────────────────────────────────────────┐
│      No Delegation (Initial)            │
│  • Keys absent                          │
│  • Claim by: investor only              │
└─────────────────────────────────────────┘
            │
            │ set_yield_claim_delegate()
            ↓
┌─────────────────────────────────────────┐
│     Active Delegation                   │
│  • YieldClaimDelegate(investor) = X     │
│  • YieldClaimDelegateRevoked = false    │
│  • Claim by: investor OR delegate       │
└─────────────────────────────────────────┘
            │
            ├─→ revoke_yield_claim_delegate()
            │           ↓
            │   ┌─────────────────────────────────────────┐
            │   │     Revoked Delegation                  │
            │   │  • YieldClaimDelegate(investor) = X     │
            │   │  • YieldClaimDelegateRevoked = true     │
            │   │  • Claim by: investor only              │
            │   └─────────────────────────────────────────┘
            │           │
            │           ↑
            └─→ set_yield_claim_delegate() (overwrites)
                    ↓
                (back to Active)
```

---

## Authorization Model (ADR-002)

All delegation operations follow strict guard ordering:

### `set_yield_claim_delegate` Flow

```
┌─ Read-Only Preconditions ─┐
│ • Validate delegate ≠ investor
└─────────────────────────────┘
           ↓
┌─ Authorization ─────────┐
│ • investor.require_auth()
└──────────────────────────┘
           ↓
┌─ Storage & Events ──────┐
│ • Set delegate
│ • Clear revocation flag
│ • Emit event
└────────────────────────┘
```

### `claim_investor_payout_as_delegate` Flow

```
┌─ Read-Only Preconditions ─────────────┐
│ • Legal hold active?
│ • Investor contribution > 0?
│ • Delegation exists?
│ • Delegation revoked?
│ • Right delegate?
│ • Escrow settled?
│ • Claim lock expired?
└────────────────────────────────────────┘
           ↓
┌─ Authorization ──────────┐
│ • delegate.require_auth()
└──────────────────────────┘
           ↓
┌─ Storage & Events ────┐
│ • Set InvestorClaimed
│ • Emit event
└──────────────────────┘
```

---

## Best Practices

### For Investors

1. **Set delegation early** (right after funding)
   ```rust
   client.fund(&investor, &amount);
   client.set_yield_claim_delegate(&investor, &delegate);
   ```

2. **Verify delegation was set**
   ```rust
   let stored = client.get_yield_claim_delegate(&investor);
   assert_eq!(stored, Some(delegate));
   ```

3. **Revoke before changing delegates**
   ```rust
   client.revoke_yield_claim_delegate(&investor);
   client.set_yield_claim_delegate(&investor, &new_delegate);
   ```

### For Delegates

1. **Check delegation is active before claiming**
   ```rust
   let delegate_addr = client.get_yield_claim_delegate(&investor);
   assert_eq!(delegate_addr, Some(my_address));
   assert!(!client.is_yield_claim_delegate_revoked(&investor));
   ```

2. **Handle idempotency gracefully**
   ```rust
   // Safe to call multiple times (no re-emit)
   client.claim_investor_payout_as_delegate(&investor, &delegate);
   ```

3. **Verify claim succeeded**
   ```rust
   let claimed = client.is_investor_claimed(&investor);
   assert!(claimed);
   ```

### For Indexers

1. **Track delegation changes**
   ```
   YieldClaimDelegationSet → new delegation active
   YieldClaimDelegationRevoked → delegation no longer usable
   InvestorPayoutClaimed → claim recorded (works for both direct & delegate)
   ```

2. **Compute delegation status**
   ```
   delegation_active = (
       YieldClaimDelegate set 
       && YieldClaimDelegateRevoked = false
   )
   ```

---

## Interaction with Other Features

### Legal Hold
- Blocks both `claim_investor_payout` and `claim_investor_payout_as_delegate` (same error 125)
- Does not prevent `set_yield_claim_delegate` or `revoke_yield_claim_delegate`

### Tiered Yield & Commitment Locks
- Yield computation same whether claimed by investor or delegate
- Commitment lock gates respected by both
- No change to yield_bps or claim_not_before logic

### Allowlisting
- Delegation only affects **claiming**, not **funding**
- Investor must be allowlisted to fund (unchanged)
- Delegate does not need to be allowlisted

### Settlement & Maturity
- Maturity gate checked same for both (status = 2, maturity reached)
- Claim lock (`InvestorClaimNotBefore`) gated same for both
- Pro-rata computation unchanged

---

## Testing

See `/workspaces/KARIS-KY/escrow/src/tests/delegation.rs` for:
- 13 comprehensive tests
- Error case coverage
- State machine verification
- Event emission checks
- Integration scenarios

Run tests with:
```bash
cd escrow
cargo test delegation
```

---

## Changelog

| Version | Date | Change |
|---------|------|--------|
| 1.0 | 2026-07-25 | Initial delegation API release |

**Schema Version**: 7 (additive-only, backward compatible with v6)

---

## Questions?

Refer to:
- `YIELD_CLAIM_DELEGATION_SUMMARY.md` – Architecture & rationale
- `docs/adr/ADR-002-auth-boundaries.md` – Authorization model
- `docs/adr/ADR-003-settlement-flow.md` – Settlement interaction
- `docs/adr/ADR-004-legal-hold.md` – Legal hold interaction
