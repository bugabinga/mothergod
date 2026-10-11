#!/usr/bin/env python3
"""Per-role aggregates over the session table, and the week that moved (#896).

`summarize_roles` is the per-role read `sessions roles` prints; it lives
here so the verb and the trend aggregate one way, once. `report` prints
retrospect's trend block: this week against the prior week, per role,
only the roles that crossed a threshold below. The director reads
sessions one wake at a time, and nothing else put a fortnight in front
of it: seven consecutive wakes judged "all sane" session by session
while the same week's queue did not move (#896). A quiet fortnight
prints one line, never nothing, so a block silent forever is
distinguishable from a broken one; an unreadable table prints
UNREADABLE and the retrospect goes on, never an empty trend read as a
quiet one (the `sessions` contract).

Thresholds, each a stated rule and pinned by trend.test.mjs:

    runs       the larger week is 1.5x the smaller, and at least FLOOR_RUNS
    turns_p50  moved by a quarter of the larger, and by FLOOR_TURNS turns
    denials    the larger week is 2x the smaller, and at least FLOOR_DENIALS
    errors     any change
    skipped    the guard held back SKIP_SHARE of the role's wakes this week

Medians, not means, because one 65-turn chat wake would otherwise paint
every BDFL run as long (ADR-0023's lesson). Measured runs only feed the
turn and token figures; unmeasured ones are counted, never zeroed in.
The guard table (#891 slice 3) holds the wakes that never became
sessions, so "skipped" is read there and only for this week: its first
row is 2026-10-11, and a prior week of zeros would read as every seat
newly throttled.
"""

import statistics
from datetime import datetime, timedelta, timezone

import d1

WINDOW = timedelta(days=7)
STAMP = "%Y-%m-%dT%H:%M:%SZ"
FLOOR_RUNS = 4
FLOOR_TURNS = 3
FLOOR_DENIALS = 3
SKIP_SHARE = 0.25

EMPTY = {"runs": 0, "measured": 0, "turns_p50": None, "turns_max": None,
         "out_tokens": 0, "denials": 0, "errors": 0, "cost_usd": None}


def med(values):
    # Always a float: statistics.median returns the int itself for an odd
    # count, and a column that prints 5 on one line and 20.00 on the next
    # reads as two different measures.
    clean = [v for v in values if v is not None]
    return float(statistics.median(clean)) if clean else None


def summarize_roles(rows):
    """Per role, from per-run rows: the aggregates retrospect's reader asks first."""
    by_role = {}
    for r in rows:
        by_role.setdefault(r.get("role") or "?", []).append(r)
    out = []
    for role, runs in sorted(by_role.items(), key=lambda kv: -len(kv[1])):
        measured = [r for r in runs if r.get("measured")]
        turns = [r.get("turns") for r in measured]
        costs = [r.get("cost_usd") for r in measured if r.get("cost_usd") is not None]
        out.append({
            "role": role,
            "runs": len(runs),
            "measured": len(measured),
            "turns_p50": med(turns),
            "turns_max": max((t for t in turns if t is not None), default=None),
            "out_tokens": sum(r.get("out_tokens") or 0 for r in measured),
            "denials": sum(r.get("denials") or 0 for r in runs),
            "errors": sum(r.get("error") or 0 for r in runs),
            "cost_usd": float(sum(costs)) if costs else None,
        })
    return out


def ratio_moved(new, old, factor, floor):
    """The larger side is at least `floor` and at least `factor` times the smaller."""
    high, low = max(new, old), min(new, old)
    return high >= floor and high >= factor * low


def moved(new, old, skips, wakes):
    """The measures a role crossed a threshold on, by name; empty when none."""
    out = []
    if ratio_moved(new["runs"], old["runs"], 1.5, FLOOR_RUNS):
        out.append("runs")
    a, b = new["turns_p50"], old["turns_p50"]
    if a is not None and b is not None:
        if abs(a - b) >= FLOOR_TURNS and abs(a - b) >= 0.25 * max(a, b):
            out.append("turns_p50")
    if ratio_moved(new["denials"], old["denials"], 2, FLOOR_DENIALS):
        out.append("denials")
    if new["errors"] != old["errors"]:
        out.append("errors")
    if wakes and skips / wakes >= SKIP_SHARE:
        out.append("skipped")
    return out


def fmt(value):
    return "-" if value is None else f"{value:.1f}"


def lines(recent_rows, prior_rows, gate_rows):
    """The block's lines, from two weeks of session rows and this week's gate rows."""
    new = {s["role"]: s for s in summarize_roles(recent_rows)}
    old = {s["role"]: s for s in summarize_roles(prior_rows)}
    held = {}
    for g in gate_rows:
        if g.get("decision") != "run":
            role = g.get("role") or "?"
            held[role] = held.get(role, 0) + 1
    roles = sorted(set(new) | set(old) | set(held),
                   key=lambda r: (-new.get(r, EMPTY)["runs"], r))
    rows, quiet = [], []
    for role in roles:
        a, b = new.get(role, EMPTY), old.get(role, EMPTY)
        skips = held.get(role, 0)
        tripped = moved(a, b, skips, a["runs"] + skips)
        if not tripped:
            quiet.append(role)
            continue
        rows.append(
            f"    {role:<11} runs {a['runs']} vs {b['runs']}   "
            f"turns p50 {fmt(a['turns_p50'])} vs {fmt(b['turns_p50'])}   "
            f"denials {a['denials']} vs {b['denials']}   "
            f"errors {a['errors']} vs {b['errors']}   "
            f"skipped {skips} of {a['runs'] + skips}   "
            f"moved: {', '.join(tripped)}"
        )
    if not rows:
        return [f"trend: quiet, {len(quiet)} roles within thresholds, 7d vs the prior 7d (trend.py)"]
    head = ["trend, 7d vs the prior 7d, per role, the rows past a threshold in trend.py:"]
    tail = [f"    quiet: {', '.join(quiet)}"] if quiet else []
    return head + rows + tail


def report(now=None, query=None):
    """Print the block. `now` and `query` are injected by the tests alone."""
    query = query or d1.query
    now = now or datetime.now(timezone.utc)
    recent_from = (now - WINDOW).strftime(STAMP)
    prior_from = (now - 2 * WINDOW).strftime(STAMP)
    print()
    try:
        rows = query("SELECT role, at, measured, turns, out_tokens, denials, error, cost_usd "
                     "FROM sessions WHERE at >= ?", [prior_from])
        gates = query("SELECT role, decision FROM guard WHERE at >= ?", [recent_from])
    except d1.Unreadable as error:
        print(f"trend: UNREADABLE ({error}); the session table could not be read, "
              "so no week is compared.")
        return
    recent = [r for r in rows if (r.get("at") or "") >= recent_from]
    prior = [r for r in rows if (r.get("at") or "") < recent_from]
    print("\n".join(lines(recent, prior, gates)))
