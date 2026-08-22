# Missing types and docs

Missing types and missing docs are language- and project-dependent slop
signals. Follow project rules first.

## Type specificity

Rust cannot have missing types, but it can have wrong ones: too general
or too specific.

The general consensus in compiled languages:

- use general types for parameters and declarations
- use specific types for values

A parameter typed narrower than the function body needs rejects valid
callers. A value typed wider than it can ever be forces every consumer
to handle cases that cannot occur.

The stronger form: a type that admits illegal states is imprecise. A
`&str` that secretly means one of five subcommands is a bug waiting for
a typo; an enum of five variants says the same thing in less space and
closes every misuse.

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
