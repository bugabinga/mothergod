# CLAUDE.md — agent contract for mothergod

Audience: Claude agents (CI sessions, heartbeat, reviewer, researcher, interactive).
Humans: read README.md and CONTRIBUTING.md instead.

## What this project is

General-purpose lossless compressor in Rust. Architecture target: filter bank →
optimal-parse LZ with in-DP repeat offsets → context-mixing adaptive arithmetic
coder. The design was derived experimentally; `research/JOURNAL.md` is the
institutional memory. Read it before touching codec code. Do not re-run
falsified experiments unless conditions changed (note which condition).

## Commands (run all before any push)

```
cargo fmt --check
cargo clippy --all-targets -- --deny warnings
cargo test --all-targets
cargo test --doc
RUSTDOCFLAGS="--deny warnings" cargo doc --no-deps
```

CI (`quality-gate`) runs exactly these. A push that fails them wastes a cycle.

## Hard rules

1. Lossless is sacred. Every codec change ships with a round-trip test on the
   change's target data class. `decompress(compress(x)) == x`, always, or the
   change does not merge.
2. The decoder never panics, never overallocates unbounded, on ANY input.
   Treat all compressed input as adversarial (bombs, truncation, bit flips).
3. Never weaken a guard, test, benchmark, or corpus to make a metric look
   better. Verification stays independent of the proposer: you do not grade
   your own claim — the reviewer agent and CI do.
4. Benchmark claims name their corpus. "X bits/byte" without "on <corpus>" is
   meaningless and gets rejected in review.
5. Format changes (frame layout, method bytes, model semantics visible in the
   bitstream) require: bump `FORMAT_VERSION`, an ADR in `docs/adr/`, and
   decode support for all previous versions unless an ADR drops one.
6. Every experiment — accepted or rejected — gets a `research/JOURNAL.md`
   entry and a `research/progress.jsonl` line. Rejections are as valuable as
   accepts; record the mechanism of failure, not just the score.
7. Small PRs. One idea per PR. Update `CHANGELOG.md` (Unreleased) in the same
   PR for anything user-visible.
8. Do not merge your own PR. The reviewer workflow does that.
9. Respect the pause: if an open issue labeled `agents-paused` exists and its
   RESUME-AT is in the future, stop and exit cleanly.

## Style

- Edition 2024, zero runtime dependencies in the core crate (dev-deps are fine).
- Lints are strict (`clippy::pedantic`, `missing_docs`); fix, don't allow —
  an `#[allow]` needs a one-line justification comment.
- Comments state invariants the code can't show. The port bug of session-1
  (rep-symbol/offset-bucket collision) existed because an invariant lived only
  in one implementation's window size. Write invariants down.

## Where things live

| Path | What |
|---|---|
| `src/` | the crate |
| `research/JOURNAL.md` | falsification journal — laws, dead theories, standing leads |
| `research/progress.jsonl` | machine-readable experiment log (schema in `research/README.md`) |
| `research/corpus/POLICY.md` | benchmark corpus rules: sealed validation, regret-scored additions |
| `docs/adr/` | architecture decision records |
| `docs/format/SPEC.md` | bitstream format spec (draft until 1.0) |
| `ROADMAP.md` | milestones; heartbeat picks work from here |
| `.github/workflows/` | the agent processes themselves — changeable by PR like any code |

## Issue/PR conventions

- Branches: `claude/<short-slug>`. Conventional-ish commit subjects, imperative.
- Labels agents maintain: `triage`, `bug`, `enhancement`, `research`,
  `blocked-on-human`, `agents-paused`, `ops-log`, `agent-approved`.
- Anything only the human operator can do (secrets, settings, uploads,
  crates.io) → label `blocked-on-human`, explain exactly what is needed, move on.
