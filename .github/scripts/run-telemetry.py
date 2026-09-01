#!/usr/bin/env python3
"""Aggregate agent run economics from audit artifacts (ADR-0023).

The capability half of model intel says what the market thinks a model can
do. This half says what our roles actually cost on our actual workload,
which is the other input ADR-0031 (model) and ADR-0021 (effort) need and
the only one measured on the work we really run.

Source of truth is the audit artifact every agent run already uploads
(`.github/actions/agent-audit`). Nothing new is stored: two windows are
aggregated straight from artifact metadata, so the trend is visible without
a database, and the report's own history in the issue outlives the
artifacts' 90-day retention.

Reads only API-authored numbers and model identifiers by whitelist. No
model-authored prose reaches the output, so a summarizing agent's injection
surface does not exist here (ADR-0019).

Self-diagnosing: any failure writes a section saying what was actually seen
and exits 0. Observability does not get to break the thing it observes.

Two consumers, one walk and one whitelist. A `.md` output is the model-intel
report (ADR-0023). A `.json` output is the site's public telemetry feed
(issue #64, rendered by site/agents.html): the same per-role summaries plus
per-run rows for the recent-runs table. The format follows the output file's
extension because that is the one dispatch a call site cannot state wrongly.
Both carry a projected API cost: the SDK prices every run at API list
rates (modelUsage costUSD, costBasis "list") and that sum is published
per run and per seat. A modelUsage entry whose costBasis is anything
other than "list" or absent (the SDK allows "managed" or "unknown", an
explicit guess) is excluded from the sum and the run counts toward
`uncosted` instead, so the label stays true even if a future model
alias ever prices that way. The project runs on subscription auth, so
the figure is a projection of what the same work would have cost on the
API, not a bill, and every surface labels it so. Until 2026-09-01 USD
was deliberately absent as risking a dishonest read; the operator
overruled that: the projection is the signal a reader can compare
against their own compute, and labeling beats omission (Telegram,
2026-09-01). From the same sums, each seat's share of the window's
projected cost is published as its burn share (issue #439): the
measurable proxy for its share of the seven-day allowance, which
nothing attributes to seats directly.

Both outputs also carry the self-wake audit (issue #144): every admitted
thread-event bdfl wake in the recent window, re-derived from the API as
ours or the operator's, independently of the wake predicate that admitted
it. Its correct value is zero, and a zero is only claimed when every wake
was actually verified.

Usage: run-telemetry.py <out.md|out.json> [window_days]
"""

import collections
import io
import json
import os
import statistics
import subprocess
import sys
import zipfile
from datetime import datetime, timedelta, timezone

out_path = sys.argv[1]
WINDOW = int(sys.argv[2]) if len(sys.argv) > 2 else 7
REPO = os.environ.get("GITHUB_REPOSITORY", "")
JSON_MODE = out_path.endswith(".json")

# Cap on artifacts downloaded per run. Two windows of a busy week ran 129
# artifacts at ~10 KB each on 2026-08-23; the cap is headroom, not a budget,
# and a breach is REPORTED rather than silently truncating coverage.
MAX_DOWNLOADS = 600


def write(text):
    with open(out_path, "w", encoding="utf-8") as fh:
        fh.write(text)


def bail(reason):
    if JSON_MODE:
        write(json.dumps({
            "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "window_days": WINDOW,
            "error": reason,
            "roles": [],
            "runs": [],
        }))
    else:
        write(f"# Run economics\n\n{reason}\n")
    sys.exit(0)


def gh(*args, binary=False):
    proc = subprocess.run(["gh", *args], capture_output=True)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode("utf-8", "replace")[:400])
    return proc.stdout if binary else proc.stdout.decode("utf-8", "replace")


if not REPO:
    bail("GITHUB_REPOSITORY is unset; nothing to query.")

now = datetime.now(timezone.utc)
recent_from = now - timedelta(days=WINDOW)
prior_from = now - timedelta(days=2 * WINDOW)

try:
    pages = json.loads(gh("api", "--paginate", "--slurp",
                          f"repos/{REPO}/actions/artifacts?per_page=100"))
except (RuntimeError, ValueError) as exc:
    bail(f"Could not list artifacts ({exc}).")

wanted = []
for page in pages:
    for art in page.get("artifacts", []):
        if not art.get("name", "").startswith("audit-") or art.get("expired"):
            continue
        try:
            made = datetime.fromisoformat(art["created_at"].replace("Z", "+00:00"))
        except (KeyError, ValueError):
            continue
        if made >= prior_from:
            wanted.append((art["id"], made))

if not wanted:
    bail(f"No unexpired audit artifacts in the last {2 * WINDOW} days.")

