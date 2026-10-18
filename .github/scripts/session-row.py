#!/usr/bin/env python3
"""Append one agent run's row to the session table (ADR-0060).

Called by `.github/actions/agent-audit` once the artifact is extracted, with
the audit output directory as argv[1]: the row is derived from the same
scrubbed files the artifact publishes and nothing more. Numbers and
identifiers only, through audit_facts.row(), the one whitelist every reader
of an artifact shares; no prose reaches the table (ADR-0019). The same row
written from the archive is `sessions backfill`.

The run's allowance readings (#891 slice 2), tool counts (slice 5) and
walls (#1241) are written before the session row, in audit_facts.statements' order, the one
`sessions backfill` writes from the archive too: a derived write that fails
leaves the run absent from `sessions`, and the Sunday walk rewrites all
four from the artifact (#1102). A reading or count the metadata cannot
yield is a `::warning::` and that table alone stays empty, because the walk
would raise on the same field and has nothing to heal.

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
from audit_facts import allowance_rows, row, statements, tools_rows, walls_rows


def size_in(audit_dir):
    """The byte count of one published text in the directory, None when absent."""
    def size(name):
        try:
            return os.path.getsize(os.path.join(audit_dir, name))
        except OSError:
            return None
    return size


def derive(label, rows):
    """One derived table's rows from the metadata, or none with a warning:
    a field of the wrong shape costs that table, never the session row."""
    try:
        return rows()
    except Exception as error:  # noqa: BLE001, the docstring's exit-0 invariant
        print(f"::warning::session-row: {label} not written: {type(error).__name__}: {error}")
        return []


def write(audit_dir):
    """Write the run's allowance readings, tool counts, walls and row, in
    that order; print the status line.

    Raises on anything that stops the row: a missing or malformed
    metadata.json, a field row() cannot turn into a number, a database that
    refuses any of the writes. main() turns the raise into the warning.
    """
    with open(os.path.join(audit_dir, "metadata.json"), encoding="utf-8") as fh:
        meta = json.load(fh)
    if not isinstance(meta, dict):
        raise ValueError("metadata.json is not an object")
    meta["_at"] = datetime.now(timezone.utc)
    values = row(meta, size_in(audit_dir))
    if values[0] is None:
        raise ValueError("metadata carries no run_id")
    readings = derive("allowance", lambda: allowance_rows(meta))
    tool_rows = derive("tools", lambda: tools_rows(meta))
    wall_rows = derive("walls", lambda: walls_rows(meta))
    for sql, params in statements(values, readings, tool_rows, wall_rows):
        d1.query(sql, params)
    suffix = ", allowance " + " ".join(r[2] for r in readings) if readings else ""
    suffix += f", tools {len(tool_rows)}" if tool_rows else ""
    suffix += f", walls {len(wall_rows)}" if wall_rows else ""
    print(f"session-row: wrote run {values[0]} attempt {values[1]} for {values[3]}{suffix}")


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
