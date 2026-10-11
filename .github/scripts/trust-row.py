#!/usr/bin/env python3
"""Write trust-ledger entries to the trust table (#891 slice 4, ADR-0060).

The ledger (#449, ADR-0043) is one `entry.json` per scheduled test run,
`{date, run_id, role, fuzz_cpu_s, crashers_new, mutation_score,
coverage_region_pct}`, uploaded as the artifact `trust-<role>-<run-id>-
<attempt>` with a 90-day retention. The artifact stays the record a human
opens; the row is the series the TRUST scorecard reads, and it outlives
the retention, so "cumulative fuzz CPU-hours" is cumulative.

Two verbs.

`trust-row.py <entry.json>` is the live write, the step after the artifact
upload in fuzz-check.yml and coverage-check.yml; the attempt is
GITHUB_RUN_ATTEMPT. It never fails the run: exit 0 on every path
(ADR-0023), a row that could not be written is a `::warning::`
annotation, and no CLOUDFLARE_API_TOKEN is a configuration, one line.

`trust-row.py backfill [--repo owner/name]` writes the rows the unexpired
artifacts hold and the table lacks, the attempt from the artifact's name,
then prints the census and exits 1 when an artifact has no row, the
contract `sessions backfill` carries. agent-model-intel.yml runs it every
Sunday, so a live write that failed is repaired within a week, and its
first run gave the table its history.

Entries dated before CRASHERS_MEASURED_SINCE counted every file libFuzzer
left, slow-unit-* included, as a crasher (survey, 2026-09-13): their
crashers_new is written NULL, unmeasured, once here, so a sum over the
table is true without every reader knowing the date.

Usage: trust-row.py <entry.json> | backfill [--repo owner/name]
"""

import io
import json
import math
import os
import re
import sys
import zipfile

import audit_facts
import d1

COLUMNS = ("run_id", "attempt", "at", "role", "fuzz_cpu_s", "crashers_new",
           "mutation_score", "coverage_region_pct")
INSERT = (f"INSERT OR REPLACE INTO trust ({', '.join(COLUMNS)}) "
          f"VALUES ({', '.join('?' * len(COLUMNS))})")
NAME = re.compile(r"^trust-([a-z]+)-(\d+)-(\d+)$")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
CRASHERS_MEASURED_SINCE = "2026-09-13T06:00:00Z"


def number(value, field, whole=False):
    """`value` as a finite number, None as None; anything else raises."""
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f"{field} is {value!r}, not a number")
    if whole:
        if value != int(value):
            raise ValueError(f"{field} is {value!r}, not a count")
        return int(value)
    return float(value)


def row(entry, attempt):
    """The entry as the table's row; raises on a field the schema refuses."""
    if not isinstance(entry, dict):
        raise ValueError("entry is not an object")
    at = entry.get("date")
    if not isinstance(at, str) or not DATE.match(at):
        raise ValueError(f"date is {at!r}, not ISO-8601 UTC")
    role = entry.get("role")
    if not isinstance(role, str) or not role:
        raise ValueError(f"role is {role!r}")
    crashers = number(entry.get("crashers_new"), "crashers_new", whole=True)
    if at < CRASHERS_MEASURED_SINCE:
        crashers = None
    return (
        int(str(entry.get("run_id", ""))),  # the writers print it as a string
        int(attempt),
        at,
        role,
        number(entry.get("fuzz_cpu_s"), "fuzz_cpu_s"),
        crashers,
        number(entry.get("mutation_score"), "mutation_score"),
        number(entry.get("coverage_region_pct"), "coverage_region_pct"),
    )


def live(path):
    if not os.environ.get("CLOUDFLARE_API_TOKEN"):
        print("trust-row: skipped, no CLOUDFLARE_API_TOKEN")
        return 0
    try:
        with open(path, encoding="utf-8") as fh:
            entry = json.load(fh)
        values = row(entry, os.environ.get("GITHUB_RUN_ATTEMPT") or 1)
        d1.query(INSERT, values)
    except Exception as error:  # noqa: BLE001, the docstring's exit-0 invariant
        print(f"::warning::trust-row: not written: {type(error).__name__}: {error}")
        return 0
    print(f"trust-row: wrote run {values[0]} attempt {values[1]} for {values[3]}")
    return 0


def key_of(name):
    match = NAME.match(name)
    return (int(match.group(2)), int(match.group(3))) if match else None


def keys_in_table():
    return {(r["run_id"], r["attempt"])
            for r in d1.query("SELECT run_id, attempt FROM trust")}


def backfill(repo):
    """Write the artifacts' rows the table lacks; print the census; 1 on a gap.

    Oldest first, as `archive` lists them, and idempotent: a second run
    finds every row present and downloads nothing. ONE ARTIFACT COSTS ONE
    UNREADABLE LINE, NEVER THE WALK (audit_facts.walk's invariant, held
    the same way: every exception is the type and the message), while a
    database that refuses a read or a write ends the walk as UNREADABLE,
    because a census over a dead table would be the lie an empty one is.
    """
    present = keys_in_table()
    artifacts = audit_facts.archive(repo, prefix="trust-")
    keys = {a.name: key_of(a.name) for a in artifacts}
    held = sum(key in present for key in keys.values())
    written, unreadable = 0, []
    for artifact in artifacts:
        key = keys[artifact.name]
        if key is None:
            unreadable.append(f"{artifact.name}: name is not trust-<role>-<run-id>-<attempt>")
            continue
        if key in present:
            continue
        try:
            blob = audit_facts.gh("api", f"repos/{repo}/actions/artifacts/{artifact.id}/zip")
            entry = json.loads(zipfile.ZipFile(io.BytesIO(blob)).read("entry.json"))
            values = row(entry, key[1])
        except Exception as error:  # noqa: BLE001, the invariant above
            unreadable.append(f"{artifact.name}: {type(error).__name__}: {str(error)[:200]}")
            continue
        d1.query(INSERT, values)
        keys[artifact.name] = (values[0], values[1])
        written += 1
    have = keys_in_table()
    missing = [name for name, key in keys.items() if key not in have]
    for line in unreadable:
        print(f"  unreadable {line}")
    floor = artifacts[0].made.strftime("%Y-%m-%dT%H:%M:%SZ") if artifacts else "none"
    print(f"trust-row backfill: artifacts {len(artifacts)} since {floor}"
          f" | present {held} | written {written}"
          f" | unreadable {len(unreadable)} | missing {len(missing)}")
    return 1 if missing else 0


def main(argv):
    verb = argv[1] if len(argv) > 1 else ""
    if verb == "backfill":
        args = argv[2:]
        repo = args[args.index("--repo") + 1] if "--repo" in args[:-1] else os.environ.get("GITHUB_REPOSITORY")
        if not repo:
            print("trust-row: backfill needs GITHUB_REPOSITORY or --repo owner/name")
            return 2
        try:
            return backfill(repo)
        except (d1.Unreadable, audit_facts.Unreadable) as error:
            print(f"trust-row: UNREADABLE: {error}")
            return 1
    if verb and len(argv) == 2:
        return live(verb)
    print(__doc__.strip().splitlines()[-1])
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
