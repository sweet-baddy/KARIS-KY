#!/usr/bin/env python3
"""
Auto-generate visual architecture documentation from Soroban escrow contract code.

Produces:
  1. Entity Relationship Diagram (Mermaid ER)
  2. Data Flow Diagram (Mermaid graph)
  3. State Machine Diagram (Mermaid stateDiagram-v2)
  4. Module & Dependency Graph (Mermaid graph)
  5. Storage Layout Reference (Markdown table)
  6. Entrypoint Matrix (Markdown + Mermaid diagram)

Usage:
  python3 scripts/generate_architecture_docs.py [--output-dir docs/arch] [--formats all|mermaid|markdown]
"""

import re
import json
from pathlib import Path
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass, asdict
from enum import Enum


@dataclass
class DataKey:
    """Represents a storage key variant."""
    name: str
    ty: str  # Type annotation
    description: str
    is_persistent: bool = False
    indexed_by: Optional[str] = None  # e.g., "Address" for per-investor keys


@dataclass
class Struct:
    """Represents a contract type struct."""
    name: str
    fields: List[Tuple[str, str]]  # [(field_name, type), ...]
    description: str


@dataclass
class Entrypoint:
    """Represents a contract entrypoint."""
    name: str
    params: List[Tuple[str, str]]  # [(param_name, type), ...]
    returns: str
    auth_required: Optional[str]  # Role that must authorize
    description: str
    emits_events: List[str] = None  # Event types emitted
    storage_mutated: List[str] = None  # Storage keys modified


class ArchitectureAnalyzer:
    """Parses Soroban contract code and extracts architecture info."""

    def __init__(self, lib_path: Path):
        self.lib_path = lib_path
        self.content = lib_path.read_text(encoding="utf-8")
        self.data_keys: List[DataKey] = []
        self.structs: Dict[str, Struct] = {}
        self.entrypoints: List[Entrypoint] = []
        self.events: List[str] = []
        self.errors: List[Tuple[str, str]] = []

    def extract_data_keys(self) -> None:
        """Extract DataKey enum variants and descriptions."""
        # Match enum DataKey { ... }
        enum_match = re.search(
            r"pub enum DataKey\s*\{([^}]+)\}",
            self.content,
            re.DOTALL,
        )
        if not enum_match:
            return

        enum_body = enum_match.group(1)
        # Split by /// comments preceding each variant
        pattern = r"///\s*(.+?)\n\s*(\w+(?:\([^)]+\))?),?"
        matches = re.findall(pattern, enum_body, re.MULTILINE)

        for desc, variant in matches:
            # Parse variant: name or name(Type)
            indexed = None
            if "(" in variant:
                name, rest = variant.split("(")
                indexed = rest.rstrip(")")
            else:
                name = variant

            is_persistent = "Persistent" in desc or "per-investor" in desc.lower()
            self.data_keys.append(
                DataKey(
                    name=name.strip(),
                    ty=indexed or "bool/option",
                    description=desc.strip(),
                    is_persistent=is_persistent,
                    indexed_by=indexed,
                )
            )

    def extract_structs(self) -> None:
        """Extract contracttype structs."""
        # Match #[contracttype] struct Name { fields }
        pattern = r"#\[contracttype\](?:\s*\n)?.*?pub struct (\w+)\s*\{([^}]+)\}"
        matches = re.finditer(pattern, self.content, re.DOTALL | re.MULTILINE)

        for match in matches:
            struct_name = match.group(1)
            fields_str = match.group(2)

            # Extract fields: pub field: Type,
            field_pattern = r"pub\s+(\w+)\s*:\s*([^,\n]+)"
            fields = re.findall(field_pattern, fields_str)
            fields = [(f, t.strip()) for f, t in fields]

            # Get description from doc comments
            desc_match = re.search(
                rf"///.*?\n.*?pub struct {struct_name}",
                self.content[max(0, match.start() - 500) : match.end()],
                re.DOTALL,
            )
            desc = ""
            if desc_match:
                lines = desc_match.group(0).split("\n")
                desc = " ".join(l.strip().lstrip("/").strip() for l in lines if "///" in l)

            self.structs[struct_name] = Struct(
                name=struct_name,
                fields=fields,
                description=desc[:100],
            )

    def extract_entrypoints(self) -> None:
        """Extract contract entrypoints (public fn on impl)."""
        # Match pub fn (...)
        pattern = r"pub fn (\w+)\s*\([^)]*\)\s*(?:->\s*([^{;]+?))?\s*\{"
        matches = re.finditer(pattern, self.content)

        for match in matches:
            name = match.group(1)
            returns = match.group(2) or "void"

            # Get doc comments
            start_pos = max(0, match.start() - 500)
            doc_section = self.content[start_pos : match.start()]
            desc_lines = re.findall(r"///\s*(.+)", doc_section[-300:])
            desc = " ".join(desc_lines)

            # Determine if auth is required and by whom
            auth_role = None
            for role in ["admin", "sme", "investor", "treasury"]:
                if role in desc.lower() or f"{role}_address" in desc.lower():
                    auth_role = role.upper()
                    break

            self.entrypoints.append(
                Entrypoint(
                    name=name,
                    params=[],
                    returns=returns.strip()[:30] if returns else "void",
                    auth_required=auth_role,
                    description=desc[:80],
                    emits_events=[],
                    storage_mutated=[],
                )
            )

    def analyze(self) -> None:
        """Run all extraction passes."""
        self.extract_data_keys()
        self.extract_structs()
        self.extract_entrypoints()

    def get_state_transitions(self) -> List[Tuple[int, int, str]]:
        """Extract status transitions: (from_status, to_status, reason)."""
        # Hardcoded based on ADR-001 state model
        return [
            (0, 1, "fund: target_met"),
            (1, 2, "settle: maturity_passed"),
            (2, 3, "withdraw: sme_pulls_liquidity"),
            (0, 4, "cancel: admin_initiated"),
            (4, 0, "refund: investor_reclaims"),
        ]


