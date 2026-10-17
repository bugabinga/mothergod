#!/usr/bin/env python3
"""Name the red scheduled checks a push to main should re-run (#1069).

Run by `recheck.yml` on every push to main, tested by `recheck-decide.test.mjs`.

A fix for a red scheduled check is proven by a run of that check, and
nothing ran it before its next cron, a week for the weekend ones. The hand
step filled the gap three times in two days (#1062, #1063, #1067), each a
BDFL dispatch on the PAT, the one token GitHub lets dispatch
(agents/OPERATIONS.md "Admin token & signing"). The maintainer that ships
the fix holds no such token, so no prompt line can own this.

`candidates()` reads the workflow files: every one with `on.schedule` and
`on.workflow_dispatch` that runs no model. The model clause pins a rule
rather than filtering anything today: no claude-code-action seat carries a
cron (ADR-0035), and a seat dispatched on the PAT is the operator-attributed
wake the BDFL prompt calls a self-trigger loop (#50).

`decide()` is pure: each candidate's runs on main in, a verdict per
candidate out. `dispatch` when the newest concluded run is red, no run of
it is queued or running, and no retry started within RETRY_HOURS; `held`
when one did; `green` otherwise. A retry is a `workflow_dispatch` run on
main, this job's or a hand one, and the red itself is never one, so a
fresh red gets its first retry at the first push after it. The bound is
the cost ceiling: a red that outlives its fix costs one run per
RETRY_HOURS, monster's matrix included, which is why RETRY_HOURS is 6 and
not 1. The retry is operator-attributed and agent-alarm admits that on
main, so a wrong fix's red is red-after-red there and the dedup swallows
it: the gain is a green within hours and a re-armed alarm, not a faster
alarm.

stdout is the dispatch list, one workflow file per line, for `xargs gh
workflow run`. stderr says why for every red candidate and ends with one
count line; every check green is that line alone. A run list that cannot
be read exits 1 with the workflow named and nothing on stdout, never as a
green: a false nothing here is a red that waits for its cron again.

Usage: recheck-decide.py [workflows-dir [epoch-seconds]]
"""
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import yaml

WORKFLOWS = Path(__file__).resolve().parent.parent / "workflows"

RETRY_HOURS = 6

# agent-alarm's reds, and its exclusions: a displaced run is debouncing
# (ADR-0014) and a skipped one a bot event the actor filter rejected,
# neither a conclusion about the check.
RED = {"failure", "timed_out", "startup_failure"}
ROUTINE = {"skipped", "cancelled"}

# The action every model seat runs; a workflow using it is never dispatched here.
MODEL = "anthropics/claude-code-action"

# `gh run list --json` fields read here.
FIELDS = "databaseId,status,conclusion,createdAt,event"
# Rows per workflow. A retry inside RETRY_HOURS sits within the newest few
# even on deploy-site, which runs on every push as well as its cron.
LIMIT = "20"


def runs_model(doc):
    """Whether any step of any job uses the model action."""
    for job in (doc.get("jobs") or {}).values():
        for step in (job or {}).get("steps") or []:
            if str((step or {}).get("uses", "")).startswith(MODEL):
                return True
    return False


def candidates(workflows=WORKFLOWS):
    """The workflow files recheck may dispatch, sorted. Returns (files, None) or (None, reason)."""
    files = []
    for path in sorted(workflows.glob("*.yml")):
        try:
            doc = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        except (OSError, yaml.YAMLError) as err:
            return None, f"{path.name}: {err}"
        # PyYAML reads a bare `on` key as the boolean True (YAML 1.1).
        triggers = doc.get("on", doc.get(True)) or {}
        if not isinstance(triggers, dict):
            continue
        if "schedule" in triggers and "workflow_dispatch" in triggers and not runs_model(doc):
            files.append(path.name)
    return files, None


def epoch(stamp):
    """Seconds since the epoch for a GitHub `createdAt`."""
    return datetime.strptime(stamp, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc).timestamp()


def decide(runs, now, retry_hours=RETRY_HOURS):
    """A verdict per candidate, `(file, verdict, why)`, in the candidates' order.

    `runs` maps a workflow file to its rows on main, any status, any order:
    newest is sorted here, not trusted (#902).
    """
    verdicts = []
    for file, rows in runs.items():
        ordered = sorted(rows, key=lambda row: epoch(row["createdAt"]), reverse=True)
        concluded = [
            row for row in ordered if row.get("status") == "completed" and row.get("conclusion") not in ROUTINE
        ]
        if not concluded:
            verdicts.append((file, "green", "no concluded run on main"))
            continue
        newest = concluded[0]
        if newest["conclusion"] not in RED:
            verdicts.append((file, "green", f"run {newest['databaseId']} concluded {newest['conclusion']}"))
            continue
        red = f"run {newest['databaseId']} concluded {newest['conclusion']} at {newest['createdAt']}"
        live = [row for row in ordered if row.get("status") != "completed"]
        if live:
            verdicts.append((file, "held", f"{red}; run {live[0]['databaseId']} is {live[0]['status']}"))
            continue
        since = now - retry_hours * 3600
        retries = [
            row for row in ordered if row.get("event") == "workflow_dispatch" and epoch(row["createdAt"]) >= since
        ]
        if retries:
            verdicts.append(
                (file, "held", f"{red}; retry {retries[0]['databaseId']} at {retries[0]['createdAt']}, within {retry_hours}h")
            )
            continue
        verdicts.append((file, "dispatch", f"{red}; no retry within {retry_hours}h"))
    return verdicts


def gh_runs(file):
    """The newest rows of one workflow on main. Returns (rows, None) or (None, reason)."""
    args = ["gh", "run", "list", "--workflow", file, "--branch", "main", "--limit", LIMIT, "--json", FIELDS]
    repo = os.environ.get("GITHUB_REPOSITORY")
    if repo:
        args[3:3] = ["--repo", repo]
    proc = subprocess.run(args, capture_output=True, text=True)
    if proc.returncode != 0:
        lines = proc.stderr.strip().splitlines()
        return None, lines[-1] if lines else f"gh exited {proc.returncode}"
    try:
        return json.loads(proc.stdout), None
    except json.JSONDecodeError:
        return None, "response was not JSON"


def main():
    workflows = Path(sys.argv[1]) if len(sys.argv) > 1 else WORKFLOWS
    # The clock is an argument so the end-to-end test can freeze it. With
    # time.time() read here and the fixtures stamped against a frozen NOW,
    # the test's held retry crossed RETRY_HOURS at 2026-10-16T21:00Z and
    # `worker` went red on every push to main with no diff behind it (#1275).
    now = float(sys.argv[2]) if len(sys.argv) > 2 else time.time()
    files, error = candidates(workflows)
    if error:
        print(f"recheck: UNREADABLE {error}", file=sys.stderr)
        return 1
    runs = {}
    for file in files:
        rows, error = gh_runs(file)
        if error:
            print(f"recheck: UNREADABLE {file}: {error}", file=sys.stderr)
            return 1
        runs[file] = rows
    verdicts = decide(runs, now)
    counts = {"dispatch": 0, "held": 0}
    for file, verdict, why in verdicts:
        if verdict == "green":
            continue
        counts[verdict] += 1
        print(f"recheck: {file}: {verdict}, {why}", file=sys.stderr)
        if verdict == "dispatch":
            print(file)
    red = counts["dispatch"] + counts["held"]
    print(
        f"recheck: candidates {len(files)} | red {red} | dispatched {counts['dispatch']} | held {counts['held']}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
