# ✅ Off-Chain Storage with Merkle Proofs — Complete Design

**Date:** July 27, 2026  
**Status:** RFC-004 DRAFT + Technical Design Complete

---

## Problem & Solution

### Problem
Large escrows with 10,000+ investors hit storage constraints:
- Each investor record: 50 bytes persistent storage
- 100,000 investors: 5 MB on-chain (hits Soroban limits)
- TTL rent: scales with investor count (100,000 TTL keys to extend)
- Operational cost: makes scaling uneconomical

### Solution
**Off-chain storage with on-chain Merkle root verification:**
- Store investor ledger off-chain (IPFS, S3, Arweave)
- Store only Merkle root on-chain (32 bytes)
- Verify claims via Merkle proofs (O(log n) bytes ≈ 544 B for 100K investors)

**Storage reduction:** 5 MB → 300 bytes (**16,667× smaller**)

---

## Deliverables

### 1. RFC-004: Off-Chain Storage with Merkle Proofs (663 lines)
- **Status:** DRAFT
- **Path:** `docs/rfc/RFC-004-off-chain-storage-merkle-proofs.md`
- **Sections:**
  - Problem statement + motivation
  - High-level design approach
  - Detailed storage schema (v6 → v7)
  - Entrypoint changes (init, claim_investor_payout)
  - Merkle verification logic
  - 4 alternatives considered (all with rationale)
  - Effort estimate: 14–22 days (3–4 week sprint)
  - Acceptance criteria (15 items)
  - Security considerations + threat model
  - Rollout plan (3 phases: testnet, partner testing, mainnet)

### 2. Technical Design Document (639 lines)
- **Path:** `docs/escrow-off-chain-storage.md`
- **Sections:**
  - Architecture overview (DRAFT vs MERKLE modes)
  - Data model (on-chain + off-chain)
  - Storage comparison table (16,667× reduction for 100K investors)
  - Merkle tree structure + verification algorithm
  - On-chain integration (init, claiming, merkle verification)
  - Off-chain tooling (Python reference implementation)
  - Performance analysis (storage, computation, latency)
  - Migration path (no forced migration)
  - Error codes (50–54)
  - Security model + threat analysis
  - IPFS workflow example (end-to-end)

---

## Key Design Decisions

### 1. Storage Modes (Backward Compatible)

**DRAFT Mode (current, default):**
- Per-investor persistent keys (one per investor)
- Practical limit: ~1,000–2,000 investors
- Current deployments continue unchanged

**MERKLE Mode (new, opt-in):**
- Merkle root + off-chain ledger
- Practical limit: ~1M investors
- Chosen at init time (immutable thereafter)

### 2. Off-Chain Storage (Decentralized)

- **IPFS:** Immutable, content-addressed, decentralized (primary)
- **S3/Arweave:** Backup/redundancy options
- **Format:** JSONL (line-delimited JSON) for streaming/partial reads
- **Size:** ~150 B per investor record

### 3. Merkle Tree (Binary, Balanced)

- **Hash function:** SHA-256 (with keccak256 for leaf encoding)
- **Proof size:** O(log n) ≈ 544 bytes for 100K investors
- **Verification:** O(log n) on-chain hash operations
- **Root:** 32 bytes (immutable after init)

### 4. Data Model

```
On-Chain (Instance Storage):
├─ InvoiceEscrow (~300 B)
├─ InvestorStorageMode: "DRAFT" | "MERKLE"
├─ InvestorDataMerkleRoot: BytesN<32> (if MERKLE)
├─ InvestorDataSource: String (if MERKLE)
└─ MerkleRootUpdatedAt: u64 (if MERKLE)

Off-Chain (IPFS):
└─ ledger.jsonl (~15 MB for 100K)
   ├─ Immutable
   ├─ Content-addressed (CID = SHA-256)
   └─ Sorted by investor address
```

---

## Implementation Strategy

### Phase 1: On-Chain (v2.1)

- [ ] Add 4 new DataKey variants
- [ ] Merkle proof verification logic
- [ ] Conditional routing in `claim_investor_payout()`
- [ ] Init with merkle parameters
- [ ] Error codes 50–54
- [ ] Effort: ~3 weeks

### Phase 2: Off-Chain Tooling

- [ ] Merkle tree builder (Python/JS)
- [ ] IPFS integration
- [ ] Proof generation/verification (reference implementation)
- [ ] End-to-end test (ledger → tree → proof → verify)
- [ ] Effort: ~1 week

### Phase 3: Integration Guide

- [ ] Integrator documentation
- [ ] Example workflows (SDK clients)
- [ ] Security best practices
- [ ] FAQ + troubleshooting

---

## Performance Impact

### Storage

| Metric | DRAFT (100K) | MERKLE (100K) | Reduction |
|--------|--------------|---------------|-----------|
| On-chain | 5 MB | 300 B | **16,667×** |
| Off-chain | — | 15 MB | — |
| TTL keys | 100K | 1 | **100,000×** |
| TTL extension cost | 50 calls | 1 call | **50×** |

### Computation

