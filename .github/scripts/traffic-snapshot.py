#!/usr/bin/env python3
"""Fold a fresh 14-day GitHub traffic snapshot into the traffic ledger.

GitHub's traffic API forgets daily views and clones after 14 days; the
ledger issue labeled traffic-data is the memory (issue #417). This
script reads the prior ledger body, merges the fresh /traffic/views and
/traffic/clones payloads into its fenced JSON block, renders the fresh
/traffic/popular/referrers and /traffic/popular/paths readings as two
tables, and writes the whole replacement body.

Referrers and paths are not merged: the API reports them as one 14-day
total with no per-day rows, so each snapshot's table is that window's
reading and the issue's edit history is their series (issue #1855). A
referrer string is the one field a stranger controls (any Referer header
reaches it), so it is rendered inside backticks with pipes, backticks
and newlines removed, and markup in it stays text.

Merge rule: newest wins per UTC day and per metric. The API reports the
current day partial, so a later snapshot's larger value for the same day
is the correction, and a day absent from the fresh window keeps its
stored value. The API pads its window with explicit zero days; they are
stored as the zeros they are.

Failure is loud by design, unlike model-intel.py's bail-to-report: the
inputs are our own repo's API and our own machinery's prior body, so an
unreadable shape means something broke, and a quiet fresh start would
silently discard the accumulated series. Exit 1 and let the red run
raise the alarm. The only silent path is an EMPTY prior body, which is
the legitimate first run.

Usage: traffic-snapshot.py <views.json> <clones.json> <referrers.json>
       <paths.json> <prior-body> <out-body>
RUN_URL names the writing run in the body.
"""

import json
import os
import re
import sys

# gh's issue-body cap is 65536 bytes; trim the oldest days if the series
# ever reaches this, recording the horizon as trimmed_before. At ~60
# bytes a day that is years away, and self-healing beats a hard failure
# nobody hits until then.
BODY_BUDGET = 60000

FENCE = re.compile(r"```json\n(.*?)\n```", re.DOTALL)


def fail(reason):
    print(f"traffic-snapshot: {reason}", file=sys.stderr)
    sys.exit(1)


def parse_prior(body):
    """Return (days, trimmed_before) from a prior ledger body, or fresh
    state for an empty one. A non-empty body without a readable JSON
    block is a defect, not a fresh start."""
    if not body.strip():
        return {}, None
    m = FENCE.search(body)
    if not m:
        fail("prior ledger body has no ```json block; refusing to drop the series")
    try:
        state = json.loads(m.group(1))
        days = state["days"]
    except (ValueError, KeyError, TypeError) as exc:
        fail(f"prior ledger JSON unreadable ({exc}); refusing to drop the series")
    return days, state.get("trimmed_before")


def merge(days, views, clones):
    """Fold both payloads into days, newest-wins per day per metric."""
    for payload, key, count_field, uniq_field in (
        (views, "views", "views", "view_uniques"),
        (clones, "clones", "clones", "clone_uniques"),
    ):
        for bucket in payload[key]:
            day = bucket["timestamp"][:10]
            row = days.setdefault(day, {})
            row[count_field] = bucket["count"]
            row[uniq_field] = bucket["uniques"]
    return days


# Longer than any real referrer host or repo path; a cap keeps a hostile
# header from padding the body toward BODY_BUDGET.
CELL_CAP = 80


def cell(text):
    """One table cell of stranger-controlled text: a literal code span
    that cannot break the table or render as markup."""
    return "`" + re.sub(r"[`|\r\n]", "", str(text))[:CELL_CAP] + "`"


def repo_path(row):
    """'/owner/name/pull/142' reads as '/pull/142', the root as '/', so
    the table survives a repository transfer unchanged."""
    tail = row["path"].split("/", 3)
    return "/" + (tail[3] if len(tail) > 3 else "")


def top_table(rows, head, label):
    """Render one popular-content payload, in the API's order (count
    descending, at most ten rows)."""
    lines = [f"| {head} (last 14 days) | count | uniques |", "|---|---|---|"]
    if not rows:
        lines.append("| none | 0 | 0 |")
    for row in rows:
        lines.append(f"| {cell(label(row))} | {row['count']} | {row['uniques']} |")
    return "\n".join(lines)


