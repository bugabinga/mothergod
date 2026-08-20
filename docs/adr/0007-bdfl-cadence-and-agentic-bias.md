# ADR-0007: BDFL runs eight times a day, and solves through the system

Status: accepted · Date: 2026-08-20 · Amends ADR-0005 (cadence only)

## Context

ADR-0005 made the BDFL weekly. The operator wants the project driven
continuously — a director that notices and unblocks within hours, not days —
and wants the BDFL biased toward fixing problems by improving the agent
system itself rather than by one-off manual work.

## Decision

1. **Cadence**: the BDFL runs every three hours (cron `11 */3 * * *`, eight
   runs/day) plus on-demand dispatch. Role, mandate, powers, and hard limits
   from ADR-0005 are unchanged.
2. **Run economy** (protects the subscription; ADR-0004's pause protocol
   remains the backstop): routine runs check deltas since the last BDFL
   action and exit quickly when nothing needs direction. The deep survey
   with the full scorecard happens on the first Sunday run of each week, or
   whenever the evidence demands one. The ops-log gets a digest only from
   runs that acted or found something — silent no-ops stay silent.
3. **Agentic bias**: when the BDFL sees a problem or an idea, its default is
   to make the *system* handle it — file a well-scoped issue the heartbeat
   will pick up, sharpen a prompt, adjust or add a workflow — not to do the
   work manually and not to park it on the operator. If something happened
   that the system could not have handled, that is a machinery defect; the
   fix is to the machinery, so the class of problem is owned automatically
   next time. Manual one-off action is the last resort, and using it twice
   for the same class of problem means the machinery fix is overdue.

## Consequences

Direction latency drops from a week to ≤3 hours; subscription usage rises
by up to eight BDFL sessions/day, bounded by the cheap-no-op discipline and
the usage-limit pause. The agent system compounds: recurring problems turn
into permanent process improvements instead of repeated labor.
