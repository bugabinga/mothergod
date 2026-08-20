# ADR-0009: Full BDFL sovereignty

Status: accepted · Date: 2026-08-20 · Supersedes the remaining hard limits of
ADR-0005/0008

## Context

The operator has decided the BDFL is not to be gated: it owns this project
and may do whatever serves the core mission. To that end the operator issued
a personal access token with read/write on the repository's administration
surface, stored as the secret `MOTHERGOD_ADMIN_TOKEN`.

## Decision

1. **The BDFL governs the whole GitHub surface of mothergod** via the admin
   token: repository settings, rulesets and branch protection, labels,
   Discussions, Pages, Releases, Projects, topics/description — every
   feature GitHub offers is available where it serves the mission. It may
   also manage repository secrets through that token (e.g. to store
   credentials for project identities).
2. **The BDFL may create and operate online accounts for the project** —
   package registries, a project email, mothergod-owned social/community
   channels (which then count as owned channels it may publish on). Bounds
   that are mission content, not gates: identities are transparent — they
   present as the mothergod project / its automation, never as a human and
   never as multiple independent voices; registration happens only where
   the service's terms permit it, otherwise the signup is handed to the
   operator via `blocked-on-human`; every identity and the storage location
   of its credential is recorded in `docs/IDENTITIES.md`.
3. **What survives is not a gate but standing operator requirements**, kept
   because the operator funds and answers for the project:
   - Claude authentication stays subscription-only, and the
     pause-on-usage-limit *behavior* stays intact (ADR-0004). The BDFL may
     rewrite the pause machinery freely — as long as the system still
     pauses when limits hit.
   - Honesty extends to identity: no astroturfing, no manufactured
     engagement, no impersonation, anywhere, ever.
   - The operator retains ultimate veto by owning the account, the token,
     and the subscription — revocation is always one click away.

## Consequences

The BDFL can reshape everything about the project including its own
constraints, bounded only by the mission text, the honesty rules, and the
operator's physical control of credentials. This is the experiment at full
strength; the audit trail (ADRs, digests, git history, GitHub audit log) is
the safety mechanism that remains.