| Operation | Time |
|-----------|------|
| Generate 100K ledger | ~100 ms (off-chain) |
| Compute merkle tree | ~500 ms (off-chain) |
| Verify proof on-chain | ~10 ms (17 hash ops) |
| Upload to IPFS | ~2–5 sec (network) |

### Scalability

| Investor Count | DRAFT | MERKLE |
|---|---|---|
| 1K | ✓ (50 KB) | ✓ (320 B proof) |
| 10K | ✓ (500 KB) | ✓ (448 B proof) |
| 100K | ✗ (5 MB, hits limits) | ✓ (544 B proof) |
| 1M | ✗ | ✓ (640 B proof) |

---

## Backward Compatibility

### ✅ No Breaking Changes

1. **Existing deployments unaffected**
   - DRAFT mode is default
   - v1.x escrows continue working
   - No migration required

2. **Gradual adoption**
   - New escrows can opt into MERKLE mode
   - Both modes can coexist
   - Network supports both simultaneously

3. **Data preservation**
   - DRAFT mode data remains on-chain
   - No re-indexing needed
   - Existing investor claims work unchanged

---

## Security Model

### Threat Analysis

| Threat | Mitigation | Cost to Break |
|--------|-----------|---|
| Forge merkle proof | Root is cryptographically bound | 2^128 hash collisions |
| Modify off-chain ledger | IPFS content-addresses by hash | Recompute all 100K records |
| Lose off-chain ledger | IPFS replication + S3 backup | Permanent data loss (unrecoverable) |
| Admin updates root maliciously | Design feature (integrators verify) | Requires admin key compromise |

### Assumptions

✓ **Hash functions:** keccak256 + sha256 are collision-resistant  
✓ **Off-chain availability:** IPFS ~99% uptime (reasonable for immutable data)  
✓ **Admin integrity:** Admin maintains accurate off-chain ledger  
✓ **Clock accuracy:** Ledger timestamps tied to Soroban ledger time  

---

## Use Cases Enabled

### Before (DRAFT Mode — Limited to 2K investors)
```
Escrow:
├─ Invoice: $50K
├─ Investors: 1,500
├─ Storage: 75 KB
└─ TTL extensions: 1,500 keys
```

### After (MERKLE Mode — Supports 100K+ investors)
```
Escrow:
├─ Invoice: $1M
├─ Investors: 50,000
├─ Storage: 300 B on-chain, 7.5 MB off-chain (IPFS)
└─ TTL extensions: 1 root key
```

### New Possibilities

1. **Large invoice pools** — 50K+ investors in single escrow
2. **Institutional investors** — Single address with 1000+ transactions (efficient claiming)
3. **Portfolio analytics** — Query across 100 invoices without O(n) calls
4. **Compliance audit** — Replay investor history from merkle proofs + ledger

---

## Timeline

| Phase | Duration | Status |
|-------|----------|--------|
| RFC Discussion | 1 week | DRAFT (starting 2026-07-27) |
| Design Review | 1 week | Design complete ✓ |
| On-Chain Implementation | 3 weeks | Not started |
| Off-Chain Tooling | 1 week | Not started |
| Testing + Integration | 1 week | Not started |
| Audit + Hardening | 2 weeks | Not started |
| **Total to Mainnet** | **~10 weeks** | Target: Q2 2027 (v2.1) |

---

## Open Questions for RFC Discussion

1. **Hash function:** Use SHA-256 for merkle, or Stellar's native hash?
2. **Off-chain storage:** IPFS primary, or S3 + IPFS backup?
3. **Proof format:** Store bit flags in proof, or external index?
4. **Migration:** Support DRAFT → MERKLE migration in v2.2?
5. **TTL:** What happens if IPFS ledger expires (data loss)?

---

## Next Steps

1. **RFC Discussion Phase** (starting 2026-07-27)
   - Solicit feedback from platform team (3–5 reviewers)
   - Address questions + iterate on design
   - Target approval: 2026-08-10

2. **Implementation Planning** (if approved)
   - Break down into PRs (on-chain + off-chain)
   - Assign developers + integrate with v2.1 roadmap
   - Set up off-chain tooling repo

3. **Testnet Deployment** (Q1 2027)
   - Deploy with synthetic 100K investor escrow
   - Validate proof generation + verification
   - Collect partner feedback

4. **Mainnet Launch** (Q2 2027, v2.1)
   - Release to production
   - Publish integrator guide
   - Support first large-scale escrows

---

## Files Generated

| File | Lines | Purpose |
|------|-------|---------|
| `docs/rfc/RFC-004-off-chain-storage-merkle-proofs.md` | 663 | RFC proposal (DRAFT) |
| `docs/escrow-off-chain-storage.md` | 639 | Technical design document |
| **Total** | **1,302** | Complete design |

---

## Summary

✅ **RFC-004 DRAFT** — Off-chain storage with Merkle proof verification  
✅ **Technical design** — Complete architecture (on-chain + off-chain)  
✅ **Performance analysis** — 16,667× storage reduction for 100K investors  
✅ **Backward compatibility** — No breaking changes, opt-in adoption  
✅ **Security model** — Threat analysis + mitigation strategies  
✅ **Implementation roadmap** — 3-phase deployment plan  

**Next:** RFC discussion (3–5 reviewers, 1 week), then approval for v2.1 implementation.

