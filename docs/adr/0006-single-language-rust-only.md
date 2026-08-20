# ADR-0006: Single implementation language — the Python archive is an oracle, not a codebase

Status: accepted · Date: 2026-08-20 · Extends ADR-0002

## Context

The founding session developed the codec twice: a Python research prototype
(model-cost proxy, fast experiment loop) and the Rust port. That split
produced the project's canonical port bug — the rep-symbol/offset-bucket
collision existed because an invariant lived silently in one implementation's
window size (CLAUDE.md, JOURNAL S1-A*). The Python proxy also mis-measured
twice on its own: ~0.1% ideal-cost optimism versus real bitstreams, and the
10 KB-slice scale trap that wrongly rejected rich contexts five times
(JOURNAL S1-L4). With the archive imported and verified
(`research/imports/session-1/`, resumable at it31), the temptation exists to
resume the Python loop for cheap experiments — reopening the two-codec gap,
doubling maintenance and review load, and spending the operator's
subscription on synchronization instead of progress.

## Decision

1. **Rust is the only implementation language.** Codec, experiments, the M2
   benchmark harness, and corpus generators are written in Rust (dev-deps
   allowed per ADR-0002). No new Python (or other-language) code enters the
   repository.
2. **The Python loop is not resumed.** Research experiments run against the
   in-repo Rust codec. For proxy-speed iteration, build an ideal-cost
   accounting mode into the Rust models (sum `-log2(p)` instead of emitting
   bits) — same trick, no port step, no drift, faster than Python.
3. **`research/imports/session-1/` is a frozen, read-only oracle.** It is
   historical evidence and a cross-check: agents may *run* it read-only to
   settle a port question ("what did the champion do here?"), and must never
   edit, extend, or maintain it. It stays outside CI, lints, and review
   scope.
4. Numbers produced by the archive's model-cost proxy are quotable only as
   historical context, clearly marked; new claims require real bitstreams
   from the Rust codec (corpus rules per `research/corpus/POLICY.md`).

## Consequences

- One codec of record: the champion and the product are the same artifact,
  and every accepted experiment is already shipped code.
- The proxy's speed advantage is recovered in Rust via the accounting mode;
  until that mode exists, experiments pay real-bitstream cost — acceptable,
  since the standing leads (SSE, parse quality, windows) need bitstream
  validation anyway.
- Reversal (e.g. a future scripting layer for corpus tooling) requires a
  superseding ADR.
