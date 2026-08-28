# ✅ RFC Process Established — Complete

**Date:** July 27, 2026  
**Status:** Complete & Ready for Use

---

## What Was Delivered

### 📋 Process Documentation (4 files)

1. **[README.md](docs/rfc/README.md)** (222 lines)
   - Process overview and lifecycle
   - Numbering rules and file naming conventions
   - Review checklist for stakeholders
   - FAQ and getting started guide

2. **[TEMPLATE.md](docs/rfc/TEMPLATE.md)** (273 lines)
   - Standard RFC structure with all sections
   - Examples for each section
   - Acceptance criteria checklist
   - Rollout plan and monitoring guidance

3. **[LINKING_GUIDE.md](docs/rfc/LINKING_GUIDE.md)** (377 lines)
   - How to link RFCs ↔ ADRs ↔ GitHub issues
   - Bidirectional linking patterns
   - Search strategies for traceability
   - Best practices for maintaining links

4. **[INDEX.md](docs/rfc/) — Embedded in README**
   - Catalog of all RFCs (status, target release, notes)
   - Quick reference for discovery

### 💡 Example RFCs (3 files)

1. **[RFC-001: Multi-Token Support](docs/rfc/RFC-001-multi-token-support.md)** (418 lines)
   - **Status:** DRAFT
   - **Complexity:** High
   - **Scope:** Multiple settlement tokens (USDC, EURC, etc.)
   - **Timeline:** v2.0 (Q1 2027)
   - **Example of:** Complex feature proposal with alternatives

2. **[RFC-002: Yield Reinvestment](docs/rfc/RFC-002-yield-reinvestment.md)** (414 lines)
   - **Status:** DISCUSSION
   - **Complexity:** Medium
   - **Scope:** Auto-compound investor yield
   - **Timeline:** v1.5 (Q4 2026)
   - **Example of:** Feature with phased rollout + metrics

3. **[RFC-003: Registry Integration](docs/rfc/RFC-003-registry-integration.md)** (465 lines)
   - **Status:** ACCEPTED (decision already made)
   - **Complexity:** Low
   - **Scope:** Escrow discoverability
   - **Timeline:** v1.4 (Q3 2026)
   - **Example of:** Accepted RFC, ready for implementation

---

## File Structure

```
docs/rfc/
├── README.md                          # Process guide + index
├── TEMPLATE.md                        # Standard RFC template
├── LINKING_GUIDE.md                   # How to link docs
│
├── RFC-001-multi-token-support.md     # DRAFT: Multiple tokens
├── RFC-002-yield-reinvestment.md      # DISCUSSION: Yield compounding
└── RFC-003-registry-integration.md    # ACCEPTED: Discoverability
```

---

## Key Features

### 1. **Lightweight Process**

✅ Minimal bureaucracy  
✅ Async-friendly (no sync meetings required)  
✅ 3-day minimum review (1 week for major)  
✅ Single decision maker (not unanimous consensus)  

### 2. **Clear Lifecycle**

```
DRAFT → DISCUSSION → ACCEPTED → IMPLEMENTED → CLOSED
```

- **DRAFT:** Author drafts, shares with 3–5 stakeholders
- **DISCUSSION:** 3–7 day review period, feedback gathering
- **ACCEPTED:** Decision made, assigned to implementer
- **IMPLEMENTED:** Feature shipped, linked to ADR/commits
- **CLOSED:** Archived, successor linked if superseded

### 3. **Traceability**

RFC → ADR → Code → GitHub Issue  
All bidirectionally linked

Readers can trace:
- **Why** a feature exists (RFC: motivation + use cases)
- **What** was decided (ADR: decision + consequences)
- **How** it was built (PRs, commits)
- **When** it shipped (release notes, GitHub timeline)

### 4. **Knowledge Building**

- RFCs capture exploratory discussion before implementation
- ADRs record final decisions for long-term reference
- Combined, they form a **decision journal** showing evolution of ideas
- New team members learn process + context simultaneously

