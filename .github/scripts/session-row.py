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


def write(audit_dir):
    """Write the run's row, then its allowance readings; print the status line.

    Raises on anything that stops the row: a missing or malformed
    metadata.json, a field row() cannot turn into a number, a database that
    refuses the write. main() turns the raise into the warning.
    """
    with open(os.path.join(audit_dir, "metadata.json"), encoding="utf-8") as fh:
        meta = json.load(fh)
    if not isinstance(meta, dict):
        raise ValueError("metadata.json is not an object")
    meta["_at"] = datetime.now(timezone.utc)
    values = row(meta, size_in(audit_dir))
    if values[0] is None:
        raise ValueError("metadata carries no run_id")
    d1.query(INSERT, values)
    # The session row is in by here, so an allowance miss is its own
    # warning and the status line still reports the row.
    windows = ""
    try:
        readings = allowance_rows(meta)
        if readings:
            d1.query(allowance_insert(readings), [v for r in readings for v in r])
            windows = ", allowance " + " ".join(r[2] for r in readings)
    except Exception as error:  # noqa: BLE001, the docstring's exit-0 invariant
        print(f"::warning::session-row: allowance not written: {type(error).__name__}: {error}")
    print(f"session-row: wrote run {values[0]} attempt {values[1]} for {values[3]}{windows}")


def main(argv):
    if len(argv) != 2:
        print("::warning::session-row: usage: session-row.py <audit-dir>")
        return 0
    if not os.environ.get("CLOUDFLARE_API_TOKEN"):
        print("session-row: skipped, no CLOUDFLARE_API_TOKEN")
        return 0
    # One guard over every read of the untrusted metadata and every write:
    # the live writer called row() bare and a thinking_tokens past 1e308
    # exited 1 through the audit action (#1002).
    try:
        write(argv[1])
    except Exception as error:  # noqa: BLE001, the docstring's exit-0 invariant
        print(f"::warning::session-row: not written: {type(error).__name__}: {error}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
