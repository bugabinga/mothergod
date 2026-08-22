# ADR-0017: The rust-craft skill, and the code-quality boundary

Status: accepted · Date: 2026-08-22

## Context

Hard rules 1 and 2 are the project's load-bearing promises: lossless is
sacred, and the decoder never panics or overallocates on any input. Both
are currently asserted in prose and checked by hand-written round-trip
tests. Nothing in the tree teaches an agent the Rust-specific shapes
that break them.

A survey of prior art (recorded in `agents/SOURCES.md`, 2026-08-22)
produced five hazards that are specific to writing a compressor in Rust,
that no lint currently catches, and that the existing `deslop` taxonomy
does not cover because it is language-agnostic by construction.

Measured state at the time of writing: `src/` is two files. Across
`src/`, `bench/` and both manifests, `fuzz`, `proptest`, `black_box` and
`forbid` appear zero times. The standard therefore has almost nothing to
apply to yet, which is the argument for writing it now: the codec will
be the large thing, and setting its shape before it lands is cheaper
than desloppping it afterward.

## Decision

A second skill, `.claude/skills/rust-craft/`, with five references:
panic discipline, allocation discipline, type precision, hot loop shape,
benchmark honesty.

Unlike `deslop` it is **not** `disable-model-invocation` and carries no
`paths` restriction. `deslop` is one agent's duty and is scoped to it.
This is a standard, and the maintainer writing codec code, the deslopper
touching it, and the reviewer judging it should all reach it.

**No new agent seat.** The deslopper was added the same day; a standard
does not need a seat to enforce it.

### The boundary, which is the part that will drift

Three artifacts hold adjacent territory:

| Artifact | Question |
|---|---|
| `deslop` | Does this code cost too much to read? |
| `docs/TESTING.md` | What must be tested, at which layer, when? |
| `rust-craft` | Is this Rust, and does it prove its own claims? |

The rule: **`deslop` is language-agnostic and retrospective;
`rust-craft` is Rust-specific and prospective.** A rule that reads the
same in Python belongs in `deslop`. A rule naming a Rust construct
belongs in `rust-craft`. No rule appears in both.

`docs/TESTING.md` owns the test strategy outright. `rust-craft` points
at it and never restates its layers, schedule, or milestones.

### Deliberately not done

The mechanisable half is named in `SKILL.md` and left unimplemented:
`forbid(unsafe_code)` and a decode-path clippy deny list would be
stronger as gates than as prose, because a rule that fails the build
beats one an agent must remember. Changing the gates, and the schedule
for fuzzing in `docs/TESTING.md`, are the system's calls through its own
process, on operator instruction ("leave fuzz and planning to system").

The full Rust API Guidelines checklist is not imported. Two of its sixty
items apply to us (`C-NEWTYPE`, `C-CUSTOM-TYPE`); the rest is written
for public library authors stabilising an API, which we are not. Taking
the whole checklist would be adopting a standard whose cost we have not
earned.

## Consequences

Two code-quality skills instead of one, which is a real cost against the
single-source-of-truth value. The boundary rule above is the mitigation,
and it is stated in three places by necessity (this ADR, `SKILL.md`, and
the skill's own routing table) because a boundary that is not visible
from inside either artifact is not enforced by anything.

The failure mode to watch for: a rule appearing in both skills, or
`rust-craft` growing a test-strategy section that duplicates
`docs/TESTING.md`. Either means the boundary has failed and the skills
should be merged rather than patched.

Second-order risk: the skill is model-invocable and unscoped, so it is
visible to every agent session in the repo. If it starts being consulted
for non-codec work, or cited in reviews of documentation changes, the
description is wrong and needs tightening.