### 5. **Template-Driven**

TEMPLATE.md provides:
- All required sections
- Examples for each
- Guidance on detail level
- Checklist for acceptance criteria
- Rollout + monitoring structure

---

## RFC Lifecycle Example

### Real scenario: Multi-Token Support (RFC-001)

**T=0 (2026-07-27):**
- Author creates RFC-001: Multi-Token Support
- Status: DRAFT
- Shares with 5 stakeholders (platform lead, storage lead, security, integrations, ops)

**T=3 days (2026-07-30):**
- Stakeholders provide feedback:
  - Security: "How do we validate new tokens?" → RFC updated
  - Storage: "Migration plan for v6→v7?" → RFC updated
  - Integrations: "Can we query by token?" → Deferred to RFC-004

**T=7 days (2026-08-03):**
- Status: DISCUSSION
- Minimum review period complete
- All must-have feedback addressed

**T=10 days (2026-08-06):**
- Platform lead decides: **ACCEPTED**
- Status: ACCEPTED
- Assigned to implementer (developer team)
- Tracked in GitHub project "v2.0 Multi-Token"

**T=30–45 days (2026-08-27 – 2026-09-11):**
- Implementer builds feature
- PR references: "Implements RFC-001"
- Code comments link to RFC
- Implementation matches RFC-001 design

**T=50 days (2026-09-16):**
- Feature complete + tested
- RFC status updated: **IMPLEMENTED**
- ADR-009 created (from RFC-001 + implementation learnings)
- ADR links back to RFC-001

**T=60 days (2026-09-26):**
- Shipped in v2.0 release
- RFC status: **CLOSED** (or **ARCHIVED**)
- GitHub issue #847 closed
- Release notes link: RFC-001 → ADR-009 → PR #1001

**Full traceability chain established:**
- RFC-001 (proposal + discussion)
- ADR-009 (final decision)
- PR #1001 (implementation)
- Issue #847 (customer request)
- v2.0 release notes (when shipped)

---

## Design Principles

### 1. **RFC ≠ ADR**

| Aspect | RFC | ADR |
|--------|-----|-----|
| **When written** | Before implementation | After decision |
| **Scope** | Proposal + discussion | Final decision only |
| **Audience** | Team (discussion) | Public (reference) |
| **Approval** | Domain owners (simple majority) | Decision maker (single authority) |
| **Lifespan** | Temporary (until decision made) | Permanent (long-term reference) |

### 2. **Opt-In Complexity**

- Simple RFCs can be shorter (skip unnecessary sections)
- Complex RFCs need full detail (alternatives, rollout, monitoring)
- TEMPLATE.md provides structure; authors adjust as needed

### 3. **Async-Friendly**

- Written format (not meetings)
- Distributed review (each reviewer async)
- 3–7 day timeline (not urgent)
- GitHub PRs/discussions for all collaboration

### 4. **Decision Preservation**

- RFCs capture exploration (rejected alternatives, tradeoffs)
- ADRs record final decision (consequences, rationale)
- Together, they prevent **re-litigation** of old decisions
- New team members understand *why* decisions were made

---

## How to Use RFCs

### For Feature Authors

1. **Read** TEMPLATE.md and one example RFC (RFC-001 or RFC-002)
2. **Create** new file: `RFC-NNN-kebab-case-title.md`
3. **Fill** template with your proposal
4. **Share** PR with 3–5 stakeholders
5. **Iterate** based on feedback (DISCUSSION phase)
6. **Wait** for decision (ACCEPTED or rejected)
7. **Implement** and link to ADR + code

### For Reviewers

1. **Read** RFC top-to-bottom
2. **Check** against review checklist (in README.md)
3. **Comment** on:
   - Problem motivation (is it real?)
   - Design soundness (does it solve the problem?)
   - Tradeoffs (are alternatives explored?)
   - Implementation (is effort realistic?)
