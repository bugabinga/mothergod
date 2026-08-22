# Panic discipline

CLAUDE.md hard rule 2: the decoder never panics on ANY input.

That rule is about untrusted input. It does not mean "never panic
anywhere", and reading it that way is harmful: it pushes you to swallow
real bugs into `Result`s that no caller can act on.

## The line

Ask one question: **who has to be wrong for this to fire?**

| Who is wrong | Correct response |
|---|---|
| The input's author (malformed, truncated, hostile) | `Result`, always. Never a panic |
| Us (an invariant our own code maintains) | A panic is legitimate, and better than encoding garbage |

A violated internal invariant is a programmer-bug signal. Silently
continuing past one produces a corrupt bitstream, which violates hard
rule 1, which is worse than a crash in a build we control.

## What this means on a decode path

Everything reachable from attacker-controlled bytes must return a
`Result` or be provably unreachable. The panics that hide in ordinary
Rust:

- **Slice and `Vec` indexing.** `buf[i]` panics. Use `.get(i).ok_or(...)`
  on any index derived from input.
- **Arithmetic.** Overflow panics in debug and wraps in release. Both
  are wrong here. Use `checked_add`, `checked_mul`, `checked_sub` and
  turn `None` into a decode error.
- **Slicing ranges.** `&buf[a..b]` panics when `b > len` or `a > b`. Use
  `.get(a..b)`.
- **Division and remainder** by an input-derived value.
- **`unwrap` / `expect`.** Even where you checked the condition three
  lines up: the check and the unwrap drift apart under later edits. If
  the invariant is genuinely local and structural, say so in a comment
  stating the invariant, per CLAUDE.md's comment rule.
- **Recursion whose depth comes from input.** Stack overflow is not a
  panic and cannot be caught. Bound the depth or make it iterative.

## What this means elsewhere

On the encode path and in internal helpers, panic freely on our own
broken invariants. If the LZ parser emits a match offset beyond the
window, that is our bug; aborting loudly is correct, and converting it
to a `Result` that the caller cannot meaningfully handle just moves the
bug downstream.

Test code and `debug_assert!` are unrestricted.

## Verification

`docs/TESTING.md` owns the test strategy, including which layer proves
this and when. Read it there; do not restate its plan here or in a PR.
