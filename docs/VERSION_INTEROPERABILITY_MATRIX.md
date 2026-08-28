# Escrow Contract Version Interoperability Matrix

Compatibility reference for which contract versions can read, settle, and interact with each other.

---

## Quick Reference: Version Compatibility at a Glance

```
                    v1    v2    v3    v4    v5    v6
v1 reading v1       ✓     —     —     —     —     —
v2 reading v1       ✓     ✓     —     —     —     —
v2 reading v2       —     ✓     —     —     —     —
v3 reading v1-2     ✓     ✓     ✓     —     —     —
v3 reading v3       —     —     ✓     —     —     —
v4 reading v1-3     ✓     ✓     ✓     ✓     —     —
v4 reading v4       —     —     —     ✓     —     —
v5 reading v1-4     ✓     ✓     ✓     ✓     ✓     —
v5 reading v5       —     —     —     —     ✓     —
v6 reading v6       —     —     —     —     —     ✓
v6 reading v5       ⚠     ⚠     ⚠     ⚠     ✓     —
v6 reading v4       ✗     ✗     ✗     ✗     ✗     ✗
```

**Legend:**
- ✓ = Full compatibility (can read and interact)
- ⚠ = Partial compatibility (can read, limited functionality)
- ✗ = Incompatible (cannot read or interact)
- — = Not applicable (same version)

---

## Detailed Version Compatibility

### Version 1 → Version 2

**What changed:**
- Added: `InvestorEffectiveYield` (per-investor yield tracking)
- Added: `InvestorClaimNotBefore` (per-investor lock period)

**Storage changes:** Additive only (new `DataKey` variants)

**v1 → v2 compatibility:**
- ✓ v2 can read v1 escrows
- ✓ New keys default to `None` when missing (backwards compatible)
- ✓ No XDR layout changes to `InvoiceEscrow`
- ⚠ Yield calculations may differ if v1 didn't track per-investor yields
- ⚠ Claim locks not enforced on v1-created records (but checked before payout)

**Upgrade path:** Additive only — no `migrate()` call required

**Example:**
```rust
// v1 escrow data (stored)
InvoiceEscrow {
    admin: Address,
    amount: 1000000000,
    funded_amount: 500000000,
    // ... no per-investor yield keys
}

// v2 reads same escrow
// Missing keys read as: .get(...).unwrap_or(default)
// InvestorEffectiveYield not in storage → returns None
// Calculations proceed with default (base yield)
```

---

### Version 2 → Version 3

**What changed:**
- Added: `FundingCloseSnapshot` (record state at close)
- Added: `MinContributionFloor` (minimum per-investor contribution)
- Added: `MaxUniqueInvestorsCap` (investor cardinality limit)
- Added: `UniqueFunderCount` (tracking number of unique funders)

**Storage changes:** Additive only (new `DataKey` variants)

**v2 → v3 compatibility:**
- ✓ v3 can read v2 escrows
- ✓ New keys default to `None` / `0` when missing
- ✓ No changes to `InvoiceEscrow` struct layout
- ⚠ Snapshot missing on v2 escrows (but not required for payout)
- ⚠ Caps not enforced on v2-created funding records

**Upgrade path:** Additive only — no `migrate()` call required

---

### Version 3 → Version 4

**What changed:**
- Added: `PrimaryAttestationHash` (single-write digest binding)
- Added: `AttestationAppendLog` (bounded audit log)

**Storage changes:** Additive only (new `DataKey` variants)

**v3 → v4 compatibility:**
- ✓ v4 can read v3 escrows
- ✓ New attestation keys default to empty/None
- ✓ No changes to core `InvoiceEscrow` struct
- ✓ Attestation is advisory (not required for settlement/claims)

**Upgrade path:** Additive only — no `migrate()` call required

---

### Version 4 → Version 5

**What changed:**
- Added: `YieldTierTable` (tiered yield schedule, first-deposit discipline)
- Added: `RegistryRef` (optional reference to escrow registry)
- Added: `Treasury` (explicit treasury account storage)
- **BREAKING:** `InvoiceEscrow` struct layout tightened

