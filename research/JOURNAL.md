# Research journal

Falsification record and institutional memory. Append-only in spirit: never
delete an entry; a revived idea gets a NEW entry referencing the old one.
Audience: agents. Terse. Mechanisms over scores.

Format per entry: `id | verdict | claim | mechanism/evidence | conditions`.
Verdicts: LAW (holds until falsified), ACCEPTED, REJECTED, LEAD (untested),
DEBT (known gap with named fix).

Entries S1-* were established in the founding session (2026-08-19, Python
prototype through Rust codec v0.6, ~41 loop iterations on Silesia/Canterbury +
custom corpus). The original `research_state.json` / `progress.jsonl` with full
numbers are pending import (ROADMAP M1); scores below are from that session's
record.

## Laws

- S1-L1 | LAW | No free lunch from re-labeling: every code that shrinks some
  inputs expands others (pigeonhole/Kraft). Consequence: stored-block floor
  (`min(compressed, raw + flag)`) is mandatory armor, caps worst case ~8.001
  bits/byte. | Counting argument + measured (random data 9.74 b/B without it).
- S1-L2 | LAW | Compression ratio is a property of the model↔data match, never
  of the algorithm alone. Every benchmark claim must name its corpus. |
  Same scheme scored 9.74 vs 1.25 b/B on different data.
- S1-L3 | LAW | H₀ (byte histogram entropy) is not compressibility; it is
  compressibility under a memoryless model. The markov-H8/2 trap (uniform
  histogram, conditional entropy 2.0) separates context modelers from
  histogram coders: ours 2.66, zstd -19 left ~2 b/B on the table. Keep this
  dataset in the corpus forever.
- S1-L4 | LAW | Richer models need more data to pay for themselves
  (bias/variance). Rich literal contexts were rejected 5× at 10 KB slices and
  accepted at real file sizes — the law had a scale term. Record slice sizes
  with every experiment.
- S1-L5 | LAW | Fitness functions are attack surfaces (Goodhart). The
  invertibility guard once killed a "sort all bytes" filter that would have
  topped the benchmark. Guards are independent of the proposer; the proposer
  never grades itself.
- S1-L6 | LAW | Word models and LZ are substitutes, not complements: matches
  consume lexical repetition before literals see it. Mixer weighted a PAQ-style
  word model to zero inside the LZ hybrid. Don't re-add without removing LZ
  from the path.
- S1-L7 | LAW | Corpus composition crowns the winner. Anti-overfit machinery:
  rotating train slices, sealed validation set (different seed AND datasets),
  adversarial corpus additions scored by REGRET vs a reference compressor
  (pure noise has zero regret → auto-rejected). Validation curve stayed
  monotone over ~10h of experiments under these guards.

## Accepted (architecture as of Rust v0.6)

- S1-A1 | ACCEPTED | Pipeline: filter bank → LZ → adaptive entropy coder.
  Filters tried per input, kept only if they win (PNG-style trial selection —
  histogram and order-1 proxies for filter selection were both falsified;
  trial everything, trust nothing).
- S1-A2 | ACCEPTED | Filter bank: delta stride-k (k=1,2,4; auto-stride via
  empirical record-length detection — kennedy.xls 0.34 vs xz 0.64), transpose
  (x-ray −0.47), BCJ x86, base64-unwrap (single biggest drop of its session),
  reverse (right-anchored structure is real).
- S1-A3 | ACCEPTED | LZ: optimal-parse two-pass priced DP, 3-slot repeat-offset
  cache carried IN the DP state (post-pass rep bolting was measurably worse),
  bucket-boundary length candidates, long-match carry, rep-aware price
  iteration, lazy fallback for fast path. 1 MB window.
- S1-A4 | ACCEPTED | Entropy stage: context-mixing binary-ish AC. Six experts
  (nibble-context, order-0, hashed order-2, alignment, two-rate fast/slow
  counters), gradient-derived mixing weights (Mahoney 2005 — replaced ad-hoc
  EG mixer, −0.190 train), context-sensitive MIX weight selection (ZPAQ),
  split literal/length models (it30: length codes polluting literal models was
  a silent tax on EVERY dataset — the most valuable find was a dumb bug).
- S1-A5 | ACCEPTED | Integer-only probability path (DOD arena, fused
  blend+quantize loop). Retired the cross-platform f64 determinism hazard;
  also 1.5–4× speed. Autovectorizes.
- S1-A6 | ACCEPTED | Block-parallel encode/decode (2 MB blocks) costs ~2.3%
  ratio (model-reset tax) — same trade zstd -T makes. Correct, unmeasured
  scaling (1-core container).

