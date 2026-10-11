#!/usr/bin/env python3
"""Write one run gate evaluation to the guard table (#891 slice 3, ADR-0060).

Run by `.github/actions/agent-guard` as its last line, after the decision is
made, logged and emitted as outputs, with the decision in the environment:
GUARD_DECISION (run, skip, exhausted, paused, stale), GUARD_REASON (the
gate's log line), GUARD_RUNG, GUARD_EFFORT and GUARD_PROJECTION (week-average
seven-day burn in %/h, '' when the reading reaches the reset or there is
none), beside the run's GITHUB_RUN_ID, GITHUB_RUN_ATTEMPT, GITHUB_EVENT_NAME,
ROLE and DISCRETIONARY. A wake the gate stops never reaches agent-audit, so
this row is the only record that the wake existed and why it did not run;
retrospect's "skipped N" and the survey's HEALTH line become a query over it
(#896).

Never fails the run: exit 0 on every path (ADR-0023). A row that could not be
written is a `::warning::` annotation; no CLOUDFLARE_API_TOKEN is a valid
configuration, a caller that passed none, and gets one line, not a warning.
Nothing on the "may this seat run" path reads the database (#891's rules):
the gate's outputs are written before this runs and never wait on it.

Usage: guard-row.py
"""

import os
import sys
from datetime import datetime, timezone

import d1

COLUMNS = ("run_id", "attempt", "at", "role", "event", "discretionary",
           "decision", "reason", "rung", "effort", "projection")
INSERT = (f"INSERT OR REPLACE INTO guard ({', '.join(COLUMNS)}) "
          f"VALUES ({', '.join('?' * len(COLUMNS))})")
DECISIONS = ("run", "skip", "exhausted", "paused", "stale")


def row(env, now):
    """The environment as the table's row; raises on a field the schema refuses."""
    decision = env.get("GUARD_DECISION", "")
    if decision not in DECISIONS:
        raise ValueError(f"GUARD_DECISION {decision!r} is none of {', '.join(DECISIONS)}")
    projection = env.get("GUARD_PROJECTION", "")
    return (
        int(env["GITHUB_RUN_ID"]),
        int(env.get("GITHUB_RUN_ATTEMPT") or 1),
        now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        env.get("ROLE", ""),
        env.get("GITHUB_EVENT_NAME") or None,
        1 if env.get("DISCRETIONARY") == "true" else 0,
        decision,
        env.get("GUARD_REASON") or None,
        env.get("GUARD_RUNG") or None,
        env.get("GUARD_EFFORT") or None,
        float(projection) if projection else None,
    )


def main():
    if not os.environ.get("CLOUDFLARE_API_TOKEN"):
        print("guard-row: skipped, no CLOUDFLARE_API_TOKEN")
        return 0
    try:
        values = row(os.environ, datetime.now(timezone.utc))
        d1.query(INSERT, values)
    except Exception as error:  # noqa: BLE001, the docstring's exit-0 invariant
        print(f"::warning::guard-row: not written: {type(error).__name__}: {error}")
        return 0
    print(f"guard-row: wrote run {values[0]} attempt {values[1]} for {values[3]}: {values[6]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
