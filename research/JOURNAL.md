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
- S2-A10 | ACCEPTED | Ninth slice of M1, and the first coder slice: the
  adaptive range coder ported to a new `src/coder.rs` module
  (`Encoder`/`Decoder`). Behavior ported from the archive's `Enc`/`Dec`
  (`research/imports/session-1/mothergod.rs`), not the code (ADR-0006):
  same 32-bit `[low, high]` interval in `u64` arithmetic, the same
  three renormalization cases (top-half fixed, bottom-half fixed,
  straddling the middle with a carry deferred via a pending-bit
  counter), the same byte-oriented bit packer. Holds no model of its
  own: `Encoder::encode`/`Decoder::decode` take a caller-supplied
  `[cum_low, cum_high)` out of `total` on every call, so the coder is
  usable by any frequency table, adaptive or fixed, once one exists.
  One behavior-preserving deviation: the archive's decoder narrowing
  loop has an empty first branch (`if hi<HALF{}`, meaning "no value
  adjustment needed, just renormalize") — restructured here as a
  `shift: bool` computed once per iteration instead of an
  if/else-if/if/else chain with an empty arm, avoiding the empty-block
  shape while keeping the exact same three cases and the exact same
  bit-for-bit renormalization. | 9 unit tests: empty and single-symbol
  streams, a skewed-frequency fixture that exercises the near-degenerate
  intervals hardest, a full 256-symbol alphabet cycled 2000 times, a
  5000-symbol xorshift32 pseudo-random fixture, raw-bit round-trip
  across six widths from 0 to 32, an interleaved
  adaptively-coded-symbol/fixed-probability-bits fixture (the exact
  shape a length/offset model will need), and a truncated-stream decode
  that asserts no panic rather than a specific output. Every symbol test
  drives the coder through a small order-0 `FreqTable` test fixture (not
  exported; the real adaptive model is S2-D2's remaining scope) so the
  round-trip exercises actual cumulative-frequency updates, not just
  fixed ranges. `cargo fmt`/`clippy --all-targets -- --deny warnings`/
  `test --all-targets`/`test --doc`/`doc --no-deps` all clean (one
  intra-doc-link warning against a private method, `Decoder::next_bit`,
  fixed by dropping the doc link, not by suppressing the lint — same
  class as S2-A8). | No bpb measurement, same reason as S2-A2 through
  S2-A9: nothing yet drives this coder with real cumulative frequencies
  from actual data, so there is still no champion to diff against —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas.
  Remaining M1 scope: see S2-D2 (now the context-mixing entropy models
  and `Method` wiring only).
- S2-A11 | ACCEPTED | Tenth slice of M1, and the first entropy-model
  slice: the order-0 adaptive frequency table ported to a new
  `src/model.rs` (`Model`), the type the flag/length/offset stages of
  S2-D2 will each instantiate directly. Behavior ported from the
  archive's `Model` (`research/imports/session-1/mothergod.rs`), not the
  code (ADR-0006): same increment-then-halve update rule (`INC` = 12,
  `LIM` = 65536), same linear cumulative-frequency scan, now driving
  `coder::Encoder`/`Decoder` (S2-A10) with real data-derived ranges
  instead of that module's own test-only `FreqTable` stand-in. `decode`
  never panics on adversarial `Decoder` state: `Decoder::target` is
  mathematically bounded to `[0, total)`, and `total` always equals the
  sum of `freq` by construction, so the cumulative scan always finds a
  symbol before running past the table regardless of what bytes
  produced the decoder's value (`rust-craft` skill,
  panic-discipline). | 8 unit tests: empty and single-symbol streams, a
  skewed-frequency fixture, a full 256-symbol alphabet, a 5000-symbol
  pseudo-random fixture, an explicit rescale-triggering fixture (10,000
  codes over a 2-symbol alphabet crosses `LIM` several times), two
  independent `Model` instances interleaving on one coder stream (the
  flag+length shape S2-D2 still needs), and a truncated-stream decode
  asserting no panic; `cargo fmt`/`clippy --all-targets -- --deny
  warnings`/`test --all-targets`/`test --doc`/`doc --no-deps` all clean
  (one private-intra-doc-link warning against the module's own private
  constants, fixed by dropping the doc links, not by suppressing the
  lint — same class as S2-A8/S2-A10). | No bpb measurement, same reason
  as S2-A2 through S2-A10: not yet wired to a `Method` variant, no
  champion to diff against — `progress.jsonl` records this as `kind:
  "patch"` with null bpb deltas. Remaining M1 scope: see S2-D2 (now the
  six-expert `Lit` literal mixer, wiring the flag/length/offset models
  as `Model` instances, and `Method` wiring only).
- S2-A12 | ACCEPTED | Eleventh slice of M1, and the second entropy-model
  slice: the six-expert context-mixing literal model ported to a new
  `src/literal.rs` (`Literal`, `Context`), the entropy stage for every
  byte an LZ parse (`lz`) leaves as a literal. Behavior ported from the
  archive's `Lit` (`research/imports/session-1/mothergod.rs`), not the
  code (ADR-0006): the same six context banks (two-rate fast/slow order-1
  keyed on the previous byte plus an after-copy bit, order-0, a 12-bit
  order-2 hash, a position/nibble "alignment" hash, and a 12-bit
  alnum-only rolling word hash), the same `(prev-byte nibble, after-copy)`
  key selecting one of 32 mixing-weight vectors, the same fixed-point
  blend (`>>16` after a `u64` per-expert scale factor) feeding the coder,
  and the same exponentiated-gradient weight update (Mahoney 2005) with
  the archive's exact learning rate and clamp. One behavior-preserving
  deviation: the archive recomputes `(b1, b2)` on every token by indexing
  `fd[pos-1]`/`fd[pos-2]` into the shared output buffer; this port
  instead carries a `Context` value forward explicitly
  (`Context::after_literal`/`after_copy`), so an encode pass and a decode
  pass reuse one update rule instead of two independent re-derivations
  that could drift apart (`rust-craft` skill, single-source-of-truth for
  state transitions the two coding directions must agree on bit-for-bit).
  | 13 unit tests: empty input, a single byte, a skewed repeat, the full
  256-value alphabet cycled, ASCII text, a 5000-byte xorshift32
  pseudo-random fixture (crosses every bank's rescale threshold,
  including the fast expert's 6144 ceiling, repeatedly), a fixture
  interleaving literal runs with simulated copy tokens (the shape
  Method-wiring will actually drive this with), four `Context`
  transition unit tests (`after_literal`, `after_copy` at 0/1/2+ bytes),
  a word-hash extend/reset check, and a truncated-stream decode asserting
  no panic; `cargo fmt`/`clippy --all-targets -- --deny warnings`/`test
  --all-targets`/`test --doc`/`doc --no-deps` all clean (one
  private-intra-doc-link warning against `Self::mix`, fixed by dropping
  the doc link, not by suppressing the lint, same class as
  S2-A8/S2-A10/S2-A11). | No bpb measurement, same reason as S2-A2
  through S2-A11: not yet wired to a `Method` variant, no champion to
  diff against; `progress.jsonl` records this as `kind: "patch"` with
  null bpb deltas. **Open question, not resolved here, see S2-D3**: this
  port keeps the archive's `f64` weight-update arithmetic verbatim, which
  `JOURNAL` S1-A5 records as superseded by an integer-only path for
  cross-platform determinism; that refactor postdates the archive (no
  artifact to port from). Carries no live risk yet: nothing in `src/`
  calls this module. Remaining M1 scope: see S2-D2 (now wiring the
  flag/length/offset stages and `Literal` against real LZ tokens, plus
  `Method` wiring). S2-D3 was resolved on 2026-08-23 by ADR-0024; read
  it there, not here.
- S2-D2 | RESOLVED by S2-A17/ADR-0026 and S2-A19/ADR-0028 | Remainder of
  M1 after the S2-A2 through S2-A12 filter, trial-selection, LZ, coder,
  order-0 model, and literal-mixer slices. S2-A17 wired the flag/length/
  offset/rep-slot `model::Model` instances, `literal::Literal`, and
  `coder` against real `lz::parse_optimal` tokens behind a new
  `Method::Lz` variant, `FORMAT_VERSION` bump, and ADR (CLAUDE.md hard
  rule 5; ADR-0026). S2-A19 closed the last remaining piece: wiring
  `filters::select::pick` and trial-encoding against candidate filters.
  Source: `research/imports/session-1/mothergod.rs` (526 lines, golfed,
  port behavior, not code, per ADR-0006).
- S2-D3 | RESOLVED by ADR-0024 | `literal::Literal` (S2-A12) ports the
  archive's exponentiated-gradient mixing-weight update verbatim,
  including `f64::exp()`. `JOURNAL` S1-A5 records "integer-only
  probability path ... retired the cross-platform f64 determinism
  hazard" as accepted architecture, but that refactor is
  transcript-only (postdates `research/imports/session-1/mothergod.rs`,
  per that directory's README "Provenance" note): no artifact exists to
  port the integer version from. `f64::exp()` is not guaranteed
  bit-identical across libm implementations, so an encoder and decoder
  built with different platforms/toolchains could compute different
  mixing weights at the same step and desync, corrupting output: a
  lossless violation (hard rule 1) if this ever backs a real frame.
  Mechanism recorded here per `research/imports/session-1/README.md`'s
  "where archive and journal disagree, say so" instruction, rather than
  silently picking a side. **Resolution (ADR-0024, 2026-08-23):** the
  hazard is the libm call, not the float type. IEEE-754 `+ - * /` are
  correctly rounded and reproducible; transcendentals are not. So the
  decode path may use basic float operations and may not call a
  transcendental, `exp()` at `literal.rs:293` is the only decode-path
  violation in the crate, and the fix is a vendored `exp` built from
  basic operations, enforced crate-wide by
  `clippy.toml`'s `disallowed-methods`. Encoder-only `log2` at
  `filters.rs:773`/`807` and `lz.rs:520` stays, under `#[allow]` with a
  written reason. The full integer mixer is no longer a prerequisite for
  S2-D2; it is an M5 speed lead, because S1-A5's other claim (1.5-4x,
  autovectorizing) is unmeasured here. Acceptance for the replacement is
  exact round-trip plus bits/byte within 1% of the `f64` mixer on a
  named corpus, not bit-identity with the archive: quantizing the update
  changes predictions by construction. Implementation is issue #161, and
  it carries its own experiment record.
- S2-D1 | DEBT | Remainder of S1-D2 after the S2-A1 generators slice:
  Silesia + Canterbury fetch-and-cache (`bench/corpus.toml`, pinned
  URL+SHA-256), the structured generator classes (jsonl/log, json,
  base64-wrapped, audio, image, sqlite-like, x86 binary — specs in
  `corpus.py`), the three-tier train/sealed/finals split plumbing, regret
  scoring, the CI baseline gate, and progress-graph rendering. Ideal-cost
  accounting mode (M2) additionally needs real model code (M1) to hang off
  of.
- S2-A13 | ACCEPTED | ROADMAP M2's adversarial decode seed corpus + suite
  (`docs/TESTING.md` layer 2), independent of S2-D1's remaining
  fetch/generator scope: a new `tests/adversarial/` directory of 13 tiny
  fixtures built to be invalid by construction (empty input, truncation
  at every header-boundary byte offset from 0 to 5, wrong magic, a
  single-byte-flipped magic, a future `FORMAT_VERSION`, an unknown
  method with and without a trailing payload, and two all-`0x00`/all-
  `0xFF` blocks plus a fixed non-matching-magic blob standing in for
  arbitrary noise) and `tests/adversarial.rs`, which reads every file in
  that directory and asserts `decompress` returns `Err`, never a panic
  (CLAUDE.md hard rule 2). Runs on every PR via `cargo test
  --all-targets`, not scheduled: this is layer 2, distinct from the
  scheduled cargo-fuzz layer 3 (M4, issue #53) it will eventually feed —
  a fuzz-found crasher promotes into this same directory as a regression
  seed once fuzzing exists. | 13/13 fixtures assert `Err` (verified each
  by hand against `decompress`'s header-parsing order: length check,
  then magic, then version, then method); `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test --all-targets`/
  `test --doc`/`doc --no-deps` all clean. | No bpb measurement: this is
  a decoder-safety suite, not a ratio experiment; `progress.jsonl`
  records this as `kind: "patch"` with null bpb deltas. Remaining M2
  scope: S2-D1 (corpus fetch/generators), ideal-cost accounting mode,
  the CI benchmark regression gate, and progress-graph rendering.
- S2-A14 | ACCEPTED | First structured-generator slice of M2's remaining
  benchmark-harness debt (S2-D1): synthetic web-server access log lines
  (`research/corpus/POLICY.md`'s "jsonl/log records" class) ported to
  `bench/src/lib.rs` as `access_log`, mirroring `entropy_ladder`/
  `markov_h8_2_trap`'s structure. Behavior ported from the founding
  session's `corpus.py` (`c['log']`, `git show
  1a3b1c8:research/imports/session-1/corpus.py`), not the code
  (ADR-0006): an 80-address IP pool, a fixed six-path request set, and a
  status code skewed toward 200 (three of five draws) via the same
  `Rng` (`SplitMix64`) already in this module, plus a new `next_index`
  helper for uniform slice indexing. One behavior-preserving deviation:
  the archive emits a fixed 1400-line log then truncates to `N` bytes;
  this port generates lines until `len` bytes are reached then truncates,
  so it produces exactly `len` bytes for any requested length instead of
  only for the one size the archive's fixed line count happened to cover.
  | 6 unit tests: exact-length output across five requested lengths
  (including a length shorter than one line), determinism, seed
  independence, a structural check that most output lines contain `GET`
  and `HTTP/1.1`, a check that a large sample's distinct leading IP
  octets stay within the 80-address pool (unlike iid random data), and
  empty input; wired into the existing frame-format round-trip test.
  `cargo fmt`/`clippy --all-targets -- --deny warnings`/`test
  --all-targets`/`test --doc`/`doc --no-deps` (`RUSTDOCFLAGS=--deny
  warnings`) all clean. | No bpb measurement: this is corpus-generation
  infra, not an experiment against a champion — `progress.jsonl` records
  this as `kind: "patch"` with null bpb deltas, same as S2-A1 through
  S2-A13. Remaining S2-D1 scope: Silesia + Canterbury fetch-and-cache,
  the six remaining structured generator classes (json, base64-wrapped,
  audio, image, sqlite-like, x86 binary), the three-tier train/sealed/
  finals split plumbing, regret scoring, the CI baseline gate, and
  progress-graph rendering.
- S2-A15 | ACCEPTED | Second structured-generator slice of M2's remaining
  benchmark-harness debt (S2-D1): a synthetic JSON API response
  (`research/corpus/POLICY.md`'s "json" class) ported to `bench/src/lib.rs`
  as `json_records`, mirroring `access_log`'s structure. Behavior ported
  from the founding session's `corpus.py` (`c['json']`, `git show
  1a3b1c8:research/imports/session-1/corpus.py`), not the code
  (ADR-0006): a `{"status": "ok", "results": [...]}` envelope around
  `user_id`/`name`/`email`/`active`/`score` records, `active` true 80% of
  the time, `score` gaussian (mean 50, stddev 15). The archive draws its
  gaussian from Python's `random.gauss`; this port adds a `standard_normal`
  helper (Box-Muller transform over the existing `Rng`'s `next_unit`) since
  the crate has no gaussian sampler yet and takes no new dependency
  (ADR-0002). One behavior-preserving deviation, the same shape as
  S2-A14's: the archive fixes the response at 500 records then truncates
  to `N` bytes; this port generates records until `len` bytes are reached
  then truncates, so it produces exactly `len` bytes for any requested
  length instead of only for the one size the archive's fixed record count
  happened to cover — truncation is not repaired to valid JSON, matching
  the archive's own raw-truncation behavior. | 5 unit tests: exact-length
  output across five requested lengths, determinism, seed independence, a
  structural check that the output starts with the response envelope and
  contains many `"user_id"` records, and an `active`-field true-fraction
  check (70-90% band, integer arithmetic to avoid a precision-loss cast)
  confirming the 80% skew; wired into the existing frame-format round-trip
  test. `cargo fmt`/`clippy --all-targets -- --deny warnings`/`test
  --all-targets`/`test --doc`/`doc --no-deps` (`RUSTDOCFLAGS=--deny
  warnings`) all clean. | No bpb measurement: this is corpus-generation
  infra, not an experiment against a champion — `progress.jsonl` records
  this as `kind: "patch"` with null bpb deltas, same as S2-A1 through
  S2-A14. Remaining S2-D1 scope: Silesia + Canterbury fetch-and-cache, the
  five remaining structured generator classes (base64-wrapped, audio,
  image, sqlite-like, x86 binary), the three-tier train/sealed/finals
  split plumbing, regret scoring, the CI baseline gate, and progress-graph
  rendering.
- S2-A16 | ACCEPTED | ADR-0024's implementable task (issue #161), and the
  resolution of S2-D3: `literal::Literal`'s exponentiated-gradient
  mixing-weight update called `gradient.exp()` (libm, not guaranteed
  bit-identical across platforms) on both the encode and decode path.
  Replaced with a crate-local `exp` (`src/literal.rs`), built from
  IEEE-754 basic operations only: classic range reduction (`x = k*ln2 +
  r`, `|r| <= ln2/2`) into a degree-7 Taylor polynomial for `e^r`
  (accurate to `~2.5e-8` there), times `2^k` computed by exact repeated
  doubling (a local `pow2`, never a `powi` call). The argument is
  clamped to `[-30, 30]` first: `update`'s caller always clamps the
  *result* (`weight * exp(gradient)`) into `[MIN_WEIGHT, MAX_WEIGHT]`, a
  span of `1e8`, and `exp(20)` already exceeds that ratio, so the clamp
  changes no caller-visible outcome, only bounds `exp`'s own domain to
  where the polynomial needs to be accurate. `update` now takes the
  `exp` function as a parameter (`fn(f64) -> f64`) instead of calling
  `f64::exp` inline, so the test suite can pass `f64::exp` back in as an
  independent reference without duplicating the rest of the method
  (`rust-craft` skill: one function computes the mixer, encode/decode/
  test all drive the same code path, only the transcendental call
  varies). `clippy.toml` (new, workspace root) enforces the ADR crate-
  wide via `disallowed-methods` on the full `f32`/`f64` transcendental
  family; the three pre-existing encoder-only `log2` sites
  (`filters.rs` x2, `lz.rs` x1 — DP pricing and filter-selection
  entropy, ADR-0024 decision 3) carry a justified
  `#[allow(clippy::disallowed_methods)]` each. `bench/` gets its own
  `clippy.toml` overriding the list back to empty: its corpus generators
  (Box-Muller gaussian, entropy-proxy scoring) never touch a bitstream,
  so the decode-path rule does not apply and forcing `#[allow]` onto
  them would misattribute it. | Acceptance per ADR-0024: exact round-
  trip (existing suite, unchanged, still green — `update`'s signature
  changed but not its behavior on the production path) plus bits/byte
  within 1% of the kept `f64::exp` reference on a named corpus. Measured
  on `research/imports/session-1/mothergod.rs` (25,524 bytes, real
  structured Rust source, the founding archive): 10,381 bytes encoded
  through `Literal` either way — bit-identical output, 0% relative
  difference, far inside the 1% budget. `cargo fmt`/`clippy --all-
  targets -- --deny warnings`/`test --all-targets`/`test --doc`/`doc
  --no-deps` (`RUSTDOCFLAGS=--deny warnings`) all clean (intra-doc
  links to the new private `exp`/`pow2` dropped to plain code spans,
  same class as S2-A8/S2-A10/S2-A11/S2-A12, not suppressed). | No
  champion to diff against yet (same as every S2-A* slice since S2-A2):
  `progress.jsonl` records this with `kind: "patch"` and null bpb
  deltas despite the real accuracy measurement above, because that
  measurement is against a reference *within this change*, not a
  before/after ratio delta on a wired codec. S2-D2's remaining scope
  (wiring `select::pick`, `lz::parse_optimal`, `model::Model`,
  `literal::Literal`, and `coder` behind a new `Method` variant,
  `FORMAT_VERSION` bump + ADR) is no longer blocked by anything in
  S2-D3/ADR-0024; it is the next M1 slice and the first one that can
  produce a real bpb number.
- S2-A17 | ACCEPTED | ADR-0026, and S2-D2's entropy-coding wiring (filter
  selection stays remaining scope, see below): a new `src/codec.rs` wires
  `lz::parse_optimal`, the flag/length/offset/rep-slot `model::Model`
  instances, `literal::Literal`, and `coder` together as `Method::Lz`
  (`FORMAT_VERSION` 0 → 1). Ported from the archive's `encode_body`/
  `decode`, not the code (ADR-0006). `compress` now tries `Method::Lz`
  and falls back to `Method::Stored` per the Stored-floor invariant
  (`docs/format/SPEC.md`). Decode bounds allocation and loop iterations to
  the payload's own declared output length (never preallocated from it),
  and rejects a corrupt match/rep distance or a declared-length mismatch
  as `Error::Corrupt` rather than panicking (`rust-craft` skill,
  allocation- and panic-discipline) — the first code in this crate to
  face an attacker-controlled length or distance field, since `decompress`
  had only ever handled `Method::Stored` before this. | 12 new `codec`
  unit tests (round-trip across empty/single-byte/cyclic/pseudo-random/
  binary-with-zeros/a real 25,524-byte source file, plus three
  adversarial-decode cases: truncated header, a declared-length lie with
  zero tokens, and a match distance reaching before output start), 3 new
  `tests/adversarial/` seed fixtures, 3 new `lib.rs`-level tests
  (Method::Lz selection, Stored fallback for tiny and incompressible
  input); `cargo fmt`/`clippy --all-targets -- --deny
  warnings`/`test --all-targets`/`test --doc`/`doc --no-deps` all clean.
  Measured on `research/imports/session-1/mothergod.rs` (25,524 bytes,
  the same named corpus ADR-0024's accuracy test uses — not the pinned
  Silesia/Canterbury sealed set, which doesn't exist yet, S2-D1):
  **2.318 bits/byte** (7,395-byte frame), against `gzip -9`'s 2.392
  bits/byte (7,629 bytes) on the same file — a real bitstream beating a
  real baseline on one file, not yet the aggregate RATIO claim the
  scorecard wants. | Two things explicitly deferred, not forgotten:
  (1) filter selection (`filters::select::pick`, trial-encoding against
  candidate filters) stays unwired; `Method::Lz` always runs on raw
  input; S2-D2 keeps this as its remaining scope. (2) `lz::parse_optimal`
  had no non-test caller before this change (ADR-0024 verified this
  explicitly), and wiring it live surfaced a real cost hazard the module's
  own tests already warned about: `dp_round`'s rep-candidate pricing scans
  `match_len` at every position, and on a single-byte run past
  `MAX_MATCH_LEN` (65535) that scan cost compounds across positions. A
  4000-byte same-byte run encodes in milliseconds; a 200,000-byte one hung
  past 60 seconds during this PR's own development and was cut from the
  test suite rather than shipped as a slow test. This is a real encode-side
  performance concern on an unremarkable input shape (a long run of one
  repeated byte — sparse files, zero-padding), now reachable from
  `compress`'s public API for the first time. Filed as issue #179 rather
  than fixed here: the fix belongs in `lz.rs`'s DP, is
  independent of this PR's wiring concern, and risks the correctness of
  already-tested code under time pressure if bolted on. SPEED is "tracked,
  not yet optimized" until M5 per the ROADMAP scorecard, and this doesn't
  touch decode or correctness, so it does not block this slice — it does
  block shipping `compress()` as trustworthy on arbitrary real-world input
  without a fix, which is why it is a `bug`, not a `LEAD`.
  (3) Review caught a decode-side amplification bomb the fixed-token-count
  argument above didn't cover: a 14-byte payload with `declared_len ==
  token_count` (both large, no real coded bytes behind them) decodes
  successfully, since `ensure_room` never fires when the two fields agree
  with each other regardless of what the actual payload bytes support —
  2,000,000 bytes in 2.35s measured, `u32::MAX` extrapolated to roughly 84
  minutes and ~4 GiB, from the same 14 bytes. A ratio check against the
  payload's own byte count can't fix this: measured directly, this
  format's adaptive models saturate fast enough that a legitimate 60,000-
  byte same-byte input already reaches a ~3,158:1 ratio at a 19-byte
  frame, so a real maximal-ratio frame and a forged header are
  indistinguishable by size alone. Fixed with `codec::MAX_DECODED_LEN`
  (256 MiB), an explicit ceiling on the declared length checked before any
  decode work — `rust-craft`'s allocation-discipline reference's "against
  a configured ceiling" bound, chosen over "against remaining input"
  because the latter doesn't hold here. Provisional pending `ROADMAP.md`
  M4's streaming/block API.
- S2-A18 | ACCEPTED | Issue #179: `lz::parse_optimal` hung on long runs of
  a single repeated byte (200,000 bytes, over 60 seconds, killed).
  Mechanism: `dp_round::relax_rep_candidates` calls `match_len` — a
  linear scan capped at `MAX_MATCH_LEN` (65535) — once per rep-cache
  slot at *every* position, with no carry-reuse equivalent to
  `next_match_candidate`'s (which already skips a fresh hash-chain walk
  when a long match found at the previous position is still valid one
  byte shorter here). `parse_greedy` never hits this because it jumps
  ahead by a whole token's length; `dp_round` must visit every position
  to consider every possible token start, so on a run past
  `MAX_MATCH_LEN` the per-position scan cost stayed near the cap at
  every position: `O(run_length × MAX_MATCH_LEN)`. Fix: `rep_match_len`
  (`src/lz.rs`), a per-distance carry mirroring `next_match_candidate`'s
  — `match_len(data, i, d) == len` implies `match_len(data, i + 1, d) >=
  len - 1` (the same run of matched byte-equalities, shifted by one
  index), so once a scan finds a run at or past `CARRY_MIN_LEN` a later
  position on the same distance decrements instead of re-scanning. Two
  bugs surfaced and were fixed while building this: (1) a first version
  keyed the carry by rep-cache *slot index*; `RepCache::promote`/
  `push_front` reorder slots on ties, which are common once a run
  passes `MAX_MATCH_LEN` and every slot's scan caps at the same length,
  so a slot-index-keyed carry went stale on every reorder and
  reproduced the same quadratic cost one level up (confirmed by
  instrumenting `match_len` call/step counts: fresh full-length scans
  jumped from ~3 to ~10,600 out of ~117,000 total calls between
  100,000- and 50,000-byte runs). Fixed by searching every carry entry
  for a matching *distance* instead of a matching array position. (2)
  that distance-keyed version then let a carry entry go stale in a
  different way: a distance can drop out of every rep slot for a
  stretch of positions and later reappear (e.g. a match's distance
  cycling back into the cache), and reusing its old length without
  accounting for the elapsed positions overestimates the true match
  length — caught by a debug-mode `attempt to subtract with overflow`
  panic during manual timing verification, then by an index-out-of-
  bounds panic in `dp_round` once the panic itself was fixed blind.
  Fixed by storing the position each entry was last measured at and
  computing `len.saturating_sub(i - measured_at)` at lookup time, a
  lower bound valid for any elapsed gap by induction, not just a
  single step. | Verified by execution, not just the round-trip suite:
  a 200,000-byte single-repeated-byte input (matching the issue's own
  repro) now completes in under 1s in an unoptimized debug build,
  versus over 60s before: `lz::tests::
  optimal_roundtrip_long_run_of_one_repeated_byte_stays_linear` pins
  this with a 15s wall-clock regression bound (generous margin over a
  slower CI runner) alongside the existing round-trip assertion.
  Scaling checked directly, not assumed: 50k/100k/200k/400k/800k-byte
  same-byte runs measured at ~0.14s/0.36s/0.93s/2.0s/4.3s, consistent
  with linear, not quadratic, growth (the unfixed code could not
  complete even the smaller sizes in this suite within a 30s timeout).
  Also checked non-uniform inputs (a period-137 byte pattern, a cyclic
  0..=255 sequence) for the same class of hang; both complete, though
  the period-137 case is still slow (~5.8s for 300,000 bytes) from a
  separate, pre-existing mechanism (`next_match_candidate`'s own
  hash-chain search, `MAX_CHAIN_TRIES_OPTIMAL` candidates each costing
  a full `match_len` scan on a refresh) that this fix does not touch —
  out of scope for issue #179, which named the rep-candidate scan
  specifically; worth its own lead if it matters in practice. `cargo
  fmt --check`, `clippy --all-targets -- --deny warnings`, `test
  --all-targets`, `test --doc`, `RUSTDOCFLAGS=--deny warnings cargo doc
  --no-deps` all clean. | S2-A17 landed on `main` while this fix was in
  progress: `lz::parse_optimal` is no longer test-only, `Method::Lz`
  (`compress`'s public path) calls it on every input, so the hang this
  entry fixes was reachable from the crate's real API, not a dormant
  cost in unwired code. Still no bpb delta here: this changes encode-
  side cost and robustness, not the bits a fixed input encodes to.
- S2-A19 | ACCEPTED | ADR-0028, S2-D2's remaining scope, in full: wires
  `filters::select::pick`'s trial selection into `Method::Lz`.
  `codec::encode` now trials every candidate `pick` shortlists (identity,
  delta, BCJ, transpose), running each through the same LZ +
  context-mixing pipeline S2-A17 wired, and keeps whichever candidate's
  encoded body is smallest. The winner is a 2-byte selector
  (`[kind, param]`, `filters::select::Candidate::to_header_bytes`)
  prefixed onto the payload, an explicit tag rather than the archive's
  packed single-byte scheme (`0..=96`=delta stride, `97`=BCJ,
  `100..=113`=transpose column index): the archive's packing needs a
  private lookup table (`TRANSPOSE_COLUMNS`) shared between the module
  that picks candidates and the module that (de)serializes them, and this
  port keeps that mapping in exactly one place instead. `FORMAT_VERSION`
  1 → 2 (CLAUDE.md hard rule 5): a version-1 `Method::Lz` payload used the
  layout without the filter prefix, so `codec::LZ_MIN_VERSION` makes
  `decompress` reject that version/method combination as
  `Error::UnsupportedVersion` explicitly, rather than relying on
  `codec::decode`'s adversarial-input defenses to fail safely on the
  misread by coincidence. All four filters preserve length, so the
  existing declared-output-length field needs no format change — it
  already means "length of the filtered bytes," and the filter is
  reversed only after that length is confirmed. | 4 new `filters.rs`
  tests (`Candidate` header-byte round trip across every kind, reject
  unknown kind, reject zero param on a parameterized kind, reject nonzero
  param on a parameterless kind), 3 new `codec.rs` tests (a synthetic
  columnar-drift round trip that asserts a non-identity filter was
  actually selected and correctly reversed — not just plumbed through
  unused; an unknown-filter-selector decode rejection; a version-gating
  regression at the `lib.rs` public-API level), 3 existing `codec.rs`
  hand-crafted-payload tests updated for the 2-byte prefix, 4 existing
  `tests/adversarial/lz-*` seed fixtures regenerated under the new
  layout (kept exercising `codec::decode`'s own bomb/mismatch/truncation
  handling — without regenerating them they would have started passing
  for the wrong reason, short-circuited by the new version gate instead
  of the hazard checks they were built to test) plus one new fixture for
  the unrecognized-filter-selector case; `cargo fmt`/`clippy --all-targets
  -- --deny warnings`/`test --all-targets`/`test --doc`/`doc --no-deps`
  all clean. | Measured on `research/imports/session-1/mothergod.rs`
  (25,524 bytes, same named corpus as S2-A17): **2.3184 bits/byte**
  (7,397-byte frame), `Candidate::Identity` selected — unchanged from
  S2-A17's 2.318 within rounding. Expected, not a bug: this file is
  structured Rust source text, and S1-R1 already found delta loses on
  text (numeric differences of letters are more scattered than the
  letters themselves); transpose needs fixed-width records this file
  doesn't have. A real ratio win from this slice needs a corpus with
  that shape — `bench/`'s structured generators or the eventual
  Silesia/Canterbury fetch (S2-D1), not this file. The wiring itself is
  proven correct independent of this corpus's null result, by the
  synthetic columnar-drift round-trip test above.
- S2-A20 | ACCEPTED | Third structured-generator slice of M2's remaining
  benchmark-harness debt (S2-D1): a base64-wrapped text payload
  (`research/corpus/POLICY.md`'s "base64-wrapped payloads" class) ported
  to `bench/src/lib.rs` as `base64_wrapped`, mirroring `access_log`/
  `json_records`'s structure. Behavior ported from the founding session's
  `corpus.py` (`c['b64-text']`, `git show
  1a3b1c8:research/imports/session-1/corpus.py`), not the code
  (ADR-0006): base64-encode a text-like payload and truncate to length.
  The archive draws its text from `/usr/share/doc/*/copyright` on the
  host filesystem, neither deterministic nor available in every
  environment; this port substitutes `json_records`, this module's own
  synthetic text source, keeping the same "compressible source pushed
  through base64's 6-bit encoding" shape. The archive's second variant,
  `b64-random` (base64 of `os.urandom`), is not ported: `entropy_ladder`
  already covers a maximum-entropy source, and wrapping it in base64
  changes only the alphabet, not the coverage. New standalone
  `base64_encode` helper (standard RFC 4648 alphabet, `=` padding): a
  second, from-scratch copy of the table `src/filters.rs`'s
  `base64_unwrap` filter also carries, kept separate rather than reused
  because `bench` never reaches into `src/` internals for corpus
  generation (every generator here is self-contained) and the alphabet
  is a fixed public standard, not project logic, so the duplication
  carries no drift risk. | 5 new unit tests: an RFC 4648 test-vector
  check on `base64_encode` itself, exact-length output across five
  requested lengths, determinism, seed independence, and an
  alphabet-membership check (every output byte is base64-alphabet or
  `=`); wired into the existing frame-format round-trip test. `cargo
  fmt`/`clippy --all-targets -- --deny warnings`/`test --all-targets`/
  `test --doc`/`doc --no-deps` (`RUSTDOCFLAGS=--deny warnings`) all
  clean. | No bpb measurement: this is corpus-generation infra, not an
  experiment against a champion — `progress.jsonl` records this as
  `kind: "patch"` with null bpb deltas, same as S2-A1 through S2-A15.
  Remaining S2-D1 scope: Silesia + Canterbury fetch-and-cache, the four
  remaining structured generator classes (audio, image, sqlite-like, x86
  binary), the three-tier train/sealed/finals split plumbing, regret
  scoring, the CI baseline gate, and progress-graph rendering. This
  entry's S2 number was renumbered from S2-A19 to S2-A20 (and
  `progress.jsonl`'s from it60 to it61) after PR #194 landed S2-A19/it60
  first, the same collision-and-renumber this journal already records for
  S2-A17/S2-A18.
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
