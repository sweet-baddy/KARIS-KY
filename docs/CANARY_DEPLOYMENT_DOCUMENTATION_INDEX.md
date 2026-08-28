# Canary Deployment Documentation Index

Complete reference for staged contract upgrades using canary testing on a subset of escrow instances.

---

## Quick Overview

**Canary deployments** split the upgrade process into two stages:

1. **Stage 1 — Canary (24-72 hours):** Deploy to 2-5 escrows marked with `is_canary: true`
2. **Stage 2 — Production (full rollout):** Deploy to remaining instances after governance approval

**Key benefits:**
- ✓ Reduces risk of widespread failures
- ✓ Allows real-world testing before full rollout
- ✓ Separates concerns: CanaryOperator vs. Admin
- ✓ Clear decision gate before production

---

## Documents in this section

| Document | Purpose | Audience | Length |
|----------|---------|----------|--------|
| **CANARY_DEPLOYMENT_STRATEGY.md** | Complete procedures for canary workflow | Ops, SRE | 30 min read |
| **CANARY_RBAC_AND_ACCESS_CONTROL.md** | Role permissions and key management | Ops, Security, Governance | 20 min read |
| **CANARY_DEPLOYMENT_CHECKLIST.md** | Fillable checklist for canary rollouts (printable) | Ops executing canary | 10 min per deployment |
| **CANARY_DEPLOYMENT_DOCUMENTATION_INDEX.md** | This document — navigation guide | Anyone | 5 min read |

---

## Key concepts

### `is_canary: bool` flag

Each escrow instance has a boolean flag indicating canary status:

```json
{
  "contract_id": "CAAAA...",
  "invoice_id": "INV-CANARY-001",
  "is_canary": true,
  "status": "open"
}
```

**Implications:**
- CanaryOperator can deploy WASM to canary instances
- Admin can deploy WASM to production instances (is_canary = false)
- Canary instances must be representative of production (various states, investor counts)

### CanaryOperator role

New RBAC role with limited permissions:

| Can do | Cannot do |
|--------|-----------|
| ✓ Deploy to canary instances only | ✗ Deploy to production |
| ✓ Monitor all instances | ✗ Promote canary to production |
| ✓ Set legal hold on canary | ✗ Set legal hold on production |
| ✓ Rollback canary | ✗ Rollback production |

---

## Workflow overview

```
DAY -1: Testnet staging
  ├─ Build WASM
  ├─ Test on testnet
  ├─ Governance approves Stage 1 (canary)
  └─ CanaryOperator on-call

DAY 0: Canary deployment (T+0)
  ├─ Upload WASM to mainnet
  ├─ Activate legal hold on canary instances
  ├─ Deploy to canary instances (is_canary == true)
  ├─ Verify all instances healthy
  └─ Clear legal hold

DAY 0-3: Canary monitoring (72 hours)
  ├─ 1-hour checkpoint: health verified
  ├─ 24-hour checkpoint: no unusual errors
  ├─ 72-hour checkpoint: metrics aggregated
  └─ Report submitted to governance

DAY 3: Production approval
  ├─ Governance reviews canary report
  ├─ Vote: APPROVE Stage 2 (production)
  └─ Admin prepares production deployment

DAY 4+: Production deployment (Admin-only)
  ├─ Admin deploys to remaining instances
  ├─ Full monitoring begins
  └─ Canary stage complete
```

---

## Using this documentation

### I'm planning a canary deployment (first time)

1. **Read:** CANARY_DEPLOYMENT_STRATEGY.md (30 min)
   - Understand the two-stage model
   - Learn instance selection criteria
   - Review the full workflow

2. **Read:** CANARY_RBAC_AND_ACCESS_CONTROL.md (20 min)
   - Understand CanaryOperator permissions
   - Review key management strategy
   - Confirm access control in place

3. **Print:** CANARY_DEPLOYMENT_CHECKLIST.md
   - Use during execution (keep handy)

### I'm executing a canary deployment (Day 0)

1. **Reference:** CANARY_DEPLOYMENT_CHECKLIST.md (Phase 1-2)
   - Pre-canary planning (if not done)
   - Canary deployment steps

