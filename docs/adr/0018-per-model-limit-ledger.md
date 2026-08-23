# ADR-0018: Per-model limit ledger and agent model ladders

Status: accepted · Date: 2026-08-23 · Amends ADR-0004 (limit handling), ADR-0012 (how the floor is expressed)

## Context

Run 32632168726 (BDFL, 2026-08-23 09:51 UTC) failed with HTTP 429 after 16
turns. The payload:

```
terminal_reason:  api_error
api_error_status: 429
rate_limit_events: seven_day_overage_included -> allowed_warning (0.99)
                   seven_day_overage_included -> rejected, resetsAt 1787781600
result:           "You've reached your Fable 5 limit. Switch to another
                   model to continue."
```

Two separate defects surfaced, pointing opposite ways.

**The detector missed it.** `agent-pause` matched result text against
`usage limit|weekly limit|5-hour limit|session limit|out of extra usage`.
A model-scoped phrase matches none of them, so no pause issue opened, no
RESUME-AT was set, no alert was sent. ADR-0004 makes pause-on-limit a
mission-tier requirement and it silently did not run.

**Had it fired, it would have been wrong.** Only the BDFL pins Fable;
`agent-review` and `agent-heartbeat` completed successfully in the same
window on other models. A global pause would have idled five agents for
three days because one agent's model ran out. The correct outcome that
day came from a broken detector, which is not a property to keep.

Operator directive, on reading the post-mortem: a Fable-specific pause is
not the fix, "because in the future new models and weird anthropic rules
will appear".

## Decision

**Limits are per-model state, not a global boolean.**

A 429 carries the only two facts needed: which model, and until when.
Both are structured in the payload (`system/init.model`, and `resetsAt`
on the rejected `rate_limit_event`). Nothing else is read.

### Ledger

An open issue labeled `model-limits` holds a fenced JSON object,
`model id -> epoch seconds`. Model ids are exact runtime strings: no
aliases, no families, so a renamed or brand-new model is simply a new
key. Entries expire against their own timestamp, so nothing needs
closing and no cleanup job exists.

`agent-pause` writes it and routes three ways:

| Signal | Route |
|---|---|
| Rejected rate limit or 429, with a model attributable | Ledger that model. **No global pause.** |
| HTTP 401/403 | Global pause, indefinite. Auth does not heal on a timer. |
| Limit text with no attributable model | Global pause on the old heuristic. Fallback only. |

**No classification of limit types.** The payload said
`seven_day_overage_included` while the message said "your Fable 5 limit",
so account-wide and model-scoped are not reliably distinguishable from
the data. Always ledger the model in use: if a limit really is
account-wide, each agent ledgers its own model as it fails and the fleet
converges on stopped, without one line of taxonomy that a future
Anthropic rule change can falsify.

### Ladders

`agents/models.json` maps each role to an ordered list. `agent-guard`
resolves it against the ledger and emits `model_flag` for the first rung
not currently limited.

**The last rung is the floor.** Nothing below it is reachable: an
exhausted ladder means that agent skips the cycle, which is a legitimate
terminal state. When this ADR shipped, that mechanism also carried
ADR-0012's "never below Opus 5"; the 2026-08-23 addendum repealed that
constraint, and the floor here is now only the guard's rule, not a
permission. An empty ladder means today's behaviour exactly: no `--model`,
action default, never falls back, never skipped for model reasons. Four
of five agents ship with an empty ladder and are unaffected.

The file sits outside `.github/` deliberately. The BDFL holds the admin
PAT and *can* push workflow files, but that path requires clearing
`http.extraheader` and lands operator-attributed. A file in `agents/` is
an ordinary app-token commit, so the BDFL executes its ADR-0012 duty with
less ceremony and fewer ways to fail.

### Failing open

Every unreadable input resolves toward running, not halting: a missing or
malformed `models.json`, an unparseable ledger body, a limit with no
`resetsAt` (which defaults to one hour). A wrong-short guess costs one
429; a wrong-long guess idles an agent for days. This is the one place
the design is deliberately not conservative.

## Consequences

**One failed run per model, per exhaustion event.** A limit is discovered
by hitting it. The guard cannot know in advance. What changes is the
*next* run, which starts on the fallback rung instead of failing
identically. Before this, every BDFL run until 26 August would have
failed the same way.

The ledger was seeded from run 32632168726's own payload
(`claude-fable-5` until 1787781600), so the BDFL recovers on its next
tick without spending a second run to learn what the first one already
recorded.

`agents-paused` narrows to what it should always have meant: the
operator's kill switch, plus auth failures. It is no longer the response
to a rate limit that has a known scope.

**Known gap, accepted deliberately.** A *retired* model is not a 429. It
would fail every run until someone edits the ladder, because the ledger
only listens for rate limits. Operator's call on reading this: "we deal
with retired models when it comes to that."

**Second gap.** A stale ladder degrades silently: if nobody adds the next
model, agents keep running an older one and nothing complains. The
`reviewed` date in `models.json` is the only signal, and the BDFL's
existing weekly survey duty is the only thing that reads it.

The `paused` output now carries two meanings, global pause and exhausted
ladder, because both mean the same thing to a workflow step. The log line
always says which fired. Renaming it would have meant editing every gate
in five workflows, and a missed edit there silently ungates an agent
against a real pause; the naming imprecision is the cheaper risk.
