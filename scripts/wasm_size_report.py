#!/usr/bin/env python3
"""
WASM size trend report generator for karis-ky escrow contract.

Usage:
    python3 scripts/wasm_size_report.py [--baseline PATH] [--out PATH] [--format {text,markdown,json}]

Reads the baseline JSON written by check_wasm_size.py and produces a trend
report showing size history, deltas, and a simple ASCII sparkline.

The report is written to stdout (default) or --out file.
"""

import argparse
import json
import sys
from pathlib import Path

DEFAULT_BASELINE = "scripts/wasm_size_baseline.json"
ABSOLUTE_LIMIT_BYTES = 1_000_000
RELATIVE_THRESHOLD = 0.10

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _human(size_bytes: int | None) -> str:
    if size_bytes is None:
        return "—"
    if size_bytes < 1024:
        return f"{size_bytes} B"
    kb = size_bytes / 1024
    if kb < 1024:
        return f"{kb:.2f} KB"
    return f"{kb / 1024:.2f} MB"


def _pct(a: int, b: int) -> str:
    """Return percentage change from b to a as a signed string."""
    if b == 0:
        return "N/A"
    pct = (a - b) / b * 100
    sign = "+" if pct >= 0 else ""
    return f"{sign}{pct:.1f}%"


def _sparkline(values: list[int], width: int = 20) -> str:
    """Build an ASCII sparkline from a list of int values."""
    if not values:
        return ""
    blocks = " ▁▂▃▄▅▆▇█"
    lo, hi = min(values), max(values)
    span = hi - lo or 1
    result = []
    # Sample up to `width` evenly spaced points
    indices = [int(i * (len(values) - 1) / (width - 1)) for i in range(min(width, len(values)))]
    for i in indices:
        level = int((values[i] - lo) / span * (len(blocks) - 1))
        result.append(blocks[level])
    return "".join(result)


# ---------------------------------------------------------------------------
# Report builders
# ---------------------------------------------------------------------------

def _build_rows(history: list[dict], current_bytes: int | None) -> list[dict]:
    """
    Combine history entries and the current baseline into a unified list of
    rows sorted oldest-first, each with delta vs. previous entry.
    """
    entries = list(history)
    if current_bytes is not None:
        entries.append(
            {
                "date": "current",
                "commit": "—",
                "ref": "—",
                "bytes": current_bytes,
                "_is_current": True,
            }
        )

    rows = []
    for i, entry in enumerate(entries):
        prev_bytes = entries[i - 1]["bytes"] if i > 0 else None
        delta_str = ""
        alert = False
        if prev_bytes is not None:
            delta = entry["bytes"] - prev_bytes
            delta_str = _pct(entry["bytes"], prev_bytes)
            if delta > 0 and (entry["bytes"] - prev_bytes) / prev_bytes > RELATIVE_THRESHOLD:
                alert = True
        rows.append(
            {
                "date": entry.get("date", "—"),
                "commit": entry.get("commit", "—"),
                "ref": entry.get("ref", "—"),
                "bytes": entry["bytes"],
                "delta": delta_str,
                "alert": alert,
                "is_current": entry.get("_is_current", False),
            }
        )
    return rows


def report_text(data: dict) -> str:
    """Plain-text trend report."""
    lines = []
    history: list[dict] = data.get("history", [])
    current_bytes: int | None = data.get("baseline_bytes")

    lines.append("=" * 70)
    lines.append("  karis-ky Escrow · WASM Size Trend Report")
    lines.append("=" * 70)
    lines.append(f"  Absolute limit : {_human(ABSOLUTE_LIMIT_BYTES)}")
    lines.append(f"  Growth alert   : >{RELATIVE_THRESHOLD:.0%} vs. previous recorded build")
    lines.append(f"  Current size   : {_human(current_bytes)}")
    if data.get("last_updated"):
        lines.append(f"  Last updated   : {data['last_updated']}")
    if data.get("last_commit"):
        lines.append(f"  Last commit    : {data['last_commit']}  ({data.get('last_ref', '')})")
    lines.append("")

    all_entries = list(history)
    if current_bytes is not None:
        all_entries.append({"bytes": current_bytes})
    if len(all_entries) >= 2:
        spark = _sparkline([e["bytes"] for e in all_entries])
        lines.append(f"  Trend  (oldest → newest): {spark}")
        lines.append("")

    rows = _build_rows(history, current_bytes)
    if not rows:
        lines.append("  No recorded history yet.")
        lines.append("  Run: python3 scripts/check_wasm_size.py --record")
    else:
        # Header
        lines.append(
            f"  {'Date':<26}  {'Commit':<8}  {'Ref':<20}  {'Size':>10}  {'Delta':>8}  Flag"
        )
        lines.append(f"  {'-'*26}  {'-'*8}  {'-'*20}  {'-'*10}  {'-'*8}  ----")
        for row in rows:
            flag = "⚠ ALERT" if row["alert"] else ("← current" if row["is_current"] else "")
            over = " [OVER LIMIT]" if row["bytes"] > ABSOLUTE_LIMIT_BYTES else ""
            lines.append(
                f"  {row['date']:<26}  {row['commit']:<8}  {row['ref']:<20}  "
                f"{_human(row['bytes']):>10}  {row['delta']:>8}  {flag}{over}"
            )

    lines.append("")
    lines.append("=" * 70)
    return "\n".join(lines)


