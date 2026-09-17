#!/usr/bin/env python3
"""Watch the Claude Code docs and report what moved since last week.

Deliberately NOT an agent: no Claude step, no tokens, no context window
(ADR-0019, ADR-0023). Diffing two fetches is arithmetic, a script does it
deterministically and for free, and third-party prose never enters an
agent's context, so there is no injection surface here at all.

Why this exists. `agents/SOURCES.md` has listed https://code.claude.com/docs
since 2026-08-20, and the deep survey's duty said "review SOURCES.md". That
reviews the *list*, not the *sources*: nothing ever fetched anything. On
2026-09-17 the operator read the env-vars page himself and sent it over,
which is the machinery doing the operator's reading (ADR-0007). A listed
source nobody fetches is a bookmark, not a watch.

What is watched, and why these:

    the index     https://code.claude.com/docs/llms.txt, the docs' own
                  machine-readable page list, one line per page. Diffing its
                  URLs names pages that appeared or vanished. 47 KB.
    the pages     the reference pages that govern this fleet's own substrate.
                  A page-level digest is the load-bearing half: the index
                  line for env-vars.md does not change when the page gains a
                  variable, so index-only watching would have missed the
                  exact thing the operator sent.

The watchlist is reference pages, and the changelog is deliberately NOT on
it. A file that changes every release carries no information when it
changes; a watcher that always fires is one nobody reads. Every CLI change
that can reach this fleet lands in one of the reference pages below, which
is why reference pages are the right watchlist and the metronome is not.

State lives in a ledger issue labeled `docs-intel`, not a committed file,
for the reason traffic-snapshot.yml records: a machinery commit to main
wakes the BDFL, and a ledger refresh must never cost a wake (issue #50,
#417). The issue's edit history is the audit trail.

Delta-only (ADR-0007 run economy): nothing moved, nothing posted. The
steady state costs one HTTP round of about 1 MB and zero tokens.

Review is not tracked here. `agents/SOURCES.md`'s adoption log already
carries the date of the last review, so the survey reads "changes newer
than my last adoption-log entry" and this file needs no unreviewed flag to
set and clear. One source of truth for "what has been considered".

A fetch that fails exits non-zero and takes the run red. A watcher that
reports "nothing changed" because it could not look is worse than no
watcher: it manufactures a reassuring signal out of a broken one.

    docs-watch.py <prior-body> <out-body>

`build(prior_body, index_text, pages, today, run_url)` is pure, which is
what lets the fixtures exercise it without a network.
"""

import datetime
import hashlib
import json
import os
import re
import sys
import urllib.error
import urllib.request

INDEX_URL = "https://code.claude.com/docs/llms.txt"

# Slug -> why this fleet cares. The "why" is printed in the ledger so a
# reader who has never seen this file knows what a hit on it means.
WATCHED = {
    "env-vars": "the session environment `.claude/settings.json` sets",
    "settings": "the schema of `.claude/settings.json` itself, env plus hooks",
    "cli-reference": "the flags `claude_args` passes: --max-turns, --model, --effort",
    "github-actions": "the action all seven agent workflows run on",
    "hooks": "the three deny-* hooks that gate every session's tools",
    "skills": "`.claude/skills/`, one directory per operating manual (ADR-0016)",
}

PAGE_URL = "https://code.claude.com/docs/en/{}.md"

# `- [Title](https://.../page.md): description` is llms.txt's only entry shape.
ENTRY = re.compile(r"^- \[([^\]]+)\]\((https://[^)]+)\)")

# How many dated rounds the ledger keeps. Older than this is git-less history
# nobody reads; the adoption log carries anything that mattered.
KEEP_ROUNDS = 12


def fail(reason):
    """Exit non-zero with a reason. See the docstring on silent watchers."""
    print(f"docs-watch: {reason}", file=sys.stderr)
    raise SystemExit(1)


def fetch(url):
    """Fetch one URL as text, or fail the run. No retries: a weekly job that
    misses one week loses one week, and a retry loop hides a dead endpoint."""
    request = urllib.request.Request(url, headers={"User-Agent": "mothergod-docs-watch"})
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return response.read().decode("utf-8")
    except (urllib.error.URLError, TimeoutError, UnicodeDecodeError) as error:
        fail(f"fetching {url}: {error}")


def digest(text):
    """Content hash, newline-normalized so a CRLF flip is not a change."""
    return hashlib.sha256("\n".join(text.splitlines()).encode("utf-8")).hexdigest()[:16]


def index_entries(text):
    """URL -> title for every page llms.txt lists."""
    entries = {}
    for line in text.splitlines():
        found = ENTRY.match(line.strip())
        if found:
            entries[found.group(2)] = found.group(1)
    return entries


