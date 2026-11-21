#!/usr/bin/env python3
"""
Validate SDK type and method coverage against the contract spec.

This script ensures that all public contract methods and types exposed in the
WASM artifact are represented in the TypeScript SDK. It detects when contract
changes are not reflected in the SDK, preventing silent ABI drift.

Usage:
  python3 scripts/validate_sdk_coverage.py \\
    --spec sdk-ts/spec.json \\
    --client sdk-ts/src/client.ts \\
    --types sdk-ts/src/types.ts

Exit codes:
  0 - All methods and types are covered
  1 - Missing methods or types in SDK
  2 - Error reading files
"""

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Set, Dict, List


def load_spec(spec_path: Path) -> dict:
    """Load and parse the contract specification."""
    try:
        with open(spec_path, "r") as f:
            return json.load(f)
    except Exception as e:
        print(f"ERROR: Failed to load spec {spec_path}: {e}", file=sys.stderr)
        raise


def read_file(path: Path) -> str:
    """Read file content."""
    try:
        with open(path, "r") as f:
            return f.read()
    except Exception as e:
        print(f"ERROR: Failed to read {path}: {e}", file=sys.stderr)
        raise


def extract_spec_methods(spec: dict) -> Dict[str, Dict]:
    """Extract method names from spec."""
    methods = {}
    
    # Entrypoints (state-mutating)
    for ep in spec.get("entrypoints", []):
        name = ep.get("name")
        if name:
            methods[name] = {"type": "entrypoint", "returns": ep.get("returns")}
    
    # Read-only entrypoints
    for ep in spec.get("read_only_entrypoints", []):
        name = ep.get("name")
        if name:
            methods[name] = {"type": "read_only", "returns": ep.get("returns")}
    
    return methods


def extract_spec_types(spec: dict) -> Set[str]:
    """Extract type names from spec."""
    types = set()
    
    # Collect types from entrypoint parameters and returns
    def collect_types_from_value(val):
        if isinstance(val, dict):
            for k, v in val.items():
                if k == "type" and isinstance(v, str):
                    # Extract custom types (not primitives)
                    if not v.startswith("Vec") and not v.startswith("Option") and \
                       v not in ("i128", "u64", "i64", "u32", "bool", "String", "Address", "Bytes"):
                        types.add(v)
                collect_types_from_value(v)
        elif isinstance(val, list):
            for item in val:
                collect_types_from_value(item)
    
    for ep in spec.get("entrypoints", []) + spec.get("read_only_entrypoints", []):
        collect_types_from_value(ep.get("params", []))
        collect_types_from_value(ep.get("returns", {}))
    
    return types


def extract_sdk_methods(client_content: str) -> Set[str]:
    """Extract method names defined in the SDK client."""
    methods = set()
    
    # Match async method definitions: "async methodName(...)"
    pattern = r'async\s+(\w+)\s*\('
    matches = re.findall(pattern, client_content)
    methods.update(matches)
    
    return methods


def extract_sdk_types(types_content: str) -> Set[str]:
    """Extract type names defined in the SDK types file."""
    types = set()
    
    # Match exported interface/type definitions
    pattern = r'export\s+(?:interface|type|enum)\s+(\w+)'
    matches = re.findall(pattern, types_content)
    types.update(matches)
    
    return types


def main():
    parser = argparse.ArgumentParser(
        description="Validate SDK coverage against contract spec"
    )
    parser.add_argument(
        "--spec",
        type=Path,
        required=True,
        help="Path to spec.json"
    )
    parser.add_argument(
        "--client",
        type=Path,
        required=True,
        help="Path to client.ts"
    )
    parser.add_argument(
        "--types",
        type=Path,
        required=True,
        help="Path to types.ts"
    )
    
    args = parser.parse_args()
    
    # Load spec
    try:
        spec = load_spec(args.spec)
    except Exception:
        return 2
    
    # Read SDK files
    try:
        client_content = read_file(args.client)
        types_content = read_file(args.types)
    except Exception:
        return 2
    
    # Extract methods and types
    spec_methods = extract_spec_methods(spec)
    spec_types = extract_spec_types(spec)
    sdk_methods = extract_sdk_methods(client_content)
    sdk_types = extract_sdk_types(types_content)
    
    # Check coverage
    missing_methods = set(spec_methods.keys()) - sdk_methods
    missing_types = spec_types - sdk_types
    
    # Report
    status = 0
    
    if missing_methods:
        print(f"\n❌ Missing SDK methods ({len(missing_methods)}):", file=sys.stderr)
        for method in sorted(missing_methods):
            method_info = spec_methods[method]
            print(f"   - {method} ({method_info['type']})", file=sys.stderr)
        status = 1
    
    if missing_types:
        print(f"\n❌ Missing SDK types ({len(missing_types)}):", file=sys.stderr)
        for type_name in sorted(missing_types):
            print(f"   - {type_name}", file=sys.stderr)
        status = 1
    
    if status == 0:
        print(
            f"✅ SDK coverage OK: {len(sdk_methods)} methods, {len(sdk_types)} types",
            file=sys.stderr
        )
    else:
        print(
            f"\nTo fix, add the missing methods and types to the SDK.",
            file=sys.stderr
        )
    
    return status


if __name__ == "__main__":
    sys.exit(main())
