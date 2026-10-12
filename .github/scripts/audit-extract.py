#!/usr/bin/env python3
"""Extract one agent run's audit files from its execution log.

Run by `.github/actions/agent-audit` (the action's header states the audit
contract: what is published, what is deliberately withheld, why failure
here never fails the run). A file here rather than a heredoc there because
CI glue Python lives in `.github/scripts/` as files (CLAUDE.md style), and
because `action.test.mjs` exercises this logic and used to slice it back
out of the YAML to do so (issue #310).

Contract: argv[1] is the output directory, already created. Everything
else arrives by environment: EXEC_FILE, ROLE, REDACT, EVENT, ACTOR,
NUMBER, SHA, REF, RUN_ID, RUN_ATTEMPT from the action step, RUNNER_TEMP,
GITHUB_WORKSPACE, GITHUB_OUTPUT, GITHUB_STEP_SUMMARY from the runner.
Exit is non-zero only when the extraction itself broke; the caller
downgrades that to a warning.
"""

import hashlib
import json
import os
import re
import sys

from allowance import valid_fraction, valid_reset, window_readings
from secret_scrub import scrub

out = sys.argv[1]
path = os.environ.get("EXEC_FILE", "")

entries = []
try:
    raw = open(path, encoding="utf-8", errors="replace").read()
    try:
        data = json.loads(raw)
        entries = data if isinstance(data, list) else [data]
    except json.JSONDecodeError:
        for line in raw.splitlines():
            line = line.strip()
            if line:
                try:
                    entries.append(json.loads(line))
                except json.JSONDecodeError:
                    pass
except OSError as exc:
    print(f"cannot read execution file: {exc}")


