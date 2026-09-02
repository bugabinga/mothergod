# Survivor triage

For a surviving mutant (a mutants-check annotation or the monthly
sweep), a torture-sweep abort (#453), or a coverage gap routed by the
weekly map (#454). All three say the same thing: some behavior is
unchecked. Triage decides where the missing check belongs; hard rule
3 forbids every shortcut that makes the number look better instead.

## Surviving mutant

A surviving mutant is a missing test by definition (TESTING.md layer
4). Ask: what observable claim did the mutation break, and which is
the cheapest layer that states that claim? Write the test there; it
is usually an example test, not a property.

Legitimate non-fixes, each with its evidence on the record:

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