truncated = 0
if len(wanted) > MAX_DOWNLOADS:
    wanted.sort(key=lambda p: p[1], reverse=True)
    truncated = len(wanted) - MAX_DOWNLOADS
    wanted = wanted[:MAX_DOWNLOADS]

runs, unreadable = [], 0
for art_id, made in wanted:
    try:
        blob = gh("api", f"repos/{REPO}/actions/artifacts/{art_id}/zip", binary=True)
        meta = json.loads(zipfile.ZipFile(io.BytesIO(blob)).read("metadata.json"))
    except (RuntimeError, ValueError, KeyError, zipfile.BadZipFile):
        unreadable += 1
        continue
    if not isinstance(meta, dict):
        unreadable += 1
        continue
    meta["_at"] = made
    runs.append(meta)

if not runs:
    bail(f"Found {len(wanted)} audit artifacts, none readable.")


def facts(meta):
    """Whitelist the numbers. Everything else in the artifact is ignored."""
    tele = meta.get("telemetry") or {}
    usage = tele.get("usage") or {}
    model_usage = tele.get("modelUsage") or {}
    details = usage.get("output_tokens_details") or {}

    out = sum(v.get("outputTokens", 0) for v in model_usage.values()
              if isinstance(v, dict)) or usage.get("output_tokens") or 0
    # The model that did the work, not every model the session touched: a
    # sub-agent or a title generation can add a second entry worth 17 tokens.
    model = ""
    if model_usage:
        model = max(model_usage.items(),
                    key=lambda kv: (kv[1] or {}).get("outputTokens", 0))[0]
    thinking = details.get("thinking_tokens")
    # Projected API cost at list rates, straight from the SDK's own
    # per-model figure. None, not 0, when the field is missing: an
    # unpriced run excluded from sums beats a total that quietly
    # undercounts.
    costs = [v["costUSD"] for v in model_usage.values()
             if isinstance(v, dict) and isinstance(v.get("costUSD"), (int, float))
             and v.get("costBasis") in (None, "list")]
    trigger = meta.get("trigger") if isinstance(meta.get("trigger"), dict) else {}
    return {
        # Artifact creation instant and run id: identifiers, not numbers,
        # but API-authored like everything else here. They exist for the
        # JSON consumer's recent-runs table and its link to the run page.
        "at": meta["_at"].strftime("%Y-%m-%dT%H:%M:%SZ"),
        "run_id": meta.get("run_id") or "",
        "role": meta.get("role") or "?",
        # Trigger identity, API-authored (github context via agent-audit):
        # what woke the run, who authored the waking event, which thread.
        # The self-wake audit (issue #144) reads these.
        "event": trigger.get("event") or "",
        "actor": trigger.get("actor") or "",
        "number": trigger.get("number") or "",
        # A run whose execution file carried no result entry was never
        # measured: guard-skipped, paused, or died before finishing. Zero is
        # not its cost, so it is excluded from every median and counted
        # separately instead. 23 of 129 artifacts were this on 2026-08-23,
        # and folding them in as zeros moved the bdfl median output from
        # 9724 to 3314 tokens.
        "measured": bool(tele),
        "model": model,
        "out": out,
        "cost": sum(costs) if costs else None,
        # Share of output spent thinking: the direct read on whether an
        # effort level is doing anything (ADR-0021).
        "think": (100.0 * thinking / out) if thinking is not None and out else None,
        "turns": tele.get("num_turns"),
        "mins": (tele.get("duration_ms") or 0) / 60000.0 or None,
        "denials": (tele.get("permission_denials") or {}).get("count", 0),
        "error": bool(meta.get("is_error")),
    }


def med(values):
    clean = [v for v in values if v is not None]
    return statistics.median(clean) if clean else None


def summarize(all_rows):
    rows = [r for r in all_rows if r["measured"]]
    if not rows:
        return None
    return {
        "n": len(rows),
        "unmeasured": len(all_rows) - len(rows),
        "out": med([r["out"] for r in rows]),
        "out_total": sum(r["out"] for r in rows),
        "cost": med([r["cost"] for r in rows]),
        "cost_total": sum(r["cost"] for r in rows if r["cost"] is not None),
        # Measured runs the SDK did not price; a non-zero means the
        # cost totals undercount and every consumer says so.
        "uncosted": sum(1 for r in rows if r["cost"] is None),
        "think": med([r["think"] for r in rows]),
        "turns": med([r["turns"] for r in rows]),
        "mins": med([r["mins"] for r in rows]),
        "denials": med([r["denials"] for r in rows]),
        "errors": sum(1 for r in rows if r["error"]),
        "models": collections.Counter(r["model"] for r in rows if r["model"]),
    }


