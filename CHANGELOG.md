# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/) once released.

## [Unreleased]

### Fixed

- `docs/benchmarks/canterbury.md` and `silesia.md` published `mothergod
  encode MB/s`/`mothergod decode MB/s` columns without naming the machine
  they ran on (issue #432): a throughput number is only comparable to
  another number from the same line, the same defect CLAUDE.md rule 4
  already forbids for a bits/byte number without its corpus. Both report
  generators (`finals_report`, `silesia_report`) now call
  `bench::reference::machine_info()` and print its CPU model, logical core
  count, CI-runner detection, and the one-thread-per-file measurement
  shape in the conditions sentence next to the throughput columns.
  `finals::format_report` grouped its version/fingerprint/machine
  arguments into a new `Provenance` struct to stay under
  `clippy::too_many_arguments`. Both reports regenerated; only the
  throughput columns changed (measured on a different machine than the
  prior run), bits/byte numbers are bit-for-bit unchanged.
- `README.md` and `site/index.html` still said the container format was
  `FORMAT_VERSION` 2 and that "no Silesia/Canterbury benchmark-suite number
  exists yet for this Rust build". Both were stale: the format is at version
  3 and frozen (ADR-0041), and `docs/benchmarks/canterbury.md` and
  `silesia.md` have carried whole-file numbers against pinned `gzip -9`,
  `zstd -19` and `xz -9e` since 2026-08-30, with the required `ratio` check
  failing if either report goes stale against `bench/baseline.json`. Both
  surfaces now publish the aggregate table, name its corpora, versions and
  date, and state the Silesia loss as plainly as the Canterbury win.
  `site/index.html` also dropped a claim that this build was "validated on
  the Silesia and Canterbury corpora", which described the founding Python
  prototype that lives only in git history (issue #415), and
  `site/status.html`'s baseline caption no longer calls the comparison an
  open roadmap box.
- `codec::decode` now rejects a match distance beyond `lz::WINDOW`
  (ROADMAP M4, bounded-memory decode guarantee): `OFFSET_BUCKETS` lets the
  offset model represent distances up to `2 * WINDOW - 1`, wider than any
  real encoder ever emits (its match finder never searches past `WINDOW`),
  and the decoder only checked a distance against the output written so
  far, not against `WINDOW` itself. Adversarial input could exploit the
  gap to force retention of output the encoder's own window guarantee
  never requires; no real bitstream is affected, since this codec's
  encoder never produces such a distance. New unit test
  `match_distance_beyond_window_is_rejected`.

### Added

- `.github/scripts/changelog-notes` (ROADMAP M6): extracts this file's own
  `[Unreleased]` section, the agent-drafted body a future release-drafting
  workflow needs. Standalone and runnable by hand for now: the workflow
  that would call it lives under `.github/workflows/`, which needs the
  admin PAT and is a process change (GOVERNANCE.md's decision table), so
  wiring it in is left for the BDFL.
- `mothergod::decompress_bounded(input, max_len)` (`research/JOURNAL.md`
  S1-P7, S2-A71, ROADMAP M4): lets a caller reject an over-budget frame
  before any allocation or decode work, using a ceiling below the crate's
  own `codec::MAX_DECODED_LEN` (256 MiB) — for an embedder with a smaller
  known memory budget. `max_len` is clamped to `MAX_DECODED_LEN`, never
  raised past it, since that constant is the only value this decoder's
  worst-case decode time has been measured against. `mothergod::decompress`
  is now a thin wrapper around it (`decompress_bounded(input,
  codec::MAX_DECODED_LEN)`), bit-for-bit unchanged: a `Method::Stored`
  frame is bounded by the new `max_len` check only when a caller opts into
  a tighter budget than `MAX_DECODED_LEN`, never by the default ceiling
  alone, since a stored payload's length is read from `input` directly and
  is never spoofable past it. A first, additive slice of M4's
  "bounded-memory decode guarantees" line, not the streaming/block API
  itself, which remains open. No `FORMAT_VERSION` bump:
  `docs/format/SPEC.md` already documents this ceiling as decoder policy,
  not a wire-format field.
- `mothergod::decodes_incrementally(input)` (`research/JOURNAL.md` S1-P7,
  S2-D4, ROADMAP M4): reports whether a frame's filter choice would let a
  future streaming/block decoder produce output in address order with
  bounded lookback, without decoding anything. `Method::Stored` and
  `Method::Lz` frames using `Identity`/`Delta`/`Bcj` answer `true`;
  `Method::Lz` frames using `Transpose` answer `false`, since its decode
  writes scattered across the whole buffer in column-major order and needs
  it fully resident. The streaming/block API itself remains open; this is
  the queryable predicate S2-D4's scoping decision named as a prerequisite,
  so that API can surface the split explicitly instead of a silent
  fallback.
- `mothergod::decompress_to_writer(input, max_len, writer)`
  (`research/JOURNAL.md` S1-P7, S2-D5, ROADMAP M4): the first real slice
  of the streaming/block API. Writes decoded bytes to any `std::io::Write`
  incrementally instead of collecting a `Vec<u8>`. Actually bounds resident
  memory to `lz::WINDOW` (1 MiB), regardless of the frame's declared
  length, for a `Method::Lz` frame whose encoder picked
  `filters::select::Candidate::Identity` — the only candidate whose
  filter-undo step is a no-op, so the decoded LZ token stream needs no
  buffering pass afterward. Every other candidate, and `Method::Stored`,
  falls back to a whole-buffer decode plus one bulk write: no worse than
  `decompress_bounded`, just not streamed yet. `decodes_incrementally`
  tells a caller which case a frame is ahead of time.
- `mothergod::decompress_to_writer` now also streams
  `filters::select::Candidate::Delta` frames (`research/JOURNAL.md`
  S1-P7, S2-A74, ROADMAP M4), not just `Identity`: a new
  `filters::delta::Undo` type undoes the delta transform one filtered byte
  at a time, so a `Method::Lz` frame whose encoder picked `Delta` now
  bounds resident memory to `lz::WINDOW` the same way `Identity` already
  did. `Bcj` and `Transpose` are unchanged, still falling back to a
  whole-buffer decode; `decodes_incrementally`'s answer is unaffected,
  since it already reported `Delta` as incremental ahead of this change.
- `mothergod::decompress_to_writer` now also streams
  `filters::select::Candidate::Bcj` frames (`research/JOURNAL.md` S1-P7,
  S2-A75, ROADMAP M4): a new `filters::bcj::Undo` type undoes the bcj
  transform as filtered bytes arrive, buffering up to 4 bytes while a
  candidate `call`/`jmp` instruction's operand is still incoming (lookahead,
  where `Delta`'s `Undo` needed lookback), so a `Method::Lz` frame whose
  encoder picked `Bcj` now bounds resident memory to `lz::WINDOW` too. Only
  `Transpose` still falls back to a whole-buffer decode, by design
  (`research/JOURNAL.md` S2-D4): its column-major write order needs the
  whole buffer resident regardless of how the rest of decode is built.
  `decodes_incrementally`'s answer is unaffected, since it already reported
  `Bcj` as incremental ahead of this change.

### Changed

- The `mothergod` CLI's `decompress` subcommand (ROADMAP M4/M6) now writes
  its output incrementally via `mothergod::decompress_to_writer` instead of
  buffering the whole decoded output in a `Vec<u8>` before writing it,
  bounding resident memory on the output side for the binary itself, not
  just the library — closing part of M6's CLI item's named remaining scope
  ("streaming I/O"). `compress` is unchanged: the optimal-parse encoder
  needs the whole input regardless, so there is nothing to stream there
  yet, and input is still read whole into memory on both sides — the
  library has a streaming writer, not yet a streaming reader. A decode
  failure partway through a file-argument run still removes the partial
  output file, the same cleanup `write_new_file` already did for a
  mid-write I/O failure. To stdout there is no equivalent mitigation: a
  decode failure now surfaces after whatever prefix `decompress_to_writer`
  already wrote, so a pipeline consuming incrementally (`| tar x`) can see
  truncated bytes ahead of the nonzero exit code, where the old whole-buffer
  decode guaranteed zero bytes reached stdout before an error. Bytes already
  written to stdout cannot be unwritten.

- `Error::TooLarge(u32)` is now `Error::TooLarge { len: u32, max: u32 }`
  (`research/JOURNAL.md` S2-A71): `decompress_bounded`'s caller-supplied
  `max_len` means the bound a `TooLarge` error names is no longer always
  `codec::MAX_DECODED_LEN`, so the error now carries the bound it actually
  violated instead of the `Display` impl assuming that constant. Source-
  level break, no `FORMAT_VERSION` bump (the wire format carries no error
  values). `codec::decode` also now clamps its own `max_len` parameter to
  `MAX_DECODED_LEN` internally rather than trusting every caller to have
  clamped first: it is `#[doc(hidden)]` but still `pub`, reachable
  directly by any crate depending on this one.

- `docs/format/SPEC.md` is now stable, frozen at `FORMAT_VERSION` 3
  (ADR-0041, ROADMAP M4, closing `research/JOURNAL.md` S1-P7): every
  version it documents (2, 3) decodes forever. CLAUDE.md hard rule 5's
  "unless an ADR drops one" carve-out no longer applies to version 2 or
  later; format evolution continues only by adding a new version. No
  `FORMAT_VERSION` bump, no code change.

- `literal::Literal::ideal_cost_bits_column_expert_pair` and
  `codec::ideal_cost_bits_column_expert_experiment`
  (`research/JOURNAL.md` S1-P5, S2-A69, ROADMAP M3's fifth standing
  lead): before-wiring measurement of whether a column-index-keyed
  literal expert helps blended into the shipped six-expert mix as a
  seventh, rather than replacing it (S2-R8 falsified the replacement).
  Prices every literal byte twice from the same pre-update six-expert
  state, so the six real experts adapt on their one real trajectory
  regardless of whether this path runs; the new `ColumnExpertState`
  adapts a column-keyed bank and its own single mixing weight on an
  independent trajectory. Deliberately pre-SSE, a separable question for
  the real wiring slice. Not wired into `Method`/`FORMAT_VERSION`:
  measurement only, no bpb change to the shipped codec, covered by
  focused unit tests. `Literal::update`'s inline frequency-rescale loop
  is now the shared `rescale_bank` free function this new method also
  uses, behavior-preserving.

- `bench`'s held-out-final reports (`docs/benchmarks/canterbury.md`,
  `silesia.md`) now carry `mothergod encode MB/s` and `mothergod decode
  MB/s` columns, wall-clock, single-thread, measured on the same run and
  the same bytes as the existing size columns (issue #364, ROADMAP SPEED
  scorecard). Decode timing also round-trips `mothergod::decompress` over
  every measured file, catching a corpus round-trip failure as a measurement
  error instead of silently reporting on unverified bytes.
  Both `docs/benchmarks/canterbury.md` and `silesia.md` are regenerated
  with real numbers (issue #366). Silesia's full-corpus run is not a
  single-turn operation on ordinary hardware: serial per-file fetch alone
  ran past 6m48s for 10 of 12 files in one attempt, before `measure_all`'s
  parallel compress/decompress even starts (`research/JOURNAL.md` S2-A68).

### Changed

- Library public API surface trimmed for 0.1 (ROADMAP M6): `bittree`,
  `codec`, `coder`, `column`, `literal`, `lz`, `model`, `ppm`, and `sse` are
  now `#[doc(hidden)]`. They stay `pub` (the `bench` crate depends on them
  by path for measurement, `mothergod::compress`/`decompress`), just off the
  rendered docs. `filters` stays fully documented (`research/JOURNAL.md`
  S2-A2 already judged it a standalone library surface in its own right).
  Downstream crates now see `MAGIC`, `FORMAT_VERSION`, `Method`, `Error`,
  `compress`, `decompress`, and `filters` on docs.rs — the crate's actual
  contract — instead of every internal module the M1 port left `pub`
  in-progress. Added a crate-root doctest (`src/lib.rs`) showing a
  compress/decompress round trip, ROADMAP M6's "examples in rustdoc" item.
  No behavior change; `decompress(compress(x)) == x` and every existing
  test is untouched.

### Added

- `mothergod` CLI binary (`src/bin/mothergod.rs`, ROADMAP M6): `compress`
  and `decompress` subcommands, mirroring `gzip -c`/`zstd -c`'s shape. With
  no file argument, reads stdin and writes stdout. With a file argument,
  follows the `.mgdc` suffix convention `tests/golden/` already uses:
  `compress FILE` writes `FILE.mgdc`, `decompress FILE.mgdc` writes `FILE`
  (suffix stripped); neither ever deletes its input, and both refuse to
  overwrite an existing output file. Streaming I/O is still follow-on scope
  (buffers the whole input, matching `mothergod::compress`/`decompress`'s
  own whole-buffer signatures). Zero new dependencies. Covered by
  `tests/cli.rs`, driving the compiled binary as a real subprocess:
  round-trip on arbitrary bytes and on empty input, a file-argument
  round-trip, refusing to clobber an existing output file, a clean failure
  decompressing a file without the `.mgdc` suffix, clean non-zero exit (no
  panic) on truncated/garbage input, and usage/help handling.

- `lz::parse_greedy_with_window` (`research/JOURNAL.md` S1-P4, S2-A65,
  ROADMAP M3's fourth standing lead): closes S2-A63's own remaining-scope
  note that `parse_greedy`'s hash-chain `MatchFinder` was "still hardcoded
  to WINDOW, unexamined" — the one match finder on the wired path not yet
  parameterized (S2-A61 did `BinaryTreeMatchFinder`, S2-A63 did
  `parse_optimal_with_window`). `parse_greedy` is now a thin wrapper
  passing the wired `WINDOW` unchanged, mirroring
  `parse_optimal`/`parse_optimal_with_window`'s own split — no effect on
  any currently-encoded bitstream, no `FORMAT_VERSION` bump.

- `column::column_of` (`research/JOURNAL.md` S1-P5, S2-A64, ROADMAP M3's
  fifth standing lead): given a position in `filters::transpose::encode`'s
  output, the pre-transpose data length, and the column count, returns
  which column produced that byte — the arithmetic a future column-index-
  keyed literal context needs, verified against an independent replay of
  `transpose::encode`'s own grouping loop rather than asserted from the
  closed form alone. Standalone primitive, not yet wired into
  `literal::Literal` or `codec.rs`: no bpb change, covered by focused unit
  tests only.

- `column::column_bank` (`research/JOURNAL.md` S1-P5, S2-A66): wraps
  `column_of`'s unbounded column index into a fixed-size bank space
  (`column % max_banks`), so a future column-index-keyed expert can size
  its storage from a constant instead of the frame's declared `columns` —
  a decoder reads `columns` from untrusted compressed input, so sizing
  bank storage to it directly would risk unbounded allocation on a
  hostile frame (CLAUDE.md hard rule 2). Same fixed-bank convention
  `literal.rs`'s existing experts already use. Standalone primitive, not
  yet wired into `literal::Literal` or `codec.rs`: no bpb change, covered
  by focused unit tests only.

- SSE wired into the literal mixer's binary decomposition
  (`research/JOURNAL.md` S1-P1 closed, ADR-0038, `FORMAT_VERSION` 2 → 3):
  `literal::Literal::encode_sse`/`decode_sse` code every literal byte as 8
  chained binary decisions (`bittree::encode_symbol_sse`/`decode_symbol_sse`),
  each calibrated by an `sse::Sse` table keyed on tree position
  (`bittree::sse_context`, 255 contexts) before it drives the range coder.
  `codec::decode` dispatches on the frame's declared version: below 3, the
  old direct 256-way `Literal::decode`; 3 and above, `decode_sse` —
  `tests/golden/v2-lz-repeated-text.mgdc` still decodes, unchanged, and a
  new `tests/golden/v3-lz-repeated-text` pair pins the new shape. Measured
  on `bench::baseline`'s 11 train cases and the two sealed-only kinds:
  net train -0.36736 bits/byte (`interleaved_audio16` -0.36368 carried
  most of it), both sealed kinds improved (`access_log` -0.01264,
  `gradient_image` -0.13472). One case regressed past `TOLERANCE_BITS`:
  `entropy_ladder_h6` +0.02368 (iid random data — SSE warm-up and the
  8-decision coding path's own overhead have no real bias to correct
  there), declared as an accepted trade per corpus policy's accept rule
  (train improvement, no validation regression); `bench/baseline.json`
  updated to the new numbers. A prior slice (S2-R1) rejected SSE behind
  the flag model's `is_copy` bit, a lone already-adaptive counter with no
  systematic bias to correct; this slice calibrates the literal mixer's
  own compound blended probability instead, the compound-estimate
  candidate S2-R1's postmortem named.

- `bittree::encode_symbol`/`decode_symbol` (`research/JOURNAL.md`
  S2-A58, S1-P1's own remaining-scope note): a standalone binary
  decomposition of a 256-symbol cumulative-frequency table into 8
  chained binary decisions, the shape `sse::Sse` needs to calibrate the
  literal mixer once wired (S2-R1's postmortem named this as the next
  SSE attempt's prerequisite, not another raw `Model` split). Not wired
  into `Literal` or `codec.rs` yet — standalone primitive only, no
  format or ratio effect.

- `bittree::sse_context` (`research/JOURNAL.md` S2-A59, S2-A58's own
  remaining-scope note): maps a bit-tree walk step (depth, decided
  prefix) to one of 255 `sse::Sse` calibration contexts, the classic
  LZMA-literal-coder node numbering. Picks S1-P1's context-keying
  question; still not wired into `Literal` or `codec.rs` — standalone
  primitive only, no format or ratio effect.

- `mothergod_bench::baseline::load_and_fingerprint`, a shared helper for
  the read-`bench/baseline.json`-parse-fingerprint sequence `finals_report`
  and `silesia_report` each pasted a copy of (issue #330: this exact block
  regrew once already after it95 first consolidated it). Encoder-side
  tooling only, no format or ratio effect.

- `src/ppm.rs`: standalone PPM-style escape primitive for ROADMAP M3's
  third standing lead (`research/JOURNAL.md` S1-P3, S2-A57) — an adaptive
  frequency table that starts every symbol unseen (frequency 0, unlike
  `Model`'s Laplace-smoothed 1) and prices "never observed in this
  context" as its own explicit escape event (PPM Method C), closing the
  gap S1-R4's near-miss diagnosis named. Not yet wired to `Literal` or
  `codec`: this slice builds and tests the primitive standalone first,
  the same order S1-P1's `Sse` (S2-A40) and S1-P2's binary-tree match
  finder (S2-A42) shipped in. No bpb change: private, unwired, and
  covered by focused unit tests only.

- `lz::BinaryTreeMatchFinder::new` takes a `window: usize` parameter
  instead of reading the crate-wide `lz::WINDOW` constant (1 MiB)
  directly (`research/JOURNAL.md` S1-P4, S2-A61, ROADMAP M3's fourth
  standing lead): several Silesia finals (`mozilla`, `nci`, `samba`,
  `sao`, `webster`) are many times larger than 1 MiB, so long-range
  repeats past that distance are currently invisible to the parse. The
  wired parse (`dp_round`) still always constructs its finder with
  `WINDOW` — bit-for-bit identical output, no format or ratio effect —
  this slice only frees the bound so a larger window is measurable on
  the standalone finder before any wiring or `FORMAT_VERSION` decision.

- `bench::long_range_repeat` (`research/JOURNAL.md` S1-P4, S2-A62): a new
  corpus generator that plants two byte-identical 4,096-byte blocks a
  caller-chosen distance apart, filling the rest with 6-bit-entropy noise.
  S2-A61 froze `lz::BinaryTreeMatchFinder`'s window bound but had no data
  shaped to measure a larger window against; this closes that gap. Not
  wired into `DatasetKind`/`bench::baseline`'s CI ratio gate (that gate's
  cases are 50,000 bytes, far below `lz::WINDOW`) — standalone, covered by
  focused unit tests only, no bpb change.

- `lz::parse_optimal_with_window`/`codec::ideal_cost_bits_with_window`
  (`research/JOURNAL.md` S1-P4, S2-A63): thread a `window: usize`
  parameter down through `dp_round` to `BinaryTreeMatchFinder`, S2-A61's
  parameterize-the-primitive pattern carried one level up the call
  stack. `parse_optimal`/`ideal_cost_bits` are now thin wrappers passing
  the wired `WINDOW` unchanged — no effect on any currently-encoded
  bitstream, no `FORMAT_VERSION` bump. Measured with `bench::
  long_range_repeat` (S2-A62): a window reaching a planted 4,096-byte
  repeat 150,000 bytes past `WINDOW` drops bits/byte by -0.0214 on both
  the train and sealed seeds, matching the expected magnitude of trading
  a literal run for one match token.

- `baseline_gate check` (the required `ratio` job's own binary) now also
  fails when `docs/benchmarks/canterbury.md` or `docs/benchmarks/
  silesia.md` embeds a `bench/baseline.json` fingerprint that no longer
  matches the committed file (issue #327, ROADMAP M2's dropped "by hand"
  gap): a baseline change signals the codec's measured behavior changed,
  so the held-out finals reports can now be stale. Caught real drift
  landing this: `canterbury.md` had gone stale since 2026-08-25,
  regenerated in this change alongside `silesia.md` (research/JOURNAL.md
  S2-A55).

- `lz::parse_optimal` now runs a third `dp_round` (`research/JOURNAL.md`
  S2-A56, closing S2-A9's own "not iterated to convergence" note): net
  -0.039 bits/byte on `bench/baseline.json`'s 11 train cases, one
  within-tolerance regression (`base64_wrapped` +0.00144), sealed-only
  `access_log` and `gradient_image` both improved, no validation
  regression — unlike every prior S1-P2 wiring attempt. Pure repetition
  of the existing reseed-from-backtrace step, not new DP machinery, so
  none of S2-R2/S2-R3's wiring risk applies; S1-P2's own named
  sqlite/json/jsonl target moved favorably but modestly and stays open.
  `bench/baseline.json`, `docs/benchmarks/baseline.{md,svg}`, and (issue
  #327's new fingerprint gate) `canterbury.md`/`silesia.md` all
  regenerated to match: Canterbury aggregate 1.382712 -> 1.381605 b/B,
  Silesia aggregate 2.069848 -> 2.068237 b/B, both a small real
  improvement in the same direction as train/sealed.
- `mothergod-bench`'s `silesia_report` binary (`research/JOURNAL.md`
  S2-D1/S2-A45's remaining "real Silesia finals numbers" line): Silesia's
  counterpart to `finals_report`, fetching each of Silesia's 12
  individually pinned files and writing `docs/benchmarks/silesia.md`.
  `finals::format_report` now takes the generator binary name as a
  parameter (shared by `finals_report` and `silesia_report`) instead of
  hardcoding `finals_report`; `bench`'s duplicated `repo_root`/`date`-
  timestamp helpers across its binaries are consolidated into
  `mothergod_bench::repo_root` and `mothergod_bench::reference::generated_at`.
- `docs/benchmarks/silesia.md`, mothergod's first real Silesia numbers
  (`research/JOURNAL.md` S2-A52's remaining scope, ROADMAP M2): aggregate
  2.069848 bits/byte vs zstd -19's 1.996629 and xz -9e's 1.829058 across
  the 12-file corpus, mothergod ahead of both on `ooffice` alone. Landed
  by `mothergod_bench::reference::measure_all`, a new shared helper
  (`finals_report` and `silesia_report` both call it now instead of each
  looping over its files in-process) that runs one OS thread per file:
  Silesia's full run is throughput-bound at ~0.14 MB/s single-threaded
  (`finals_report`'s module doc has the per-file measurement), on the
  order of half an hour end to end serially, too slow for one PR's
  by-hand run; parallel across files, this run finished in 8m20s wall
  clock (22m15s total CPU) on 4 cores.

- `lz::PriceCounts::observe` (`research/JOURNAL.md` S1-P2, S2-A50):
  standalone primitive that bumps one already-decided token's frequency
  counts, factored out of `PriceCounts::tally` (which now calls it in a
  loop, behavior unchanged). `tally` only ever replays a *complete* token
  sequence; closing S1-P2's other named gap (the DP's price table is
  frozen per round, not adaptive per position) needs `dp_round`'s forward
  pass to feed its own already-finalized moves into a running table as it
  advances, which `tally` cannot do. Not yet called from `dp_round`, the
  same standalone-primitive-first order `BinaryTreeMatchFinder` (S2-A42)
  and `Sse` (S2-A40) shipped in. No bpb change: private, unwired, and
  `tally`'s refactor is proven behavior-preserving by a dedicated test.

- `lz::parse_optimal`'s DP now searches matches with `BinaryTreeMatchFinder`
  instead of the hash-chain `MatchFinder` (`research/JOURNAL.md` S1-P2,
  S2-A48): -0.05376 bits/byte net on `bench/baseline.json`'s 11 train
  cases, no case regressed beyond `TOLERANCE_BITS`, sealed-only `access_log`
  -0.00016 and `gradient_image` unchanged. Twice rejected before this
  (S2-R2, then S2-A47) on the issue #179 speed guard and, once that was
  fixed, on process — `tests/golden.rs` demanded a `FORMAT_VERSION` bump
  for an encoder-only change; issue #290's ruling removed that requirement,
  so this slice is the identical wiring, now legitimately landed with the
  current-version golden fixture regenerated instead. Not yet a win on
  S1-P2's actual sqlite/json/jsonl target: window eviction and
  per-position adaptive prices remain.

- Agent telemetry page at [mothergod.dev/agents.html](https://mothergod.dev/agents.html)
  (issue #64): per-seat run economics (output tokens, turns, minutes,
  denials, errors) for the last 7 days with prior-week trend, plus the 25
  most recent runs, aggregated daily from the public audit artifacts.
  Metadata only, no model-written prose; USD deliberately absent because
  the subscription-auth figure is notional, not a bill.

- `corpus-fetch-check` scheduled workflow (`research/JOURNAL.md` S2-A45,
  issue #231): weekly CI coverage for `bench`'s off-by-default
  `corpus-fetch` feature, running clippy, tests with `--include-ignored`
  (so the real-network `bench/corpus.toml` pin smoke test finally
  executes), and doc. Closes the zero-coverage hole the feature gate
  left: a stale corpus pin now surfaces on Sunday's scheduled run
  instead of mid-experiment.

- `lz::BinaryTreeMatchFinder` (`research/JOURNAL.md` S1-P2, S2-A42):
  standalone binary-tree match finder — insertion keeps each hash bucket
  as a binary search tree ordered by candidate suffix bytes (LZMA's bt4
  shape) instead of `MatchFinder`'s newest-first hash chain, so one
  downward walk finds the exact longest match among the candidates on
  the insertion path. Not yet wired into `parse_greedy` or
  `parse_optimal`, the same standalone-primitive-first order S1-P1's
  `Sse` shipped in (S2-A40). Length-prefix reuse (`len0`/`len1`, S2-A43)
  since: each comparison starts from the shorter of the two common
  lengths already proven on the "less"/"greater" chains rather than
  byte 0 — a real ~3.5x on near-duplicate structured data, though it
  does not by itself fix the single-repeated-byte pathology that made
  the S2-R2 wiring attempt break the issue #179 speed guard. A `nice_len`
  early exit (S2-A44) since: the walk stops visiting further candidates
  once the best match found so far is at least `nice_len` long, cutting
  candidate count rather than per-candidate cost — a real ~3.3x on the
  same near-duplicate shape, compounding with length-prefix reuse, but
  still no effect on the single-repeated-byte pathology (measured, not
  assumed, before landing). `nice_len` now also bounds each candidate's
  own suffix comparison (S2-A46), not just how many candidates get
  visited: `suffix_common_len` gained a `limit` parameter, so a single
  candidate can never cost more than `O(nice_len)` regardless of how long
  its true common run is, at the cost of reporting that candidate's match
  as exactly `nice_len` long when its true length is longer. This closes
  the issue #179 pathology the two prior slices measured and could not
  fix: the same 200,000-byte single-repeated-byte fixture that took 32.9s
  with `nice_len` alone now completes in about 0.1s with `nice_len` 128
  (measured directly against the standalone finder, not yet through
  `dp_round`, which still uses `MatchFinder`'s hash chain).

- `src/sse.rs`: standalone secondary symbol estimation (SSE/APM) primitive
  for ROADMAP M3's oldest standing lead (`research/JOURNAL.md` S1-P1,
  S2-A40) — an adaptive, per-context probability calibration table, in
  linear-domain bins rather than the classic logit-domain ones so it needs
  no libm transcendental (`clippy.toml`, ADR-0024). Not yet wired to
  `codec`: no binary probability stream exists yet to calibrate, so this
  slice builds and tests the primitive standalone first, the same order
  every M1 filter and LZ slice shipped in.

- `Encoder::encode_bit`/`Decoder::decode_bit` (`research/JOURNAL.md`
  S1-P1, S2-A41): code one bit at an arbitrary caller-supplied
  probability, the second prerequisite (after `Sse`, S2-A40) an
  `Sse`-calibrated binary decision needs before it can be wired into
  `codec`. Proven together with `Sse` in a new integration test; still
  not wired to any bitstream.

- `tests/golden/`: golden-frame regression tests (ROADMAP M4,
  `docs/TESTING.md` layer 5, `research/JOURNAL.md` S2-A39). Pins a real
  `FORMAT_VERSION` 2 frame; `decompress` matching the pinned plaintext is
  a real cross-platform guarantee (decode is integer-only), re-encoding
  matching the pinned frame is a same-toolchain regression pin only
  (encoder-only `f64::log2` calls in `lz.rs`/`filters.rs`, per
  `docs/adr/0024-no-libm-on-the-decode-path.md`). `docs/TESTING.md`
  corrected to stop claiming untested cross-platform byte-identity.

- Ideal-cost accounting mode, closing slice (`research/JOURNAL.md` S2-A38,
  ADR-0006): `codec::ideal_cost_bits` sums the whole-codec ideal coding cost
  of a file, the flag/length/offset/slot streams and literal bytes together,
  by pricing the same `lz::parse_optimal` token stream `codec::encode_tokens`
  would encode through `Model::ideal_cost_bits` and `Literal::ideal_cost_bits`
  instead of a real `Encoder` — closes the whole-codec pass S2-A30 and
  S2-A31 each flagged as remaining scope.
- `docs/benchmarks/`: mothergod's bits/byte on `bench/baseline.json`'s fixed
  cases, rendered as a static SVG bar chart (`baseline.svg`) plus a markdown
  table (`baseline.md`), generated together by a new `render_baseline_graph`
  binary (`cargo run -p mothergod-bench --release --bin
  render_baseline_graph`) so they can't drift apart. Own generator corpus
  only, mothergod against itself (`research/JOURNAL.md` S2-A36) — see the
  next entry for the real, named-corpus comparison.
- `docs/benchmarks/canterbury.md`: mothergod's first real bits/byte numbers
  against pinned reference compressors on a named held-out final
  (`research/corpus/POLICY.md`'s Canterbury corpus): `gzip -9`, `zstd -19`,
  `xz -9e`, per file and aggregated. Aggregate: mothergod 1.380218 bits/byte
  vs zstd -19's 1.469771 and xz -9e's 1.403395 — ahead of the stronger
  reference on this corpus, though not yet on every file. Generated by a new
  `finals_report` binary (`cargo run -p mothergod-bench --release --features
  corpus-fetch --bin finals_report`); Silesia is out of scope for this slice
  (throughput, not missing code — `research/JOURNAL.md` S2-A37).
- `site/status.html`: a live project status page (issue #95) rendering
  `site/status-data.json` — milestone bar (from ROADMAP.md's own checkboxes),
  experiment ledger and 7-day flow stats (from `research/progress.jsonl` and
  `git log`), and an honest "not yet measurable" benchmarks note until
  ROADMAP M2 lands real Silesia/Canterbury numbers. `site-status/`, a new
  workspace crate, generates the snapshot (`cargo run -p mothergod-site-status
  --release --bin generate`); the scheduled workflow that reruns it
  automatically is remaining scope (`research/JOURNAL.md` S2-D1), since
  wiring a new `.github/workflows/` file needs `GH_ADMIN_TOKEN`.
- `cargo x`, an agent-facing repository quality command with embedded formatters
  for Rust, JSON/JSONL, TOML, YAML, JavaScript/TypeScript, HTML, and SVG, plus
  Clippy and Markdown linting. Tasks expose conventional help, path-scoped
  checks, actionable diagnostics, safe Markdown fixes, stable exit codes, and
  an independently cached CI binary.
- `cargo x test` (ADR-0029, first of four steps): wraps `cargo test
  --all-targets`, `cargo test --manifest-path x/Cargo.toml`, and
  `cargo test --doc` as one fixed plan, stopping at the first failing suite
  and naming the command to re-run just that one.
- `cargo x doc` (ADR-0029, second of four steps): wraps
  `RUSTDOCFLAGS="--deny warnings" cargo doc --no-deps`, the CLAUDE.md doc
  gate, naming the command to re-run on failure.
- `cargo x check` (ADR-0029, third of four steps): the umbrella gate running
  `fmt --check`, `lint`, `test`, then `doc`, in order, stopping at the first
  failing stage and naming the command to re-run just that one. CLAUDE.md's
  Commands block collapses to this one command.
- Benchmark harness, first slice (`research/JOURNAL.md` S2-A1): a new
  `bench/` workspace crate with the two mandatory corpus generators from
  `research/corpus/POLICY.md` ported to Rust — `entropy_ladder` (iid bytes
  at a chosen order-0 entropy) and `markov_h8_2_trap` (uniform histogram,
  low conditional entropy). Core `mothergod` crate stays zero-dependency.
- Codec port, first slice (`research/JOURNAL.md` S2-A2): `filters` module
  with a fixed-stride delta filter, ported from the founding session's
  archived codec. Not yet wired to a compression `Method` — the LZ, model,
  and coder modules it will sit behind are still to come.
- Codec port, second filter slice (`research/JOURNAL.md` S2-A3): a
  row-major-to-column-major transpose filter, ported from the founding
  session's archived codec. `filters` is now submodules (`delta`,
  `transpose`) to keep each filter's `encode`/`decode` pair namespaced.
- Codec port, third filter slice (`research/JOURNAL.md` S2-A4): the x86
  call/jmp (BCJ) filter, ported from the founding session's archived
  codec as a `bcj` submodule of `filters`.
- Codec port, fourth filter slice (`research/JOURNAL.md` S2-A5): the
  base64-unwrap filter, ported from the founding session's archived
  codec as a `base64_unwrap` submodule of `filters`. Unlike the earlier
  filters, unwrapping is a data-dependent decision rather than a
  caller-supplied parameter, so `encode` prefixes a one-byte flag that
  `decode` reads back; supported by a new zero-dependency standard
  base64 encode/strict-decode pair (no base64 crate, per ADR-0002).
- Codec port, fifth filter slice (`research/JOURNAL.md` S2-A6): a
  byte-order reversal filter, ported from the founding session's
  archived codec as a `reverse` submodule of `filters`. Self-inverse
  (`encode` and `decode` are the same operation), covering M1's
  filter-bank checklist in full: `pick_filters`, the LZ, model, and
  coder modules remain.
- Codec port, trial-selection slice (`research/JOURNAL.md` S2-A7): a
  `filters::select` submodule with a `pick` function that shortlists
  which filters (delta, BCJ, transpose) are worth a full trial encode
  against a given input, using an order-1 entropy proxy on a bounded
  probe. Ported from the archive's `pick_filters`. Not yet called by
  anything — the LZ, model, and coder modules it will feed remain.
- Deslopper agent (ADR-0016): a fifth agent seat that removes slop from
  `src/` twice daily without changing observable behaviour, one scope per
  PR, approved by the reviewer like any other agent PR. Its taxonomy and
  scope rule ship as a Claude Code skill at `.claude/skills/deslop/`.
- Real-time operator wake (issue #5): Telegram messages hit a Cloudflare
  Worker at `bot.mothergod.dev` that stores them in KV and dispatches
  the BDFL within seconds, replacing the per-run `getUpdates` poll.
- Mechanical Telegram commands for immediate project operations and reads:
  `/help`, `/status`, `/pause`, `/resume`, `/run`, `/budget`, `/runs`,
  `/blocked`, `/diff`, `/agents`, and `/digest`. Slash commands never wake
  the BDFL; ordinary prose keeps the existing KV-to-BDFL route.
- Codec port, LZ slice one (`research/JOURNAL.md` S2-A8): a new `lz`
  module with the greedy/lazy parser (`Token`, `parse_greedy`), ported
  from the archive's `lz` function, plus `replay`, the token-stream
  inverse that proves it losslessly reversible ahead of the entropy
  coder that will eventually consume it. The archive's DP-priced
  optimal parse (`lz_opt`) is a follow-up slice; it runs this parser
  internally as its price-seeding first pass.
- Codec port, LZ slice two (`research/JOURNAL.md` S2-A9): `lz::parse_optimal`,
  the archive's DP-priced optimal parse, seeded by `parse_greedy` and
  costed against a lightweight frequency-table price model (no real
  entropy coder exists yet to price against). One deliberate correctness
  fix over the archive: this parse's internal repeat-offset-cache
  bookkeeping always matches `replay`'s, closing a round-trip hazard
  present in the archive's own DP (see the journal entry for the
  mechanism).
- Codec port, first coder slice (`research/JOURNAL.md` S2-A10): a new
  `coder` module with `Encoder`/`Decoder`, the adaptive range coder
  ported from the archive's `Enc`/`Dec`. Driven directly by
  caller-supplied cumulative-frequency ranges; the adaptive frequency
  tables (the archive's `Model` and the six-expert `Lit` mixer) that
  will supply those ranges are the next slice.
- Codec port, first entropy-model slice (`research/JOURNAL.md` S2-A11):
  a new `model` module with `Model`, the order-0 adaptive frequency
  table ported from the archive, driving `coder::Encoder`/`Decoder`
  with real data-derived cumulative-frequency ranges. The flag/length/
  offset stages of the entropy coder will each be one `Model` instance;
  the six-expert `Lit` literal mixer remains a separate, larger slice.
- Codec port, second entropy-model slice (`research/JOURNAL.md` S2-A12):
  a new `literal` module with `Literal`, the six-expert context-mixing
  literal model ported from the archive's `Lit`. Blends a two-rate
  order-1 pair, order-0, order-2, an alignment hash, and a word hash
  under gradient-derived mixing weights. Not yet wired to a `Method`
  variant; carries a known open question (`research/JOURNAL.md` S2-D3)
  about the archive's continued use of `f64` in weight adaptation versus
  the integer-only path `JOURNAL` S1-A5 records as accepted, to resolve
  before the Method-wiring PR that will need an ADR and `FORMAT_VERSION`
  bump anyway.
- `#![forbid(unsafe_code)]` on the `mothergod` crate root (issue #76): no
  `unsafe` exists in `src/` today, so the gate costs nothing and closes
  that door permanently.
- Adversarial decode seed corpus (ROADMAP M2, `docs/TESTING.md` layer 2):
  a new `tests/adversarial/` directory of tiny fixtures built to be
  invalid (header truncations at every byte boundary, bit-flipped magic,
  a future format version, an unknown method) and `tests/adversarial.rs`,
  which asserts every fixture decodes to a graceful `Err`, never a panic.
  Runs on every PR; future fuzz-found crashers (M4) promote into this
  directory as regression seeds.
- Benchmark harness, first structured-generator slice (`research/JOURNAL.md`
  S2-A14): `access_log` in the `bench` crate, synthetic web-server access
  log lines (the "jsonl/log records" class in
  `research/corpus/POLICY.md`), ported from the founding session's
  `corpus.py`. Produces exactly the requested byte length from a small
  IP/path/status pool via the existing deterministic `Rng`.
- Benchmark harness, second structured-generator slice (`research/JOURNAL.md`
  S2-A15): `json_records` in the `bench` crate, a synthetic JSON API
  response (the "json" class in `research/corpus/POLICY.md`), ported from
  the founding session's `corpus.py`. Records carry a gaussian `score`
  (Box-Muller, mean 50, stddev 15) and an `active` field true 80% of the
  time; generates records until the requested byte length is reached,
  same deviation as `access_log`.
- `clippy.toml` with `disallowed-methods` covering the float transcendental
  family (`exp`, `ln`, `log2`, `log10`, `powf`, `powi`, `sin`, `cos`,
  `tan`, `f32`/`f64`), enforcing ADR-0024: nothing on the decode path may
  call a libm function, since implementations can disagree in the last
  ulp and desync an encoder and decoder mid-frame. Scoped to `src/` only
  (`bench/clippy.toml` overrides it back off for the corpus-generation
  crate, which never touches a bitstream).
- `Method::Lz` (`research/JOURNAL.md` S2-A17, ADR-0026, `FORMAT_VERSION`
  0 → 1): the first real compression method, wiring the already-ported
  `lz`, `model`, `literal`, and `coder` modules together — optimal-parse
  LZ tokens, entropy-coded by adaptive flag/length/offset/rep-slot models
  and a six-expert context-mixing literal model, over an adaptive range
  coder. `compress` now tries `Method::Lz` and falls back to
  `Method::Stored` whenever that is not smaller. Decode bounds allocation
  and loop iterations to the payload's own declared output length rather
  than trusting it, and rejects a corrupt match/rep distance or a
  declared-length mismatch as an error rather than panicking. The declared
  output length itself is capped at 256 MiB (`codec::MAX_DECODED_LEN`,
  new `Error::TooLarge`), checked before any decode work: without it, a
  payload where the declared length and token count agree with each
  other (both attacker-chosen, unrelated to the bytes actually sent)
  could force multi-minute, multi-gigabyte decode work from a
  double-digit-byte input. Filter selection is not wired in yet
  (`Method::Lz` always runs on raw input); that remains open M1 scope.
  Measured 2.318 bits/byte on
  `research/imports/session-1/mothergod.rs` (25,524 bytes), against
  `gzip -9`'s 2.392 bits/byte on the same file.
- Filter selection wired into `Method::Lz` (`research/JOURNAL.md` S2-D2,
  in full; ADR-0028, `FORMAT_VERSION` 1 → 2): `compress` now trials every
  candidate filter `filters::select::pick` shortlists (delta, BCJ,
  transpose, or none) against the real LZ + context-mixing pipeline and
  keeps whichever produces the smallest frame, closing M1's last open
  checklist item. The winning filter is a 2-byte selector prefixed onto
  the payload (`[kind, param]`); an unrecognized selector, or a zero
  `param` on a filter that requires one, decodes to `Error::Corrupt`
  rather than being parsed. A version-1 frame's `Method::Lz` payload used
  a layout this build no longer understands (no filter prefix), so
  `decompress` rejects that combination as `Error::UnsupportedVersion`
  explicitly (`codec::LZ_MIN_VERSION`) rather than misreading it.
  Measured 2.3184 bits/byte on the same named corpus as the entry above
  (unchanged from 2.318: this file is structured text, and `Candidate::
  Identity` wins — `JOURNAL` S1-R1 already predicted delta loses on
  text); a synthetic columnar-drift round-trip test proves the wiring
  picks and correctly reverses a non-identity filter end to end.
- Benchmark harness, third structured-generator slice (`research/JOURNAL.md`
  S2-A20): `base64_wrapped` in the `bench` crate, a base64-wrapped text
  payload (the "base64-wrapped payloads" class in
  `research/corpus/POLICY.md`), ported from the founding session's
  `corpus.py`. Wraps `json_records` output in a new standalone
  `base64_encode` helper (RFC 4648, zero-dependency) and truncates to the
  requested length.
- Benchmark harness, fourth structured-generator slice (`research/JOURNAL.md`
  S2-A21): `interleaved_audio16` in the `bench` crate, interleaved 16-bit
  audio samples (the "audio" class in `research/corpus/POLICY.md`), ported
  from the founding session's `corpus.py`. Each sample sums a slow and a
  fast sine wave plus gaussian noise, truncated toward zero and wrapped to
  16 bits, matching Python's `int(...) & 0xffff`.
- Benchmark harness, fifth structured-generator slice (`research/JOURNAL.md`
  S2-A22): `gradient_image` in the `bench` crate, a synthetic grayscale
  gradient image (the "gradient image" class in
  `research/corpus/POLICY.md`), ported from the founding session's
  `corpus.py`. Row-major pixels over 200-pixel-wide rows, each a baseline
  plus a horizontal and a vertical sine wave plus gaussian noise, truncated
  toward zero and wrapped to a byte, matching Python's `int(...) & 0xff`.
- Benchmark harness, sixth structured-generator slice (`research/JOURNAL.md`
  S2-A23): `sqlite_like_records` in the `bench` crate, fixed-width binary
  rows over a timestamp/category/measurement schema (the "sqlite-like
  records" class in `research/corpus/POLICY.md`), ported from the founding
  session's `corpus.py`. Unlike the earlier structured classes, the
  archive's byte layout came from a real `sqlite3` file, not a formula, so
  this port captures the schema's shape as fixed 20-byte little-endian rows
  instead of reimplementing SQLite's on-disk format.
- Benchmark harness, seventh and final structured-generator slice
  (`research/JOURNAL.md` S2-A24): `x86_dense_code` in the `bench` crate, a
  synthetic x86-64 instruction stream dense with `call`/`jmp rel32`
  opcodes (the "x86-dense binaries" class in `research/corpus/POLICY.md`).
  The archive's source (a slice of the host's `libc.so.6`) is neither
  deterministic nor available in every environment, so this port
  substitutes a synthetic instruction stream built to stress the `bcj`
  filter (S2-A4) instead: short filler instructions interleaved with
  `call`/`jmp` opcodes targeting a small pool of synthetic function starts.
  Completes S2-D1's structured-generator list; Silesia/Canterbury
  fetch-and-cache and the train/sealed split plumbing remain.
- Fuzz targets (`research/JOURNAL.md` S2-A25, issue #53): a new `fuzz/`
  crate (`cargo-fuzz`, dev-only, its own nightly toolchain — the
  sanitizer-coverage instrumentation `cargo-fuzz` needs is nightly-only)
  with two targets against the real codec: `decode_arbitrary` (hard rule
  2 as an executable — `decompress` must not panic or overallocate on
  arbitrary bytes) and `roundtrip` (hard rule 1 as an executable —
  `decompress(compress(x)) == x`). Not wired into any required check;
  the scheduled smoke run landed later as `fuzz-check.yml` (S2-A53,
  below), a dedicated workflow rather than a `monster.yml` lane.
- Silesia/Canterbury fetch-and-cache (`research/JOURNAL.md` S2-A26,
  `research/corpus/POLICY.md`): `bench/corpus.toml` pins all 12 Silesia
  files and the Canterbury tarball by URL + SHA-256, and a new
  `bench::corpus` module (opt-in `corpus-fetch` Cargo feature, off by
  default) fetches, verifies, and disk-caches them, refusing a checksum
  mismatch. The feature gate keeps `ureq`/`sha2` — needed for the HTTPS
  fetch and the integrity check, out of scope for core `mothergod`'s
  zero-dependency rule (ADR-0002) but still a real cost to every PR's
  required `cargo test --all-targets` as an unconditional dependency of a
  default workspace member — out of the fast gate's build graph.
- Corpus decompression (`research/JOURNAL.md` S2-A28): `bench::corpus::decompress_silesia`
  (bzip2, via the decode-only `bzip2-rs`) and `bench::corpus::extract_canterbury`
  (gzip+tar, via `flate2`'s pure-Rust backend and `tar`) turn a fetched
  entry's compressed bytes into the raw corpus file(s), folded into the
  same opt-in `corpus-fetch` feature gate as the fetch-and-cache slice.
  Remaining S1-D2 scope: the train/sealed/finals split plumbing, regret
  scoring, the CI baseline gate, and progress-graph rendering.
- Train/sealed split plumbing, first slice (`research/JOURNAL.md` S2-A29):
  `bench::train_window` takes a rotating, circularly-wrapping window of a
  generator's output keyed by an iteration counter, so repeated experiment
  iterations see a different offset instead of memorizing one
  (`research/corpus/POLICY.md`, "Train slices"). Sealed-validation seed and
  dataset-kind separation remain.
- Ideal-cost accounting mode, first slice (`research/JOURNAL.md` S2-A30,
  ADR-0006): `Model::ideal_cost_bits` sums `-log2(p)` against the
  order-0 flag/length/offset model's live adaptive state instead of
  driving a real `Encoder`, the Rust-native replacement for the
  founding session's Python model-cost proxy. `Literal`'s six-expert
  mixer gets the same method in a follow-up slice.
- Ideal-cost accounting mode, second slice (`research/JOURNAL.md`
  S2-A31, ADR-0006): `Literal::ideal_cost_bits` is `Model::ideal_cost_bits`'s
  counterpart for the six-expert literal mixer, pricing a byte against
  the same mixed distribution `Literal::encode` codes against without
  touching an `Encoder`. Both entropy stages now support ideal-cost
  accounting; a whole-codec pass summing them together landed in S2-A38,
  above.
- Train/sealed split plumbing, seed half (`research/JOURNAL.md` S2-A32):
  `bench::sealed_seed` derives a sealed-validation seed from a train seed
  (`research/corpus/POLICY.md`, "different seed... from train"), distinct
  from it and injective across the seed space. Which dataset kinds are
  sealed-only remains.
- Train/sealed split plumbing, dataset-kind half (`research/JOURNAL.md`
  S2-A33): `bench::DatasetKind` enumerates the nine corpus generators and
  `DatasetKind::sealed_only` designates `AccessLog` and `GradientImage`
  sealed-validation-only (`research/corpus/POLICY.md`, "held-out dataset
  kinds") — neither has a filter in `src/filters.rs` whose documented
  purpose matches its shape, so they measure generalization undiluted by
  a filter tuned for exactly their shape. Regret scoring, the CI
  baseline gate, and progress-graph rendering remain.
- Regret scoring (`research/JOURNAL.md` S2-A34): `bench::regret` scores a
  candidate corpus addition (`research/corpus/POLICY.md`, "Growing the
  corpus") as mothergod's bits/byte minus the stronger of the two pinned
  reference compressors' (`zstd -19`, `xz -9e`) bits/byte on the same
  data. Positive regret is the accept criterion; pure noise needs no
  separate auto-reject case since every compressor is equally bad at it,
  so regret already comes out near zero. Not yet called by anything — it
  exists for the CI baseline gate to consult once it exists. The CI
  baseline gate and progress-graph rendering remain.
- CI baseline gate, measurement half (`research/JOURNAL.md` S2-A35,
  ROADMAP M2): a new `bench::baseline` module measures mothergod's
  bits/byte on eleven fixed-seed, fixed-length cases (the full entropy
  ladder plus every train-eligible `DatasetKind`, sealed-only kinds
  excluded so a PR-time gate never tunes against the sealed set) and
  compares against the committed `bench/baseline.json`, initial values
  measured on today's codec. `cargo run -p mothergod-bench --release
  --bin baseline_gate -- check` (or `-- write` to update the committed
  numbers after an accepted ratio change) is ready to wire into CI as a
  new non-required job; the wiring itself needs a `.github/workflows/`
  push, which needs `GH_ADMIN_TOKEN` (`agents/GOVERNANCE.md`, "Push
  identity"), not available to the agent that measured this. Progress-graph
  rendering, the CI wiring, and the scheduled `corpus-fetch` workflow
  (issue #231) remain.
- Scheduled fuzz smoke run (`research/JOURNAL.md` S2-A53, issues
  #53/#295): `fuzz-check.yml` runs both `fuzz/` targets
  (`decode_arbitrary`, `roundtrip`) weekly, Sunday 06:13 UTC, 30 seconds
  each on Linux x64, failing the job and uploading crashers on a find.
  Completes the scheduled-run scope S2-A25 left open; cross-OS fuzzing,
  OSS-Fuzz, and an allocation-limiter target remain M4 scope.

### Removed

- The interactive `@claude` mention agent (operator directive). Mentions
  no longer trigger anything; open an issue instead, the heartbeat
  triages and answers daily.

### Changed

- The status page ([mothergod.dev/status](https://mothergod.dev/status.html))
  regenerates its data at every site deploy instead of reading a
  hand-committed snapshot that drifted stale within days (ADR-0037):
  `.github/scripts/status-data.py` derives milestones from ROADMAP.md
  checkboxes, the experiment ledger from `research/progress.jsonl`, and
  the new benchmarks table from `bench/baseline.json`, the CI ratio
  gate's own numbers. The unwired `site-status` crate is deleted; its
  planned commit-back wiring was the pattern PR #34 already falsified.
- The agent telemetry feed (`site/agent-metrics.json`, behind
  [mothergod.dev/agents.html](https://mothergod.dev/agents.html)) and the
  model-intel report carry a self-wake audit (issue #144): every admitted
  thread-event BDFL wake in the 7-day window is re-derived from the API,
  independently of the wake predicate that admitted it. Zero is claimed
  only when every wake was verified; fetch failures are counted and named.
  Validated live against the two known pre-#142 incidents, which it flags
  exactly.

- `tests/golden.rs`'s re-encode pin (issue #290's ruling): an encoder-only
  change (a parse/pricing heuristic that picks a different valid token
  sequence, with `decode` byte-for-byte unchanged) no longer needs a
  `FORMAT_VERSION` bump to pass. `FORMAT_VERSION` versions the decode
  contract only. Such a change now moves the current-version fixture pair
  into the new `tests/golden/superseded/` (decode-only forever after,
  proving old frames still decode) and regenerates the pair in
  `tests/golden/`, declared in the PR body with measured justification, the
  `bench/baseline.json` pattern. CLAUDE.md rule 5 and `docs/TESTING.md`
  layer 5 gained one clarifying line each. Unblocks `research/JOURNAL.md`
  S1-P2's `dp_round` wiring (S2-A47, issue #290).

- CI gate (operator directive): the four cargo jobs (fmt, clippy, test,
  doc) skip on pull requests that change no cargo input, decided by one
  `changes` job filtering on tool-input file types rather than tree
  paths; pushes to `main` always run everything.
- Pause detector (ADR-0004, amended): the usage-limit marker list gains
  the "session limit" dialect after run 32588022230 slipped through
  unpaused, and RESUME-AT now honors a UTC reset time advertised in the
  error message, falling back to the blanket +6h/+24h rule when none is
  present.
- Two-realm repository layout (ADR-0010): agent-system files moved to
  `agents/` (governance, operations, personas, sources, identities),
  strictly separated from the classical project tree.
- `codec::MAX_DECODED_LEN`'s doc comment (`research/JOURNAL.md` S2-A27,
  issue #219): states the measured worst-case decode time (~314s at the
  256 MiB ceiling, a steady ~1170 ns/byte, confirmed linear from 1 MiB
  to 256 MiB) instead of a guessed "low single-digit minutes", and
  correctly names the all-literal decode path as the expensive branch
  instead of the "cheapest branch" an earlier version of the comment
  claimed. No behavior change: the ceiling and decode logic are
  unchanged, only the documentation of their already-shipped
  characteristics.

### Fixed

- `docs/benchmarks/baseline.md`/`baseline.svg` regenerated from the current
  `bench/baseline.json`: stale since #301 (S2-A48) landed the
  `BinaryTreeMatchFinder` wiring and moved several baseline cases (e.g.
  `entropy_ladder_h1` 1.298080 -> 1.268960 b/B) without anyone re-running
  `render_baseline_graph`. `bench/baseline.json` is the source of truth;
  these files are a rendering of it and drift silently once out of sync.
- `lz::BinaryTreeMatchFinder::insert_and_find` now evicts a candidate from
  the tree the instant its distance exceeds `WINDOW`, instead of leaving it
  reachable and filtering it only at report time (`research/JOURNAL.md`
  S1-P2, S2-A49). Closes the tree-walk-cost half of the doc comment's
  standing complaint on inputs over `WINDOW` (2^20 bytes); provably no bpb
  change, since in-window candidates are always visited before any
  out-of-window one, so no previously reported match can become
  unreported.
- The allowance-sensing chain went blind on 2026-08-26 when the rate-limit
  event payload moved utilization from flat fields into nested
  `unifiedWindows`: the audit artifact's allowance index emitted empty, the
  #202 guard ledger never received its first write, and the retrospect
  budget footer printed a calm "nothing to project" instead of an alarm.
  `agent-audit`'s extract step and `retrospect` now read both shapes
  (nested preferred when present), and a payload with events but no
  readable utilization is reported as a probable shape change, naming the
  whole blind chain, so the next silent format drift is loud.
- Site prose used the em dash the house voice bans (issue #298):
  `site/index.html` and `site/status.html` rewritten with comma, colon, or
  semicolon in place of every prose em dash. Page `<title>` separators, the
  logo `alt` text, and the `.principles` CSS list marker are typography, not
  prose, and keep theirs per the issue's ruling.
- `compress()` hung on long runs of a single repeated byte (issue #179,
  found while landing `Method::Lz`, S2-A17): a 200,000-byte input took
  over 60 seconds and had to be killed. `lz::parse_optimal`'s
  rep-candidate match-length scan re-walked the whole run at every
  position, with no carry-reuse equivalent to the existing hash-chain
  search's carry. Fixed with a per-distance carry
  (`research/JOURNAL.md` S2-A18); the same 200,000-byte input now
  compresses in under a second, verified against the public API
  directly and pinned by a new wall-clock-bounded regression test.
- `literal::Literal`'s exponentiated-gradient mixing-weight update
  (`research/JOURNAL.md` S2-D3, resolved by ADR-0024) called
  `f64::exp()` on both the encode and decode path; replaced with a
  crate-local `exp` built from IEEE-754 basic operations only (range
  reduction plus a polynomial, `2^k` by exact repeated doubling), so
  encoder and decoder compute a bit-identical mixing weight on every
  platform. Verified against a kept `f64::exp` reference: bit-identical
  encoded output on a 25,524-byte named corpus (well within the 1%
  budget ADR-0024 sets). Unblocks M1's remaining `Method`-wiring slice
  (issue #161).
- The reviewer agent approved PRs but sometimes skipped merging them
  (issue #21), leaving the operator to merge by hand (PRs #15, #19). Root
  cause: the PASS procedure told the reviewer to run
  `gh pr checks <n> --watch --fail-fast` before merging, but that check
  list includes the reviewer's own still-running job, which cannot
  complete while being watched from inside itself; confirmed against the
  repo's ruleset, which requires only `test`/`doc`/`clippy`/`fmt`, not
  `review`, so watching the reviewer's own check was never necessary.
  The reviewer now merges unconditionally as its last action
  (`gh pr merge <n> --squash --auto`, falling back to a plain squash
  merge) instead of watching first.

- The reviewer wrongly labeled PR #22 `blocked-on-human`, believing an
  unsigned branch commit would fail the `required_signatures` ruleset on
  `main` (issue #24). It does not: squash merge creates a new commit
  server-side, signed by GitHub, independent of the source branch's own
  signature status. The fact now lives in `agents/GOVERNANCE.md`
  ("Merging"), which every reviewing agent reads, and the reviewer's merge
  step spells out the specific belief to reject. Follow-up, found while
  landing this fix on PR #25: `gh pr merge` runs its own client-side
  mergeable-state check and can refuse a squash the REST API accepts
  immediately; the reviewer's merge step now falls back to `gh api -X PUT
  .../pulls/<n>/merge` when `gh pr merge` refuses citing branch policy.

- The usage-limit pause detector false-positived on the system's own
  documentation (issue #11): it grepped the whole session transcript, and
  this repo's prompts and docs legitimately contain phrases like "usage
  limit" and "weekly limit" because they describe the pause machinery
  itself. A max-turns failure of the BDFL's first run was thereby
  misclassified as a weekly usage limit, pausing all agents for 24 hours.
  The detector now inspects only structured error result objects, so a
  successful run can never pause the system and only a genuine limit error
  triggers. Turn and time budgets raised generously across all agents
  (BDFL 120→500 turns, heartbeat 130→400, researcher 150→500, reviewer
  100→300, interactive 60→200) per the operator's directive: tight limits
  poison good runs, smart limits need experience first.
- `agent-review` refused to run on any PR authored by our own `claude[bot]`
  identity (BDFL or heartbeat PRs) — `claude-code-action`'s default
  bot-actor guard blocked it before it read a single file. Would have
  silently broken review→automerge for every agent-authored PR the moment
  heartbeat opened one. Scoped `allowed_bots: "claude"` to fix; fork PRs
  stay excluded so no external bot gains anything.
- `agent-review` also could not merge what it just verified when the PR
  author is `claude[bot]`: GitHub refuses self-approval at the platform
  level (`gh pr review --approve` → "Can not approve your own pull
  request"), independent of branch protection. Since this repo's ruleset
  requires zero approving reviews to merge, an approval was never actually
  load-bearing — the reviewer's prompt now posts its verification as a
  plain comment instead when self-approval fails, then proceeds to label
  and merge as normal.
- The `main` branch ruleset carried four rules (`copilot_code_review`,
  `code_coverage`, `code_quality`, `code_scanning`) for tools this repo
  never runs, plus `require_extra_approval_for_unattributed_changes`,
  which the reviewer can never satisfy for its own agent-authored PRs
  (GitHub refuses self-approval). Both fail silently as
  `mergeStateStatus: BLOCKED` with no indication which rule is at fault.
  Every merge to `main` since the repo's creation had therefore fallen
  back to the operator merging by hand via admin bypass — the autonomous
  reviewer/heartbeat merge pipeline (ADR-0003) had never actually run
  end to end. Removed the unsatisfiable rules and disabled the
  extra-approval flag; verified by merging PR #2 (a routine dependabot
  bump) with a plain agent token — first fully autonomous merge to
  `main`.
- `README.md` and `site/index.html`'s status text still said the container
  format's only method was `Stored`, though `Method::Lz` (optimal-parse LZ
  over the context-mixing range coder) has been wired since ADR-0026/0028
  (`FORMAT_VERSION` 2). Corrected both to name `Lz` and note that no
  Silesia/Canterbury benchmark-suite number exists yet for this Rust build
  — that is `ROADMAP.md` milestone M2, not yet landed; see
  `research/JOURNAL.md` S2-A17 for a dev-time spot-check, not the
  aggregate claim the scorecard wants.

### Added

- Project website at [mothergod.dev](https://mothergod.dev): a minimal,
  honest landing page (`site/`) stating pre-alpha status, linking the
  roadmap, research journal, and governance docs, deployed to Cloudflare
  Pages by `deploy-site.yml` on every push touching `site/`.
- Project mark (`assets/logo.svg`, halo variant) in README and rustdoc;
  brand sheet at `assets/mark.html` (anatomy, palette, scale test, source).
- Telegram status bot integration: automatic pause alerts from every agent
  workflow, dire escalations and weekly digest from the BDFL, and an
  operator inbox read at each BDFL wake-up; chat id self-bootstraps on the
  operator's first message to the bot.

- Crate skeleton with v0 framed container format (magic, version, method
  byte); `Stored` method only.
- Quality-gate CI (fmt, clippy, tests, docs).
- Agent-run development system: daily maintainer heartbeat, adversarial PR
  reviewer with autonomous merge, weekly researcher, interactive `@claude`,
  usage-limit pause mechanism.
- Governance, contributing, security, roadmap, and research-journal
  documentation; journal seeded with the founding session's findings.
- Founding-session artifacts archived verbatim in
  `research/imports/session-1/` (codec import-verified lossless), and a
  weekly BDFL driver agent that directs the project and evolves the
  non-code processes without ceremony (ADR-0005).
- BDFL core mandate and success scorecard codified in ROADMAP.md (mission:
  trustworthy, honest, wanted; metrics: RATIO/TRUST/SPEED/USERS/SIMPLICITY,
  FLOW/HEALTH/HONESTY) and wired into the weekly digest; BDFL steers all
  non-code OSS aspects (docs, blog, launches — external posting
  operator-gated). BDFL cadence raised to every three hours with run-economy
  rules, and an explicit bias to solve problems by improving the agent
  system itself (ADR-0007).
- Single-language policy (ADR-0006): Rust only — the founding Python
  harness was verified, then moved to git history; its proxy-speed
  experimentation is to be recovered via an ideal-cost accounting mode in
  the Rust models. Corpus sourcing plan and test-suite strategy codified
  (`research/corpus/POLICY.md`, `docs/TESTING.md`).
