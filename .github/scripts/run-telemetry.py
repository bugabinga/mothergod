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

Usage: run-telemetry.py <out.md> [window_days]
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

# Cap on artifacts downloaded per run. Two windows of a busy week ran 129
# artifacts at ~10 KB each on 2026-08-23; the cap is headroom, not a budget,
# and a breach is REPORTED rather than silently truncating coverage.
MAX_DOWNLOADS = 600


def write(text):
    with open(out_path, "w", encoding="utf-8") as fh:
        fh.write(text)


def bail(reason):
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
    return {
        "role": meta.get("role") or "?",
        # A run whose execution file carried no result entry was never
        # measured: guard-skipped, paused, or died before finishing. Zero is
        # not its cost, so it is excluded from every median and counted
        # separately instead. 23 of 129 artifacts were this on 2026-08-23,
        # and folding them in as zeros moved the bdfl median output from
        # 9724 to 3314 tokens.
        "measured": bool(tele),
        "model": model,
        "out": out,
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
        "think": med([r["think"] for r in rows]),
        "turns": med([r["turns"] for r in rows]),
        "mins": med([r["mins"] for r in rows]),
        "denials": med([r["denials"] for r in rows]),
        "errors": sum(1 for r in rows if r["error"]),
        "models": collections.Counter(r["model"] for r in rows if r["model"]),
    }


recent, prior = collections.defaultdict(list), collections.defaultdict(list)
for meta in runs:
    row = facts(meta)
    (recent if meta["_at"] >= recent_from else prior)[row["role"]].append(row)

roles = sorted(set(recent) | set(prior))


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
    "| role | model actually run | runs | out tok | think% | turns | min |"
    " denials | errors |",
    "|---|---|---:|---:|---:|---:|---:|---:|---:|",
]

for role in roles:
    new, old = summarize(recent.get(role, [])), summarize(prior.get(role, []))
    if new is None:
        n = len(recent.get(role, []))
        note = f"{n} unmeasured" if n else "no runs"
        lines.append(f"| {role} | - | 0 | - | - | - | - | - | ({note}) |")
        continue
    models = ", ".join(f"`{m}`×{c}" for m, c in new["models"].most_common(3)) or "-"
    lines.append(
        f"| {role} | {models} | {new['n']} "
        f"| {cell(new['out'], old and old['out'])} "
        f"| {cell(new['think'], old and old['think'], '{:.0f}')} "
        f"| {cell(new['turns'], old and old['turns'])} "
        f"| {cell(new['mins'], old and old['mins'], '{:.1f}')} "
        f"| {cell(new['denials'], old and old['denials'], '{:.1f}')} "
        f"| {new['errors']}/{new['n']} |"
    )

summaries = [s for s in (summarize(v) for v in recent.values()) if s]
total_out = sum(s["out_total"] for s in summaries)
measured = sum(s["n"] for s in summaries)
unmeasured = sum(s["unmeasured"] for s in summaries)
lines += [
    "",
    f"Window total: {total_out:,} output tokens across {measured} measured "
    f"runs. A further {unmeasured} runs carried no result entry (paused, "
    "guard-skipped, or died before finishing) and are excluded from every "
    "median rather than counted as zero.",
    "",
    "Cost in USD is deliberately absent: under subscription auth the "
    "action's figure is notional, not a bill, and a public number that "
    "reads as spend but is not fails honesty. Output tokens, turns and "
    "minutes are what the subscription and the contributor's wait "
    "actually pay.",
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