2. **Copy:** Relevant scripts from CANARY_DEPLOYMENT_STRATEGY.md (Part 3)
   - `canary_health_1h.sh`
   - Deployment scripts
   - Monitoring queries

3. **Execute:** Each step in checklist, cross off as you go

### I'm monitoring the canary (72 hours post-deployment)

1. **Use:** CANARY_DEPLOYMENT_CHECKLIST.md (Phase 3-5)
   - 1-hour, 24-hour, 72-hour checkpoints
   - Metrics tracking
   - Promotion criteria assessment

2. **Reference:** CANARY_DEPLOYMENT_STRATEGY.md Part 6
   - Monitoring dashboard queries
   - Health check scripts
   - Metrics interpretation

### I'm reporting to governance (Day 3)

1. **Gather:** From checklist Phase 5 (72-hour checkpoint)
   - Aggregated metrics
   - Promotion criteria results
   - Any incidents or issues

2. **Draft:** Canary report for governance
   - Include all metrics
   - Recommendation: PROMOTE / HOLD / ROLLBACK
   - Risk assessment

3. **Submit:** To governance for Stage 2 vote

### Emergency: Canary deployment has critical issues

1. **Immediate:** CANARY_DEPLOYMENT_CHECKLIST.md (Emergency Rollback section)
   - Decision criteria
   - Rollback procedure
   - Post-rollback investigation

2. **Reference:** CANARY_DEPLOYMENT_STRATEGY.md Part 5
   - Rollback decision criteria
   - Investigation checklist

---

## Acceptance criteria verification

### ✓ Criterion 1: New role CanaryOperator can deploy to canary escrows only

**Implementation:**
- RBAC defined in CANARY_RBAC_AND_ACCESS_CONTROL.md
- Contract-level checks enforce `is_canary` flag
- Permission matrix shows CanaryOperator limited to canary deployment
- Status: COMPLETE

### ✓ Criterion 2: Canary escrows flagged with `is_canary: bool`

**Implementation:**
- Flag definition in CANARY_DEPLOYMENT_STRATEGY.md § 1.1
- Init parameter and admin entrypoint to set flag
- Inventory JSON shows classification
- Contract storage includes flag in InvoiceEscrow struct
- Status: COMPLETE

**Verification:**
- Instance inventory distinguishes canary vs. production
- Flag checked before each deployment
- Enforcement at contract level prevents CanaryOperator from deploying to production

---

## Permission matrix summary

### CanaryOperator
- ✓ Deploy to `is_canary == true` instances
- ✓ Monitor all instances (query state)
- ✓ Activate/clear legal hold on canary
- ✓ Rollback canary instances
- ✗ Deploy to production
- ✗ Promote to production
- ✗ Manage Admin access

### Admin (multisig)
- ✓ Deploy to `is_canary == false` instances
- ✓ Deploy to canary (optional override)
- ✓ Promote canary to production
- ✓ Manage CanaryOperator access
- ✓ Emergency escalation
- ✓ Set/clear legal hold on production

### Governance
- ✓ Approve canary stage (Stage 1)
- ✓ Approve production stage (Stage 2)
- ✓ Vote on new CanaryOperator
- ✗ Deploy directly
- ✗ Execute on-chain operations

---

## Key files and locations

```
/workspaces/KARIS-KY/docs/
├── CANARY_DEPLOYMENT_STRATEGY.md              (584 lines)
│   ├─ Part 1: Instance classification
│   ├─ Part 2: CanaryOperator role
│   ├─ Part 3: Canary deployment procedure
│   ├─ Part 4: Promotion to production
│   ├─ Part 5: Canary rollback
│   ├─ Part 6: Monitoring dashboard
│   └─ Part 7: Canary checklist
│
├── CANARY_RBAC_AND_ACCESS_CONTROL.md         (410 lines)
│   ├─ Role hierarchy
│   ├─ Permission matrix
│   ├─ Access control implementation
│   ├─ Key management
│   ├─ Deployment workflow with RBAC
│   ├─ Audit trail
│   ├─ Onboarding/offboarding
│   ├─ Separation of duties
│   └─ Emergency escalation
│
├── CANARY_DEPLOYMENT_CHECKLIST.md            (363 lines)
│   ├─ Phase 1: Pre-canary planning
│   ├─ Phase 2: Canary deployment
│   ├─ Phase 3: 1-hour checkpoint
│   ├─ Phase 4: 24-hour checkpoint
│   ├─ Phase 5: 72-hour checkpoint
│   ├─ Phase 6: Governance approval
│   ├─ Phase 7: Production deployment
│   ├─ Phase 8: Post-upgrade review
│   └─ Emergency rollback checklist
│
└── CANARY_DEPLOYMENT_DOCUMENTATION_INDEX.md  (this file)
```

