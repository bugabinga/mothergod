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
measurement time and never committed. Measured 2026-10-26 against
`gzip 1.12`, `Zstandard 1.5.7` and `XZ Utils 5.4.5` at the flags below.

| corpus | **mothergod** | gzip -9 | zstd -19 | xz -9e |
|---|---|---|---|---|
| Canterbury | **1.347** | 2.079 | 1.470 | 1.403 |
| Silesia | **2.005** | 2.553 | 1.997 | 1.829 |

mothergod beats both `zstd -19` and `xz -9e` in aggregate on Canterbury, and
loses to both in aggregate on Silesia. Per file, against whichever of the two
is stronger on that file, it wins 7 of 11 on Canterbury and 1 of 12 on
Silesia. The wins are Canterbury's six smallest text and source files
(`alice29.txt`, `asyoulik.txt`, `cp.html`, `fields.c`, `grammar.lsp`,
`xargs.1`), its `kennedy.xls` spreadsheet, and Silesia's `ooffice` library.
The widest losses are Silesia's `sao` star catalog, `mozilla` install archive
and `mr` medical image, and Canterbury's `sum` executable. Closing Silesia
heads [`ROADMAP.md`](ROADMAP.md), as open-ended research with no date
attached. Every file's numbers:
[Canterbury](docs/benchmarks/canterbury.md),
[Silesia](docs/benchmarks/silesia.md).

**It is slow, single-run, on one CI machine.** mothergod encoded Canterbury at
0.319 MB/s and decoded it at 5.018 MB/s; it encoded Silesia at 0.211 MB/s and
decoded it at 3.464 MB/s. In the same run, `xz -9e` encoded Silesia at 1.949
MB/s and decoded it at 88.426 MB/s, more than nine times faster to encode and
more than twenty times faster to decode.
The same two reports name the machine, give every file's rates, and end
with every level of gzip, zstd and xz beside mothergod, peak memory
included. The commands that regenerate them:
[`docs/benchmarks/`](docs/benchmarks/README.md#regenerating).

**Pre-alpha: no release, no packaged binary, no version tag.** The container
format (`FORMAT_VERSION` 17) is specified but not frozen. Before the first
release, each format change retires the version before it, so the next build
that changes the format will not read a file you compress today. After that,
until 1.0 a version can still be retired; from 1.0 on, none ever is. The
rules: [`docs/format/SPEC.md`](docs/format/SPEC.md). Do not use this for data
you care about yet.

## Try it

There is no download: no release, no tag, no package. Building it yourself is
the only way to run it, and the core crate has zero runtime dependencies, so
this compiles exactly one crate. The test suite runs weekly on Linux, Windows
and macOS, each on x64 and ARM64, and on Android x64; the
[lane table](docs/TESTING.md#current-automated-cadence) lists every one.

```sh
git clone https://github.com/bugabinga/mothergod
cd mothergod
cargo build --release --bin mothergod
```

The first run needs no file of yours, because the clone ships one:

```console
$ ./target/release/mothergod -v -c LICENSE > /dev/null
mothergod: LICENSE: 1074 -> 630 bytes (58.6%)
```

`-v` asked for that line; without it, a run that worked prints nothing. `-c`
sent the compressed bytes to stdout, which `/dev/null` swallowed, so the run
writes no file and repeats cleanly. One 1 KB file is a smoke test, not a
benchmark; the tables above are the benchmark.

The CLI speaks the grammar `gzip`, `xz` and `zstd` already taught you: it
compresses, `-d` decompresses, stdin to stdout when you name no file. Round-trip
something of yours and check the result rather than taking our word for it.

```sh
./target/release/mothergod    < FILE      > FILE.mgdc
./target/release/mothergod -d < FILE.mgdc > FILE.out
cmp FILE FILE.out   # silence means the round trip was lossless
```

Given a file, it writes `<file>.mgdc` beside it, and unlike `gzip` it never
deletes the input. `--help` lists the rest on one screen: flags you know from
`gzip`, and no compression levels or tuning knobs. `--version` also prints the
format version the build reads; put it in a bug report. Expect the encode to
take a while, at the rates above.

Library API (subject to change until 1.0):

```rust
let frame = mothergod::compress(b"hello");
assert_eq!(mothergod::decompress(&frame).unwrap(), b"hello");
```

Those two hold the whole input and output in memory. A file or stream of any
length goes through `std::io` instead, in 16 MiB blocks, so memory follows the
blocks and not the input. A block is not the peak: encoding one needs far
more working memory, and two encode at once on two cores. A 256 MiB stream of
zeros encodes in under 1 GiB, a bound
[checked weekly](.github/workflows/memory-check.yml).

```rust
use std::{fs::File, io};

mothergod::copy_encode(File::open("big.tar")?, File::create("big.tar.mgdc")?)?;
let mut reader = mothergod::read::Decoder::new(File::open("big.tar.mgdc")?);
io::copy(&mut reader, &mut File::create("big.tar.out")?)?;
```

A producer that writes rather than reads, such as a `tar` builder, writes
into `mothergod::write::Encoder` instead. Call its `finish`, or the last block
never reaches the file.

`cargo doc --open` documents each call's memory bound and errors.

## Who builds it

mothergod began as an autoresearch loop. On 2026-08-19 one Claude session ran
about 40 iterations of propose, score bits per byte on validation data, keep
or reject, record; a proposed filter had to invert first. Bits per byte is
also the metric
[karpathy/autoresearch](https://github.com/karpathy/autoresearch) minimizes,
on language models instead of files. That harness is preserved at
commit [`1a3b1c8`](https://github.com/bugabinga/mothergod/tree/1a3b1c8/research/imports/session-1),
and its findings open the research journal. The loop still runs; around it
grew a versioned format, a decoder fuzzed against hostile input, and review
by an agent that did not write the change.

Day-to-day development is done by Claude agents running on GitHub Actions:
triage, implementation, adversarial code review, research experiments,
releases. Slowly, in public, like a real team would. A human operator holds
the veto and the keys. That is the second experiment in this repository, and
the numbers above are the first one's report card. How it works:
[`agents/GOVERNANCE.md`](agents/GOVERNANCE.md). What it has decided, and
what it later reversed:
[mothergod.dev/decisions](https://mothergod.dev/decisions).

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

- Lossless is sacred; the decoder never panics on any input, and a flipped bit
  in a compressed file is an error, never quietly wrong output.
- Every benchmark claim names its corpus; a ratio is a claim about data.
- Every experiment, failed or not, is recorded. Rejections are knowledge.
- Verification is independent of the proposer: agents never grade their own
  claims.

## License

[MIT](LICENSE)
