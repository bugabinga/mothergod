#!/usr/bin/env python3
"""Append one agent run's row to the session table (ADR-0060).

Called by `.github/actions/agent-audit` once the artifact is extracted, with
the audit output directory as argv[1]: the row is derived from the same
scrubbed files the artifact publishes and nothing more. Numbers and
identifiers only, through audit_facts.row(), the one whitelist every reader
of an artifact shares; no prose reaches the table (ADR-0019). The same row
written from the archive is `sessions backfill`.

The run's allowance readings follow as rows of the allowance table (#891
slice 2), through audit_facts.allowance_rows, the same function `sessions
backfill` writes them with from the archive.

Never fails the run: exit 0 on every path, because observability does not
get to break the thing it observes (ADR-0023). A row that could not be
written is a `::warning::` annotation, and the artifact remains that run's
record. No CLOUDFLARE_API_TOKEN is a valid configuration, a workflow that did
not pass one, and gets one line, not a warning.

Usage: session-row.py <audit-dir>
"""

import json
import os
import sys
from datetime import datetime, timezone

import d1
from audit_facts import INSERT, allowance_insert, allowance_rows, row


def size_in(audit_dir):
    """The byte count of one published text in the directory, None when absent."""
    def size(name):
        try:
            return os.path.getsize(os.path.join(audit_dir, name))
        except OSError:
            return None
    return size


def main(argv):
    if len(argv) != 2:
        print("::warning::session-row: usage: session-row.py <audit-dir>")
        return 0
    audit_dir = argv[1]
    if not os.environ.get("CLOUDFLARE_API_TOKEN"):
        print("session-row: skipped, no CLOUDFLARE_API_TOKEN")
        return 0
    try:
        with open(os.path.join(audit_dir, "metadata.json"), encoding="utf-8") as fh:
            meta = json.load(fh)
        if not isinstance(meta, dict):
            raise ValueError("metadata.json is not an object")
    except (OSError, ValueError) as error:
        print(f"::warning::session-row: not written: {error}")
        return 0
    meta["_at"] = datetime.now(timezone.utc)
    values = row(meta, size_in(audit_dir))
    if values[0] is None:
        print("::warning::session-row: not written: metadata carries no run_id")
        return 0
    try:
        d1.query(INSERT, values)
    except d1.Unreadable as error:
        print(f"::warning::session-row: not written: {error}")
        return 0
    windows = ""
    readings = allowance_rows(meta)
    if readings:
        try:
            d1.query(allowance_insert(readings), [v for r in readings for v in r])
            windows = ", allowance " + " ".join(r[2] for r in readings)
        except d1.Unreadable as error:
            print(f"::warning::session-row: allowance not written: {error}")
    print(f"session-row: wrote run {values[0]} attempt {values[1]} for {values[3]}{windows}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
