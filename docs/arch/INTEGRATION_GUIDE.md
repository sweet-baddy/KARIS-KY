# Integration Guide: Using Auto-Generated Architecture Docs

This guide explains how to leverage the auto-generated architecture documentation for development, auditing, and integration work.

---

## What Was Generated

### Mermaid Diagrams (Markdown-embedded)
Located in `docs/arch/`:

1. **[state-machine.md](state-machine.md)** — Escrow lifecycle (5 states, 4 transitions)
2. **[data-flow.md](data-flow.md)** — Authorization, storage, token, and event paths
3. **[entity-relationships.md](entity-relationships.md)** — Data model structure
4. **[module-structure.md](module-structure.md)** — Code organization
5. **[entrypoint-matrix.md](entrypoint-matrix.md)** — Role-based API surface

### PlantUML Diagrams (can be exported to SVG/PDF)
Located in `docs/arch/plantuml/`:

1. **storage-model.puml** — Class diagram of DataKey catalog
2. **funding-sequence.puml** — Sequence diagram: Investor → Contract → Token
3. **settlement-sequence.puml** — Sequence diagram: SME settlement → settlement
4. **usecases.puml** — Use-case diagram by role (Admin, SME, Investor, Treasury)
5. **components.puml** — Component architecture

### Reference Tables (Markdown)
Located in `docs/arch/storage-reference.md`:

- Storage layout (29 DataKey variants)
- Contract types (InvoiceEscrow, SmeCollateralCommitment, etc.)
- Entrypoint catalog (67 public functions)

---

## How to Use These Docs

### For Integrators Building Clients

**Start here:**
1. Read [ARCHITECTURE.md](ARCHITECTURE.md) for overview
2. Study [entrypoint-matrix.md](entrypoint-matrix.md) to find your role's functions
3. Review [data-flow.md](data-flow.md) for authorization sequence
4. Reference [storage-reference.md](storage-reference.md) for types

**Example:** Building an Investor SDK
- Find investor entrypoints in [entrypoint-matrix.md](entrypoint-matrix.md)
- See funding sequence in `plantuml/funding-sequence.puml`
- Check `InvoiceEscrow` struct fields in [storage-reference.md](storage-reference.md)
- Implement error handling for typed error codes 36–41 (token issues)

### For Smart Contract Auditors

**Checklist:**
1. Review [state-machine.md](state-machine.md) — verify forward-only transitions
2. Examine [data-flow.md](data-flow.md) — trace authorization guards
3. Compare `docs/arch/` with `docs/adr/` — ensure decisions are implemented
4. Check [storage-reference.md](storage-reference.md) for storage layout
5. Validate that all public functions appear in [entrypoint-matrix.md](entrypoint-matrix.md)

**Key audit points:**
- ✓ No backward transitions (e.g., WITHDRAWN → FUNDED)
- ✓ Auth gates always before storage writes
- ✓ Per-investor keys in persistent storage (v6)
- ✓ Legal hold blocks all risk-bearing transitions
- ✓ Terminal dust sweep only in terminal states

### For Backend/Indexer Development

**Key diagrams:**
- [state-machine.md](state-machine.md) — status transitions to listen for
- [entity-relationships.md](entity-relationships.md) — data model for schema design
- `plantuml/funding-sequence.puml` — event emission order
- `plantuml/settlement-sequence.puml` — complex multi-step flow

**Example:** Building an event indexer
- Watch for `EscrowInitialized` → status 0 (OPEN)
- Watch for `EscrowFunded` → status 1 (FUNDED)
- Watch for `EscrowSettled` → status 2 (SETTLED)
- Index `InvestorContribution(Address)` persistent keys
- Alert if legal hold is set

### For Developers Modifying the Contract

**Before making changes:**
1. Run `python3 scripts/generate_architecture_docs.py` to get baseline
2. Make code changes
3. Run again and commit `docs/arch/` changes
4. Verify diagrams still match intent (especially [state-machine.md](state-machine.md))

**If adding a new entrypoint:**
- Add doc comment with `/// Admin: ...` or `/// SME: ...` tag
- Run generator
- Check that it appears in [entrypoint-matrix.md](entrypoint-matrix.md) and correct role
- Update [ARCHITECTURE.md](ARCHITECTURE.md) key metrics if significant

**If adding a DataKey variant:**
- Add detailed doc comment with storage type (instance/persistent) and indexed-by info
- Run generator
- Verify it appears in [storage-reference.md](storage-reference.md)
- Check that [entity-relationships.md](entity-relationships.md) still makes sense

---

## Exporting Diagrams to PDF/SVG

The PlantUML files in `docs/arch/plantuml/` can be converted to vector graphics:

### Using PlantUML CLI

```bash
# Install PlantUML (if not present)
sudo apt-get install plantuml

# Convert all .puml files to SVG
plantuml -tsvg docs/arch/plantuml/*.puml

# Convert to PDF
plantuml -tpdf docs/arch/plantuml/*.puml

# Convert to PNG (rasterized)
plantuml -tpng docs/arch/plantuml/*.puml
```

