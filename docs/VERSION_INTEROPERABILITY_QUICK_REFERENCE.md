# Version Interoperability Quick Reference

One-page summary for version compatibility decisions.

---

## Can version X read/settle/claim on escrow created by version Y?

### Read operations (get_escrow, get_investor_yield, etc.)

```
v1 WASM reading escrows:   v1 only ✓
v2 WASM reading escrows:   v1, v2 ✓
v3 WASM reading escrows:   v1, v2, v3 ✓
v4 WASM reading escrows:   v1, v2, v3, v4 ✓
v5 WASM reading escrows:   v1, v2, v3, v4, v5 ✓
v6 WASM reading escrows:   v6 only ✗ (v5 instance storage incompatible)
                           v5 with redeploy ✓ (after data restoration)
```

**Rule:** New WASM can read old escrows UNLESS storage location changed (v5→v6)

---

### Settlement operations (settle, withdraw, claim_investor_payout)

```
v1 WASM settling:    v1 escrows ✓
v2 WASM settling:    v1, v2 escrows ✓
v3 WASM settling:    v1, v2, v3 escrows ✓
v4 WASM settling:    v1, v2, v3, v4 escrows ✓
v5 WASM settling:    v1, v2, v3, v4, v5 escrows ✓
v6 WASM settling:    v6 escrows only ✓
                     v5 escrows ✗ (investor data lost)
```

**Rule:** Settlement logic compatible across versions UNLESS investor data location changes (v5→v6)

---

### Funding operations (fund, fund_batch)

```
v1 WASM funding:     v1 escrows ✓
v2 WASM funding:     v1, v2 escrows ✓
v3 WASM funding:     v1, v2, v3 escrows ✓
v4 WASM funding:     v1, v2, v3, v4 escrows ✓
v5 WASM funding:     v1, v2, v3, v4, v5 escrows ✓
v6 WASM funding:     v6 escrows only ✓
                     v5 escrows ✗ (cannot restore investor records from funding alone)
```

---

## Upgrade decision matrix

### Question: Should I upgrade WASM in place or redeploy?

| Scenario | Decision | Reason |
|----------|----------|--------|
| v1 → v2 | Upgrade | Additive (new keys only) |
| v2 → v3 | Upgrade | Additive (new keys only) |
| v3 → v4 | Upgrade | Additive (new keys only) |
| v4 → v5 | Check struct | If `InvoiceEscrow` layout changed: Redeploy |
| v5 → v6 | Redeploy | Storage location change (persistent vs. instance) |
| v6 → v7+ | Check struct | Depends on what changed |

**Action:** Before upgrade, `git diff HEAD~1 escrow/src/lib.rs | grep -A10 '#\[contracttype\]'`

---

## Version feature matrix

| Feature | v1 | v2 | v3 | v4 | v5 | v6 |
|---------|----|----|----|----|----|----|
| Basic fund/settle/claim | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Per-investor yields | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Claim locks (timestamps) | ✗ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Funding close snapshot | ✗ | ✗ | ✓ | ✓ | ✓ | ✓ |
| Min contribution floor | ✗ | ✗ | ✓ | ✓ | ✓ | ✓ |
| Max investor cap | ✗ | ✗ | ✓ | ✓ | ✓ | ✓ |
| Attestation API | ✗ | ✗ | ✗ | ✓ | ✓ | ✓ |
| Tiered yields | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ |
| Persistent storage (investors) | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |

---

## Troubleshooting: Version mismatch issues

### "get_escrow panics with XDR error"

**Diagnosis:** WASM version incompatible with stored data

**Typical cause:** Tried to upgrade when struct layout changed

**Solution:** Redeploy (new instance required)

```bash
# Check: is this a redeploy situation?
git diff HEAD~1 escrow/src/lib.rs | grep -A10 'pub struct InvoiceEscrow'
# If struct layout differs → You needed to redeploy, not upgrade
```

---

### "Investor record not found, claim fails"

**Diagnosis:** v5→v6 redeploy without restoring investor data

**Typical cause:** Deployed v6 to new instance, forgot to re-record funding

**Solution:** Restore investor records before investor claims

```bash
# Check v5 instance for funding records
stellar contract invoke --id $V5_INSTANCE -- get_escrow

# Re-record in v6 instance (before investor claims)
stellar contract invoke \
  --id $V6_INSTANCE \
  -- fund_batch \
  --entries '[{"investor":"G...","amount":"1000000"}]'
```

---

### "New version is slower, performance degraded"

**Diagnosis:** New code path not optimized

**Typical cause:** Not a version incompatibility; a performance regression

**Solution:** Profile and optimize, re-test on testnet, re-canary

---

### "Indexer fails to query both v5 and v6 instances"

**Diagnosis:** Indexer code assumes all instances have same schema

**Typical cause:** Query for persistent storage keys fails on v5 instances

**Solution:** Indexer must track version and query appropriately

