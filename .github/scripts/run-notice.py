#!/usr/bin/env python3
"""Compose a one-screen Telegram notice for a finished agent run.

Operator directive (Telegram, 2026-08-27): heartbeat runs notify the
operator like BDFL runs do, but nothing in the session drafts the message.
The agent's final response already summarizes the run, so drafting a second
summary in-session would spend context to say the same thing twice. This
composes the notice mechanically instead: a status line from the audit
metadata, then the final response clipped to one phone screen.

Reads the files agent-audit extracted, AFTER its secret scrub, so this text
inherits that redaction and adds no new leak surface. Composing only; the
send is tg-send's job (`run-notice.py <audit-dir> <label> | tg-send
--notice`).

Never exits non-zero for missing or partial audit data: a notice that a run
finished without a readable record is still a notice, and observability does
not get to break the thing it observes. The caller's step guards the send
with continue-on-error regardless.
"""

import json
import os
import sys

CLIP = 500  # chars of response; the notice must fit one phone screen


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: run-notice.py <audit-dir> <label>", file=sys.stderr)
        return 2
    audit_dir, label = sys.argv[1], sys.argv[2]

    run_url = "{}/{}/actions/runs/{}".format(
        os.environ.get("GITHUB_SERVER_URL", "https://github.com"),
        os.environ.get("GITHUB_REPOSITORY", ""),
        os.environ.get("GITHUB_RUN_ID", ""),
    )

    meta = read_metadata(audit_dir)
    if meta is None:
        # A run that died before the audit wrote anything still gets
        # reported; the link is all the evidence there is.
        print(f"{label}: finished, no audit record\n{run_url}")
        return 0

    telemetry = meta.get("telemetry") or {}
    status = "RED" if meta.get("is_error") else "green"
    turns = telemetry.get("num_turns")
    minutes = round((telemetry.get("duration_ms") or 0) / 60000)
    stats = ", ".join(
        [status]
        + ([f"{turns} turns"] if turns is not None else [])
        + ([f"{minutes}m"] if minutes else [])
    )

    lines = [f"{label}: {stats}"]
    response = clipped_response(audit_dir)
    if response:
        lines.append(response)
    if status == "RED" or not response:
        lines.append(run_url)
    print("\n".join(lines))
    return 0


def read_metadata(audit_dir: str):
    """The audit metadata as a dict, or None for anything else.

    A non-dict that parses (null, a list) is as much "no record" as a
    missing file; letting it through would crash on the first .get() and
    break the docstring's no-nonzero-exit promise (PR #274 review).
    """
    try:
        with open(os.path.join(audit_dir, "metadata.json"),
                  encoding="utf-8") as fh:
            meta = json.load(fh)
    except (OSError, ValueError):
        return None
    return meta if isinstance(meta, dict) else None


def clipped_response(audit_dir: str) -> str:
    """The final response, readable on a phone, or empty.

    agent-audit writes a `(...)` placeholder when the execution file carried
    no result entry; that is absence, not content. Markdown emphasis and
    code ticks are stripped because Telegram renders them literally; line
    structure is kept because heartbeat summaries are bulleted. The clip
    cuts at a whitespace boundary so the ellipsis never splits a word.
    """
    try:
        with open(os.path.join(audit_dir, "output-response.md"),
                  encoding="utf-8") as fh:
            text = fh.read().strip()
    except OSError:
        return ""
    if not text or text.startswith("("):
        return ""
    text = text.replace("**", "").replace("`", "")
    if len(text) > CLIP:
        text = text[:CLIP].rsplit(None, 1)[0] + "…"
    return text


if __name__ == "__main__":
    sys.exit(main())
