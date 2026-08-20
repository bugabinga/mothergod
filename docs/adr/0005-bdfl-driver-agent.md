# ADR-0005: BDFL driver agent

Status: accepted · Date: 2026-08-20

## Context

A team of narrow agents (maintainer, reviewer, researcher) can each do their
job and still let the project drift: PRs ping-pong, roadmaps go stale,
processes rot, and nobody holds the authority to say "we're doing this
differently now". Real projects solve this with a maintainer-of-maintainers.
The operator explicitly wants the non-code fabric — workflows, prompts,
scaffolding, docs — to evolve and get more optimal over time, not stay frozen
at day one.

## Decision

Add a weekly **BDFL driver** agent (`agent-bdfl.yml`, Sundays + on-demand)
with a director's mandate: unblock stalled work, prune stale artifacts,
reprioritize the roadmap against evidence, and edit everything non-code —
including the other agents' prompts and workflows — **without approval
ceremony**. It is the sole exception to "never merge your own PR": it merges
its own non-code PRs once the quality gate passes.

Bounded, still:

- **Code stays with the team.** The BDFL directs codec/test/bench work via
  roadmap and issues; it does not ship code itself. The reviewer's
  independence for code PRs is untouched.
- **The money lines are untouchable.** The pause machinery, subscription-only
  auth, secrets handling, and workflow permission levels remain
  blocked-on-human (ADR-0004), even for the BDFL.
- **Bold is not silent.** Every decision that changes others' behavior is
  written where they'll read it; direction changes get ADRs; every run ends
  with a digest on the ops-log issue, including a nag list of
  blocked-on-human items for the operator.

This supersedes the stricter process-change rule in ADR-0003/GOVERNANCE
(previously: all process changes via reviewer with high-risk review) for
non-code changes authored by the BDFL. Process changes from other agents
still go through the reviewer.

## Consequences

The system can now rewrite its own playbook weekly, which is the point — and
the risk. Mitigations: the hard limits above, the written-record rule, the
operator's standing veto, and the fact that a bad BDFL edit to a workflow is
one `git revert` away. The human oversight surface shifts from approving
changes to reading digests.