def report_markdown(data: dict) -> str:
    """Markdown trend report (suitable for GitHub Actions job summary)."""
    lines = []
    history: list[dict] = data.get("history", [])
    current_bytes: int | None = data.get("baseline_bytes")

    lines.append("## 📦 WASM Size Trend Report")
    lines.append("")
    lines.append(f"| | |")
    lines.append(f"|---|---|")
    lines.append(f"| **Current size** | {_human(current_bytes)} |")
    lines.append(f"| **Absolute limit** | {_human(ABSOLUTE_LIMIT_BYTES)} |")
    lines.append(f"| **Growth alert threshold** | >{RELATIVE_THRESHOLD:.0%} |")
    if data.get("last_updated"):
        lines.append(f"| **Last updated** | {data['last_updated']} |")
    if data.get("last_commit"):
        lines.append(f"| **Last commit** | `{data['last_commit']}` ({data.get('last_ref', '')}) |")
    lines.append("")

    rows = _build_rows(history, current_bytes)
    if not rows:
        lines.append(
            "_No history recorded yet. Run `python3 scripts/check_wasm_size.py --record` "
            "on a passing build to start tracking._"
        )
    else:
        lines.append("| Date | Commit | Ref | Size | Delta | Status |")
        lines.append("|------|--------|-----|-----:|------:|--------|")
        for row in rows:
            if row["alert"]:
                status = "⚠️ ALERT"
            elif row["bytes"] > ABSOLUTE_LIMIT_BYTES:
                status = "🚨 OVER LIMIT"
            elif row["is_current"]:
                status = "✅ current"
            else:
                status = "✅"
            commit = f"`{row['commit']}`" if row["commit"] != "—" else "—"
            lines.append(
                f"| {row['date']} | {commit} | {row['ref']} "
                f"| {_human(row['bytes'])} | {row['delta']} | {status} |"
            )

    lines.append("")

    # Sparkline
    all_entries = list(history)
    if current_bytes is not None:
        all_entries.append({"bytes": current_bytes})
    if len(all_entries) >= 2:
        spark = _sparkline([e["bytes"] for e in all_entries])
        lines.append(f"**Trend** (oldest → newest): `{spark}`")
        lines.append("")

    return "\n".join(lines)


def report_json(data: dict) -> str:
    """Re-emit the baseline JSON as pretty-printed JSON."""
    return json.dumps(data, indent=2)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate a WASM size trend report for karis-ky escrow."
    )
    parser.add_argument(
        "--baseline",
        default=DEFAULT_BASELINE,
        help=f"Path to baseline JSON (default: {DEFAULT_BASELINE})",
    )
    parser.add_argument(
        "--out",
        default=None,
        help="Output file path (default: stdout)",
    )
    parser.add_argument(
        "--format",
        choices=["text", "markdown", "json"],
        default="text",
        help="Report format (default: text)",
    )
    args = parser.parse_args()

    baseline_path = Path(args.baseline)
    if not baseline_path.exists():
        print(
            f"ERROR: Baseline file not found: {baseline_path}\n"
            "Run check_wasm_size.py --record first.",
            file=sys.stderr,
        )
        sys.exit(1)

    with baseline_path.open() as fh:
        data = json.load(fh)

    if args.format == "text":
        output = report_text(data)
    elif args.format == "markdown":
        output = report_markdown(data)
    else:
        output = report_json(data)

    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out).write_text(output + "\n")
        print(f"Report written to {args.out}")
    else:
        print(output)


if __name__ == "__main__":
    main()
