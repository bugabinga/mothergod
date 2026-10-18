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
`allowance_rows`, so the two writers that share `row` share them too; the
tools table's rows (#891 slice 5) by `tools_rows`, and the walls table's
(#1241) by `walls_rows`, the same way. Both send one run's rows as one
`script`, one call per run (#1355).
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


def notional_cost(model_usage):
    """Projected API cost at list rates, straight from the SDK's own
    per-model figure. None, not 0, when the field is missing or not a
    per-model mapping: an unpriced run excluded from sums beats a total
    that quietly undercounts. The one price rule, for `facts` and for
    every reader that needs only the price of an artifact whose other
    fields may be in an older shape.
    """
    if not isinstance(model_usage, dict):
        return None
    costs = [v["costUSD"] for v in model_usage.values()
             if isinstance(v, dict) and isinstance(v.get("costUSD"), (int, float))
             and v.get("costBasis") in (None, "list")]
    return sum(costs) if costs else None


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
    trigger = meta.get("trigger") if isinstance(meta.get("trigger"), dict) else {}
    persona = meta.get("persona") if isinstance(meta.get("persona"), dict) else {}
    tools = tele.get("tools") if isinstance(tele.get("tools"), dict) else {}
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
        "cost": notional_cost(model_usage),
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
        # Write-shaped tool calls that did not error (#896 item 5, the set is
        # audit-extract.py's); None on an artifact from before the count.
        "writes": as_int(tools.get("writes")),
    }


# The session table's row, in the column order infra/sessions/schema.sql
# declares. Both writers bind it to INSERT; a new column is one line here
# and one ALTER line in the schema, in the same diff.
COLUMNS = (
    "run_id", "attempt", "at", "role", "event", "actor", "number",
    "commit_sha", "measured", "model", "out_tokens", "think_pct", "cost_usd",
    "turns", "duration_ms", "denials", "error", "stop_reason", "persona_sha",
    "prompt_bytes", "response_bytes", "writes",
)
INSERT = (f"INSERT OR REPLACE INTO sessions ({', '.join(COLUMNS)}) "
          f"VALUES ({', '.join('?' * len(COLUMNS))})")


# The allowance table's row: the last valid reading of one window one run
# reported. `at` and `role` are the session's, joined on (run_id, attempt),
# never repeated here.
ALLOWANCE_COLUMNS = ("run_id", "attempt", "window", "utilization", "resets_at", "overage")

# The API reports three windows; past MAX_WINDOWS the artifact is not a
# run's and its rows are cut here, where they are made, so both writers
# inherit the cap (#995 round 6).
MAX_WINDOWS = 16


def allowance_insert(rows):
    """One INSERT OR REPLACE for every row in `rows`, one statement of the run's script."""
    values = f"({', '.join('?' * len(ALLOWANCE_COLUMNS))})"
    return (f"INSERT OR REPLACE INTO allowance ({', '.join(ALLOWANCE_COLUMNS)}) "
            f"VALUES {', '.join([values] * len(rows))}")


def allowance_rows(meta):
    """The last valid reading per window in the run's rate_limit_events, as rows.

    The shape rule is `allowance.window_readings`; the validation is the
    consumers' union: a reading exists at all on `valid_fraction` alone
    (retrospect's footer informs on the fraction), and keeps its reset only
    when `valid_reset` (the guard's allowance ledger demands both). Last wins,
    in event order, because the API reports the window's current state each
    time. No run_id, no events, or none readable is an empty list, never a
    row of nulls: a run that reported nothing is absent, which is what a
    census can tell apart. At most `MAX_WINDOWS` rows, the first by name.
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
    return [last[kind] for kind in sorted(last)[:MAX_WINDOWS]]


# The tools table's row: one tool the run called or was refused, with its
# counts (#891 slice 5). `at` and `role` are the session's, joined on
# (run_id, attempt), never repeated here.
TOOLS_COLUMNS = ("run_id", "attempt", "tool", "uses", "denials")
# A run names the tools it used, a dozen against an offered set of 28 on
# 2026-10-12; past MAX_TOOLS the artifact is not a run's.
MAX_TOOLS = 100


def tools_insert(rows):
    values = f"({', '.join('?' * len(TOOLS_COLUMNS))})"
    return (f"INSERT OR REPLACE INTO tools ({', '.join(TOOLS_COLUMNS)}) "
            f"VALUES {', '.join([values] * len(rows))}")


def count(value):
    """A count the extractor wrote, or None: a bool, a float or a string is not one."""
    if isinstance(value, bool) or not isinstance(value, int):
        return None
    return value if 0 <= value < 2**63 else None


def tools_rows(meta):
    """One row per tool the run called or was refused, sorted by name.

    Rows exist only where the use record does (`telemetry.tools.used`,
    #946): an older artifact names its refused tools and nothing it used,
    and a row of unknowns for it would read as a run that used nothing.
    `uses` is the record's count, 0 for a tool only refused. `denials` is
    the per-tool count where the artifact carries one
    (`permission_denials.by_tool`), NULL where it names the tool refused
    and carries no count, 0 for a tool it does not name. A count that is
    not one raises, so the artifact costs one unreadable line (the walk's
    invariant) and never a row of invented zeros.
    """
    run_id = as_int(meta.get("run_id"))
    tele = meta.get("telemetry") if isinstance(meta.get("telemetry"), dict) else {}
    record = tele.get("tools") if isinstance(tele.get("tools"), dict) else {}
    used = record.get("used")
    if run_id is None or not isinstance(used, dict):
        return []
    attempt = as_int(meta.get("run_attempt")) or 1
    refused = tele.get("permission_denials")
    refused = refused if isinstance(refused, dict) else {}
    by_tool = refused.get("by_tool") if isinstance(refused.get("by_tool"), dict) else None
    named = ({str(n) for n in refused["tools"]}
             if isinstance(refused.get("tools"), list) else set())
    names = set(used) | (set(by_tool) if by_tool is not None else named)
    rows = []
    for name in sorted(names)[:MAX_TOOLS]:
        uses = count(used.get(name, 0))
        if uses is None:
            raise ValueError(f"tools.used[{name!r}] is not a count")
        if by_tool is not None:
            denials = count(by_tool.get(name, 0))
            if denials is None:
                raise ValueError(f"permission_denials.by_tool[{name!r}] is not a count")
        else:
            denials = None if name in named else 0
        rows.append((run_id, attempt, name, uses, denials))
    return rows


# The walls table's row: one wall one run hit, the guard or harness rule
# that refused a tool (#1241). `at` and `role` are the session's, joined
# on (run_id, attempt), never repeated here.
WALLS_COLUMNS = ("run_id", "attempt", "tool", "wall")
# A run's reasons are distinct triples, a handful at most; past MAX_WALLS
# the artifact is not a run's.
MAX_WALLS = 100
# A wall's name is a hook's file name or the harness's decision type, both
# fixed vocabulary; a longer one is not a name and is cut, never stored.
WALL_NAME_CAP = 64


def walls_insert(rows):
    values = f"({', '.join('?' * len(WALLS_COLUMNS))})"
    return (f"INSERT OR REPLACE INTO walls ({', '.join(WALLS_COLUMNS)}) "
            f"VALUES {', '.join([values] * len(rows))}")


def wall_of(entry):
    """(tool, wall) for one reasons entry, or None for one of another shape.

    A hook-path denial's reason is `hook: line` (walls.py), the hook being
    the wall; a denial the harness decided itself carries its
    decision_reason_type (mode, rule) as `type`, which is the wall. Fixed
    vocabulary both ways, so the name is an identifier, never the model's
    text, and the keys-only rule for model prose (ADR-0019) holds.
    """
    if not isinstance(entry, dict):
        return None
    tool, kind, why = entry.get("tool"), entry.get("type"), entry.get("reason")
    if not all(isinstance(v, str) for v in (tool, kind, why)) or not tool:
        return None
    wall = why.split(":", 1)[0].strip() if kind == "hook" else kind
    return (tool, wall[:WALL_NAME_CAP]) if wall else None


def walls_rows(meta):
    """One row per wall the run hit, sorted, from `permission_denials.reasons`.

    Rows exist only where the reasons record does (#1110): an older
    artifact counts its denials and names no wall, and gets no row, the
    absence the tools table keeps for an artifact before #946. An entry of
    another shape is skipped, not raised on: a skipped entry loses one
    row and invents nothing, where a tools count of the wrong shape would
    have invented a zero.
    """
    run_id = as_int(meta.get("run_id"))
    tele = meta.get("telemetry") if isinstance(meta.get("telemetry"), dict) else {}
    refused = tele.get("permission_denials")
    reasons = refused.get("reasons") if isinstance(refused, dict) else None
    if run_id is None or not isinstance(reasons, list):
        return []
    attempt = as_int(meta.get("run_attempt")) or 1
    hits = {(run_id, attempt) + named for named in map(wall_of, reasons) if named}
    return sorted(hits)[:MAX_WALLS]


def statements(values, readings, tool_rows, wall_rows):
    """Every write of one run's rows as (sql, params), the session row LAST.

    `sessions backfill` walks the artifacts whose (run_id, attempt) the
    sessions table lacks, so a session row present is the mark that the
    run's allowance, tools and walls rows are in. Written last, a derived
    write that fails leaves the run absent and the next walk writes all
    four from the artifact; written first, as before #1102, the miss was one
    warning and then a gap for good, past the artifact's 90 days with
    nothing left to repair it from. Both writers, session-row.py live and
    backfill from the archive, take their order from here and nowhere else,
    through `script`.
    """
    if readings:
        yield allowance_insert(readings), [v for r in readings for v in r]
    if tool_rows:
        yield tools_insert(tool_rows), [v for r in tool_rows for v in r]
    if wall_rows:
        yield walls_insert(wall_rows), [v for r in wall_rows for v in r]
    yield INSERT, list(values)


# D1 reads at most 100 KB of text per statement, in a batch or alone. A
# run's largest statement, its tools insert, is a few kilobytes; past this
# cap the artifact is not a run's, and `script` raises, so the walk counts
# it unreadable and the live writer warns, never a call that fails and
# takes the runs beside it (#995 round 6's class, in literal form: a bound
# text left the statement small, an inlined one is the statement).
MAX_STATEMENT_BYTES = 50_000


def literal(value):
    """`value`, one `bindable` returned, as the SQL literal D1 parses back to it.

    Inlined because the endpoint refuses params on a call of more than one
    statement ("params with multiple statements is not supported", probed
    2026-10-18), and one call per run is what a walk of the archive can
    afford (#1355). A string's quote doubles, the one escape SQLite's
    literal has; a NUL is cut, because the parser reads it as the end of
    the text and every statement after it would go unexecuted. A float is
    finite (`bindable`), so its `repr` is a literal SQLite reads back.
    """
    if value is None:
        return "NULL"
    if isinstance(value, str):
        return "'" + value.replace("\x00", "").replace("'", "''") + "'"
    if isinstance(value, float):
        return repr(value)
    return str(int(value))


def script(values, readings, tool_rows, wall_rows):
    """One run's `statements` as one `;`-joined text, their params inlined.

    The one write both writers send, one call per run (#1355). The order is
    `statements`' own, the session row last, so a call that fails midway
    leaves the run absent, never present without its derived rows.
    """
    parts = []
    for sql, params in statements(values, readings, tool_rows, wall_rows):
        holes = sql.split("?")
        if len(holes) != len(params) + 1:
            raise ValueError(f"{len(holes) - 1} placeholders for {len(params)} values")
        text = "".join(h + literal(p) for h, p in zip(holes, params)) + holes[-1]
        size = len(text.encode("utf-8"))
        if size > MAX_STATEMENT_BYTES:
            raise ValueError(f"a statement of {size} bytes, over {MAX_STATEMENT_BYTES}")
        parts.append(text)
    return ";\n".join(parts)


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
        f["writes"],
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


def archive(repo, since=None, prefix="audit-"):
    """Every unexpired artifact of `repo` whose name starts with `prefix`, oldest first.

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
            if not art.get("name", "").startswith(prefix) or art.get("expired"):
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
