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

# The workflow whose previous success anchors both duties. Hardcoded rather
# than a flag: these are the BDFL's duties, and a --since flag on the caller
# overrides the whole derivation anyway.
ANCHOR_WORKFLOW = "agent-bdfl.yml"

# GitHub's timestamp shape, shared by every endpoint we read, which is what
# makes plain string comparison chronological.
STAMP = "%Y-%m-%dT%H:%M:%SZ"


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


def previous_run_start(repo, this_run):
    """Start of the previous successful ANCHOR_WORKFLOW run, excluding this one.

    Returns (canonical stamp, run id). Start rather than finish, deliberately:
    a BDFL run can take longer than the cadence, and anchoring on its finish
    would leave everything that happened during it covered by nobody. The cost
    is that a duty may see an item its predecessor already saw. Overlap is a
    re-read; a gap is a lost signal.
    """
    runs = json.loads(
        gh(
            "run", "list", "--repo", repo, "--workflow", ANCHOR_WORKFLOW,
            "--status", "success", "--limit", "10",
            "--json", "databaseId,startedAt,createdAt",
        )
    )
    for run in runs:
        if this_run and str(run["databaseId"]) == str(this_run):
            continue
        return iso(run["startedAt"] or run["createdAt"]), run["databaseId"]
    die(
        f"no successful {ANCHOR_WORKFLOW} run to anchor on. "
        "Pass --since <ISO8601> with the window you mean."
    )
    return None  # unreachable; die() exits. Keeps the return type honest.
