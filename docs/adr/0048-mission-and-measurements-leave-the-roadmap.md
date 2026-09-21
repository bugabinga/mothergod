# ADR-0048: Mission and measurements leave ROADMAP.md

Status: accepted · Date: 2026-09-21 · Prompted by issue #487 (operator report, 2026-09-02) · Relocates the file named in ADR-0011 and ADR-0047

## Context

`ROADMAP.md` held three kinds of content with three owners and three lifetimes.
The Mission is the operator's and near-frozen, the one text no agent may amend (ADR-0011).
The Scorecard's definitions are the BDFL's and change rarely.
The Scorecard's measurements are generated: `status-data.py` and `trust-telemetry.py` compute them at deploy and publish them on `/status`, and `docs/benchmarks/` holds the per-file reports.

The file nonetheless carried copies of the numbers in prose: a Silesia aggregate and its gap to `zstd -19`, dated 2026-08-29, in two places, and a clause reading "once the trust ledger lands (#449)" four days after it had landed.
ADR-0047 removed a third duplicated state, milestone checkboxes, and named the mechanism: a hand-copied fact drifts, and the page rendering it publishes the drift.
A measured number copied into prose drifts the same way.

The Mission's exposure is different in kind.
A frozen text inside a file every agent edits is protected by attention alone.
`.github/CODEOWNERS` already requests the operator's review on constitution-level paths, as visibility rather than a gate (ADR-0009), but it can only name whole files.

## Decision

`ROADMAP.md` holds what the BDFL maintains by hand and nothing else: the scorecard's definitions, the milestone-versus-program distinction, rank, the rationale for each placement, and the product shape.

The Mission lives in `MISSION.md`, moved verbatim.
ADR-0011's rule attaches to that file: the operator alone amends it, and the BDFL proposes via `blocked-on-human`.
`CODEOWNERS` names the operator on it, so every pull request touching it requests the operator's review.

A measured scorecard number appears only where it is published, and each metric's definition names that home.
RATIO and SIMPLICITY read `/status` from `status-data.py`; TRUST reads the trust ledger `trust-telemetry.py` aggregates on the same page (#449); SPEED reads the throughput columns of `docs/benchmarks/`; USERS reads `marketing/JOURNAL.md`; the process metrics read the weekly survey digest.
No metric grows a second generator: the publisher that already owns the evidence is the source, and prose points at it.

## Consequences

An agent editing the roadmap cannot touch the mission by accident, because the mission is not in the file it is editing, and the operator sees every change to `MISSION.md` before merge.

The scorecard reads as a contract rather than a status report and cannot go stale on a number, because it carries none.
A reader wanting the number follows a pointer to a page generated that day.

Every pointer to "the Mission section of `ROADMAP.md`" now names `MISSION.md`: `CLAUDE.md`, `agents/GOVERNANCE.md`, the BDFL prompt, the `web-ui` and `information-placement` skills.
ADR-0011 and ADR-0047 carry an inline location note instead of a rewrite, because their decisions stand.

Naming each publisher exposed one metric with none: SIMPLICITY's public API surface.
The scorecard says so, and #651 schedules the publisher, per the standing rule that an unmeasurable metric is itself a top gap.

## Rejected alternatives

Keep the mission in `ROADMAP.md` and gate it with `CODEOWNERS` and the ruleset's code-owner-review box.
The box applies to every `CODEOWNERS` path at once, including `docs/adr/`, and would put the operator in front of every ADR; ADR-0009 left it off for that reason, and a section cannot be named by path in any case.

Move the scorecard definitions out as well, leaving `ROADMAP.md` to milestones alone.
Definitions and rank share an owner and a cadence, and a scorecard in its own file is one more place the heartbeat reads every day for a text that changes a few times a year.
