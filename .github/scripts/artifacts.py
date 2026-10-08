#!/usr/bin/env python3
"""Print what every seat posted since the anchor, each under the rule set it answers to.

Imported by `retrospect`, not run. The window is the same anchor every
per-run duty shares (anchor.py), so the artifacts land in the same read as
the sessions that wrote them.

Issue #485 opened on an operator report: agent text, issues most of all,
reads as slop, and the per-run retrospect kept saying "clean". Two causes.
The standard was prose ("on voice per CLAUDE.md") with nothing per artifact
to check against; CLAUDE.md "Voice" now carries the rules with ids. And the
lens never looked where the operator reads: retrospect printed each
session's final response, which the operator sees once as a phone notice,
and never the PR body, the issue, the review verdict or the digest, which
are the project's permanent surface. This prints those, so the judge reads
the artifact and names the rule.

Each artifact is one of five kinds, and the kind names the rule set:

    PR       a pull request opened in the window       T (title), P (body)
    issue    an issue opened in the window              I
    review   a reviewer comment on a PR                 R
    reply    any other comment on an issue or PR        Q
    digest   a comment on the ops-log issue             S

The seat is read from the role footer gh-comment and gh-pr assemble
(`_<role> · [Claude Code](...)_` on the last non-empty line, footer.py,
#590). An artifact without one is not a seat's prose: the operator's
issues, dependabot's PRs, a ledger script's table; none is judged here.
The footer comes off before printing because the header already names the
role.

Comments on `ledger` issues are skipped except the ops log: a ledger is a
script's table (traffic, allowance, model intel), and the ops log is where
every seat posts its digest, the status register S judges. The ledger set
is read from the same fetch that lists the threads, because a comment
bumps its thread's `updated_at` and so every commented thread is in it.

What this does NOT cover, said plainly: the final response, which
`describe` prints beside its session; a CHANGELOG line, which lives in the
diff and is the reviewer's to check against L; a comment EDITED in the
window but created before it, because the lens judges what was written,
not what was touched; and an artifact on a thread the `issues?since=`
page did not return, whose comments print as replies on an issue. Bodies
are cut at CAP for the same reason RESPONSE_CAP cuts a response: altitude
and voice are legible in the opening.
"""

import json
import re
import sys

from anchor import gh, iso
from attribution import issue_number
from footer import strip
from rounds import footer

# The ops-log issue (CLAUDE.md "Where things live"): a ledger by label, the
# status register by use.
OPS_LOG = 3

# The role on gh-comment's and gh-pr's footer line. Anchored at the start of
# the last non-empty line, so a footer quoted mid-body does not count.
ROLE = re.compile(r"^_([\w-]+) · \[Claude Code\]\(")

# One body longer than this is cut, as RESPONSE_CAP cuts a response.
CAP = 1800

# Kind -> the rule-set ids in CLAUDE.md "Voice" the judge cites.
RULES = {"PR": "T,P", "issue": "I", "review": "R", "reply": "Q", "digest": "S"}


def role_of(body):
    """The seat that posted a body, from its footer line, or None."""
    match = ROLE.match(footer(body or ""))
    return match.group(1) if match else None


def kind_of(number, is_pr, is_comment, role):
    """Which of the five artifact kinds a post is."""
    if not is_comment:
        return "PR" if is_pr else "issue"
    if number == OPS_LOG:
        return "digest"
    if is_pr and role == "reviewer":
        return "review"
    return "reply"


def threads_since(repo, anchor):
    """Every issue and PR updated since the anchor, oldest first.

    `--slurp` wraps each page in an outer array so pagination cannot produce
    concatenated invalid JSON; flattening it back is the price.
    """
    pages = json.loads(
        gh(
            "api", "--paginate", "--slurp",
            f"repos/{repo}/issues?state=all&since={anchor}&per_page=100",
        )
    )
    return [thread for page in pages for thread in page]


def classify(threads, comments, anchor):
    """The seats' artifacts in the window, oldest first, as printable dicts.

    `threads` is the `issues?since=` page, `comments` the `issues/comments
    ?since=` page: both key on update time, so creation is filtered here.
    """
    by_number = {t["number"]: t for t in threads}
    ledger = {
        n for n, t in by_number.items()
        if n != OPS_LOG and any(label.get("name") == "ledger" for label in t.get("labels") or [])
    }
    found = []
    for thread in threads:
        role = role_of(thread.get("body"))
        if iso(thread["created_at"]) < anchor or role is None:
            continue
        is_pr = "pull_request" in thread
        found.append({
            "at": iso(thread["created_at"]),
            "role": role,
            "kind": kind_of(thread["number"], is_pr, False, role),
            "number": thread["number"],
            "title": thread.get("title") or "",
            "url": thread.get("html_url") or "",
            "body": thread.get("body") or "",
        })
    for comment in comments:
        number = issue_number(comment)
        role = role_of(comment.get("body"))
        if iso(comment["created_at"]) < anchor or role is None or number in ledger:
            continue
        thread = by_number.get(number) or {}
        is_pr = "pull_request" in thread
        found.append({
            "at": iso(comment["created_at"]),
            "role": role,
            "kind": kind_of(number, is_pr, True, role),
            "number": number,
            "title": "",
            "url": comment.get("html_url") or "",
            "body": comment.get("body") or "",
        })
    found.sort(key=lambda a: a["at"])
    return found


def label(artifact):
    """`issue #926 [I]`, `review on #925 [R]`: the kind, the thread, the set."""
    kind = artifact["kind"]
    where = f"#{artifact['number']}"
    if kind in ("review", "reply"):
        where = f"on {where}"
    elif kind == "digest":
        where = f"on #{OPS_LOG}"
    return f"{kind} {where} [{RULES[kind]}]"


def render(artifacts, out=None):
    """Print the section, one artifact per block, bodies capped. Returns the count."""
    out = out or sys.stdout
    if not artifacts:
        print("\nposted since the anchor: no seat opened a PR or issue, or commented.", file=out)
        return 0
    print(
        "\nposted since the anchor, by artifact (rule ids in CLAUDE.md \"Voice\": "
        "T title, P PR body, I issue, R review, Q reply, S digest):",
        file=out,
    )
    for artifact in artifacts:
        title = f"  {artifact['title']}" if artifact["title"] else ""
        print(f"\n{artifact['at']}  {artifact['role']}  {label(artifact)}{title}", file=out)
        print(f"    {artifact['url']}", file=out)
        body, _ = strip(artifact["body"])
        body = body or "(empty body)"
        if len(body) > CAP:
            body = f"{body[:CAP]}\n[cut, {len(body) - CAP} more characters]"
        for line in body.splitlines():
            print(f"    {line}", file=out)
    return len(artifacts)


def report(repo, anchor, comments):
    """Print the section; return how many artifacts it showed, for the liveness line."""
    return render(classify(threads_since(repo, anchor), comments, anchor))
