#!/usr/bin/env python3
"""Compose a Telegram notice for a finished agent run.

Operator directive (Telegram, 2026-08-27): heartbeat runs notify the
operator like BDFL runs do, but nothing in the session drafts the message.
The agent's final response already summarizes the run, so drafting a second
summary in-session would spend context to say the same thing twice. This
composes the notice mechanically instead, out of that response and the audit
metadata.

The result leads and the provenance trails. Every notice used to open with
`<label>: green, 12 turns, 8m`, which is the same sentence on every run and
answers a question the operator was not asking; they read past it to reach
the one line that differed (operator report, 2026-09-13). Uniform text at the
end of a message is a signature, uniform text at the front is a wall. So the
run's own words come first, and the label, turn count and duration go last in
one italic line. A failed run inverts this, because there the fact that it
failed IS the news.

Reads the files agent-audit extracted, AFTER its secret scrub, so this text
inherits that redaction and adds no new leak surface. Composing only; the
send is tg-send's job (`run-notice.py <audit-dir> <label> | tg-send
--notice`), and tg-send owns rendering the house markdown this passes
through untouched.

Never exits non-zero for missing or partial audit data: a notice that a run
finished without a readable record is still a notice, and observability does
not get to break the thing it observes. The caller's step guards the send
with continue-on-error regardless.
"""

import json
import os
import sys

from opener import trim

# Chars of response kept. 500 cut mid-sentence and lost the conclusion the
# operator actually wanted, which reads as the run hiding something; a clip
# this size ends most summaries at their real end, and the ones it does cut
# say so and link the full record.
CLIP = 1200


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
        print(f"**{label} finished.** No audit record survived it, so the "
              f"run log is the only account of what happened.\n{run_url}")
        return 0

    telemetry = meta.get("telemetry") or {}
    failed = bool(meta.get("is_error"))
    turns = telemetry.get("num_turns")
    minutes = round((telemetry.get("duration_ms") or 0) / 60000)
    response, clipped = clipped_response(audit_dir)

    lines = []
    if failed:
        lines.append(f"**{label} failed.**")
    if response:
        lines.append(response)
    elif not failed:
        lines.append(f"**{label} finished** and left no summary behind.")
    # The link is offered exactly when this message is not the whole story.
    if failed or clipped or not response:
        lines.append(run_url)
    facts = (
        [label]
        + ([f"{turns} turns"] if turns is not None else [])
        + ([f"{minutes}m"] if minutes else [])
    )
    lines.append("_" + ", ".join(facts) + "_")
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


def clipped_response(audit_dir: str) -> tuple[str, bool]:
    """The final response and whether it was cut short.

    agent-audit writes a `(...)` placeholder when the execution file carried
    no result entry; that is absence, not content. Markdown is left alone:
    tg-send renders the house dialect into Telegram's markup now, so
    stripping emphasis here would only throw away the structure that makes a
    bulleted summary scannable. A completion announcement in front of the
    result is not left alone: `opener.trim` drops it, because the first line
    is the whole message to a reader who stops there and "Heartbeat done."
    is not a message.

    The cut lands where the writer already paused, preferring a paragraph
    break to a line break to a word break, and never before halfway, because
    a boundary found too early costs more text than a ragged edge. The caller
    needs the second return value to decide whether to offer the run link:
    truncation the reader cannot see or recover from is how a notice comes
    across as withholding.
    """
    try:
        with open(os.path.join(audit_dir, "output-response.md"),
                  encoding="utf-8") as fh:
            text = fh.read().strip()
    except OSError:
        return "", False
    if not text or text.startswith("("):
        return "", False
    text, _ = trim(text)
    if len(text) <= CLIP:
        return text, False
    head = text[:CLIP]
    for boundary in ("\n\n", "\n", " "):
        cut = head.rfind(boundary)
        if cut > CLIP // 2:
            return head[:cut].rstrip() + "…", True
    return head.rstrip() + "…", True


if __name__ == "__main__":
    sys.exit(main())
