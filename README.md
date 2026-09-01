<p align="center">
  <img src="assets/logo.svg" alt="mothergod — chevrons compressing the golden byte" width="180"/>
</p>

# mothergod

[![ci](https://github.com/bugabinga/mothergod/actions/workflows/ci.yml/badge.svg)](https://github.com/bugabinga/mothergod/actions/workflows/ci.yml)
[mothergod.dev](https://mothergod.dev)

**General-purpose lossless compression, developed by an autonomous AI dev
team.**

mothergod is two experiments in one repository:

1. **A compressor.** A context-mixing LZ hybrid in Rust — filter bank →
   optimal-parse LZ with in-DP repeat offsets → adaptive arithmetic coding
   with gradient-mixed experts. Its design was derived from first principles
   in a single research session; where it stands against `gzip`, `zstd` and
   `xz` today is the table below. Every design decision traces to a recorded
   experiment — see [`research/JOURNAL.md`](research/JOURNAL.md).

2. **A development team made of agents.** Day-to-day development — triage,
   implementation, adversarial code review, research experiments, releases —
   is done by Claude agents running on GitHub Actions, slowly, in public,
   like a real team would. A human operator holds the veto and the keys.
   How this works: [`agents/GOVERNANCE.md`](agents/GOVERNANCE.md).

## Where it stands

Aggregate bits per byte, lower is better, on the held-out final corpora:
Canterbury (11 files, 2.8 MB) and Silesia (12 files, 212 MB), each pinned by
URL and SHA-256 in [`bench/corpus.toml`](bench/corpus.toml), fetched at
measurement time and never committed. Measured 2026-08-30 against
`gzip 1.12`, `Zstandard 1.5.7` and `XZ Utils 5.4.5` at the flags below.

| corpus | **mothergod** | gzip -9 | zstd -19 | xz -9e |
|---|---|---|---|---|
| Canterbury | **1.374** | 2.081 | 1.470 | 1.403 |
| Silesia | **2.061** | 2.553 | 1.997 | 1.829 |

mothergod beats both `zstd -19` and `xz -9e` in aggregate on Canterbury, and
loses to both in aggregate on Silesia. Per file, against whichever of the two
is stronger on that file, it wins 5 of 11 on Canterbury and 1 of 12 on
Silesia. Closing Silesia is [`ROADMAP.md`](ROADMAP.md)'s current milestone.
Speed is recorded on every run and not yet worked on. Per-file tables, the
throughput columns, and the command that regenerates each report:
[`docs/benchmarks/`](docs/benchmarks/).

**Pre-alpha: no release, no packaged binary, no version tag.** The container
format (`FORMAT_VERSION` 3) carries `Stored` and `Lz` (optimal-parse LZ over
an adaptive, context-mixing range coder; `research/JOURNAL.md` S2-D2/S2-D3).
That format is frozen ([`docs/format/SPEC.md`](docs/format/SPEC.md),
ADR-0041): no future version may drop decode support for a version 2 or 3
frame, so a frame written today stays readable. Everything around it still
moves: the library API, the CLI, and the ratio above. Do not use this for
data you care about yet.

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

- **Found a bug / have an idea / want to discuss?**
  [Open an issue](../../issues/new/choose).
  An agent will triage and answer it, usually within a day.
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
