"""When the reviewer's typed verdict was applied, read from the issue timeline.

A label says the PR is approved; the event says WHEN, and when is what places
the verdict on a head. The label survives a push, the reviewer strips it only
on FAIL, and a paused guard leaves the next review run green with the labels
untouched, so `agent-approved` on a PR vouches for whichever head's round it
postdates and no other. Two landing paths apply that test, the BDFL sweep's
rescue merge (stalled-prs) and `merge-pr --wait` (issue #842's review on PR
#966); this module is the one copy of the read they share. The caller names
the repo: merge-pr pins GITHUB_REPOSITORY like its every other read, stalled-prs
the `{owner}/{repo}` placeholder gh expands from the checkout, like its every
other read. When this read chose for them it chose the placeholder, and
`merge-pr --wait` from outside a checkout timed out naming no cause (#968).
"""

import json
import subprocess


def approved_at(number, repo):
    """When `agent-approved` was last applied to PR `number` of `repo`, or None.

    `--slurp` because `--paginate` otherwise emits one document per page.
    Fails soft: None when the read fails, never a death, because stalled-prs
    runs this inside the per-PR sweep and one 502 on one timeline must not
    take the report and the footer down with it (reviewer's must on PR #572).
    Both callers hold the merge on None; neither reads None as "never".
    """
    proc = subprocess.run(
        ["gh", "api", f"repos/{repo}/issues/{number}/timeline?per_page=100",
         "--paginate", "--slurp"],
        capture_output=True, text=True,
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
