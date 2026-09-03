#!/usr/bin/env python3
"""Detect issue comments posted around gh-comment, by author login.

Imported by `retrospect`, not run. The window is the same anchor every
per-run duty shares (anchor.py), and retrospect is where the BDFL already
reads everything the seats posted, so the flag lands beside the sessions
it indicts.

The predicate is issue #510's: `gh-comment` rides GH_WORKFLOW_TOKEN and
refuses to run without a ROLE, so a comment that went through it is
authored by `github-actions[bot]` and carries the role footer. A
`claude[bot]`-authored comment on an ISSUE therefore bypassed the script
and lost its attribution. Author login is the whole predicate; no body
parsing, because prose is the soft substrate and the login is the token
that posted.

PR threads are exempt by design: gh-comment is issues-only (its own
docstring), and reviewer verdicts and merge rationales legitimately ride
`claude[bot]` there. The comments endpoint returns both kinds, so the
claude[bot] subset is resolved thread by thread before flagging; the
subset is normally empty, which keeps the resolution calls at zero.

What this does NOT cover, said plainly: commits, PR bodies, PR comments,
and any surface beyond issue comments (#485's territory, not this
module's); comments by the operator or by other bots; and a bypass whose
comment was DELETED before the next retrospect ran. An old comment
EDITED since the anchor re-surfaces here, because `since` filters on
update time. That is a re-read, not a false positive: the login tell
holds regardless of when the body changed.
"""

import json

from anchor import gh, iso

# The login gh-comment can never produce. Everything hangs on this one tell.
BYPASS_AUTHOR = "claude[bot]"


def issue_number(comment):
    """`.../issues/411` -> 411. The payload names its thread only by URL."""
    return int(comment["issue_url"].rstrip("/").rsplit("/", 1)[-1])


def candidates(comments):
    """The claude[bot]-authored subset, filtered first so PR resolution stays cheap."""
    return [
        c for c in comments
        if (c.get("user") or {}).get("login") == BYPASS_AUTHOR
    ]


def flagged(comments, pr_numbers):
    """Bypass comments: claude[bot]-authored, on a thread that is not a PR."""
    return [c for c in candidates(comments) if issue_number(c) not in pr_numbers]


def comments_since(repo, anchor):
    """Every issue and PR comment updated since the anchor, oldest first.

    `--slurp` wraps each page in an outer array so pagination cannot
    produce concatenated invalid JSON; flattening it back is the price.
    """
    pages = json.loads(
        gh(
            "api", "--paginate", "--slurp",
            f"repos/{repo}/issues/comments?since={anchor}&per_page=100",
        )
    )
    return [comment for page in pages for comment in page]


def pr_threads(repo, numbers):
    """Which of these thread numbers are PRs. One API read per number.

    Called only on the claude[bot] subset's threads, so the usual cost is
    zero calls and the worst observed is a handful.
    """
    prs = set()
    for number in sorted(numbers):
        payload = json.loads(gh("api", f"repos/{repo}/issues/{number}"))
        if "pull_request" in payload:
            prs.add(number)
    return prs


def report(repo, anchor):
    """Print the bypass section if any, return (comments read, bypassed).

    Both numbers ride the caller's liveness line, so a check that silently
    stopped reading is distinguishable from a clean window.
    """
    comments = comments_since(repo, anchor)
    suspects = candidates(comments)
    prs = pr_threads(repo, {issue_number(c) for c in suspects})
    flags = flagged(suspects, prs)
    if flags:
        print("\nattribution bypass (issue comment posted around gh-comment, issue #510):")
        for comment in flags:
            print(f"    {iso(comment['created_at'])}  #{issue_number(comment)}  {comment['html_url']}")
        print("    The footer cannot be retrofitted from here; find the seat "
              "in the sessions above and fix its path, not its prose.")
    return len(comments), len(flags)
