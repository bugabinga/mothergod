---
name: adr
description: "Use when creating, superseding, correcting, or reviewing architecture decision records under docs/adr/."
compatibility: "mothergod repository; Claude Code Agent Skills"
user-invocable: true
---

# Architecture decision records

`docs/adr/0030-stable-architecture-decision-records.md` owns ADR policy.
This skill applies it without duplicating it.

## Procedure

1. Read ADR-0030 and the records governing the candidate.
   Follow their supersession chain before acting.
2. Apply ADR-0030's significance test.
   If the candidate fails it, stop and use the artifact that owns the information.
3. Name the decision in one sentence.
   If the sentence contains independently reversible choices, split it.
4. Gather only the context needed to understand the choice.
   Link detailed chronology to its issue, PR, audit, or research entry instead of copying it.
5. Allocate the next number from `docs/adr/` immediately before creating the file.
   Recheck against `main` before push because agents run concurrently.
   Renumber collisions; issue #195 owns making them fail mechanically.
6. Write one compact record with `Context`, `Decision`, and `Consequences`.
   Add `Rejected alternatives` only when a credible contender explains the decision or prevents likely relitigation.
7. For a changed decision, create a new ADR.
   Link both directions in the same change: the replacement names what it supersedes, and the old status points to the replacement.
8. Update the current source of truth affected by the decision in the same PR.
   An ADR explains the choice; it is not runtime configuration.
9. Edit an accepted ADR directly only for non-semantic maintenance or a factual correction.
   A factual correction preserves the prior claim in an inline note and cites its source.

## Review gate

Reject the ADR when any answer is no:

- Does it contain exactly one architecturally significant decision?
- Can a reader understand the decision without reading linked chronology?
- Is detailed evidence linked rather than duplicated?
- Are current values stored in executable config or current documentation?
- Is it free of diary entries, dated addenda, and implementation narration?
- Are supersession links present in both directions?
- Does every factual correction preserve the prior claim and provenance?
- For a format change, does the same PR bump `FORMAT_VERSION` and update `docs/format/SPEC.md`?

After merge, the decision, rationale, and consequences are history.
Changing them requires a superseding ADR.
