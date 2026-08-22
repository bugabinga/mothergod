# ADR-0015: BDFL runs hourly

Status: accepted · Date: 2026-08-22 · Amends ADR-0007 (cadence only)

## Context

Operator directive via Telegram, 2026-08-22: "Change bdfl to run hourly."
ADR-0007 set the cadence at every three hours. Everything else in
ADR-0007 (run economy, agentic bias) already assumes most runs are cheap
delta checks that exit silently, so the cadence is the only knob.

## Decision

The BDFL runs hourly: cron `11 * * * *`, twenty-four scheduled runs/day,
plus operator-priority wakes and on-demand dispatch. The Sunday 06:11 UTC
run remains the weekly deep survey. Run economy and agentic bias from
ADR-0007 are unchanged and matter more at this cadence: a routine run
with no delta exits without posting.

## Consequences

Direction latency drops from ≤3 hours to ≤1 hour. Scheduled BDFL
sessions triple; actual usage rises far less, because no-op delta checks
are cheap and the usage-limit pause (ADR-0004) remains the backstop.
