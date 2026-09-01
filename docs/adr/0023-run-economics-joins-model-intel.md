# ADR-0023: Run economics joins model intel, and the report is its own store

Status: superseded by ADR-0042 (cost exclusion only) · Date: 2026-08-23 · Serves ADR-0012, ADR-0021 · Extends ADR-0019

## Context

ADR-0019 built the capability half of model intel: Artificial Analysis
scores, weekly, delta-only, into an issue the BDFL already triages.

It answers half a question. ADR-0012 asks the BDFL to pick each role's
model and ADR-0021 its effort level, and neither is answerable from
capability alone. "Is this role worth a stronger rung" needs what the
current rung costs per unit of work, and capability benchmarks measure
somebody else's workload.

The other half already existed and nothing read it. Every agent run
uploads an audit artifact whose `metadata.json` carries `usage`,
`modelUsage`, `num_turns`, `duration_ms` and `permission_denials`
(`.github/actions/agent-audit`, issue #64). Ninety days of the truth,
measured on the work we actually run, unread.

The operator raised this as issue #118 and left the design open.

## Decision

**One report, two collectors, one assembly step, one channel.**
`.github/scripts/run-telemetry.py` aggregates our audit artifacts;
`model-intel.py` keeps the Artificial Analysis half; the workflow
concatenates two sections and owns the document's frame. Neither
collector depends on the other, and neither owns the footer, so a
failure in one still ships the other. Joining the halves by hand every
time was the cost this removes.

**No new storage. The report is the store.** KV and R2 were both on the
table because audit artifacts expire at 90 days. They are not needed.
The collector aggregates *two* windows in one pass, so the trend is in
every report; and the report is posted to a GitHub issue, whose edit
history is permanent, free, versioned, and readable by a human without
a client. A database would have duplicated a series GitHub already keeps
for us, which is the synchronization debt CLAUDE.md's first value warns
about. When the 90-day horizon eventually bites, the older snapshots are
in the issue's history, which outlives the artifacts.

**State and event are different signals, so they get different
mechanics.** Run economics is state: the issue body is rewritten every
week and the issue is never closed by the job. A capability delta is an
event: it additionally posts a comment, which is the only thing that
notifies. Delta-only (ADR-0007) governs notification, not state; a
weekly stream of "the numbers moved" would be noise, because the numbers
always move.

**Weekly, in the same run.** The decision this feeds is made weekly at
best. A faster collector would report to nobody.

**Metrics are what the project actually pays.** Output tokens, turns,
minutes, permission denials, error rate, thinking share, and the model
each role actually ran. Cost in USD is deliberately excluded: under
subscription auth the action's figure is notional, not a bill, and a
public number that reads as spend but is not fails the mission's
honesty clause. The audit action already re-keys it
`total_cost_usd_notional` for the same reason.

The workflow keeps its inaccurate `agent-` prefix. It runs no agent, but
renaming splits the run history and edits four call sites to buy
nothing. Recorded here so the next reader does not re-open it.

## Consequences

The first read paid for the pipeline before the pipeline existed. Across
129 artifacts it showed:

- Four of five roles were running `claude-sonnet-5` by action default,
  never chosen. ADR-0012 exists to prevent exactly that.
- The maintainer sat at 11 median permission denials per run, all Bash,
  in 6 of its 6 most recent runs. Fixed in PR #121.
- Aggregating over all time hid that the reviewer's identical problem had
  already been fixed, which is why the report windows and never reports a
  single lifetime number.

Sampling is bounded and stated, never silent: the collector caps
downloads and reports any artifact it skipped or could not read. Runs
whose execution file carried no result entry are excluded from every
median rather than counted as zero, because an unmeasured run is not a
free one; folding them in moved the BDFL's median output from 9724
tokens to 3314.

`model-intel.py` no longer emits the attribution footer. It writes a
section; the workflow writes the document.

The workflow now needs `actions: read` to list and download this repo's
own artifacts, and 20 minutes instead of 10.

## Alternatives rejected

**Cloudflare KV or R2 for a trend line.** Operator-approved and still
wrong: it builds a second store for a series the issue already keeps,
and picks up a synchronization debt for it.

**A second workflow and a second issue.** The decision needs both halves
side by side; two channels means joining them by hand forever, which is
the problem being solved.

**A cost-regression alarm.** Tempting, and deferred with no baseline to
fire against and a flat-rate subscription behind it. Wall clock is the
budget that actually hurts, and the "stay fast" duty already owns it. If
a cost regression proves urgent in practice, it earns its own trip wire
then.

**Buying the Artificial Analysis Pro tier for per-model token counts.**
Paying for a worse proxy of what our own artifacts measure directly.

**Making it an agent.** ADR-0019's three reasons hold unchanged, and the
whitelist extractor here reads only API-authored numbers and model ids;
no model-authored prose reaches the report.
