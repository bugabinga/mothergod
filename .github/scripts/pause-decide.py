#!/usr/bin/env python3
"""Route a failed agent run: a model's ledger entry, the global pause, or nothing.

Run by `.github/actions/agent-pause`, tested by `pause-decide.test.mjs`. It
lived as a heredoc inside that action's YAML through three edits in six weeks
(the ledger, ADR-0018; the 404 path, 2026-09-24; the rung key, ADR-0059),
each testable only by regex-extracting the heredoc in a session shell (issue
#894). `guard-decide.py` left its heredoc at #375 for the same reason: a
decider that can cancel a run has to be provable. This one can idle the fleet.

`decide()` is pure: the execution log's entries, the rung and the clock in, a
verdict out. No file, no network, no `time.time()` outside `main()`.

The verdict is four fields the action reads with `IFS='|'`:

    none   |      |        |         no limit and no auth failure
    auth   |      | status |         401 or 403: the token, not a limit
    ledger | key  | resets | why     one rung blocked until `resets` (epoch)
    pause  | kind | marker | resume  a limit with no model to pin it on

Only a `result` entry flagged `is_error` is read for a verdict. Transcript
prose never pauses (#11): an assistant turn that discusses usage limits is
not a usage limit.
"""
import json
import os
import re
import sys
import time
from datetime import datetime, timedelta, timezone

# The text-marker fallback, for a limit the API reported in words alone.
MARKER = re.compile(
    r"usage[ _]limit|weekly limit|5-hour limit|session limit"
    r"|out of extra usage|reached your .{0,40}limit"
)
RESETS_AT = re.compile(r"resets (\d{1,2}):(\d{2})\s*(am|pm)\s*\(utc\)")

# A rung the subscription does not serve (adopted ahead of its launch, or
# since retired) is a 404, and without a ledger entry every wake on that
# ladder fails until someone edits models.json. Seven days blocks it past the
# weekly model review, and the ladder falls through to the next rung on the
# next wake.
NOT_SERVED = 7 * 86400
# A 429 with no advertised reset: short, because a wrong-long guess idles an
# agent for days and a wrong-short one costs one 429.
RATE_LIMITED = 3600


def entries_of(raw):
    """The execution file's entries: one JSON array, or one object per line."""
    try:
        data = json.loads(raw)
        return data if isinstance(data, list) else [data]
    except json.JSONDecodeError:
        pass
    entries = []
    for line in raw.splitlines():
        line = line.strip()
        if line:
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return entries


def _resume_at(text, now):
    """The ISO timestamp a `resets H:MM am/pm (UTC)` phrase names, or ''."""
    r = RESETS_AT.search(text)
    if not r:
        return ""
    hour = int(r.group(1)) % 12 + (12 if r.group(3) == "pm" else 0)
    now_dt = datetime.fromtimestamp(now, timezone.utc)
    t = now_dt.replace(hour=hour, minute=int(r.group(2)), second=0, microsecond=0)
    if t <= now_dt:
        t += timedelta(days=1)
    return t.strftime("%Y-%m-%dT%H:%M:%SZ")


def decide(entries, rung, now):
    """The verdict for one run's execution log, as a tuple of four strings."""

    def typed(t):
        return [e for e in entries if isinstance(e, dict) and e.get("type") == t]

    failures = [e for e in typed("result") if e.get("is_error")]
    if not failures:
        return ("none", "", "", "")

    # The ledger key is the rung the guard chose, as agents/models.json
    # spells it (a family alias, ADR-0059), because the guard compares rungs
    # against keys by exact string and knows no families. The runtime's
    # resolved id (system/init.model) is what the alias meant on this run and
    # rides along in the log line only. A seat with no ladder passes no rung;
    # the resolved id is its key, which the guard never consults for that seat.
    model = ""
    for e in typed("system"):
        if e.get("subtype") == "init" and e.get("model"):
            model = e["model"]
    key = rung or model
    ran_as = f", ran as {model}" if rung and model and model != rung else ""

    status = None
    for e in failures:
        if isinstance(e.get("api_error_status"), int):
            status = e["api_error_status"]

    rejected = [
        e["rate_limit_info"]
        for e in typed("rate_limit_event")
        if isinstance(e.get("rate_limit_info"), dict)
        and e["rate_limit_info"].get("status") == "rejected"
    ]

    if status in (401, 403):
        return ("auth", "", str(status), "")

    if status == 404 and key:
        return ("ledger", key, str(int(now) + NOT_SERVED), f"not served{ran_as}")

    if (rejected or status == 429) and key:
        # Latest advertised reset among the rejections, so a run never
        # retries before the window that actually blocked it.
        resets = [r.get("resetsAt") for r in rejected if isinstance(r.get("resetsAt"), int)]
        until = max(resets) if resets else int(now) + RATE_LIMITED
        return ("ledger", key, str(until), f"rate-limited{ran_as}")

    # Fallback only: a limit with no model to attribute it to.
    text = " ".join(json.dumps(e).lower() for e in failures)
    m = MARKER.search(text)
    if not m:
        return ("none", "", "", "")
    kind = "weekly" if "weekly limit" in text else "rolling"
    # The marker is quoted into the pause issue and read back through
    # `IFS='|'`; a pipe inside it would shift the resume field.
    return ("pause", kind, m.group(0).replace("|", " "), _resume_at(text, now))


def main():
    raw = open(sys.argv[1], encoding="utf-8", errors="replace").read()
    print("|".join(decide(entries_of(raw), os.environ.get("RUNG", ""), time.time())))


if __name__ == "__main__":
    main()
