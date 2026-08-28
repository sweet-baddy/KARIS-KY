# Architecture Documentation Generation — Complete Summary

**Date Generated:** July 27, 2026  
**Tool:** `scripts/generate_architecture_docs.py` + `scripts/arch_to_plantuml.py`  
**Output:** `docs/arch/` with Mermaid and PlantUML diagrams

---

## What Was Delivered

### 📊 Visual Diagrams

| Diagram | Format | Purpose | Location |
|---------|--------|---------|----------|
| **State Machine** | Mermaid + PlantUML | 5 states, forward-only transitions | [state-machine.md](docs/arch/state-machine.md) |
| **Data Flow** | Mermaid | Auth, storage, token, event paths | [data-flow.md](docs/arch/data-flow.md) |
| **Entity Relationships** | Mermaid ER | Data model structure | [entity-relationships.md](docs/arch/entity-relationships.md) |
| **Module Structure** | Mermaid | Code organization | [module-structure.md](docs/arch/module-structure.md) |
| **Entrypoint Matrix** | Mermaid | Role-based API surface | [entrypoint-matrix.md](docs/arch/entrypoint-matrix.md) |
| **Funding Sequence** | PlantUML | Investor → Contract → Token flow | [funding-sequence.puml](docs/arch/plantuml/funding-sequence.puml) |
| **Settlement Sequence** | PlantUML | SME settlement process | [settlement-sequence.puml](docs/arch/plantuml/settlement-sequence.puml) |
| **Use Cases** | PlantUML | 4 roles × ~4 entrypoints each | [usecases.puml](docs/arch/plantuml/usecases.puml) |
| **Storage Model** | PlantUML | DataKey class diagram | [storage-model.puml](docs/arch/plantuml/storage-model.puml) |
| **Components** | PlantUML | Lib, validation, external_calls | [components.puml](docs/arch/plantuml/components.puml) |

### 📑 Reference Documents

| Document | Content | Audience |
|----------|---------|----------|
| [README.md](docs/arch/README.md) | Navigation guide, regeneration instructions | Everyone |
| [ARCHITECTURE.md](docs/arch/ARCHITECTURE.md) | Overview, key metrics, ADR index | Architecture review |
| [INTEGRATION_GUIDE.md](docs/arch/INTEGRATION_GUIDE.md) | How to use docs, export diagrams, common tasks | Integrators, developers |
| [storage-reference.md](docs/arch/storage-reference.md) | 29 DataKey variants, 6 contract types, 67 entrypoints | Implementers, auditors |

### 🔧 Generator Tools

| Tool | Purpose | Usage |
|------|---------|-------|
| [generate_architecture_docs.py](scripts/generate_architecture_docs.py) | Extract code + generate Mermaid diagrams | `python3 scripts/generate_architecture_docs.py` |
| [arch_to_plantuml.py](scripts/arch_to_plantuml.py) | Generate PlantUML diagrams | `python3 scripts/arch_to_plantuml.py` |

---

## Key Metrics Captured

```
✓ 29 DataKey variants (instance + persistent storage)
✓ 6 contract types (InvoiceEscrow, SmeCollateralCommitment, etc.)
✓ 67 public entrypoints (admin, SME, investor, treasury, public roles)
✓ 5 state codes (0=OPEN, 1=FUNDED, 2=SETTLED, 3=WITHDRAWN, 4=CANCELLED)
✓ 4 authorization roles (admin, SME, investor, treasury)
✓ 8 Architecture Decision Records (ADR-001 through ADR-008)
```

---

## File Structure

```
docs/arch/
├── README.md                      # Navigation + regeneration guide
├── ARCHITECTURE.md                # Overview & key metrics
├── INTEGRATION_GUIDE.md           # How to use these docs
├── storage-reference.md           # Detailed catalog (Markdown table)
│
├── state-machine.md               # Status transitions (Mermaid stateDiagram-v2)
├── data-flow.md                   # Auth/storage/token paths (Mermaid graph)
├── entity-relationships.md        # Data model (Mermaid ER)
├── module-structure.md            # Code org (Mermaid graph)
├── entrypoint-matrix.md           # API surface (Mermaid graph)
│
└── plantuml/                      # Exportable to SVG/PDF/PNG
    ├── storage-model.puml         # DataKey class diagram
    ├── funding-sequence.puml      # Investor → Contract → Token
    ├── settlement-sequence.puml   # SME settlement flow
    ├── usecases.puml              # 4 roles × entrypoints
    └── components.puml            # lib.rs, external_calls.rs, etc.
```

---

## How These Docs Support Different Workflows

### 👨‍💻 Smart Contract Developer

**Use case:** Implementing new features or fixing bugs