```javascript
// Indexer pseudo-code
if (escrowVersion === 6) {
  // Query persistent storage for investor data
  const yield = await queryPersistentKey(investorYieldKey);
} else if (escrowVersion === 5) {
  // Query instance storage for investor data
  const yield = await queryInstanceKey(investorYieldKey);
}
```

---

## Version compatibility at a glance (compact)

### You're running v5 WASM. What escrows can you read?

**All escrows created by v1, v2, v3, v4, v5** ✓

Read any old escrow state, settle it, pay out investors — works fine.

Exception: If escrow was created as v5 and later redeployed as v6, you cannot interact with the v6 version.

---

### You're running v6 WASM. What escrows can you read?

**Only v6 escrows** ✓

v5 and earlier escrows: ✗ (investor data in different storage location)

**Workaround:** If v5 escrow was redeployed as v6, restore investor records post-redeploy.

---

### You deployed v6 to canary instances. What happens?

**Canary (v6) works independently** ✓
**Production (v5) continues running** ✓
**Both can be read by indexer** ✓ (need version-aware queries)
**Investor migration:** Only happens when v5 escrows are redeployed to v6

---

## Decision tree: Should I upgrade this escrow?

```
Escrow version: N
New WASM version: M

  1. Is M == N?
     └─ Already on latest ✓

  2. Is M < N?
     └─ Cannot downgrade ✗

  3. Is M > N?
     ├─ 3a. Did InvoiceEscrow struct layout change?
     │     ├─ YES → Redeploy (new instance) ✗
     │     └─ NO → Additive upgrade ✓
     │
     └─ 3b. Did storage location change (e.g., v5→v6)?
           ├─ YES → Redeploy (new instance) ✗
           └─ NO → Additive upgrade ✓
```

---

## Canary + production version mix

### Is it safe to have v5 production and v6 canary simultaneously?

**For read operations:** ✓ Yes (indexer reads both, needs version awareness)

**For settlement:** ✓ Yes (different instances, isolated)

**For investors:** ⚠ Not fully safe — investor data not transferable
- Investor funded on v5 escrow (instance storage)
- Cannot claim from v6 escrow (persistent storage)
- Must wait for full v6 rollout (redeploy all v5 escrows)

---

## Migration checklist by version jump

### v1 → v2 (or any additive upgrade)
- [ ] Test on testnet
- [ ] Upgrade WASM in place
- [ ] Verify `get_version` returns v2
- [ ] Verify old escrows still readable

### v4 → v5 (conditional redeploy)
- [ ] Check: Did `InvoiceEscrow` struct layout change?
- [ ] If NO: Additive upgrade (follow v1→v2 path above)
- [ ] If YES: Redeploy (follow v5→v6 path below)

### v5 → v6 (mandatory redeploy)
- [ ] Create new v6 instances (matching v5 invoice IDs)
- [ ] Init each v6 instance with same parameters as v5
- [ ] Restore investor records (fund_batch with pre-redeploy data)
- [ ] Verify escrow state matches v5 (funded_amount, etc.)
- [ ] Retire v5 instances (legal hold + archive)
- [ ] Update all external pointers (indexer, API, UI)
- [ ] Notify investors (new contract ID)

---

## Version at deployment: how to know what you have?

```bash
# Check WASM version (read from contract storage)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SECRET \
  --network mainnet \
  -- get_version

# Returns: 1, 2, 3, 4, 5, or 6

# Check WASM binary version (git tag or build artifact)
git describe --tags  # → v2.0.0 or similar

# Reconcile: Deployed WASM should match schema version in storage
```

---

## FAQ: Version compatibility

**Q: Can I upgrade v1 directly to v6?**
A: Only if all intermediate versions (v2-v5) are additive upgrades. v5→v6 is breaking, so you'll hit it at that point. Workaround: Do staged upgrades, or redeploy at v6.

**Q: Can v6 WASM read v4 escrow data?**
A: No. v5→v6 is the only breaking change; v6 has no compatibility path for v5 or older.

**Q: If I redeploy v5→v6, can I access the old v5 escrow?**
A: No. The old v5 instance is archived (legal hold). Investors must use the new v6 instance. Their contributions must be re-recorded.

**Q: How long can an escrow stay on v1?**
A: As long as you want. The system will keep upgrading it (v1→v2→v3→v4→v5), all additive, until v5→v6 where you must redeploy.

**Q: Do investors need to do anything during version upgrades?**
A: Usually no (read-only). Exception: v5→v6 redeploy — they may need to re-fund or use new contract ID for claims.

---

## Related docs

- **MULTI_INSTANCE_UPGRADE_GUIDE.md** — How to perform upgrades/redeployments
- **OPERATOR_RUNBOOK.md** — Redeploy vs. upgrade decision tree
- **README.md** — Schema version changelog (source of truth)
- **UPGRADE_DECISION_TREES.md** — Troubleshooting version issues
