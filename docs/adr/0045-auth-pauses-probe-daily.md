# ADR-0045: Authentication pauses probe daily instead of waiting on a human

Status: accepted · Date: 2026-09-12 · Extends ADR-0004 · Prompted by issue #517

## Context

ADR-0004's pause protocol is self-healing for usage limits: the pause issue
carries a `RESUME-AT`, and the first run after it closes the issue and resumes.
The authentication branch (HTTP 401/403) was filed without one, on the theory
that a bad token does not heal on a timer, so only the operator closing the
issue could resume the fleet.

That made the operator's issue-close the single exit from an outage, and the
system communicated it as optional ("close to resume earlier"). Issue #517 is
the incident: the operator refreshed the token 72 minutes after the 401 alert
and said so on Telegram, but the Telegram-reading wake was itself paused, and
every run for the next nine days was a green five-second skip. A fixed token
plus a dark factory, with no signal distinguishing it from a healthy quiet one.

Two facts the original theory missed: a 401 probe run fails in seconds and
burns no allowance, and the fix event (a refreshed secret) is invisible to the
system, so only a probe can observe it.

## Decision

An authentication pause carries `RESUME-AT: now+24h` like every other pause.
The existing guard auto-resume then probes daily: the first run after
RESUME-AT closes the issue and proceeds; a still-bad token fails that run,
which refiles the pause for another day and re-alerts Telegram.

The pause alert names the pause issue by URL and states the contract: fix the
`CLAUDE_CODE_OAUTH_TOKEN` secret and the fleet heals itself within a day;
close the issue to probe sooner; delete the RESUME-AT line to stop probing.

## Consequences

- An outage ends at most 24 hours after the operator fixes the token, with no
  GitHub action required of them.
- Each outage day costs one failed probe run and produces one Telegram alert,
  so a dead factory is audible daily instead of silent indefinitely.
- The operator keeps both manual overrides: close early, or remove RESUME-AT
  for an indefinite pause (ADR-0004's manual-pause semantics are unchanged).
- ADR-0004's requirement that the system pauses on usage limits survives
  untouched; this record only removes the human from the auth-recovery loop.