## Rejected (do not re-run without changed conditions)

- S1-R1 | REJECTED | Delta filter on text: numeric differences of letters are
  MORE scattered than the letters. Filters must match the structure kind.
- S1-R2 | REJECTED | Parity context for audio: the d2 filter already
  de-interleaves; phase information was spent. Filters and contexts can be
  substitutes too.
- S1-R3 | REJECTED | Offset-by-length conditioning: flat, no effect.
- S1-R4 | REJECTED ×4, then ACCEPTED | Rich literal contexts: died by iid-tax,
  data starvation, global-mixing inadequacy, and count-backoff dilution — all
  at 10 KB slices; accepted at real file sizes (see S1-L4). The near-miss
  diagnosis named the right fix: PPM-style escape (back off only for symbols
  unseen in context), still a LEAD below.
- S1-R5 | REJECTED | Count-backoff toward order-0 for literals: damages exactly
  the contexts that predict best (val objectors: json, log).

## Standing leads (ordered; heartbeat/researcher pick from the top)

- S1-D1 | RESOLVED 2026-08-20 | Founding artifacts fully imported to
  `research/imports/session-1/` (codec, complete harness, state, progress
  log). Codec import-verified lossless; harness verified end-to-end — it
  reproduces the it31 champion's sealed-validation scores exactly (VAL
  20.697) and is resumable at it31. Sole residue: it32–it41 artifacts
  postdate the archive (transcript-only). Porting the codec into `src/` is
  ROADMAP M1.
- S1-D2 | DEBT | Benchmark harness in-repo: Silesia + Canterbury fetch,
  entropy-ladder + markov-trap generators, sealed validation split, regret
  scoring, baseline gate in CI, progress graphs from progress.jsonl.
- S2-A1 | ACCEPTED | First slice of S1-D2: the two mandatory corpus
  generators (POLICY.md) ported to Rust as a new `bench/` workspace crate —
  `entropy_ladder` (iid bytes at a chosen order-0 entropy) and
  `markov_h8_2_trap` (uniform histogram, low conditional entropy). Behavior
  ported from the founding session's `corpus.py` (`_skewed_weights` bisection
  + additive random walk), not the code (ADR-0006). | Measured on 200,000-byte
  samples, seed 0xC0FFEE123456789A: entropy ladder targets {1,2,4,6,8} bits
  landed at {0.998, 1.998, 3.997, 5.996, 7.999} (all within 0.004 bits);
  markov-H8/2 trap landed at h0=7.998, h1=1.987 (targets 8.0/2.0). | No
  round-trip, ratio, or sealed-validation measurement in this change — there
  is no real codec yet (M1) and no champion to diff against, so this is
  infra, not an experiment; `progress.jsonl` records it as `kind: "patch"`
  with null bpb deltas per the prerequisite-check rule. Root `Cargo.toml`
  gained `[workspace]`; core crate (`mothergod`) still zero-deps (ADR-0002),
  `bench/` depends on it by path. Remaining S1-D2 scope untouched: see S2-D1.
- S2-A2 | ACCEPTED | First slice of M1: the fixed-stride delta filter
  (JOURNAL S1-A2) ported to `src/filters.rs` as a standalone reversible
  transform (`encode`/`decode`, wrapping arithmetic, `stride: NonZeroUsize`
  so a zero stride — which would destroy data instead of transforming it —
  is unrepresentable rather than runtime-checked). Behavior ported from the
  archive's `sdelta`/`usdelta` (`research/imports/session-1/mothergod.rs`),
  not the code (ADR-0006): forward accumulation reads the mutable output on
  decode, the immutable input on encode, so short-data and zero-length
  inputs are a no-op in both directions with no bounds panic. | 6 unit
  tests (round-trip across 10 strides between 1 and 1001 on 1000-byte
  cyclic data, empty input, single byte, stride longer than data, u8-wrap
  construction); `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test --all-targets`/`test
  --doc`/`doc --no-deps` all clean. | No bpb measurement: this filter is not
  yet wired to a `Method` variant (needs parse+models+coder to be worth
  measuring, and a `FORMAT_VERSION` bump per CLAUDE.md hard rule 5 once it
  is), so there is still no champion to diff against — `progress.jsonl`
  records this as `kind: "patch"` with null bpb deltas, same as S2-A1.
  Exposed as `pub mod filters` (not `pub(crate)`): keeps the module reachable
  without tripping `dead_code` under `--deny warnings` while unwired, and
  filters are a defensible standalone library surface on their own merits.
  Remaining M1 scope: see S2-D2.