def parse_prior(body):
    """Read the state block out of a ledger body. Anything unreadable is
    treated as no prior state, which reports every page as new exactly once
    rather than crashing the watch on a hand-edited issue."""
    found = re.search(r"```json\n([\s\S]*?)\n```", body or "")
    if not found:
        return {"pages": {}, "index": {}, "rounds": []}
    try:
        prior = json.loads(found.group(1))
    except json.JSONDecodeError:
        return {"pages": {}, "index": {}, "rounds": []}
    return {
        "pages": prior.get("pages") or {},
        "index": prior.get("index") or {},
        "rounds": prior.get("rounds") or [],
    }


def build(prior_body, index_text, pages, today, run_url):
    """Return (body, changed). Pure, so the fixtures can drive it offline.

    `pages` is slug -> page text. `changed` is False on the first run only
    when the docs genuinely match the prior state; a first run with no prior
    state reports every page as newly tracked, once."""
    prior = parse_prior(prior_body)
    entries = index_entries(index_text)
    if not entries:
        fail("llms.txt parsed to zero pages; the index format changed")

    lines = []

    if prior["index"]:
        added = sorted(set(entries) - set(prior["index"]))
        removed = sorted(set(prior["index"]) - set(entries))
        for url in added:
            lines.append(f"- **new page** [{entries[url]}]({url})")
        for url in removed:
            lines.append(f"- **page gone** {prior['index'][url]} ({url})")
    else:
        lines.append(f"- **watch started** tracking {len(entries)} pages in the index")

    digests = {slug: digest(text) for slug, text in pages.items()}
    for slug in sorted(pages):
        was = prior["pages"].get(slug)
        if was is None:
            lines.append(f"- **now watched** `{slug}`: {WATCHED[slug]}")
        elif was != digests[slug]:
            url = PAGE_URL.format(slug)
            lines.append(f"- **changed** [`{slug}`]({url}): {WATCHED[slug]}")

    changed = bool(lines)
    rounds = prior["rounds"]
    if changed:
        rounds = [{"date": today, "lines": lines}] + [
            r for r in rounds if r.get("date") != today
        ][: KEEP_ROUNDS - 1]

    state = {"pages": digests, "index": entries, "rounds": rounds}

    body = [
        "# Claude docs watch",
        "",
        "Machine-written by `.github/scripts/docs-watch.py`; edits here are",
        "overwritten. What the fleet runs on, watched weekly: the docs index",
        "plus the reference pages that govern this project's own substrate.",
        "",
        "The BDFL reads this on the Sunday survey's sources duty and logs what",
        "it adopted or rejected in `agents/SOURCES.md`. That log's newest entry",
        "date is the review watermark: everything below it is unconsidered.",
        "",
        "## Changes, newest first",
        "",
    ]
    if rounds:
        for round_ in rounds:
            body.append(f"### {round_['date']}")
            body.extend(round_["lines"])
            body.append("")
    else:
        body.append("Nothing has moved since the watch started.")
        body.append("")

    body.append("## Watched pages")
    body.append("")
    body.append("| Page | Why this fleet cares |")
    body.append("|---|---|")
    for slug in sorted(WATCHED):
        body.append(f"| [`{slug}`]({PAGE_URL.format(slug)}) | {WATCHED[slug]} |")
    body.append("")
    body.append(f"Index: [llms.txt]({INDEX_URL}), {len(entries)} pages.")
    body.append(f"Last checked {today} by [this run]({run_url}).")
    body.append("")
    body.append("<!-- state below; docs-watch.py reads it back next week -->")
    body.append("")
    body.append("```json")
    body.append(json.dumps(state, indent=1, sort_keys=True))
    body.append("```")
    body.append("")
    body.append("---")
    body.append("_Generated by [Claude Code](https://claude.ai/code)_")

    return "\n".join(body), changed


def main():
    if len(sys.argv) != 3:
        fail("usage: docs-watch.py <prior-body> <out-body>")
    prior_path, out_path = sys.argv[1], sys.argv[2]

    try:
        with open(prior_path, encoding="utf-8") as handle:
            prior_body = handle.read()
    except FileNotFoundError:
        prior_body = ""

    index_text = fetch(INDEX_URL)
    pages = {slug: fetch(PAGE_URL.format(slug)) for slug in WATCHED}

    today = os.environ.get("DOCS_WATCH_TODAY") or datetime.datetime.now(
        datetime.timezone.utc
    ).strftime("%Y-%m-%d")
    run_url = os.environ.get("RUN_URL", "")

    body, changed = build(prior_body, index_text, pages, today, run_url)

    with open(out_path, "w", encoding="utf-8") as handle:
        handle.write(body)

    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as handle:
            handle.write(f"changed={'true' if changed else 'false'}\n")

    print("docs-watch: something moved" if changed else "docs-watch: nothing moved")


if __name__ == "__main__":
    main()
