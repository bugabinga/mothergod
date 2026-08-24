# ADR-0031: Affordable model choice belongs to the BDFL

Status: accepted · Date: 2026-08-24 · Supersedes ADR-0012 · Prompted by issue #197

## Context

ADR-0012 made the BDFL responsible for model choice but imposed an operator-controlled floor on the BDFL's own model.
During the first measured allowance incident, that floor left the only remaining model-tier lever outside the role responsible for keeping the agent system available.

The operator clarified in issue #197 that the floor was unintended and granted the BDFL full authority over `agents/models.json`.
The incident's readings, temporary tier change, and restoration discussion belong to that issue.
The durable decision is the authority and objective behind model choice.

## Decision

The BDFL owns model and effort choices for every agent, including itself.
It may change any ladder or floor in `agents/models.json` without an operator approval step.

The BDFL runs the most capable model the project can afford.
Capability and affordability are both binding: a stronger model that exhausts the shared allowance and leaves the factory unavailable is not affordable.
Other roles use the model and effort appropriate to their work, judged by published capability evidence and observed project economics.

`agents/models.json` records current choices.
Audit artifacts and incident issues record transient allowance readings.
Issue #202 owns automating tier changes from allowance headroom.
None of those operational states amend this decision.

## Consequences

Model choice has one owner and no authority dead-end during an allowance incident.
The BDFL may lower its own capability when availability is the more valuable constraint, then raise it when headroom returns.

The original quiet-decay risk remains: the role judging its own capability may choose badly.
Model-intel evidence and run economics make that choice reviewable, while repository history records every configuration change.

No model identifier, tier, effort setting, or restoration threshold is frozen in this ADR.
Those values can change without turning a temporary operating condition into architecture.
