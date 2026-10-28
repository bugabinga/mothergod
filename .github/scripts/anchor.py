#!/usr/bin/env python3
"""The window every per-run duty shares: since the previous BDFL run.

Imported, not run. `operator-sweep` asks what the operator did since the
anchor; `retrospect` asks how the agent sessions since the anchor went. Both
mean the same instant, so the instant is defined once here. Two definitions
would drift on the day one of them learns something, and the drift would show
up as a duty quietly covering a different window than the duty beside it.

Sibling imports work because Python puts a script's own directory first on
`sys.path`, and both callers live in this directory.
"""

import json
import os
import subprocess
import sys
from datetime import datetime, timezone

import d1

# The ops-log issue (CLAUDE.md "Where things live"): where the survey's
# marker lives (survey-due) and where the state read takes its last three
# comments from (state). One number here, so the two readers cannot drift.
OPS_LOG = 3

# The workflow whose previous session anchors both duties. Hardcoded rather
# than a flag: these are the BDFL's duties, and a --since flag on the caller
# overrides the whole derivation anyway.
ANCHOR_WORKFLOW = "agent-bdfl.yml"

# Successful runs the anchor walk may cross before giving up. A guard-skipped
# run (pause, throttle, exhausted ladder) concludes `success` in seconds with
# no session behind it, six a day on the BDFL cron alone, so a week-long
# outage stacks some forty of them in front of the last real run (#517, #525).
RUN_WALK = 60

# GitHub's timestamp shape, shared by every endpoint we read, which is what
# makes plain string comparison chronological.
STAMP = "%Y-%m-%dT%H:%M:%SZ"

# Hours past which an anchor is not a cadence. The BDFL cron fires every four
# hours, so three days means a limit pause or an outage (the hollow-run walk
# above already counts those, #525) or a runs page served stale: run 37152004054 anchored a
# month back on one such page, read 166 sessions and printed a footer nobody
# could tell was wrong (#902). Both cases get one loud line and no retry,
# because re-reading a stale page returns the stale page; the line is what
# makes the next occurrence diagnosable.
STALE_HOURS = 72


def die(message):
    """Exit non-zero. Reserved for a tool failing, never for a quiet result.

    Names the calling command, so a failure in a shared helper still tells the
    reader which of the two duties they were running.
    """
    print(f"{os.path.basename(sys.argv[0])}: {message}", file=sys.stderr)
    sys.exit(1)


def gh(*args):
    """Run gh and return stdout. gh carries the token; nothing here sees one."""
    proc = subprocess.run(["gh", *args], capture_output=True, text=True)
    if proc.returncode != 0:
        die(f"`gh {' '.join(args)}` failed: {proc.stderr.strip()}")
    return proc.stdout


def iso(value):
    """Canonicalize to GitHub's own timestamp shape.

    Every API time shares this shape, so string comparison is chronological.
    That invariant is what lets a descending walk stop early.
    """
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        die(f"{value!r} is not a timestamp. Write it as 2026-08-23T16:44:04Z.")
    return parsed.astimezone(timezone.utc).strftime(STAMP)


def stamp_to_dt(value):
    """Canonical stamp back to an aware datetime, for arithmetic on it."""
    return datetime.strptime(value, STAMP).replace(tzinfo=timezone.utc)


def stale(stamp, now=None):
    """Hours since `stamp` when that exceeds STALE_HOURS, else None."""
    now = now or datetime.now(timezone.utc)
    age = (now - stamp_to_dt(stamp)).total_seconds() / 3600
    return age if age > STALE_HOURS else None