4. **Approve** or request changes

### For Team Leads

1. **Assign** RFCs to reviewers (3–5 per RFC)
2. **Set** review deadline (3–7 days)
3. **Collect** feedback
4. **Make** decision (ACCEPTED/REJECTED)
5. **Assign** to implementer if accepted
6. **Track** in roadmap/GitHub project

### For Integrators

1. **Browse** `/docs/rfc/` for planned features
2. **Read** RFCs in DISCUSSION/ACCEPTED status
3. **Provide** feedback if affected
4. **Plan** integration timelines

---

## Integration with Existing Processes

### RFC ← ADR ← Code

```
Existing Process (ADRs):
  └─ `docs/adr/` — Accepted architectural decisions

RFC Enhancement:
  └─ `docs/rfc/` — Proposals + discussion
       ↓ (if accepted)
       → ADR (final decision)
```

### RFC ← GitHub Issues ← PRs

```
GitHub Issues:
  ├─ Bug reports (no RFC needed)
  ├─ Feature requests (may generate RFC)
  └─ Discussions (may propose RFC)

RFC Process:
  └─ Takes feature requests → explores design → generates ADR
       ↑ (if approved)
       ← Links back to original issue
```

### RFC ← Architecture Docs

```
Architecture Diagrams (`docs/arch/`):
  └─ Auto-generated from code

RFC Enhancement:
  └─ Code comments link to RFC/ADR
       → Diagrams now include reference to design decisions
       → Readers can trace: diagram → code → RFC → ADR
```

---

## Expected Outcomes

### Short-term (Q3 2026)

- [ ] 2–3 RFCs in discussion (RFC-001, RFC-002, etc.)
- [ ] 1–2 RFCs accepted and in development
- [ ] Team familiar with process
- [ ] RFC-003 (Registry) ships in v1.4

### Medium-term (Q4 2026)

- [ ] 5–10 RFCs total (mix of DRAFT, DISCUSSION, ACCEPTED, CLOSED)
- [ ] First ADRs created from accepted RFCs
- [ ] Traceability chain established (RFC → ADR → code)
- [ ] Team considers RFC process normal

### Long-term (2027)

- [ ] RFC backlog reflects roadmap
- [ ] Integrators cite RFCs in feature requests
- [ ] RFC decision journal becomes institutional knowledge
- [ ] New team members learn by reading RFC history

---

## Metrics to Track

1. **RFC Velocity**
   - RFCs written per quarter
   - Review time (days from DRAFT to ACCEPTED)
   - Acceptance rate (% accepted vs. rejected)

2. **Decision Quality**
   - Rework rate (% of RFCs that require design changes during implementation)
   - Regression rate (% of implemented features needing backports/fixes)

3. **Knowledge Sharing**
   - Team members citing RFCs in discussions (% of decisions)
   - New team members reading RFC history (onboarding metric)
   - External integrators citing RFCs (ecosystem metric)

---

## Example Workflows

### Workflow 1: Simple Feature Request

```
Issue #999: "Support X"
  → Author creates RFC-004
  → DRAFT status, shares with team
  → 3 reviewers approve in 5 days
  → ACCEPTED (2026-08-15)
  → Implemented in v1.6
```

### Workflow 2: Controversial Design

```
Issue #1000: "Redesign settlement"
  → Multiple approaches proposed
  → RFC-005a vs RFC-005b (competing designs)
  → DISCUSSION phase extended (2 weeks)
  → Team chooses RFC-005b
  → RFC-005a marked CLOSED: Rejected
  → RFC-005b proceeds to ACCEPTED
```

### Workflow 3: Cross-team Feature

```
Issue #1001: "Add yield reinvestment"
  → RFC-002 written (product team)
  → Stakeholders: product, storage, security, integrations
  → Extended discussion (1 week, many comments)
  → Design modified based on feedback
  → ACCEPTED (2026-08-10)
  → Assigned to backend team
  → Shipped in v1.5 (2026-10-01)
  → ADR-010 created from RFC-002
```

