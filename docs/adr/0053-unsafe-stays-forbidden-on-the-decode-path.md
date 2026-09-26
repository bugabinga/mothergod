# ADR-0053: `forbid(unsafe_code)` stays absolute on the decode path, conditional elsewhere

Status: accepted · Date: 2026-09-26 · Extends ADR-0017

## Context

ADR-0017 named a crate-wide `#![forbid(unsafe_code)]` as the mechanisable
half of `rust-craft`'s panic and allocation discipline; #135 added it when
`src/` held two files and no `unsafe`, calling it a door closed
"permanently." The operator forwarded an article claiming Rust 1.87 lets
platform intrinsics be called without per-intrinsic `unsafe`. S2-A103/it169
(`research/JOURNAL.md`, 2026-09-26) re-checked it against the tree: the
stabilization moves `unsafe` to one call site per `#[target_feature]`
dispatch boundary, does not remove it, and the crate-wide forbid still
catches that one site. The entry closed with "explicit SIMD stays closed
until ADR-0017 itself is revisited," naming this ADR as the reopening
mechanism without pre-judging its answer.

The operator then asked, by Telegram the same day, to rethink the ban on
its merits: "what is important is a stable product, not dogma." That
deserves a real answer, not a reflex in either direction.

The evidence for lifting it: SPEED is measured under the ROADMAP floor
(`≥1 MB/s` decode single-thread) on several corpus files, by up to an order
of magnitude (`research/JOURNAL.md`, e.g. `~0.14 MB/s` on `xml`). That gap
is real.

The evidence against lifting it for that reason specifically: the same
JOURNAL program (S1-P6) had already redirected its live lever to
S1-A6's block-parallel encode/decode, thread-based and needing no
`unsafe`, before this SIMD re-check ever ran; tANS and explicit SIMD were
independently exhausted at the small-slice level (S2-A88, S2-A90, S2-A80).
No concrete SIMD candidate exists today with a measured win the forbid
is the thing blocking. And the ban is not incidental to ADR-0017: it is
the cheapest, most legible mechanism this project has for MISSION's
non-negotiable 1 ("decoder safe on any input") and hard rule 2 (never
panics or overallocates on adversarial compressed input) — a claim any
reader can verify themselves with `grep -rn unsafe src/`, unlike a fuzz
or Miri corpus that only ever samples the input space.

## Decision

`#![forbid(unsafe_code)]` on the decode path is absolute, not open to
future reconsideration by re-litigating this ADR: it is how the project
keeps MISSION's "decoder safe on any input" mechanically true rather than
merely asserted, against input this project treats as adversarial by
design.

Elsewhere in the crate (encode-only code, never reachable from
`decompress`/`decompress_to_writer`/`decompress_bounded`), the ban is a
strong default, not a taboo. It lifts only for a specific accepted
research candidate (`compression-experiment` skill, a `research/JOURNAL.md`
entry with a measured win no safe alternative reaches), scoped to one
isolated module, `#[allow(unsafe_code)]` with the one-line justification
CLAUDE.md's style section already requires, and covered by the weekly
Miri lane before it ships. No such candidate exists today; nothing in
`src/` changes as part of this ADR.

## Rejected alternatives

**Lift the crate-wide ban now, to unblock SIMD.** Rejected: the JOURNAL's
own trail shows SIMD is not the lever the current SPEED gap needs, so
this trades a verifiable, permanent trust claim for a door that reopens a
path already found exhausted.

**Drop `forbid(unsafe_code)` entirely, rely on Miri and fuzzing.**
Rejected: both sample the input space; the forbid is a total guarantee
for the cost of a build attribute. MISSION's non-negotiable is not
"probably safe."

## Consequences

A future SIMD or intrinsics re-check (the next one, when a new
stabilization lands) has a condition to check against instead of an
open philosophical question: does a concrete candidate exist with a
measured win, and is it encode-only. `research/JOURNAL.md` entries citing
"ADR-0017 revisited" as a blocker should cite this ADR instead.

The decode path's guarantee does not soften. The cost is the one already
paid: no explicit SIMD, ever, in code reachable from decode.
