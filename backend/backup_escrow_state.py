#!/usr/bin/env python3
"""
backup_escrow_state.py — Off-chain point-in-time snapshot of karis-ky escrow state.

Queries all read-only entrypoints via the Stellar/Soroban RPC and writes a
timestamped JSON file.  No contract auth is required; no on-chain storage is
consumed.

Why off-chain?
  On-chain restore_from_backup is rejected (see docs/adr/ADR-008-backup-restore-rejection.md).
  State snapshots belong in the operator's backup infrastructure, not in
  contract storage.

Retention policy:
  Snapshot files are named:
      escrow_backup_<contract_id_prefix>_seq<ledger>_<utc_timestamp>.json
  Prune old files with standard filesystem tooling or object-storage lifecycle
  rules.  The script does not auto-prune.

Usage:
    python3 backup_escrow_state.py \\
        --rpc-url  https://soroban-testnet.stellar.org \\
        --contract CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX \\
        --network-passphrase "Test SDF Network ; September 2015" \\
        [--investors GADDR1 GADDR2 ...] \\
        [--output-dir ./backups]

Dependencies:
    stellar-sdk >= 11.0.0   (pip install stellar-sdk)

    Install: pip install "stellar-sdk>=11.0.0"
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from typing import Any

# ---------------------------------------------------------------------------
# Optional stellar-sdk import — fail with a clear message if missing.
# ---------------------------------------------------------------------------
try:
    from stellar_sdk import SorobanServer, Network
    from stellar_sdk.soroban_rpc import GetLatestLedgerResponse
    from stellar_sdk.xdr import SCVal
    import stellar_sdk.scval as scval
except ImportError:
    print(
        "ERROR: stellar-sdk not installed.\n"
        "       Run: pip install 'stellar-sdk>=11.0.0'",
        file=sys.stderr,
    )
    sys.exit(1)


# ---------------------------------------------------------------------------
# Soroban RPC helpers
# ---------------------------------------------------------------------------

def _invoke_read(
    server: SorobanServer,
    contract_id: str,
    function_name: str,
    args: list[SCVal] | None = None,
    network_passphrase: str = Network.TESTNET_NETWORK_PASSPHRASE,
) -> Any:
    """
    Simulate a read-only contract call and return the decoded Python value.

    Raises RuntimeError on RPC or simulation failure.
    """
    from stellar_sdk import Keypair, TransactionBuilder, Account
    from stellar_sdk.contract import ContractClient

    # Use a throw-away keypair for simulation — no auth or signing needed.
    source_keypair = Keypair.random()
    source_account = Account(source_keypair.public_key, sequence=0)

    client = ContractClient(contract_id, server.server_url, network_passphrase)
    result = client.invoke(
        function_name,
        args or [],
        source=source_keypair.public_key,
    )
    return result


def _safe_invoke(
    server: SorobanServer,
    contract_id: str,
    fn: str,
    args: list[SCVal] | None = None,
    network_passphrase: str = Network.TESTNET_NETWORK_PASSPHRASE,
) -> Any:
    """Invoke and return result, or None on any error (graceful degradation)."""
    try:
        return _invoke_read(server, contract_id, fn, args, network_passphrase)
    except Exception as exc:  # noqa: BLE001
        return {"__error": str(exc)}


# ---------------------------------------------------------------------------
# Snapshot builder
# ---------------------------------------------------------------------------

def build_snapshot(
    server: SorobanServer,
    contract_id: str,
    network_passphrase: str,
    investor_addresses: list[str],
) -> dict[str, Any]:
    """
    Query all read-only escrow entrypoints and return a snapshot dict.

    The snapshot is intentionally read-only: it captures observable state
    without requiring any admin or SME authorization.
    """
    inv = lambda fn, args=None: _safe_invoke(  # noqa: E731
        server, contract_id, fn, args, network_passphrase
    )

    # ------------------------------------------------------------------
    # Core escrow state
    # ------------------------------------------------------------------
    escrow = inv("get_escrow")
    version = inv("get_version")
    legal_hold = inv("get_legal_hold")
    snapshot = inv("get_funding_close_snapshot")
    unique_funder_count = inv("get_unique_funder_count")
    escrow_summary = inv("get_escrow_summary")

    # ------------------------------------------------------------------
    # Immutable config (set once at init)
    # ------------------------------------------------------------------
    funding_token = inv("get_funding_token")
    treasury = inv("get_treasury")
    registry_ref = inv("get_registry_ref")
    min_contribution_floor = inv("get_min_contribution_floor")
    max_unique_investors_cap = inv("get_max_unique_investors_cap")
    distributed_principal = inv("get_distributed_principal")
    has_maturity_lock = inv("has_maturity_lock")

    # ------------------------------------------------------------------
    # Attestations
    # ------------------------------------------------------------------
    primary_attestation = inv("get_primary_attestation_hash")
    attestation_log = inv("get_attestation_append_log")

    # ------------------------------------------------------------------
    # SME collateral metadata
    # ------------------------------------------------------------------
    sme_collateral = inv("get_sme_collateral_commitment")

    # ------------------------------------------------------------------
    # Per-investor state (for each supplied address)
    # ------------------------------------------------------------------
    investor_data: dict[str, dict[str, Any]] = {}
    for addr in investor_addresses:
        addr_scval = scval.to_address(addr)
        investor_data[addr] = {
            "contribution": inv("get_contribution", [addr_scval]),
            "investor_yield_bps": inv("get_investor_yield_bps", [addr_scval]),
            "investor_claim_not_before": inv(
                "get_investor_claim_not_before", [addr_scval]
            ),
            "is_investor_claimed": inv("is_investor_claimed", [addr_scval]),
            "is_investor_refunded": inv("is_investor_refunded", [addr_scval]),
            "is_investor_allowlisted": inv("is_investor_allowlisted", [addr_scval]),
            "compute_investor_payout": inv("compute_investor_payout", [addr_scval]),
        }

    return {
        "escrow": escrow,
        "schema_version": version,
        "legal_hold": legal_hold,
        "has_maturity_lock": has_maturity_lock,
        "funding_close_snapshot": snapshot,
        "unique_funder_count": unique_funder_count,
        "escrow_summary": escrow_summary,
        "funding_token": funding_token,
        "treasury": treasury,
        "registry_ref": registry_ref,
        "min_contribution_floor": min_contribution_floor,
        "max_unique_investors_cap": max_unique_investors_cap,
        "distributed_principal": distributed_principal,
        "primary_attestation_hash": primary_attestation,
        "attestation_append_log": attestation_log,
        "sme_collateral_commitment": sme_collateral,
        "investors": investor_data,
    }


# ---------------------------------------------------------------------------
# Consistency check
# ---------------------------------------------------------------------------

def check_snapshot_consistency(snap: dict[str, Any]) -> list[str]:
    """
    Return a list of warning strings for any detectable inconsistencies.

    This is a best-effort sanity check for use before a planned upgrade — NOT
    a substitute for on-chain invariant enforcement.
    """
    warnings: list[str] = []

    escrow = snap.get("escrow") or {}
    if isinstance(escrow, dict) and "__error" not in escrow:
        status = escrow.get("status")
        funded_amount = escrow.get("funded_amount", 0)
        distributed = snap.get("distributed_principal")
        if isinstance(distributed, int) and isinstance(funded_amount, int):
            if distributed > funded_amount:
                warnings.append(
                    f"distributed_principal ({distributed}) > funded_amount "
                    f"({funded_amount}): possible accounting drift"
                )
        if status in (2, 3) and funded_amount == 0:
            warnings.append(
                f"escrow status={status} (settled/withdrawn) but funded_amount=0"
            )

    legal_hold = snap.get("legal_hold")
    if legal_hold is True:
        warnings.append(
            "Legal hold is ACTIVE — all state-changing entrypoints are blocked"
        )

    return warnings


# ---------------------------------------------------------------------------
# File output
# ---------------------------------------------------------------------------

def write_snapshot(
    snap: dict[str, Any],
    contract_id: str,
    ledger_sequence: int,
    output_dir: str,
) -> str:
    """Write snapshot to a timestamped JSON file and return the path."""
    os.makedirs(output_dir, exist_ok=True)
    now_utc = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    contract_prefix = contract_id[:8]
    filename = (
        f"escrow_backup_{contract_prefix}_seq{ledger_sequence}_{now_utc}.json"
    )
    filepath = os.path.join(output_dir, filename)

    output = {
        "_meta": {
            "tool": "backup_escrow_state.py",
            "contract_id": contract_id,
            "ledger_sequence": ledger_sequence,
            "captured_at_utc": now_utc,
            "note": (
                "Read-only snapshot. Not a rollback artefact. "
                "See docs/adr/ADR-008-backup-restore-rejection.md."
            ),
        },
        "state": snap,
    }

    with open(filepath, "w", encoding="utf-8") as fh:
        json.dump(output, fh, indent=2, default=str)

    return filepath


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Off-chain point-in-time snapshot of karis-ky escrow state.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument(
        "--rpc-url",
        required=True,
        help="Soroban RPC endpoint, e.g. https://soroban-testnet.stellar.org",
    )
    p.add_argument(
        "--contract",
        required=True,
        dest="contract_id",
        help="Bech32 / StrKey contract address (C...)",
    )
    p.add_argument(
        "--network-passphrase",
        default=Network.TESTNET_NETWORK_PASSPHRASE,
        help=(
            "Stellar network passphrase. "
            "Defaults to testnet. "
            "Use 'Public Global Stellar Network ; September 2015' for mainnet."
        ),
    )
    p.add_argument(
        "--investors",
        nargs="*",
        default=[],
        metavar="GADDR",
        help="Optional list of investor Stellar addresses to include per-investor state for.",
    )
    p.add_argument(
        "--output-dir",
        default="./backups",
        help="Directory to write snapshot JSON files (created if absent). Default: ./backups",
    )
    p.add_argument(
        "--stdout",
        action="store_true",
        help="Print the snapshot JSON to stdout instead of writing a file.",
    )
    return p.parse_args()


def main() -> None:
    args = parse_args()

    print(f"Connecting to RPC: {args.rpc_url}", file=sys.stderr)
    server = SorobanServer(args.rpc_url)

    # Fetch latest ledger for provenance metadata.
    try:
        latest: GetLatestLedgerResponse = server.get_latest_ledger()
        ledger_seq = latest.sequence
    except Exception as exc:  # noqa: BLE001
        print(f"WARNING: could not fetch latest ledger: {exc}", file=sys.stderr)
        ledger_seq = 0

    print(
        f"Snapshotting contract {args.contract_id} at ledger ~{ledger_seq}",
        file=sys.stderr,
    )

    snap = build_snapshot(
        server=server,
        contract_id=args.contract_id,
        network_passphrase=args.network_passphrase,
        investor_addresses=args.investors,
    )

    warnings = check_snapshot_consistency(snap)
    if warnings:
        print("\n⚠️  Consistency warnings:", file=sys.stderr)
        for w in warnings:
            print(f"   - {w}", file=sys.stderr)
    else:
        print("✓ Consistency check passed.", file=sys.stderr)

    if args.stdout:
        output = {
            "_meta": {
                "tool": "backup_escrow_state.py",
                "contract_id": args.contract_id,
                "ledger_sequence": ledger_seq,
                "note": (
                    "Read-only snapshot. Not a rollback artefact. "
                    "See docs/adr/ADR-008-backup-restore-rejection.md."
                ),
            },
            "state": snap,
        }
        json.dump(output, sys.stdout, indent=2, default=str)
        print()
    else:
        filepath = write_snapshot(snap, args.contract_id, ledger_seq, args.output_dir)
        print(f"✓ Snapshot written to: {filepath}", file=sys.stderr)


if __name__ == "__main__":
    main()
