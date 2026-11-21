#!/usr/bin/env python3
"""
Generate or update the SDK spec (spec.json) from the built WASM contract.

This script extracts the contract specification from the WASM artifact's metadata
and generates a machine-readable SDK interface file. Used in CI to detect and
prevent ABI drift between the contract source and committed SDK artifacts.

Usage:
  python3 scripts/generate_spec.py \\
    --wasm target/wasm32-unknown-unknown/release/karis_ky_escrow.wasm \\
    --output sdk-ts/spec.json

Exit codes:
  0 - Success
  1 - Error reading WASM or generating spec
  2 - Generated spec differs from existing spec (use --force to overwrite)
"""

import argparse
import json
import sys
from pathlib import Path


def extract_spec_from_wasm(wasm_path: Path) -> dict:
    """
    Extract the contract specification from WASM metadata.
    
    In a real implementation, this would use Soroban SDK tools or custom parsing.
    For now, this is a placeholder that demonstrates the structure.
    
    Args:
        wasm_path: Path to the built WASM artifact
        
    Returns:
        Specification dictionary
    """
    if not wasm_path.exists():
        raise FileNotFoundError(f"WASM file not found: {wasm_path}")
    
    # NOTE: In a production system, you would:
    # 1. Use `stellar contract spec` command or Soroban SDK to extract metadata
    # 2. Parse the contract's const_metadata XDR section
    # 3. Deserialize to JSON
    
    # For now, return a minimal structure. In CI, this would call:
    # `stellar contract spec <wasm_path> --output json`
    
    print(f"INFO: Would extract spec from WASM: {wasm_path}", file=sys.stderr)
    
    # Placeholder: return empty spec structure
    # Real implementation uses stellar-cli or Soroban SDK
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "karis-ky Escrow Contract ABI",
        "description": "Machine-readable specification of the karis-ky escrow smart contract interface for SDK consumption. Auto-generated from contract source.",
        "contract": {
            "name": "LiquifactEscrow",
            "package": "karis_ky_escrow",
            "description": "Holds investor funds for an invoice until settlement on Stellar Soroban",
            "schema_version": 8,
            "interface_version": 1,
        },
        "entrypoints": [],
        "read_only_entrypoints": [],
        "error_codes": {"groups": {}, "codes": {}},
    }


def generate_spec(wasm_path: Path) -> dict:
    """Generate the full contract specification."""
    try:
        spec = extract_spec_from_wasm(wasm_path)
        return spec
    except Exception as e:
        print(f"ERROR: Failed to generate spec from {wasm_path}: {e}", file=sys.stderr)
        raise


def compare_specs(existing: dict, generated: dict) -> bool:
    """
    Compare two specs and return True if they differ.
    
    Args:
        existing: Existing spec dictionary
        generated: Generated spec dictionary
        
    Returns:
        True if specs differ, False if identical
    """
    return json.dumps(existing, sort_keys=True) != json.dumps(generated, sort_keys=True)


def main():
    parser = argparse.ArgumentParser(
        description="Generate SDK spec.json from WASM contract metadata"
    )
    parser.add_argument(
        "--wasm",
        type=Path,
        required=True,
        help="Path to the built WASM artifact"
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Output path for spec.json"
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing spec without comparing"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Only check if spec would change; do not write"
    )
    
    args = parser.parse_args()
    
    # Generate spec from WASM
    try:
        generated_spec = generate_spec(args.wasm)
    except Exception as e:
        print(f"FATAL: {e}", file=sys.stderr)
        return 1
    
    # Check for drift if output exists
    if args.output.exists() and not args.force:
        try:
            with open(args.output, "r") as f:
                existing_spec = json.load(f)
            
            if compare_specs(existing_spec, generated_spec):
                if args.check:
                    print(
                        f"ERROR: Generated spec differs from {args.output}\n"
                        "       Contract ABI has drifted from the committed spec.\n"
                        "       Run 'python3 scripts/generate_spec.py' to update.",
                        file=sys.stderr
                    )
                    return 2
                else:
                    print(
                        f"WARN: Generated spec differs from {args.output}; "
                        "updating...",
                        file=sys.stderr
                    )
        except json.JSONDecodeError as e:
            print(f"ERROR: Existing spec is not valid JSON: {e}", file=sys.stderr)
            return 1
    
    # Write spec
    if not args.check:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, "w") as f:
            json.dump(generated_spec, f, indent=2)
        print(f"INFO: Wrote spec to {args.output}", file=sys.stderr)
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
