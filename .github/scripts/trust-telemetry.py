#!/usr/bin/env python3
"""Aggregate the testing portfolio's trust ledger from run artifacts (#449, ADR-0043).

ADR-0043's audit-by-number half: cumulative fuzz CPU-hours, new crashers,
mutation score, and region coverage, machine-written by the scheduled test
workflows themselves and rendered on the status page. Ledger numbers are
maps, never gates (the Goodhart guard is binding); no required check reads
this output.

Same pipeline as run-telemetry.py's run economics: each writer uploads its
own small `entry.json` artifact (`trust-<role>-<run-id>-<attempt>`, see
fuzz-check.yml) instead of committing to a tracked file, which is what
deploy-site.yml already avoids for status-data.json and agent-metrics.json
(concurrent-append conflicts on a tracked file, PR #34). The cost this
pattern buys: cumulative figures below cover the trailing artifact
retention window (90 days), not all of project history. The weekly digest
is the durable record past that horizon, the same trade run-telemetry.py's
cost figures already make.

Each entry is `{date, run_id, role, fuzz_cpu_s, crashers_new,
mutation_score, coverage_region_pct}`; a writer omits or nulls fields it
does not measure (#450/#451 fuzzing, #454 coverage, #455 mutation own
theirs). `loc_code`/`loc_test` from the issue's schema are deliberately not
carried per entry: the status page already publishes current source line
counts from status-data.py, and stamping every writer with a duplicate
source-tree walk would fork that single source of truth.

Self-diagnosing per run-telemetry.py's rule: any failure writes a section
saying what was actually seen and exits 0.

Usage: trust-telemetry.py <out.json>
"""

import io
import json
import os
import subprocess
import sys
import zipfile
from datetime import datetime, timezone

out_path = sys.argv[1]
REPO = os.environ.get("GITHUB_REPOSITORY", "")

# Headroom, not a budget: weekly/monthly cadence writers keep this list
# short for a long time. A breach is reported, never silently truncated.
MAX_DOWNLOADS = 200

FIELDS = ("fuzz_cpu_s", "crashers_new", "mutation_score", "coverage_region_pct")


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


def gh(*args, binary=False):
    proc = subprocess.run(["gh", *args], capture_output=True)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode("utf-8", "replace")[:400])
    return proc.stdout if binary else proc.stdout.decode("utf-8", "replace")


if not REPO:
    bail("GITHUB_REPOSITORY is unset; nothing to query.")

try:
    pages = json.loads(gh("api", "--paginate", "--slurp",
                          f"repos/{REPO}/actions/artifacts?per_page=100"))
except (RuntimeError, ValueError) as exc:
    bail(f"Could not list artifacts ({exc}).")

wanted = []
for page in pages:
    for art in page.get("artifacts", []):
        if not art.get("name", "").startswith("trust-") or art.get("expired"):
            continue
        wanted.append((art["id"], art.get("created_at") or ""))

if not wanted:
    bail("No unexpired trust-ledger artifacts yet: the scheduled test "
         "workflows have not reported since this ledger landed (#449).")

truncated = 0
if len(wanted) > MAX_DOWNLOADS:
    wanted.sort(key=lambda p: p[1])
    truncated = len(wanted) - MAX_DOWNLOADS
    wanted = wanted[truncated:]

entries, unreadable = [], 0
for art_id, _created in wanted:
    try:
        blob = gh("api", f"repos/{REPO}/actions/artifacts/{art_id}/zip", binary=True)
        entry = json.loads(zipfile.ZipFile(io.BytesIO(blob)).read("entry.json"))
    except (RuntimeError, ValueError, KeyError, zipfile.BadZipFile):
        unreadable += 1
        continue
    if isinstance(entry, dict) and entry.get("date") and entry.get("run_id"):
        entries.append(entry)
    else:
        unreadable += 1

if not entries:
    bail(f"Found {len(wanted)} trust-ledger artifacts, none readable.")

entries.sort(key=lambda e: (e["date"], str(e["run_id"])))


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
    "unreadable": unreadable,
    "truncated": truncated,
    # Sum over whatever is still in the retention window, not all-time
    # (see module docstring); the label on the consuming page says so.
    "cumulative_fuzz_cpu_hours": sum(e.get("fuzz_cpu_s") or 0 for e in entries) / 3600.0,
    "crashers_total": sum(e.get("crashers_new") or 0 for e in entries),
    "trend": trend,
    "entries": entries[-25:],
})
print(f"trust-telemetry: {len(entries)} entries, {unreadable} unreadable, "
      f"{truncated} over cap")