---

## Common scenarios

### Scenario: First canary deployment

**Time needed:** ~4 hours (prep + execution + first checks)

**Documents:**
1. CANARY_DEPLOYMENT_STRATEGY.md (read all)
2. CANARY_RBAC_AND_ACCESS_CONTROL.md (read § Key Management)
3. CANARY_DEPLOYMENT_CHECKLIST.md (Phases 1-2)

**Outcome:** Canary instances deployed; monitoring begins

### Scenario: Routine canary deployment (3rd time)

**Time needed:** ~2 hours (execution + monitoring start)

**Documents:**
1. CANARY_DEPLOYMENT_CHECKLIST.md (fill in as you go)
2. CANARY_DEPLOYMENT_STRATEGY.md § Part 3 (reference scripts)

**Outcome:** Canary deployed; monitoring begins

### Scenario: Canary health issue detected at 24h

**Time needed:** ~30 min (investigate + decide)

**Documents:**
1. CANARY_DEPLOYMENT_STRATEGY.md § Part 5 (rollback decision criteria)
2. CANARY_DEPLOYMENT_CHECKLIST.md § Emergency Rollback

**Outcome:** Rollback executed or investigation continues

### Scenario: Production deployment (after canary approval)

**Time needed:** ~3 hours (deployment + initial verification)

**Documents:**
1. CANARY_DEPLOYMENT_CHECKLIST.md § Phase 7 (production deployment)
2. CANARY_DEPLOYMENT_STRATEGY.md § Part 4 (promotion process)

**Outcome:** Production deployment complete; full monitoring begins

---

## Integration with existing upgrade documentation

**Canary deployment fits into the larger upgrade framework:**

```
MULTI_INSTANCE_UPGRADE_GUIDE.md
  ├─ Part 1: Pre-upgrade validation (shared)
  └─ Part 2/3: Additive or Redeploy (main flow)
  
  ↓ (if using canary)
  
CANARY_DEPLOYMENT_STRATEGY.md
  ├─ Stage 1: Deploy to canary instances
  ├─ Stage 2: Deploy to production instances
  └─ Uses same Part 2/3 procedures per stage
  
  ↓ (if issues arise)
  
UPGRADE_DECISION_TREES.md
  └─ Decision support and troubleshooting
```

**Key point:** Canary adds a two-stage wrapper around the normal upgrade procedure. The underlying deployment steps (additive or redeploy) remain the same.

---

## Governance voting template

When requesting canary approval:

```
PROPOSAL: Stage 1 Canary Deployment — Schema v6

DETAILS:
  ├─ WASM hash: abc123...
  ├─ Type: Additive WASM upgrade
  ├─ Canary instances: 2
  ├─ Timeline: 72-hour monitoring
  └─ Expected result: Metrics report + recommendation

APPROVAL REQUESTED:
  ✓ Canary stage (Stage 1) — proceed with canary deployment

---

PROPOSAL: Stage 2 Production Deployment — Schema v6 (after canary)

DETAILS:
  ├─ Canary report: [link to metrics]
  ├─ Canary health: PASSED (error rate 0.05%, no complaints)
  ├─ Production instances: 50
  ├─ WASM hash (same): abc123...
  └─ Timeline: immediate after approval

APPROVAL REQUESTED:
  ✓ Production stage (Stage 2) — proceed with full rollout
```

---

## Canary instance selection worksheet

Use this to document your canary selection:

