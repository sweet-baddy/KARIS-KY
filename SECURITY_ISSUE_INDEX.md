# Security Issue: Migrate Replay Protection — Complete Documentation

**Date Created:** Wednesday, 2026-08-26  
**Status:** Backlog (pre-implementation security gap)  
**Priority:** HIGH  
**Severity:** HIGH (when migration logic is implemented)

---

## Quick Navigation

| Document | Purpose | Read Time |
|----------|---------|-----------|
| **ISSUE_SUMMARY_MIGRATE_REPLAY.md** | Executive summary; quick reference for issue tracking | 5 min |
| **SECURITY_ISSUE_MIGRATE_REPLAY_PROTECTION.md** | Full technical specification; problem statement, solution, acceptance criteria | 15 min |
| **IMPLEMENTATION_GUIDE_MIGRATE_REPLAY.md** | Step-by-step developer guide; code examples, tests, documentation updates | 20 min |
| **This file** | Navigation and overview | 5 min |

---

## What Is This Issue?

The `migrate()` function in the escrow contract will be vulnerable to **transaction replay attacks** when migration logic is eventually implemented (e.g., v6 → v7 schema upgrade).

### Current State (v7 today)
- ✅ All code paths **panic before any storage writes** → no risk today
- ✅ Auth guard is already required by documentation
- ✅ No migration logic exists

### Future Risk (when migration logic is added)
- ❌ Without contract-level idempotency guards, state rewrites could be applied twice if:
  - A transaction is replayed (admin key compromised, retry loops, etc.)
  - Off-chain systems accidentally re-submit migration calls
- ❌ Result: yield calculations duplicated, per-investor records corrupted, audit logs inconsistent

### The Fix
Implement **contract-level idempotency protection** via migration execution nonce:

```rust
// Check: has this version transition already been applied?
if env.storage().instance().has(&DataKey::MigrationExecutionLog(6, 7)) {
    fail(&env, EscrowError::MigrationAlreadyApplied)  // Code 93
}

// After state transformation:
env.storage().instance().set(&DataKey::MigrationExecutionLog(6, 7), &env.ledger().sequence());
```

---

## Who Should Read This?

| Role | Priority | Read | Why |
|------|----------|------|-----|
| **Security Auditor** | 🔴 CRITICAL | Full issue doc | Understand attack vector & mitigation |
| **Backlog Owner** | 🔴 CRITICAL | Issue summary + Implementation guide | Prepare backlog item; estimate effort |
| **Rust Developer** | 🟠 HIGH | Implementation guide | Code examples, tests, exact changes needed |
| **Operator/DevOps** | 🟠 HIGH | Issue summary + OPERATOR_RUNBOOK updates | Understand replay behavior and error codes |
| **Tech Lead** | 🔴 CRITICAL | Full issue doc | Design review, risk assessment |
| **Product Owner** | 🟡 MEDIUM | Issue summary | Understand risk and timeline |

---

## Key Facts

| Aspect | Detail |
|--------|--------|
| **Vulnerability Class** | Replay attack (contract-level, not network-level) |
| **Current Risk** | NONE (no storage writes until migration logic added) |
| **Future Risk** | HIGH (when migration rewrites storage) |
| **Attack Preconditions** | Migration logic implemented + transaction replayed |
| **Impact** | State corruption, inconsistent accounting, failed audits |
| **Fix Complexity** | **Low** — ~50 lines of code, 3 tests, 3 doc updates |
| **Effort Estimate** | 2–3 hours (implementation + tests + docs) |
| **Breaking Change** | **NO** — additive; error code 93 is new |
| **Requires Redeploy** | **NO** — can be deployed as WASM upgrade |
| **Blocked By** | Nothing (can be implemented independently) |
| **Blocks** | Any future migration path (v6→v7, etc.) |

---

## Three Documents Provided

### 1. ISSUE_SUMMARY_MIGRATE_REPLAY.md

**Best for:** Issue trackers, backlog planning, quick reference

**Contains:**
- High-level problem statement (2 min read)
- Current vs. future risk (comparison table)
- Solution overview (4 bullet points)
- Acceptance criteria checklist (9 items)
- Related issues and references
- Operator Q&A

**Use when:** Creating a GitHub issue, JIRA ticket, or backlog item

---

### 2. SECURITY_ISSUE_MIGRATE_REPLAY_PROTECTION.md

**Best for:** Technical review, security audit, detailed planning

**Contains:**
- Full problem description (attack scenario, root cause)
- Proposed solution (detailed design)
- Acceptance criteria (9 detailed criteria with verification steps)
- Implementation checklist (7 tasks)
- Security considerations (threat model, out-of-scope vs. in-scope)
- Testing strategy (4 unit tests with scenarios)
- Deployment impact analysis
- References (Soroban docs, related ADRs)

**Use when:** Security review, architectural review, design phase

---

### 3. IMPLEMENTATION_GUIDE_MIGRATE_REPLAY.md

**Best for:** Developers implementing the fix

**Contains:**
- 6-step implementation walkthrough
- **Step 1:** Add DataKey enum variants (exact code)
- **Step 2:** Add error code (exact code)
- **Step 3:** Update migrate() function (before/after, highlighted changes)
- **Step 4:** Add unit tests (3 test templates with assertions)
- **Step 5:** Update documentation (exact markdown for 3 files)
- **Step 6:** Verification checklist (12 items)
- Local testing commands
- Commit message template
- FAQ (8 Q&A pairs)

