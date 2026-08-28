# Escrow Contract Upgrade Documentation Index

This section contains comprehensive guides for safely upgrading karis-ky escrow contracts across multiple live instances.

---

## Quick Links

| Document | Purpose | Audience | Length |
|----------|---------|----------|--------|
| **MULTI_INSTANCE_UPGRADE_GUIDE.md** | Complete step-by-step upgrade procedures for both additive and breaking changes | Ops, DevOps, SRE | 30–45 min read |
| **UPGRADE_CHECKLIST.md** | Fillable checklist for every upgrade (print-friendly) | Ops performing upgrade | 5–10 min per upgrade |
| **UPGRADE_DECISION_TREES.md** | Decision trees and pitfall reference for quick troubleshooting | Anyone executing upgrades | 5 min lookup |
| **OPERATOR_RUNBOOK.md** (existing) | Redeploy vs. on-chain upgrade decision tree and CLI examples | Ops, governance | 20 min read |
| **escrow-error-messages.md** (existing) | Complete typed error code reference | Ops, developers | Reference only |

---

## When to use each document

### Planning an upgrade (Do this first)

1. **Read:** OPERATOR_RUNBOOK.md (Section 1 – decision tree)
   - Determine: redeploy vs. WASM upgrade?
   
2. **Read:** MULTI_INSTANCE_UPGRADE_GUIDE.md (Parts 1–2 or 1 & 3)
   - Choose path: additive upgrade or redeploy
   - Understand: pre-flight, deployment, rollback, monitoring

3. **Print:** UPGRADE_CHECKLIST.md
   - Keep handy during execution

### During an upgrade (Day 1+)

1. **Reference:** UPGRADE_CHECKLIST.md
   - Cross off items as you complete them
   - Verify nothing is skipped

2. **Lookup:** UPGRADE_DECISION_TREES.md
   - Stuck on a step? Check symptom-to-action table
   - Unsure about migrate()? Check decision tree 2

3. **Consult:** MULTI_INSTANCE_UPGRADE_GUIDE.md (specific section)
   - Run the commands from your chosen path (additive or redeploy)
   - Follow worked examples for real-world timing

### Troubleshooting issues

1. **Start:** UPGRADE_DECISION_TREES.md (Decision Tree 4)
   - What error did you get?
   - What's the first action?

2. **Deep dive:** UPGRADE_DECISION_TREES.md (Common Pitfalls section)
   - Does your issue match any pitfall description?
   - What's the fix and prevention?

3. **Escalate:** Contact ops on-call (if root cause unclear)

---

## Document overview

### MULTI_INSTANCE_UPGRADE_GUIDE.md

**Sections:**
- **Part 1: Pre-Upgrade Validation** — Checklists and instance inventory
- **Part 2: Additive WASM Upgrade** — Zero-downtime upgrade for new keys only
- **Part 3: Redeploy** — Breaking changes; new contract ID required
- **Part 4: Rollback Procedure** — Emergency revert for both paths
- **Part 5: Monitoring** — Real-time health checks and post-upgrade audit
- **Part 6: Checklist by type** — Quick summary per upgrade type
- **Part 7: FAQ** — Common questions answered
- **Part 8: Worked example — additive** — Real timeline with actual commands
- **Part 9: Worked example — redeploy** — Step-by-step redeploy scenario
- **Part 10: Emergency procedures** — Legal hold, rollback, investigation
- **Appendices** — Templates, monitoring queries, version matrix, approval workflow

**When to use:** Before any upgrade; reference during execution

---

### UPGRADE_CHECKLIST.md

**Sections:**
- **Pre-Upgrade Phase** — Build, verify, approvals, testnet staging
- **Additive WASM Upgrade Path** — All steps to deploy new WASM to mainnet
- **Redeploy Path** — Snapshot, deploy new instances, restore data, notify integrators
- **Post-Upgrade Monitoring** — 1h, 24h, 72h health checks
- **Rollback Decision Tree** — When to rollback and how
- **Emergency Contacts** — Who to call and response times
- **Sign-off section** — Document upgrade metadata for audit trail
- **Post-Upgrade Review** — Retrospective meeting template

