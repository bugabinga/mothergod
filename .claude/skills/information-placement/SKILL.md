---
name: information-placement
description: "Use when placing or reviewing mothergod agent guidance, procedures, decisions, operational history, or work across prompts, skills, ADRs, runbooks, issues, the ops log, audit artifacts, and research records."
user-invocable: true
---

# Information placement

ADR-0030 owns information lifetime and placement.
ADR-0025 distinguishes conditional procedure from reference material.
This skill applies both without copying them.

## Procedure

1. Inspect the candidate target, adjacent sources, and intended reader.
   Name the information and the future action it should change.
2. Search for its current authoritative statement.
   Update or point to that source instead of appending a second version.
3. Split content with different lifetimes or audiences into separate outputs.
   Route each output through ADR-0030's information model.
4. Distill the smallest content that changes the intended reader's action.
   Keep detailed evidence and chronology in their owning record, then link it.
5. Put optional depth behind a concrete trigger.
   Long examples and chronology do not enter always-loaded prompts.
6. When the same manual question or step appears more than twice, use
   `compile-judgement`.
   Compile it only when that skill's hotness, ownership, substrate, and
   liveness tests all pass.
7. Preserve work ownership.
   Scoped work lives in a routed issue; mission, milestones, scorecard, and
   priority state live in `ROADMAP.md`.

## Completion gate

- One current authoritative home exists.
- Every other mention is a pointer or generated view.
- The intended reader receives the information only when useful.
- Historical evidence remains discoverable.
- Current behavior is not encoded only in history.