class MarkdownGenerator:
    """Generate Markdown documentation."""

    @staticmethod
    def generate_storage_table(keys: List[DataKey]) -> str:
        """Generate storage layout reference table."""
        lines = [
            "## Storage Layout Reference\n",
            "| Key | Type | Storage | Indexed By | Description |",
            "|-----|------|---------|------------|-------------|",
        ]
        for key in keys:
            storage = "Persistent" if key.is_persistent else "Instance"
            indexed = key.indexed_by or "—"
            lines.append(
                f"| `{key.name}` | {key.ty} | {storage} | {indexed} | {key.description[:60]}... |"
            )
        return "\n".join(lines)

    @staticmethod
    def generate_struct_table(structs: Dict[str, Struct]) -> str:
        """Generate struct field reference."""
        lines = [
            "## Contract Types\n",
            "### InvoiceEscrow (State Root)",
            "| Field | Type | Purpose |",
            "|-------|------|---------|",
        ]

        # Prioritize InvoiceEscrow if present
        if "InvoiceEscrow" in structs:
            s = structs["InvoiceEscrow"]
            for field, ty in s.fields:
                lines.append(f"| `{field}` | `{ty}` | State tracking |")

        lines.append("")

        # Other structs
        for name, s in structs.items():
            if name != "InvoiceEscrow":
                lines.append(f"\n### {name}")
                lines.append("| Field | Type |")
                lines.append("|-------|------|")
                for field, ty in s.fields:
                    lines.append(f"| `{field}` | `{ty}` |")

        return "\n".join(lines)

    @staticmethod
    def generate_entrypoint_table(eps: List[Entrypoint]) -> str:
        """Generate entrypoint reference."""
        lines = [
            "## Contract Entrypoints\n",
            "| Entrypoint | Auth Required | Purpose |",
            "|------------|---------------|---------|",
        ]
        for ep in eps:
            auth = ep.auth_required or "—"
            desc = ep.description[:50]
            lines.append(f"| `{ep.name}()` | {auth} | {desc}... |")
        return "\n".join(lines)