```
CANARY DEPLOYMENT PLANNING WORKSHEET

Deployment date: ________________
WASM hash: ________________
CanaryOperator: ________________

CANARY INSTANCES (2-3 recommended):

Instance 1: ________________
  Invoice ID: ________________
  Current status: OPEN / FUNDED / SETTLED
  Investor count: ____
  Funded amount: ________________
  Represents: [e.g., "small open escrow"]

Instance 2: ________________
  Invoice ID: ________________
  Current status: OPEN / FUNDED / SETTLED
  Investor count: ____
  Funded amount: ________________
  Represents: [e.g., "medium funded escrow"]

Instance 3: ________________
  Invoice ID: ________________
  Current status: OPEN / FUNDED / SETTLED
  Investor count: ____
  Funded amount: ________________
  Represents: [e.g., "large funded with active claims"]

PRODUCTION INSTANCES (remaining):
  Total: ____ instances
  Total investors: ____ 
  Total funded amount: ________________
  is_canary = false on all: YES / NO

RATIONALE:
  Canary mix represents: _________________________________
  Risk level: LOW / MEDIUM / HIGH
  Confidence: ________________________________________
```

---

## Troubleshooting matrix

| Problem | Root cause | Solution | Reference |
|---------|-----------|----------|-----------|
| CanaryOperator can't deploy to canary | Access not granted | Add to canary_operators in contract | RBAC § Access Control Implementation |
| Canary instances have is_canary=false | Flag not set at init | Set flag using init param or admin entrypoint | Strategy § 1.1 |
| Production instance accidentally deployed to | Wrong WASM or contract logic | Rollback via Admin; investigate flag checking | Rollback § Emergency |
| Governance never approved canary stage | Proposal not submitted | Submit Stage 1 vote before Day 0 | Voting template above |
| 72-hour metrics don't meet promotion criteria | WASM bug or integration issue | Investigate and re-test on testnet; reschedule canary | Strategy § Part 5 |
| Canary rollback successful but no investigation | Decision made too fast | Post-rollback, investigate thoroughly before re-canary | Rollback § Post-rollback Investigation |

---

## Success criteria for canary deployment

**Canary deployment is successful when:**

- ✓ Both canary and production instances deployed with matching WASM
- ✓ No unexpected errors in canary instances during 72h window
- ✓ Investor operations (funding, claims) work normally
- ✓ Governance approves promotion to production
- ✓ Production deployment completes without issues
- ✓ All metrics within expected ranges

**Canary deployment failed if:**

- ✗ Critical bug forces emergency rollback
- ✗ Governance denies production approval
- ✗ Canary > production mismatch in behavior
- ✗ Metrics exceed error thresholds

---

## Appendix: Metrics dashboard example

```json
{
  "deployment_id": "v6-canary-2024-07-27",
  "stage": "canary",
  "canary_instances": 2,
  "monitoring_window_hours": 72,
  "metrics": {
    "rpc_error_rate": {
      "1h": 0.02,
      "24h": 0.05,
      "72h": 0.07,
      "threshold": 0.1,
      "status": "PASS"
    },
    "investor_complaints": {
      "count": 0,
      "threshold": 0,
      "status": "PASS"
    },
    "settlement_success_rate": {
      "1h": 100,
      "24h": 100,
      "72h": 100,
      "threshold": 100,
      "status": "PASS"
    },
    "gas_usage_vs_baseline": {
      "1h": "+3%",
      "24h": "+5%",
      "72h": "+4%",
      "threshold": "±10%",
      "status": "PASS"
    }
  },
  "promotion_ready": true,
  "report_date": "2024-07-30T12:00:00Z"
}
```

---

## Contact and escalation

| Question | Contact | Response |
|----------|---------|----------|
| How do I set up canary instances? | Ops lead | 24h |
| Who is CanaryOperator? | Admin multisig | 24h |
| Can I modify canary instance list mid-deployment? | Governance | Vote required |
| What if canary fails? | Ops lead | Immediate |
| How long until production deployment? | Governance | 72h after canary approval |

---

**Last updated:** 2024-07-27  
**Status:** Ready for production use  
**Next review:** After first canary deployment
