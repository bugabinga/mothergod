# ADR-0030: Stable architecture decision records

Status: accepted · Date: 2026-08-24 · Supersedes ADR-0001 · Prompted by issue #204

## Context

Architecture decisions carry unusual weight in this project because the team is a series of stateless agent sessions.
A later session must recover the forces behind a decision without blindly preserving or reversing it.

ADR-0001 established numbered, immutable records, but left two gaps.
Its catch-all admitted any choice a future session might reconsider, rather than only architecturally significant decisions.
It also said nothing about where later measurements, incidents, recovery notes, and changed decisions belong.
Several records consequently absorbed operational diaries and amendments.
The decision remained present, but became harder to distinguish from events that happened afterward.

## Decision

The ADR series is an append-only collection of stable records.
An accepted ADR captures one architecturally significant decision: one affecting the project's structure, non-functional characteristics, dependencies, interfaces, or construction techniques.
The same test applies to the compressor and the agent system.
Ordinary implementation choices remain in their issue or PR.

ADRs use sequential, monotonically increasing numbers that are never reused.
A PR is the proposal; merge means accepted.
The status of a merged ADR is therefore either `accepted` or `superseded by ADR-NNNN`.

Each ADR has a short noun-phrase title and these parts:

- **Context** states the forces in tension, factually and without arguing the verdict.
  A decision-driving measurement may appear with its source, but an incident timeline may not.
- **Decision** states the project's response in present-tense, active prose.
- **Consequences** states the resulting context, including costs.
- **Rejected alternatives** is optional.
  It exists only when a credible contender helps explain the decision.

One or two pages is an editorial bias toward terseness, clarity, and simplicity, not a validity ceiling.
A long draft is a signal to look for multiple decisions, diary material, or prose that has not been compressed.

Accepted ADRs are stable, not untouchable.
These later edits are legitimate:

- a supersession status and pointer;
- non-semantic maintenance such as typo, formatting, and broken-link fixes;
- a factual correction that preserves the prior claim in an inline correction note and cites its provenance.

A change to the decision, its rationale, or its consequences requires a new ADR.
Supersession links both directions: the replacement names what it supersedes, and the replaced record points to the replacement.

Information lives according to its lifetime:

| Information | Home |
|---|---|
| Binding rule needed every session | `CLAUDE.md`, persona, or the smallest role prompt |
| Conditional procedure | Skill, loaded when its trigger applies |
| Current recovery recipe | Runbook or executable script |
| Raw incident timeline and discussion | Dedicated GitHub issue or PR |
| Cross-run status digest and index | Ops-log issue |
| Machine-produced run evidence | Audit artifact |
| Durable architectural decision | ADR |
| Research hypothesis, measurement, and verdict | Research journal |

An ADR links to evidence in those homes and includes only enough context to make the decision understandable.
It does not duplicate their chronology.

## Consequences

A future session can treat an accepted ADR as a stable account of one decision without replaying operational history.
Changed decisions remain visible as new records, so the series shows both what governed the project and what governs it now.

The significance test excludes many choices that ADR-0001's catch-all admitted.
Their rationale is not lost: issues and PRs remain the record for ordinary work, while ADRs retain the decisions that shape later work.

Stability permits maintenance rather than demanding ritual immutability.
That leaves judgement at the correction boundary.
The `adr` skill makes the boundary an explicit authoring and review step; review remains responsible for rejecting a disguised change of decision.

As a one-time migration, ADR-0012's amendment becomes a superseding decision and ADR-0027's operational readings move to their source incident, issue #197.
Existing ADR prose otherwise remains historical evidence rather than being rewritten to match this policy retroactively.
