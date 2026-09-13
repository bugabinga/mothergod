#!/usr/bin/env python3
"""Compose a Telegram notice for a finished agent run.

Operator directive (Telegram, 2026-08-27): heartbeat runs notify the
operator like BDFL runs do, but nothing in the session drafts the message.
The agent's final response already summarizes the run, so drafting a second
summary in-session would spend context to say the same thing twice. This
composes the notice mechanically instead: a status line from the audit
metadata, then the agent's final response, in the markdown tg-send
renders into Telegram formatting.

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

# A guard against a runaway response, not an editorial limit. It used to be
# 500 chars, one phone screen, and the operator lost what those screens cut:
# run 34727798310's notice ended mid-sentence, dropping both the blocker that
# needed the BDFL's credentials and the next steps (operator report,
# 2026-09-13). Length is the agent's problem to solve by writing less, not
# this script's to solve by deleting the end. Well under tg-send's 4096 so
# the rendered markup still fits.
CLIP = 3000
# Marks the seam of a middle clip. SEAM is the half that carries no count,
# so main() can recognise a clipped response without re-deriving the length.
ELISION = "[… {} chars omitted, full run log linked below …]"
SEAM = ELISION.partition("{}")[2]


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

    # Health telemetry rides only on a red run. "green, 12 turns, 4m" led
    # every notice this bot has ever sent, which is how the operator came to
    # describe the whole format as repetitive preamble (2026-09-13): a number
    # that says "normal" every time teaches nothing and pushes the news down
    # the screen. On a red run it is the news, and the run page holds it
    # either way.
    red = bool(meta.get("is_error"))
    telemetry = meta.get("telemetry") or {}
    turns = telemetry.get("num_turns")
    minutes = round((telemetry.get("duration_ms") or 0) / 60000)
    stats = ", ".join(
        ([f"{turns} turns"] if turns is not None else [])
        + ([f"{minutes}m"] if minutes else [])
    )

    lines = [f"**{label}: RED**" + (f" ({stats})" if stats else "")
             if red else f"**{label}**"]
    response = clipped_response(audit_dir)
    if response:
        lines.append(response)
    # A clipped notice promises the full text is one link away, so the link
    # goes in even on a green run.
    if red or not response or SEAM in response:
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
    no result entry; that is absence, not content. The markdown is passed
    through untouched, because tg-send renders it into Telegram's own
    formatting; this function used to delete the markers instead, which is
    why every notice arrived with its emphasis flattened.

    An over-long response loses its middle, not its tail. What an agent
    leaves for the operator sits at both ends: what shipped at the top, what
    is blocked or next at the bottom. A head clip keeps the least actionable
    half and hides that it did so behind one ellipsis, which is how a
    maintainer run's "needs the BDFL's credentials" never reached the phone.
    Both cuts land on whitespace, so neither splits a word.
    """
    try:
        with open(os.path.join(audit_dir, "output-response.md"),
                  encoding="utf-8") as fh:
            text = fh.read().strip()
    except OSError:
        return ""
    if not text or text.startswith("("):
        return ""
    if len(text) <= CLIP:
        return text
    # Two thirds head, one third tail: the opening carries the narrative and
    # needs the room, the closing is dense and short.
    head = text[: CLIP * 2 // 3].rsplit(None, 1)[0]
    tail = text[-(CLIP // 3):].split(None, 1)[-1]
    dropped = len(text) - len(head) - len(tail)
    return f"{head}\n\n{ELISION.format(dropped)}\n\n{tail}"


if __name__ == "__main__":
    sys.exit(main())
