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
- S2-D2 | DEBT | Remainder of M1 after the S2-A2 delta-filter slice: the
  other filter kinds (transpose, BCJ, base64-unwrap, reverse) and the
  `pick_filters` trial-selection heuristic; the optimal-parse LZ stage
  (`lz`/`lz_opt`, 3-slot repeat-offset cache, priced DP); the context-mixing
  entropy models (`Lit` six-expert arena, flag/length/offset models); the
  range coder (`Enc`/`Dec`); and wiring all of it behind a new `Method`
  variant with a `FORMAT_VERSION` bump + ADR (CLAUDE.md hard rule 5). Source:
  `research/imports/session-1/mothergod.rs` (526 lines, golfed — port
  behavior, not code, per ADR-0006). One PR per module per the M1 checklist.
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
