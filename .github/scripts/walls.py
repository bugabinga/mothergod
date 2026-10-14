"""A guard hook's decision, written where each of its two readers looks (#990).

A deny is a line on the run page and a record in the audit. The line is
`GITHUB_STEP_SUMMARY`, the human's account, `note` appends it. The record
is `walls.jsonl` under `RUNNER_TEMP`, `denied` appends it beside the line,
and `audit-extract.py` lifts it into `permission_denials.reasons`, where
the retrospect reads the wall under the session it judges.

Two walls stood between the reason and that list. The harness emits a
`system/permission_denied` entry for a denial it decided itself, a mode
or a rule, and none for one decided on the PreToolUse hook path, which is
every guard this project runs; since #700 landed the reasons block on
2026-09-23 no retrospect has cited a hook's reason. And the hooks' summary
lines cannot be read back: GitHub gives each step its own summary file,
the hooks write from the Claude step, and the audit's extract step opens
an empty one of its own. The run page joins them, which is why a reason
showed there and nowhere a retrospect looks.

`RUNNER_TEMP` is one directory for every step of a job, the channel the
Claude step's prompt file already crosses to the audit. Outside the
runner there is neither summary nor record. The record's shape is the
harness's own reasons entry, `{tool, type, reason}` with `type` `hook`,
so the audit keeps one list of one shape. A line is fixed vocabulary
plus numbers and process names, never the model's text, which the
audit's keys-only rule for model prose relies on.

Neither write raises. A PreToolUse hook that dies with an exception exits
1, which the harness reads as a non-blocking error and lets the call
proceed, so a runner path that cannot be opened would turn the deny into
an allow (#1111). A write that fails says so on stderr and the hook goes
on to its own exit 2 and message.
"""

import json
import os
import sys

RECORD = "walls.jsonl"


def note(hook: str, line: str) -> None:
    """Append `hook: line` to the step summary, the run page's account."""
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        _append(summary, f"{hook}: {line}\n")


def denied(hook: str, tool: str, line: str) -> None:
    """`note`, plus the record the audit lifts into `permission_denials.reasons`.

    `tool` is what was refused: the tool name a PreToolUse hook saw, or
    `Stop` for a Stop hook's block.
    """
    note(hook, line)
    path = record_path()
    if path:
        _append(path, json.dumps({"tool": tool, "type": "hook", "reason": f"{hook}: {line}"}) + "\n")


def _append(path: str, text: str) -> None:
    """Append `text`, or name on stderr what was lost; never raise (#1111)."""
    try:
        with open(path, "a", encoding="utf-8") as fh:
            fh.write(text)
    except OSError as exc:
        print(f"walls: {path}: {exc.strerror or exc}", file=sys.stderr)


def record_path() -> str | None:
    """Where this job's record lives, or None outside the runner."""
    temp = os.environ.get("RUNNER_TEMP")
    return os.path.join(temp, RECORD) if temp else None


def read() -> list[dict[str, str]]:
    """Every wall this job's hooks recorded, in order; [] with no record.

    A line that is not a `{tool, type, reason}` of strings is skipped: the
    audit never fails on its own input, and the count the harness gives
    still prints.
    """
    path = record_path()
    if not path or not os.path.exists(path):
        return []
    walls = []
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            if (isinstance(entry, dict) and set(entry) == {"tool", "type", "reason"}
                    and all(isinstance(v, str) for v in entry.values())):
                walls.append(entry)
    return walls