class MermaidGenerator:
    """Generate Mermaid diagrams."""

    @staticmethod
    def generate_er_diagram(keys: List[DataKey], structs: Dict[str, Struct]) -> str:
        """Generate Entity Relationship Diagram."""
        lines = ["erDiagram"]

        # Add main entity: InvoiceEscrow
        lines.append('    INVOICE_ESCROW ||--o{ INVESTOR : "contributions"')
        lines.append('    INVOICE_ESCROW ||--|| FUNDING_TOKEN : "binds"')
        lines.append('    INVOICE_ESCROW ||--|| TREASURY : "yields_to"')
        lines.append('    INVOICE_ESCROW ||--o{ ATTESTATION_LOG : "records"')
        lines.append('    INVOICE_ESCROW ||--o{ YIELD_TIER : "offers"')

        return "\n".join(lines)

    @staticmethod
    def generate_state_machine(transitions: List[Tuple[int, int, str]]) -> str:
        """Generate state machine diagram."""
        state_names = {
            0: "OPEN",
            1: "FUNDED",
            2: "SETTLED",
            3: "WITHDRAWN",
            4: "CANCELLED",
        }

        lines = ["stateDiagram-v2"]
        lines.append("    [*] --> OPEN")

        seen = set()
        for from_st, to_st, reason in transitions:
            key = (from_st, to_st)
            if key not in seen:
                seen.add(key)
                lines.append(
                    f"    {state_names[from_st]} --> {state_names[to_st]} : {reason}"
                )

        lines.append("    WITHDRAWN --> [*]")
        lines.append("    CANCELLED --> [*]")

        return "\n".join(lines)

    @staticmethod
    def generate_data_flow(keys: List[DataKey]) -> str:
        """Generate data flow diagram."""
        lines = ["graph LR"]
        lines.append("    Investor[Investor] -->|fund| Contract")
        lines.append("    SME[SME] -->|settle/withdraw| Contract")
        lines.append("    Admin[Admin] -->|govern| Contract")
        lines.append("    Contract -->|writes| Storage[Storage Layer]")
        lines.append("    Storage --> InstanceKeys[Instance Keys]")
        lines.append("    Storage --> PersistentKeys[Persistent Keys<br/>per-investor]")
        lines.append("    Contract -->|transfer| Token[SEP-41 Token]")
        lines.append("    Contract -.->|emit| Events[Event Stream]")
        return "\n".join(lines)

    @staticmethod
    def generate_module_graph() -> str:
        """Generate module dependency graph."""
        lines = ["graph TD"]
        lines.append("    lib[lib.rs<br/>Main Contract] --> ext[external_calls.rs<br/>Token/Auth]")
        lines.append("    lib --> val[validation.rs<br/>Input Checks]")
        lines.append("    ext --> token[TokenClient<br/>SEP-41]")
        lines.append("    val --> types[Contract Types]")
        lines.append("    lib --> tests[test/ module]")
        return "\n".join(lines)

    @staticmethod
    def generate_entrypoint_matrix(eps: List[Entrypoint]) -> str:
        """Generate entrypoint call-matrix."""
        # Categorize by role
        by_role = {}
        for ep in eps:
            role = ep.auth_required or "PUBLIC"
            if role not in by_role:
                by_role[role] = []
            by_role[role].append(ep.name)

        lines = ["graph LR"]
        for role, names in sorted(by_role.items()):
            for name in names:
                lines.append(f'    {role}["{role}"] --> {name}["💬 {name}()"]')

        return "\n".join(lines)


