# Survivor triage

For a surviving mutant (a mutants-check annotation or the monthly
sweep), a torture-sweep abort (#453), or a coverage gap routed by the
weekly map (#454). All three say the same thing: some behavior is
unchecked. Triage decides where the missing check belongs; hard rule
3 forbids every shortcut that makes the number look better instead.

## Surviving mutant

A surviving mutant is a missing test only where the suite makes the
claim the mutation breaks (ADR-0056; TESTING.md "Two classes"). Place
the line first:

- **Decode-safety territory**: `.github/scripts/mutants-debt`'s
  `TERRITORY` regex is the one place that rule lives; read it there; a
  restatement here would drift the moment the regex is corrected
  without this file. A missing suite test. Ask what observable claim
  the mutation broke and which is the cheapest layer that states it,
  then write it there: usually an example through the public API or an
  adversarial seed, rarely a property. `mutants-debt` files these
  after a merge.
- **Scaffold territory**: encoder pricing, model arithmetic, apparatus,
  anything else. A map reading. On an open PR the annotation is for
  the reviewer, who may still ask for a test when the line carries a
  contract the suite should state; after a merge nothing is filed and
  no test is owed. Never answer it with a hand-computed pin on a
  private helper, or a refactor that exposes one to a test: that is
  scaffold held to the suite's standard, the pattern ADR-0056 retires.

Legitimate non-fixes in either territory, each with its evidence on the
record:

- the mutant is equivalent, or undecidable by `cargo test` (the
  `try_new`/`Default` allocation class only torture can see): an
  exclusion in `tests/mutants.toml` with the proof (#391's shape);
- the mutant sits in encoder pricing where any parse is valid output:
  round-trip cannot kill it, the real claim is ratio, and ratio is
  layer 7's job; check the gate cases cover the affected path and say
  so;
- the mutant is a timeout: undecided, not evidence (layer 4 reads
  `missed.txt`, never the exit code).

## Torture abort

An abort under injected allocation failure marks decode growing
memory before validating input, which is hard rule 2's audit. Fix by
validating earlier or bounding the growth (rust-craft's
`allocation-discipline.md` owns the mechanics), and keep the failing
allocation index as a regression case.

## Coverage gap

Coverage feeds triage, never a target. An uncovered region is a
question: is the code unreachable (delete it, the best outcome), or
is a behavior unchecked (the ladder's rung 1)? Never write a test
that merely executes lines; a test without an assertion that would
fail is the manufactured kind ADR-0043 forbids.

## Never

Weaken an assert, loosen an allocation bound, raise
`TOLERANCE_BITS`, cfg-out a test, trim a corpus, or close the
survivor as wontfix without evidence. If the instrument itself is
wrong, that case goes in an issue against the instrument, decided by
someone other than whoever's change it flagged (hard rule 3:
verification stays independent of the proposer).