**When to use:** Print and fill during upgrade execution (keep as record)

---

### UPGRADE_DECISION_TREES.md

**Sections:**
- **Decision Tree 1:** What upgrade path should I use? (code review flow)
- **Decision Tree 2:** Should I call migrate()? (version mismatch prevention)
- **Decision Tree 3:** Is this instance safe to upgrade right now? (status checks)
- **Decision Tree 4:** What do I do if the upgrade fails? (error diagnosis)
- **Common Pitfalls (1–10)** — What went wrong, why, how to fix, how to prevent
- **Symptom-to-action lookup** — Quick table for troubleshooting
- **Success criteria** — How to verify each upgrade type succeeded

**When to use:** Stuck during upgrade; quick reference for decisions or errors

---

## Upgrade paths at a glance

### Path A: Additive WASM Upgrade (safer, faster)

| Aspect | Detail |
|--------|--------|
| **When to use** | No struct layout changes; only new `DataKey` variants |
| **Time to mainnet** | 1–2 hours (after testnet staging) |
| **Downtime** | None (concurrent operations OK during upgrade) |
| **Reversibility** | High — revert old WASM hash anytime |
| **Complexity** | Low — straightforward CLI invocations |
| **Investor impact** | None — contract ID unchanged, claims/payouts unaffected |
| **Redeploy needed** | No |
| **migrate() call needed** | No (unless you implement storage rewrite) |

**Step summary:**
1. Build and upload WASM to mainnet
2. (Optional) Activate legal hold on funded instances
3. (If upgrade entrypoint exists) Invoke upgrade with new WASM hash
4. Verify all instances return expected schema
5. (Optional) Clear legal hold
6. Monitor 72 hours

---

### Path B: Redeploy (riskier, more complex)

| Aspect | Detail |
|--------|--------|
| **When to use** | `InvoiceEscrow` or stored struct layout changed |
| **Time to mainnet** | 2–4 hours per batch of instances |
| **Downtime** | Investor onboarding paused during redeploy |
| **Reversibility** | Low — old contract ID is archived, not reverted |
| **Complexity** | High — new instances, data migration, integrator updates |
| **Investor impact** | High — new contract ID, must update endpoints |
| **Redeploy needed** | Yes |
| **migrate() call needed** | No (new instances start fresh) |

**Step summary:**
1. Build, upload WASM, deploy new instances per old instance
2. Init new instances with same parameters
3. Restore investor contributions (if pre-existing)
4. Update integrators with new contract IDs
5. Activate legal hold on old instances
6. Monitor 72 hours and complete investor migration

---

## Pre-upgrade decision flowchart (printable)

```
┌─────────────────────────────────────────────────────────┐
│ New WASM release ready for mainnet                       │
└─────────────────────────────┬───────────────────────────┘
                              │
                    ┌─────────▼─────────┐
                    │ Any #[contracttype]│
                    │ struct changed?    │
                    └─────────┬─────────┘
              ┌───────────────┴───────────────┐
              │ YES                           │ NO
              ▼                               ▼
         ┌─────────────┐              ┌──────────────┐
         │ REDEPLOY    │              │ Check: Any   │
         │ REQUIRED    │              │ DataKey      │
         └─────────────┘              │ removed?     │
              ✗                       └──────┬───────┘
         Expect new             ┌───────────┴──────────┐
         contract ID            │ YES                  │ NO
                                ▼                       ▼
                           ┌─────────────┐      ┌──────────────┐
                           │ REDEPLOY    │      │ Additive     │
                           │ REQUIRED    │      │ WASM UPDATE  │
                           └─────────────┘      │ SAFE         │
                                ✗               └──────────────┘
                           Expect new                   ✓
                           contract ID           Same contract
                                                 ID preserved
```

---

## Governance approval workflow

