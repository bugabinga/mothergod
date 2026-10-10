"""The whitelist read of one audit artifact, stated once: the parse, the row, the walk.

Three consumers: `run-telemetry.py` aggregates it into the model-intel report
and the site feed (ADR-0023), `session-row.py` writes it as one row of the
session table the moment a run ends, and `sessions backfill` writes the same
row from the archive (ADR-0060, issue #891). All read API-authored numbers
and identifiers and nothing else, so a summarizing agent's injection surface
does not exist here (ADR-0019). A second copy of `facts` would be the one
parse drifting into two, which is why it moved out of run-telemetry.py the
day a second reader appeared; the archive walk and the row followed it the
day the backfill became a third.

`meta` is the parsed metadata.json with `_at` set by the caller to the
instant the row describes: the artifact's creation time when walking the
archive, now when the run itself is writing.

The allowance table's rows (#891 slice 2) come from the same metadata by
`allowance_rows`, so the two writers that share `row` share them too.
"""

import collections
import io
import json
import math
import subprocess
import zipfile
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime

from allowance import valid_fraction, valid_reset, window_readings


class Unreadable(Exception):
    """The archive could not be listed, or one artifact could not be read."""


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
    persona = meta.get("persona") if isinstance(meta.get("persona"), dict) else {}
    return {
        # Artifact creation instant and run id: identifiers, not numbers,
        # but API-authored like everything else here. They exist for the
        # JSON consumer's recent-runs table and its link to the run page.
        "at": meta["_at"].strftime("%Y-%m-%dT%H:%M:%SZ"),
        "run_id": meta.get("run_id") or "",
        "attempt": meta.get("run_attempt") or "",
        "role": meta.get("role") or "?",
        "commit": meta.get("commit") or "",
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
        "duration_ms": tele.get("duration_ms"),
        "mins": (tele.get("duration_ms") or 0) / 60000.0 or None,
        "denials": (tele.get("permission_denials") or {}).get("count", 0),
        "error": bool(meta.get("is_error")),
        "stop_reason": tele.get("stop_reason") or "",
        # Which persona text the run carried: the key for "since the
        # persona changed" questions, a hash so no prose travels.
        "persona_sha": persona.get("sha256") or "",
    }


# The session table's row, in the column order infra/sessions/schema.sql
# declares. Both writers bind it to INSERT; a new column is one line here
# and one ALTER line in the schema, in the same diff.
COLUMNS = (
    "run_id", "attempt", "at", "role", "event", "actor", "number",
    "commit_sha", "measured", "model", "out_tokens", "think_pct", "cost_usd",
    "turns", "duration_ms", "denials", "error", "stop_reason", "persona_sha",
    "prompt_bytes", "response_bytes",
)
INSERT = (f"INSERT OR REPLACE INTO sessions ({', '.join(COLUMNS)}) "
          f"VALUES ({', '.join('?' * len(COLUMNS))})")


# The allowance table's row: the last valid reading of one window one run
# reported. `at` and `role` are the session's, joined on (run_id, attempt),
# never repeated here.
ALLOWANCE_COLUMNS = ("run_id", "attempt", "window", "utilization", "resets_at", "overage")


def allowance_insert(rows):
    """One INSERT OR REPLACE for every row in `rows`: one round trip per artifact."""
    values = f"({', '.join('?' * len(ALLOWANCE_COLUMNS))})"
    return (f"INSERT OR REPLACE INTO allowance ({', '.join(ALLOWANCE_COLUMNS)}) "
            f"VALUES {', '.join([values] * len(rows))}")


def allowance_rows(meta):
    """The last valid reading per window in the run's rate_limit_events, as rows.

    The shape rule is `allowance.window_readings`; the validation is the
    consumers' union: a reading exists at all on `valid_fraction` alone
    (retrospect's footer informs on the fraction), and keeps its reset only
    when `valid_reset` (the artifact-name index demanded both). Last wins,
    in event order, because the API reports the window's current state each
    time. No run_id, no events, or none readable is an empty list, never a
    row of nulls: a run that reported nothing is absent, which is what a
    census can tell apart.
    """
    run_id = as_int(meta.get("run_id"))
    events = meta.get("rate_limit_events")
    if run_id is None or not isinstance(events, list):
        return []
    attempt = as_int(meta.get("run_attempt")) or 1
    last = {}
    for info in events:
        for kind, window in window_readings(info):
            utilization = window.get("utilization")
            if not isinstance(kind, str) or not valid_fraction(utilization):
                continue
            reset = window.get("resetsAt")
            overage = window.get("isUsingOverage")
            last[kind] = (run_id, attempt, kind, float(utilization),
                          bindable(reset) if valid_reset(reset) else None,
                          int(overage) if isinstance(overage, bool) else None)
    return [last[kind] for kind in sorted(last)]


def as_int(value):
    """An id or count as an integer, or None: '' and junk are absent, not 0."""
    try:
        return int(value)
    except (TypeError, ValueError, OverflowError):  # 1e999 parses to inf
        return None


def bindable(value):
    """The value if D1 can bind it, else None.

    `facts` passes some artifact fields through raw, so a hostile one can be
    a dict, a list, `inf` (`costUSD: 1e999`, which `json.dumps` writes as the
    non-JSON constant `Infinity`) or an integer past SQLite's 64 bits. Any of
    them makes the INSERT fail, and one failed INSERT is not one artifact's
    loss: it kills the backfill at that artifact, and oldest first makes the
    death permanent (#995 round 6).
    """
    if isinstance(value, float):
        return value if math.isfinite(value) else None
    if isinstance(value, int):
        return value if -2**63 <= value < 2**63 else None
    return value if isinstance(value, str) else None


