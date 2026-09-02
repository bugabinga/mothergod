---
name: rust-craft
description: mothergod's Rust standard for codec code. Five hazards specific to writing a compressor in Rust - where a panic is legitimate and where it violates hard rule 2, how to bound allocation against a hostile length field, using types so offsets and lengths and symbol ids cannot be silently swapped, where a branch sits relative to a hot loop, and why a benchmark that skips black_box on its inputs reports a number for code that never ran. Consult while writing or reviewing Rust in src/. Not a slop taxonomy - that is the deslop skill.
user-invocable: true
---

# Rust craft

The standard for Rust in this repo. Prospective: consult it while
writing code and while judging a diff, not as a cleanup pass.

## Boundary

Four artifacts have adjacent territory. They do not overlap, and
keeping that true is a standing duty, not a one-time check.

| Artifact | Question it answers |
|---|---|
| `deslop` skill | Does this code cost too much to read? |
| `docs/TESTING.md` | What must be tested, at which layer, when? |
| `test-craft` skill | Is this one test written and triaged well? |
| this skill | Is this Rust, and does it prove its own claims? |

The line: **`deslop` is language-agnostic and retrospective; this is
Rust-specific and prospective.** A rule that reads the same in Python
belongs in `deslop`. A rule naming a Rust construct belongs here. No
rule lives in both places.

`docs/TESTING.md` owns the test strategy outright. Point at it. Never
restate its layers, its schedule, or its milestones, here or in a PR
body: a second copy of a plan is a plan that will drift.

## The five

| Reference | Hazard |
|---|---|
| `panic-discipline.md` | which panics violate hard rule 2 and which are legitimate |
| `allocation-discipline.md` | the compression-bomb class: capacity from a hostile length |
| `type-precision.md` | offsets, lengths and symbol ids are all `usize` and all confusable |
| `hot-loop-shape.md` | push ifs up, push fors down |
| `benchmark-honesty.md` | `black_box` on inputs, not just outputs |

Read the ones the code in front of you triggers. Do not read all five
by reflex.

Rules mechanisable as lints get promoted to build gates and removed
from this prose; issue #76 tracks the current batch.
