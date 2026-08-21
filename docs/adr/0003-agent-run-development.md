# ADR-0003: Fully autonomous agent-run development

Status: accepted · Date: 2026-08-20

## Context

The operator's goal is for this project to be developed and maintained end to
end by AI agents, slowly over time, like a real dev team — with real OSS
artifacts and processes. The operator chose full autonomy (agent review +
green CI suffices to merge) over human-gated merging, and a daily work
cadence.

## Decision

The dev team is a set of Claude sessions launched by GitHub Actions
(`anthropics/claude-code-action@v1`):

| Role | Trigger | Job |
|---|---|---|
| Maintainer heartbeat | daily cron | fix red PRs → triage issues → ship one small PR from ROADMAP/journal |
| Reviewer | every PR | adversarial review; verifies claims by executing them; merges on green CI + passing review |
| Researcher | weekly cron | one experiment per session, per the autoresearch loop contract (propose → guard → benchmark → journal) |
| Interactive | `@claude` mention | Q&A, small on-demand tasks |

(Amended 2026-08-21: the interactive agent was removed by operator
directive; `@claude` mentions no longer trigger anything. Questions go
in issues, triaged by the heartbeat.)

Separation of duties is the safety mechanism replacing human review: the
proposer never merges its own work; the reviewer runs in a separate session
with an adversarial prompt; CI guards are independent of both. Process files
(workflows, CLAUDE.md) are themselves changed only by PR, with high-risk
review rules (GOVERNANCE.md). The human operator retains an absolute veto and
exclusive control of secrets, settings, releases-to-registry, security and
conduct.

## Cadence rationale

Daily heartbeat + weekly research matches "slowly, like a real team", bounds
subscription usage, and keeps every unit of work small enough to review
meaningfully.

## Consequences

Mistakes can merge without a human seeing them; the mitigations are small
PRs, adversarial review, strict CI, the pause mechanism (ADR-0004), and the
operator's revert power. We accept this risk deliberately — measuring how far
this model can go *is the experiment*.