```
Ops + Security → Code Review → Governance Vote → Testnet Staging → Mainnet Deploy

1. Ops identifies new WASM ready for release
2. Security team reviews code diff (5–10 min)
3. Ops classifies as additive or breaking (1 min, OPERATOR_RUNBOOK Section 1)
4. Governance multisig votes on upgrade type (can be async, <24h typical)
5. Ops stages on testnet, runs health checks (1–2 hours)
6. (Approved) Ops deploys to mainnet during agreed window
7. Ops monitors 72 hours, provides status to governance
8. Governance review: completed/failed/rolled back
```

---

## Upgrade execution timeline (typical)

### Additive upgrade (best case)
```
T+0h:    All pre-flight checks pass; testnet staged; approved
T+0.5h:  Upload WASM to mainnet → hash recorded
T+1h:    Legal hold activated (if funded instances)
T+1.5h:  Upgrade invocation succeeds on all instances
T+2h:    Verification complete; legal hold cleared; monitoring starts
T+72h:   Final audit complete; upgrade declared successful
```

### Redeploy (best case)
```
T+0h:    Pre-redeploy snapshots captured
T+0.5h:  Upload WASM to mainnet → hash recorded
T+1h:    New instances deployed and initialized
T+2h:    Investor data restored to new instances
T+3h:    Integrators updated with new contract IDs
T+4h:    Old instances archived (legal hold); monitoring starts
T+72h:   Final audit complete; old instances confirmed retired
```

---

## Common pitfall checklist (prevent these)

- [ ] Calling `migrate()` during additive upgrade → **Don't; it panics**
- [ ] Attempting to upgrade with struct changes → **Redeploy instead**
- [ ] Forgetting to activate legal hold before upgrade → **Concurrent ops risk**
- [ ] Skipping testnet staging → **Surprised by mainnet bugs**
- [ ] Updating integrators after redeploy, not during → **Investors see wrong contract ID**
- [ ] Leaving legal hold active after upgrade → **Blocks investor claims**
- [ ] Using old contract ID after redeploy → **No-ops and confusion**
- [ ] Not documenting old → new contract mapping → **Audit trail lost**
- [ ] Missing pre-flight build checks → **Failed upgrade with no rollback**
- [ ] Upgrading without inventory sync → **May upgrade wrong instance**

---

## How each document complements the others

```
OPERATOR_RUNBOOK.md ◄──┐
(What type of       │   ├─► MULTI_INSTANCE_UPGRADE_GUIDE.md
 upgrade?)          │   │   (Complete step-by-step procedure)
                    │   │
                    │   ├─► UPGRADE_CHECKLIST.md
                    │   │   (Fill during execution)
                    │   │
                    └───┴─► UPGRADE_DECISION_TREES.md
                            (Troubleshooting & pitfalls)

                    ├─► escrow-error-messages.md
                    │   (If contract errors occur)
                    │
                    └─► TROUBLESHOOTING_GUIDE.md
                        (If RPC/Soroban issues)
```

---

## Glossary of key terms (from documents)

| Term | Definition | Docs |
|------|-----------|-------|
| **Additive upgrade** | New WASM deployment where only new `DataKey` variants are added; no stored struct changes | All |
| **Redeploy** | Deploy new contract instance; old instance archived; new contract ID required | All |
| **Legal hold** | Admin-set flag blocking settlement, withdrawal, and claims (for safety during upgrades) | Multi, Runbook |
| **Schema version** | `DataKey::Version` stored on-chain; tracks storage evolution independently of WASM | Multi, Runbook |
| **XDR** | Stellar's binary encoding format for contract types; layout changes require redeploy | Runbook, Trees |
| **migrate()** | Admin-gated entrypoint for storage rewrites between versions (panics if mismatch) | Multi, Runbook, Trees |
| **WASM hash** | Unique identifier for uploaded WASM bytecode on a network | Multi, Trees, Runbook |

---

## FAQ: Choosing the right document

