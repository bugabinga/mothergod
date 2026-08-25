---
name: agent-skill-craft
description: "Use when creating, editing, moving, or reviewing mothergod skills or workflow-prompt procedures under .claude/skills/ and .github/workflows/."
user-invocable: true
---

# Agent skill craft

ADR-0025 owns procedure-versus-reference placement.
ADR-0030 owns information lifetime.
This skill applies both without copying them.

## Procedure

1. Read ADR-0025, ADR-0030, the target workflow prompt, and the existing
   `.claude/skills/` entries near the change.
2. State one trigger and one job.
   Split work whose activation conditions differ.
3. Inventory every candidate prompt sentence as one of:
   - task-conditional procedure;
   - identity;
   - authority or safety;
   - posting or routing;
   - completion constraint.
   Move only task-conditional procedure.
4. Choose the smallest carrier:
   - `description` names concrete activation conditions;
   - `SKILL.md` holds ordered procedure, invariants, and completion;
   - `references/` holds procedure-specific depth behind a named condition;
   - `scripts/` holds repeated deterministic work selected by
     `compile-judgement`, including its liveness signal.
5. Never copy project policy, reference documents, their heading structure, or
   generated indexes into a skill.
   Point to the authoritative source.
6. Delete moved procedure in the same PR.
   Replace it with terse conditional prose containing concrete trigger words
   and a nearby exclusion.
7. Route required scheduled-role procedures explicitly.
   Model discovery may supplement that route; it does not replace it.
8. Keep one skill per PR unless several files are inseparable parts of that
   single routed procedure.

## Validation

1. Validate frontmatter against the current official Agent Skills
   specification and this repository's existing conventions.
2. Check every relative link and referenced project path.
3. Confirm no instruction gained a second authoritative copy.
4. Exercise one harmless activation and one nearby non-activation through each
   wired role.
5. When replacing prompt text, compare completion invariants before and after.
6. Run helper checks and every CLAUDE.md pre-push gate.
7. Record activation evidence, invariant comparison, and removed always-loaded
   context in the PR body.