**Use when:** Ready to implement the fix

---

## Implementation Timeline

### Phase 1: Backlog & Review (Today)
- [ ] Share security issue with team
- [ ] Conduct security review using full issue doc
- [ ] Add to backlog/sprint planning
- [ ] Estimate effort (2–3 hours)

### Phase 2: Implementation (Before any migration logic)
- [ ] Implement using guide (6 steps, ~2 hours)
- [ ] Write 3 unit tests (~30 min)
- [ ] Update 3 documentation files (~30 min)
- [ ] Local verification (cargo build, cargo test, cargo clippy)

### Phase 3: Code Review & Merge
- [ ] Submit PR with all 3 implementation changes
- [ ] Peer review (security + architecture focus)
- [ ] Address feedback
- [ ] Merge to main

### Phase 4: Pre-Migration Gate
- [ ] ✅ Idempotency protection deployed as WASM upgrade
- [ ] ✅ Verified on testnet and mainnet
- [ ] Now safe to implement any v6→v7 migration logic

---

## Architecture

```
migrate(from_version) called
│
├─ 1. Auth check (required admin)
│
├─ 2. Read stored version
│
├─ 3. Validate version alignment
│
├─ 4. **NEW: Check idempotency nonce**
│    └─ If MigrationExecutionLog(from, to) exists → FAIL with code 93
│
├─ 5. Version boundary checks
│
├─ 6. [Future] Apply migration logic (if implemented)
│    └─ State rewrite, yield calculations, etc.
│
├─ 7. [Future] Write new version
│
└─ 8. **NEW: Write idempotency nonce** (prevents replay)
    └─ Set MigrationExecutionLog(from, to) = sequence
```

---

## Storage Impact

### New DataKey Variants

```rust
// Per-instance storage (instance TTL)
DataKey::MigrationExecutionLog(from_version: u32, to_version: u32)
  → value: ledger_sequence (u64)
  → size: ~16 bytes
  → written once per migration path per instance

DataKey::MigrationCompletedAt(from_version: u32, to_version: u32)  [optional]
  → value: ledger_timestamp (u64)
  → size: ~16 bytes
  → written once per migration path per instance (audit trail)
```

### Per-Instance Storage Footprint
- **2–3 nonce keys per migration path** (minimal; most instances only migrate once)
- **No per-investor storage added** (idempotency is instance-level)
- **No persistent storage keys** (nonce is in instance storage; survives instance TTL extension)

---

## Error Code 93 Semantics

### EscrowError::MigrationAlreadyApplied = 93

| Property | Value |
|----------|-------|
| **Name** | MigrationAlreadyApplied |
| **Code** | 93 |
| **Emitted by** | `migrate()` |
| **Condition** | `MigrationExecutionLog(from_version, SCHEMA_VERSION)` key already exists |
| **Recovery** | Verify on-chain version; no retry needed |
| **Semantics** | Typed error; stable across releases |

**When you see it:**
- Operator called `migrate(6)` twice
- Second call sees the nonce and rejects replay
- This is **expected behavior** (safety mechanism)

**What to do:**
1. Check on-chain version with `get_version()`
2. If it matches target (e.g., 7), migration succeeded
3. Do not retry

---

## References

### In This Repository

- `escrow/src/lib.rs` — Contract source
  - `DataKey` enum (line ~506–665)
  - `EscrowError` enum (line ~200–440)
  - `migrate()` function (line ~4370–4419)

- `docs/escrow-error-messages.md` — Error code reference
- `docs/OPERATOR_RUNBOOK.md` — Migration implementation guide (§2)
- `docs/escrow-security-checklist.md` — Security assumptions (§5.1)
- `docs/adr/ADR-007-storage-key-evolution.md` — Storage versioning strategy

### External

- [Soroban Contract Authorization](https://developers.stellar.org/docs/build/guides/auth/contract-authorization)
- [Soroban Storage Model](https://developers.stellar.org/docs/learn/storing-data/contract-storage)
- [Stellar Ledger Semantics](https://developers.stellar.org/docs/learn/storing-data)

---

## Contact & Questions

- **Security concern?** Include full issue doc in disclosure
- **Implementation question?** Refer to step-by-step guide
- **Backlog planning?** Use issue summary
- **Code review?** Reference implementation guide

---

## Checklist for Issue Triage

- [ ] Issue title: "[SECURITY] Add replay protection for migrate calls"
- [ ] Priority: HIGH
- [ ] Severity: HIGH (when migration logic added)
- [ ] Status: Backlog
- [ ] Component: escrow/src/lib.rs
- [ ] Epic: Schema Upgrade / Migration Framework
- [ ] Related issues: ADR-007, ADR-009, OPERATOR_RUNBOOK §2
- [ ] Effort estimate: 2–3 hours
- [ ] Blocked by: Nothing
- [ ] Blocks: Any v6→v7 or later migration path
- [ ] Requires: Security review, peer review, test coverage
- [ ] Documents attached: ✅ 3 (issue, implementation guide, this index)

---

## Version History

| Date | Version | Status | Notes |
|------|---------|--------|-------|
| 2026-08-26 | 1.0 | Draft | Initial comprehensive documentation created |

---

**Last Updated:** 2026-08-26 12:57 UTC  
**Next Review:** Before implementing any migration logic  
**Owner:** Security & Engineering Team
