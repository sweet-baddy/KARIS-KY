# Linking RFCs, ADRs, and GitHub Issues

This guide explains how to establish bidirectional links between RFCs, Architecture Decision Records (ADRs), and GitHub issues to build institutional knowledge and traceability.

---

## Overview

**Goal:** Create a web of connected design decisions so that:
- Readers understand how a feature evolved from proposal → design → implementation
- Maintainers can trace why a decision was made
- New team members learn the decision-making process

**Three document types:**

| Type | Purpose | Timeline | Example |
|------|---------|----------|---------|
| **RFC** | Proposed feature/change requiring discussion | Before implementation | RFC-001: Multi-Token Support |
| **ADR** | *Accepted* architectural decision now implemented | After decision made | ADR-009: Multi-Token Settlement |
| **GitHub Issue** | Bug report, task, or discussion thread | Anytime | #847: Support EURC settlement |

---

## Link Patterns

### Pattern 1: RFC → ADR (After Acceptance + Implementation)

**When:** Feature accepted and implemented

**RFC header update:**
```markdown
# RFC-001: Multi-Token Support

**Status:** IMPLEMENTED  
**Implemented as:** [ADR-009: Multi-Token Settlement](../adr/ADR-009-multi-token-settlement.md)
```

**ADR header:**
```markdown
# ADR-009: Multi-Token Settlement Model

**Based on RFC:** [RFC-001: Multi-Token Support](../rfc/RFC-001-multi-token-support.md)
```

**Why this pattern:**
- RFC captures exploratory discussion
- ADR records *final* decision + implementation constraints
- Readers can trace: idea → discussion → decision → code

---

### Pattern 2: RFC ↔ GitHub Issue (Bidirectional)

**When:** RFC references a GitHub issue, or issue links to RFC

**In RFC:**
```markdown
**Related:** 
- Issue #847: [Support EURC in addition to USDC](https://github.com/karis-ky/escrow-contracts/issues/847)
```

**In GitHub Issue:**
```
Fixes: [RFC-001: Multi-Token Support](../docs/rfc/RFC-001-multi-token-support.md)
```

**Why bidirectional:**
- Issue tracker is primary communication channel
- RFC provides formal proposal + design details
- Easy to jump between contexts

---

### Pattern 3: ADR ↔ RFC (If RFC Pre-Dated ADR)

**When:** Historical decision already has RFC

**In ADR:**
```markdown
# ADR-005: Tiered Yield

**Related RFC:** [RFC-002: Yield Reinvestment](../rfc/RFC-002-yield-reinvestment.md)
```

**In RFC:**
```markdown
**Related:** 
- ADR-005: [Tiered Yield](../adr/ADR-005-tiered-yield.md)
```

---

### Pattern 4: Cross-RFC References

**When:** RFCs are related or build on each other

**In RFC-001:**
```markdown
**Related:** RFC-003 (Registry Integration) — may enable discovery for multi-token escrows
```

**In RFC-003:**
```markdown
**Related:** RFC-001 (Multi-Token Support) — registry queries return token information
```

---

### Pattern 5: Issue → Multiple RFCs (Epic)

**When:** Large feature broken into multiple RFCs

**GitHub Issue #847 (Epic):**
```markdown
# Multi-Token Support Epic

## Related RFCs:
- [RFC-001: Multi-Token Escrow Design](../docs/rfc/RFC-001-multi-token-support.md)
- [RFC-004: Multi-Token Registry Query](../docs/rfc/RFC-004-registry-queries.md)
- [RFC-005: Settlement Token Conversion](../docs/rfc/RFC-005-token-conversion.md)

## Tracking:
- [x] RFC-001 approved (2026-08-10)
- [ ] RFC-004 in discussion (2026-08-13)
- [ ] RFC-005 in draft (2026-08-15)
```

---

## Linking in Code Comments

When implementing a feature, reference the RFC/ADR in code:

```rust
/// Enable multi-token settlement (RFC-001, ADR-009).
/// See: docs/rfc/RFC-001-multi-token-support.md
pub fn settle(
    env: Env,
    settlement_token: Option<Address>,
) -> Result<EscrowSettled, EscrowError> {
    // ... implementation ...
}
```

**In commit messages:**
```
feat: implement multi-token settlement (RFC-001, ADR-009)

Implements the design from RFC-001: Multi-Token Support
and ADR-009: Multi-Token Settlement Model.

Closes #847
```

**In PR description:**
```markdown
## Summary
Adds settlement token selection to escrow contract.

## Related
- RFC: [RFC-001: Multi-Token Support](../docs/rfc/RFC-001-multi-token-support.md)
- ADR: [ADR-009: Multi-Token Settlement](../docs/adr/ADR-009-multi-token-settlement.md)
- Issue: #847

## Testing
- Unit tests for token validation
- Integration tests for multi-token settlement flow
- Backward compat tests (v1.3 escrows settle in funding token only)
```

---

## Linking in Documentation

### In Contract README

Link relevant RFCs/ADRs:

```markdown
## Multi-Token Settlement

The contract supports settlement in multiple tokens (Stellar stablecoins).
See [RFC-001: Multi-Token Support](../docs/rfc/RFC-001-multi-token-support.md)
and [ADR-009](../docs/adr/ADR-009-multi-token-settlement.md).

### Limitations (v1.0)
- No automatic conversion between tokens (same-token settlement only)
- See [RFC-005: Settlement Token Conversion](../docs/rfc/RFC-005-token-conversion.md)
  for future enhancements
```

