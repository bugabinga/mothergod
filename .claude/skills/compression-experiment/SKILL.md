---
name: compression-experiment
description: "Use when the mothergod researcher chooses, runs, or records one compression experiment, including a prerequisite codec or benchmark capability patch needed to measure it."
user-invocable: true
---

# Compression experiment

`research/JOURNAL.md` owns experimental memory and standing leads.
`research/README.md` owns the progress schema.
`research/corpus/POLICY.md` owns corpus separation, acceptance, and quoted
number provenance.
This skill coordinates one run without copying those rules.

## Preflight

1. Read `research/JOURNAL.md` in full, then `research/README.md`,
   `research/corpus/POLICY.md`, and `ROADMAP.md`.
2. Inspect open PRs and `claude/*` branches.
   Do not duplicate work already in flight.
3. Confirm the codec and benchmark harness can measure the next candidate.

## Capability route

When measurement is not yet possible:

1. Select the smallest unclaimed codec or benchmark capability patch needed
   for the next experiment.
2. Implement it and prove the capability with focused tests.
3. Run the project quality gates.
4. Record `kind="patch"` through the journal and progress schema.
   Use null deltas only under `research/README.md`'s capability-patch rule.
5. Stop after the capability verdict is recorded.

## Experiment route

When measurement is possible:

1. Select exactly one candidate, in order:
   - the journal's top standing lead;
   - one implementable literature idea with its source;
   - one cheap wild swing.
2. State one falsifiable hypothesis naming the expected bits-per-byte effect
   and target data class.
3. Implement the candidate without changing its benchmark, corpus, or
   acceptance guard.
4. Apply the required round-trip and adversarial guards, then measure through
   the corpus policy's train and sealed-validation procedure.
5. Apply the corpus policy's verdict and number-provenance rules unchanged.
6. Record the mechanism and verdict in both research records.
   Keep accepted code and its focused tests; delete rejected candidate code.
7. Run the project quality gates and leave exactly the recorded PR scope.

## Environmental failure

Return the exact blocker without inventing a verdict or measurement.
The researcher prompt owns where that operational result is posted.
