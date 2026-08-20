# ADR-0010: Two-realm repository layout

Status: accepted · Date: 2026-08-20

## Context

Agent-system files (governance, personas, operations manual, identity
registry, reading list) had accumulated across the root and `docs/`,
interleaved with classical project files (test strategy, format spec,
ADRs). The operator wants a clear separation between the software project
and the agent system that builds it.

## Decision

Two realms, one boundary:

- **Classical project** — root community files (README, LICENSE,
  CONTRIBUTING, CHANGELOG, SECURITY, CODE_OF_CONDUCT, ROADMAP), `src/`,
  `docs/` (TESTING, format spec, ADRs), `research/`, `assets/`.
- **Agent system** — `agents/` (GOVERNANCE, OPERATIONS, PERSONALITY,
  SOURCES, IDENTITIES; see `agents/README.md`).

Platform-forced exceptions, documented rather than fought:
`/.github/workflows|actions` (GitHub only executes from there) and
`/CLAUDE.md` (the harness loads it from root) belong to the agent realm
despite their location.

`docs/adr/` stays a single decision series for both realms — one project,
one decision history.

Placement rule for every new file: configures/describes/steers an agent →
`agents/` (or `.github/` if executable); needed by a human contributor to
the compressor → classical tree.

## Consequences

Reference paths changed once (this ADR's PR); pre-existing ADRs retain
their historical paths unedited. Agents get an unambiguous rule instead of
precedent-by-littering; the BDFL enforces it during pruning.