def text_of(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for block in content:
            if isinstance(block, dict) and block.get("type") == "text":
                parts.append(block.get("text", ""))
            elif isinstance(block, str):
                parts.append(block)
        return "\n".join(p for p in parts if p)
    return ""


# Input. The first user turn was expected to carry the composed
# prompt, but a live run (32562011724) parsed 152 entries and matched
# none. claude-code-action writes the composed prompt to exactly
# $RUNNER_TEMP/claude-prompts/claude-prompt.txt (verified live from
# inside run 32562967070 of this repo); read that one path, no
# globbing, so an unrelated file can never masquerade as the prompt.
# The log fallback keeps the longest user turn. Census below is keys
# only, never values: a census must not become a leak.
prompt = ""
prompt_file = os.path.join(os.environ.get("RUNNER_TEMP", "/tmp"), "claude-prompts", "claude-prompt.txt")
if os.path.isfile(prompt_file):
    try:
        prompt = open(prompt_file, encoding="utf-8", errors="replace").read()
        print(f"prompt from file: {prompt_file}")
    except OSError:
        pass

if not prompt:
    for e in entries:
        if not isinstance(e, dict):
            continue
        msg = e.get("message") if isinstance(e.get("message"), dict) else e
        role = e.get("role") or msg.get("role") or ""
        if e.get("type") in ("user", "human") or role in ("user", "human"):
            candidate = text_of(msg.get("content"))
            if len(candidate) > len(prompt):
                prompt = candidate

# Persona (issue #168). It reaches the model as
# --append-system-prompt-file agents/personas/<role>.md, a CLI flag,
# so it appears in neither the captured user prompt nor the action's
# own options log, which records only "systemPrompt": preset. Without
# this block, "text in" is a half-truth.
#
# The path is derived from role because that mapping IS the
# convention all five workflows follow, so the path stays stated
# once. A workflow that deviates reports exists=false, which is the
# visible zero, not a silent pass.
#
# An ABSENT file is already loud without us: the CLI refuses to start
# ("Error: Append system prompt file not found: <path>", exit 1,
# verified against 2.1.241), so that case reddens the claude step
# before this action runs. What is left to catch is the quiet one, a
# file that exists and is empty or is not the text anyone meant, and
# that is what bytes and sha256 answer.
persona_path = f"agents/personas/{os.environ.get('ROLE', '')}.md"
persona, persona_found = "", False
try:
    with open(os.path.join(os.environ.get("GITHUB_WORKSPACE", ""), persona_path),
              encoding="utf-8", errors="replace") as fh:
        persona = fh.read()
    persona_found = True
except OSError as exc:
    print(f"::warning::persona not readable at {persona_path}: {exc}")
if persona_found and not persona.strip():
    print(f"::warning::persona at {persona_path} is empty: this run had no persona")
persona_meta = {
    "path": persona_path,
    "exists": persona_found,
    "bytes": len(persona.encode("utf-8")),
    "sha256": hashlib.sha256(persona.encode("utf-8")).hexdigest() if persona_found else None,
}

# Output: the last result entry carries the agent's final response.
# Telemetry (issue #64): run economics lifted from the result entry
# by whitelist, never wholesale. These values are API-authored
# numbers and enums, so the keys-only rule (which guards against
# model-authored prose) does not apply. permission_denials is the
# exception: its tool_input is model text, keep count and names only.
# usage/modelUsage publish unsanitized, a deliberate deviation from
# upstream (claude-code-action's sanitizeModelUsage strips them to
# model limits for console logs): upstream protects arbitrary users'
# sessions, while this project publishes its own agents' usage as
# scorecard evidence, per the operator's issue #64 line that
# aggregate numbers are safe to be public and prose is not.
# total_cost_usd re-keys to total_cost_usd_notional: under
# subscription auth it is a notional figure, not a bill, and a
# public number that reads as spend but is not fails honesty.
response, is_error, telemetry = "", None, {}
for e in entries:
    if isinstance(e, dict) and e.get("type") == "result":
        for key in ("result", "text", "content"):
            if e.get(key):
                response = text_of(e[key])
                break
        is_error = e.get("is_error", is_error)
        for key in ("num_turns", "duration_ms",
                    "duration_api_ms", "ttft_ms", "stop_reason",
                    "terminal_reason", "api_error_status", "usage",
                    "modelUsage"):
            if key in e:
                telemetry[key] = e[key]
        if "total_cost_usd" in e:
            telemetry["total_cost_usd_notional"] = e["total_cost_usd"]
        denials = e.get("permission_denials")
        if isinstance(denials, list):
            telemetry["permission_denials"] = {
                "count": len(denials),
                "tools": sorted({d.get("tool_name", "?")
                                 for d in denials if isinstance(d, dict)}),
            }

# Why each denial happened (run 35842553330): the result entry counts
# denials and names tools, so the retrospect read "denied 1 (Edit)" on a
# session that had hit the harness's protected-path wall at .cargo/ and
# ended its headless turn asking permission of nobody. The count named
# no wall, and the transcript that did is withheld by design. Each
# `system/permission_denied` entry carries decision_reason_type (an
# enum) and decision_reason, text authored by the harness or by one of
# our own PreToolUse hooks, whose messages are fixed constants that
# never quote the tool input; so the keys-only rule for model text does
# not apply, and REASON_CAP bounds the exposure should either author
# ever start echoing an input. Distinct triples only: one wall hit
# three times is one line to read.
REASON_CAP = 240
reasons = []
for e in entries:
    if not (isinstance(e, dict) and e.get("type") == "system"
            and e.get("subtype") == "permission_denied"):
        continue
    why = e.get("decision_reason")
    if not isinstance(why, str):
        why = json.dumps(why, sort_keys=True) if why is not None else ""
    entry = {
        "tool": str(e.get("tool_name") or "?"),
        "type": str(e.get("decision_reason_type") or "?"),
        "reason": why[:REASON_CAP],
    }
    if entry not in reasons:
        reasons.append(entry)
if reasons:
    summary_block = telemetry.setdefault("permission_denials", {
        "count": len(reasons),
        "tools": sorted({r["tool"] for r in reasons}),
    })
    summary_block["reasons"] = reasons

# Tools (issue #725): what the harness offered and what the model used,
# names and counts only, never inputs, the same keys-only rule the denial
# record follows. `--allowedTools` pre-approves and does not hide, so the
# offered set is the one record that can show an envelope flag doing what
# it says; `system/init`'s `tools` is that set, API-authored, present in
# the first entry of every run. Use counts are the other half (#486's
# per-seat trims want them) and come from the assistant turns' tool_use
# blocks, deduplicated by block id because stream-json may emit one
# assistant entry per content block. An artifact with no init entry
# records `offered: null`, the visible unknown, never an empty list.
tools_offered = None
for e in entries:
    if isinstance(e, dict) and e.get("type") == "system" and e.get("subtype") == "init":
        raw_tools = e.get("tools")
        if isinstance(raw_tools, list):
            tools_offered = sorted({
                str(t.get("name", "?")) if isinstance(t, dict) else str(t)
                for t in raw_tools
            })
        break
uses, seen_blocks = [], set()
for e in entries:
    if not (isinstance(e, dict) and e.get("type") == "assistant"):
        continue
    msg = e.get("message") if isinstance(e.get("message"), dict) else {}
    for block in msg.get("content") or []:
        if not (isinstance(block, dict) and block.get("type") == "tool_use"):
            continue
        block_id = block.get("id")
        if block_id in seen_blocks:
            continue
        seen_blocks.add(block_id)
        uses.append((block_id, str(block.get("name") or "?"), block.get("input")))
tool_uses = {}
for _, name, _ in uses:
    tool_uses[name] = tool_uses.get(name, 0) + 1

# Writes (#896 item 5): how many of the run's tool calls changed the
# project's shared state, so "a wake that changed nothing" is a column of
# the session row and a seat's no-op share is a query (trend.py), not a
# posture. A write is a Bash call that runs one of the four posting and
# shipping scripts, or `gh issue edit` or `gh pr edit` (a label, milestone
# or title is how the curator and the reviewer act without a post), at
# command position, or a github_file_ops commit or delete, whose result was
# not an error: a hook denial and a script that exited non-zero both come
# back as a tool_result with is_error. tg-send is not a write, because
# every BDFL wake sends its status line, no-op included. The command text
# is matched and never kept: the count is the whole record, the keys-only
# rule above.
WRITE_TOOLS = {"mcp__github_file_ops__commit_files", "mcp__github_file_ops__delete_files"}
# Command position: the start, or after `;`, `&`, `|`, `(`, `{` or a newline,
# then an optional `then`, `do`, `else` or `!`, then any run of `VAR=value`
# assignments (a quoted value may hold spaces), `env`, `timeout` with its
# options and duration, and `bash` or `sh` running a script file, then the
# command by any path, followed by whitespace or the end. So
# `sed -n 1,9p .github/scripts/gh-comment` reads the script and is not a
# write, and `gh-comment.test.mjs` is a test file, not a post. Known
# misses, undercounts both: `bash -c '...'` and a quoted command. Known
# overcount: a heredoc or commit-message line that begins with a script
# name, because matching text cannot tell it from a command.
WRITE_SCRIPT = re.compile(
    r"(?:^|[;&|({\n])\s*(?:(?:then|do|else|!)\s+)?"
    r"(?:(?:[A-Za-z_][A-Za-z0-9_]*=(?:\"[^\"]*\"|'[^']*'|\S*)|env|bash|sh"
    r"|timeout(?:\s+-\S+(?:\s+\d\S*)?)*\s+\d\S*)\s+)*"
    r"(?:(?:\S*/)?(?:gh-comment|gh-pr|merge-pr|push-branch)"
    r"|gh\s+(?:issue|pr)\s+edit)(?=\s|$)")


def is_write(name, tool_input):
    if name in WRITE_TOOLS:
        return True
    if name != "Bash" or not isinstance(tool_input, dict):
        return False
    command = tool_input.get("command")
    return isinstance(command, str) and WRITE_SCRIPT.search(command) is not None


errored = set()
for e in entries:
    if not isinstance(e, dict):
        continue
    if e.get("type") == "system" and e.get("subtype") == "permission_denied":
        if e.get("tool_use_id") is not None:
            errored.add(e.get("tool_use_id"))
        continue
    if e.get("type") != "user":
        continue
    msg = e.get("message") if isinstance(e.get("message"), dict) else {}
    for block in msg.get("content") or []:
        if (isinstance(block, dict) and block.get("type") == "tool_result"
                and block.get("is_error") is True and block.get("tool_use_id") is not None):
            errored.add(block.get("tool_use_id"))
writes = sum(1 for block_id, name, tool_input in uses
             if block_id not in errored and is_write(name, tool_input))
telemetry["tools"] = {"offered": tools_offered, "used": dict(sorted(tool_uses.items())),
                      "writes": writes}

# Rate-limit state (issue #63): the typed rate_limit_event payload is
# API-authored metadata, published verbatim so its semantics can be
# decided from evidence. Presence alone is NOT a pause signal: it
# appeared in a healthy no-op run (issue #11's lesson applies).
rate_limits = [e.get("rate_limit_info") for e in entries
               if isinstance(e, dict) and e.get("type") == "rate_limit_event"
               and e.get("rate_limit_info") is not None]

# The last valid seven-day reading, as two plain outputs for the action's
# ledger step (#202): the one-reading guard ledger (#309) is the only
# consumer left on this path, since the worker's /budget reads the
# allowance table session-row.py writes (#891 slice 2b). The payload shape
# rule and the validation asymmetry against retrospect's budget footer
# live in allowance.py, stated once; a reading with either half invalid
# is skipped here, and the footer's per-kind alarm is the loud signal when
# a window stops parsing for this consumer too.
last_utilization, last_reset = "", ""
for info in rate_limits:
    for kind, window in window_readings(info):
        utilization, reset = window.get("utilization"), window.get("resetsAt")
        if (kind != "seven_day"
                or not valid_fraction(utilization)
                or not valid_reset(reset)):
            continue
        last_utilization, last_reset = utilization, reset
with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as fh:
    fh.write(f"utilization={last_utilization}\n")
    fh.write(f"resets_at={last_reset}\n")

shape = {}
for e in entries:
    if isinstance(e, dict):
        key = f"{e.get('type')}/{e.get('subtype')}" if e.get("subtype") else str(e.get("type"))
        bucket = shape.setdefault(key, {"count": 0, "keys": set()})
        bucket["count"] += 1
        bucket["keys"].update(e.keys())
shape = {k: {"count": v["count"], "keys": sorted(v["keys"])} for k, v in sorted(shape.items())}

meta = {
    "role": os.environ.get("ROLE", ""),
    "trigger": {
        "event": os.environ.get("EVENT", ""),
        "actor": os.environ.get("ACTOR", ""),
        "number": os.environ.get("NUMBER", ""),
    },
    "commit": os.environ.get("SHA", ""),
    "ref": os.environ.get("REF", ""),
    "run_id": os.environ.get("RUN_ID", ""),
    "run_attempt": os.environ.get("RUN_ATTEMPT", ""),
    "is_error": is_error,
    "telemetry": telemetry,
    "rate_limit_events": rate_limits,
    "entries_parsed": len(entries),
    "persona": persona_meta,
    "prompt_extracted": bool(prompt),
    "response_extracted": bool(response),
    "entry_shape": shape,
}

files = {
    "input-persona.md": persona if persona_found else f"(no persona file at {persona_path})",
    "input-prompt.md": prompt or "(prompt not found in execution file)",
    "output-response.md": response or "(final response not found in execution file)",
    "metadata.json": json.dumps(meta, indent=2),
}

secrets = os.environ.get("REDACT", "").splitlines()

# The scrubber is shared (secret_scrub.py), not copied: this file used to own
# the only copy under a comment calling the artifact "the one point where
# session text leaves the machine", which was false and left issue comments,
# PR bodies and Telegram unscrubbed.
redactions = 0
scrubbed = {}
for name, body in files.items():
    body, count = scrub(body, secrets)
    redactions += count
    scrubbed[name] = body
    with open(os.path.join(out, name), "w", encoding="utf-8") as fh:
        fh.write(body)

# Human view (operator report, 2026-08-23): the artifact is a zip
# download, unreadable in the browser. Render the same scrubbed
# text on the run page via the step summary; the artifact stays
# the full-fidelity 90-day record. Only scrubbed bodies reach
# here. Clips guard GitHub's 1 MiB summary cap, which is
# byte-based, so clip counts UTF-8 bytes, not codepoints
# (PR #105 review).
summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
if summary_path:
    def clip(text, limit):
        encoded = text.encode("utf-8")
        if len(encoded) <= limit:
            return text
        kept = encoded[:limit].decode("utf-8", errors="ignore")
        return kept + f"\n\n(clipped at {limit} of {len(encoded)} bytes; full text in the artifact)"

    number = os.environ.get("NUMBER", "")
    trigger = os.environ.get("EVENT", "") + (f" #{number}" if number else "")
    summary = "\n".join([
        f"## Audit: {os.environ.get('ROLE', '?')}",
        "",
        f"trigger {trigger}, is_error={is_error}, "
        f"turns={telemetry.get('num_turns', '?')}, "
        f"denials={telemetry.get('permission_denials', {}).get('count', 0)}, "
        f"tools_offered={len(tools_offered) if tools_offered is not None else '?'}, "
        f"persona={persona_meta['bytes']}B",
        "",
        "### Response",
        "",
        clip(scrubbed["output-response.md"], 200_000),
        "",
        "<details><summary>Persona</summary>",
        "",
        f"`{persona_path}`, {persona_meta['bytes']} bytes, "
        f"sha256 {persona_meta['sha256'] or 'none, file not found'}",
        "",
        clip(scrubbed["input-persona.md"], 30_000),
        "",
        "</details>",
        "",
        "<details><summary>Prompt</summary>",
        "",
        clip(scrubbed["input-prompt.md"], 150_000),
        "",
        "</details>",
        "",
        "<details><summary>Metadata</summary>",
        "",
        # Four-backtick fence: survives a triple backtick inside
        # the JSON string values (PR #105 review).
        "````json",
        clip(scrubbed["metadata.json"], 30_000),
        "````",
        "",
        "</details>",
        "",
    ])
    with open(summary_path, "a", encoding="utf-8") as fh:
        fh.write(summary)

print(f"audit: {len(entries)} entries, "
      f"persona={persona_path} {persona_meta['bytes']}B, "
      f"prompt={bool(prompt)}, "
      f"response={bool(response)}, redactions={redactions}")
