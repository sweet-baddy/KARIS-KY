# Yield Claim Delegation - Visual Guide

## Feature Overview

```
┌──────────────────────────────────────────────────────────────────────┐
│                    ESCROW INVOICE LIFECYCLE                         │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  OPEN PHASE                                                          │
│  ┌─────────────────────────┐                                        │
│  │ Investor A funds        │  ← NEW: Can delegate here              │
│  │ $1000 principal         │                                         │
│  └─────────────────────────┘                                        │
│           │                                                          │
│           ↓                                                          │
│  ┌─────────────────────────────┐                                    │
│  │ set_yield_claim_delegate    │ ← Investor A → Multisig            │
│  │ (A, MultisigAddress)        │                                    │
│  └─────────────────────────────┘                                    │
│           │                                                          │
│           ↓                                                          │
│  FUNDED PHASE                                                        │
│  ┌─────────────────────────┐                                        │
│  │ Escrow funded           │                                        │
│  │ Target met              │                                        │
│  └─────────────────────────┘                                        │
│           │                                                          │
│           ↓                                                          │
│  SETTLED PHASE                                                       │
│  ┌─────────────────────────┐                                        │
│  │ settle() called          │                                        │
│  │ Maturity reached         │                                        │
│  └─────────────────────────┘                                        │
│           │                                                          │
│           ├─→ Investor A claims directly:                           │
│           │   claim_investor_payout(&A)                             │
│           │                                                          │
│           ├─→ OR Multisig claims on behalf:                         │
│           │   claim_investor_payout_as_delegate(&A, &Multisig)     │
│           │                                                          │
│           ├─→ Investor revokes after settlement:                    │
│           │   revoke_yield_claim_delegate(&A)                       │
│           │   (blocks further delegate claims)                      │
│           │                                                          │
│           ↓                                                          │
│  CLAIMED PHASE                                                       │
│  ┌─────────────────────────┐                                        │
│  │ Payout recorded         │                                        │
│  │ InvestorClaimed = true  │ ← Same flag for both paths             │
│  └─────────────────────────┘                                        │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

---

## Delegation State Diagram

```
                     ┌─────────────────────┐
                     │   No Delegation     │
                     │  (Keys absent)      │
                     │                     │
                     │ Claim by: Investor  │
                     └────────┬────────────┘
                              │
                              │ set_yield_claim_delegate()
                              │
                              ↓
                     ┌─────────────────────────────────┐
                     │   Active Delegation             │
                     │ YieldClaimDelegate = Address X  │
                     │ YieldClaimRevoked = false       │
                     │                                 │
                     │ Claim by: Investor OR Delegate  │
                     └──────────┬──────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
        │ revoke_            │ set_                  │
        │ delegate()          │ delegate() (new)      │
        │                       │                       │
        ↓                       ↓                       ↓
     ┌──────────────┐   ┌─────────────────┐   ┌───────────────┐
     │   Revoked    │   │ Active (Updated)│   │ Same Address  │
     │  Delegation  │   │   (with Y)      │   │   (no-op)     │
     │ Revoked=true │   │   Revoked=false │   │               │
     │              │   │                 │   └───────────────┘
     │ Claim by:    │   │ Claim by:       │
     │ Investor     │   │ Investor OR Y   │
     │ only         │   │                 │
     └──────┬───────┘   └─────────────────┘
            │
            │ set_delegate() (reset)
            │
            ↓ (goes to Active with new address)
```

---

## Authorization Flows

### Flow 1: Set Delegation

```
Investor Calls: set_yield_claim_delegate(&investor, &delegate)
                         │
                         ↓
        ┌────────────────────────────────┐
        │  1. PRECONDITIONS (read-only)  │
        │  • delegate ≠ investor?        │
        │    → YES, continue             │
        │    → NO, error 165             │
        └────────────────────────────────┘
                         │
                         ↓
        ┌────────────────────────────────┐
        │  2. AUTHORIZATION              │
        │  • investor.require_auth()     │
        │    → Signature valid?          │
        │    → YES, continue             │
        │    → NO, panic                 │
        └────────────────────────────────┘
                         │
                         ↓
        ┌────────────────────────────────┐
        │  3. EFFECTS                    │
        │  • Set YieldClaimDelegate      │
        │  • Clear YieldClaimRevoked     │
        │  • Emit event                  │
        └────────────────────────────────┘
```

### Flow 2: Claim as Delegate

```
Delegate Calls: claim_investor_payout_as_delegate(&investor, &delegate)
                         │
                         ↓
        ┌────────────────────────────────┐
        │  1. PRECONDITIONS (read-only)  │
        │  • Legal hold? NO, continue    │
        │  • Contribution > 0? YES       │
        │  • Delegation exists? YES      │
        │  • Not revoked? YES            │
        │  • Right delegate? YES         │
        │  • Settled? YES (status=2)     │
        │  • Lock expired? YES           │
        │    → All pass, continue        │
        │    → Any fail, error (125-128) │
        └────────────────────────────────┘
                         │
                         ↓
        ┌────────────────────────────────┐
        │  2. AUTHORIZATION              │
        │  • delegate.require_auth()     │
        │    → Signature valid?          │
        │    → YES, continue             │
        │    → NO, panic                 │
        └────────────────────────────────┘
                         │
                         ↓
        ┌────────────────────────────────┐
        │  3. EFFECTS                    │
        │  • Set InvestorClaimed         │
        │  • Emit event                  │
        │  • (idempotent)                │
        └────────────────────────────────┘
