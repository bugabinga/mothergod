# ADR-0037: Status publication is deploy-time CI glue

Status: accepted · Date: 2026-08-28 · Clarifies ADR-0020 · Prompted by operator request (Telegram, 2026-08-28)

## Context

`site/status.html` renders `site/status-data.json` (issue #95). PR #245
shipped the page, the JSON as a hand-committed snapshot, and a workspace
crate `site-status/` (678 lines plus a binary) meant to regenerate it via
a future scheduled workflow committing the result. That wiring never
landed, and the committed-artifact pattern it needed was already
falsified: PR #34 moved `/agent-metrics.json` to deploy-time generation
because commits to a tracked data file conflict, and issue #50 makes
admin-token commit-back a self-trigger hazard. Within three days the
snapshot claimed 35 experiments while `research/progress.jsonl` held 51,
and its benchmarks note still said "not yet measurable" after the ratio
gate landed (#286), because the prose fields were hardcoded strings in
the crate's binary.

ADR-0020 reserves Rust for anything "whose output is a number this
project publishes or a bitstream it ships". Two sessions read that clause
oppositely: #245 built a Rust crate for status data, while
`run-telemetry.py` publishes run counts and token medians at
`/agents.html` as Python glue. Both merged. A clause two conforming
sessions read oppositely is not carrying its meaning.

## Decision

"A number this project publishes" in ADR-0020 means a number this
project **measures**: the codec, experiments, the bench harness, corpus
tooling. Republishing a number measured elsewhere, or counting repo-meta
facts (lines, commits, log entries), is CI glue and may be Python.

`site/status-data.json` is generated at deploy time by
`.github/scripts/status-data.py` from repository evidence (ROADMAP.md
checkboxes, `research/progress.jsonl`, `bench/baseline.json`,
`src/lib.rs`'s own const and doc comments, git history), never
committed, exactly the `/agent-metrics.json` pattern. The `site-status`
crate is deleted.

## Consequences

- Staleness is bounded by the deploy cadence: daily cron plus a redeploy
  on merges touching the page's sources. The page cannot silently
  contradict the repository for longer than a day.
- Hand-kept prose leaves the pipeline: the page derives "what works
  today" from `src/lib.rs` doc comments and renders `bench/baseline.json`
  numbers directly, so no sentence can rot inside a binary.
- 678 lines of reviewed Rust are deleted three days after merging. Cost
  accepted: the crate's only wiring plan was a commit-back workflow this
  project had already rejected, and keeping two generators is a drift
  machine.
- Measurement stays Rust. This ADR moves none: the only codec numbers on
  the page come from `bench/baseline.json`, produced by the Rust harness
  and enforced by the required `ratio` check.

## Rejected alternatives

- Wire the crate into deploy-site instead: puts a workspace build on the
  site deploy path for work stdlib Python does in a second, and still
  requires de-hardcoding the prose fields, so the crate grows rather
  than the tree shrinking.
- Scheduled commit-back of the JSON: the falsified pattern (PR #34,
  issue #50) that left this page stale in the first place.
