# ADR-0059: Model ladders name families, not generations

Status: accepted · Date: 2026-10-03 · Amends ADR-0018 (ledger key), ADR-0019 (intake scope), ADR-0021 (empty effort) · Prompted by issue #890

## Context

Every rung in `agents/models.json` was a full model id such as `claude-sonnet-5-5`.
A generation launch therefore cost the factory a decision per family: pick the id, keep the replaced generation beneath it as a 404 floor, check whether the new default effort moved, and verify reachability, which no run can probe.
Two launches in ten days reached the ladders only after the operator named them on Telegram; the factory's two watches, a third-party index and the Claude Code docs pages, missed both (issue #890).

The Claude Code CLI accepts a family alias for `--model`: `fable`, `opus`, `sonnet`, `haiku`, each meaning that family's newest model (`claude --help`, v2.1.288; the cli-reference page `docs-watch` follows says the same).
On the fleet's CLI the aliases resolved to the exact ids the ladders carried, `claude-sonnet-5-5`, `claude-opus-5-5`, `claude-fable-5-1`, read from the session init event on 2026-10-03.
The action the workflows run, `claude-code-action@v1`, installs a CLI version it pins per release, and the floating `v1` tag advances with those releases, so the alias table advances without an edit here.

Two parts of the machinery assumed a rung is a runtime id.
`agent-pause` keys the `model-limits` ledger by `system/init.model`, the resolved id, and `agent-guard` compares rungs against ledger keys by exact string, so an alias rung would never be blocked by its own 429.
An empty effort takes the model's default, which moved from high to medium between Opus 5 and Opus 5.5; under an alias, a generation bump would import such a cut silently.

Operator directive (Telegram, 2026-10-03): the ladder cares about the alias and the effort level; research only needs to find new families.

## Decision

A rung is a family alias.
The CLI owns which generation a family means; the ladder owns only the order of families and the effort.

The `model-limits` ledger is keyed by the rung the guard chose.
The guard emits the bare rung beside `model_flag`, every agent workflow hands it to `agent-pause`, and `agent-pause` ledgers that string, naming the resolved id only in the log line and the alert.
A seat with no ladder passes no rung and is ledgered under the resolved id, which the guard never consults for it.

Effort is explicit on every seat and every thrift block.
An empty effort remains mechanically legal (ADR-0021) and is no longer a value this file carries.

Model intake (ADR-0019) narrows to new families: a `claude` entry in the catalogue that names no family on a ladder, or a new alias on the cli-reference page.
A generation inside a family is not a finding.
The same-day rule that kept two generations of one family on a ladder, the replaced one as the floor, is withdrawn: the floor beneath a family is the next family.

## Consequences

A generation launch costs no PR, no review and no canary; it arrives with the action's next CLI, and the fleet moves to it on the day the CLI says so rather than on a decision.
The retrospect, which reads every session, is the tell for a generation that reasons worse, and the resolved id stays on record where facts belong: run-telemetry's "model actually run" column and the audit artifacts, so "which model did that" remains a query.

An alias the subscription does not serve, or that the CLI moved to a generation it does not serve yet, is a 404 like any other rung: ledgered for seven days, the seat drops to the next family at the cost of one run, visible in the ledger issue.

`/models` and the ladder now state intent, family and effort, and nothing else.
The operator's premise becomes a dependency: the fleet's CLI must stay current.
The floating `v1` tag provides that within hours of each action release.
A SHA pin would provide it one weekly dependabot PR at a time, because the action hardcodes the CLI version it installs (`base-action/action.yml`, `CLAUDE_CODE_VERSION`) and the pin fixes that version until the pin moves.
(Corrected 2026-10-03: this line first said a SHA pin "would silently end" the currency. Dependabot has moved this repository's SHA pins since PR #514; operator question, Telegram message 863.)

## Rejected alternatives

Keep ids and fix the watches, issue #890's plan: a sharper watch still leaves three decisions per launch with the factory, and the operator had already made them twice.

Map resolved ids to families inside the guard, `claude-sonnet-5-5` to `sonnet`: a naming convention in the decider, which ADR-0018 refused for the ledger and which rots with the first id that breaks the pattern.
Passing the rung is one input and no convention.