- S2-A3 | ACCEPTED | Second slice of M1: the row-major-to-column-major
  transpose filter (`JOURNAL` S1-A2) ported to `src/filters.rs` as a
  standalone reversible transform (`transpose::encode`/`decode`), mirroring
  S2-A2's structure. Behavior ported from the archive's `tpose`/`untpose`
  (`research/imports/session-1/mothergod.rs`), not the code (ADR-0006):
  rewrites `data`, interpreted as rows of `columns` bytes, column by
  column; `columns` is `NonZeroUsize` so a zero column count (no rows to
  transpose) is unrepresentable. `filters.rs` split its flat `encode`/
  `decode` into `delta` and `transpose` submodules so the two filters'
  functions of the same name don't collide — no external code referenced
  the old flat names (grepped clean), so the rename is not a breaking
  change to anything real. | 6 unit tests per filter kind (12 total, up
  from 6): round-trip across 10 column counts between 1 and 1001 on
  1000-byte cyclic data, empty input, single byte, columns wider than the
  data, an explicit grouping check on a short example, single-column
  identity; `cargo fmt`/`clippy --all-targets -- --deny
  warnings`/`test --all-targets`/`test --doc`/`doc --no-deps` all clean. |
  No bpb measurement, same reason as S2-A2: not yet wired to a `Method`
  variant, so there is still no champion to diff against —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas.
  Remaining M1 scope: see S2-D2.
