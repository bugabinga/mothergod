# ADR-0008: BDFL owns agent permission envelopes; BDFL runs at maximum

Status: accepted · Date: 2026-08-20 · Amends the hard limits of ADR-0005/0007

## Context

The hard limits so far froze all workflow permissions as blocked-on-human.
The operator has decided the BDFL should govern what the other agents may
do — their GitHub `permissions:` blocks and Claude tool allowlists — and
that the BDFL itself needs maximum permissions and open network to do its
job without artificial friction.

## Decision

1. **The BDFL decides permission envelopes for every agent** (heartbeat,
   reviewer, researcher, interactive): workflow `permissions:` blocks,
   `--allowedTools` lists, `--max-turns`, and equivalents. Changes follow
   the usual BDFL rules — written record, behavioral diff stated, shipped
   by PR. Least privilege per role remains the design principle the BDFL
   applies (e.g. the reviewer staying write-less is current policy the BDFL
   may revisit, on the record).
2. **The BDFL itself runs at maximum**: its job requests the widest
   GITHUB_TOKEN permission set a workflow can hold, and its Claude session
   runs with an unrestricted tool allowlist — full shell, all file tools,
   web search/fetch, open runner network. This is a grant of trust, not an
   invitation to bypass the division of labor: the BDFL still ships code
   only through the maintainer/reviewer pipeline.
3. **Still operator-only, for every agent including the BDFL**: the
   usage-limit pause machinery (agent-guard/agent-pause and their wiring),
   subscription-only authentication (no other Anthropic credentials), and
   secrets handling. Permission changes by any agent *other than* the BDFL
   still escalate — now to the BDFL rather than to the operator.

## Consequences

The system can retune its own privilege boundaries at BDFL speed; the
operator's oversight narrows to money, credentials, and the pause switch.
Risk is bounded by the ephemeral CI runner, the reviewer pipeline for code,
the written-record rule, and the operator's standing veto/revert power.