---

## Next Steps

1. **Publish RFCs** (this is done ✓)
   - docs/rfc/ folder created
   - TEMPLATE.md available
   - 3 examples provided

2. **Announce process** to team
   - Slack notification with link to README.md
   - Demo during team meeting (optional)
   - Add to onboarding docs

3. **Start using RFCs**
   - Identify 2–3 features to propose as RFCs
   - Schedule first RFC review (week of 2026-08-05)
   - Collect feedback and iterate process

4. **Link existing ADRs** (optional)
   - ADR-001 through ADR-008 already exist
   - Can retroactively create RFCs if desired (for historical context)
   - Not required; forward-facing RFCs are priority

5. **Monitor and adjust**
   - After first 5–10 RFCs, assess process
   - Adjust timeline, complexity, or sections as needed
   - Publish learnings

---

## Files Checklist

✅ **Process Documentation**
- [x] `docs/rfc/README.md` — Main process guide
- [x] `docs/rfc/TEMPLATE.md` — RFC template
- [x] `docs/rfc/LINKING_GUIDE.md` — How to connect RFCs/ADRs/issues

✅ **Examples**
- [x] `docs/rfc/RFC-001-multi-token-support.md` — DRAFT example (high complexity)
- [x] `docs/rfc/RFC-002-yield-reinvestment.md` — DISCUSSION example (medium complexity)
- [x] `docs/rfc/RFC-003-registry-integration.md` — ACCEPTED example (low complexity)

✅ **Summary**
- [x] `RFC_PROCESS_COMPLETE.md` — This file

---

## Quick Reference

| Need | File |
|------|------|
| **Understand the process** | `docs/rfc/README.md` |
| **Write a new RFC** | `docs/rfc/TEMPLATE.md` |
| **Link RFCs to ADRs/issues** | `docs/rfc/LINKING_GUIDE.md` |
| **See a DRAFT RFC** | `docs/rfc/RFC-001-multi-token-support.md` |
| **See a DISCUSSION RFC** | `docs/rfc/RFC-002-yield-reinvestment.md` |
| **See an ACCEPTED RFC** | `docs/rfc/RFC-003-registry-integration.md` |

---

## FAQ

**Q: Should I write an RFC for every feature?**  
A: No. Use for major features, design changes, or anything requiring discussion. Skip for bug fixes, minor optimizations.

**Q: How do I know when to go from DRAFT → DISCUSSION?**  
A: When RFC is complete and ready for review (all sections filled, examples included).

**Q: Who decides ACCEPTED vs. REJECTED?**  
A: The owner/lead for the domain (specified in RFC header). One person, not consensus.

**Q: Can I change an RFC after it's ACCEPTED?**  
A: Yes, but update the status to reflect changes (e.g., ACCEPTED v1 → v2). Document what changed and why.

**Q: What if I disagree with an accepted RFC?**  
A: Document your disagreement in the RFC. Final decision still stands. Future RFC can propose alternative.

**Q: Do old RFCs stay in docs/rfc/ forever?**  
A: Yes. They're permanent record of decision-making. Mark CLOSED, but don't delete.

---

## Summary

✅ **Lightweight RFC process established** for design decisions  
✅ **3 example RFCs** provided (DRAFT, DISCUSSION, ACCEPTED)  
✅ **Linking guide** shows how to connect RFCs ↔ ADRs ↔ code  
✅ **Clear lifecycle** (DRAFT → DISCUSSION → ACCEPTED → IMPLEMENTED → CLOSED)  
✅ **Async-friendly** (no sync meetings required)  
✅ **Knowledge building** (decision journal for team + new hires)  

**Ready to use!** Start with [docs/rfc/README.md](docs/rfc/README.md)

