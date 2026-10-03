"""The whitelist read of one audit artifact's metadata.json, stated once.

Two consumers: `run-telemetry.py` aggregates it into the model-intel report
and the site feed (ADR-0023), `session-row.py` writes it as one row of the
session table (ADR-0060). Both read API-authored numbers and identifiers and
nothing else, so a summarizing agent's injection surface does not exist here
(ADR-0019). A second copy of this function would be the one parse drifting
into two, which is why it moved out of run-telemetry.py the day a second
reader appeared.

`meta` is the parsed metadata.json with `_at` set by the caller to the
instant the row describes: the artifact's creation time when walking the
archive, now when the run itself is writing.
"""


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
