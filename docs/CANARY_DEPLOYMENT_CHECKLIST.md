# Canary Deployment Checklist

Fillable checklist for canary deployments. Print and use for each staged rollout.

---

## Phase 1: Pre-Canary Planning (Day -3 to -1)

### Code preparation
- [ ] New WASM built: `cargo build --target wasm32v1-none --release`
- [ ] All CI gates pass:
  - [ ] Format: `cargo fmt --all -- --check`
  - [ ] Linter: `cargo clippy -p karis-ky_escrow -- -D warnings`
  - [ ] Tests: `cargo test -p karis-ky_escrow`
  - [ ] Coverage: `cargo llvm-cov --fail-under-lines 95`
- [ ] Security code review completed and approved
- [ ] ADR/changelog updated for new version

### Testnet validation
- [ ] Upload WASM to testnet
- [ ] Deploy to testnet canary instances
- [ ] Run smoke tests:
  - [ ] `get_version` returns expected schema
  - [ ] `get_escrow` reads successfully
  - [ ] Investor funding works (if applicable)
  - [ ] Settlement/claims work (if applicable)
- [ ] No errors in testnet logs
- [ ] Performance within baseline ±15%

### Canary instance selection
- [ ] Canary instances identified: ________________
- [ ] Canary count: _____ (typically 2-3 instances, 5-10% of base)
- [ ] Mix of instance states documented:
  - [ ] Open status (funding in progress)
  - [ ] Funded status (ready for settlement)
  - [ ] Variety of investor counts
- [ ] Inventory JSON created and verified
- [ ] All canary instances have `is_canary: true` flag

### Governance approval
- [ ] Stage 1 (canary) approval request submitted to governance
- [ ] Vote scheduled: _____________________
- [ ] Vote result: APPROVED / REJECTED
- [ ] If approved, date/time approved: ______________

### CanaryOperator readiness
- [ ] CanaryOperator assigned: _____________________
- [ ] CanaryOperator key funded with XLM (for gas)
- [ ] CanaryOperator has read access to all instances (verified)
- [ ] CanaryOperator trained on canary procedures
- [ ] CanaryOperator on-call for next 72 hours

### Monitoring setup
- [ ] Dashboard created for canary metrics
- [ ] Health check scripts prepared (`canary_health_1h.sh`, etc.)
- [ ] Rollback plan documented
- [ ] Incident response contact list prepared

---

## Phase 2: Canary Deployment (Day 0)

### Pre-deployment verification
- [ ] All Pre-Canary Planning items checked off above
- [ ] Governance has approved Stage 1
- [ ] WASM hash recorded: `_______________________________`
- [ ] Canary instances list confirmed (no production mixed in):
  - [ ] Instance 1: __________ (is_canary verified)
  - [ ] Instance 2: __________ (is_canary verified)
  - [ ] Instance 3: __________ (is_canary verified)

### Legal hold activation
- [ ] Legal hold activated on all canary instances
- [ ] Status verified: all instances report `legal_hold_active: true`
- [ ] Timestamp recorded: ______________

### WASM upload
- [ ] Build completed and verified
- [ ] WASM uploaded to mainnet
- [ ] WASM hash verified matches testnet deployment
- [ ] Hash recorded: `_______________________________`

### Deployment execution
- [ ] Upgrade invocation issued to instance 1
  - [ ] Invocation successful (no timeout/error)
  - [ ] Timestamp: ______________
- [ ] Upgrade invocation issued to instance 2
  - [ ] Invocation successful
  - [ ] Timestamp: ______________
- [ ] Upgrade invocation issued to instance 3
  - [ ] Invocation successful
  - [ ] Timestamp: ______________

### Post-deployment verification
- [ ] Version query returns expected schema on all instances
- [ ] `get_escrow` reads without errors on all instances
- [ ] Escrow status unchanged (before ≈ after)
- [ ] Funded amount unchanged (state integrity)
- [ ] Legal hold remains active (as set pre-deployment)

### Legal hold clearance
- [ ] Legal hold cleared on all canary instances
- [ ] Status verified: all instances report `legal_hold_active: false`
- [ ] Timestamp recorded: ______________
- [ ] Canary deployment complete: ______________

---

