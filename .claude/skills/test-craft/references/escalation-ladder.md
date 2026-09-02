# The escalation ladder

Example test, then property, then structured fuzz (matklad's "How to
Test"). Each rung buys wider input coverage at higher authoring and
diagnosis cost. Climb only when the lower rung demonstrably runs out.

1. **Example test.** One concrete input, one asserted output.
   Cheapest to write, cheapest to diagnose, and it documents intent.
   Every bug fix starts here: the failing input, pinned, named after
   the bug.
2. **Property test.** Escalate when you catch yourself writing the
   third example that varies one dimension; the property is that
   dimension. The codec's properties, in descending power:
   - round-trip: `decompress(compress(x)) == x` over a generated
     class;
   - filter invertibility: `unfilt(filt(x)) == x`, on data the filter
     targets and on data it does not;
   - API agreement: the three decode APIs agree on every input, valid
     or invalid;
   - invariant preservation: module-local invariants of the kind the
     journal once had implicit (the rep-symbol/offset-bucket
     disjointness class).
   The property replaces no example: keep the examples as anchors,
   the property as the sweep.
3. **Structured fuzz.** Escalate when the interesting inputs are
   adversarial rather than generated: a property explores what your
   strategy imagines, coverage-guided fuzz finds what nobody
   imagined. The tell: a property that keeps needing a smarter
   generator to reach deep decoder state is a fuzz target with extra
   steps. Fuzz targets live in `fuzz/fuzz_targets/` and run on the
   scheduled tiers per TESTING.md, never in the PR gate.

Stopping rule: before adding a rung, name the failure class it
catches and check that no cheaper rung, and no existing layer in
TESTING.md's list, already catches it. More testing is not a goal; a
caught class is.