def row(meta, size):
    """One table row from the artifact's metadata, in COLUMNS order.

    Every value is one D1 can bind (`bindable`); a `role` that is not text
    becomes "?", the column being NOT NULL.

    `size(name)` is the byte count of a published text, `input-prompt.md` or
    `output-response.md`, or None. It is asked only when the extractor
    flagged the text as extracted: where the text was missing it wrote a
    placeholder line, whose byte count would read as a tiny prompt.
    """
    f = facts(meta)
    values = (
        as_int(f["run_id"]), as_int(f["attempt"]) or 1, f["at"], f["role"],
        f["event"] or None, f["actor"] or None, as_int(f["number"]),
        f["commit"] or None, int(f["measured"]), f["model"] or None,
        f["out"] if f["measured"] else None, f["think"], f["cost"],
        f["turns"], f["duration_ms"], f["denials"], int(f["error"]),
        f["stop_reason"] or None, f["persona_sha"] or None,
        size("input-prompt.md") if meta.get("prompt_extracted") else None,
        size("output-response.md") if meta.get("response_extracted") else None,
    )
    values = tuple(bindable(v) for v in values)
    return values[:3] + (values[3] or "?",) + values[4:]


# One unexpired audit artifact as the listing names it. `made` is its
# creation instant, the `at` of a row written from the archive.
Artifact = collections.namedtuple("Artifact", "id name made")

# What one walk hands back per artifact: `error` is None and `meta` carries
# `_at` when the artifact was read; otherwise `error` says why it was not.
Read = collections.namedtuple("Read", "artifact meta sizes error")

# Downloads in flight at once. Each zip call is a redirect to blob storage
# at under half a second, and the archive held 1,773 artifacts on
# 2026-10-09, 13 minutes serial; eight in flight is two, and well inside
# the token's hourly rate limit and GitHub's concurrency limits.
WORKERS = 8


def gh(*args):
    """Run gh and return its stdout as bytes; a non-zero exit is Unreadable.

    gh carries the token; nothing here sees one, and the failure text is
    gh's own, which never prints it.
    """
    proc = subprocess.run(["gh", *args], capture_output=True)
    if proc.returncode != 0:
        raise Unreadable(proc.stderr.decode("utf-8", "replace").strip()[:400])
    return proc.stdout


def archive(repo, since=None):
    """Every unexpired audit artifact of `repo`, oldest first.

    `since`, a datetime, keeps the artifacts created at or after it. Oldest
    first because the archive expires from the front: a walk that dies
    midway has written what was about to be lost. The listing is GitHub's,
    so an expired artifact is gone from here the day it is gone from the
    archive.
    """
    try:
        pages = json.loads(gh("api", "--paginate", "--slurp",
                              f"repos/{repo}/actions/artifacts?per_page=100"))
    except ValueError as error:
        raise Unreadable(f"artifact listing is not JSON: {error}") from None
    found = []
    for page in pages:
        for art in page.get("artifacts", []):
            if not art.get("name", "").startswith("audit-") or art.get("expired"):
                continue
            try:
                made = datetime.fromisoformat(art["created_at"].replace("Z", "+00:00"))
            except (KeyError, ValueError):
                continue
            if since is None or made >= since:
                found.append(Artifact(art["id"], art["name"], made))
    found.sort(key=lambda a: a.made)
    return found


def fetch(repo, artifact):
    """One artifact's metadata with `_at` set, and the byte size of every file in it."""
    blob = gh("api", f"repos/{repo}/actions/artifacts/{artifact.id}/zip")
    try:
        with zipfile.ZipFile(io.BytesIO(blob)) as zf:
            sizes = {info.filename: info.file_size for info in zf.infolist()}
            meta = json.loads(zf.read("metadata.json"))
    except (KeyError, ValueError, zipfile.BadZipFile) as error:
        raise Unreadable(str(error)) from None
    if not isinstance(meta, dict):
        raise Unreadable("metadata.json is not an object")
    meta["_at"] = artifact.made
    return meta, sizes


def walk(repo, artifacts):
    """Yield one Read per artifact, in listing order, WORKERS downloads at a time.

    ONE ARTIFACT COSTS ONE UNREADABLE READ, NEVER THE WALK. The archive is
    hostile by assumption (ADR-0019) and oldest first makes any uncaught
    error permanent: the artifact that raised never gets a row, so every
    rerun dies at the same place with nothing after it written. `fetch`
    names the shapes it can explain; everything else (zipfile's RuntimeError
    on an encrypted member, NotImplementedError on method 99, RecursionError
    on nested brackets, the next one) is caught here as the type and the
    message. Naming them one at a time missed three times (#995 rounds 3 to 5).
    """
    def one(artifact):
        try:
            meta, sizes = fetch(repo, artifact)
        except Unreadable as error:
            return Read(artifact, None, None, str(error))
        except Exception as error:  # noqa: BLE001, the invariant above
            return Read(artifact, None, None, f"{type(error).__name__}: {str(error)[:200]}")
        return Read(artifact, meta, sizes, None)

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        yield from pool.map(one, artifacts)
