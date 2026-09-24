# ADR-0051: The curator's bootstrap duty retires

Status: accepted · Date: 2026-09-24 · Amends ADR-0044 (the bootstrap charter item) · Prompted by issue #486 (fifth prompt audit, PR #726)

## Context

ADR-0044 gave the curator four charter items. The first, bootstrap,
made every run ensure ten labels and the ops-log issue exist, a block
moved verbatim from the heartbeat prompt that had already created
them. The fifth #486 audit (PR #726) checked the block against that
issue's checklist: all ten labels exist and predate the seat, and
issue #3 has been open since the repository's first week, so the
trigger never fired in this file. The label list was a second copy of
CLAUDE.md's "Labels agents maintain" and had drifted from it. The
invariant it guarded has mechanisms already: `gh-comment --new
--label` dies on a label GitHub does not know, and `survey-due`
addresses the ops log as issue #3 by number, which takes comments
open or closed.

## Decision

The bootstrap item leaves the curator's charter. Labels and the
ops-log issue are created repository state, not a standing duty of
any seat. ADR-0044's other three items, triage, grooming and
adversarial critique, stand unchanged. GOVERNANCE.md "Roles" carries
the current charter.

## Consequences

- The curator's prompt no longer opens on a no-op; its pin in
  `.github/scripts/prompt-bytes` drops with it.
- A deleted label surfaces as a failed `gh-comment --new --label` in
  the seat that needed it, one loud failure instead of a silent
  nightly `|| true`. Re-creating the label is the fix, by whoever
  meets the failure.
- No seat re-creates the ops log, so the ledger stays one issue, #3,
  and the machinery keeps addressing it by number.