**Storage changes:** Additive keys + struct layout change

**v4 → v5 compatibility:**
- ⚠ v5 can read v4 data only if `InvoiceEscrow` layout didn't change
- ✗ If struct layout changed: REDEPLOY REQUIRED
- ✓ New keys (`YieldTierTable`, `RegistryRef`, `Treasury`) default to None/empty
- ✗ v4 cannot read v5 escrows (tier table logic incompatible)

**Upgrade path:**
- **If `InvoiceEscrow` layout identical:** Additive upgrade (no redeploy)
- **If `InvoiceEscrow` layout changed:** REDEPLOY REQUIRED (same-address upgrade not possible)

**Migration requirement:** Check struct layout in git diff before deciding

```rust
// v4 InvoiceEscrow
pub struct InvoiceEscrow {
    pub admin: Address,
    pub amount: i128,
    pub funded_amount: i128,
    // ... existing fields
}

// v5 InvoiceEscrow (if layout changed)
pub struct InvoiceEscrow {
    pub admin: Address,
    pub amount: i128,
    pub funded_amount: i128,
    pub yield_tier_selected: Option<u32>,  // NEW
    // ... existing fields reorganized?
}
// → REDEPLOY REQUIRED if layout differs
```

---

### Version 5 → Version 6

**What changed:**
- **BREAKING:** Per-investor keys moved to persistent storage
- Reason: Decouple per-address TTL from instance footprint (unbounded growth risk)

**Storage changes:** Struct layout fundamentally different

**v5 → v6 compatibility:**
- ✗ v6 cannot read v5 per-investor keys (different storage location)
- ✗ v5 cannot read v6 per-investor keys (new persistent storage location)
- ✗ No in-place upgrade or `migrate()` path
- ✗ **REDEPLOY REQUIRED**

**Why redeploy is mandatory:**
v5 escrows store per-investor contributions in instance-scoped keys:
```
DataKey::InvestorContribution(Address) → stored in instance
```

v6 moves to persistent storage with unbounded lifetime:
```
DataKey::InvestorContribution(Address) → stored persistently (different location)
```

The contract cannot enumerate and rewrite all investor keys (no bulk enumeration on Soroban). Therefore:
- Old instances must be archived (legal hold)
- New instances deployed from same WASM
- Investor data re-recorded post-init (or restored from indexer)

**Upgrade path:** REDEPLOY REQUIRED (mandatory)

**Migration procedure:**
1. Deploy new v6 instance
2. Call `init()` with same parameters as v5 instance
3. Restore investor contributions via `fund()` or `fund_batch()`
4. Archive v5 instance (legal hold, redirect pointers)

---

## Cross-Version Interaction Scenarios

### Scenario 1: Investor funding v1 → v2 escrow

**Setup:**
- Escrow created as v1, now running v2 WASM
- Investor submits funding

