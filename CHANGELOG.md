# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/) once released.

## [Unreleased]

### Added

- cli: `mothergod` takes the grammar gzip, xz and zstd share: it compresses,
  `-d` (`--decompress`) decompresses, `-c` (`--stdout`) writes to stdout and
  leaves the file alone, `-` names stdin, `--` ends options, and any number of
  files are processed in turn, each failure reported without stopping the rest
  (#983, ADR-0066). `mothergod -dc x.mgdc | tar x` works as it does with the
  genre's tools. With no file it reads stdin and writes stdout; with a file it
  writes `<file>.mgdc` and back. Nothing overwrites an existing file or deletes
  its input, and a file output keeps its input's modification time and, on
  Unix, its permission bits, as gzip, bzip2 and xz do (#1040). Decompression
  streams instead of buffering the whole file. Compressing to stdout refuses a
  terminal, so a forgotten redirect does not fill the screen with binary
  (#1039). A reader that closes stdout early (`| head`) ends it silently with
  status 141, as gzip does.
- format: concatenated frames decode to their payloads back to back, as
  `cat a.gz b.gz | gunzip` does, and bytes after the last frame are an error
  instead of being dropped in silence with exit status 0 (#978, ADR-0067).
  The price is 8 bytes on each compressed frame, which now says where it
  ends. A caller's `decompress_bounded` limit below 256 MiB caps the whole
  stream's output, not each frame's, and `decompress` holds the compressed
  frames of a stream to 256 MiB of output together; `decompress_to_writer`
  and the CLI stream past that at constant memory.
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
- Bitstream format at `FORMAT_VERSION` 12, specified in
  [`docs/format/SPEC.md`](docs/format/SPEC.md). Until 1.0 a version a
  release has written is retired only after a later release that still
  reads it and writes its successor, named here, so you can re-compress
  first; from 1.0 on, no version is ever retired (ADR-0050).
- Format versions below 12 no longer decode: no release ever wrote them, and
  before the first release each version bump retires its predecessor
  (ADR-0063). Frames from a source build older than this one must be
  re-compressed.
- Ratio, on the held-out finals ([`bench/corpus.toml`](bench/corpus.toml),
  pinned by URL and SHA-256): beats both `zstd -19` and `xz -9e` in
  aggregate on Canterbury (1.351 bits/byte vs 1.470 and 1.403); trails both
  in aggregate on Silesia (2.032 vs 1.997 and 1.829). Per-file tables and
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

### Fixed

- format: `compress` stores an input longer than 256 MiB instead of writing a
  compressed frame that `decompress` rejects as too large, which made the
  output of a 256 MiB to 4 GiB input unreadable. Such inputs lose their
  compression until frames split large inputs (#1086).

### Changed

- Status page: the TRUST panel's fuzz CPU-hours and crasher count are
  cumulative since the ledger's first entry (2026-09-02) instead of a
  trailing 90-day window, because each entry is now a row of a table that
  outlives the artifact retention (#1077).
- crate: the published package holds `src/`, `tests/` (minus the
  repository-only `claims.rs` and `mutants.toml`), the one research file
  the unit tests embed, the README, LICENSE and CHANGELOG, 90 files
  instead of 374, and its own `cargo test` passes (#1021, #1024).
