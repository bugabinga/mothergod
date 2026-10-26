"""When the reviewer's typed verdict was applied, read from the issue timeline.

A label says the PR is approved; the event says WHEN, and when is what places
the verdict on a head. The label survives a push, the reviewer strips it only
on FAIL, and a paused guard leaves the next review run green with the labels
untouched, so `agent-approved` on a PR vouches for whichever head's round it
postdates and no other. Two landing paths apply that test, the BDFL sweep's
rescue merge (stalled-prs) and `merge-pr --wait` (issue #842's review on PR
#966); this module is the one copy of the two reads they share: when the
label was applied, and when the round it must postdate began. The caller names
the repo: merge-pr pins GITHUB_REPOSITORY like its every other read, stalled-prs
the `{owner}/{repo}` placeholder gh expands from the checkout, like its every
other read. When this read chose for them it chose the placeholder, and
`merge-pr --wait` from outside a checkout timed out naming no cause (#968).
The read rides the token sidecar's newest mint where a seat runs one
(#1687), because a `--wait` outlasts the token the session started with.
"""

import json
import os
import subprocess

from app_token import prefer_file


def approved_at(number, repo):
    """When `agent-approved` was last applied to PR `number` of `repo`, or None.

    `--slurp` because `--paginate` otherwise emits one document per page.
    Fails soft: None when the read fails, never a death, because stalled-prs
    runs this inside the per-PR sweep and one 502 on one timeline must not
    take the report and the footer down with it (reviewer's must on PR #572).
    Both callers hold the merge on None; neither reads None as "never".
    """
    env = dict(os.environ)
    prefer_file(env)
    proc = subprocess.run(
        ["gh", "api", f"repos/{repo}/issues/{number}/timeline?per_page=100",
         "--paginate", "--slurp"],
        capture_output=True, text=True, env=env,
    )
    if proc.returncode != 0:
        return None
    try:
        stamps = [
            event["created_at"]
            for page in json.loads(proc.stdout)
            for event in page
            if event.get("event") == "labeled"
            and (event.get("label") or {}).get("name") == "agent-approved"
        ]
    except (AttributeError, KeyError, TypeError, ValueError):
        return None
    return max(stamps) if stamps else None


def round_started(starts):
    """When the review round on a head began: the earliest of its review
    runs' `starts`, or None when no run carries one.

    One head collects many review runs: a cancelled run and its replacement
    (PR #377), a PASS and the `edited` run that skips the model after it
    (#1246), a paused guard's empty run. Both hazards the `labeled < started`
    test exists for (above) key on a NEW head, and the first run on a head is
    the one that saw it arrive; every later run on the same head re-reads it
    and moves no head. Reading the newest run instead held #1243 at a PASS
    whose label event predated the `edited` re-read by sixteen seconds, and
    `merge-pr --wait` exited 4 on a head every gate had passed (#1301). A run
    with no start stamp is skipped, not fatal: it is a queued re-read beside
    a concluded round. Both callers hold on None, which this returns only
    when no run on the head has a stamp. Stamps are GitHub's ISO-8601 UTC
    strings, so the string minimum is the time minimum.
    """
    stamps = [start for start in starts if start]
    return min(stamps) if stamps else None
