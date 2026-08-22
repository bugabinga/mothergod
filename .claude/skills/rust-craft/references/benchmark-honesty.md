# Benchmark honesty

rustc is aggressive enough to delete the work you are trying to measure.
A benchmark that reports an impossible number is not a fast
implementation; it is a benchmark that did not run.

## The black_box rule

**`black_box` goes on the inputs as well as the outputs.**

```text
loop {
    black_box(f(a, b));      // wrong: inputs stay visible
}
```

With the inputs visible, the compiler evaluates `f(a, b)` once, hoists
it out of the loop, and you are timing `black_box` on a constant.
`black_box(a + b)` becomes `black_box(3)`.

```text
loop {
    black_box(f(black_box(a), black_box(b)));   // right
}
```

The output-only form is the default shape in most examples, which is why
this is so common.

## Checks before you quote a number

- **Is it physically possible?** Sub-nanosecond for real work means it
  did not happen. Compare against a rough cycle budget before believing
  a result.
- **Does the function return something?** A benchmark body returning
  `()` gives the optimizer nothing it must preserve.
- **Release profile?** A debug-build number is meaningless and
  misleading in both directions.
- **Did the speedup survive adding `black_box` to the inputs?** If it
  vanished, it was never real.

## Reporting

CLAUDE.md rule 4: every number names its corpus. "1.9 bits/byte" is not
a result; "1.9 bits/byte on `entropy_ladder(h=2.0)`" is.

State the machine, the profile, and the exact command alongside any
comparison. A benchmark you cannot re-run is a claim, not a measurement,
and the reviewer will treat it as one.

Corpus rules live in `research/corpus/POLICY.md`. Experiment records go
in `research/JOURNAL.md` and `research/progress.jsonl`, per CLAUDE.md
rule 6, including the rejections.
