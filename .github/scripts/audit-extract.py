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
import sys

from allowance import valid_fraction, valid_reset, window_readings

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

# Rate-limit state (issue #63): the typed rate_limit_event payload is
# API-authored metadata, published verbatim so its semantics can be
# decided from evidence. Presence alone is NOT a pause signal: it
# appeared in a healthy no-op run (issue #11's lesson applies).
rate_limits = [e.get("rate_limit_info") for e in entries
               if isinstance(e, dict) and e.get("type") == "rate_limit_event"
               and e.get("rate_limit_info") is not None]

# Compact allowance observation for list-only consumers such as the
# Telegram worker's /budget command. GitHub exposes artifact names and
# creation times without downloading their ZIPs, so a suffix carrying
# utilization basis points and reset epoch makes two observations
# enough to derive burn and runway. The full rate_limit_events payload
# below remains authoritative; this is a rounded, lossy index only.
#
# The payload shape rule and the validation asymmetry against
# retrospect's budget footer live in allowance.py, stated once. The
# index encodes both fields, so a reading with either half invalid is
# skipped here; the footer's per-kind alarm is the loud signal when a
# window stops parsing for this consumer too.
allowance_index = ""
last_utilization, last_reset = "", ""
for info in rate_limits:
    for kind, window in window_readings(info):
        utilization, reset = window.get("utilization"), window.get("resetsAt")
        if (kind != "seven_day"
                or not valid_fraction(utilization)
                or not valid_reset(reset)):
            continue
        basis_points = int(utilization * 10_000 + 0.5)
        allowance_index = f"-u{basis_points}-r{reset}"
        last_utilization, last_reset = utilization, reset
with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as fh:
    fh.write(f"allowance_index={allowance_index}\n")
    # Plain (non-suffix) form for the ledger-write step in the action
    # (#202): a shell step reads these instead of re-parsing
    # allowance_index.
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

secrets = [v.strip() for v in os.environ.get("REDACT", "").splitlines() if len(v.strip()) >= 8]
redactions = 0
scrubbed = {}
for name, body in files.items():
    for value in secrets:
        if value in body:
            body = body.replace(value, "***REDACTED***")
            redactions += 1
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
