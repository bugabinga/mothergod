# ADR-0015: BDFL runs hourly

Status: accepted · Date: 2026-08-22 · Amended 2026-08-23 · Amends ADR-0007 (cadence only)

## Context

Operator directive via Telegram, 2026-08-22: "Change bdfl to run hourly."
ADR-0007 set the cadence at every three hours. Everything else in
ADR-0007 (run economy, agentic bias) already assumes most runs are cheap
delta checks that exit silently, so the cadence is the only knob.

## Decision

The BDFL runs hourly: cron `11 * * * *`, twenty-four scheduled runs/day,
plus operator-priority wakes and on-demand dispatch. (Correction,
2026-08-30, PR #368: the value has since moved under ADR-0027's
allowance lever; the cron line in `infra/telegram-worker/wrangler.toml`
is the current cadence.) Run economy and
agentic bias from ADR-0007 are unchanged and matter more at this cadence:
a routine run with no delta exits without posting.

Amended 2026-08-23 (PR #96): the weekly deep survey originally rode the
fixed Sunday 06:11 UTC slot. Dispatch debouncing (ADR-0014) can cancel
any fixed slot silently, and did on 2026-08-23. The survey trigger is
now evidence-based: the first scheduled Sunday wake that finds no
deep-survey digest dated that Sunday in the ops-log runs it; chat wakes
never carry it.

## Consequences

Direction latency drops from ≤3 hours to ≤1 hour. Scheduled BDFL
sessions triple; actual usage rises far less, because no-op delta checks
are cheap and the usage-limit pause (ADR-0004) remains the backstop.