## Phase 3: Canary Monitoring — 1-Hour Checkpoint

**Time: T + 1 hour after deployment**

### Health check execution
- [ ] Health check script run: `bash canary_health_1h.sh`
- [ ] Report generated: `canary_report_1h.txt`
- [ ] All instances responding: YES / NO
- [ ] Any RPC errors? YES / NO
  - If YES, describe: ________________________________________

### Instance-by-instance verification
**Instance 1: __________**
- [ ] Version query successful
- [ ] get_escrow returns data without error
- [ ] Status unchanged
- [ ] Legal hold: FALSE (confirmed cleared)

**Instance 2: __________**
- [ ] Version query successful
- [ ] get_escrow returns data without error
- [ ] Status unchanged
- [ ] Legal hold: FALSE

**Instance 3: __________**
- [ ] Version query successful
- [ ] get_escrow returns data without error
- [ ] Status unchanged
- [ ] Legal hold: FALSE

### Initial metrics
- [ ] RPC error rate: _____ %
- [ ] Gas usage normal: YES / NO
- [ ] Any contract panics: NO
- [ ] Any investor complaints: NO
- [ ] Summary: ________________________________________________

---

## Phase 4: Canary Monitoring — 24-Hour Checkpoint

**Time: T + 24 hours after deployment**

### Continuous monitoring review
- [ ] No alerts triggered in 24h window
- [ ] No escalations to ops team
- [ ] Logs reviewed for anomalies: YES / NO
  - If found, describe: _____________________________________

### Business metrics (if applicable)
- [ ] Investor funding (if any): ____ new deposits
  - [ ] All successful (no failures)
- [ ] Settlement transactions (if any): ____ transactions
  - [ ] All successful (100% success rate)
- [ ] Investor claims (if any): ____ claims
  - [ ] All successful

### Performance metrics
- [ ] Gas usage vs. baseline: _____ % (acceptable: ±15%)
- [ ] RPC latency: _____ ms (acceptable: < 1000ms)
- [ ] Error rate: _____ % (acceptable: < 0.1%)
- [ ] All instances healthy: YES / NO

### State integrity check
- [ ] Escrow state still matches post-deployment (no drift)
- [ ] No unexpected legal holds activated
- [ ] No investor disputes reported

### Assessment
- [ ] Canary health: GOOD / CONCERNING / FAILED
- [ ] Notes: ________________________________________________

---

## Phase 5: Canary Monitoring — 72-Hour Checkpoint (Final)

**Time: T + 72 hours after deployment**

### Aggregated metrics
- [ ] Total RPC errors over 72h: _____ (acceptable: < 0.1%)
- [ ] Total investor complaints: _____ (acceptable: 0)
- [ ] Settlement success rate: _____ % (acceptable: 100%)
- [ ] Average gas usage vs. baseline: _____ % (acceptable: ±10%)
- [ ] Uptime: _____ % (acceptable: > 99%)

### Promotion criteria check
| Criterion | Threshold | Actual | Status |
|-----------|-----------|--------|--------|
| Error rate | < 0.1% | ____ | ✓/✗ |
| Investor complaints | 0 | ____ | ✓/✗ |
| Settlement success | 100% | ____ | ✓/✗ |
| RPC availability | > 99% | ____ | ✓/✗ |
| Gas usage | ±10% baseline | ____ | ✓/✗ |

**Overall canary result:** PASS / FAIL

### Final logs and artifacts
- [ ] 72-hour monitoring report compiled
- [ ] All logs archived
- [ ] Metrics exported for governance review
- [ ] Incident report (if any): ________________________________

---

## Phase 6: Governance Approval for Production

### Canary report preparation
- [ ] Executive summary written
- [ ] Metrics dashboard screenshot taken
- [ ] Error logs reviewed (none / minor issues only)
- [ ] Recommendation: PROMOTE / HOLD / ROLLBACK

### Governance vote
- [ ] Stage 2 (production) approval request submitted
- [ ] Vote scheduled: _____________________
- [ ] Canary report attached to vote
- [ ] Vote result: APPROVED / REJECTED / DEFERRED
- [ ] If approved, date/time: ______________

