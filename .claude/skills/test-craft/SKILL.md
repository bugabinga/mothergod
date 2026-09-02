---
name: test-craft
description: The operating manual for mothergod's testing portfolio (ADR-0043). Conditional procedure for writing and triaging tests - when an example test should escalate to a property or a fuzz target, how to build a proptest strategy for codec types that shrinks well, how a fuzz crasher becomes a tests/adversarial/ seed, and how to answer a surviving mutant, a torture abort, or a coverage gap without weakening a guard. Use when writing or reviewing tests, fuzz targets, or property strategies, or when acting on a fuzz, mutation, torture, or coverage finding. Not the strategy - docs/TESTING.md owns what runs when. Not Rust hazards - that is rust-craft.
user-invocable: true
---

# Test craft

How to write and triage one test well. Prospective, like rust-craft:
consult it while writing, not as a cleanup pass.

## Boundary

| Artifact | Question it answers |
|---|---|
| `docs/TESTING.md` | What must be tested, at which layer, when? |
| `rust-craft` skill | Is this Rust, and does it prove its own claims? |
| this skill | Is this one test written and triaged well? |

`docs/TESTING.md` owns the layers, the tiers, and the cadence. Point
at it, never restate it. Before writing any test, name the claim it
proves and check which layer already proves it: a test that re-proves
a cheaper layer's claim is noise with a maintenance bill.

## Invariants (every reference assumes these)

- Hard rule 3 binds all triage: a red instrument is answered by
  strengthening code or tests, never by weakening a guard, assert,
  bound, or corpus. An instrument you believe is wrong gets an issue
  making that case; it does not get quietly loosened.
- The required gate stays deterministic: seeded generators, fixed
  cases. Statistical exploration lives in the scheduled tiers
  (TESTING.md's tier table).
- Ledger numbers are maps, never targets (ADR-0043). A test whose
  only effect is moving coverage or mutation score is a manufactured
  assertion; do not write it.

## The four

| Reference | When |
|---|---|
| `references/escalation-ladder.md` | adding a test for new behavior or a found bug |
| `references/proptest-strategies.md` | writing or changing a property test or its strategy |
| `references/crasher-promotion.md` | a fuzz run or torture sweep found a crasher |
| `references/survivor-triage.md` | a surviving mutant, torture abort, or coverage gap needs an answer |

Read the one the work in front of you triggers, not all four.
