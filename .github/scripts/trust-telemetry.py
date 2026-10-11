#!/usr/bin/env python3
"""Aggregate the testing portfolio's trust ledger from the trust table (#449, ADR-0043).

ADR-0043's audit-by-number half: cumulative fuzz CPU-hours, new crashers,
mutation score, and region coverage, machine-written by the scheduled test
workflows themselves and rendered on the status page. Ledger numbers are
maps, never gates (the Goodhart guard is binding); no required check reads
this output.

The rows are the `trust` table in D1 (ADR-0060, #891 slice 4): one per
entry a writer uploads as its `trust-<role>-<run-id>-<attempt>` artifact,
written beside it by trust-row.py and reconciled from the artifacts every
Sunday. Cumulative figures are cumulative since the table's first row,
which the output names as `since`; the artifact walk this replaced summed
the trailing 90-day retention window and the page said so.

Each entry is `{date, run_id, role, fuzz_cpu_s, crashers_new,
mutation_score, coverage_region_pct}`; a writer leaves the fields it does
not measure null (#450/#451 fuzzing, #454 coverage, #455 mutation own
theirs). `loc_code`/`loc_test` from the issue's schema are deliberately not
carried per entry: the status page already publishes current source line
counts from status-data.py, and stamping every writer with a duplicate
source-tree walk would fork that single source of truth.

Self-diagnosing per run-telemetry.py's rule: any failure writes a section
saying what was actually seen and exits 0.

Usage: trust-telemetry.py <out.json>
"""

import json
import sys
from datetime import datetime, timezone

import d1

out_path = sys.argv[1]

FIELDS = ("fuzz_cpu_s", "crashers_new", "mutation_score", "coverage_region_pct")
SELECT = ("SELECT run_id, at, role, fuzz_cpu_s, crashers_new, mutation_score, "
          "coverage_region_pct FROM trust ORDER BY at, run_id, attempt")


def write(obj):
    with open(out_path, "w", encoding="utf-8") as fh:
        json.dump(obj, fh, indent=1)
        fh.write("\n")


def bail(reason):
    write({
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "error": reason,
        "count": 0,
        "entries": [],
    })
    sys.exit(0)


try:
    rows = d1.query(SELECT)
except d1.Unreadable as exc:
    bail(f"Could not read the trust table ({exc}).")

if not rows:
    bail("The trust table is empty: no scheduled test workflow has written a "
         "row since the table landed (#891 slice 4).")

entries = [{"date": r["at"], "run_id": str(r["run_id"]), "role": r["role"],
            **{f: r.get(f) for f in FIELDS}} for r in rows]


def latest_and_previous(field):
    """The two most recent entries that actually measured `field`."""
    seen = [e[field] for e in entries if e.get(field) is not None]
    if not seen:
        return None, None
    if len(seen) == 1:
        return seen[-1], None
    return seen[-1], seen[-2]


trend = {f: dict(zip(("latest", "previous"), latest_and_previous(f))) for f in FIELDS}

write({
    "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "count": len(entries),
    "since": entries[0]["date"],
    "cumulative_fuzz_cpu_hours": sum(e["fuzz_cpu_s"] or 0 for e in entries) / 3600.0,
    "crashers_total": sum(e["crashers_new"] or 0 for e in entries),
    "trend": trend,
    "entries": entries[-25:],
})
print(f"trust-telemetry: {len(entries)} entries since {entries[0]['date']}")
