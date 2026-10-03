#!/usr/bin/env python3
"""Append one agent run's row to the session table (ADR-0060).

Called by `.github/actions/agent-audit` once the artifact is extracted, with
the audit output directory as argv[1]: the row is derived from the same
scrubbed files the artifact publishes and nothing more. Numbers and
identifiers only, through audit_facts.facts(), the whitelist run-telemetry.py
reads; no prose reaches the table (ADR-0019).

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
from audit_facts import facts

COLUMNS = (
    "run_id", "attempt", "at", "role", "event", "actor", "number",
    "commit_sha", "measured", "model", "out_tokens", "think_pct", "cost_usd",
    "turns", "duration_ms", "denials", "error", "stop_reason", "persona_sha",
    "prompt_bytes", "response_bytes",
)
INSERT = (f"INSERT OR REPLACE INTO sessions ({', '.join(COLUMNS)}) "
          f"VALUES ({', '.join('?' * len(COLUMNS))})")


def as_int(value):
    """An id or count as an integer, or None: '' and junk are absent, not 0."""
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def size_of(audit_dir, name, extracted):
    """Bytes of one published text, None when the extractor found nothing.

    The extractor writes a placeholder line where the text was missing; its
    byte count would read as a tiny prompt, so the extracted flag decides.
    """
    if not extracted:
        return None
    try:
        return os.path.getsize(os.path.join(audit_dir, name))
    except OSError:
        return None


def row_of(meta, audit_dir, now):
    """One table row from the artifact's metadata, in COLUMNS order."""
    meta["_at"] = now
    f = facts(meta)
    return (
        as_int(f["run_id"]), as_int(f["attempt"]) or 1, f["at"], f["role"],
        f["event"] or None, f["actor"] or None, as_int(f["number"]),
        f["commit"] or None, int(f["measured"]), f["model"] or None,
        f["out"] if f["measured"] else None, f["think"], f["cost"],
        f["turns"], f["duration_ms"], f["denials"], int(f["error"]),
        f["stop_reason"] or None, f["persona_sha"] or None,
        size_of(audit_dir, "input-prompt.md", meta.get("prompt_extracted")),
        size_of(audit_dir, "output-response.md", meta.get("response_extracted")),
    )


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
    row = row_of(meta, audit_dir, datetime.now(timezone.utc))
    if row[0] is None:
        print("::warning::session-row: not written: metadata carries no run_id")
        return 0
    try:
        d1.query(INSERT, row)
    except d1.Unreadable as error:
        print(f"::warning::session-row: not written: {error}")
        return 0
    print(f"session-row: wrote run {row[0]} attempt {row[1]} for {row[3]}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
