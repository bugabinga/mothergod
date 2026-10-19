# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/) once released.

## [Unreleased]

### Added

- site: mothergod.dev/decisions renders the ADR series from `docs/adr/`,
  status first, each superseded decision linked to the one that replaced
  it, searchable in the browser, so a reader learns what is decided today
  without cloning the repository (#1229).
- library: `mothergod::write::Encoder` wraps any `std::io::Write` and
  compresses what is written through it, so a `tar` builder, `io::copy` or an
  HTTP body can write into a compressor without the caller collecting the whole
  input first. It buffers one 16 MiB block and writes one frame per block,
  encoding up to two blocks at once on two cores (#1322); `finish` writes the last frame and returns the writer. Written
  without `flush`, its stream is byte for byte what `copy_encode` writes, and
  a failing writer ends the encoder instead of leaving a stream that skips a
  block (#979).
- library: `mothergod::read::Decoder` wraps any `std::io::Read` and yields the
  decompressed bytes, so a stream drops into `io::copy`, a `BufReader` or a
  `tar` builder without the caller buffering the compressed file. It holds one
  frame at a time, up to 16 MiB for a stream `compress` or `copy_encode` wrote
  and up to twice that in buffer capacity, and checks the frame's check value before returning any of its bytes, so a
  damaged frame yields nothing (#979).
- cli: `mothergod` takes the grammar gzip, xz and zstd share: it compresses,
  `-d` (`--decompress`) decompresses, `-c` (`--stdout`) writes to stdout and
  leaves the file alone, `-` names stdin, `--` ends options, and any number of
  files are processed in turn, each failure reported without stopping the rest
  (#983, ADR-0066). `mothergod -dc x.mgdc | tar x` works as it does with the
  genre's tools. With no file it reads stdin and writes stdout; with a file it
  writes `<file>.mgdc` and back. Nothing overwrites an existing file or deletes
  its input, and a file output keeps its input's modification time and, on
  Unix, its permission bits, as gzip, bzip2 and xz do (#1040). Decompression
  writes as it decodes but reads the compressed file whole first, so its
  memory grows with the compressed size (#978). Compressing to stdout refuses
  a terminal, so a forgotten redirect does not fill the screen with binary
  (#1039). A reader that closes stdout early (`| head`) ends it silently with
  status 141, as gzip does.
- cli: `-t` (`--test`) decodes each input and writes nothing, so a silent exit 0
  says a `.mgdc` file is intact and exit 1 names the ones that are not, without
  creating a file or needing the suffix (#983).
- cli: `-v` (`--verbose`) says on stderr, per input, how many bytes went in and
  came out and, for a compression, the result as a share of the input, so a
  user sees what a run did without touching the data on stdout (#983).
- cli: `-q` (`--quiet`) cancels `-v`'s report when it comes later, as it does
  in gzip, xz and zstd, so a script that wraps `mothergod -v` can silence it by
  appending the flag; errors are always reported (#983).
- cli: `-f` (`--force`) overwrites an existing output file instead of refusing
  it, replacing the file rather than truncating it, so a hard link to the
  input is never cut under the read; `-k` (`--keep`) is accepted and changes
  nothing, because the input is always kept (#983).
- format: concatenated frames decode to their payloads back to back, as
  `cat a.gz b.gz | gunzip` does, and bytes after the last frame are an error
  instead of being dropped in silence with exit status 0 (#978, ADR-0067).
  The price is 8 bytes on each compressed frame, which now says where it
  ends. A caller's `decompress_bounded` limit below 256 MiB caps the whole
  stream's output, not each frame's, and `decompress` holds the compressed
  frames of a stream to 256 MiB of output together; `decompress_to_writer`
  and the CLI stream past that at constant memory.
- format: a bit flip anywhere in a frame is an error, never wrong bytes with
  exit status 0 (#1020, ADR-0068). Each frame ends in a CRC-32 of the bytes
  before it, as gzip and xz carry, and a mismatch is `Error::BadChecksum`,
  reported before any of the frame is decoded, so a damaged frame writes
  nothing. The price is 4 bytes on each frame and one pass over it at decode,
  measured at 380 MB/s, where `Lz` decode of Rust source runs at 11 MB/s.
- compress: the encoder also tries the input reversed and then differenced at
  the stride the reversed bytes rank first, so fixed-width little-endian
  records are coded most-significant byte first. On the `sqlite_like_records`
  generator, 50,000 bytes: 20,864 to 20,292 bytes, 2.7% smaller; on the first
  64 KiB of a PIE binary's `.rela.dyn` relocation section, 7.5% smaller; no
  other benchmark case moves. Encode takes 6 to 14% longer on the benchmark
  cases, one more trial encode where a stride ranks; decode cost does not
  measurably change (#1222, ADR-0073). Format change: `FORMAT_VERSION` 16,
  files written by 15 do not decode.
- compress: the encoder also tries the input in reverse byte order, so a
  block coded better backwards is stored that way. On the sealed
  `access_log` generator, 50,000 bytes: 5,377 to 5,340 bytes, 0.69% smaller;
  on `interleaved_audio16`, 0.05% smaller; no other benchmark case moves.
  Encode takes 24 to 31% longer on the benchmark cases, because every input
  pays one more trial encode; decode cost does not measurably change
  (#1123, ADR-0072). Format change: `FORMAT_VERSION` 15, files written by 14
  do not decode.
- compress: an input that is entirely standard base64 (one unbroken line, RFC
  4648 alphabet, canonical padding) is decoded before it is coded and
  re-wrapped on decode, so the model sees the bytes underneath. On
  `base64_wrapped`, a text payload base64-encoded, 50,000 bytes: 0.532 to 0.395
  bits/byte, and no other benchmark case moves. Line-wrapped base64 (MIME,
  PEM) and base64 inside a larger file are not covered (#1206, ADR-0071).
  Format change: `FORMAT_VERSION` 14, files written by 13 do not decode.
- compress: an input is cut into 16 MiB blocks, one frame each, so peak
  encode memory stays under 1 GiB however long the input: 256 MiB of zeros
  took 11 GB and did not finish in 300 s, and now takes 946 MB and 6 minutes
  (#1094, #978). Inputs of at most 16 MiB compress to the same bytes as
  before. On the four Silesia files over 16 MiB the output changes by +0.11% in
  total (webster +0.44%, mozilla -0.06%): each block restarts the
  model.
- library: `mothergod::copy_encode(reader, writer)` compresses everything a
  `Read` yields into a `Write`, one 16 MiB block and one frame at a time (two
  blocks encode at once on two cores), so its memory is bounded by the block
  and not by the input (#978). The CLI
  compresses through it, so a file or pipe of any length compresses in under
  1 GiB. Up to 256 MiB the bytes equal `compress`'s. Past it, `compress`
  stores the input, plus a few bytes, so that `decompress` reads back every
  frame it writes (#1086); `copy_encode` compresses, and
  `decompress_to_writer` and the CLI read the result back, while
  `decompress` rejects it as `TooLarge`.
- library: `mothergod::Error` and `WriteError` convert into `std::io::Error`,
  so `?` works in a function returning `io::Result` without a `map_err`
  (#1081). A truncated frame is `UnexpectedEof`, an allocator failure is
  `OutOfMemory`, every other failure is `InvalidData`; the conversion boxes
  nothing, so a decode error cannot abort under memory pressure.
- cli: `--help` and `--version` (or `-h` and `-V`) are the rest of the
  interface, with no compression levels and no tuning knobs (#1036).
  `--version` prints the frame format version the build reads, the one fact
  a bug report or packaging script needs to say which files decode (#977).
- Library crate (`mothergod::compress`/`decompress`, plus
  `decompress_bounded`, `decompress_to_writer`, `decodes_incrementally`, and
  the `filters` module): a documented public API with a crate-root doctest;
  internal modules stay `pub` for the sibling `bench` crate's own use but are
  hidden from docs.rs.
- Codec: a filter bank (delta, transpose, x86 BCJ) feeding an optimal-parse
  LZ with in-DP repeat offsets and a six-expert context-mixing adaptive
  range coder, blended in the logit domain and SSE-calibrated. Zero
  runtime dependencies.
- Bitstream format at `FORMAT_VERSION` 16, specified in
  [`docs/format/SPEC.md`](docs/format/SPEC.md). Until 1.0 a version a
  release has written is retired only after a later release that still
  reads it and writes its successor, named here, so you can re-compress
  first; from 1.0 on, no version is ever retired (ADR-0050).
- Format versions below 16 no longer decode: no release ever wrote them, and
  before the first release each version bump retires its predecessor
  (ADR-0063). Frames from a source build older than this one must be
  re-compressed.
- Ratio, on the held-out finals ([`bench/corpus.toml`](bench/corpus.toml),
  pinned by URL and SHA-256): beats both `zstd -19` and `xz -9e` in
  aggregate on Canterbury (1.347 bits/byte vs 1.470 and 1.403); trails both
  in aggregate on Silesia (2.029 vs 1.997 and 1.829). Per-file tables and
  the reproduction command: [`docs/benchmarks/`](docs/benchmarks/).
- Trust: a frame of incompressible data cut short is now an error with
  exit status 1 instead of a silent prefix with exit status 0; the price
  is 8 bytes on such a frame (#1019).
- Trust: the decoder answers truncation, corruption, and any other
  adversarial input with an `Err`, never a panic or unbounded allocation,
  verified by a fuzzed, mutation-tested, and allocation-torture-swept test
  suite, with region coverage measured weekly and trended on the status
  page ([`docs/TESTING.md`](docs/TESTING.md)).
- Status page: the SIMPLICITY scorecard's public API surface (item count
  visible on docs.rs, with a trend once a second reading exists), the other
  half of the metric alongside the existing `src/` line counts. `cargo x
  doc` keeps the published count honest against what rustdoc actually
  built (`docs/api-surface.txt`).
- `site/404.html`: mothergod.dev's soft-404 (#753 item 3) is a real page
  now, carrying the same site nav as every other page so a broken or
  mistyped link still offers a way to the three real pages instead of a
  silent copy of the homepage.
- Status page: names, per corpus, the fastest gzip, zstd or xz level that
  compresses at least as well as mothergod and the best-compressing level
  that is at least as fast, with their bits/byte and encode MB/s and the
  date the pair last moved, so a reader sees what mothergod's ratio costs
  in speed against the level they would otherwise pick (#1340).

### Changed

- encode: the optimal parse holds 21 bytes of state per input byte, down
  from 41, so a 16 MiB block peaks at 372 MB instead of 700 MB and
  Silesia `dickens` at 364 MB instead of 562 MB, with output bytes
  unchanged and encode time within +4% on 16 MiB of zeros (#1341).
- encode: on two or more cores a file or stream past 16 MiB encodes two
  blocks at once, so Silesia `mozilla` compresses in 261 s instead of 358 s
  (1.37 times faster, 4-core runner) with output bytes unchanged; peak memory
  there is 879 MB against 521 MB serial, and 256 MiB of zeros peaks at 776 MB,
  both under the 1 GiB bound. A one-core machine encodes serially as before
  (#1322).
- encode: peak memory falls by 12% to 18% on Silesia (`mozilla` from 617 to
  509 MiB, `dickens` from 355 to 312 MiB) because the optimal parse frees
  each round's token list before the next round allocates; output bytes are
  unchanged and 16 MiB of zeros stays at 364 MiB (#1382).
- Status page: the TRUST panel's fuzz CPU-hours and crasher count are
  cumulative since the ledger's first entry (2026-09-02) instead of a
  trailing 90-day window, because each entry is now a row of a table that
  outlives the artifact retention (#1077).
- crate: the published package holds `src/`, `tests/` (minus the
  repository-only `claims.rs` and `mutants.toml`), the one research file
  the unit tests embed, the README, LICENSE and CHANGELOG, 90 files
  instead of 374, and its own `cargo test` passes (#1021, #1024).

### Fixed

- bench: the encode and decode MB/s in `docs/benchmarks/` reports are timed
  one file at a time, so a figure compares between files; the old run
  timed 12 threads on 4 cores, and `dickens` read 0.022 MB/s against 0.104
  alone. The published figures are regenerated the same way: Canterbury
  encodes at 0.235 MB/s, up from 0.129, and Silesia at 0.188, up from 0.040,
  on one CI machine; bits per byte are unchanged (#1321, #1324).
- compress: the same input compresses to the same bytes on every run;
  the transpose column score summed in a per-process hash order, so a
  near-tie between column counts could pick a different filter from one run
  to the next (#1242).
- cli: a stderr that cannot take a message (`2>/dev/full`, a closed
  descriptor) no longer aborts the run with exit 101 after the output is
  complete; the message is dropped and the exit status stays what the
  conversion earned (#1161).
- cli: `--help` and `--version` on a stdout that cannot take them
  (`>/dev/full`) exit 1 with `mothergod: writing stdout: ...` instead of
  panicking with exit 101; a closed pipe exits 141 silently, as compression
  already does (#1167).
- `read::Decoder`, `decompress`: an `Lz` frame whose header declares a
  payload longer than a `Stored` frame of the same output holds is `Corrupt`
  as soon as its header is read, so `Decoder::with_max_len(src, 1 << 20)` no
  longer buffers hundreds of MiB of a hostile source for a frame that
  declares 10 bytes of output. No frame `compress` wrote is affected (#1137,
  ADR-0069).
- `decompress`: a memory-starved process gets `Error::OutOfMemory` instead of
  an abort when it decodes a stored (incompressible) frame, the one decode
  allocation that was not fallible (#1130).