**Flow:**
1. Investor calls `fund()` on v2 WASM
2. v2 reads v1 escrow data
3. Missing `InvestorEffectiveYield` keys default to base yield
4. Funding recorded; escrow marked funded if target met
5. ✓ Works (per-investor yield not tracked, but doesn't break settlement)

**Result:** ✓ Compatible

---

### Scenario 2: Settlement v1 → v3 escrow

**Setup:**
- Escrow created as v1
- Multiple upgrades: v1 → v2 → v3
- Now at v3 WASM; escrow marked for settlement

**Flow:**
1. SME calls `settle()` on v3 WASM
2. v3 reads v1 escrow (missing snapshot, caps, yields)
3. Funding close snapshot is missing (not required for settlement)
4. No investor cap enforced (wasn't tracked in v1)
5. Settlement proceeds with available state
6. ✓ Works (missing optional state doesn't block settlement)

**Result:** ✓ Compatible

---

### Scenario 3: Investor claim v5 → v6

**Setup:**
- Escrow created as v5
- Operator deployed v6 to same instance (BREAKING change)
- Investor tries to claim payout

**Flow:**
1. Investor calls `claim_investor_payout()` on v6 WASM
2. v6 looks for investor record in persistent storage
3. Record missing (stored in v5 instance storage, not persistent)
4. ✗ Claim fails: investor not found
5. ✗ No fallback; no migration path

**Result:** ✗ **INCOMPATIBLE — This is why v5→v6 requires redeploy**

**Workaround:** Restore investor records post-redeploy before allowing claims

---

### Scenario 4: SME collateral record v4 → v5

**Setup:**
- Escrow created as v4
- SME calls `record_sme_collateral_commitment()` on v4
- Operator upgrades to v5
- SME tries to query collateral record

**Flow:**
1. v4 stores collateral in `DataKey::SmeCollateralCommitment` (XDR shape v4)
2. v5 reads same key (XDR shape v5)
3. If struct layout unchanged: ✓ Reads successfully
4. If struct layout changed: ✗ XDR decode fails
5. If v5 added new required fields: ✗ Decode fails

**Result:** ⚠ **Depends on struct layout changes**

---

### Scenario 5: Indexer migration v5 → v6

**Setup:**
- Indexer tracking v5 escrows
- Operator deploys v6 to new instances
- Indexer needs to track both v5 and v6

**Flow:**
1. Indexer queries v5 instances:
   - Calls `get_escrow()` ✓ Works
   - Calls `get_investor_yield()` ✓ Works (instance keys)

2. Indexer queries v6 instances:
   - Calls `get_escrow()` ✓ Works
   - Calls `get_investor_yield()` ✓ Works (persistent keys)

3. ✓ Indexer can track both

**Result:** ✓ **Compatible (indexer is read-only)**

---

## Version-specific constraints

### v1 constraints
- No per-investor yield tracking
- No investor caps
- No attestations
- Simple funding/settlement only

### v2 constraints
- Per-investor yields not enforced as locks
- No funding close snapshot
- No investor caps enforced

### v3 constraints
- Snapshot not retroactively applied to v2 escrows
- Caps not enforced on v2-created funding records

### v4 constraints
- Attestations are advisory (not enforced for settlement)
- Tier yields not available

### v5 constraints
- Instance-scoped per-investor keys (bounded TTL)
- Cannot scale beyond ~1000 unique investors per instance without TTL churn
- Tier yields select on first deposit only

### v6 constraints
- Per-investor keys in persistent storage
- Requires redeploy from v5 (no in-place upgrade path)
- Investor records must be re-recorded after redeploy

---

## When to redeploy vs. upgrade

### Redeploy required when:

✗ `InvoiceEscrow` struct layout changes (XDR shape)
✗ Stored `#[contracttype]` struct layout changes
✗ Storage key location fundamentally changes (v5→v6)
✗ Existing stored data cannot be read by new WASM

**Example decisions:**
```rust
// v4 → v5: Adding new field to InvoiceEscrow
pub struct InvoiceEscrow {
    // ... existing fields ...
    pub yield_tier: Option<u32>,  // NEW FIELD
}
// → REDEPLOY (new instances can't decode old XDR)

// v1 → v2: Only new DataKey variants
// → ADDITIVE UPGRADE (old data still readable)
```

### Additive upgrade safe when:

✓ Only new `DataKey` variants added
✓ No changes to existing stored struct layouts
✓ Old data readable with `.get(...).unwrap_or(default)`
✓ New fields optional / non-blocking

**Example:**
```rust
// v3 → v4: Only new keys
pub enum DataKey {
    // ... existing variants ...
    PrimaryAttestationHash,        // NEW
    AttestationAppendLog,          // NEW
}
// → ADDITIVE UPGRADE (old escrows still readable)
```

---

## Compatibility decision tree

```
START: You want to read an escrow at version N with WASM version M

1. Is N == M?
   ├─ YES → COMPATIBLE ✓
   └─ NO → Continue

2. Is N < M?
   ├─ YES (upgrading from old WASM)
   │  │
   │  3. Has InvoiceEscrow struct layout changed?
   │     ├─ NO (only new keys) → COMPATIBLE (additive upgrade) ✓
   │     └─ YES → INCOMPATIBLE (redeploy happened) ✗
   │
   └─ NO (M < N, downgrading)
      └─ → INCOMPATIBLE (cannot downgrade WASM) ✗

3. Is M == N-1?
   ├─ YES (single version upgrade)
   │  └─ Likely COMPATIBLE (unless breaking change documented)
   └─ NO (multi-version jump)
      └─ Check each transitional step
```

---

## Version changelog and breaking changes

| Version | Release | Key changes | Breaking? |
|---------|---------|-------------|-----------|
| 1 | v1.0 | Initial schema (fund/settle/claim) | N/A |
| 2 | v1.1 | Per-investor yields, claim locks | No (additive) |
| 3 | v1.2 | Funding snapshot, caps, attestations | No (additive) |
| 4 | v1.3 | Attestation API extended | No (additive) |
| 5 | v2.0 | Tiered yields, yield tiers table | **Yes if struct changed** |
| 6 | v2.1 | Per-investor keys → persistent storage | **Yes (redeploy required)** |

---

## Storage layout reference

### v1-4: Instance-scoped storage

```
DataKey::Escrow → InvoiceEscrow (v1-4 layout)
DataKey::Version → 1, 2, 3, or 4
DataKey::InvestorContribution(Address) → amount
DataKey::InvestorEffectiveYield(Address) → yield (v2+)
DataKey::InvestorClaimNotBefore(Address) → timestamp (v2+)
// All keys instance-scoped (TTL = instance TTL)
```

**Pros:** Simple, predictable
**Cons:** Unbounded growth; per-address TTL risk

### v5: Instance-scoped with tiers

```
DataKey::Escrow → InvoiceEscrow (v5 layout)
DataKey::Version → 5
DataKey::YieldTierTable → [Tier1, Tier2, Tier3] (NEW in v5)
DataKey::InvestorContribution(Address) → amount
DataKey::InvestorYieldTier(Address) → selected tier
// Same storage location as v1-4
```

**Change:** Only new keys; same storage location
**Compatibility:** Additive (v4 data readable by v5)

### v6: Persistent storage for investors

```
DataKey::Escrow → InvoiceEscrow (v6 layout)
DataKey::Version → 6
DataKey::YieldTierTable → [Tier1, Tier2, Tier3]
DataKey::InvestorContribution(Address) → amount (PERSISTENT, not instance)
DataKey::InvestorYieldTier(Address) → selected tier (PERSISTENT)
// New storage location; cannot enumerate from v5
```

**Change:** Storage location fundamentally different
**Compatibility:** ✗ Incompatible (redeploy required)

---

## Interoperability during canary deployments

### Scenario: Canary v6, production v5

**Setup:**
- Canary instances: v6 (is_canary=true)
- Production instances: v5 (is_canary=false)
- Shared indexer reading both

**Indexer queries:**
```bash
# Query v5 instance
stellar contract invoke --id PROD_V5 -- get_escrow
# → Returns v5 layout ✓

# Query v6 instance
stellar contract invoke --id CANARY_V6 -- get_escrow
# → Returns v6 layout (compatible return type) ✓

# Investor query on v5
stellar contract invoke --id PROD_V5 -- get_investor_yield --investor G...
# → Reads from instance storage ✓

# Investor query on v6
stellar contract invoke --id CANARY_V6 -- get_investor_yield --investor G...
# → Reads from persistent storage ✓
```

**Result:** ✓ **Canary and production can coexist** (read-only from indexer perspective)

**But:** v5 escrows cannot be migrated to v6 in-place; must redeploy

---

## Settlement across versions

### Can v5 settle an escrow created by v1?

**Flow:**
1. Escrow created by v1 (minimal fields)
2. Upgraded: v1 → v2 → v3 → v4 → v5
3. SME calls `settle()` on v5 WASM

**Questions:**
- ✓ Can v5 read v1 escrow? YES (all fields present, just fewer optional keys)
- ✓ Is settlement logic same? YES (core logic unchanged)
- ✓ Are yields calculated? YES (v5 reads base yield)
- ✓ Is snapshot applied? YES (v5 creates snapshot)
- ✓ Are caps checked? NO (v1 has no caps, but v5 respects None/0 defaults)

**Result:** ✓ **Settlement works across all versions** (if escrow is funded)

---

## Investor claims across versions

### Can investor claim from v3 escrow on v6 WASM?

**Pre-redeploy (same instance, upgraded WASM):**
- v1 escrow → v6 WASM: ✓ Works (investor data in instance storage)

**Post-redeploy (new instance, v6 WASM):**
- v1 escrow → v6 instance: ✗ Fails (investor data not restored)
  - Workaround: Re-fund investor before claim

**Result:** ⚠ **Depends on redeploy** (investors must be restored post-redeploy)

---

## Backwards compatibility guarantees

### What karis-ky commits to:

- ✓ New versions can always read old escrow state (when using additive upgrades)
- ✓ Settlement logic evolves but remains compatible with old data
- ✓ New optional keys default safely (no breaking changes to read paths)
- ✗ Struct layout changes require redeploy (documented and planned)
- ✗ Storage location changes require redeploy (rare; v5→v6 only)

### What operators must track:

- [ ] Version of each live instance
- [ ] Whether latest WASM is additive or breaking
- [ ] If breaking: redeploy plan and investor notification
- [ ] Indexer compatibility during canary deployments

---

## Version upgrade examples

### Example 1: v1 → v6 (full path)

```
v1 instance (created 2024-01-01)
  ├─ Upgrade to v2 (2024-02-01) — Additive ✓
  ├─ Upgrade to v3 (2024-03-01) — Additive ✓
  ├─ Upgrade to v4 (2024-04-01) — Additive ✓
  ├─ Upgrade to v5 (2024-05-01) — Additive ✓
  └─ Upgrade to v6 (2024-06-01) — BREAKING ✗
      └─ Action: Redeploy to new instance (NEW_V6_ID)
         1. Deploy new v6 instance
         2. Init with same params as v1
         3. Restore investor records from v1
         4. Retire v1 (legal hold + archive)
         5. Redirect pointers to NEW_V6_ID
```

### Example 2: Canary v6 before full rollout

```
Canary escrows (2 instances, is_canary=true)
  ├─ Deploy v6 (2024-06-01, canary)
  ├─ Monitor 72 hours ✓ Success
  ├─ Governance approves Stage 2

Production escrows (50 instances, is_canary=false)
  ├─ Still on v5
  ├─ After governance approval:
  │  ├─ Evaluate: v5 → v6 is breaking
  │  ├─ Plan: Redeploy all 50 instances
  │  ├─ Execute: Deploy + init + restore (batched)
  │  └─ Complete: All on v6
  └─ Retire all v5 instances (legal hold + archive)
```

---

## Appendix: Full compatibility matrix

| From \ To | v1 | v2 | v3 | v4 | v5 | v6 |
|-----------|----|----|----|----|----|----|
| **v1** | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **v2** | ✗ | ✓ | ✓ | ✓ | ✓ | ✗ |
| **v3** | ✗ | ✗ | ✓ | ✓ | ✓ | ✗ |
| **v4** | ✗ | ✗ | ✗ | ✓ | ✓ | ✗ |
| **v5** | ✗ | ✗ | ✗ | ✗ | ✓ | ⚠ |
| **v6** | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |

**Key:**
- ✓ = Forward compatible (can upgrade)
- ⚠ = Redeploy required (same WASM, new instance)
- ✗ = Cannot upgrade (downgrade or different version required)

---

## References

- **OPERATOR_RUNBOOK.md** — Redeploy vs. upgrade decision tree
- **README.md** — Schema version changelog (source of truth)
- **escrow-error-messages.md** — Typed errors during version mismatches
- **ADR-007** — Storage key evolution policy
