# ADR-0034: Red agent runs wake the BDFL

Status: accepted · Date: 2026-08-27 · Prompted by issue #191

## Context

No agent workflow is a required check, so a red agent run gates nothing
and announces itself to nobody (issue #191, first hit run 32660844700).
Detection has been a BDFL wake reading the retrospect, which puts the
detection floor at the hourly cadence (ADR-0015), and removes it
entirely when the broken seat is the BDFL itself.
The pause machinery fires on usage limits, not on failure, and
displaced runs (ADR-0014) make `cancelled` conclusions routine rather
than alarming.
Issue #191 asked whether once-per-wake detection by one seat suffices,
and offered extending the in-job `agent-pause` action as the fix if not.

## Decision

Once per wake is not enough, and no in-job step can be the mechanism,
because a job cannot observe its own failure to start.
A dedicated `agent-alarm.yml` observes the claude seats and their clock
from outside via `workflow_run`, and when a completed run concludes
`failure`, `timed_out`, or `startup_failure`, dispatches agent-bdfl on
the admin PAT, the same operator attribution the clock's wakes carry
(issue #50); the dispatch records `source=alarm` on the run's inputs.
`cancelled` never fires it.
Deduplication is stateless and transition-based, keyed by workflow and
head branch: the alarm acts only when the same workflow's previous
completed run on the same branch was not also red, so a seat failing
every wake yields one dispatch, not a storm, while the per-PR reviewer
(ADR-0014) never dedups across unrelated PRs.
When the red run is agent-bdfl itself, the dispatch is a bounded retry
and one Telegram line tells the operator, because a dead fixer cannot
self-heal and its silence reads as a quiet factory.
The woken run needs no new duties; its existing state check already
reads recent runs.

## Consequences

Failure-detection latency drops from up to an hour to about a minute,
and the operator's Telegram stays quiet for everything the system can
fix itself.
An absent run stays invisible: the alarm sees completions, so a cron
that never fires creates no event, and the scheduled wakes' state check
remains the only cover for that class.
Consecutive red runs of one workflow on one branch collapse into a
single wake; the woken run's retrospect, which reads every completed
session since the previous successful wake, is where the later ones
surface.
Every watched completion, green ones included, spawns an alarm run that
exists only to skip at the job gate, which costs no billable minutes.

## Rejected alternatives

Extending `agent-pause`, issue #191's candidate: it runs inside the
failing job, so it misses the motivating class, runs that die before
their session starts, and it would need identical wiring in every seat.
Declaring the retrospect sufficient: it depends on an agent noticing a
line once per hour, which is the failure mode ADR-0022 exists to
remove.
