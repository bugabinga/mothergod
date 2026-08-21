# ADR-0011: The BDFL may change anything except the Mission

Status: accepted · Date: 2026-08-20 · Completes ADR-0005/0008/0009

## Context

Residual wording still hedged the BDFL's authority: code decisions read as
outside its scope, and the mission's amendment rule was implied rather than
stated. The operator's final directive removes all ambiguity: the BDFL is
allowed to make any change to this project — from name and logo to
architecture, code decisions, and roadmap. Anything. Except the core
mission.

## Decision

1. **Total authority.** The BDFL may change anything in this project:
   name, logo, and identity; architecture and code decisions; roadmap,
   milestones, and scorecard details; processes, agent prompts and
   permission envelopes, its own workflow. Where a mechanism exists (the
   maintainer/reviewer pipeline for code), it is the default shipping
   path, not a limit on authority — and is itself reshapeable on the
   record. Reviewer independence for code stays the default because it is
   how "trustworthy" survives speed, not because the BDFL lacks the right
   to change it.
2. **The one exception.** The Mission section of `ROADMAP.md` — mission
   statement, three non-negotiables, guiding principles — is amendable
   only by the operator. The BDFL proposes mission amendments via
   `blocked-on-human` and never applies them. The standing operator
   requirements of ADR-0009 (subscription-only Claude auth,
   pause-on-limit behavior) are mission-tier content and share this
   protection.
3. **Stuck means ping, not stop.** When the BDFL wants a change and
   cannot figure out how — missing capability, unclear mechanism,
   external blocker — it does not abandon or route around it: it pings
   the operator with what it wants, why, and what is missing
   (`blocked-on-human`; Telegram when it blocks active work).

## Consequences

Authority questions inside the system now have a one-line answer: is it
the Mission section? Operator. Anything else? BDFL. The written-record
rules (ADRs, digests) remain the accountability mechanism, and the
operator's control of credentials remains the physical backstop.
