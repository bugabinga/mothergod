# ADR-0036: Engineered reds do not wake the fixer

Status: accepted · Date: 2026-08-28 · Supersedes ADR-0034

## Context

ADR-0034 wakes the BDFL on any completed agent run concluding
`failure`, `timed_out`, or `startup_failure`, cutting failure-detection
latency from the hourly cadence to about a minute.

One red is engineered rather than suffered.
When a PR head's copy of `agent-review.yml` differs from `main`'s, the
review action's anti-tamper validation refuses to start and exits 0;
the workflow converts that green nothing into a deliberate red plus a
PR comment naming the owner and the next step, because PR #111 merged
unreviewed behind exactly such a green.
The refusal fires minutes after the PR opens, while the run that
authored the PR is still shepherding it to merge, so both BDFL wakes
this red caused found nothing to rescue (runs 33144578157 and
33147868186, reacting to the refusals on #286 and #292).

## Decision

The alarm distinguishes the machinery failing from the machinery
speaking.

`agent-alarm.yml` observes the agent seats and their clock from outside
via `workflow_run`, and when a completed run concludes `failure`,
`timed_out`, or `startup_failure`, dispatches agent-bdfl on the admin
PAT, recording `source=alarm`.
Deduplication is stateless and transition-based, keyed by workflow and
head branch: the alarm acts only when the same workflow's previous
completed run on the same branch was not also red.
When the red run is agent-bdfl itself, the dispatch is a bounded retry
and one Telegram line tells the operator, because a dead fixer cannot
self-heal.

Two red classes never fire it, because both are the machinery working:

- `cancelled`, which is routine debouncing (ADR-0014);
- a run whose only failing step is agent-review's refusal sentinel,
  matched by step name ("A green review means a review happened"),
  because that red exists to mark an unreviewed head and its PR comment
  already names the owner and the next step.

The sentinel match fails open: a step rename in `agent-review.yml`
restores spurious wakes, which the retrospect surfaces as noise, never
silence about a genuine failure.

## Consequences

A PR whose head trips the anti-tamper validation no longer burns a BDFL
wake on its own by-design red; its rescue paths stay where they were,
the refusal comment on the PR and the scheduled wakes' state check.
Detection latency for genuine reds keeps ADR-0034's minute floor, and
an absent run stays invisible, covered only by the scheduled wakes,
because no completion event exists to observe.
The step name becomes an interface between two workflow files; the
alarm documents it at the match site, and a mismatch degrades to
spurious wakes rather than missed failures.

## Rejected alternatives

Filtering on the PR's changed files from the alarm: it duplicates the
anti-tamper condition and misses the stale-branch case, which produces
the same engineered red.
Making the refusal green or neutral so no alarm event exists: PR #111
is why the red exists; a review that did not happen must not look like
one.