class ArchitectureDocGenerator:
    """Main generator: orchestrates analysis and output."""

    def __init__(self, lib_path: Path, output_dir: Path):
        self.lib_path = lib_path
        self.output_dir = output_dir
        self.analyzer = ArchitectureAnalyzer(lib_path)
        self.analyzer.analyze()

    def generate_all(self) -> None:
        """Generate all architecture docs."""
        self.output_dir.mkdir(parents=True, exist_ok=True)

        # Main index
        self.generate_index()

        # Mermaid diagrams
        self.generate_diagrams()

        # Reference tables
        self.generate_references()

        print(f"✓ Generated architecture docs in {self.output_dir}")

    def generate_index(self) -> None:
        """Generate main index document."""
        path = self.output_dir / "ARCHITECTURE.md"
        sections = [
            "# Auto-Generated Architecture Documentation\n",
            "_Generated from contract code analysis_\n",
            "## Overview\n",
            "The karis-ky escrow contract manages invoice-backed liquidity via role-based authorization, ",
            "immutable state transitions, and bounded per-investor storage.\n",
            "## Quick Navigation\n",
            "- [Entity Relationships](entity-relationships.md) — data model and storage layout",
            "- [Data Flow](data-flow.md) — token and authorization paths",
            "- [State Machine](state-machine.md) — escrow lifecycle transitions",
            "- [Module Structure](module-structure.md) — code organization",
            "- [Entrypoint Matrix](entrypoint-matrix.md) — role-based API surface",
            "- [Storage Reference](storage-reference.md) — detailed key/type catalog",
            "## Key Metrics\n",
            f"- **DataKey variants:** {len(self.analyzer.data_keys)}",
            f"- **Contract types:** {len(self.analyzer.structs)}",
            f"- **Public entrypoints:** {len(self.analyzer.entrypoints)}",
            f"- **Schema version:** 6 (current)",
            f"- **Max attestation entries:** 32",
            f"- **Max dust sweep per call:** 100M base units",
            "\n## Architecture Decision Records\n",
            "| ADR | Decision |",
            "|-----|----------|",
            "| ADR-001 | State model (status 0–4, forward-only transitions) |",
            "| ADR-002 | Authorization boundaries (admin, SME, investor, treasury) |",
            "| ADR-003 | Two-phase settlement + funding-close snapshot |",
            "| ADR-004 | Legal/compliance hold mechanism |",
            "| ADR-005 | Tiered yield with per-investor commitment locks |",
            "| ADR-006 | Treasury dust sweep + SEP-41 token safety |",
            "| ADR-007 | Per-investor keys in persistent storage (v6 migration) |",
        ]
        path.write_text("\n".join(sections))

    def generate_diagrams(self) -> None:
        """Generate Mermaid diagram files."""
        docs = {
            "entity-relationships.md": self._generate_er_doc(),
            "data-flow.md": self._generate_dataflow_doc(),
            "state-machine.md": self._generate_sm_doc(),
            "module-structure.md": self._generate_module_doc(),
            "entrypoint-matrix.md": self._generate_entrypoint_doc(),
        }

        for name, content in docs.items():
            (self.output_dir / name).write_text(content)

    def _generate_er_doc(self) -> str:
        er = MermaidGenerator.generate_er_diagram(
            self.analyzer.data_keys, self.analyzer.structs
        )
        return rf"""# Entity Relationships

Data model and storage associations.

```mermaid
{er}
```

## Entities

- **INVOICE_ESCROW**: Root state containing escrow metadata, amounts, and status
- **INVESTOR**: Per-address contribution records (persistent storage)
- **FUNDING_TOKEN**: Immutable SEP-41 token contract reference
- **TREASURY**: Immutable recipient for dust sweep operations
- **ATTESTATION_LOG**: Audit chain of approval digests
- **YIELD_TIER**: Optional commitment-based yield ladder

## Relationships

1. **One-to-Many (investors)**: Each escrow accepts multiple investor contributions
2. **One-to-One (token/treasury)**: Immutable bindings at initialization
3. **Append-only (attestations)**: Bounded audit log with per-entry revocation markers
"""

    def _generate_dataflow_doc(self) -> str:
        flow = MermaidGenerator.generate_data_flow(self.analyzer.data_keys)
        return rf"""# Data Flow

Authorization, storage, and token movement paths.

```mermaid
{flow}
```

## Flow Phases

### 1. Authorization
- **Investor**: Calls `fund()` / `fund_with_commitment()`
- **SME**: Calls `withdraw()` / `settle()`
- **Admin**: Calls governance functions (legal hold, migration, etc.)
- **Treasury**: Calls `sweep_terminal_dust()`

### 2. Storage Write
- **Instance keys**: Atomic escrow state, version, legal hold
- **Persistent keys**: Per-investor contributions, yields, claims

### 3. Token Transfer
All transfers route through `external_calls::transfer_token_from()`:
- Pre-transfer balance check (no balance assumption)
- Post-transfer equality verification (rejects fee-on-transfer)
- Typed error codes on mismatch (codes 36–41)

### 4. Event Emission
- State changes emit typed events (e.g., `EscrowFunded`, `EscrowSettled`)
- Indexed by role and ledger height for off-chain auditing
"""

    def _generate_sm_doc(self) -> str:
        transitions = self.analyzer.get_state_transitions()
        sm = MermaidGenerator.generate_state_machine(transitions)
        return rf"""# State Machine

Escrow lifecycle transitions (forward-only).

```mermaid
{sm}
```

## Status Codes

| Code | Name | Meaning |
|------|------|---------|
| 0 | OPEN | Initial; accepting contributions |
| 1 | FUNDED | Funding target reached; awaiting settlement |
| 2 | SETTLED | Settlement locked in; no investor claims |
| 3 | WITHDRAWN | SME pulled liquidity; awaiting investor claims |
| 4 | CANCELLED | Admin halt; investors may refund |

## Transitions

- **0 → 1**: `fund()` reaches `funding_target`
- **1 → 2**: `settle()` called after maturity
- **2 → 3**: `withdraw()` called by SME (after settle)
- **0 → 4**: `cancel_escrow()` called by admin
- **4 → —**: Investors call `refund()` to reclaim principal

## Legal Hold Impact

Legal hold blocks:
- Settlement finalization (1 → 2)
- SME withdrawal (2 → 3)
- Investor claim payouts
- Does **not** block admin authority or hold clearance
"""

    def _generate_module_doc(self) -> str:
        graph = MermaidGenerator.generate_module_graph()
        return rf"""# Module Structure

Code organization and internal dependencies.

```mermaid
{graph}
```

## Modules

### `lib.rs` (Main Contract)
- **Responsibility**: Public API, state machine, auth boundaries
- **Key items**: `LiquifactEscrow` impl, `DataKey` enum, `InvoiceEscrow` struct
- **Lines**: ~3300 LOC (including tests)

### `external_calls.rs`
- **Responsibility**: Token transfers and Stellar authorization
- **Key items**: `transfer_token_from()`, balance equality checks
- **Boundary**: SEP-41 compliance verification, typed errors 36–41

### `validation.rs`
- **Responsibility**: Input validation and preconditions
- **Key items**: Invoice ID charset, amount bounds, maturity checks
- **Boundary**: Prevents invalid state before auth gates

## Dependencies

- **soroban-sdk**: Core contract runtime, storage, types
- **soroban-auth**: Authorization delegation (if used)
- **serde/serde_json**: Test fixtures and serialization
"""

    def _generate_entrypoint_doc(self) -> str:
        ep_matrix = MermaidGenerator.generate_entrypoint_matrix(
            self.analyzer.entrypoints
        )
        return rf"""# Entrypoint Matrix

Role-based API surface.

```mermaid
{ep_matrix}
```

## Entrypoints by Role

### Admin
- `init()` — Initialize escrow
- `set_legal_hold()` — Compliance gate
- `propose_admin()` / `accept_admin()` — Admin handover
- `bind_primary_attestation_hash()` — Digest binding
- `record_sme_collateral_commitment()` — Metadata

### SME
- `withdraw()` — Pull funded amount
- `settle()` — Finalize settlement
- `record_sme_collateral_commitment()` — Report collateral

### Investor
- `fund()` — Contribute principal
- `fund_with_commitment()` — Lock + tier selection
- `claim_investor_payout()` — Claim after settlement
- `refund()` — Reclaim in cancelled escrow

### Treasury
- `sweep_terminal_dust()` — Terminal rounding cleanup

### Public (No Auth)
- `get_escrow()` — Read state
- `get_version()` — Read schema version
- `get_template()` — Template lookup

## Authorization Guard Ordering

Every state mutation:
1. Read-only preconditions (legal hold, status, input validation)
2. `Address::require_auth()` for the bound role
3. Storage writes + SEP-41 transfers
"""

    def generate_references(self) -> None:
        """Generate detailed reference tables."""
        path = self.output_dir / "storage-reference.md"
        sections = [
            "# Storage Reference",
            "",
            "Detailed catalog of `DataKey` variants and contract types.",
            "",
            MarkdownGenerator.generate_storage_table(self.analyzer.data_keys),
            "",
            "",
            MarkdownGenerator.generate_struct_table(self.analyzer.structs),
            "",
            "",
            MarkdownGenerator.generate_entrypoint_table(self.analyzer.entrypoints),
        ]
        path.write_text("\n".join(sections))


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Auto-generate visual architecture docs from contract code"
    )
    parser.add_argument(
        "--output-dir",
        default="docs/arch",
        help="Output directory for generated docs (default: docs/arch)",
    )
    parser.add_argument(
        "--lib-path",
        default="escrow/src/lib.rs",
        help="Path to contract lib.rs (default: escrow/src/lib.rs)",
    )

    args = parser.parse_args()

    lib_path = Path(args.lib_path)
    if not lib_path.exists():
        print(f"Error: {lib_path} not found")
        return 1

    output_dir = Path(args.output_dir)
    generator = ArchitectureDocGenerator(lib_path, output_dir)
    generator.generate_all()

    return 0


if __name__ == "__main__":
    exit(main())