**Q: I'm new to escrow upgrades. Where do I start?**
A: Read OPERATOR_RUNBOOK.md Section 1 first (5 min). Then read MULTI_INSTANCE_UPGRADE_GUIDE.md Part 1 (15 min). Then print UPGRADE_CHECKLIST.md for your upgrade day.

**Q: I'm about to upgrade. What's my quick reference?**
A: Print UPGRADE_CHECKLIST.md and UPGRADE_DECISION_TREES.md. Keep both open during execution.

**Q: My upgrade is stuck with an error. Where's the answer?**
A: UPGRADE_DECISION_TREES.md, section "Decision Tree 4" or "Symptom-to-action lookup."

**Q: What was that issue with legal hold again?**
A: UPGRADE_DECISION_TREES.md, Pitfall #3 "Missing pre-upgrade legal hold."

**Q: Can I upgrade all instances in parallel?**
A: Yes, uploads can be parallel. But invoke upgrade sequentially per instance (simpler rollback). See MULTI_INSTANCE_UPGRADE_GUIDE.md Part 2, Step 2.2.

**Q: What if redeploy fails halfway through?**
A: UPGRADE_DECISION_TREES.md, Pitfall #5. Emergency response: activate legal hold, disable onboarding, proceed with fix.

**Q: I rolled back successfully. What's next?**
A: UPGRADE_CHECKLIST.md, Rollback section, Step 5 ("Investigation and post-incident").

---

## Checklists quick reference

| Situation | Checklist to use | Time to complete |
|-----------|-----------------|------------------|
| First-time upgrade | MULTI_INSTANCE_UPGRADE_GUIDE + UPGRADE_CHECKLIST | 30–45 min prep + 2–4 hours execution |
| Routine additive upgrade | UPGRADE_CHECKLIST (additive path) | 5 min prep + 1–2 hours execution |
| Routine redeploy | UPGRADE_CHECKLIST (redeploy path) | 5 min prep + 2–4 hours execution |
| Troubleshooting mid-upgrade | UPGRADE_DECISION_TREES (Decision Tree 4) | 5–10 min diagnosis |
| Post-upgrade audit | MULTI_INSTANCE_UPGRADE_GUIDE Part 5 | 15–30 min per upgrade |

---

## Appendix: File locations and sizes

```
/docs/
  MULTI_INSTANCE_UPGRADE_GUIDE.md    32 KB  (Complete guide, all paths)
  UPGRADE_CHECKLIST.md                10 KB  (Fillable checklist)
  UPGRADE_DECISION_TREES.md           15 KB  (Trees, pitfalls, lookups)
  OPERATOR_RUNBOOK.md                 18 KB  (Original redeploy decision + CLI)
  escrow-error-messages.md            21 KB  (Typed error codes reference)
  
  + 25 other escrow-specific docs
```

---

## How to stay current

After each upgrade deployment:

1. [ ] Update instance inventory spreadsheet with new contract IDs / schema versions
2. [ ] Archive signed upgrade checklist (compliance record)
3. [ ] If issues arose, update UPGRADE_DECISION_TREES.md with new pitfall
4. [ ] Document any deviations from runbook in ops wiki
5. [ ] Brief next ops shift on lessons learned

---

## Support and escalation

| Issue | First resource | Escalate to |
|-------|-----------------|-------------|
| What upgrade path? | OPERATOR_RUNBOOK § 1 | Security team |
| Checklist reminder | UPGRADE_CHECKLIST | Ops manager |
| Stuck on error | UPGRADE_DECISION_TREES | Ops on-call |
| Contracts corrupted | UPGRADE_DECISION_TREES § Emergency | Security + Governance |
| XDR decode panic | OPERATOR_RUNBOOK § 5 | Senior engineer |
| Governance approval | MULTI_INSTANCE_UPGRADE_GUIDE Appendix D | Governance multisig |

---

**Last updated:** 2024-07-27 (schema version 6 stable)
**Maintainer:** karis-ky ops team  
**Next review:** After v7 schema is deployed