recent, prior = collections.defaultdict(list), collections.defaultdict(list)
rows = []
for meta in runs:
    row = facts(meta)
    rows.append(row)
    (recent if meta["_at"] >= recent_from else prior)[row["role"]].append(row)

roles = sorted(set(recent) | set(prior))

# Self-wake audit (issue #144). The bdfl wake predicate admits a
# thread-event run when the actor is the operator and the triggering body
# carries no attribution link; the #142 incident ran 460 times because the
# predicate was wrong and nothing measured its decisions. This re-derives
# each admitted wake from the API, so a predicate defect shows up as a
# non-zero here instead of as a human reading run metadata by hand.
#
# Admitted means the audit artifact exists: the wake predicate ran the job.
# `measured` is deliberately NOT required, because the incident's runs died
# mid-run and a self-wake that wastes 22 runner-minutes before dying is
# still a self-wake.
#
# Classification, by event shape:
# - issues / pull_request: the wake IS the thread, so the thread's author
#   and body are the wake's. Fetched fresh, never trusted from the
#   predicate's own inputs. Ours when the author is a Bot or the body
#   carries the attribution link (a thread opened on the operator's PAT is
#   operator-authored; only its body gives it away).
# - comment and review events: the wake's author is the event actor, ours
#   when it is a bot account. A machinery comment posted unattributed on
#   the operator's own token is invisible here, but it is equally invisible
#   to the predicate, so this audit still covers everything the predicate
#   claims to decide.
THREAD_EVENTS = frozenset(("issues", "issue_comment", "pull_request",
                           "pull_request_review",
                           "pull_request_review_comment"))
ATTRIBUTION = "[Claude Code](https://claude.ai/code)"


def self_wake_audit(all_rows):
    """Count admitted bdfl thread-event wakes that were our own output."""
    checked, ours, unverifiable, flagged = 0, 0, 0, []
    threads = {}
    for r in all_rows:
        if r["event"] not in THREAD_EVENTS:
            continue
        checked += 1
        is_ours = r["actor"].endswith("[bot]")
        if not is_ours and r["event"] in ("issues", "pull_request"):
            number = r["number"]
            if number not in threads:
                try:
                    threads[number] = json.loads(
                        gh("api", f"repos/{REPO}/issues/{number}"))
                except (RuntimeError, ValueError):
                    threads[number] = None
            thread = threads[number]
            if thread is None:
                unverifiable += 1
                continue
            is_ours = ((thread.get("user") or {}).get("type") == "Bot"
                       or ATTRIBUTION in (thread.get("body") or ""))
        if is_ours:
            ours += 1
            flagged.append(r["run_id"])
    return {"checked": checked, "self_triggered": ours,
            "unverifiable": unverifiable, "flagged_runs": flagged}


self_wakes = self_wake_audit(recent.get("bdfl", []))


def self_wake_line():
    """One sentence, and a zero only when it was actually measured."""
    n, bad, dark = (self_wakes["checked"], self_wakes["self_triggered"],
                    self_wakes["unverifiable"])
    if bad:
        runs_txt = ", ".join(f"run {r}" for r in self_wakes["flagged_runs"])
        return (f"**{bad} of {n} admitted bdfl thread-event wakes in the "
                f"window were our own output** ({runs_txt}): the wake "
                "predicate admitted what it exists to skip (issue #144).")
    if dark:
        return (f"Self-wake audit: {dark} of {n} admitted bdfl thread-event "
                "wakes could not be re-derived from the API; zero is not "
                "claimed for them (issue #144).")
    return (f"Self-wake audit: 0 of {n} admitted bdfl thread-event wakes "
            "in the window were our own output (issue #144; measured, "
            "not assumed).")


# Per-seat burn share (issue #439): each seat's slice of the window's
# projected cost, computed once here so every consumer shows the same
# number. It proxies the seat's share of the seven-day allowance on the
# assumption that the allowance weighs usage roughly as list pricing
# does, by model-weighted tokens; the proxy's error is whatever that
# assumption misses. None, not 0, when the window priced nothing, and
# equally when the seat itself priced nothing (`cost` is None exactly
# then, the same check the $/run column makes): a seat of all-uncosted
# runs has an unknown share, and 0% would assert the opposite of the
# dash beside it.
recent_sums = {role: summarize(recent.get(role, [])) for role in roles}
costed_total = sum(s["cost_total"] for s in recent_sums.values() if s)
for s in recent_sums.values():
    if s:
        s["cost_share"] = (100.0 * s["cost_total"] / costed_total
                           if costed_total and s["cost"] is not None
                           else None)