**Start with:** [module-structure.md](docs/arch/module-structure.md)
1. Understand which file owns which responsibility
2. Review [state-machine.md](docs/arch/state-machine.md) to understand transitions
3. Check [storage-reference.md](docs/arch/storage-reference.md) for related DataKey variants
4. Add doc comments to new code (auto-extracted on regeneration)
5. Run `python3 scripts/generate_architecture_docs.py` before committing

### 🔌 Integration Engineer (SDK/Client)

**Use case:** Building a wallet, dApp, or backend service

**Start with:** [INTEGRATION_GUIDE.md](docs/arch/INTEGRATION_GUIDE.md)
1. Identify your role in [entrypoint-matrix.md](docs/arch/entrypoint-matrix.md)
2. Study the sequence diagram (funding or settlement)
3. Review [storage-reference.md](docs/arch/storage-reference.md) for types to serialize
4. Check [data-flow.md](docs/arch/data-flow.md) for authorization order
5. Reference [ARCHITECTURE.md](docs/arch/ARCHITECTURE.md) for error codes

**Investor SDK example:**
```
entrypoint-matrix.md → find Investor functions
  → funding-sequence.puml → see exact call order
  → storage-reference.md → get InvoiceEscrow & types
  → ARCHITECTURE.md → review typed errors (36-41 for token issues)
  → code against exported types
```

### 🔍 Security Auditor

**Use case:** Code review, threat modeling, compliance check

**Checklist:**
- [ ] [state-machine.md](docs/arch/state-machine.md) — verify no backward transitions
- [ ] [data-flow.md](docs/arch/data-flow.md) — check auth guards before writes
- [ ] [entity-relationships.md](docs/arch/entity-relationships.md) — validate data model
- [ ] [storage-reference.md](docs/arch/storage-reference.md) — confirm storage bounds
- [ ] [ARCHITECTURE.md](docs/arch/ARCHITECTURE.md) — trace ADR implementations
- [ ] `plantuml/funding-sequence.puml` — verify transfer safety
- [ ] `plantuml/components.puml` — check module boundaries

**Key validation points:**
```
✓ Legal hold always blocks risk-bearing transitions
✓ Per-investor storage is persistent (bounded per instance cap)
✓ Token transfers include pre/post balance equality checks
✓ All state mutations require auth before storage write
✓ No function can revert to earlier state
```

### 📊 Operations/DevOps

**Use case:** Deployment, monitoring, upgrades

**Start with:** [ARCHITECTURE.md](docs/arch/ARCHITECTURE.md)
1. Review schema version (current: 6)
2. Check ADR-007 if upgrading from v5
3. Use [state-machine.md](docs/arch/state-machine.md) to monitor transitions
4. Reference [entrypoint-matrix.md](docs/arch/entrypoint-matrix.md) for rate-limiting configuration
5. See `docs/OPERATOR_RUNBOOK.md` for detailed procedures

### 📚 Documentation Writer

**Use case:** Maintaining user guides, API docs

**Extract from:** [storage-reference.md](docs/arch/storage-reference.md), [entrypoint-matrix.md](docs/arch/entrypoint-matrix.md)
- Copy entrypoint list and signatures
- Link to sequence diagrams for visual explanation
- Reference storage layout for data type documentation
- Use state machine diagram in user guides

**Example:** "After an investor calls `fund()`, check [state-machine.md](docs/arch/state-machine.md) to see what status becomes 1 (FUNDED)"

---

## Workflow: Keeping Docs in Sync

### On Each Code Change

```bash
# 1. Make code changes
vim escrow/src/lib.rs

# 2. Regenerate docs
python3 scripts/generate_architecture_docs.py --output-dir docs/arch
python3 scripts/arch_to_plantuml.py --output-dir docs/arch/plantuml

# 3. Review changes
git diff docs/arch/

# 4. Commit
git add docs/arch/
git commit -m "docs: regenerate after adding feature_X"

# 5. (Optional) Export to PDF for stakeholders
plantuml -tsvg docs/arch/plantuml/*.puml
```

### What Auto-Extracts

These are generated from code:
- ✓ DataKey enum variants (from `pub enum DataKey`)
- ✓ Contract structs (from `#[contracttype]` items)
- ✓ Public entrypoints (from `pub fn` in `impl LiquifactEscrow`)
- ✓ Status transitions (hardcoded from ADR-001)
- ✓ Module organization (hardcoded from file structure)

What stays manual:
- ✗ Doc comments (you write these; generator preserves them)
- ✗ ADR index (manually maintained in ARCHITECTURE.md)
- ✗ Examples and use cases
- ✗ Operational procedures (in docs/OPERATOR_RUNBOOK.md)

---

## Example: Adding a New Entrypoint

**Scenario:** Adding `emergency_pause()` entrypoint for admin.