**If REJECTED or DEFERRED:**
- [ ] Reason documented: ____________________________________
- [ ] Corrective action (if any): ____________________________
- [ ] Next steps: _________________________________________

---

## Phase 7: Production Deployment (Admin-only)

### Admin preparation
- [ ] Admin (multisig) notified of governance approval
- [ ] Admin gathered for signing session
- [ ] WASM hash verified (same as canary): `_____________`
- [ ] Production instance list prepared (is_canary == false):
  - [ ] __________, __________, __________, ...
  - [ ] Total production instances: _____

### Production deployment execution
- [ ] Legal hold activated on all production instances
- [ ] Upgrade invocations issued to all instances (can be batched)
- [ ] All upgrades successful (no timeouts/errors)
- [ ] Timestamp completed: ______________

### Production verification
- [ ] Version query on 5+ random production instances: all correct
- [ ] get_escrow on 5+ random instances: no errors
- [ ] State integrity verified (spot-check funded_amount)
- [ ] Legal hold cleared on all production instances

### Production monitoring begins
- [ ] 1h, 24h, 72h checkpoints scheduled
- [ ] Monitoring dashboard updated
- [ ] Ops team notified of full rollout

---

## Phase 8: Post-Upgrade Review

### Meeting scheduled
- [ ] Retrospective meeting scheduled: _____________________
- [ ] Attendees: Ops, Security, Governance, CanaryOperator

### Review items
- [ ] What went well: __________________________________________
- [ ] What could improve: ______________________________________
- [ ] Any incidents or surprises: _______________________________
- [ ] Canary value assessment: __________________________________
- [ ] Changes to procedures (if any): ___________________________

### Documentation updates
- [ ] Runbook updated with lessons learned
- [ ] Monitoring dashboard procedures improved: YES / NO
- [ ] RBAC or access control refinements needed: YES / NO
  - If YES, action items: ___________________________________

### Sign-off

```
Canary deployment ID: ________________________________________
CanaryOperator: ________________________  Date: ______________
Admin (multisig): ________________________  Date: ______________
Governance: ________________________  Date: ______________

Canary result:  ✓ SUCCESS  ☐ PARTIAL  ☐ FAILED
Production deployment:  ✓ COMPLETE  ☐ ONGOING  ☐ ROLLED BACK
```

---

## Emergency Rollback Checklist

**Use if canary deployment has critical issues**

### Initial response
- [ ] Issue identified and documented
- [ ] CanaryOperator notified (escalation: IMMEDIATE)
- [ ] Governance notified (escalation: IMMEDIATE)
- [ ] Incident channel opened

### Rollback decision
- [ ] Root cause identified (or decision to rollback first, investigate later)
- [ ] Rollback authorized by CanaryOperator (canary only) or Admin (production)
- [ ] Old WASM hash confirmed: `_______________________________`

### Rollback execution
- [ ] Legal hold activated on affected instances
- [ ] Rollback invocation issued (upgrade to old WASM hash)
- [ ] All instances successfully reverted
- [ ] Timestamp completed: ______________

### Post-rollback verification
- [ ] All instances respond to RPC
- [ ] Version query shows old schema
- [ ] get_escrow returns state without errors
- [ ] Investor data intact (spot-check funded_amount)

### Investigation
- [ ] Root cause documented
- [ ] Code fix prepared (if applicable)
- [ ] Re-test on testnet: YES / NO / PENDING
- [ ] Timeline for re-canary: _____________________

---

## Escalation contacts

| Role | Contact | Response time |
|------|---------|----------------|
| CanaryOperator | _________________ | 30 min |
| Ops lead | _________________ | 15 min |
| Admin (multisig) | _________________ | 1 hour |
| Security team | _________________ | 30 min |
| Governance | _________________ | 2 hours |

---

## Appendix: Metrics queries

```bash
# Error rate over 72h
stellar-indexer query errors \
  --contract CANARY_INSTANCE \
  --start "2024-07-27T12:00:00Z" \
  --end "2024-07-30T12:00:00Z"

# Settlement success rate
stellar-indexer query transactions \
  --contract CANARY_INSTANCE \
  --type settlement \
  --success_only

# Gas usage (typical per call)
stellar-indexer query gas_usage \
  --contract CANARY_INSTANCE \
  --function upgrade
```