### In API Documentation

```markdown
### settle(settlement_token: Option<Address>) -> Result<EscrowSettled>

Select a settlement token from the allowlist.

**Design:** [RFC-001: Multi-Token Support](../../docs/rfc/RFC-001-multi-token-support.md)
**Implementation:** [ADR-009: Multi-Token Settlement](../../docs/adr/ADR-009-multi-token-settlement.md)

**Parameters:**
- `settlement_token`: Token address from allowlist (None = funding token)

**Errors:**
- Error 42: Token not in allowlist (see RFC-001 §Design)
```

---

## Maintaining Links

### When RFC is Accepted

1. **Update RFC header:**
   ```markdown
   **Status:** ACCEPTED
   **Date Accepted:** YYYY-MM-DD
   ```

2. **Start ADR (if warranted):**
   - Copy RFC into ADR template
   - Add RFC reference
   - Refactor for final decision format

3. **Link both:**
   - RFC → ADR
   - ADR → RFC

### When RFC is Implemented

1. **Update RFC:**
   ```markdown
   **Status:** IMPLEMENTED
   **Implemented as:** [ADR-009: ...](../adr/ADR-009-...md)
   **PR:** escrow-contracts#1001
   ```

2. **Add to archive:**
   - RFC remains in `/docs/rfc/` (permanent record)
   - Mark status clearly so readers know it's historical

### When RFC is Rejected/Superseded

1. **Mark status:**
   ```markdown
   **Status:** CLOSED: Rejected
   **Reason:** Alternative approach chosen (see ADR-011)
   ```

2. **Link successor:**
   ```markdown
   **Superseded by:** [ADR-011: ...](../adr/ADR-011-...md)
   ```

### When ADR Changes

1. **Add ADR revision note:**
   ```markdown
   # ADR-009: Multi-Token Settlement

   **Revisions:**
   - v1 (2026-07-15): Initial design
   - v2 (2026-08-01): Added oracle support (see RFC-005)
   ```

2. **Update RFC if needed:**
   - Link to revised ADR
   - Add note if design changed significantly

---

## Searching Links

### Finding all RFCs related to a topic

```bash
# Search for "token" across all RFCs
grep -r "token" docs/rfc/ --include="*.md"

# Find all linked ADRs
grep -r "ADR-" docs/rfc/ --include="*.md"

# Find all open RFCs
grep -r "Status.*DRAFT\|DISCUSSION" docs/rfc/ --include="*.md"
```

### Finding all ADRs referencing an RFC

```bash
grep -r "RFC-001" docs/adr/ --include="*.md"
```

### Finding GitHub issues linked to RFCs

```bash
# In GitHub:
# Search: "RFC-001" in repo (returns issues that mention RFC)
# Search: is:issue "RFC-001"
```

---

## Best Practices

1. **Use full paths:** Include relative path in links so they work across contexts
   - ✓ `[RFC-001](../rfc/RFC-001-multi-token-support.md)`
   - ✗ `[RFC-001](RFC-001-multi-token-support.md)` (breaks from other directories)

2. **Bidirectional links:** If A → B, ensure B → A
   - RFC → ADR
   - ADR → RFC
   - Both → GitHub issue

3. **Timestamp links:** Use dates to understand decision timeline
   - "Proposed 2026-07-27"
   - "Accepted 2026-08-10"
   - "Implemented 2026-09-01"

4. **Status visibility:** Always mark RFC/ADR status clearly
   - `**Status:** DRAFT | DISCUSSION | ACCEPTED | IMPLEMENTED | CLOSED`

5. **Link in code:** Add comments referencing RFC/ADR for major features
   - Helps future maintainers understand *why* code exists

6. **Keep links fresh:** When documents change, update references
   - Stale links undermine knowledge sharing

---

## Example: Full Traceability

**User journey:** Developer encounters multi-token code, wants to understand design

1. **Finds code comment:**
   ```rust
   // Multi-token settlement (RFC-001, ADR-009)
   ```

2. **Reads RFC-001:**
   - Understands problem + alternatives
   - Sees it's IMPLEMENTED

3. **Follows to ADR-009:**
   - Sees final decision + consequences
   - Understands why specific approach chosen

4. **Checks GitHub issue #847:**
   - Sees customer request that motivated feature
   - Finds PRs that implemented it
   - Sees when it shipped (v1.4)

5. **Digs into code:**
   - Understands purpose + constraints
   - Can make informed modifications

**All connected by links!**

---

## FAQ

**Q: Should every RFC become an ADR?**  
A: No. Simple RFCs might not warrant formal ADR. Only upgrade if decision is architectural + long-term. Rejected RFCs stay as RFCs.

**Q: Can I link to external docs?**  
A: Yes, but prefer internal links (more stable). External links: use full URL + add note if link breaks.

**Q: What if an RFC references future RFCs?**  
A: Link as TBD with note: "Future RFC (planned): Token conversion oracle". Update when RFC written.

**Q: How do I search GitHub issues for RFC mentions?**  
A: Use GitHub search: `is:issue RFC-001` or `is:pr RFC-001`

**Q: Can ADRs reference RFCs that came after?**  
A: Yes, but note it: "RFC-002 (written later) explores related extension". This captures historical record.

