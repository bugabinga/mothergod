#!/usr/bin/env python3
"""Detect comments posted around gh-comment, by author login.

Imported by `retrospect`, not run. The window is the same anchor every
per-run duty shares (anchor.py), and retrospect is where the BDFL already
reads everything the seats posted, so the flag lands beside the sessions
it indicts.

The predicate is issue #510's: `gh-comment` rides GH_WORKFLOW_TOKEN and
refuses to run without a ROLE, so a comment that went through it is
authored by `github-actions[bot]` and carries the role footer. A
`claude[bot]`-authored comment therefore bypassed the script and lost its
attribution. Author login is the whole predicate; no body parsing, because
prose is the soft substrate and the login is the token that posted.

PR threads count since issue #590: gh-comment takes a PR number, CLAUDE.md
routes every PR comment through it, and the reviewer's verdict goes the
same way (agent-review.yml). They were exempt before that, and each
claude[bot] comment cost one API read to learn whether its thread was a
PR; that resolution went with the exemption.

What this does NOT cover, said plainly: commits and PR bodies (gh-pr owns
the body's footer, and `git log` is where a missed one shows, because the
squash copies the body); review bodies (`gh pr review`), which no seat
posts; comments by the operator or by other bots; and a bypass whose
comment was DELETED before the next retrospect ran. An old comment EDITED
since the anchor re-surfaces here, because `since` filters on update time.
That is a re-read, not a false positive: the login tell holds regardless
of when the body changed.
"""

import json

from anchor import gh, iso

# The login gh-comment can never produce. Everything hangs on this one tell.
BYPASS_AUTHOR = "claude[bot]"


def issue_number(comment):
    """`.../issues/411` -> 411. The payload names its thread only by URL."""
    return int(comment["issue_url"].rstrip("/").rsplit("/", 1)[-1])


def flagged(comments):
    """Bypass comments: the claude[bot]-authored subset, on any thread."""
    return [
        c for c in comments
        if (c.get("user") or {}).get("login") == BYPASS_AUTHOR
    ]


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


def report(repo, anchor):
    """Print the bypass section if any, return (comments read, bypassed).

    Both numbers ride the caller's liveness line, so a check that silently
    stopped reading is distinguishable from a clean window.
    """
    comments = comments_since(repo, anchor)
    flags = flagged(comments)
    if flags:
        print("\nattribution bypass (comment posted around gh-comment, issues #510, #590):")
        for comment in flags:
            print(f"    {iso(comment['created_at'])}  #{issue_number(comment)}  {comment['html_url']}")
        print("    The footer cannot be retrofitted from here; find the seat "
              "in the sessions above and fix its path, not its prose.")
    return len(comments), len(flags)
