#!/usr/bin/env python3
"""
Convert architecture diagrams to PlantUML format (supports PDF/SVG export).

This tool generates PlantUML diagrams from Mermaid output or direct code analysis,
enabling export to multiple image formats via PlantUML CLI or online renderer.

Usage:
  python3 scripts/arch_to_plantuml.py [--format mermaid|plantuml] [--output-dir docs/arch]
"""

from pathlib import Path
from typing import List, Tuple


class PlantUMLGenerator:
    """Generate PlantUML diagrams."""

    @staticmethod
    def generate_class_diagram_datakeys(keys: List) -> str:
        """Generate class diagram for storage model."""
        lines = [
            "@startuml",
            "title Storage Model - DataKey Catalog",
            "",
            "package InstanceStorage <<Rectangle>> {",
            "  class Escrow",
            "  class Version",
            "  class LegalHold",
            "  class FundingCloseSnapshot",
            "  class YieldTierTable",
            "}",
            "",
            "package PersistentStorage <<Rectangle>> {",
            "  class InvestorContribution",
            "  class InvestorEffectiveYield",
            "  class InvestorClaimNotBefore",
            "  class InvestorClaimed",
            "  class InvestorAllowlisted",
            "  class InvestorRefunded",
            "}",
            "",
            "Escrow : invoice_id: Symbol",
            "Escrow : admin: Address",
            "Escrow : sme_address: Address",
            "Escrow : amount: i128",
            "Escrow : funding_target: i128",
            "Escrow : funded_amount: i128",
            "Escrow : yield_bps: i64",
            "Escrow : maturity: u64",
            "Escrow : status: u32 (0-4)",
            "",
            "InvestorContribution : per_address_value: i128",
            "InvestorEffectiveYield : chosen_yield_bps: i64",
            "InvestorClaimNotBefore : lock_until_timestamp: u64",
            "",
            "Escrow -- InvestorContribution",
            "Escrow -- InvestorEffectiveYield",
            "Escrow -- InvestorClaimNotBefore",
            "",
            "@enduml",
        ]
        return "\n".join(lines)

    @staticmethod
    def generate_sequence_diagram_funding() -> str:
        """Generate sequence diagram for funding flow."""
        lines = [
            "@startuml",
            "title Funding Flow Sequence",
            "",
            "participant Investor",
            "participant Contract",
            "participant Token",
            "participant Storage",
            "",
            "Investor -> Contract: fund(amount)",
            "activate Contract",
            "",
            "Contract -> Contract: validate_input()",
            "Contract -> Contract: check_legal_hold()",
            "",
            "Contract -> Investor: require_auth()",
            "Investor -> Contract: ✓ authorized",
            "",
            "Contract -> Token: transfer_from(investor, contract, amount)",
            "activate Token",
            "Token -> Contract: ✓ transferred",
            "deactivate Token",
            "",
            "Contract -> Storage: write InvestorContribution(investor) += amount",
            "activate Storage",
            "Storage -> Contract: ✓ written",
            "deactivate Storage",
            "",
            "Contract -> Storage: read funded_amount",
            "activate Storage",
            "Storage -> Contract: current funded_amount",
            "deactivate Storage",
            "",
            "alt funded_amount >= funding_target",
            "  Contract -> Storage: write status = 1 (FUNDED)",
            "  Storage -> Contract: ✓",
            "  Contract -> Contract: emit EscrowFunded event",
            "end",
            "",
            "Contract -> Investor: ✓ success",
            "deactivate Contract",
            "",
            "@enduml",
        ]
        return "\n".join(lines)

    @staticmethod
    def generate_sequence_diagram_settlement() -> str:
        """Generate sequence diagram for settlement flow."""
        lines = [
            "@startuml",
            "title Settlement Flow Sequence",
            "",
            "participant SME",
            "participant Contract",
            "participant Registry",
            "participant Storage",
            "",
            "SME -> Contract: settle()",
            "activate Contract",
            "",
            "Contract -> Contract: check_legal_hold()",
            "Contract -> Contract: check_status == FUNDED",
            "Contract -> Storage: read maturity",
            "activate Storage",
            "Storage -> Contract: maturity timestamp",
            "deactivate Storage",
            "",
            "Contract -> Contract: check_ledger_time >= maturity",
            "",
            "Contract -> SME: require_auth()",
            "SME -> Contract: ✓ authorized",
            "",
            "Contract -> Storage: write status = 2 (SETTLED)",
            "activate Storage",
            "Storage -> Contract: ✓",
            "deactivate Storage",
            "",
            "Contract -> Contract: emit EscrowSettled event",
            "Contract -> SME: ✓ success",
            "deactivate Contract",
            "",
            "note right of SME",
            "  Now SME can withdraw()",
            "  and investors can claim_investor_payout()",
            "end note",
            "",
            "@enduml",
        ]
        return "\n".join(lines)

    @staticmethod
    def generate_usecase_diagram() -> str:
        """Generate use-case diagram for roles."""
        lines = [
            "@startuml",
            "title Contract Use Cases by Role",
            "",
            "left to right direction",
            "",
            "actor Admin",
            "actor SME",
            "actor Investor",
            "actor Treasury",
            "",
            "(initialize escrow) as init",
            "(set legal hold) as hold",
            "(propose admin) as propose",
            "(bind attestation) as attest",
            "",
            "(fund escrow) as fund",
            "(fund with commitment) as fund_commit",
            "(claim payout) as claim",
            "  (withdraw) as withdraw",
            "(settle escrow) as settle",
            "",
            "(sweep dust) as sweep",
            "(record collateral) as collateral",
            "",
            "Admin --> init",
            "Admin --> hold",
            "Admin --> propose",
            "Admin --> attest",
            "",
            "Investor --> fund",
            "Investor --> fund_commit",
            "Investor --> claim",
            "",
            "SME --> withdraw",
            "SME --> settle",
            "SME --> collateral",
            "",
            "Treasury --> sweep",
            "",
            "@enduml",
        ]
        return "\n".join(lines)

    @staticmethod
    def generate_component_diagram() -> str:
        """Generate component diagram."""
        lines = [
            "@startuml",
            "title Contract Architecture - Components",
            "",
            "[LiquifactEscrow\\nContract] as contract",
            "[ValidationModule] as validation",
            "[ExternalCalls\n(Token Interface)] as ext",
            "[DataStore\n(Instance)] as inst_store",
            "[DataStore\n(Persistent)] as pers_store",
            "[TokenContract\n(SEP-41)] as token",
            "",
            "contract --> validation : uses",
            "contract --> ext : delegates to",
            "contract --> inst_store : read/write",
            "contract --> pers_store : read/write",
            "ext --> token : transfer_from()",
            "",
            "database \"Storage Keys\" {",
            "  [Escrow State]",
            "  [Version]",
            "  [Per-Investor Contributions]",
            "  [Yield Tiers]",
            "  [Attestation Log]",
            "}",
            "",
            "@enduml",
        ]
        return "\n".join(lines)


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Generate PlantUML architecture diagrams")
    parser.add_argument(
        "--output-dir",
        default="docs/arch/plantuml",
        help="Output directory for PlantUML files",
    )

    args = parser.parse_args()
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    diagrams = {
        "storage-model.puml": PlantUMLGenerator.generate_class_diagram_datakeys(None),
        "funding-sequence.puml": PlantUMLGenerator.generate_sequence_diagram_funding(),
        "settlement-sequence.puml": PlantUMLGenerator.generate_sequence_diagram_settlement(),
        "usecases.puml": PlantUMLGenerator.generate_usecase_diagram(),
        "components.puml": PlantUMLGenerator.generate_component_diagram(),
    }

    for name, content in diagrams.items():
        path = output_dir / name
        path.write_text(content)
        print(f"✓ {path}")

    print(
        f"\nGenerated {len(diagrams)} PlantUML diagrams in {output_dir}"
    )
    print("\nTo convert to SVG/PDF, use:")
    print(f"  plantuml -tsvg {output_dir}/*.puml")
    print(f"  plantuml -tpdf {output_dir}/*.puml")
    print("\nOr use the online renderer: http://www.plantuml.com/plantuml/uml/")

    return 0


if __name__ == "__main__":
    exit(main())