if JSON_MODE:
    def portable(summary):
        """Counter to pairs; everything else in a summary is already JSON."""
        return {**summary, "models": summary["models"].most_common()} if summary else None

    rows.sort(key=lambda r: r["at"], reverse=True)
    write(json.dumps({
        "generated_at": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "window_days": WINDOW,
        "artifact_count": len(runs),
        "unreadable": unreadable,
        "truncated": truncated,
        "self_wakes": self_wakes,
        "roles": [{"role": role,
                   "recent": portable(recent_sums[role]),
                   "prior": portable(summarize(prior.get(role, [])))}
                  for role in roles],
        "runs": rows[:25],
    }, indent=1))
    print(f"telemetry json: {len(runs)} runs, {len(roles)} roles, "
          f"{unreadable} unreadable, {truncated} over cap")
    sys.exit(0)


def cell(new, old, fmt="{:.0f}"):
    """Value with its change against the prior window, or a bare value."""
    if new is None:
        return "-"
    text = fmt.format(new)
    if old is None or old == 0:
        return text
    change = 100.0 * (new - old) / old
    if abs(change) < 10:
        return text
    arrow = "▲" if change > 0 else "▼"
    return f"{text} {arrow}{abs(change):.0f}%"


lines = [
    "# Run economics",
    "",
    f"Median per run, last {WINDOW} days, against the {WINDOW} before it. "
    f"Arrows mark a move over 10%. Source: {len(runs)} audit artifacts, "
    "our own workload.",
    "",
    "| role | model actually run | runs | out tok | $/run | burn% | think% |"
    " turns | min | denials | errors |",
    "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
]

for role in roles:
    new, old = recent_sums[role], summarize(prior.get(role, []))
    if new is None:
        n = len(recent.get(role, []))
        note = f"{n} unmeasured" if n else "no runs"
        lines.append(f"| {role} | - | 0 | - | - | - | - | - | - | - | ({note}) |")
        continue
    models = ", ".join(f"`{m}`×{c}" for m, c in new["models"].most_common(3)) or "-"
    share = "-" if new["cost_share"] is None else f"{new['cost_share']:.0f}%"
    lines.append(
        f"| {role} | {models} | {new['n']} "
        f"| {cell(new['out'], old and old['out'])} "
        f"| {cell(new['cost'], old and old['cost'], '{:.2f}')} "
        f"| {share} "
        f"| {cell(new['think'], old and old['think'], '{:.0f}')} "
        f"| {cell(new['turns'], old and old['turns'])} "
        f"| {cell(new['mins'], old and old['mins'], '{:.1f}')} "
        f"| {cell(new['denials'], old and old['denials'], '{:.1f}')} "
        f"| {new['errors']}/{new['n']} |"
    )

summaries = [s for s in recent_sums.values() if s]
total_out = sum(s["out_total"] for s in summaries)
total_cost = sum(s["cost_total"] for s in summaries)
uncosted = sum(s["uncosted"] for s in summaries)
measured = sum(s["n"] for s in summaries)
unmeasured = sum(s["unmeasured"] for s in summaries)
cost_note = (
    f" {uncosted} measured runs carried no price and are missing from "
    "that total." if uncosted else ""
)
lines += [
    "",
    f"Window total: {total_out:,} output tokens across {measured} measured "
    f"runs. A further {unmeasured} runs carried no result entry (paused, "
    "guard-skipped, or died before finishing) and are excluded from every "
    "median rather than counted as zero.",
    "",
    self_wake_line(),
    "",
    f"Projected API cost for the window: ${total_cost:,.2f} at list "
    "rates, the SDK's own per-run figure. The agents run on "
    "subscription auth, so this is what the same work would have cost "
    "on the API, not a bill; the subscription and the contributor's "
    f"wait are what is actually paid.{cost_note} The burn% column is "
    "each seat's slice of that projected cost, the measurable proxy "
    "for its share of the seven-day allowance (issue #439); it assumes "
    "the allowance weighs usage roughly as list pricing does, by "
    "model-weighted tokens.",
]

notes = []
if truncated:
    notes.append(f"{truncated} in-window artifacts skipped at the "
                 f"{MAX_DOWNLOADS} download cap; raise it or shorten the window.")
if unreadable:
    notes.append(f"{unreadable} artifacts were unreadable and excluded.")
if notes:
    lines += ["", "Coverage: " + " ".join(notes)]

write("\n".join(lines) + "\n")
print(f"telemetry: {len(runs)} runs, {len(roles)} roles, "
      f"{unreadable} unreadable, {truncated} over cap")
