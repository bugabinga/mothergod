# ADR-0004: Claude subscription auth only; system pauses on usage limits

Status: accepted · Date: 2026-08-20 · Concurrency clause amended by ADR-0014

## Context

The operator funds the agents exclusively through their Claude subscription
(no API billing). Subscription usage is limited per 5-hour rolling window and
per week; agent workflows must not burn the operator's interactive usage nor
fail chaotically when limits hit.

## Decision

1. All agent workflows authenticate with `claude_code_oauth_token:
   ${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}` (created by the operator via
   `claude setup-token`). No `ANTHROPIC_API_KEY` anywhere; adding one
   requires a superseding ADR.
2. **Pause protocol.** The pause state is an open issue labeled
   `agents-paused` containing a line `RESUME-AT: <ISO-8601 UTC>`.
   - Before doing anything, every agent workflow runs the guard
     (`.github/actions/agent-guard`): if a pause issue is open and RESUME-AT
     is in the future → skip the run; if RESUME-AT has passed → close the
     issue (auto-resume) and proceed.
   - After every Claude run, the detector (`.github/actions/agent-pause`)
     scans the execution log for usage-limit markers; on hit it opens the
     pause issue — RESUME-AT now+6h for rolling-window limits, now+24h when
     the message indicates a weekly limit.
   - The operator can pause manually at any time by opening such an issue
     (any body; missing RESUME-AT means "until manually closed") and resume
     by closing it.
3. Frugality rules: one agent session per trigger, bounded `--max-turns`,
   concurrency groups so agent jobs never run in parallel with each other,
   and cron times chosen off the hour.

## Rationale

An issue is durable, visible to the human, auditable, and writable with only
`issues: write` — no repo-settings permissions needed. Auto-resume keeps the
system self-healing; manual pause gives the operator a big red button that
needs no terminal.

## Consequences

A paused system does nothing — including PR review — until resume; queued
work simply waits for the next cron tick after resume. Acceptable: the
project is designed to move slowly.
