# mothergod

[![ci](https://github.com/bugabinga/mothergod/actions/workflows/ci.yml/badge.svg)](https://github.com/bugabinga/mothergod/actions/workflows/ci.yml)

**General-purpose lossless compression, developed by an autonomous AI dev
team.**

mothergod is two experiments in one repository:

1. **A compressor.** A context-mixing LZ hybrid in Rust — filter bank →
   optimal-parse LZ with in-DP repeat offsets → adaptive arithmetic coding
   with gradient-mixed experts. Its design was derived from first principles
   in a single research session and validated on the Silesia and Canterbury
   corpora, where the prototype beat `zstd -19` in aggregate with real,
   losslessly-verified bitstreams. Every design decision traces to a recorded
   experiment — see [`research/JOURNAL.md`](research/JOURNAL.md).

2. **A development team made of agents.** Day-to-day development — triage,
   implementation, adversarial code review, research experiments, releases —
   is done by Claude agents running on GitHub Actions, slowly, in public,
   like a real team would. A human operator holds the veto and the keys.
   How this works: [`GOVERNANCE.md`](GOVERNANCE.md).

## Status: pre-alpha, format unstable

The repository currently contains the project scaffolding and a v0 container
format whose only method is `Stored`. The research prototype (codec v0.6) is
being imported/reconstructed — progress is tracked in
[`ROADMAP.md`](ROADMAP.md). Do not use this for data you care about yet.

## Try it

```sh
git clone https://github.com/bugabinga/mothergod
cd mothergod
cargo test
```

Library API (subject to change until 1.0):

```rust
let frame = mothergod::compress(b"hello");
assert_eq!(mothergod::decompress(&frame).unwrap(), b"hello");
```

## Interacting with the project

- **Found a bug / have an idea?** [Open an issue](../../issues/new/choose).
  An agent will triage it, usually within a day.
- **Want to discuss?** Mention `@claude` in any issue or PR and the
  interactive agent will answer.
- **Want to contribute code?** See [`CONTRIBUTING.md`](CONTRIBUTING.md).
  Human PRs are reviewed by the same adversarial reviewer agent.
- **Something looks wrong with the automation?** Ping the operator,
  [@bugabinga](https://github.com/bugabinga).

## Principles (the short version)

- Lossless is sacred; the decoder never panics on any input.
- Every benchmark claim names its corpus — a ratio is a claim about data.
- Every experiment, failed or not, is recorded. Rejections are knowledge.
- Verification is independent of the proposer: agents never grade their own
  claims.

## License

[MIT](LICENSE)
