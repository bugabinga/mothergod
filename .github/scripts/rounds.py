#!/usr/bin/env python3
"""Count reviewer posts per merged PR: the number #663 judges review rounds by.

Imported by `retrospect`, not run. The window is the same anchor every
per-run duty shares (anchor.py), so the line lands in the footer beside the
sessions that produced the rounds.

Issue #663 opened on PR #660: five review rounds to surface four musts,
every must present in the original diff. The fix argued for was a prompt
line (shipped in #688) and a probe list; what neither side had was a
before-number in a measure both could reproduce. The curator's hand count
(40 PRs since `45ab5bd`: 30 took one post, 7 two, 3 three or more) is that
measure, and this prints it every wake so the next argument starts from a
number instead of the last PR remembered.

The counter is the reviewer's role footer, `_reviewer ·`, on the last
non-empty line of a comment: gh-comment writes it there and strips any a
seat typed (#590), so from `45ab5bd` on it is the one tell that a comment
is the reviewer's. Author login cannot serve here: every seat posts as
`github-actions[bot]` through the same script. A footer quoted mid-body
does not count, because only the last line is the script's.

What this does NOT cover, said plainly: PRs merged before `45ab5bd`
(2026-09-23), whose reviewer comments carry no footer and count as zero;
review rounds on a PR still open, which land in the window that merges it;
and the 101st PR merged in one window, because `gh pr list` is capped at
LIMIT and the line says so when the cap is hit.
"""

import json
import sys

from anchor import gh

REVIEWER_FOOTER = "_reviewer ·"
# `gh pr list --limit` ceiling; a window with this many merges is reported
# as capped rather than silently truncated.
LIMIT = 100
# Buckets on the line: 0, 1, 2 exact, 3 meaning three or more.
TAIL = 3


def footer(body):
    """The last non-empty line of a comment body, where gh-comment writes the role."""
    lines = [line.strip() for line in body.splitlines() if line.strip()]
    return lines[-1] if lines else ""


def reviewer_posts(pr):
    """How many of a PR's comments the reviewer posted."""
    return sum(
        1
        for comment in pr.get("comments") or []
        if footer(comment.get("body") or "").startswith(REVIEWER_FOOTER)
    )


def merged_since(repo, anchor):
    """Every PR merged since the anchor, with its comments, newest first."""
    return json.loads(
        gh(
            "pr", "list", "--repo", repo, "--state", "merged",
            "--search", f"merged:>={anchor}", "--limit", str(LIMIT),
            "--json", "number,mergedAt,comments",
        )
    )


def buckets(prs):
    """Posts-per-PR histogram {0, 1, 2, 3+} and the 3+ tail as (number, posts)."""
    counts = {n: 0 for n in range(TAIL + 1)}
    tail = []
    for pr in prs:
        posts = reviewer_posts(pr)
        counts[min(posts, TAIL)] += 1
        if posts >= TAIL:
            tail.append((pr["number"], posts))
    tail.sort(key=lambda item: (-item[1], item[0]))
    return counts, tail


def render(prs, out=sys.stdout, limit=LIMIT):
    """Print the one footer line; return the merged count."""
    if not prs:
        print("rounds: merged 0 since the anchor", file=out)
        return 0
    counts, tail = buckets(prs)
    parts = [
        f"rounds: merged {len(prs)}",
        "posts per PR " + " ".join(
            f"{n}{'+' if n == TAIL else ''}:{counts[n]}" for n in range(TAIL + 1)
        ),
    ]
    if tail:
        parts.append("tail " + " ".join(f"#{number} ({posts})" for number, posts in tail))
    if len(prs) >= limit:
        parts.append(f"capped at {limit}, the oldest merges are not counted")
    print(" | ".join(parts), file=out)
    return len(prs)


def report(repo, anchor):
    """Print the rounds line for the window; return the merged count."""
    return render(merged_since(repo, anchor))
