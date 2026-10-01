#!/usr/bin/env python3
"""Turn a cargo-llvm-cov region report into a trust-ledger entry (#454).

`coverage-check.yml` runs `cargo llvm-cov --package mothergod --json` and
hands this script that JSON. It writes the one field this writer owns,
`coverage_region_pct`, in the schema `trust-telemetry.py` documents
(`{date, run_id, role, fuzz_cpu_s, crashers_new, mutation_score,
coverage_region_pct}`); the other three stay null, same convention
`fuzz-check.yml` already uses for the fields it doesn't own.

Region, not line: line coverage counts a `match` arm's opening brace as
covered the instant any arm runs. Region coverage is per-arm, per-branch of
a boolean, matching how a decoder actually fails (docs/TESTING.md #9).

The worst-covered files are not stored in the ledger: the schema above is
documented in one place (`trust-telemetry.py`) and this writer does not fork
it for one field nobody trends yet. They are printed as `::notice::`
annotations instead, the same place `mutants-check.yml` already surfaces its
missed-mutant list, so a heartbeat doing triage finds them in the run log
without a second source of truth to keep in sync.

Self-diagnosing per `run-telemetry.py`'s rule: a missing or unreadable
report writes a null-coverage entry naming why, and exits 0, so an
instrumentation failure here never blocks the workflow's other steps.

Usage: coverage-ledger.py <llvm-cov.json> <entry.json>
"""

import json
import os
import sys
from datetime import datetime, timezone

WORST_N = 3


def entry(coverage_region_pct):
    return {
        "date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "run_id": os.environ.get("GITHUB_RUN_ID", ""),
        "role": "coverage",
        "fuzz_cpu_s": None,
        "crashers_new": None,
        "mutation_score": None,
        "coverage_region_pct": coverage_region_pct,
    }


def write(out_path, obj):
    with open(out_path, "w", encoding="utf-8") as fh:
        json.dump(obj, fh, indent=1)
        fh.write("\n")


def bail(out_path, reason):
    print(f"::warning::coverage-ledger: {reason}")
    write(out_path, entry(None))
    sys.exit(0)


def main(report_path, out_path):
    if not os.path.isfile(report_path):
        bail(out_path, f"{report_path} does not exist; cargo-llvm-cov did not run to completion.")

    try:
        with open(report_path, encoding="utf-8") as fh:
            report = json.load(fh)
        data = report["data"][0]
        pct = round(data["totals"]["regions"]["percent"], 2)
    except (ValueError, KeyError, IndexError, TypeError) as exc:
        bail(out_path, f"could not read {report_path}: {type(exc).__name__}: {exc}")
        return

    root = os.getcwd()
    worst = []
    for f in data.get("files", []):
        regions = f.get("summary", {}).get("regions", {})
        if not regions.get("count"):
            continue
        name = os.path.relpath(f["filename"], root)
        worst.append((regions["percent"], name))
    worst.sort()

    for pct_f, name in worst[:WORST_N]:
        print(f"::notice::coverage-ledger: {name} at {pct_f:.2f}% region coverage")

    write(out_path, entry(pct))
    print(f"coverage-ledger: crate at {pct:.2f}% region coverage, "
          f"{len(worst)} file(s) measured")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <llvm-cov.json> <entry.json>", file=sys.stderr)
        sys.exit(2)
    main(sys.argv[1], sys.argv[2])
