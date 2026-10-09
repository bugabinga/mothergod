<p align="center">
  <img src="assets/logo.svg" alt="mothergod — chevrons compressing the golden byte" width="180"/>
</p>

# mothergod

[![ci](https://github.com/bugabinga/mothergod/actions/workflows/ci.yml/badge.svg)](https://github.com/bugabinga/mothergod/actions/workflows/ci.yml)
[mothergod.dev](https://mothergod.dev)

**A general-purpose lossless compressor in Rust, built for compression ratio
rather than speed.** The way to judge it is bits per byte on the corpora
everyone quotes, against the reference compressors at their strongest
settings. That table is directly below. Every design decision traces to a
recorded experiment in
[`research/JOURNAL.md`](research/JOURNAL.md), rejections included.

## Where it stands

Aggregate bits per byte, lower is better, on the held-out final corpora:
Canterbury (11 files, 2.8 MB) and Silesia (12 files, 212 MB), each pinned by
URL and SHA-256 in [`bench/corpus.toml`](bench/corpus.toml), fetched at
measurement time and never committed. Measured 2026-10-08 against
`gzip 1.12`, `Zstandard 1.5.7` and `XZ Utils 5.4.5` at the flags below.

| corpus | **mothergod** | gzip -9 | zstd -19 | xz -9e |
|---|---|---|---|---|
| Canterbury | **1.351** | 2.081 | 1.470 | 1.403 |
| Silesia | **2.032** | 2.553 | 1.997 | 1.829 |

mothergod beats both `zstd -19` and `xz -9e` in aggregate on Canterbury, and
loses to both in aggregate on Silesia. Per file, against whichever of the two
is stronger on that file, it wins 7 of 11 on Canterbury and 1 of 12 on
Silesia. Closing Silesia is [`ROADMAP.md`](ROADMAP.md)'s current milestone.
Per-file tables, the throughput columns, and the command that regenerates
each report: [`docs/benchmarks/`](docs/benchmarks/).

**It is slow, single-run, on one CI machine.** mothergod encoded Canterbury at
0.130 MB/s and decoded it at 2.924 MB/s; it encoded Silesia at 0.061 MB/s and
decoded it at 1.420 MB/s. Methodology, per-file rates, and the regeneration
command: [`docs/benchmarks/`](docs/benchmarks/).

**Pre-alpha: no release, no packaged binary, no version tag.** The container
format (`FORMAT_VERSION` 10) is specified but not frozen: until 1.0 a version
can be retired. What each version promises and the retirement rules:
[`docs/format/SPEC.md`](docs/format/SPEC.md). Do not use this for data you
care about yet.

## Try it

There is no download: no release, no tag, no package. Building it yourself is
the only way to run it, and the core crate has zero runtime dependencies, so
this compiles exactly one crate.

```sh
git clone https://github.com/bugabinga/mothergod
cd mothergod
cargo build --release --bin mothergod
```

The CLI has the shape `gzip -c` and `zstd -c` already taught you: two
subcommands, stdin to stdout when you name no file. Round-trip something of
yours and check the result rather than taking our word for it.

```sh
./target/release/mothergod compress   < FILE      > FILE.mgdc
./target/release/mothergod decompress < FILE.mgdc > FILE.out
cmp FILE FILE.out   # silence means the round trip was lossless
```

Given a file argument instead, `compress` writes `<file>.mgdc` and
`decompress` reads one back to the name with the suffix stripped. Neither
deletes its input, and neither overwrites an existing file.
`mothergod --help` is the entire interface: no flags, no compression levels,
no tuning knobs. Expect the encode to take a while, at the rates above.

Library API (subject to change until 1.0):

```rust
let frame = mothergod::compress(b"hello");
assert_eq!(mothergod::decompress(&frame).unwrap(), b"hello");
```

## Who builds it

mothergod began as an autoresearch loop. On 2026-08-19 one Claude session ran
about 40 iterations of propose, check the change is lossless, score bits per
byte on held-out data, keep or reject, record. Bits per byte is also the
metric [karpathy/autoresearch](https://github.com/karpathy/autoresearch)
minimizes, on language models instead of files. That harness is preserved at
commit [`1a3b1c8`](https://github.com/bugabinga/mothergod/tree/1a3b1c8/research/imports/session-1),
and its findings open the research journal. The loop still runs; around it
grew a versioned format, a decoder fuzzed against hostile input, and review
by an agent that did not write the change.

Day-to-day development is done by Claude agents running on GitHub Actions:
triage, implementation, adversarial code review, research experiments,
releases. Slowly, in public, like a real team would. A human operator holds
the veto and the keys. That is the second experiment in this repository, and
the numbers above are the first one's report card. How it works:
[`agents/GOVERNANCE.md`](agents/GOVERNANCE.md).

## Interacting with the project

- **Found a bug / have an idea / want to discuss?**
  [Open an issue](../../issues/new/choose).
  An agent triages daily, usually answering within a day. Silence
  beyond that means the system paused itself: check for an open issue
  labeled `agents-paused`, or read
  [mothergod.dev/status](https://mothergod.dev/status) for
  commits merged in the last 7 days.
- **Want to contribute code?** See [`CONTRIBUTING.md`](CONTRIBUTING.md).
  Human PRs are reviewed by the same adversarial reviewer agent.
- **Something looks wrong with the automation?** Ping the operator,
  [@bugabinga](https://github.com/bugabinga).

## Principles (the short version)

- Lossless is sacred; the decoder never panics on any input.
- Every benchmark claim names its corpus; a ratio is a claim about data.
- Every experiment, failed or not, is recorded. Rejections are knowledge.
- Verification is independent of the proposer: agents never grade their own
  claims.

## License

[MIT](LICENSE)