- S2-A4 | ACCEPTED | Third slice of M1: the x86 call/jmp (BCJ) filter
  (`JOURNAL` S1-A2) ported to `src/filters.rs` as a standalone reversible
  transform (`bcj::encode`/`decode`), mirroring S2-A2/S2-A3's structure.
  Behavior ported from the archive's `bcj(d, enc)` (`research/imports/
  session-1/mothergod.rs`), not the code (ADR-0006): the single
  boolean-flag function became two functions, one per direction, matching
  this module's established encode/decode-pair shape. Rewrites the 4-byte
  little-endian operand following every `0xE8`/`0xE9` opcode between a
  position-relative offset and an absolute one; only the opcode byte gates
  which positions are touched, and the scan jumps past the whole
  instruction on a match, so the operand bytes it just wrote are never
  re-examined as a new opcode — decode rediscovers exactly the same
  positions encode found. | 8 unit tests: round-trip on empty input, an
  opcode with too few trailing bytes to hold an operand (identity), an
  explicit E8 and E9 operand rewrite check, identity when no opcode byte
  is present, round-trip over 2000 bytes of cyclic data (which contains
  0xE8/0xE9), 20 adjacent instructions back to back, and an operand large
  enough that the relative-to-absolute add wraps u32; `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test --all-targets`/`test
  --doc` all clean (`doc --no-deps` under `RUSTDOCFLAGS=--deny warnings`
  blocked locally by an unrelated sandbox permission gate on env-prefixed
  commands; plain `cargo doc --no-deps` built with zero warnings and CI's
  `doc` gate re-verifies with the flag before merge). | No bpb measurement,
  same reason as S2-A2/S2-A3: not yet wired to a `Method` variant, so
  there is still no champion to diff against — `progress.jsonl` records
  this as `kind: "patch"` with null bpb deltas. Remaining M1 scope: see
  S2-D2 (now base64-unwrap, reverse, `pick_filters`, LZ, models, coder,
  and `Method` wiring only).
- S2-A5 | ACCEPTED | Fourth slice of M1: the base64-unwrap filter
  (`JOURNAL` S1-A2, "single biggest drop of its session") ported to
  `src/filters.rs` as a standalone reversible transform
  (`base64_unwrap::encode`/`decode`). Behavior ported from the archive's
  `filt`/`unfilt` pair (`research/imports/session-1/research_state.json`
  `.filters.b64`; absent from `mothergod.rs`, confirmed by S2-A4's
  cross-check), not the code (ADR-0006): unlike delta/transpose/bcj, this
  filter's decision (unwrap or not) is data-dependent rather than a
  caller-supplied parameter, so `encode` always prepends a one-byte flag
  (`1` = unwrapped, `0` = passed through) and `decode` reads it back
  instead of taking a filter parameter — the shape a self-describing
  filter needs, distinct from the other three's pure `data -> data`
  pairs. A standard base64 codec (encode/strict decode, zero
  dependencies) was written to support it, since the crate has no base64
  crate to call (ADR-0002); "canonical" is checked by re-encoding the
  decoded bytes and comparing to the input, which catches non-canonical
  padding bits the same way the archive's `b64encode(dec)==d` check did.
  | 10 unit tests: round-trip on empty input and input too short to try
  (`MIN_LEN`), unwrap of valid base64, pass-through of non-base64 data,
  pass-through of invalid padding placement, pass-through of valid-looking
  but non-canonical padding bits (an explicit guard-exercise assertion),
  round-trip across all three padding-length classes, decode of empty
  input, and round-trip of a 300-byte binary payload; `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test --all-targets`/`test
  --doc`/`doc --no-deps` (plain, `RUSTDOCFLAGS=--deny warnings` blocked
  locally by the same sandbox permission gate noted in S2-A4; zero
  warnings either way) all clean. | No bpb measurement, same reason as
  S2-A2 through S2-A4: not yet wired to a `Method` variant, so there is
  still no champion to diff against — `progress.jsonl` records this as
  `kind: "patch"` with null bpb deltas. Remaining M1 scope: see S2-D2
  (now `reverse`, `pick_filters`, LZ, models, coder, and `Method` wiring
  only).
- S2-A6 | ACCEPTED | Fifth and final filter slice of M1: byte-order
  reversal (`JOURNAL` S1-A2, "right-anchored structure is real") ported
  to `src/filters.rs` as a standalone reversible transform
  (`reverse::encode`/`decode`). Behavior ported from the archive's
  `filt`/`unfilt` pair (`research/imports/session-1/research_state.json`
  `.filters.rev`; absent from `mothergod.rs`, same basis as S2-A5's
  `.filters.b64` port), not the code (ADR-0006): `filt`/`unfilt` are both
  `d[::-1]`, so this filter is its own inverse — `decode` is `encode`
  under a different name, kept as two functions to match the other four
  filters' encode/decode-pair shape rather than exposing a single
  `reverse` function callers would have to know is symmetric. | 6 unit
  tests: round-trip on empty input and a single byte, an explicit
  byte-order check, an explicit "encoding twice is the identity" check,
  round-trip across 8 lengths between 0 and 1000 on 1000-byte cyclic
  data, and a palindrome as a fixed point of `encode`; `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test --all-targets`/
  `test --doc`/`doc --no-deps` all clean. | No bpb measurement, same
  reason as S2-A2 through S2-A5: not yet wired to a `Method` variant, so
  there is still no champion to diff against — `progress.jsonl` records
  this as `kind: "patch"` with null bpb deltas. Completes M1's filter-bank
  checklist (all five kinds from S1-A2 now in `src/filters.rs`); remaining
  M1 scope: see S2-D2 (now `pick_filters`, LZ, models, coder, and `Method`
  wiring only).
- S2-A7 | ACCEPTED | Sixth slice of M1: the `pick_filters` trial-selection
  heuristic ported to `src/filters.rs` as a `select` submodule
  (`select::pick`, `select::Candidate`), mirroring the other filter
  submodules' structure. Behavior ported from the archive's
  `pick_filters` (`research/imports/session-1/mothergod.rs`), not the
  code (ADR-0006): only the filters `pick_filters` itself covers —
  delta, BCJ, transpose — are shortlisted; `base64_unwrap` and `reverse`
  are absent from that function in the archive too (same basis as
  S2-A5/S2-A6). One behavior-preserving deviation from the archive's raw
  `u8` candidate ids (0=identity, 1..=96=delta stride, 97=BCJ,
  100+i=transpose): a `Candidate` enum, so a caller can't mix up a delta
  stride with a transpose column count — both are plain integers in the
  archive's id space. `NonZeroUsize` for stride/column count, matching
  `delta`/`transpose`'s existing parameter types. | 6 unit tests:
  identity always present (including on empty input), a synthetic
  small-random-walk fixture where the winning candidate is the delta
  stride matching the walk's column count, an opcode-dense fixture that
  shortlists BCJ and a sparse one that doesn't, a transpose-structured
  fixture that shortlists a transpose candidate, and a length check that
  transpose is never shortlisted below `MIN_TRANSPOSE_LEN`; `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test --all-targets`/
  `test --doc`/`doc --no-deps` all clean. | No bpb measurement, same
  reason as S2-A2 through S2-A6: `pick` is not yet called by anything —
  wiring it into an actual trial-encode loop needs the LZ/model/coder
  stages this shortlist is meant to gate, still to come. `progress.jsonl`
  records this as `kind: "patch"` with null bpb deltas. Remaining M1
  scope: see S2-D2 (now LZ, models, coder, and `Method` wiring only).
- S2-A8 | ACCEPTED | Seventh slice of M1, and the first LZ slice: the
  greedy/lazy parser ported to a new `src/lz.rs` module (`Token`,
  `parse_greedy`), plus `replay`, its inverse. Behavior ported from the
  archive's `lz` (`research/imports/session-1/mothergod.rs`), not
  `lz_opt`, not the code (ADR-0006): `lz_opt`'s DP prices candidates
  against the entropy models' own frequency tables, which don't exist
  in this crate yet, and `lz_opt` runs `lz` internally as its
  price-seeding first pass — this parser is a real prerequisite, not a
  detour. One behavior-preserving deviation: the archive's single
  `find` closure (shared by the rep-cache scan and the hash-chain
  search) becomes two named functions (`match_len`,
  `MatchFinder::find_best`), and its raw `(usize, usize)` `(0, 0)`
  "no match" sentinel pair becomes `Option<(usize, Distance)>` —
  `Distance` a `NonZeroU32` newtype, so a match can no longer be
  represented with a zero distance, closing the exact confusion class
  the session-1 port bug came from (`rust-craft` skill,
  type-precision: a rep-symbol/offset-bucket collision that existed
  because an invariant lived only in one implementation's window
  size). `RepSlot` is an enum (`First`/`Second`/`Third`), not a raw
  index, for the same reason. | 11 unit tests: empty input, a single
  byte, an all-literals fixture with no repeats, a simple 5x repeat
  (exercises `Token::Match`), a 1000-byte run-length fixture (distance
  1, shorter than the eventual match length — proves `copy_match`
  handles overlapping source and destination), a 200,000-byte run
  (spans multiple tokens past `MAX_MATCH_LEN` = 65535), an alternating
  two-pattern fixture (exercises `Token::Rep` and cache reuse), 5000
  bytes of cyclic 0..=255 data, a structured fixture with near-duplicate
  26-byte blocks at non-initial distances (exercises the one-step
  lazy-matching check), and a 1000-byte fixture with zero bytes present;
  every test asserts `replay(parse_greedy(data)) == data` plus (where
  matches are expected) that at least one `Match`/`Rep` token was
  emitted, and that no token's length exceeds `MAX_MATCH_LEN`. `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test
  --all-targets`/`test --doc`/`doc --no-deps` all clean (two
  intra-doc-link warnings against private items, `match_len`,
  `MatchFinder::find_best`, `REP_SLOTS`, fixed by dropping the doc
  links, not by suppressing the lint). | No bpb measurement, same
  reason as S2-A2 through S2-A7: no `Method` variant to wire this
  behind yet, and no entropy coder to measure a real bitstream through
  — `progress.jsonl` records this as `kind: "patch"` with null bpb
  deltas. `WINDOW` (1 MiB, `JOURNAL` S1-A3) is exposed as `pub`; a
  future streaming/block API and the entropy models' offset-bucket
  encoding will both need to agree on it. Remaining M1 scope: see
  S2-D2 (now `lz_opt`'s DP price tables, the context-mixing entropy
  models, the range coder, and `Method` wiring).
- S2-A9 | ACCEPTED | Eighth slice of M1, and the second LZ slice: the
  DP-priced optimal parse ported to `src/lz.rs` as `parse_optimal`,
  backed by a new `dp_round` (one DP pass) and `PriceCounts`/`PriceTable`
  (the archive's frequency-table price model). Behavior ported from the
  archive's `lz_opt` (`research/imports/session-1/mothergod.rs`), not the
  code (ADR-0006): a first pass with `parse_greedy` seeds a price table
  (16-context nibble literal histogram, length/offset bucket histograms,
  a scalar rep price), two DP rounds each find the min-price path under
  the current table, and round 0's resulting tokens reseed a sharper
  table for round 1 — the archive's own 2-round structure, not iterated
  to convergence. Below `OPTIMAL_MIN_LEN` (64 bytes) falls back straight
  to `parse_greedy`, matching the archive's `n<64` short-circuit; the
  archive's `carry` reuse (a long match found at one position is one byte
  shorter at the next, same distance, so a match ≥64 bytes doesn't repeat
  a fresh 640-try hash-chain search at every position it spans) is
  ported unchanged. | One deliberate correctness fix over the archive's
  own DP, not a port of its behavior: on a fresh (non-repeat) match, the
  archive's `lz_opt` updates its internal price-simulation rep cache by
  deduplicating the new distance against the existing three slots
  (dropping whichever slot already held it), but the archive's actual
  decoder (`decode`, not `lz_opt`) always shifts blindly — the same rule
  this crate's `replay` already implements via `RepCache::push_front`.
  Porting the dedup rule would let the DP choose a later `Token::Rep`
  slot based on a cache state `replay` never reaches, corrupting
  round-trip exactly when a fresh match's distance happens to coincide
  with an already-cached one (`rust-craft` skill, invariant-mismatch:
  the DP's internal bookkeeping and the actual replay/decode bookkeeping
  must be the same function, or a later token silently references the
  wrong state). Hard rule 1 makes that not a judgment call: `dp_round`'s
  cache updates always match `replay`'s (`RepCache::push_front` on
  `Token::Match`, `RepCache::promote` on `Token::Rep`), so this class of
  bug cannot occur here regardless of input. | 11 new unit tests mirroring
  `parse_greedy`'s suite (empty, single byte, below-`OPTIMAL_MIN_LEN`
  falls back to `parse_greedy` exactly, all-literals, a simple repeat, an
  overlapping run-length fixture, a 200,000-byte run exercising the carry
  path, an alternating rep-cache fixture, cyclic data, a structured
  near-duplicate fixture, zero-byte binary data, dense 3-byte-distance
  repeats exercising the length-3 short-match candidate, and a
  xorshift-based pseudo-random fixture), every one asserting
  `replay(parse_optimal(data)) == data` plus the `MAX_MATCH_LEN` bound;
  `cargo fmt`/`clippy --all-targets -- --deny warnings`/`test
  --all-targets`/`test --doc`/`doc --no-deps` all clean. | No bpb
  measurement, same reason as S2-A2 through S2-A8: not yet wired to a
  `Method` variant, no entropy coder to measure a real bitstream through
  — `progress.jsonl` records this as `kind: "patch"` with null bpb
  deltas. Remaining M1 scope: see S2-D2 (now the context-mixing entropy
  models, the range coder, and `Method` wiring only).
- S2-D2 | DEBT | Remainder of M1 after the S2-A2 through S2-A9 filter,
  trial-selection, and LZ slices: the context-mixing entropy models
  (`Lit` six-expert arena, flag/length/offset models); the range coder
  (`Enc`/`Dec`); and wiring all of it (including `select::pick` and
  `lz::parse_optimal`) behind a new `Method` variant with a
  `FORMAT_VERSION` bump + ADR (CLAUDE.md hard rule 5). Source:
  `research/imports/session-1/mothergod.rs` (526 lines, golfed — port
  behavior, not code, per ADR-0006). One PR per module per the M1
  checklist.
- S2-D1 | DEBT | Remainder of S1-D2 after the S2-A1 generators slice:
  Silesia + Canterbury fetch-and-cache (`bench/corpus.toml`, pinned
  URL+SHA-256), the structured generator classes (jsonl/log, json,
  base64-wrapped, audio, image, sqlite-like, x86 binary — specs in
  `corpus.py`), the three-tier train/sealed/finals split plumbing, regret
  scoring, the CI baseline gate, and progress-graph rendering. Ideal-cost
  accounting mode (M2) additionally needs real model code (M1) to hang off
  of.
- S1-P1 | LEAD | SSE (secondary symbol estimation) — oldest unmerged
  literature lead; targets the five zstd text holdouts (combined deficit
  0.11 b/B: alice .019, lcet .044, dickens .054, plrabn .086, sao .109).
- S1-P2 | LEAD | btultra2-class parse: binary-tree match finder with exact
  price feedback + per-position adaptive prices (ours were frozen per round).
  Targets sqlite/json/jsonl residue.
- S1-P3 | LEAD | PPM-style escape for literal contexts (see S1-R4).
- S1-P4 | LEAD | LZMA-class windows for large files (xz's remaining edge).
- S1-P5 | LEAD | Per-column modeling after transpose (filter-aware coder,
  OpenZL direction). Target: sao.
- S1-P6 | LEAD | Speed tier: bit-decomposed coding (LPAQ-style, ~10×), tANS
  fast path (~100×, zstd-class -1 mode), explicit AVX2 blend (~1.5×).
- S1-P7 | LEAD | Production hardening: fuzzing (decoder-never-panics),
  streaming mode, frozen format spec v1.
- S1-P8 | LEAD | GLN-style predictors / more experts (2026 AIT Challenge
  entries) — only after SSE.