```

---

## Use Case: Multisig-Backed Investor

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SCENARIO: Corporate Treasurer                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  STEP 1: Setup (Week 1)                                             │
│  ┌───────────────────────────────────────────────┐                 │
│  │ Treasurer Account (Signing Key)              │                 │
│  │ • Generate investor address: 0xTREAS_INV    │                 │
│  │ • Generate multisig address: 0xMULTI        │                 │
│  │                                              │                 │
│  │ Action: set_yield_claim_delegate()           │                 │
│  │   • investor = 0xTREAS_INV                   │                 │
│  │   • delegate = 0xMULTI                       │                 │
│  └───────────────────────────────────────────────┘                 │
│                         │                                           │
│  STEP 2: Funding (Week 2-4)                                        │
│  ┌───────────────────────────────────────────────┐                 │
│  │ • Treasury funds invoice: 0xTREAS_INV $50K   │                 │
│  │ • Delegation stored, active                  │                 │
│  │ • Treasurer account secured offline          │                 │
│  └───────────────────────────────────────────────┘                 │
│                         │                                           │
│  STEP 3: Settlement (Week 8)                                       │
│  ┌───────────────────────────────────────────────┐                 │
│  │ • SME receives principal: $50K               │                 │
│  │ • Yield accrued: $2.5K (5% annualized)      │                 │
│  │ • Escrow settled                             │                 │
│  └───────────────────────────────────────────────┘                 │
│                         │                                           │
│  STEP 4: Claim (Week 9)                                            │
│  ┌───────────────────────────────────────────────┐                 │
│  │ Multisig Wallet (Hot Key)                    │                 │
│  │ • Operations team has hot key                │                 │
│  │ • Calls: claim_investor_payout_as_delegate() │                 │
│  │   - investor = 0xTREAS_INV                   │                 │
│  │   - delegate = 0xMULTI                       │                 │
│  │ • Payout recorded: $52.5K principal + yield  │                 │
│  └───────────────────────────────────────────────┘                 │
│                         │                                           │
│  RESULT:                                                             │
│  ✅ Treasurer funds safely (cold key offline)                       │
│  ✅ Operations claims easily (hot key online)                       │
│  ✅ Custody separation maintained                                   │
│  ✅ Audit trail shows both addresses                                │
│                                                                       │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Error Decision Tree

```
                    claim_investor_payout_as_delegate()
                                 │
                                 ↓
                   ┌─────────────────────────┐
                   │ Legal hold active?      │
                   └────────┬────────────────┘
                            │
                    YES → Error 125 ✗
                            │
                            NO → ↓
                   ┌─────────────────────────┐
                   │ Investor has funds?     │
                   │ (contribution > 0)      │
                   └────────┬────────────────┘
                            │
                    NO → Error 126 ✗
                            │
                            YES → ↓
                   ┌─────────────────────────┐
                   │ Delegation set?         │
                   └────────┬────────────────┘
                            │
                    NO → Error 166 ✗
                            │
                            YES → ↓
                   ┌─────────────────────────┐
                   │ Not revoked?            │
                   └────────┬────────────────┘
                            │
                    YES (revoked) → Error 167 ✗
                            │
                            NO (not revoked) → ↓
                   ┌─────────────────────────┐
                   │ Right delegate?         │
                   │ (matches stored)        │
                   └────────┬────────────────┘
                            │
                    NO → Error 166 ✗
                            │
                            YES → ↓
                   ┌─────────────────────────┐
                   │ Escrow settled?         │
                   │ (status = 2)            │
                   └────────┬────────────────┘
                            │
                    NO → Error 127 ✗
                            │
                            YES → ↓
                   ┌─────────────────────────┐
                   │ Claim lock expired?     │
                   │ (now ≥ not_before)      │
                   └────────┬────────────────┘
                            │
                    NO → Error 128 ✗
                            │
                            YES → ↓
                   ┌─────────────────────────┐
                   │ ✅ SUCCESS              │
                   │ • Claim recorded        │
                   │ • Event emitted         │
                   │ • (idempotent)          │
                   └─────────────────────────┘
```

---

## Storage Layout

### Per-Investor Persistent Storage

```
Before Delegation (Initial State):
┌──────────────────────────────────────┐
│ Investor 0xALICE                     │
├──────────────────────────────────────┤
│ InvestorContribution: 5000           │
│ InvestorEffectiveYield: 500          │
│ InvestorClaimNotBefore: 1234567890   │
│ InvestorClaimed: false               │
│ [YieldClaimDelegate: absent]         │
│ [YieldClaimRevoked: absent]          │
└──────────────────────────────────────┘

