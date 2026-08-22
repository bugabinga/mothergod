# Hot loop shape

Two rules that compose. Both are about where a decision lives relative
to a loop.

## Push ifs up

Move conditionals toward the caller. Branch on mode, method, or
configuration **once**, outside the loop, not once per symbol.

Two payoffs, and the first matters more:

- **Fewer states.** With the branching in one place you can see the
  redundant conditions and the dead ones. A function called a million
  times with a flag that never changes has a state space it does not
  need.
- **Fewer checks.** Enforcing a precondition at the call site removes
  the repeated check from the interior. The habit is viral in a good
  way: pushed far enough, the check reaches the API edge and the
  interior becomes total.

## Push fors down

Make the batch operation the primitive, not the scalar one.

```text
encode_block(&[u8])        // the primitive
encode_byte(u8)            // not this, called in a loop by the caller
```

Batching amortises setup, lets the implementation reorder work, and is
the precondition for any vectorisation. A scalar API forecloses all
three, permanently, because callers will have written the loop.

In this repo the filters, the match search, and the coder all process
runs of bytes. Each should take a slice.

## Together

Hoisting the branch out of the loop is what makes the loop body
uniform enough to batch. That is the whole trick.

## The caveat that matters here

This is a **shape** rule, not a performance claim. Shape it this way
because it constrains the code less and leaves optimisation possible.

Any actual claim that this made something faster is a benchmark claim,
and CLAUDE.md rule 4 applies: it names its corpus, or it gets rejected
in review. Shape first, measure second, never assert in between.
