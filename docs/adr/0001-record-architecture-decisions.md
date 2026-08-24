# ADR-0001: Record architecture decisions

Status: superseded by ADR-0030 · Date: 2026-08-20

## Decision

Significant decisions are recorded as numbered ADRs in `docs/adr/`, in this
format (context → decision → consequences, one page max). ADRs are immutable;
a change of mind is a new ADR superseding the old one.

For this project ADRs carry extra weight: the dev team is a series of
stateless agent sessions, so *written* decisions are the only decisions that
exist. Anything not in an ADR, `CLAUDE.md`, or `research/JOURNAL.md` will be
forgotten and eventually contradicted.

## What requires an ADR

- Bitstream format changes (with a `FORMAT_VERSION` bump — CLAUDE.md rule 5).
- Dependency additions to the core crate (currently: zero allowed).
- Changes to the agent process model or autonomy level.
- Anything a future session would otherwise plausibly redo differently.

## Consequences

Slight writing overhead per decision; in exchange, agents inherit each
other's reasoning instead of re-deriving it.