def session_rows(run_ids):
    """The listed run ids with a sessions row (ADR-0060); None when the table is unreadable.

    The row is the liveness marker every real session leaves, so it is what
    separates a run from a hollow one: a guard-skipped run concludes
    `success` with no session, and anchoring on it covered two hours where the
    outage was nine days (#525). A session that died before its audit step
    has no row either and is skipped the same way, on purpose: erring long
    costs a re-read, erring short after an outage loses operator input
    silently.

    One query for the whole page, so the ordinary wake spends one read where
    it spent one artifacts call and an outage spends one where it spent one
    per hollow run. The table, not the artifact, because the two mark the
    same runs from the same writer (agent-audit writes both as the run ends)
    and the table lives on a store an Actions degradation does not touch:
    run 39811812824 read `total_count 0` from `/artifacts` for sixty
    successes while `run list` answered every start, and died (#1530).
    None, never an empty set, when the table could not be read, so the
    caller probes artifacts instead of walking the page as hollow; the one
    line says which provider answered.
    """
    if not run_ids:
        return set()
    marks = ", ".join("?" for _ in run_ids)
    try:
        rows = d1.query(f"SELECT run_id FROM sessions WHERE run_id IN ({marks})", [int(i) for i in run_ids])
    except d1.Unreadable as error:
        print(
            f"{os.path.basename(sys.argv[0])}: session table UNREADABLE ({error}); "
            "anchoring on audit artifacts instead.",
            file=sys.stderr,
        )
        return None
    return {str(row.get("run_id")) for row in rows}


def held_session(repo, run_id):
    """True when the run uploaded an audit artifact (ADR-0023).

    The fallback for an unreadable session table, per run: the artifact is
    the same liveness mark as the row, from the same writer, read from
    Actions. Both providers down is the one shape that dies asking for
    `--since`.
    """
    count = gh("api", f"repos/{repo}/actions/runs/{run_id}/artifacts", "--jq", ".total_count")
    return int(count.strip() or 0) > 0


def previous_run_start(repo, this_run, now=None):
    """Start of the previous ANCHOR_WORKFLOW run that held a session, excluding this one.

    Returns (canonical stamp, run id). Start rather than finish, deliberately:
    a BDFL run can take longer than the cadence, and anchoring on its finish
    would leave everything that happened during it covered by nobody. The cost
    is that a duty may see an item its predecessor already saw. Overlap is a
    re-read; a gap is a lost signal.

    Walks newest first and asks the session table once for the whole page
    (artifacts per run when the table is unreadable), so the ordinary wake
    spends one extra call and an outage spends one, or one per hollow run
    crossed. Newest first is sorted here, not trusted from the API: the one
    page that came back out of order anchored a duty a month back (#902).
    `now` is for tests; the clock is the default.
    """
    runs = json.loads(
        gh(
            "run", "list", "--repo", repo, "--workflow", ANCHOR_WORKFLOW,
            "--status", "success", "--limit", str(RUN_WALK),
            "--json", "databaseId,startedAt,createdAt",
        )
    )
    runs.sort(key=lambda run: iso(run["startedAt"] or run["createdAt"]), reverse=True)
    held = session_rows([run["databaseId"] for run in runs])
    for run in runs:
        if this_run and str(run["databaseId"]) == str(this_run):
            continue
        if held is None:
            if not held_session(repo, run["databaseId"]):
                continue
        elif str(run["databaseId"]) not in held:
            continue
        stamp = iso(run["startedAt"] or run["createdAt"])
        if (age := stale(stamp, now)) is not None:
            print(
                f"{os.path.basename(sys.argv[0])}: anchor {stamp} (run {run['databaseId']}) "
                f"is {age / 24:.1f} days old against the worker clock's tick: a limit pause, an outage, "
                f"or a runs page served stale (#902). Every window measured from it is that "
                "wide.",
                file=sys.stderr,
            )
        return stamp, run["databaseId"]
    die(
        f"no {ANCHOR_WORKFLOW} run with a session row (or an audit artifact, the "
        f"table being unreadable) among the last {RUN_WALK} successes to anchor on. "
        "Pass --since <ISO8601> with the window you mean."
    )
    return None  # unreachable; die() exits. Keeps the return type honest.
