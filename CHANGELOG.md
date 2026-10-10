# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/) once released.

## [Unreleased]

### Added

- `mothergod` CLI (`compress`/`decompress`): stdin/stdout by default, or a
  file argument writing/reading the `.mgdc` suffix; neither subcommand
  overwrites an existing output or deletes its input. Decompression streams
  its output instead of buffering the whole file. `mothergod --help` is the
  whole interface: no flags, no compression levels. A reader that closes
  stdout early (`| head`) ends it silently with status 141, as gzip does.
- Library crate (`mothergod::compress`/`decompress`, plus
  `decompress_bounded`, `decompress_to_writer`, `decodes_incrementally`, and
  the `filters` module): a documented public API with a crate-root doctest;
  internal modules stay `pub` for the sibling `bench` crate's own use but are
  hidden from docs.rs.
- Codec: a filter bank (delta, transpose, x86 BCJ) feeding an optimal-parse
  LZ with in-DP repeat offsets and a six-expert context-mixing adaptive
  range coder, blended in the logit domain and SSE-calibrated. Zero
  runtime dependencies.
- Bitstream format at `FORMAT_VERSION` 11, specified in
  [`docs/format/SPEC.md`](docs/format/SPEC.md). Until 1.0 a version a
  release has written is retired only after a later release that still
  reads it and writes its successor, named here, so you can re-compress
  first; from 1.0 on, no version is ever retired (ADR-0050).
- Format versions below 11 no longer decode: no release ever wrote them, and
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