After set_yield_claim_delegate(&ALICE, &MULTISIG):
┌──────────────────────────────────────┐
│ Investor 0xALICE                     │
├──────────────────────────────────────┤
│ InvestorContribution: 5000           │
│ InvestorEffectiveYield: 500          │
│ InvestorClaimNotBefore: 1234567890   │
│ InvestorClaimed: false               │
│ YieldClaimDelegate: 0xMULTISIG       │ ← NEW
│ YieldClaimRevoked: false             │ ← NEW
└──────────────────────────────────────┘

After revoke_yield_claim_delegate(&ALICE):
┌──────────────────────────────────────┐
│ Investor 0xALICE                     │
├──────────────────────────────────────┤
│ InvestorContribution: 5000           │
│ InvestorEffectiveYield: 500          │
│ InvestorClaimNotBefore: 1234567890   │
│ InvestorClaimed: false               │
│ YieldClaimDelegate: 0xMULTISIG       │ (preserved for audit)
│ YieldClaimRevoked: true              │ ← UPDATED
└──────────────────────────────────────┘

After claim_investor_payout_as_delegate(&ALICE, &MULTISIG):
┌──────────────────────────────────────┐
│ Investor 0xALICE                     │
├──────────────────────────────────────┤
│ InvestorContribution: 5000           │
│ InvestorEffectiveYield: 500          │
│ InvestorClaimNotBefore: 1234567890   │
│ InvestorClaimed: true                │ ← UPDATED
│ YieldClaimDelegate: 0xMULTISIG       │
│ YieldClaimRevoked: true              │
└──────────────────────────────────────┘
```

---

## Event Flow

```
Timeline of Events:

Week 1 (Fund Setup):
┌────────────────────────────────────┐
│ YieldClaimDelegationSet            │
│ • investor: 0xTREAS_INV            │
│ • delegate: 0xMULTI                │
│ • topics: [name, investor, delegate]
└────────────────────────────────────┘

Week 2-4 (Funding):
┌────────────────────────────────────┐
│ EscrowFunded (existing event)       │
│ • funded_amount: 50000              │
│ • investor_count: 42                │
└────────────────────────────────────┘

Week 8 (Settlement):
┌────────────────────────────────────┐
│ EscrowSettled (existing event)      │
│ • status: 2 (settled)               │
└────────────────────────────────────┘

Week 9 (Claim):
┌────────────────────────────────────┐
│ InvestorPayoutClaimed               │
│ • investor: 0xTREAS_INV             │ (same event)
│ • invoice_id: INV-001               │ (whether claimed by
│ • topics: [name, investor]          │  investor OR delegate)
└────────────────────────────────────┘
```

---

## Integration Checklist

### For Investor Setup

- [ ] Generate investor address (cold storage)
- [ ] Generate delegate address (multisig or hot wallet)
- [ ] Fund invoice with investor address
- [ ] Call `set_yield_claim_delegate(investor, delegate)`
- [ ] Verify with `get_yield_claim_delegate(investor)`

### For Settlement & Claim

- [ ] Wait for escrow to settle (maturity + SME settlement)
- [ ] Check `compute_investor_payout(investor)` to see payout amount
- [ ] Call `claim_investor_payout_as_delegate(investor, delegate)` from delegate address
- [ ] Verify with `is_investor_claimed(investor)` returns true

### For Revocation

- [ ] Call `revoke_yield_claim_delegate(investor)` from investor address
- [ ] Verify with `is_yield_claim_delegate_revoked(investor)` returns true
- [ ] Set new delegation or claim directly if needed

---

## Compatibility Matrix

| Feature | v6 Instances | v7 Instances | Interaction |
|---------|-------------|------------|------------|
| Direct claim | ✅ Works | ✅ Works | Unchanged |
| Delegation claim | ❌ N/A | ✅ Works | New feature |
| Tiered yield | ✅ Works | ✅ Works | Compatible |
| Legal hold | ✅ Works | ✅ Works | Blocks both |
| Maturity gates | ✅ Works | ✅ Works | Respected for both |

---

## Performance Impact

```
Storage (Persistent):
  Per investor with delegation: +2 keys (~64 bytes)
  Indexes 42 unique investors: +128 bytes
  Negligible impact on contract instance size

Gas (per delegation operation):
  set_yield_claim_delegate: ~2 storage writes
  revoke_yield_claim_delegate: ~1 storage write
  claim_investor_payout_as_delegate: ~1 storage write (same as direct claim)
  
TTL Extension:
  Delegation keys have same TTL as investor contributions
  No separate management required
```

---

## Recap: Three Key Operations

```
1️⃣  INVESTOR SETS DELEGATION
    set_yield_claim_delegate(&investor, &delegate)
    Effect: Now delegate can claim after settlement

2️⃣  DELEGATE CLAIMS PAYOUT
    claim_investor_payout_as_delegate(&investor, &delegate)
    Effect: Same as investor claim, just signed by delegate

3️⃣  INVESTOR REVOKES DELEGATION
    revoke_yield_claim_delegate(&investor)
    Effect: Delegate can no longer claim, investor can claim directly
```

---

**Ready to integrate! 🚀**
