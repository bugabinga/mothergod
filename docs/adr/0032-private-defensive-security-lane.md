# ADR-0032: Private defensive-security lane keeps operator triage

Status: accepted · Date: 2026-08-24 · Prompted by issue #233

## Context

Issue #205 proposed a `defensive-security` skill and an autonomous agent
lifecycle: scan, draft a private advisory, confirm or reject it, remediate,
get independent review, release, and publish or request a CVE, all without
a human step.

That lifecycle assumes infrastructure and authority this project does not
have. `SECURITY.md` routes undisclosed reports around the agent system
entirely. `agents/GOVERNANCE.md`'s decision table reserves
"Security-report triage" to the operator alone, no carve-out. No security
persona, model-ladder entry, credential, or workflow exists. The BDFL's
admin token (`MOTHERGOD_ADMIN_TOKEN`) is single-seat by rule already.
Installing skill procedure ahead of these decisions would create a
public-looking private lifecycle with no authority, credentials, or
separation behind it, which is worse than having none.

Issue #233 asked for the architecture before any implementation PR: who
decides what stays private, what the agent may touch, how proposer and
reviewer stay independent, which credential does which job, and what a
human sees without seeing the vulnerability.

## Decision

A defensive-security agent may exist, but it prepares, it does not decide.
The operator keeps every judgment call `SECURITY.md` and `GOVERNANCE.md`
already give them; this ADR adds one narrow power on top, drafting and
remediating a candidate in private, and nothing else changes hands.

**Authority.** Confirming a candidate is real, publishing an advisory,
requesting a CVE, crediting a reporter, and triggering the release remain
the operator's alone, unchanged from `SECURITY.md` and the governance
table. The one new power: a BDFL-directed security agent may privately
draft a candidate advisory and prepare a fix in that advisory's temporary
private fork. A draft is a proposal with no effect until the operator
confirms it, so it is not triage.

**Scope.** `SECURITY.md`'s threat model is unchanged: decoder panics,
overallocation, round-trip violations, and cross-platform nondeterminism
on attacker-controlled input, i.e. `src/`. Workflows, webhooks,
credentials, dependency CVEs, and configuration stay out of this agent's
scope; they are owned today by CodeQL, the reviewer, and ordinary BDFL
tool-envelope stewardship (`agents/GOVERNANCE.md` "Tool envelopes").
Widening scope is a separate decision with its own ADR, not a default
this one grants.

**Private execution.** Ordinary scanning (existing public tests and
fuzzers against public code) is ordinary CI and stays public. The instant
a run produces a plausible candidate, every following command is
candidate-specific and none of its detail may reach a public issue, PR,
branch, commit, workflow log, ops-log comment, or Telegram message.
GitHub Security Advisories and their temporary private forks are the only
venue; they already exist for this purpose and need no new infrastructure.

**State.** The draft advisory is the index; no parallel private database.
The agent keeps phase, evidence links, assessed revision, exact private
head, author, reviewer, verdict, and retry count as a structured comment
pinned in the advisory thread, because advisories carry no custom fields
of their own. Retention ends when the advisory closes, published or
rejected, both operator decisions.

**Separation.** Mirrors the public reviewer/maintainer split
(`agents/GOVERNANCE.md` "Roles"): one session performs one phase. A
scanner phase drafts and never confirms its own candidate. A remediation
phase fixes and never reviews its own fix. An independent reviewer phase
approves the fix against its exact private head. Release consumes that
exact head, not a rebuilt one. None of these run in the same session or
the same run as the phase before it, the same non-negotiable the public
reviewer already carries.

**Credentials.** A new fine-grained PAT scoped to security-advisory
read/write and the private-fork remote only, distinct from `claude[bot]`'s
app token and never `MOTHERGOD_ADMIN_TOKEN` (already forbidden by
existing rule; reaffirmed, not new). Candidate code execution (building
and testing the candidate) runs with no GitHub-write credential in its
environment at all, so a malicious candidate input has nothing to exfiltrate
through.

**Release and disclosure ordering.** Confirm (operator) → remediate in
the private fork → independent private review PASS on the exact head →
merge to the fork's default → patched release cut, still agent-prepared
and operator-triggered per the existing release rule → publish the
advisory → request a CVE if warranted. A fix is never visible publicly
before a usable release exists. A failed review returns to remediation,
still private. A rejected candidate closes the advisory, operator-recorded,
still private unless the operator chooses to disclose it.

**Observability.** No candidate detail is ever public. The one signal is
a private count, "N candidates open, oldest age X", the BDFL reads via the
Advisories API and folds into its normal digest, no title, no CWE, no
file path, no branch name.

The `defensive-security` skill issue #205 named may now be proposed against
this architecture: identity, authority, confidentiality, untrusted-evidence
handling, target boundaries, separation of duties, posting rules, and stop
conditions, wired to the concrete triggers this ADR defines.

## Consequences

The operator's per-report judgment stays exactly where `SECURITY.md`
already put it; nothing about undisclosed-report handling gets weaker.
The agent system gains a bounded, auditable-in-private preparation lane
instead of an autonomous confirm/reject/publish pipeline this project has
no infrastructure or track record to run safely yet.

A real vulnerability now takes longer to reach a release than a fully
autonomous pipeline would, because the operator's confirmation is a
synchronous step. That cost is deliberate: the mission's "trustworthy"
survives the BDFL's speed by keeping a human in the loop for the one
judgment call, confirm or reject, that determines whether a fix branch
exists at all.

No new credential, workflow, or role exists yet. Implementing this
architecture (the security persona, model-ladder entry, PAT, and
workflow) is follow-up work the BDFL schedules through the agent-system
queue, and the `defensive-security` skill from issue #205 is scoped
against this record rather than the autonomous lifecycle it originally
proposed.
