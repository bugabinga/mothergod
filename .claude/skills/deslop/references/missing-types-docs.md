# Missing types and docs

Missing types and missing docs are language- and project-dependent slop
signals. Follow project rules first.

## Type specificity

Rust cannot have missing types, but it can have wrong ones: too general
or too specific. The signal is yours to flag; the rules are
Rust-specific, so they live in the rust-craft skill,
`references/type-precision.md`: newtypes where confusion would be
silent, enums instead of stringly values, wide parameters and narrow
values, range encoded in the type. Judge the finding there, not here.

## Missing docs or comments

Complex, surprising, or side-effect-heavy code almost always deserves a
comment or doccomment.

## Good comments

Good comments explain context, reason, motivation. They explain why the
code exists or why it is shaped this way. Invariants the code cannot
show are the highest-value case: a constraint that lives only in one
function's head is a bug waiting for a second implementation.

## Bad comments

Never simply translate what the code does into human language. A comment
that only restates the code is noise.

## Pseudo-code examples

### Bad: comment only repeats the code

```text
# Add one to retry_count.
retry_count = retry_count + 1
```

This only translates the code into human language.

### Better: comment explains reason or context

```text
# The first retry happens immediately because the remote cache often becomes
# visible one tick after the write succeeds.
retry_count = retry_count + 1
```

This explains the reason and context.

### Worth documenting: surprising side effect

```text
function calculate_total(order):
    # Also marks expired discounts as used because the billing system expects
    # discount state to be finalized during total calculation.
    expire_old_discounts(order)
    return sum order lines
```

Complex, surprising, or side-effect-heavy code almost always deserves a
comment or doccomment.
