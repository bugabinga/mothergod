# ADR-0029: `cargo x` becomes the sole quality interface

Status: accepted · Date: 2026-08-24 · Prompted by operator (PR #225 thread)

## Context

CLAUDE.md's Commands block is six lines across two tools:

```
cargo x fmt --check
cargo x lint
cargo test --all-targets
cargo test --manifest-path x/Cargo.toml
cargo test --doc
RUSTDOCFLAGS="--deny warnings" cargo doc --no-deps
```

`x` (introduced in #225, no ADR of its own) already owns discovery, scope, and
diagnostics for `fmt` and `lint`. The other four lines are bare
`cargo`/`rustdoc` invocations: no shared discovery, no shared error
shape, and a fifth thing for every agent prompt and contributor to
memorize verbatim rather than discover with `cargo x help`.

The operator's read, on the #225 thread: centralizing scaffold
complexity behind one interface makes agents faster (one command
shape to parse instead of five) and more token-efficient (one help
system instead of copy-pasted invocation lines in every prompt and in
CLAUDE.md), and gives the project one place to make error messages
teach the next step instead of five.

## Decision

`cargo x` grows two more tasks and one umbrella, matching the shape
`fmt`/`lint` already established:

- `cargo x test` — wraps `cargo test --all-targets`, `cargo test
  --manifest-path x/Cargo.toml`, and `cargo test --doc` behind one
  invocation. No new selection semantics: this is a fixed test plan,
  not a file-scoped task like `fmt`/`lint`, because the three suites
  are not scoped by the same PATH semantics as source-file linting.
- `cargo x doc` — wraps `RUSTDOCFLAGS="--deny warnings" cargo doc
  --no-deps`, so the deny-warnings flag lives in one place instead of
  in every agent's memorized command line.
- `cargo x check` — runs `fmt --check`, `lint`, `test`, and `doc` in
  that order, stopping at the first failure with that task's normal
  diagnostics. This is the one line CLAUDE.md's Commands section
  becomes, and the one line CI's `rust-ci` composite action runs.

CLAUDE.md's Commands block collapses to:

```
cargo x check
```

with the constituent tasks named in `x/README.md` for anyone who
wants to scope down during iteration (`cargo x fmt -- src`, `cargo x
test -- doc`, etc. — exact scoping surface is an implementation
question, not fixed by this ADR).

## Sequencing

One idea per PR (CLAUDE.md rule 7), landed in order, each independently
mergeable:

1. `cargo x test`, no behavior change versus the three `cargo test`
   invocations it replaces.
2. `cargo x doc`, no behavior change versus the `rustdoc` invocation
   it replaces.
3. `cargo x check` umbrella, plus the CLAUDE.md Commands-block rewrite.
4. `rust-ci` composite action calls `cargo x check` instead of its own
   step list; CI's required-check names (`fmt`, `clippy`, `test`,
   `doc`) are a GitHub branch-protection concern, not a `cargo x`
   concern, so they stay as CI job/step names wrapping the one x
   invocation, not as separate x subcommands.

Filed as issue, `agent-system`, for the BDFL to pick up incrementally
rather than done in one sitting.

## Consequences

CLAUDE.md's Commands section drops from six lines naming two tools to
one line naming one. Every agent prompt that currently repeats a
subset of the six lines (reviewer, heartbeat, CI) gets the same
collapse for free once it points at `cargo x check`, because the
detail moves into `x`'s own `--help` text instead of being restated
per prompt (ADR-0025's logic: injected/discoverable beats repeated).

Cost: `x`'s test task now shells out to `cargo test` three times
under the hood rather than agents doing it directly, so a test-only
failure is one more process hop from the raw `cargo test` output.
`x`'s existing diagnostic contract (name the path, problem, next
command) is the mitigation: `cargo x test` on failure should name
which of the three suites failed and print the next command to
re-run just that one, not just relay raw `cargo test` stderr.

Risk, named and accepted: `x` becomes a single point of failure for
the whole quality gate. It already was one for `fmt`/`lint`; this
extends the same trust to `test`/`doc`. `x/Cargo.toml`'s own tests
(`cargo test --manifest-path x/Cargo.toml`) are the existing
mitigation and stay in the plan at step 1, still run directly rather
than through `x` itself, so `x` never has to test its own test
runner through itself.
