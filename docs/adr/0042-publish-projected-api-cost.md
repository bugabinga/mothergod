# ADR-0042: Publish projected API cost, labeled, not excluded

Status: accepted · Date: 2026-09-01 · Supersedes ADR-0023's cost exclusion

## Context

ADR-0023 deliberately excluded USD from run economics: under subscription
auth the SDK's per-run figure is notional, not a bill, and a public number
that reads as spend but is not fails the mission's honesty clause. That
held until the operator overruled it directly (Telegram, 2026-09-01): the
run-economics report and `agents.html` track runs and tokens but omit
cost, and the projected figure is valuable signal to a reader regardless
of subscription auth. The honesty concern is real but is a labeling
problem, not a reason to omit the number entirely.

## Decision

**Publish the SDK's projected cost, labeled as a projection everywhere it
appears, never omitted.** `run-telemetry.py` sums `modelUsage[*].costUSD`
per run, seat, and window and both consumers render it: the run-economics
report gets a `$/run` column and a window total, `agents.html` gets a stat
tile and per-seat/per-run `$` columns. Every surface states in words that
the figure is a projection at API list rates, not a bill, replacing
omission with a label.

**A modelUsage entry is only summed when its `costBasis` is `"list"` or
absent.** The SDK type also allows `"managed"` or `"unknown"`, an explicit
guess. Folding one of those into the list-rate sum would silently misprice
it, which is the failure this ADR exists to avoid. A run with no summable
entry is counted in a separate `uncosted` field and excluded from every
total, so a total can undercount loudly, never silently.

No new pricing table is added; the SDK already prices each run per model
(`modelUsage.*.costUSD`, `costBasis`).

## Consequences

ADR-0023's cost exclusion is reversed; its other decisions (one report,
two collectors, no new storage, state vs. event mechanics) stand
unchanged and are not reopened here.

`.github/scripts/audit-extract.py` separately re-keys the session-total
`total_cost_usd` to `total_cost_usd_notional`, a labeling choice rather
than an exclusion. It already matches this ADR's philosophy (label, don't
omit) and is left as is.

## Rejected alternatives

**Leave cost excluded.** Overruled directly by the operator: the
projection is a signal a reader can compare against their own compute,
and labeling beats omission.

**Sum every `costUSD` regardless of `costBasis`.** Cheaper to write, but
folds a non-list-rate guess into a total labeled "at list rates," which is
the exact silent inaccuracy this project's honesty clause exists to
prevent.