def render(days, trimmed_before, views, clones, referrers, paths, run_url):
    dates = sorted(days)
    total_views = sum(r.get("views", 0) for r in days.values())
    total_clones = sum(r.get("clones", 0) for r in days.values())
    state = {"days": {d: days[d] for d in dates}}
    if trimmed_before:
        state["trimmed_before"] = trimmed_before
    day_lines = ",\n".join(
        f'    "{d}": {json.dumps(days[d], sort_keys=True)}' for d in dates
    )
    fenced = '{\n  "days": {\n' + day_lines + "\n  }"
    if trimmed_before:
        fenced += f',\n  "trimmed_before": "{trimmed_before}"'
    fenced += "\n}"
    span = f"{dates[0]} to {dates[-1]}" if dates else "empty"
    trim_note = (
        f"\nDays before {trimmed_before} were trimmed to fit the issue-body cap.\n"
        if trimmed_before
        else ""
    )
    return f"""GitHub repo traffic, accumulated. The API forgets daily views and
clones after 14 days; the weekly traffic-snapshot workflow folds each
fresh window into the JSON block below, newest data winning per day.
The machinery writes this issue; nobody edits it by hand (issue #417).
Uniques are per day and per 14-day window; they do not sum across days.
Clone rows count every git clone including this repo's own Actions
checkouts, so read them as machinery-dominated; views are the human
signal. The referrer and path tables are the fresh window's reading
alone, since the API keeps no per-day rows for them; this issue's edit
history is their series (issue #1855).

| last 14 days (API window) | count | uniques |
|---|---|---|
| views | {views["count"]} | {views["uniques"]} |
| clones | {clones["count"]} | {clones["uniques"]} |

{top_table(referrers, "referrer", lambda r: r["referrer"])}

{top_table(paths, "path", repo_path)}

Accumulated: {len(dates)} days, {span}, {total_views} views, {total_clones} clones.
{trim_note}
```json
{fenced}
```

Last written by {run_url}

---
_Generated by [Claude Code](https://claude.ai/code)_
"""


def build(prior_body, views, clones, referrers, paths, run_url):
    """The whole pipeline, pure, for the test driver."""
    for payload, key in ((views, "views"), (clones, "clones")):
        if not isinstance(payload.get(key), list) or "count" not in payload:
            fail(f"/traffic/{key} payload shape unexpected; keys: {sorted(payload)}")
    for rows, key, field in ((referrers, "referrers", "referrer"), (paths, "paths", "path")):
        if not isinstance(rows, list) or not all(
            isinstance(r, dict) and {field, "count", "uniques"} <= set(r) for r in rows
        ):
            fail(f"/traffic/popular/{key} payload shape unexpected")
    days, trimmed_before = parse_prior(prior_body)
    days = merge(days, views, clones)
    body = render(days, trimmed_before, views, clones, referrers, paths, run_url)
    while len(body.encode()) > BODY_BUDGET and len(days) > 1:
        oldest = min(days)
        del days[oldest]
        trimmed_before = min(days)
        body = render(days, trimmed_before, views, clones, referrers, paths, run_url)
    return body


def main():
    views_path, clones_path, referrers_path, paths_path, prior_path, out_path = sys.argv[1:7]
    try:
        views = json.load(open(views_path, encoding="utf-8"))
        clones = json.load(open(clones_path, encoding="utf-8"))
        referrers = json.load(open(referrers_path, encoding="utf-8"))
        paths = json.load(open(paths_path, encoding="utf-8"))
    except (OSError, ValueError) as exc:
        fail(f"payload unreadable ({exc})")
    try:
        prior_body = open(prior_path, encoding="utf-8").read()
    except OSError:
        prior_body = ""
    body = build(
        prior_body, views, clones, referrers, paths,
        os.environ.get("RUN_URL", "an unrecorded run"),
    )
    with open(out_path, "w", encoding="utf-8") as fh:
        fh.write(body)


if __name__ == "__main__":
    main()
