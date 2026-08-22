---
name: deslop
description: Find and remove slop in mothergod's Rust source. Slop is code that works but costs more to read, change, and trust than it should - accidental complexity, duplication, single-use indirection, long functions, deep nesting, magic values, error handling that is too thin or too thick, legacy overengineering, imprecise types, and comments that restate the code. Invoked explicitly as /deslop by the deslopper agent; not for model auto-invocation.
disable-model-invocation: true
user-invocable: true
---

# Deslop

Slop is code that works and costs too much: to read, to change, to
trust. It is not a bug. Nothing here is about correctness, and nothing
here licenses a behaviour change.

The taxonomy lives in `references/`. Read only the ones the code in
front of you actually triggers.

| Reference | Signal |
|---|---|
| `complexity.md` | accidental complexity, abstraction that earns nothing |
| `duplication.md` | the same knowledge in two places |
| `poor-patterns.md` | patterns applied by reflex, not by need |
| `single-use-functions.md` | indirection with exactly one caller |
| `long-functions.md` | functions doing several jobs at once |
| `deep-nesting.md` | control flow the reader has to hold on a stack |
| `magic-values.md` | literals whose meaning, unit, or relationship is hidden |
| `poor-error-handling.md` | errors swallowed, stringified, or lost |
| `overengineered-error-handling.md` | error machinery larger than the errors |
| `legacy-overengineering.md` | structure built for a future that never came |
| `missing-types-docs.md` | types too general or too specific; comments that restate code |

## Scope

One PR is one scope. A scope is either a **place** or a **seam**. Never
both in one PR.

**Place-scoped:** one region, every defect inside it.
**Seam-scoped:** one cross-cutting concern, every site it touches, and
nothing else at those sites.

Scope inversely with blast radius. A function many callers depend on, or
on a hot path, or carrying an invariant: that function alone, against
every rule. A module with few callers and no invariant of its own: the
whole module.

Operational test: can you enumerate every caller and every test that must
still pass? If not, shrink.

In this repo the decoder is maximum blast radius; codec internals default
to one function.

**Lower bound:** the change must be worth one review cycle.
**Upper bound:** never every defect everywhere.

**File count is not the measure.** A seam followed end to end may touch
twenty files and still be one scope. Never split a coherent scope to make
a diff look smaller: a half-followed seam leaves the codebase speaking two
idioms at once, which is worse than either idiom alone.

## Procedure

1. Pick the scope. State it in one sentence before you touch anything.
   If you cannot state it in one sentence, it is not one scope.
2. Read the scope in full, plus every caller. Read the references the
   code actually triggers.
3. Establish the baseline: run the CLAUDE.md quality gates before you
   edit. A gate that was already red is not yours to hide.
4. Change. Preserve behaviour exactly. Deletion beats addition: the
   best fix removes the code rather than improving it.
5. Prove it. Every change must be provably behaviour-preserving. Where
   the existing tests do not prove it, add the test that does, before
   the change, and show it passing on both sides. A change you cannot
   prove preserves behaviour does not ship.
6. Run every gate again. Green, or the change does not leave the branch.

## Hard limits

- Never change observable behaviour. Not the bitstream, not the API,
  not an error's meaning, not a panic into a `Result` or back.
- Never touch a test to make it pass. If a test breaks, the change was
  wrong. Tests are the measurement; you do not adjust the ruler.
- CLAUDE.md's hard rules outrank everything here. In particular: the
  decoder never panics on any input, and comments that record invariants
  stay, however redundant they look.
- Read the project's lifecycle from `ROADMAP.md`, `FORMAT_VERSION`, and
  `CHANGELOG.md`'s `[Unreleased]` section before arguing that a
  compatibility shim is or is not needed. Do not assume.
- One PR, one scope. If you find a second scope, note it in the PR body
  and leave it.
