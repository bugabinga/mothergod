# ADR-0014: one concurrency group per scheduled agent

Status: accepted · Date: 2026-08-22 · Proposed by the BDFL (follow-up to PR #30)

## Context

ADR-0004 serialized all agent jobs behind one `agents-work` concurrency
group as a frugality rule. Two facts have since eroded it. The reviewer
already runs in its own per-PR group, so agents run in parallel today;
the rule guards a state that no longer exists. And PR #30 quadrupled the
heartbeat cadence, which quadruples the chance that an operator-priority
BDFL wake queues behind a running heartbeat instead of starting within
seconds, the guarantee the wake trigger was added to provide. The
researcher's Saturday run sits in the same group with a 300-minute
timeout, the worst single blocker.

Serialization also buys nothing it claims to: it does not reduce token
spend, it only delays it, and usage-limit hits are owned by the pause
machinery (ADR-0004's actual mechanism), not by queueing.

## Decision

Each scheduled agent gets its own concurrency group, named after its
workflow: `agent-bdfl`, `agent-heartbeat`, `agent-research`, all with
`cancel-in-progress: false`. The BDFL's burst debounce (one running plus
one pending) is preserved; it never depended on sharing.

This amends ADR-0004's clause "concurrency groups so agent jobs never
run in parallel with each other" to "concurrency groups so no agent runs
in parallel with itself". The rest of ADR-0004 stands.

## Consequences

An operator wake starts within seconds regardless of what the other
agents are doing. Agents can now run concurrently; their write surfaces
are disjoint by design (separate branches, per-agent repo variables),
and the reviewer plus squash-only history absorb any PR-level collision.
If concurrent sessions ever produce a real conflict class, that is new
evidence and this decision gets revisited with it on the record.