### Using Online Renderer

1. Copy contents of a `.puml` file
2. Paste at http://www.plantuml.com/plantuml/uml/
3. Download as SVG/PNG from "Export" menu

### Embedding in Documentation

**In Markdown:**
```markdown
![Storage Model](docs/arch/plantuml/storage-model.svg)
```

**In LaTeX/PDF:**
```latex
\includegraphics[width=0.8\textwidth]{docs/arch/plantuml/storage-model.pdf}
```

---

## Regenerating After Code Changes

### When to Regenerate

- **Always before PR merge** — keeps docs in sync
- **After adding/removing entrypoints**
- **After changing DataKey variants**
- **After modifying struct fields**
- **Before major releases**

### How to Regenerate

```bash
cd /workspaces/KARIS-KY

# Generate Mermaid diagrams (embedded in Markdown)
python3 scripts/generate_architecture_docs.py --output-dir docs/arch

# Generate PlantUML diagrams (exportable to SVG/PDF)
python3 scripts/arch_to_plantuml.py --output-dir docs/arch/plantuml

# Export PlantUML to SVG (requires plantuml CLI)
plantuml -tsvg docs/arch/plantuml/*.puml

# Commit all changes
git add docs/arch/
git commit -m "docs: regenerate architecture diagrams"
```

### What Gets Regenerated

- ✓ DataKey catalog (from enum variants + doc comments)
- ✓ Contract types (from #[contracttype] structs)
- ✓ Entrypoints (from public fn items)
- ✓ State transitions (hardcoded from ADR-001)
- ✓ Module graph (hardcoded module list)

### What Stays Manual

- ✗ ADR index in [ARCHITECTURE.md](ARCHITECTURE.md)
- ✗ Text descriptions and interpretations
- ✗ Integration examples and use cases
- ✗ Operational procedures and deployment steps

---

## FAQ

### Q: Why is my new entrypoint not appearing in the diagram?

**A:** The generator uses simple regex to find public functions. Ensure:
- Function is declared as `pub fn name(...) {`
- Function has a doc comment (e.g., `/// Admin: ...`)
- Function is in `impl LiquifactEscrow` block

If still missing, manually add it to [entrypoint-matrix.md](entrypoint-matrix.md).

### Q: Can I edit the diagrams manually?

**A:** Yes, but regenerating will overwrite them. Best practice:
- Edit in code (add doc comments)
- Regenerate
- Only manually edit if generator has gaps (then file an issue)

### Q: How do I add a new DataKey variant?

**A:** 
1. Add to `DataKey` enum with detailed doc comment:
   ```rust
   /// Per-investor refund status. Persistent storage. Absent ⇒ false.
   InvestorRefunded(Address),
   ```
2. Run generator
3. Verify in [storage-reference.md](storage-reference.md)

### Q: What if the state machine changes?

**A:** Edit `get_state_transitions()` in `scripts/generate_architecture_docs.py`:
```python
def get_state_transitions(self) -> List[Tuple[int, int, str]]:
    return [
        (0, 1, "fund: target_met"),
        (1, 2, "settle: maturity_passed"),
        # ... etc
    ]
```

### Q: Can these docs be version-controlled?

**A:** Yes! They're plain Markdown and PlantUML text:
```bash
git add docs/arch/
git commit -m "docs: regenerate architecture after v6 migration"
```

Diffs show exactly what changed in the API surface or storage model.

---

## Tools and Commands Reference

### Generate All Docs
```bash
python3 scripts/generate_architecture_docs.py --output-dir docs/arch
python3 scripts/arch_to_plantuml.py --output-dir docs/arch/plantuml
```

### View Mermaid Diagrams
- GitHub: Renders automatically in Markdown files
- Local editor: Use VS Code extension "Markdown Preview Mermaid Support"
- Online: https://mermaid.live

### Export PlantUML
```bash
plantuml -tsvg docs/arch/plantuml/storage-model.puml
plantuml -tpdf docs/arch/plantuml/components.puml
```

### Validate Diagrams
```bash
# Check PlantUML syntax
plantuml -check docs/arch/plantuml/*.puml
```

---

## Related Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — Overview and key metrics
- **[docs/adr/](../adr/)** — Architecture Decision Records
- **[docs/OPERATOR_RUNBOOK.md](../OPERATOR_RUNBOOK.md)** — Deployment procedures
- **[escrow/README.md](../../escrow/README.md)** — Contract development
- **[docs/escrow-security-checklist.md](../escrow-security-checklist.md)** — Audit checklist

---

## Contributing Improvements

If you find gaps in auto-generation:

1. **Document the gap** — what's missing or inaccurate?
2. **Propose a fix** in `scripts/generate_architecture_docs.py`
3. **Test** the regeneration
4. **Submit PR** with regenerated docs

Example enhancements:
- Extract authorization role tags from doc comments more robustly
- Add call-graph analysis (which functions call which)
- Generate OpenAPI/GraphQL schema docs
- Add performance/gas cost annotations to entrypoints