### Step 1: Write Code
```rust
#[contractimpl]
impl LiquifactEscrow {
    /// Admin: Pause all funding and settlement (compliance gate).
    /// Emergency action; can be cleared by admin or hold clearance.
    pub fn emergency_pause(env: Env) -> Result<(), EscrowError> {
        let escrow = Self::get_escrow(&env)?;
        escrow.admin.require_auth();
        
        // ... implementation ...
    }
}
```

### Step 2: Regenerate
```bash
python3 scripts/generate_architecture_docs.py --output-dir docs/arch
```

### Step 3: Verify in Docs
- ✓ Appears in [entrypoint-matrix.md](docs/arch/entrypoint-matrix.md) under Admin role
- ✓ [storage-reference.md](docs/arch/storage-reference.md) updated with new entrypoint list
- ✓ [ARCHITECTURE.md](docs/arch/ARCHITECTURE.md) key metrics updated (now 68 entrypoints)

### Step 4: Commit
```bash
git add docs/arch/
git commit -m "docs: add emergency_pause to architecture docs"
```

---

## Exporting Diagrams to PDF/SVG

All PlantUML files can be exported to visual formats:

```bash
# Install PlantUML (Ubuntu/Debian)
sudo apt-get install plantuml

# Export all to SVG (web-friendly, scalable)
plantuml -tsvg docs/arch/plantuml/*.puml

# Export all to PDF (print-friendly)
plantuml -tpdf docs/arch/plantuml/*.puml

# Export one to PNG (rasterized for presentations)
plantuml -tpng docs/arch/plantuml/storage-model.puml
```

**Output locations:**
```
docs/arch/plantuml/
├── storage-model.svg          # Vector graphic
├── storage-model.pdf          # Print-ready
├── funding-sequence.svg
├── funding-sequence.pdf
├── settlement-sequence.svg
└── ...
```

**Embed in Markdown:**
```markdown
![Storage Model](docs/arch/plantuml/storage-model.svg)
```

**Embed in LaTeX:**
```latex
\includegraphics[width=0.9\textwidth]{docs/arch/plantuml/storage-model.pdf}
```

---

## Troubleshooting

### Q: Diagram not rendering on GitHub?

**A:** Mermaid support varies by GitHub version. Fallback:
1. Export PlantUML to SVG: `plantuml -tsvg docs/arch/plantuml/*.puml`
2. Embed SVG in README: `![Name](docs/arch/plantuml/name.svg)`

### Q: New function not appearing in entrypoint-matrix.md?

**A:** Check:
- Is it declared `pub fn name(...) {` (not private)?
- Does it have a doc comment `/// ...`?
- Is it in the `#[contractimpl]` block?

If still missing, manually add to [entrypoint-matrix.md](docs/arch/entrypoint-matrix.md).

### Q: PlantUML installation fails?

**A:** Use online renderer instead:
1. Copy `.puml` file contents
2. Paste at http://www.plantuml.com/plantuml/uml/
3. Export from browser

### Q: Storage reference shows old data?

**A:** Regenerate and check commit log:
```bash
python3 scripts/generate_architecture_docs.py --output-dir docs/arch
git diff docs/arch/storage-reference.md
```

---

## Next Steps

### For Users of This Repository

1. **Bookmark:** [ARCHITECTURE.md](docs/arch/ARCHITECTURE.md) — your entry point
2. **Explore:** Use [README.md](docs/arch/README.md) to navigate by use case
3. **Reference:** Link [entrypoint-matrix.md](docs/arch/entrypoint-matrix.md) in your SDK/client docs
4. **Export:** Generate SVG/PDF for presentations: `plantuml -tsvg docs/arch/plantuml/*.puml`

### For Maintainers

1. **On every PR:** Regenerate and commit `docs/arch/` changes
2. **Before releases:** Export PlantUML to SVG for stakeholders
3. **On migrations:** Update state transitions in `get_state_transitions()` if needed
4. **For gaps:** Enhance extractors in `scripts/generate_architecture_docs.py`

### For Contributors

1. **Write good doc comments:**
   ```rust
   /// Admin: Set legal hold for compliance. Blocks settlement, withdrawal, and claims.
   ```
2. **Follow naming conventions:**
   - DataKey variants: `PascalCase`
   - Entrypoints: `snake_case`
   - Structs: `PascalCase`
3. **Run generator before committing** to keep docs fresh

---

## Summary

✅ **Generated:** 14 documentation files (10 Markdown, 5 PlantUML)  
✅ **Extracted:** 29 storage keys, 6 types, 67 entrypoints  
✅ **Diagrams:** State machine, data flow, ER, module graph, 5 sequences/usecases  
✅ **Tools:** Regenerable from code; synced to source  
✅ **Format:** Mermaid (GitHub-native) + PlantUML (exportable to PDF/SVG)  
✅ **Maintenance:** Auto-extract from code; manual docs stay manual  

🎯 **Ready for:** Integration, auditing, development, stakeholder communication

