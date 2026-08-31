# ADR-0035: The Telegram worker is the agent clock

Status: accepted · Date: 2026-08-28 · Prompted by the 2026-08-27 clock outage (issue #276)

## Context

Scheduled agent wakes ran on GitHub's `schedule:` trigger, since PR #207
concentrated in one workflow, agent-clock.yml, that dispatches the seats
by PAT. That design fixed the first scheduler failure, bot-attributed
schedule runs failing the claude-code-action token exchange
(agent-clock.yml's header, 2026-08-23), but kept GitHub as the scheduler.

The scheduler has now failed a second, different way. After the
bot-authored PR #268 edited agent-clock.yml's cron lines, GitHub created
no scheduled runs for eight hours: workflow reported active, no error,
no run, no signal, until the operator asked why Telegram was silent.
Delivery was degraded before that too: under the 2026-08-24 reduced
cadence, fires arrived up to two hours late or not at all. GitHub's
schedule registration is a black box that fails silently, its behavior
around bot-authored cron edits is undocumented, and most clock edits in
this project are bot-authored by design. A scheduler that dies silently
when the factory edits its own clock is disqualified, whatever the exact
mechanism.

The project already runs a Cloudflare Worker (infra/telegram-worker/)
that dispatches these same seat workflows on demand with an
operator-issued PAT, is deployed automatically on push to main, and is
tested in CI.

## Decision

The Cloudflare Worker is the clock for every scheduled agent seat.
Cron triggers in infra/telegram-worker/wrangler.toml fire the worker's
`scheduled` handler, which wakes the seats its CLOCK table names for
that expression and records each attempt in the `clocklog` KV key, last
48 ticks, so liveness is verifiable from any repo-side run without
Cloudflare console access. The wrangler.toml cron lines are the single
source of truth for cadence; the values remain ADR-0015/0027's.

No agent workflow may carry a native `schedule:` trigger.
(Correction, PR #419: the rule's scope is claude-code-action seats,
whose token exchange the schedule actor can kill; script-only
workflows such as agent-model-intel keep native schedules safely.
The original text stated the ban unqualified.) The GitHub
schedules in agent-clock.yml and agent-deslop.yml shadow the worker for
one day of recorded ticks as a reversion path, then issue #276 deletes
agent-clock.yml and deslop's `schedule:` block; duplicate wakes during
the shadow collapse in the seats' concurrency groups and claim checks
(ADR-0014).

## Consequences

- A silently dead GitHub schedule can no longer stop the factory;
  GitHub outages can still delay individual wakes, and a failed
  dispatch waits for the next tick rather than retrying.
- Cloudflare becomes availability-critical for scheduled wakes, joining
  its existing critical role in the operator inbox. Clock and inbox
  share one worker and one deploy, deliberately: one machine to reason
  about, one liveness record, at the cost of shared fate.
- Changing cadence is a worker deploy (push to main) instead of a
  workflow edit; deploy failures are visible as red runs, which wake
  the BDFL (ADR-0034).
- The `clocklog` key gives every run a cheap answer to "when did the
  clock last tick", which GitHub never offered.

## Rejected alternatives

A watchdog (worker cron checking agent-clock's latest run, re-kicking
it when stale) keeps two schedulers plus a checker: strictly more
entangled than one scheduler, and it papers over a scheduler we cannot
observe instead of retiring it.
