# Research journal

Falsification record and institutional memory. Append-only in spirit: never
delete an entry; a revived idea gets a NEW entry referencing the old one.
Audience: agents. Terse. Mechanisms over scores.

Format per entry: `id | verdict | claim | mechanism/evidence | conditions`.
Verdicts: LAW (holds until falsified), ACCEPTED, REJECTED, LEAD (untested),
DEBT (known gap with named fix).
A DEBT entry that blocks a ROADMAP milestone links a tracking issue,
because this journal is memory, not a queue: only issues get picked up
(#165, S2-D3 stalled M1 for two days). DEBT that blocks nothing stays
issue-less.

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
- S2-R1 | REJECTED | S1-P1's third slice: wired `Sse` + `encode_bit`/
  `decode_bit` (S2-A40/S2-A41) behind the flag model's literal/copy
  sub-decision, decomposing the old three-ary flag `Model` into an
  SSE-calibrated `is_copy` bit plus a `copy_kind` bit (`FORMAT_VERSION` 3
  candidate; kept `codec::decode` able to read `FORMAT_VERSION` 2 frames
  too, since `tests/golden/v2-lz-repeated-text` commits this crate to
  that forever). Measured on `bench::baseline`'s 11 train-tier cases and
  the two sealed-only kinds (`access_log`, `gradient_image`): train net
  effect ~+0.011 b/B worse (7 of 11 cases regressed, all individually
  under `TOLERANCE_BITS`; `entropy_ladder_h6` alone +0.007), sealed split
  one improvement (`access_log` −0.0014) and one regression
  (`gradient_image` +0.0072). No train improvement and one validation
  regression fails corpus policy's accept rule outright. Mechanism: an
  order-0 adaptive frequency count over a single binary outcome (what
  `is_copy` already was before this change) has little systematic
  calibration bias for SSE to correct — SSE earns its keep calibrating a
  *compound* estimate (several signals blended, e.g. a context-mixing
  predictor's output), not a lone frequency counter already tracking its
  own rate — consistent with the small but real ladder-case tax
  `research/corpus/POLICY.md`'s entropy ladder exists to catch. Separately,
  none of the train/sealed generators are natural-language text, so this
  slice never actually tested S1-P1's named target (the five zstd text
  holdouts); that is a real corpus gap, not a workaround (finals are
  never inside the experiment loop, so Canterbury could not have been the
  accept signal here regardless). Candidate code (the `codec.rs`/`lib.rs`
  wiring and dual-version decode) reverted in full, per the
  `compression-experiment` skill's "delete rejected candidate code" —
  `Sse` and `Encoder::encode_bit`/`Decoder::decode_bit` themselves
  (S2-A40/S2-A41) are unaffected, since this rejection is about their
  combination with `is_copy`, not their own correctness.
- S2-R2 | REJECTED | S1-P2's wiring slice: swapped `dp_round`'s hash-chain
  `MatchFinder` for `BinaryTreeMatchFinder` (S2-A42), matching each
  position's `finder.insert(i)` + `finder.find_best(i, ...)` pair with one
  `finder.insert_and_find(i, MAX_TREE_DEPTH_OPTIMAL)` call (same 640 depth
  as the retired `MAX_CHAIN_TRIES_OPTIMAL`, held equal on purpose so the
  measurement isolated the finder swap). Ratio-wise this won outright: net
  train effect ~−0.054 b/B across `bench::baseline`'s 11 cases (no case
  regressed; `entropy_ladder_h1` −0.029, `entropy_ladder_h2` −0.012,
  `markov_h8_2_trap` −0.012 carried most of it), sealed split both flat or
  improved (`access_log` −0.00016, `gradient_image` unchanged). Rejected
  anyway: `cargo test --all-targets` failed on
  `lz::tests::optimal_roundtrip_long_run_of_one_repeated_byte_stays_linear`,
  a committed regression guard (issue #179) that requires a 200,000-byte
  single-byte-run to parse in under 15s — this wiring took 71s. Mechanism:
  `BinaryTreeMatchFinder::insert_and_find` fuses insertion with search in
  one mutating call (S2-A42's own docs), so `dp_round`'s `carry` reuse can
  no longer skip the walk on a long run the way it skipped
  `MatchFinder::find_best` — it can only skip *using* a fresher result,
  not computing one. Compounding that, S2-A42 also deliberately deferred
  the length-prefix-reuse optimization (`len0`/`len1`) real bt4 finders use
  to keep each comparison near the tree height; without it, highly
  repetitive data (near-identical suffixes everywhere) makes
  `suffix_common_len` return close to `MAX_MATCH_LEN` (65535) on most of
  the up to `max_depth` (640) candidates `insert_and_find` visits per
  position, an unbounded-by-carry O(max_depth × MAX_MATCH_LEN) per-position
  cost the hash-chain path never had. A ratio-only measurement (this
  journal's usual accept gate) would have missed this: the failure surfaced
  from the existing test suite, not from `bench::baseline`. Next attempt
  needs the length-prefix-reuse optimization, a cheap insert-only fast path
  so `carry` can skip the walk again, or both, before this finder can
  safely replace `MatchFinder` in `dp_round`. Candidate code (the
  `dp_round`/`next_match_candidate` wiring and `MAX_TREE_DEPTH_OPTIMAL`)
  reverted in full; `BinaryTreeMatchFinder` itself (S2-A42) is unaffected.
- S2-R3 | REJECTED | S1-P2's other wiring slice: fed `dp_round`'s own
  forward pass into `PriceCounts::observe` (S2-A50) as it advanced,
  rebuilding `PriceTable` from the running counts every
  `PRICE_REBUILD_INTERVAL` (4,096) finalized moves instead of leaving
  prices frozen at the round's seed table for its whole pass. Every
  position's `dp[i]`/`parent[i]` is finalized in strictly increasing order
  (proved in S2-A50), so observing `parent[i]`'s move the instant the loop
  reaches `i` is sound regardless of whether `i` ends up on the final
  backtrace. Measured on `bench::baseline`'s 11 train-tier cases and the
  two sealed-only kinds: train net effect ~−0.050 b/B
  (`entropy_ladder_h4` −0.027, `entropy_ladder_h6` −0.016,
  `markov_h8_2_trap` −0.011 carried most of it; two cases regressed within
  tolerance, `base64_wrapped` +0.0096 and `json_records` +0.0077), sealed
  split one regression (`access_log` +0.0178) and one improvement
  (`gradient_image` −0.0053). One validation regression fails corpus
  policy's accept rule outright, independent of the net train number.
  Mechanism: observing *every* position's finalizing move, not just the
  tokens the final backtrace actually uses, feeds the running price table
  with locally-competitive-but-discarded candidates alongside real ones:
  for literal-context-heavy, text-like data (`json_records`, `access_log`)
  that noise measurably hurts the literal price table's fit to the
  sequence actually emitted; for near-memoryless sources (the entropy
  ladder, the markov trap) more samples of any kind help pure frequency
  convergence regardless of which candidate produced them, which is why
  the net aggregate looks like a win while the named target
  (sqlite/json/jsonl) does not move favorably, the same shape of false
  signal S2-A47 flagged once already. Swept `PRICE_REBUILD_INTERVAL` at
  256/512/1024/2048/4096 on train only (`research/corpus/POLICY.md`
  permits tuning against train): the per-case sign pattern was identical
  at every value, so this is not an undertuned cadence, it is the sampling
  rule itself. Next attempt, if any, wants to observe only tokens that
  survive to the final backtrace (a backward pass after the forward DP
  completes, or restructuring the loop to discover the backtrace
  incrementally), not every position's locally-finalized move. Candidate
  code (the `dp_round`/`parse_optimal` wiring, `move_to_token`,
  `PRICE_REBUILD_INTERVAL`) reverted in full; `PriceCounts::observe`/
  `tally` themselves (S2-A50) are unaffected.
- S2-R4 | REJECTED | S2-A56's own closing line named the next candidate
  slice: whether a fourth `dp_round` keeps paying. It does on train but not
  on sealed, so it is rejected on the same rule that sank S2-R3, at a much
  smaller magnitude. Wiring: identical pattern to S2-A56's own addition,
  reseeding a fourth round's price table from the third round's own token
  sequence; no new DP machinery. Measured on `bench::baseline`'s 11 train
  cases and the two sealed-only kinds (`access_log`, `gradient_image`),
  `CASE_LEN` 50,000, fixed seeds: train net effect ~−0.0197 b/B, nine of
  eleven cases improved (`entropy_ladder_h6` −0.00736, `entropy_ladder_h2`
  −0.00432, `markov_h8_2_trap` −0.00416 carried most of it), two flat
  (`entropy_ladder_h8`, `interleaved_audio16`), one regression within
  `TOLERANCE_BITS` (`base64_wrapped` +0.00208, its second round in a row
  moving the wrong way, now S2-A56 and S2-R4 both). Sealed split: one
  regression, `access_log` +0.00032; one improvement, `gradient_image`
  −0.00288. The `access_log` regression is two orders of magnitude smaller
  than S2-R3's (+0.0178) but the corpus policy's accept rule draws no
  tolerance line for the sealed set the way `TOLERANCE_BITS` does for the
  CI gate: "no validation regression" is binary, and S2-R3's own text
  already ruled a validation regression fails the accept rule "independent
  of the net train number." Applying a magnitude carve-out here that S2-R3
  did not get would be tuning the accept rule against the outcome, not
  applying it. Mechanism not diagnosed further (unlike S2-R3, this is not a
  new sampling rule, just one more reseed of an already-converging table;
  a plausible read is the DP approaching a fixed point where each
  subsequent round's price table overfits the previous round's specific
  token sequence rather than the source, with `access_log`'s literal-heavy
  structure the first to show it). `dp_round`/`parse_optimal` unchanged
  from S2-A56's three-round shape; candidate code (the fourth `dp_round`
  call and its reseed) never committed, only exercised via a local
  scratch example, deleted after measurement. Remaining S1-P2 scope
  unchanged: an observation rule limited to backtrace survivors, still
  unbuilt (S2-A51/S2-R3's stopping point). A fifth-plus round is not a
  standing lead on this evidence; the DP round count stays at three until
  something changes the mechanism, not just the parameter.
- S2-R5 | REJECTED | S1-P2's own named remaining scope after S2-R3 (and
  still open after S2-R4, an unrelated fourth-round variant): an
  observation rule limited to tokens that survive to the final backtrace,
  not every position's locally-finalized move. Implemented as a backward
  walk over `state.parent` every `OBSERVE_INTERVAL` (4,096) positions,
  from the current (already-finalized, per S2-A50) position back to the
  previous checkpoint, feeding only the moves actually on that walk into
  `PriceCounts::observe` — unlike S2-R3, a discarded `relax` candidate is
  never counted, only a move the DP kept. Not the round's *true* final
  backtrace either (that only exists once position `n` is reached, and an
  intermediate checkpoint's chain to reach position `i` can differ from
  the chain the eventual full backtrace uses to cross the same span), but
  strictly closer to it than S2-R3's every-candidate sweep. Measured on
  `bench::baseline`'s 11 train-tier cases and the two sealed-only kinds,
  `CASE_LEN` 50,000, fixed seeds: net train effect ~0.000 b/B (five cases
  improved, one flat, five regressed, no net direction —
  `entropy_ladder_h4` −0.09184 the largest single mover, offset almost
  exactly by `x86_dense_code` +0.032, `base64_wrapped` +0.01584,
  `markov_h8_2_trap`/`entropy_ladder_h2` +0.0248 each,
  `entropy_ladder_h1` +0.01152). Sealed split: `access_log` +0.00256
  (regression), `gradient_image` −0.01264 (improvement). One validation
  regression fails corpus policy's accept rule outright, independent of
  the net train number (S2-R3's own ruling). S1-P2's named
  sqlite/json/jsonl target was a wash, not a win: `sqlite_like_records`
  −0.00208, `json_records` +0.00208, exactly offsetting. Mechanism: this
  slice specifically removed S2-R3's diagnosed noise source (discarded
  candidates), and the sealed regression persisted anyway — evidence
  S2-R3's "candidate noise" diagnosis was not the complete explanation.
  A better-supported read: each checkpoint's running counts are built
  from only the file's own prefix consumed so far, a partial and
  potentially unrepresentative sample once a source's structure varies
  over its length the way literal-heavy log/record formats
  (`access_log`, `json_records` — the two cases both this slice and
  S2-R3 regressed) do; repricing mid-round pulls the table toward the
  prefix's specific shape at the cost of the remainder, a recency bias
  rather than a candidate-selection one. Candidate code (the `dp_round`
  checkpoint walk, `OBSERVE_INTERVAL`, the `move_len`/`move_to_token`
  extraction from `reconstruct`, and the added test) reverted in full,
  matching every prior S1-P2 rejection; `PriceCounts::observe`/`tally`
  themselves (S2-A50) are unaffected.
- S2-R6 | REJECTED | S1-P3's own remaining scope (S2-A57's module doc):
  pick where the PPM escape's lower-order fallback lands. Tried "order-0"
  — [`crate::literal::Literal`] already has one non-context-keyed bank
  (`ORDER0_BASE`) with strictly more evidence than any of its five
  context-specific experts, and every one of those five is exactly as
  likely to be sparse as whichever one is escaping, so order-0 was the
  best-reasoned of the doc's three named candidates. Measured before
  committing to a wiring: a new pairing method,
  `Literal::ideal_cost_bits_escape_fallback_experiment`, computed each
  literal byte's ideal cost twice from the same pre-update model state —
  once under the shipped mix, once with a substitution rule (an expert
  whose own bank has never observed this exact symbol beyond its initial
  Laplace floor, `freq == 1`, contributes order-0's own frequency/total
  for that symbol instead) — updating the model only once, from the real
  frequencies, so the pair shares one adaptation trajectory and differs
  only in what it's priced at. `Literal::mix`'s own weighted-average
  identity guarantees the baseline side sums to `1.0` across all 256
  symbols; the substituted side generally does not (a bank's 256 entries
  become a mix of its own normalized mass and order-0's, two
  differently-normalized sources), so the pairing method builds the full
  256-symbol distribution both ways and divides by each one's own true
  sum rather than assuming one — caught by a test that initially asserted
  the wrong invariant (see the accepted code, `mixed_distribution`'s own
  doc). Not wired into `Method`/`FORMAT_VERSION`; a matching whole-file
  pairing, `codec::ideal_cost_bits_escape_fallback_experiment`, priced
  every non-literal symbol identically into both totals (the hypothesis
  names literal contexts only) and let a scratch bin
  (`bench/src/bin/scratch_ppm_fallback.rs`, deleted after this
  measurement per `research/README.md`'s convention) run it over
  `bench::baseline`'s 11 train cases and the two sealed-only kinds, fixed
  seeds, `CASE_LEN` 50,000, matching S2-A56's methodology. Net regression:
  +0.0458 b/B average across the 11 train cases (5 improved, 6
  regressed), dominated by `interleaved_audio16` (+0.243097),
  `markov_h8_2_trap` (+0.128494), `x86_dense_code` (+0.078521), and
  S1-P3's own named target `sqlite_like_records` (+0.039703, the wrong
  direction). Sealed split: `access_log` −0.001425 (mild improvement),
  `gradient_image` +0.541159 (severe regression) — one validation
  regression fails corpus policy's accept rule outright, independent of
  the net train number. Only the entropy ladder's lower-order points and
  `json_records` improved (`entropy_ladder_h1` −0.002357 through `_h6`
  −0.007524, `json_records` −0.002649), `entropy_ladder_h8` flipped
  positive (+0.019302). Mechanism: order-0's fallback helps exactly when
  a byte's likelihood does not depend on context — true by construction
  for the entropy ladder (IID sources), where the global marginal and any
  local one coincide — and actively hurts whenever it does, which is
  every other case tested, including two purpose-built or real-world
  classes: `markov_h8_2_trap` is specifically constructed so the global
  histogram is uniform and uninformative while conditional structure
  carries all the signal, so substituting the global marginal into a
  sparse context destroys exactly the information the mixer needs; the
  four structured generators (`interleaved_audio16`, `gradient_image`,
  `sqlite_like_records`, `x86_dense_code`) each have systematically
  different local distributions across positions/contexts by
  construction (interleaved channels, image rows, fixed-width records,
  opcode patterns), so a fresh context's "never seen here" usually means
  "hasn't recurred yet," not "globally rare," and order-0's confidently-
  skewed global answer is worse than the neutral floor it replaced. This
  doc's own distinction from `JOURNAL` S1-R5 ("this primitive escapes
  only a genuinely never-seen symbol; a well-trained context essentially
  never pays the escape cost") holds in principle but does not save the
  result: a context-specific bank stays sparse for a long time in exactly
  the structured formats this project targets, so the fallback fires
  often enough to matter — a milder version of S1-R5's own failure mode
  (leaning on order-0 damages the contexts that most need to stay
  confident), reached by a different, more conditional route. Candidate
  code (`Literal::mixed_distribution`,
  `Literal::ideal_cost_bits_escape_fallback_experiment`, their five unit
  tests, and `codec::EscapeFallbackExperimentSink`/
  `ideal_cost_bits_escape_fallback_experiment`) reverted in full; `Ppm`
  itself (S2-A57) is unaffected — this slice never routed through it (see
  its own module doc for why: `Literal`'s "unseen" signal is `freq == 1`,
  one level up from `Ppm`'s `freq == 0`, so a second copy of `Ppm`'s
  bookkeeping would have duplicated state `Literal` already carries).
  `research/progress.jsonl` it106. Remaining S1-P3 scope: unclear, the
  same shape S1-P2 reached after its own repeated rejections — a fallback
  target other than "the global marginal," not a third variant of "which
  existing bank," is owed before spending another slice here.

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
  of. Structured generator classes done as of S2-A24 (all seven ported,
  S2-A14 through S2-A24); fetch-and-cache done as of S2-A26; decompression
  done as of S2-A28; split plumbing's rotating-window piece done as of
  S2-A29; the sealed-validation split's seed derivation done as of
  S2-A32; its dataset-kind separation done as of S2-A33; regret scoring
  done as of S2-A34; progress-graph rendering (of `bench/baseline.json`,
  the only real numbers available) done as of S2-A36; the gzip/zstd/xz
  reference column and real numbers on one held-out final (Canterbury)
  done as of S2-A37; the scheduled `--features corpus-fetch` workflow
  done as of S2-A45; the CI baseline gate wired as the required `ratio`
  check in `ci.yml` (issue #284, ROADMAP M2's gate box checked).
  Remaining, no longer milestone-blocking so carrying no tracking issue
  per the #165 convention: real Silesia numbers (S2-A37's `finals_report`
  binary covers Silesia too, in code — its remaining scope is throughput,
  not a missing feature: ~0.14 MB/s measured means the full corpus needs
  on the order of half an hour of `mothergod::compress` time, too slow
  for a by-hand run; revisit when M3+ speed work lands, as a scheduled
  job before a per-PR gate).
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
- S2-A21 | ACCEPTED | Fourth structured-generator slice of M2's remaining
  benchmark-harness debt (S2-D1): interleaved 16-bit audio samples
  (`research/corpus/POLICY.md`'s "audio" class) ported to `bench/src/lib.rs`
  as `interleaved_audio16`, mirroring `access_log`/`json_records`/
  `base64_wrapped`'s structure. Behavior ported from the founding session's
  `corpus.py` (`c['audio16']`, `git show
  1a3b1c8:research/imports/session-1/corpus.py`), not the code (ADR-0006):
  each sample sums a slow sine (amplitude 2500, period 37 samples), a fast
  sine (amplitude 1500, period 11 samples), and gaussian noise (stddev
  200), truncated toward zero and kept to the low 16 bits. Python's
  `int(...)` truncates a float toward zero exactly like `as i64` does, and
  its `& 0xffff` on a (possibly negative) arbitrary-precision int keeps the
  low 16 bits in two's complement, exactly what `as u16` produces from that
  `i64` — both cast steps carry a `#[allow]` with that reasoning rather
  than a `try_from`, since the wraparound is intentional, not an error
  case. One behavior-preserving deviation, the same shape as `access_log`'s:
  the archive fixes the sample count at `N / 2` then emits exactly that
  many bytes (so an odd `N` silently loses its last requested byte); this
  port generates samples until `len` bytes are reached then truncates, so
  it produces exactly `len` bytes for any requested length. `sin`/`cos` are
  disallowed crate-wide by ADR-0024's `clippy.toml` (decode-path
  cross-platform determinism), but `bench/`'s own `clippy.toml` already
  overrides that back to empty (S2-A16) since corpus generation never
  touches a bitstream. | 4 new unit tests: exact-length output across six
  requested lengths including odd ones, determinism, seed independence, and
  a structural check that consecutive 16-bit samples (little-endian) mostly
  differ by far less than the full range, unlike iid data; wired into the
  existing frame-format round-trip test. `cargo fmt`/`clippy --all-targets
  -- --deny warnings`/`test --all-targets`/`test --doc`/`doc --no-deps`
  (`RUSTDOCFLAGS=--deny warnings`) all clean. | No bpb measurement: this is
  corpus-generation infra, not an experiment against a champion —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas,
  same as S2-A1 through S2-A20. Remaining S2-D1 scope: Silesia + Canterbury
  fetch-and-cache, the three remaining structured generator classes (image,
  sqlite-like, x86 binary), the three-tier train/sealed/finals split
  plumbing, regret scoring, the CI baseline gate, and progress-graph
  rendering.
- S2-A22 | ACCEPTED | Fifth structured-generator slice of M2's remaining
  benchmark-harness debt (S2-D1): a synthetic grayscale gradient image
  (`research/corpus/POLICY.md`'s "gradient image" class) ported to
  `bench/src/lib.rs` as `gradient_image`, mirroring `interleaved_audio16`'s
  structure. Behavior ported from the founding session's `corpus.py`
  (`c['image']`, `git show 1a3b1c8:research/imports/session-1/corpus.py`),
  not the code (ADR-0006): row-major pixels over 200-pixel-wide rows, each
  the sum of a baseline (90), a horizontal sine (amplitude 70, period 31
  pixels), a vertical sine (amplitude 50, period 23 rows), and gaussian
  noise (stddev 8), truncated toward zero and kept to the low byte.
  Python's `int(...)` truncates a float toward zero exactly like `as i32`
  does, and its `& 0xff` on a (possibly negative) arbitrary-precision int
  keeps the low byte in two's complement, exactly what `as u8` produces
  from that `i32`. One behavior-preserving deviation, the same shape as
  `interleaved_audio16`'s: the archive fixes the row count at `N / 200 + 1`
  then truncates the flattened result to `N` bytes; this port generates
  pixels until `len` bytes are reached then stops, so it produces exactly
  `len` bytes for any requested length. | 4 new unit tests: exact-length
  output across six requested lengths, determinism, seed independence, and
  a structural check that consecutive pixels within a row mostly differ by
  far less than the full byte range, unlike iid data; wired into the
  existing frame-format round-trip test. `cargo fmt`/`clippy --all-targets
  -- --deny warnings`/`test --all-targets`/`test --doc`/`doc --no-deps`
  (`RUSTDOCFLAGS=--deny warnings`) all clean. | No bpb measurement: this is
  corpus-generation infra, not an experiment against a champion —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas,
  same as S2-A1 through S2-A21. Remaining S2-D1 scope: Silesia + Canterbury
  fetch-and-cache, the two remaining structured generator classes
  (sqlite-like, x86 binary), the three-tier train/sealed/finals split
  plumbing, regret scoring, the CI baseline gate, and progress-graph
  rendering.
- S2-A23 | ACCEPTED | Sixth structured-generator slice of M2's remaining
  benchmark-harness debt (S2-D1): fixed-width binary rows over a
  timestamp/category/measurement schema (`research/corpus/POLICY.md`'s
  "sqlite-like records" class) ported to `bench/src/lib.rs` as
  `sqlite_like_records`. Behavior ported from the founding session's
  `corpus.py` (`c['sqlite']`, `git show
  1a3b1c8:research/imports/session-1/corpus.py`), not the code (ADR-0006):
  the archive opened a real `sqlite3` connection, created `table
  m(ts int, s text, v real)`, and inserted rows of a linearly increasing
  timestamp (`1700000000 + i*60`), a category drawn from `{temp, hum,
  pres}`, and a gaussian measurement (mean 20, stddev 3), then read the
  resulting file's raw bytes. Unlike the audio/image classes, there is no
  formula to port for the byte layout itself: the archive's on-disk bytes
  were whatever the installed `sqlite3` library's page format, freelist
  state, and per-value varint serial-type encoding happened to produce, not
  a design choice recorded anywhere, and reproducing them exactly would
  mean re-implementing SQLite's storage engine — out of scope for a
  zero-dependency corpus generator (ADR-0002) and not what "sqlite-like"
  asks for. This port instead captures the schema's shape directly: each
  row is a fixed 20-byte little-endian record (8-byte timestamp, 4-byte
  null-padded category, 8-byte measurement), which exercises the same
  repeated-structure/mixed-type compression opportunity the class exists
  to probe. | 5 new unit tests: exact-length output across six requested
  lengths including odd ones, determinism, seed independence, a check that
  every full row's category field is one of the three fixed values, and a
  check that timestamps strictly increase row over row; wired into the
  existing frame-format round-trip test. `cargo fmt`/`clippy --all-targets
  -- --deny warnings`/`test --all-targets`/`test --doc`/`doc --no-deps`
  (`RUSTDOCFLAGS=--deny warnings`) all clean. | No bpb measurement: this is
  corpus-generation infra, not an experiment against a champion —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas,
  same as S2-A1 through S2-A22. Remaining S2-D1 scope: Silesia + Canterbury
  fetch-and-cache, the one remaining structured generator class (x86
  binary), the three-tier train/sealed/finals split plumbing, regret
  scoring, the CI baseline gate, and progress-graph rendering.
- S2-A24 | ACCEPTED | Seventh and final structured-generator slice of M2's
  remaining benchmark-harness debt (S2-D1): a synthetic x86-64 instruction
  stream dense with `call`/`jmp rel32` opcodes
  (`research/corpus/POLICY.md`'s "x86-dense binaries" class) ported to
  `bench/src/lib.rs` as `x86_dense_code`. Behavior ported from the founding
  session's `corpus.py` (`c['elf']`, `git show
  1a3b1c8:research/imports/session-1/corpus.py`), not the code (ADR-0006):
  the archive read 40,000 bytes from an offset into the host's installed
  `libc.so.6`, which is neither deterministic (varies by libc build) nor
  available in every environment (nothing to read on a host without one).
  Same deviation shape as S2-A23's `sqlite_like_records`: rather than real
  machine code, the port captures the structural property the class exists
  to probe — `src/filters.rs`'s `bcj` doc comment notes call targets
  cluster on a small set of functions, which as relative offsets encode
  differently per occurrence but as absolute addresses collide, which is
  what a downstream model actually matches. `x86_dense_code` emits a stream
  of short filler instructions (prologue/epilogue, register moves,
  arithmetic, short conditional jumps, 15-entry pool) with a 25% per-step
  chance of a `call`/`jmp rel32` targeting one of 48 synthetic function
  starts spaced 64 bytes apart instead — deliberately denser in call/jmp
  opcodes than real compiled code, since the class exists to stress the
  `bcj` filter (S2-A4) rather than to resemble a realistic binary. | 5 new
  unit tests: exact-length output across seven requested lengths including
  ones shorter than one instruction, determinism, seed independence, a
  floor check that call/jmp opcodes are >5% of bytes (real code is far
  sparser; this is a "dense" corpus by design), and a direct round trip
  through `filters::bcj::encode`/`decode` (independent of the frame-format
  round trip every generator gets); wired into the existing frame-format
  round-trip test. `cargo fmt`/`clippy --all-targets -- --deny
  warnings`/`test --all-targets`/`test --doc`/`doc --no-deps`
  (`RUSTDOCFLAGS=--deny warnings`) all clean. | No bpb measurement, same
  reason as S2-A1 through S2-A23: corpus-generation infra, not an
  experiment against a champion — `progress.jsonl` records this as `kind:
  "patch"` with null bpb deltas. S2-D1's structured-generator list is now
  complete (jsonl/log, json, base64-wrapped, audio, image, sqlite-like,
  x86-dense — all seven classes ported); remaining S2-D1 scope: Silesia +
  Canterbury fetch-and-cache, the three-tier train/sealed/finals split
  plumbing, regret scoring, the CI baseline gate, and progress-graph
  rendering.
- S2-A25 | ACCEPTED | ROADMAP M4's first slice (issue #53, journal lead
  S1-P7): a `fuzz/` crate (`cargo-fuzz`, dev-only tooling, ADR-0006
  compliant) with two targets against the real codec, not the archive.
  `decode_arbitrary` feeds arbitrary bytes to `mothergod::decompress` —
  hard rule 2 ("the decoder never panics, never overallocates unbounded,
  on ANY input") as an executable, exercising both `Method::Stored` and
  `Method::Lz` now that S2-D2 wired the latter. `roundtrip` asserts
  `decompress(compress(x)) == x` for arbitrary `x` — hard rule 1 as an
  executable. `cargo-fuzz`'s libFuzzer engine needs sanitizer-coverage
  compiler passes (`-Zsanitizer`, `-Cpasses=sancov-module`) that are
  nightly-only, so `fuzz/` carries its own `rust-toolchain.toml` pinning
  nightly and its own `[workspace]` (an empty table breaking it out of
  the root workspace, matching upstream `cargo-fuzz` convention) so the
  root's pinned-stable required checks (`fmt`/`clippy`/`test`/`doc`,
  bare `cargo <cmd>`, default-members only) never touch it and never
  need nightly. Smoke-verified locally, not in CI yet: 15s each,
  `decode_arbitrary` (24k executions, no panic) and `roundtrip` (635
  executions, no panic, no round-trip failure). `decode_arbitrary`'s run
  flagged one "slow unit" (12s on a 113-byte input) via libFuzzer's own
  outlier detection, not a crash: the input decodes toward
  `codec::MAX_DECODED_LEN` (256 MiB), the known, already-bounded
  compression-bomb amplification every LZ-family decoder has — bounded
  is not zero-cost, and hard rule 2 asks for bounded, not fast. |
  `cargo fmt --check`/`clippy --all-targets -- --deny warnings` clean on
  the `fuzz/` crate itself (nightly toolchain, not a required check);
  the five root CLAUDE.md gates (`fmt`, `clippy --all-targets`, `test
  --all-targets`, `test --doc`, `doc --no-deps` under
  `RUSTDOCFLAGS=--deny warnings`) all clean, confirming `fuzz/`'s
  separate-workspace isolation holds. | No bpb measurement: this is
  test infrastructure, not an experiment against a champion —
  `progress.jsonl` records this as `kind: "patch"` with null bpb
  deltas. Remaining issue #53 scope: a scheduled smoke run wired into
  `monster.yml`'s cross-OS matrix (#42) rather than the fast PR gate,
  then the OSS-Fuzz application (`blocked-on-human`: needs a contact
  email linked to a Google account).
- S2-A26 | ACCEPTED | Next slice of the remaining S1-D2/S2-D1 scope after
  the seven structured generators (S2-A18..A24): `bench/corpus.toml`
  pins Silesia's 12 files and the Canterbury tarball by URL + SHA-256
  (`research/corpus/POLICY.md`, "Borrowed corpora are never committed
  ... pins each file by URL + SHA-256"), and a new `bench::corpus`
  module fetches, verifies, and disk-caches them by name, refusing a
  checksum mismatch rather than returning wrong bytes. Pins are the
  SHA-256 of the compressed download as served today (bz2 per Silesia
  file, one tar.gz for Canterbury); decompression is not wired — a
  consumer needing raw corpus bytes decompresses at the point of use
  until that slice lands. Gated behind an opt-in `corpus-fetch` Cargo
  feature (`ureq` for HTTPS, `sha2` for the digest — both optional):
  `bench` is a root workspace default-member, so an unconditional
  dependency here would tax every PR's required `cargo test
  --all-targets` with `ureq`'s transitive tree (rustls, `url`/`idna`/ICU
  for URL parsing) though only this one module needs it; CLAUDE.md's
  required checks build default features only, so the gate keeps their
  cost unchanged, same shape as `fuzz/`'s isolation in S2-A25. | The
  manifest parser and the cache/checksum logic are unit-tested without
  network (a fake fetch closure standing in for the real one): cache
  miss fetches and verifies, a checksum mismatch is rejected and never
  cached, a cache hit never calls fetch again, an unknown name errors.
  Real pins verified against the live URLs in a `#[ignore]`d test, run
  manually (`cargo test -p mothergod-bench --features corpus-fetch --
  --ignored`): `xml` (Silesia, smallest file) and `cantrbry` (the whole
  Canterbury tarball) both fetched and matched their pinned SHA-256. All
  five root CLAUDE.md gates clean with default features (the new
  dependencies never enter that build); `clippy --all-targets --features
  corpus-fetch -- --deny warnings` and `cargo doc --features
  corpus-fetch` also clean, confirming the gated module itself is not
  secretly broken just because the fast gate can't see it. | No bpb
  measurement: this is fetch infrastructure, not an experiment against a
  champion — `progress.jsonl` records this as `kind: "patch"` with null
  bpb deltas. Remaining S1-D2/S2-D1 scope: decompression wiring for both
  formats, the three-tier train/sealed/finals split plumbing, regret
  scoring, the CI baseline gate, progress-graph rendering, and a
  scheduled workflow exercising `--features corpus-fetch` so the gated
  module gets real CI coverage instead of only local verification (same
  gap S2-A25 left for `fuzz/`).
- S2-A27 | ACCEPTED | Issue #219: characterized the decode-time vs.
  declared-length relationship the reviewer's S2-A25 fuzz run first
  surfaced as a 12s slow-unit, later found by the same fuzzer at 151.8s
  on a 16-byte input — "an order of magnitude past reported, amplification
  factor under-sampled" was the concern. Mechanism confirmed, not new:
  `codec::MAX_DECODED_LEN`'s worst case is an all-literal decode, since
  every literal byte pays `literal::Literal::decode`'s full six-expert
  mix over the 256-symbol alphabet (rebuilds all 256 cumulative entries
  from scratch every call), while a match/rep byte is a single unmodeled
  array copy — literal decode is the expensive branch, not (as the
  pre-existing `MAX_DECODED_LEN` doc comment mislabeled it) the cheap
  one. | Measured directly (release build, hand-crafted payload:
  `Candidate::Identity` header, an empty coded stream so every flag/byte
  decodes to its zero symbol, `token_count` far past `declared_len` so
  `ensure_room` is what stops the loop): 1/4/16/64/256 MiB declared
  lengths decoded in 1.23s/4.91s/19.63s/78.53s/313.95s, a steady ~1170
  ns/byte at every size (no growth in the per-byte constant) — linear,
  not polynomial or worse. This matches S2-A17's own extrapolation
  (2.35s at 2,000,000 bytes ≈ 1175 ns/byte) almost exactly, so nothing
  changed between then and now except sample size: 12s and 151.8s are
  both ordinary points on the same line (≈10 MiB and ≈130 MiB
  respectively), not evidence of unbounded blowup. | No time/work budget
  added: the existing size ceiling already bounds this linearly, and a
  separate time budget would need to be at least as generous as the
  legitimate-decode case it must not reject, which collapses to the same
  bound. Two things this changed: (1) `MAX_DECODED_LEN`'s doc comment
  now states the measured ~314s/256 MiB ceiling and ~1170 ns/byte
  constant instead of a guessed "low single-digit minutes", and
  correctly labels the literal branch as the expensive one, not the
  cheap one (`rust-craft`/Truth value — verify a claim against a run,
  fix the sentence once checked). (2) the ~1170 ns/byte constant is now
  a concrete number against S1-P6 (speed-tier lead): decoding an
  all-literal 256 MiB stream at that rate is ~854 KB/s, under the
  ROADMAP SPEED floor (≥1 MB/s decode) on a realistic literal-heavy
  input, not just an adversarial one — `Literal::mix`'s O(256×6)
  from-scratch rebuild per byte is the mechanism, real fix (incremental
  cumulative frequencies) is M5 scope, not this issue's.
- S2-A28 | ACCEPTED | Next slice of the remaining S1-D2/S2-D1 scope after
  S2-A26's fetch-and-cache: decompression. `bench::corpus::decompress_silesia`
  unwraps one Silesia file's bzip2 stream (`bzip2-rs`, decode-only, pure
  Rust, no system libbzip2 to discover in CI); `bench::corpus::extract_canterbury`
  lists every file inside Canterbury's gzip-compressed tarball as
  `(path, bytes)` pairs (`flate2`, default `rust_backend`/`miniz_oxide`
  so no system zlib either, plus `tar` with its default `xattr` feature
  turned off since nothing here extracts to disk). All three are new
  optional dependencies folded into the existing `corpus-fetch` feature
  gate (S2-A26's Cargo-doc-comment reasoning: `bench` is a root
  workspace default-member, so the tree stays out of every PR's required
  `cargo test --all-targets` unless the feature is on). | `bzip2-rs` has
  no encoder, so its round-trip fixture is generated externally
  (`bz2.compress` in Python, embedded as a byte literal) rather than
  self-produced; `flate2`/`tar` write as well as read, so the Canterbury
  tests build their own gzip+tar fixtures in-crate, including a
  30-file/20,000-byte-each archive to exercise multi-block gzip/tar
  streams, not just a single small one. Both functions also get a
  malformed-input test (non-bzip2 bytes, non-gzip bytes) returning
  `Err`, never a panic. All five root CLAUDE.md gates clean with default
  features (the new dependencies never enter that build, same as
  S2-A26); `clippy --all-targets --features corpus-fetch -- --deny
  warnings` and `cargo doc --features corpus-fetch` also clean. | No bpb
  measurement: decompression infrastructure, not an experiment against a
  champion — `progress.jsonl` records this as `kind: "patch"` with null
  bpb deltas. Remaining S1-D2/S2-D1 scope: the train/sealed/finals split
  plumbing, regret scoring, the CI baseline gate, progress-graph
  rendering, and a scheduled workflow exercising `--features
  corpus-fetch` for real CI coverage (same gap S2-A25 and S2-A26 left).
- S2-A29 | ACCEPTED | First slice of S1-D2/S2-D1's train/sealed/finals
  split plumbing: `bench::train_window(data, window_len, iteration)`
  (`research/corpus/POLICY.md`, "Train slices — rotating windows over
  each dataset; a different window every iteration so offsets can't be
  memorized"). Applies only to the in-repo generators (entropy ladder,
  markov trap, structured classes) — Silesia/Canterbury are held-out
  finals, run whole-file at milestones, never inside the experiment loop
  (POLICY.md "Held-out finals"), so they don't participate in this split.
  The window wraps circularly around `data`: `start = iteration mod
  data.len()`, and the return value is `data[start..]` followed by
  `data[..window_len - (data.len() - start)]` when the window would run
  past the end. Circular wraparound, not clamping, so a window as long as
  `data` itself still differs every iteration (a one-byte left rotation
  per iteration) instead of freezing at iteration 0 — the policy's
  "a different window every iteration" holds even in that degenerate
  case. | 9 unit tests: exact-length output across a matrix of window
  lengths (including the full buffer) and iteration counts up to
  `u64::MAX`, iteration 0 starts at the front, a non-wrapping slide,
  wraparound at the exact boundary, one full `data.len()`-cycle repeats
  the same window, consecutive iterations differ, the whole-buffer case
  rotates rather than staying fixed, and the two invalid-input panics
  (zero-length window, window longer than `data`); all five root
  CLAUDE.md gates clean. | No bpb measurement: split plumbing, not an
  experiment against a champion — `progress.jsonl` records this as
  `kind: "patch"` with null bpb deltas. Remaining S1-D2/S2-D1 scope: the
  sealed validation set's held-out seed and dataset-kind separation
  (this slice only rotates train windows; nothing yet designates which
  seeds/generators are sealed-only), regret scoring, the CI baseline
  gate, progress-graph rendering, and the scheduled `corpus-fetch`
  workflow.
- S2-A30 | ACCEPTED | ROADMAP M2's ideal-cost accounting mode (ADR-0006),
  first slice: `Model::ideal_cost_bits` sums `-log2(freq[symbol] /
  total)` against the same adaptive state `Model::encode` drives, then
  applies the same `update` call, without touching an `Encoder` at all —
  the Rust-native replacement ADR-0006 calls for the archive's Python
  model-cost proxy, so an experiment loop can price a distribution
  without paying for real arithmetic coding. Unlike `Literal::update`'s
  `exp` (ADR-0024, S2-D3), this never sits on the coding path — no
  bitstream depends on it, encoder or decoder — so it uses `f64::log2`
  directly behind a justified `clippy::disallowed_methods` allow instead
  of a vendored implementation; ADR-0024's cross-platform-determinism
  requirement binds what an encoder and decoder must agree on, and this
  method is neither. | 4 unit tests: a fresh table's uniform-distribution
  cost is exactly 2 bits over a 4-symbol alphabet; cost strictly
  decreases as a symbol gets coded repeatedly; a model driven through
  `ideal_cost_bits` alone ends in the same state as one driven through
  `encode` alone (checked by coding one more symbol on each and
  comparing cost); summed ideal cost over 5,000 pseudo-random symbols
  (32-wide alphabet) tracks the real `Encoder`'s bit-exact output within
  1% (Xorshift32, seed `0x12345678`), the same tolerance shape as
  `literal.rs`'s vendored-`exp` accuracy check. All five root CLAUDE.md
  gates clean. | No bpb measurement: this is the accounting tool an
  experiment would use, not itself an experiment against a champion —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas.
  Remaining scope: `Literal`'s six-expert mixer needs the same
  accounting method (larger slice, mirrors S2-A12's relationship to
  S2-A11), and nothing yet sums a whole-codec ideal-cost pass across
  `lz`'s flag/length/offset streams and `literal`'s bytes together.
- S2-A31 | ACCEPTED | ROADMAP M2's ideal-cost accounting mode (ADR-0006),
  second slice: `Literal::ideal_cost_bits`, S2-A30's flagged
  counterpart for the six-expert mixer. Reuses `Literal::mix` to build
  the same mixed cumulative-frequency table `encode` codes against,
  prices the requested byte as `-log2((cum[symbol+1] - cum[symbol]) /
  cum[ALPHABET])`, then calls the same `update` (with the vendored
  `exp`, ADR-0024) `encode` does, so the model ends in the state a real
  encode pass would have left it in. No `Encoder` touched, same
  determinism argument as S2-A30: this never sits on the coding path,
  so the `f64::log2` call needs only a justified
  `clippy::disallowed_methods` allow, not a vendored implementation. |
  3 unit tests: cost strictly decreases as a byte gets coded repeatedly
  under a stabilized context; a model driven through `ideal_cost_bits`
  alone ends in the same state as one driven through `encode` alone
  (checked by coding one more byte on each and comparing cost, and by
  comparing the two paths' `Context` values directly); summed ideal
  cost over the archived codec source (`research/imports/session-1/
  mothergod.rs`, 25,524 bytes, the same fixture `literal.rs`'s
  vendored-`exp` accuracy test uses) tracks the real `Encoder`'s
  bit-exact output within 1%. All five root CLAUDE.md gates clean. |
  No bpb measurement, same reason as S2-A30: this is the accounting
  tool an experiment would use, not itself an experiment against a
  champion — `progress.jsonl` records this as `kind: "patch"` with
  null bpb deltas. Remaining S2-D1 scope: nothing yet sums a
  whole-codec ideal-cost pass across `lz`'s flag/length/offset streams
  and `literal`'s bytes together.
- S2-A32 | ACCEPTED | Seed half of S2-D1's remaining sealed-validation
  split (`research/corpus/POLICY.md`, "Sealed validation set — different
  seed AND different datasets from train"): `bench::sealed_seed(train_seed)`
  derives a sealed-validation seed from a train seed by feeding it (XORed
  with a fixed key) through the same `SplitMix64` step `bench::Rng` uses.
  That step is a bijection on `u64`, so distinct train seeds always derive
  distinct sealed seeds — but it does not prove a sealed seed can never
  coincide with some unrelated seed picked directly for train; a provable
  split would need a structural reservation (e.g. a fixed high bit), which
  conflicts with seeds already in use elsewhere in this crate that set
  every bit pattern (S2-A1's `0xC0FF_EE12_3456_789A`). Same caveat as `Rng`
  itself: reproducible and distinct from its own input, not
  cryptographically unpredictable — the property this needs. | 4 unit
  tests: determinism, `sealed_seed(seed) != seed` across a spot-checked
  set including `u64::MAX`, injectivity swept over 10,000 consecutive
  seeds, and no collision between that same sealed range and the plain
  train seeds `0..10_000`. All five root CLAUDE.md gates clean. | No bpb
  measurement: split-plumbing infra, not an experiment against a champion
  — `progress.jsonl` records this as `kind: "patch"` with null bpb
  deltas. Remaining S2-D1 scope: which dataset kinds are sealed-only
  (never appearing in train) is still undesignated, plus regret scoring,
  the CI baseline gate, progress-graph rendering, and the scheduled
  `corpus-fetch` workflow (issue #231).
- S2-A33 | ACCEPTED | Dataset-kind half of S2-D1's remaining
  sealed-validation split (`research/corpus/POLICY.md`, "held-out seeds
  AND held-out dataset kinds"): `bench::DatasetKind` enumerates the nine
  generator kinds and `DatasetKind::sealed_only` designates two of the
  seven structured classes, `AccessLog` and `GradientImage`, as
  sealed-only, train-slice code must never request them. Rationale for
  the split: `EntropyLadder` and `MarkovH82Trap` are POLICY's mandatory
  datasets, checking the coder against the theoretical floor and the
  histogram-coder trap on every train iteration rather than
  generalization, so both stay in train regardless. Of the remaining
  five, `InterleavedAudio16`, `X86DenseCode`, `Base64Wrapped`, and
  `SqliteLikeRecords` each have a filter in `src/filters.rs` whose
  documented purpose matches their shape (delta, BCJ, base64-unwrap,
  transpose respectively) that train slices actively exercise; an
  earlier draft of this entry held `SqliteLikeRecords` sealed-only on
  the claim that no filter targets fixed-width binary records, which
  review (PR #238) disproved by running `transpose::encode` against the
  generator's actual 20-byte rows: -0.94 bits/byte order-1 entropy,
  matching the win `transpose`'s own doc comment cites as its
  justification (S1-A2). `AccessLog` and `GradientImage` have no filter
  whose documented purpose matches their shape; scanning every delta
  stride (1..=96) and transpose column count (2..=96) against each
  finds nothing past 0.15 bits/byte, noise next to the sqlite case — so
  holding these two sealed-only measures whether the parse/model/coder
  stages generalize on their own instead of re-testing a filter tuned
  for exactly this data. No structural mechanism enforces the split yet —
  the enum exists for the CI baseline gate and regret scoring (both
  still S2-D1 debt) to consult once they exist; nothing calls
  `DatasetKind` today. | 3 unit tests: `DatasetKind::ALL` has no
  duplicates, the two mandatory kinds are never sealed-only, and the
  sealed-only set is a nonempty proper subset of `ALL` (catches both "no
  kind is held out" and "every kind is held out, leaving nothing to
  train against"). All five root CLAUDE.md gates clean. | No bpb
  measurement, same reason as S2-A32: split-plumbing infra, not an
  experiment against a champion — `progress.jsonl` records this as
  `kind: "patch"` with null bpb deltas. Remaining S2-D1 scope: regret
  scoring, the CI baseline gate, progress-graph rendering, and the
  scheduled `corpus-fetch` workflow (issue #231).
- S2-A34 | ACCEPTED | Regret scoring, next slice of the remaining S2-D1
  debt after S2-A33's `DatasetKind` (`research/corpus/POLICY.md`,
  "Growing the corpus"): `bench::regret(ours_bpb, zstd_bpb, xz_bpb)`
  scores a candidate corpus addition as our bits/byte minus the
  stronger (lower) of the two pinned reference compressors' bits/byte on
  the same data — POLICY.md names `zstd -19` and `xz -9e` as the two
  references, and scoring against whichever does better keeps a data
  class from counting as "we're relatively bad at this" when we only
  lose to the weaker one. Positive regret is the accept criterion.
  POLICY.md also auto-rejects pure noise as a named special case ("has
  zero regret, and is auto-rejected"); this needed no separate branch,
  because noise is equally incompressible for every compressor, so all
  three inputs sit near 8 bits/byte and the subtraction already lands
  near zero, failing the positive-regret test on its own. | 5 unit
  tests: zero regret when ours matches the stronger reference, positive
  when ours loses to both, negative when ours beats both, symmetric
  under swapping which reference argument is stronger, and near-zero on
  a synthetic pure-noise triple (all three inputs ~8 bits/byte). All
  five root CLAUDE.md gates clean. | No bpb measurement, same reason as
  S2-A33: scoring infra, not an experiment against a champion —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas.
  Not yet called by anything — it exists for the CI baseline gate (still
  S2-D1 debt) to consult once it exists. Remaining S2-D1 scope: the CI
  baseline gate, progress-graph rendering, and the scheduled
  `corpus-fetch` workflow (issue #231).
- S2-A35 | ACCEPTED | The CI baseline gate's measurement half, next slice
  of the remaining S2-D1 debt after S2-A34's `regret`: a new
  `bench::baseline` module measures mothergod's real bits/byte (via
  `mothergod::compress`, not the ideal-cost accounting mode) on eleven
  fixed-seed (`CASE_SEED`), fixed-length (`CASE_LEN` = 50,000 bytes)
  cases — one per entropy-ladder target (`research/corpus/POLICY.md`'s
  five mandatory points) plus one per non-ladder `DatasetKind::ALL`
  entry — and compares against a committed `bench/baseline.json`,
  flagging any case whose bits/byte grew past `TOLERANCE_BITS` (0.02)
  since the last commit. `DatasetKind::sealed_only` kinds (`AccessLog`,
  `GradientImage`, S2-A33) are excluded by construction: a PR-time gate
  an agent reacts to and fixes is a tuning loop, and running it against
  the sealed set would smuggle sealed data into that loop through the
  back door (`research/corpus/POLICY.md`, "no agent ever tunes against
  it"). `bench/baseline.json` and `parse_baseline` are a hand-rolled
  flat JSON object, not a general reader — same deliberate scope limit
  `bench/corpus.toml`'s manual TOML reader takes, since this format only
  ever needs to round-trip what `format_baseline` itself writes. A new
  `baseline_gate` binary (`cargo run -p mothergod-bench --release --bin
  baseline_gate -- check`, or `-- write` to update the committed numbers
  after an accepted ratio change) is ready to wire into CI as a new
  non-required job (same shape as the existing `worker`/`adr-numbers`
  jobs, so the ruleset's four required check names stay untouched), but
  the wiring itself is left for whoever holds `GH_ADMIN_TOKEN`: it is a
  `.github/workflows/` push, and `agents/GOVERNANCE.md`'s "Push identity"
  reserves that credential for what the app token cannot write, not
  available to the session that measured this. That may be the reason
  this bullet sat as "remaining S2-D1 scope" across S2-A32 through
  S2-A34 despite everything it depends on being ready since S2-A33. |
  15 unit tests: case coverage (every train-eligible kind and the full
  ladder present, sealed-only kinds absent), determinism, exact case
  length, `bits_per_byte` on a known ratio and on empty input,
  format/parse round-trip including sort order and optional trailing
  comma, a malformed-line parse error naming its line number, and
  `regressions` on matching/within-tolerance/past-tolerance/improved/
  missing-on-either-side inputs. All five root CLAUDE.md gates clean;
  measured once on today's codec to seed `bench/baseline.json` (e.g.
  `entropy_ladder_h1` 1.298080 bits/byte against a 1.0-bit target,
  `markov_h8_2_trap` 2.447040 against a 2.0-bit floor — the real coder's
  gap above each dataset's theoretical floor, not itself a claim this
  entry investigates). Remaining S2-D1 scope: the CI wiring itself (needs
  `GH_ADMIN_TOKEN`), progress-graph rendering, and the scheduled
  `corpus-fetch` workflow (issue #231).
- S2-A36 | ACCEPTED | Progress-graph rendering, next slice of the
  remaining S2-D1 debt after S2-A35's `baseline` module (ROADMAP M2:
  "per-dataset graphs rendered ... into `docs/benchmarks/`"): a new
  `bench::graph` module renders `bench/baseline.json` as a hand-rolled
  static SVG bar chart plus a markdown table, no charting dependency (11
  bars, well inside hand-rolled SVG). 11 cases is past the dataviz
  convention's ~7-class chart-alone ceiling, so both forms ship together
  (`docs/benchmarks/baseline.svg` + `baseline.md`), generated in one pass
  so they can't drift apart. A new `render_baseline_graph` binary
  (`cargo run -p mothergod-bench --release --bin render_baseline_graph`)
  writes both from the committed baseline; not yet on a schedule for the
  same reason `baseline_gate` isn't wired into CI (S2-A35): a
  `.github/workflows/` push needs `GH_ADMIN_TOKEN`
  (`agents/GOVERNANCE.md`, "Push identity"), not available to this
  session. `docs/benchmarks/README.md` states plainly what's still
  missing (no gzip/zstd/xz column, no Silesia/Canterbury numbers) rather
  than letting the chart imply more than `bench/baseline.json` measures.
  | 9 unit tests on `bench::graph`: SVG well-formedness (one `<path>` per
  bar), ascending sort order, HTML-entity escaping of title/subtitle
  (guards against a case name or the "as of" stamp breaking the markup),
  empty-input rendering, corner-radius clamping for a bar narrower than
  the radius (an early draft's radius/width-halving guard, caught before
  commit by manual geometry review, not by a failing test — the fix
  landed with a regression test alongside it), a regression test pinning
  bar width and the top gridline to the same rounded axis ceiling (an
  early draft scaled bars by the raw data max but gridlines by its
  ceiling, so the top gridline overshot the plot's right edge into the
  value-label column — caught by rendering the SVG and checking
  coordinate bounds with a script before committing, not by CI), and the
  markdown table's header/row/pipe-escaping shape. All five root
  CLAUDE.md gates clean. | No bpb measurement: this renders an existing
  measurement, it doesn't take one — `progress.jsonl` records this as
  `kind: "patch"` with null bpb deltas, same as S2-A29 through S2-A35.
  Remaining S2-D1 scope: the CI baseline gate and the scheduled
  `corpus-fetch` workflow (issue #231), both `.github/workflows/` pushes
  reserved for `GH_ADMIN_TOKEN`; a gzip/zstd/xz reference column and real
  Silesia/Canterbury numbers, both new scope this entry surfaced rather
  than closed.
- S2-A37 | ACCEPTED | The gzip/zstd/xz reference column and real
  held-out-final numbers S2-A36 surfaced as new scope, one corpus at a
  time (ROADMAP M2, ROADMAP Scorecard's RATIO metric): a new
  `bench::reference` module (behind `corpus-fetch`, same gate as
  `bench::corpus`) shells `gzip -9`/`zstd -19`/`xz -9e` on a temp-file
  argument rather than piping stdin, sidestepping the pipe deadlock a
  multi-megabyte write risks under `Command::output()`'s stdin-less
  capture. A new `bench::finals` module (deliberately *not*
  `corpus-fetch`-gated, so it builds under the default features CLAUDE.md's
  required checks run) formats a report: per-file bits/byte for
  `mothergod::compress` and all three references plus one aggregate row,
  computed as total-compressed-over-total-original bytes rather than an
  average of per-file ratios (the latter over-weights small files against
  their real share of the corpus). Reuses `baseline::bits_per_byte` and
  `regret` rather than a second copy of either, and `graph`'s markdown
  pipe-escaping (promoted to `pub(crate)` for the reuse) rather than a
  third. A new `finals_report` binary (`cargo run -p mothergod-bench
  --release --features corpus-fetch --bin finals_report`) fetches
  Canterbury (the `cantrbry` manifest entry), extracts its 11 files,
  measures all four compressors, and writes
  `docs/benchmarks/canterbury.md`. Silesia is not run: measured
  throughput on this codec's optimal-parse LZ is ~0.14 MB/s (`xml`, the
  smallest Silesia file, 5.3 MB in 39s), so the full ~200 MB corpus would
  cost on the order of half an hour of `mothergod::compress` time alone —
  the binary's own module doc names this as the reason, not a missing
  feature; Silesia numbers most naturally land behind the scheduled
  `corpus-fetch` workflow (issue #231) once it exists, not a slow
  by-hand run. **First real result**: on Canterbury, mothergod's
  aggregate is 1.380218 bits/byte against zstd -19's 1.469771 and
  xz -9e's 1.403395 (gzip -9: 2.080544) — mothergod beats the stronger
  reference by regret −0.023176 on this corpus, ROADMAP's RATIO ladder
  rung (2) ("win or tie every file vs zstd -19") not yet true per-file
  (`lcet10.txt` +0.054, `plrabn12.txt` +0.082, `sum` +0.323 against zstd,
  full table in `docs/benchmarks/canterbury.md`) but true in aggregate on
  this one corpus. `docs/benchmarks/baseline.md`'s own generated header
  corrected to point at `canterbury.md` instead of claiming no
  gzip/zstd/xz comparison exists anywhere in the crate. | 15 new unit
  tests across `reference` (gzip/zstd/xz each shrink a repetitive
  fixture, an unknown command errors, empty input doesn't error,
  `--version` returns a nonempty line for all three, an unknown command's
  version errors) and `finals` (files sort by name in the rendered
  report, corpus name/versions are named, the aggregate row is the
  byte-weighted number and not the naive average of per-file ratios, the
  aggregate row names the file count, zero measurements render a `0/0`
  aggregate without dividing by zero, a `|` in a file name is escaped).
  All five root CLAUDE.md gates clean on default features;
  `cargo clippy -p mothergod-bench --all-targets --features corpus-fetch
  -- --deny warnings` and `cargo test -p mothergod-bench --features
  corpus-fetch` (110 passed, 1 ignored network smoke test) both clean,
  run by hand since CI's required checks build default features only
  (same S1-D2 gap issue #231 will close). | Real bpb: this is the first
  entry with an actual held-out-final measurement rather than a null
  patch delta, but it isn't a `train_delta_bpb`/`val_delta_bpb` in the
  schema's sense either — there is no champion-vs-candidate comparison,
  only a new measurement capability's first output — so `progress.jsonl`
  still records `kind: "patch"` with null deltas, same as every S2-D1
  infra entry, and puts the real numbers in its `mechanism` field
  instead. Remaining S2-D1 scope: the CI baseline gate's
  `.github/workflows/` wiring and the scheduled `corpus-fetch` workflow
  (issue #231), both needing `GH_ADMIN_TOKEN`; real Silesia numbers,
  blocked on throughput (S1-P6's speed-tier lead) rather than on missing
  code.
- S2-A38 | ACCEPTED | ROADMAP M2's ideal-cost accounting mode (ADR-0006),
  closing slice: `codec::ideal_cost_bits`, the whole-codec pass S2-A30 and
  S2-A31 each flagged as remaining scope after building
  `Model::ideal_cost_bits` and `Literal::ideal_cost_bits`. Runs
  `lz::parse_optimal` over already-filtered data, then walks the same
  flag/length/offset/slot/literal sequence `codec::encode_tokens` would
  encode, pricing each through the two per-model methods instead of
  driving an `Encoder`. A new private `ideal_cost_bucketed` mirrors
  `encode_bucketed`'s split: `Model::ideal_cost_bits` prices the bucket
  symbol, and the residual low bits (raw, unmodeled — `Encoder::encode_bits`
  emits them literally) cost exactly their own count, added as a plain
  `f64`. Operates at `encode_tokens`'s layer (one already-chosen filter's
  output), not `encode`'s filter-trial loop above it — an experiment
  pricing a candidate doesn't need to pay for trialing every filter
  candidate too. | 3 unit tests: empty input costs zero; summed ideal cost
  over the archived codec source (`research/imports/session-1/
  mothergod.rs`, 25,524 bytes, the same fixture every other ideal-cost
  accuracy test in this crate uses) tracks `encode_tokens`'s real
  `Encoder` output (past its 8-byte header, which no `ideal_cost_bits`
  call ever prices) within 1%; a 50x repeat of an 8-byte pattern costs
  under half the bits/byte of same-length pseudo-random data, confirming
  the pass actually reflects the LZ/model pipeline's own sense of
  compressibility rather than merely tracking one fixture's real length.
  All five root CLAUDE.md gates clean. | No bpb measurement: this is the
  accounting tool ROADMAP M2 calls for, not itself an experiment against a
  champion — `progress.jsonl` records this as `kind: "patch"` with null
  bpb deltas, closing the pattern S2-A30 through S2-A37 share. Remaining
  S2-D1 scope: the CI baseline gate's `.github/workflows/` wiring and the
  scheduled `corpus-fetch` workflow (issue #231), both needing
  `GH_ADMIN_TOKEN`; real Silesia numbers, blocked on throughput
  (S1-P6) rather than on missing code.
- S2-A39 | ACCEPTED | ROADMAP M4's "golden frames per `FORMAT_VERSION`"
  and `docs/TESTING.md` layer 5, first slice: `tests/golden/` pins one
  real `FORMAT_VERSION` 2 `Method::Lz` frame (`v2-lz-repeated-text.mgdc`,
  62 bytes, the archive of `compress()` on a 2,250-byte repeated-text
  fixture) plus its plaintext, and `tests/golden.rs` asserts
  `decompress(golden) == plaintext` for every pair and, for the current
  `FORMAT_VERSION` only, `compress(plaintext) == golden`. Two claims of
  different strength, not one: the decode half is a real cross-platform
  guarantee (decode is integer-only end to end, S1-A5), the re-encode
  half only pins this runner's toolchain, because `lz.rs` pricing and
  `filters.rs` filter scoring keep `f64::log2` as encoder-only floats
  (ADR-0024 decision 3) that libm does not promise bit-identical across
  targets. `docs/TESTING.md` layer 5 previously asserted "byte-identical
  bitstream on every platform" as a blanket claim with no test behind
  it; corrected to name which half is proven and which remains a plan,
  per the Truth value (a stale sentence read confidently is how this
  project produces wrong statements). Every future `FORMAT_VERSION`
  bump adds a new pair and keeps every old one, so hard rule 5's
  "decode support for all previous versions" is a running test rather
  than a doc-comment claim, same shape as `old_version_lz_frame_is_
  rejected_not_misparsed` already gave the version/method combination
  that has no golden payload of its own. | 1 test, iterating every
  `<name>.mgdc`/`<name>.plaintext` pair in `tests/golden/` (mirrors
  `tests/adversarial.rs`'s seed-corpus-iteration shape): the fixture's
  file name and its frame's version byte must agree, decode must match
  the pinned plaintext, and (current version only) re-encoding the
  plaintext must reproduce the pinned frame byte-for-byte. All five
  root CLAUDE.md gates clean; the new test runs inside the existing
  `test` required check, no `.github/workflows/` change needed. | No
  bpb measurement: this is `docs/TESTING.md` layer 5 infrastructure,
  not a ratio experiment against a champion — `progress.jsonl` records
  this as `kind: "patch"` with null bpb deltas. Remaining layer-5 scope:
  the multi-platform CI matrix that would prove the encoder claim too,
  a `.github/workflows/` push reserved for whoever holds
  `GH_ADMIN_TOKEN`, same constraint as S2-D1's remaining CI wiring
  (issue #231).
- S2-A40 | ACCEPTED | First slice of ROADMAP M3's top standing lead
  (S1-P1, SSE): a standalone secondary symbol estimation (SSE/APM)
  primitive, `src/sse.rs` (`Sse`). Not a port — S1-P1 is a literature lead
  the founding session never implemented (`research/imports/session-1/`
  greps clean of any SSE/APM code), so there is no archive behavior to
  carry forward, unlike every other module in this crate (ADR-0006).
  Classic PAQ/APM design (Mahoney 2005): a small side context plus a
  primary model's probability estimate index into an adaptive table that
  has learned to correct that context's systematic bias, read by linear
  interpolation between two neighboring bins, written by nudging both
  toward the observed outcome. One deliberate design deviation: PAQ warps
  its bin spacing through a logit transform (`stretch`/`squash`,
  `ln`/`exp`) to concentrate resolution near 0 and 1; this crate's
  `clippy.toml` forbids every libm transcendental crate-wide (ADR-0024),
  since a probability the encoder computes and the decoder must reproduce
  bit-for-bit cannot depend on a function libm implementations disagree
  on in the last ulp — the exact problem `literal.rs`'s vendored `exp`
  (S2-A16) already solved for the mixing-weight update. Rather than
  vendor a second transcendental pair, this module uses linear-domain
  bins instead: coarser resolution near the extremes than a production
  APM would want, but built from `+ - * /` and `f64::clamp` only, so no
  new transcendental surface is needed. Output is clamped to
  `[1/4096, 1 - 1/4096]`: an adaptive table fed by finite, noisy evidence
  should never claim an outcome is impossible, the same reasoning
  `model::Model` already applies by starting every frequency at 1. |
  10 unit tests: a fresh table is near-identity (untrained bins return
  approximately their input probability); output stays strictly inside
  `(0.0, 1.0)` even after 10,000 updates all pushing one direction;
  calibration converges within 0.03 of a synthetic context's true 90%
  observed rate when the primary estimate is a constant, uninformative
  0.5 (the systematic-bias correction S1-P1 is for); two contexts adapt
  independently; `refine` is monotonic in its input probability on a
  fresh table; out-of-range input probabilities clamp rather than panic;
  out-of-range context indices panic (mirrors `model::Model::encode`'s
  documented bound); `contexts()` reports the constructed count. All five
  root CLAUDE.md gates clean (two intra-doc-link warnings against private
  items, `BINS` and `Self::position`, fixed by dropping the doc links,
  same class as S2-A8/S2-A10/S2-A11/S2-A12/S2-A16). | No bpb measurement:
  nothing in this crate has a binary probability stream to calibrate yet
  — the flag stream `codec.rs` codes is three-ary (literal/match/rep),
  and `literal::Literal` codes a 256-ary symbol directly rather than a
  sequence of binary decisions, so wiring `Sse` against either needs a
  decomposition this slice does not build. `progress.jsonl` records this
  as `kind: "patch"` with null bpb deltas, per `research/README.md`'s
  capability-patch rule, same reason as every unwired M1 filter/LZ slice
  (S2-A2 through S2-A12). Remaining S1-P1 scope: decompose one binary
  sub-decision to calibrate — the flag model's "is this a copy, not a
  literal" split is the smallest candidate, since it is already the
  coarsest three-way choice in the pipeline — wire `Sse` behind it, bump
  `FORMAT_VERSION`, and measure a real bpb delta on the corpus policy's
  train/sealed split against the five zstd text holdouts S1-P1 names.
- S2-A41 | ACCEPTED | Second slice of ROADMAP M3's top standing lead
  (S1-P1, SSE): the other prerequisite S2-A40 left outstanding — a
  probability-driven bit-coding primitive, since `coder.rs` previously only
  drove the range coder from a [`Model`] frequency table
  (`Encoder::encode`/`Decoder::decode`) or a fixed, unmodeled 50/50 split
  (`encode_bits`/`decode_bits`), neither of which an `Sse`-calibrated
  probability can feed. Added `Encoder::encode_bit`/`Decoder::decode_bit`:
  code one bit at an arbitrary caller-supplied `probability_of_one`,
  quantized into a `BIT_SCALE`-wide (2^16) integer threshold by a shared
  `quantize_probability` (`+ - * /`, `f64::clamp`, and rounding only, no
  libm transcendental, so encoder and decoder compute the identical
  threshold bit-for-bit — ADR-0024's determinism rule, the same reasoning
  that led `Sse` to linear-domain bins). Threshold clamped to
  `1..=BIT_SCALE - 1` so neither outcome is ever assigned zero width,
  mirroring `Model::new`'s "nothing is ever impossible to code" guarantee.
  Proved the two primitives compose: a new `sse.rs` integration test drives
  `Encoder::encode_bit`/`Decoder::decode_bit` from `Sse::refine`'s output
  (constant, uninformative 0.5 "primary" estimate, same shape as
  S2-A40's `converges_toward_the_true_observed_rate` test) over a
  2,000-outcome, 90%-skewed synthetic sequence: round-trips exactly and
  costs well under two-thirds the bytes of the same sequence coded at a
  fixed 50/50 split. Caught and fixed one bug before landing: the first
  cut assigned `probability_of_one`'s interval to the *wrong* bit (the
  likely outcome got the narrow range), which a skewed-input cost test in
  `coder.rs` caught immediately — it demanded fewer bits than a fixed
  split and instead measured 6.5x more. | 5 new `coder.rs` unit tests
  (round trip across a probability range including near-0/near-1; the
  unlikely outcome specifically, not just the likely one; out-of-range
  `probability_of_one` clamps rather than panics; a 99%-skewed sequence
  costs under a quarter of the fixed-50/50 byte count) plus the `sse.rs`
  integration test above. All five root CLAUDE.md gates clean (one
  private-intra-doc-link warning, `encode_bit`'s doc linking to the
  private `quantize_probability`, fixed by dropping the link, same class
  as S2-A8/S2-A10/S2-A11/S2-A12/S2-A16/S2-A40). | No bpb measurement:
  still not wired into `codec.rs`'s bitstream — this closes the
  primitive-availability gap, not the wiring decision. `progress.jsonl`
  records this as `kind: "patch"` with null bpb deltas, same reason as
  S2-A40. Remaining S1-P1 scope unchanged from S2-A40's text: decompose
  the flag model's literal/copy split, wire `Sse` and `encode_bit`/
  `decode_bit` behind it, bump `FORMAT_VERSION`, and measure a real bpb
  delta on the corpus policy's train/sealed split against the five zstd
  text holdouts.
- S2-A42 | ACCEPTED | First slice of ROADMAP M3's second standing lead
  (S1-P2, btultra2-class parse): a binary-tree match finder,
  `lz::BinaryTreeMatchFinder`, standalone and not yet wired into
  `parse_greedy` or `parse_optimal` — the same standalone-primitive-first
  order S1-P1 used (S2-A40/S2-A41). Unlike `MatchFinder`'s hash chain,
  which walks candidates newest-first and gives up after a fixed
  `max_tries`, insertion here keeps each hash bucket as a binary search
  tree ordered by the candidate's suffix bytes (LZMA's bt4 shape, ported
  as behavior not code per ADR-0006 — no bt4 source exists anywhere in
  this crate or the archive; built from the published algorithm
  description instead): one downward walk both inserts the new position
  and returns the longest match among the nodes on the insertion path, so
  with `max_depth` at least the bucket's tree height the match found is
  length-exact (proven equal to a brute-force scan of the same hash
  bucket). A shallower `max_depth` is *not* the same trade `MatchFinder`
  makes via `max_tries`: `MatchFinder::find_best` is read-only, so a low
  `max_tries` bounds only that one call, while `insert_and_find` mutates
  the tree on every call — cutting a walk short permanently unlinks the
  unvisited candidates from the bucket, so one shallow call degrades
  every later, even full-depth, query into that bucket, and repeated
  shallow calls compound the loss (caught by post-merge review stress
  testing, not by the 6 shipped tests below; fixed in the doc comments,
  not the algorithm — the pruning matches real bt4's `cutValue`
  mechanic per ADR-0006, so it is correct behavior, just previously
  undersold). A wiring slice must treat `max_depth` as a constant
  per-pass setting, never a value varied call-to-call for speed.
  Deliberately does not carry two things a wired-in successor needs:
  eviction of positions older than `WINDOW` (the tree only grows) and the
  `len0`/`len1` prefix-reuse optimization real bt4 finders use to avoid
  re-comparing already-matched bytes (`suffix_common_len` always compares
  from scratch) — both are speed/memory work, not correctness work, and
  wait for the wiring slice. | 6 unit tests: no match before any position
  is inserted; an exact repeat's reported length and distance checked
  directly; a brute-force cross-check across 400 bytes of low-entropy
  pseudo-random data (Xorshift32 mod 5, so hash buckets build real tree
  structure) proving every returned length matches a full same-bucket
  scan when `max_depth` cannot truncate the walk (the first draft of this
  test compared against *every* earlier position rather than only those
  sharing `i`'s hash bucket, and failed on a length-1 match neither
  finder can ever see by construction — corrected before landing, not a
  finder bug); zero `max_depth` finds nothing but leaves the tree
  structure consistent for later inserts; a 65,000+-byte identical run
  stays within `MAX_MATCH_LEN` and does not hang. All five root
  CLAUDE.md gates clean (one private-intra-doc-link warning from the new
  public struct's docs naming private `MatchFinder`/`suffix_common_len`,
  fixed by dropping the links, same class as S2-A41). | No bpb
  measurement: not yet wired to any parse pass, so there is no champion
  to diff against — `progress.jsonl` records this as `kind: "patch"` with
  null bpb deltas, same reason as S2-A40/S2-A41. Remaining S1-P2 scope:
  wire this finder into `parse_optimal` in place of (or alongside)
  `MatchFinder`, add window eviction, add per-position adaptive prices
  (the DP's price table is currently frozen per round, S1-P2's other
  named gap), and measure a real bpb delta on sqlite/json/jsonl-shaped
  data, S1-P2's named target.
- S2-A43 | ACCEPTED | Second slice of ROADMAP M3's second standing lead
  (S1-P2, btultra2-class parse): the `len0`/`len1` length-prefix reuse
  optimization S2-A42 named and deliberately deferred, added to
  `lz::BinaryTreeMatchFinder::insert_and_find`. Prompted by re-reading
  S2-R2's own diagnosis before attempting SSE's next slice (S1-P1's
  remaining scope names an SSE path that S2-R1 already closed; S1-P2's
  remaining scope named a concrete, buildable prerequisite instead —
  picked over SSE for being unclaimed and well-specified, not because it
  ranks above S1-P1 in the standing-leads list). `suffix_common_len`
  gained a `start` parameter; `insert_and_find` tracks the common length
  already proven against the nearest node linked so far on each of the
  "less"/"greater" chains and starts each new comparison from the
  shorter of the two instead of byte 0. Sound because both chains stay
  sorted relative to `i`: any node still to be visited lies between the
  last-linked "less" node and the last-linked "greater" node in suffix
  order, so it shares at least their common prefix with `i` before a
  single byte of it is compared. **Measured, not assumed, against the
  exact fixture S2-R2 failed on** (`compression-experiment` skill's
  "prove the capability" step, `rust-craft`'s mechanical-sympathy
  emphasis on measuring instead of assuming): a hand-timed A/B (`start`
  forced to 0 vs. this optimization) on 200,000 bytes of one repeated
  value showed no measurable difference (~15ms at n=3,000 either way,
  scaled). Mechanism, found by tracing the algorithm rather than
  guessing from the timing alone: on a run of one repeated byte, every
  candidate's suffix ties with `i`'s up to the shorter one's end, so
  every comparison's tie-break (`insert_and_find`'s ordering rule) sends
  every candidate to the *same* side — the untouched side's common
  length never leaves 0, and `min(len0, len1)` is 0 forever. Length-prefix
  reuse only pays off when the walk actually alternates sides. A second
  hand-timed A/B on 300 near-duplicate 200-byte blocks (one varying byte
  per block, deliberately shaped closer to S1-P2's sqlite/json/jsonl
  target than a single-byte run) showed a real ~3.5x (970ms unoptimized,
  280ms with reuse) — real alternation from the varying byte gives the
  optimization something to bound. New test
  `binary_tree_near_duplicate_blocks_benefit_from_prefix_reuse` pins that
  win as a guard (asserts under 3s, generous headroom over the measured
  280ms); the existing `binary_tree_matches_brute_force` test (unchanged)
  is the correctness guard — it already independently re-derives every
  reported match's length via `match_len`, which a wrong `start` bound
  would have failed immediately, and still passes.
  | 1 new test (above); all five root CLAUDE.md gates clean. | No bpb
  measurement: still not wired to any parse pass, same reason as
  S2-A42. `progress.jsonl`
  records this as `kind: "patch"` with null bpb deltas. **Does not by
  itself unblock S2-R2's swap**: the issue #179 fixture (one repeated
  byte) is exactly the case this optimization cannot help, by the
  mechanism above, so re-wiring today would fail the same speed guard the
  same way. Remaining S1-P2 scope: a fix for the one-sided-branching
  pathology specifically — a cheap insert-only fast path for `dp_round`'s
  `carry` to skip through (so `insert_and_find`'s full walk is never
  reached on a long carried run), or a `nice_len`-style early exit once a
  found match is already long enough that a longer one cannot improve the
  DP price — before repeating the swap; window eviction and per-position
  adaptive prices remain untouched from S2-A42.
- S2-A44 | ACCEPTED | Third slice of ROADMAP M3's second standing lead
  (S1-P2, btultra2-class parse): `nice_len`, a third parameter on
  `lz::BinaryTreeMatchFinder::insert_and_find`, the early-exit half of the
  fix S2-A43 named but did not build (a cheap insert-only fast path, or a
  `nice_len`-style early exit). The walk now stops visiting further
  candidates once the best match found so far is at least `nice_len` long,
  cut off the same way an exhausted `max_depth` already is; passing
  `MAX_MATCH_LEN` (65535) reproduces the old unbounded search exactly,
  since `suffix_common_len` itself never reports a longer match than that.
  **Falsified the obvious follow-on hypothesis before it shipped**
  (`compression-experiment` skill's "measured, not assumed"): the first
  draft claimed this closes S2-A43's named one-sided-branching pathology
  (issue #179's 200,000-byte single-repeated-byte fixture) outright,
  reasoning that a low `nice_len` (128) would stop the walk after one
  candidate instead of up to `max_depth`. Measured against that exact
  fixture: 32.9s, still an order of magnitude past the issue's 15s speed
  guard. Root cause, found by tracing rather than re-guessing: `nice_len`
  only bounds how many *candidates* a walk visits, not the cost of scanning
  any single one — on a repeated-byte run, `suffix_common_len` itself scans
  to `MAX_MATCH_LEN` on the very *first* candidate, before `nice_len` is
  ever consulted. Rewritten before landing to claim only what's true: a
  new test measures `nice_len`'s real, positive effect on a shape where
  per-candidate cost is cheap and candidate *count* is what varies (the
  300-near-duplicate-block generator S2-A43 already introduced, unbounded
  `max_depth`) — 69ms with `nice_len` 50 vs. 228ms with no early exit, a
  real ~3.3x, smaller than the `len0`/`len1` win but compounding with it
  (both apply to every call). Does **not** unblock S2-R2's swap: the issue
  #179 fixture is unaffected, by the mechanism above. Remaining S1-P2
  scope, now sharper: the wiring blocker isn't candidate count, it's the
  per-candidate scan cost on highly repetitive runs, which needs an actual
  insert-only path that skips `suffix_common_len` itself while a `carry`
  is active, not a smaller `max_depth`-like bound layered on top of it;
  window eviction and per-position adaptive prices remain untouched from
  S2-A42.
- S2-A45 | ACCEPTED | S1-D2/S2-D1's scheduled-workflow slice, named as
  remaining scope since S2-A25: a new `corpus-fetch-check` workflow
  (`.github/workflows/corpus-fetch-check.yml`) runs clippy, tests, and doc
  for `mothergod-bench` with `--features corpus-fetch`, weekly (Sunday
  05:41 UTC, offset from the other advisory sweeps) plus
  `workflow_dispatch`, tests with `--include-ignored` so
  `fetch_and_cache_smoke_tests_the_real_pins` finally executes against the
  live pinned URLs. Closes the hole the feature gate left: keeping
  `ci.yml`'s required checks cheap meant zero CI coverage for the gated
  module, non-network unit tests included, so a stale `bench/corpus.toml`
  pin (URL moved, checksum drifted) would have surfaced only when an
  experiment run needed the real corpus. Designed and locally verified by
  the heartbeat in issue #231; landed by the BDFL because
  `.github/workflows/` paths need `GH_ADMIN_TOKEN` (issue #24). `fmt` is
  deliberately absent from the job: rustfmt formats by syntax, not active
  `cfg`s, so `ci.yml`'s default-feature check already covers the module's
  formatting. | Re-verified before landing, not restated from the issue:
  clippy clean, 111/111 tests pass including the real-network smoke test
  (the suite has grown from the issue's 62 since it was filed), doc build
  clean. The first scheduled run is the end-to-end proof of the YAML
  itself; a red run there is the alarm doing its job. | No bpb deltas:
  CI-coverage infrastructure, `kind: "patch"` in `progress.jsonl`.
  Remaining S1-D2/S2-D1 scope after this: the CI baseline gate's workflow
  wiring and real Silesia finals numbers (throughput-bound, per S2-D1).
- S2-A46 | ACCEPTED | Fourth slice of ROADMAP M3's second standing lead
  (S1-P2, btultra2-class parse): closes the gap S2-A44 measured and named
  as remaining scope — `nice_len` bounded how many candidates
  `lz::BinaryTreeMatchFinder::insert_and_find` visited, but not the cost
  of scanning any one of them, so the issue #179 fixture (200,000 bytes of
  one repeated value) still cost a full `MAX_MATCH_LEN`-length
  `suffix_common_len` scan on its very first candidate. `suffix_common_len`
  gained a `limit` parameter (`max_len = ....min(limit)`), and
  `insert_and_find` now passes its own `nice_len` as that limit instead of
  only checking `best_len < nice_len` between candidates. Sound for the
  same reason S2-A43's `start` bound is: a truncated scan still reports a
  real, verified common length, just possibly short of the true one — the
  struct's own docs already frame `nice_len` as "good enough, stop
  looking," and this extends that trade from "stop looking at more
  candidates" to "stop paying to confirm this one further," the same
  trade a small `max_depth` already makes over candidate count. On a
  repeated-byte run every candidate's capped scan now reaches `nice_len`
  immediately, so the walk stops after exactly one candidate instead of
  paying a second `MAX_MATCH_LEN`-length scan per position. | 2 new
  tests: `binary_tree_nice_len_caps_reported_length_when_true_match_is_longer`
  (a 300-byte repeated-'x' run with `nice_len` 50 must report length 50,
  not the true 299 — the correctness half, that the cap is honored, not
  just fast) and
  `binary_tree_nice_len_bounds_per_candidate_scan_cost_on_repeated_byte_run`,
  the issue #179 fixture itself run directly against the standalone finder
  with `nice_len` 128, asserting under 5s. All five root CLAUDE.md gates
  clean; `binary_tree_matches_brute_force` (nice_len `MAX_MATCH_LEN`,
  where the new `limit` is a no-op) still passes unchanged, the
  correctness guard for the unbounded case. | **Measured, not assumed**:
  the new issue #179 fixture test completes in ~0.11s in an unoptimized
  debug build (~0.04s release), against S2-A44's own measurement of 32.9s
  for `nice_len` alone on the identical fixture — roughly a 300x
  reduction, and comfortably inside the 15s budget `dp_round`'s existing
  regression guard (`optimal_roundtrip_long_run_of_one_repeated_byte_stays_linear`)
  enforces once this finder is wired in. No bpb measurement: still not
  wired to any parse pass (this measures the standalone finder directly,
  not through `dp_round`, which still uses `MatchFinder`'s hash chain) —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas,
  same as S2-A42 through S2-A44. **Does not yet re-attempt S2-R2's swap**:
  this closes the per-candidate-cost half of the gap S2-A44 named, but
  `dp_round`'s `carry` reuse still cannot skip `insert_and_find`'s call
  entirely on a long run the way it skips `MatchFinder::find_best` — every
  position still pays one bounded-now-but-nonzero `O(nice_len)` call, and
  `dp_round` would need a `nice_len` choice of its own (currently nothing
  in `dp_round` passes one) plus window eviction (still absent from
  S2-A42) before the wiring slice is worth re-attempting. Remaining S1-P2
  scope: pick a `nice_len` for `dp_round`'s use of this finder, add window
  eviction, add per-position adaptive prices (the DP price table is still
  frozen per round), then re-attempt S2-R2's swap and measure a real bpb
  delta on sqlite/json/jsonl-shaped data, S1-P2's named target.
- S2-A47 | REJECTED (process, not ratio) | Re-attempted S2-R2's swap per
  S2-A46's own remaining-scope note: `dp_round` called
  `finder.insert_and_find(i, MAX_TREE_DEPTH_OPTIMAL=640,
  NICE_LEN_OPTIMAL=128)` every position in place of `MatchFinder::insert`
  + a carry-skipped `find_best`, `MAX_TREE_DEPTH_OPTIMAL` held equal to
  the retired `MAX_CHAIN_TRIES_OPTIMAL` so the measurement isolated the
  finder swap, same methodology as S2-R2. Measured, not assumed: `cargo
  x check` clean; `cargo test --all-targets` green including the issue
  #179 regression guard S2-R2 broke (budget 15s, measured 1.06s debug /
  0.10s release, S2-A46's `nice_len` scan cap doing exactly the job it
  was built for); `bench::baseline` (11 train cases) net **-0.05376
  b/B** (`x86_dense_code` alone worsened, by +0.00016, inside
  `TOLERANCE_BITS` 0.02 but not literally zero-regression); sealed-only
  kinds `access_log` **-0.00016**, `gradient_image` unchanged. These
  numbers reproduce S2-R2's (it85) train/val deltas bit-for-bit, because
  `nice_len=128` never actually truncates a match on any of these
  generators — only the previously-unbounded per-candidate scan cost
  changed. Not S1-P2's named target either way: `json_records` and
  `sqlite_like_records` moved by -0.00064 and 0 respectively, the win is
  almost entirely `entropy_ladder_h1`/`h2` and `markov_h8_2_trap`.
  **Not merged, and not rejected on the merits**: `tests/golden.rs`'s
  `fixtures_decode_and_reencode_to_the_pinned_frame` fails, because
  `compress()`'s token choices changed, and its own message requires a
  `FORMAT_VERSION` bump + ADR (CLAUDE.md hard rule 5) to fix. This
  change touches no frame layout, method byte, or model semantic hard
  rule 5 names — `codec::decode` is byte-for-byte unchanged and a
  pre-change frame still decodes identically — so bumping the version
  for it would be the first bump ever for a purely encoder-side parse
  heuristic (0→1 and 1→2, ADR-0026/ADR-0028, were both genuinely
  decode-visible). Editing the golden pin to narrow its scope in the
  same PR that benefits from the narrowing is exactly the "don't grade
  your own claim" hazard hard rule 3 names, so this stopped instead of
  either bumping the version on its own authority or touching the test.
  Ruling requested: issue #290. Candidate code (the `dp_round` wiring,
  `MAX_TREE_DEPTH_OPTIMAL`, `NICE_LEN_OPTIMAL`) reverted in full, same as
  S2-R2; `BinaryTreeMatchFinder` itself (S2-A42/S2-A46) is unaffected.
  Remaining S1-P2 scope, once issue #290 resolves: land this wiring
  (with or without the format ceremony, per the ruling), then window
  eviction and per-position adaptive prices, still untouched from
  S2-A42.
- S2-A48 | ACCEPTED | Landed S2-A47's identical `dp_round` wiring: issue
  #290's ruling resolved between S2-A47 and this slice (`tests/golden.rs`,
  #292/#293) — an encoder-only change (`decode` byte-for-byte unchanged)
  regenerates the current-version golden fixture instead of needing a
  `FORMAT_VERSION` bump, so the sole blocker is gone. `dp_round` now
  builds a `BinaryTreeMatchFinder` and calls `insert_and_find(i,
  MAX_TREE_DEPTH_OPTIMAL, NICE_LEN_OPTIMAL)` every position in place of
  `MatchFinder::insert` + the `carry`-skipped `find_best` the deleted
  `next_match_candidate` wrapped; that wrapper and the normal-match
  `carry` variable are deleted outright rather than kept dead, since
  `insert_and_find`'s fused insert+search leaves nothing for a
  skip-the-search cache to skip (`rep_carry`, the separate rep-candidate
  cache, is unaffected and still runs every position). Measured fresh
  rather than trusted from S2-A47's record (`compression-experiment`
  skill, sealed-set discipline): `cargo run -p mothergod-bench --release
  --bin baseline_gate -- check` against a `git stash`-restored pre-change
  build reproduced S2-A47's train numbers bit-for-bit (net **-0.05376
  b/B** across `bench::baseline`'s 11 cases: `entropy_ladder_h1` -0.02912,
  `h2` -0.01200, `json_records` -0.00064, `markov_h8_2_trap` -0.01216,
  `x86_dense_code` +0.00016 worse — inside `TOLERANCE_BITS` 0.02 — every
  other case flat); a standalone throwaway binary (`bench/src/bin/
  measure_sealed_tmp.rs`, deleted before landing) measured the two
  sealed-only kinds the same way, before/after the same stash: `access_log`
  **-0.00016**, `gradient_image` unchanged, also bit-for-bit against
  S2-A47. `bench/baseline.json` updated to the new numbers (`baseline_gate
  -- write` then `cargo x fmt`) so this improvement is the regression
  floor going forward, not just a one-time measurement. `cargo test
  --all-targets`: 158 lib tests green including the issue #179 guard
  (unmeasured this slice; S2-A47 measured 1.06s debug/0.10s release
  against the same 15s budget, and nothing touching the bounded cost
  changed). `tests/golden.rs`'s re-encode pin failed as expected
  (`compress`'s token choices changed, 63 bytes vs the prior 62); the
  pre-change `v2-lz-repeated-text` pair moved to `tests/golden/superseded/`
  unchanged (still decode-checked, forever) and the pair in `tests/golden/`
  regenerated from the new `compress()` output via a second throwaway
  binary (`bench/src/bin/regen_golden_tmp.rs`, also deleted before
  landing) — `codec::decode` itself untouched, so `superseded_fixtures_still_decode`
  and the main fixture's own decode assertion both stay green. Not
  S1-P2's own named target even now: the win is still almost entirely
  `entropy_ladder`/`markov_h8_2_trap`, `json_records` moved a fraction of
  its own deficit and `sqlite_like_records` didn't move at all — real
  progress on that target needs the window eviction and per-position
  adaptive prices S2-A42 deferred, unchanged by this slice. Remaining
  S1-P2 scope: those two, still.
- S2-A49 | ACCEPTED | Closed the window-eviction half of S1-P2's
  remaining scope, unchanged since S2-A42:
  `BinaryTreeMatchFinder::insert_and_find`'s walk now evicts a candidate
  the instant its distance exceeds `WINDOW`, instead of continuing to
  link it into the less/greater chains and descend into its own
  children (only filtered at report time, as before). Proved safe
  before landing, not just tested: a node's `left`/`right` fields are
  written exactly once, at its own insertion, from whatever the
  bucket's tree held at that moment — strictly older positions only —
  so the walk's visited positions are provably a strictly decreasing
  sequence and the first out-of-window node's *entire* remaining
  subtree is also out-of-window; cutting there drops that subtree from
  the bucket for good; nothing re-links it once `head[h]` moves to a
  newer root. Because in-window candidates are always visited before
  any out-of-window one (same monotonic property), no previously
  reported match can ever become unreported: this is a pure
  tree-walk-cost fix, not a ratio change, closing the doc comment's own
  complaint that "long inputs pay unbounded tree-walk cost for buckets
  `WINDOW` cannot use." 2 new tests: an exact-boundary check (distance
  == `WINDOW` still reported, `WINDOW` + 1 never is) and a structural
  one that walks every position still reachable from a bucket's root
  after the eviction and asserts the stale one is gone, not merely
  excluded from the return value — the second fails against the
  pre-fix code (the old root gets linked into the new root's child
  regardless of distance). | `cargo x check` clean; the issue #179
  speed guard and `cargo run -p mothergod-bench --release --bin
  baseline_gate -- check` (11 cases) both stay green with **no bpb
  change**, exactly as predicted — no generator or golden fixture in
  this crate exceeds `WINDOW` (2^20 bytes) yet, so the new code path is
  untouched by any existing measurement; `research/README.md`'s
  capability-patch rule applies (null deltas, no champion to diff
  against). | Remaining S1-P2 scope: per-position adaptive prices,
  still untouched from S2-A42.
- S2-A50 | ACCEPTED | First slice of S1-P2's other remaining gap
  (per-position adaptive prices, untouched since S2-A42): `PriceCounts`
  gained `observe(token, prev_byte)`, bumping one already-decided
  token's counts, factored out of `tally`'s per-token match arm (`tally`
  now calls it in a loop; identical behavior, proven by a dedicated
  test rather than assumed from the refactor). Standalone, not yet
  called from `dp_round`: the DP's forward pass processes positions in
  strictly increasing order and every relax edge moves strictly
  forward, so by the time the loop reaches position `i`, `dp[i]` is
  already final — nothing at or after `i` can still improve it. That
  means the move that finalized `dp[i]` could legitimately feed a
  running price table as the loop advances, which is what "per-position
  adaptive" means; `tally` cannot do this because it only ever replays
  a *complete* token sequence handed to it after the fact, so it cannot
  sit inside `dp_round`'s own loop. Wiring that observation loop into
  `dp_round` — deciding how often to re-derive `PriceTable` from the
  running `PriceCounts` (every position is affordable in isolation, a
  fixed-size ~4,200-entry rebuild each time, but not amortized over a
  multi-megabyte input at every byte) and re-measuring bpb — is the
  next slice, deliberately deferred: same standalone-primitive-first
  order `BinaryTreeMatchFinder` (S2-A42) and `Sse` (S2-A40) shipped in,
  chosen here to avoid touching `dp_round`'s hot loop, the golden
  fixture, or the issue #179 speed guard in the same change that
  introduces a new, unverified DP behavior. | 6 new unit tests: `observe`
  on a literal bumps exactly its `(context, byte)` cell and nothing
  else, both with a real preceding byte and at stream start (`None`
  context, matching `tally`'s own `pos > 0` check); `observe` on a
  `Match` bumps length and offset only; `observe` on a `Rep` bumps
  length and `rep` only, not offset; repeated `observe` calls
  accumulate; and a direct comparison of `tally`'s output against
  driving `observe` one token at a time from outside (the shape a
  future caller would use) on the same sequence, asserting identical
  counts. `cargo x check` clean; `baseline_gate check` unaffected (11
  cases, no regression) since `dp_round`/`parse_optimal` are untouched.
  | No bpb measurement: `observe` is not called from any parse pass, so
  there is no champion to diff against — `progress.jsonl` records this
  as `kind: "patch"` with null bpb deltas, same reason as S2-A40/S2-A42.
  Remaining S1-P2 scope: wiring the observation loop into `dp_round`
  and measuring a real bpb delta on sqlite/json/jsonl-shaped data,
  S1-P2's named target, still unmoved by every slice so far.
- S2-A51 | REJECTED, see S2-R3 | Tenth slice of S1-P2's remaining scope:
  wired S2-A50's `PriceCounts::observe` into `dp_round`'s forward pass: a
  running price table, rebuilt from the observed counts every 4,096
  finalized moves, replacing the round's single frozen `PriceTable`. Net
  train effect ~−0.050 b/B across `bench::baseline`'s 11 cases, but the
  sealed split regressed on `access_log` (+0.0178 b/B): corpus policy's
  accept rule requires no validation regression, so this fails regardless
  of the net number, and the named target (`json_records`,
  `sqlite_like_records`) did not move favorably either. Full mechanism and
  the `PRICE_REBUILD_INTERVAL` sweep that ruled out an undertuned cadence:
  S2-R3. Remaining S1-P2 scope: an observation rule limited to tokens that
  survive to the final backtrace, not every position's locally-finalized
  move, the actual named sqlite/json/jsonl target, still unmoved by every
  slice so far.
- S1-P1 | RESOLVED 2026-08-29, closed by S2-A60 (`FORMAT_VERSION` 3,
  ADR-0038) | SSE (secondary symbol estimation), oldest standing lead —
  wired behind the literal mixer's binary decomposition
  (`bittree::encode_symbol_sse`/`decode_symbol_sse`, keyed by
  `bittree::sse_context`, refining the six-expert mixer's own blended
  probability at each of the 8 chained binary decisions instead of a lone
  counter). Net train **-0.36736 b/B**, both sealed kinds (`access_log`,
  `gradient_image`) improved, one case (`entropy_ladder_h6`) regressed
  inside the accepted trade. S2-R1's earlier attempt (an SSE stage over
  the flag model's lone order-0 `is_copy` counter) had failed for lack of
  a compound estimate to calibrate; that was the fix. Not the lead's
  originally-named target in a directly measured sense — the five zstd
  text holdouts are held-out finals, never inside the experiment loop
  (`research/corpus/POLICY.md`). Full mechanism, numbers, and the five
  slices that built it (S2-A40/S2-A41/S2-A58/S2-A59/S2-A60): S2-A60's own
  entry below.
- S1-P2 | LEAD | btultra2-class parse: binary-tree match finder with exact
  price feedback + per-position adaptive prices (ours were frozen per round).
  Targets sqlite/json/jsonl residue. First slice: S2-A42 (standalone
  binary-tree match finder, not yet wired). Second slice, wiring it
  straight into `dp_round` in place of the hash-chain `MatchFinder`: tried
  and rejected, S2-R2 — won on ratio (net train −0.054 b/B, no case
  regressed) but broke the issue #179 speed guard, because
  `insert_and_find` fuses insertion with search so `dp_round`'s `carry`
  can no longer skip the walk on a long run, and S2-A42 deliberately
  deferred the length-prefix-reuse optimization that would keep each
  comparison cheap regardless. Third and fourth slices narrowed but did not
  close that gap: length-prefix reuse (S2-A43, real ~3.5x on near-duplicate
  data, no measurable effect on the issue #179 fixture itself, since every
  candidate ties and lands on the same side there) and a `nice_len` early
  exit (S2-A44, real ~3.3x on the same near-duplicate shape, also no effect
  on the fixture — `nice_len` bounds candidates visited, not the cost of
  scanning the one candidate a repeated-byte run always finds first).
  Fifth slice, S2-A46: closed that per-candidate-cost gap directly by
  having `nice_len` also bound `suffix_common_len`'s own scan (a `limit`
  parameter), not just the between-candidate check — the issue #179
  fixture (200,000 bytes of one repeated value) run directly against the
  standalone finder dropped from S2-A44's measured 32.9s to ~0.11s
  (`nice_len` 128), a ~300x reduction, at the cost of reporting a
  candidate's match as exactly `nice_len` long when the true run is
  longer. Sixth slice, S2-A47: re-attempted S2-R2's swap with
  `nice_len=128` — this time the issue #179 guard passes (1.06s debug /
  0.10s release) and the ratio win reproduces S2-R2's exactly (train
  -0.05376 b/B, sealed access_log -0.00016/gradient_image unchanged; not
  actually on the named sqlite/json/jsonl target — the win is almost
  entirely `entropy_ladder`/`markov_h8_2_trap`). Blocked on process, not
  ratio or speed: failed `tests/golden.rs`'s re-encode pin, which at the
  time wanted a `FORMAT_VERSION` bump + ADR for any `compress()` output
  change at the current version, even though this one touched no frame
  layout, method byte, or model semantic CLAUDE.md hard rule 5 actually
  names. Ruling requested rather than decided unilaterally: issue #290,
  resolved (`tests/golden.rs` now regenerates the current-version fixture
  for an encoder-only change instead). Seventh slice, S2-A48: landed the
  identical wiring under that ruling — pre-change `v2-lz-repeated-text`
  moved to `tests/golden/superseded/`, current pair regenerated, numbers
  reproduce S2-A47 bit-for-bit. Eighth slice, S2-A49: closed the window-
  eviction half of the remaining scope — `insert_and_find` now evicts a
  candidate's entire remaining subtree the instant its distance exceeds
  `WINDOW`, proven (not just tested) to never change a reported match,
  since the walk's visited positions are strictly decreasing. Ninth
  slice, S2-A50: standalone primitive for the other half — `PriceCounts`
  can now be fed one already-decided token at a time (`observe`), not
  just replayed from a complete sequence (`tally`) — still not called
  from `dp_round`. Tenth slice, S2-A51/S2-R3: wired that observation loop
  into `dp_round`'s forward pass, rebuilding the price table from the
  running counts every 4,096 finalized moves. Rejected: a real sealed-
  validation regression (`access_log` +0.018 b/B), and the named target
  didn't move favorably either: the net train win was, again, entropy-
  ladder/markov statistical convergence, not sqlite/json/jsonl structure
  (the same shape S2-A47 already flagged once). Eleventh slice, S2-A56:
  a third `dp_round` round (own entry has the numbers), a different
  thread from the intra-round pricing question this scope note is about
  — S1-P2's remaining scope stayed the observation rule below throughout.
  Twelfth slice, S2-R4: a fourth `dp_round`, same shape as S2-A56,
  rejected on a small but real sealed regression, also not this thread.
  Thirteenth slice, S2-R5: tried the
  exact rule S2-R3 named as its remaining scope (observe only tokens
  that survive a backtrace, not every locally-finalized move),
  approximated as a checkpointed backward walk over `state.parent`
  rather than every relax candidate. Also rejected: the same
  `access_log` regression class, smaller (+0.00256) but still present,
  and removing S2-R3's diagnosed candidate-noise source did not fix it,
  pointing at a recency bias in what a partial file prefix's counts
  represent instead. Intra-round adaptive pricing has now failed on two
  different, deliberately-chosen observation rules; the next attempt, if
  any, is not a third variant of the same idea. Remaining S1-P2 scope:
  unclear — the sqlite/json/jsonl target has moved only slightly across
  every slice tried since S2-A46 (S2-A56's three-round DP: json_records
  −0.00256; S2-R5: json_records +0.00208, sqlite_like_records −0.00208,
  a wash), so the DP-pricing angle on this lead may be near its ceiling;
  a differently-shaped idea, not a pricing-cadence or observation-rule
  tweak, is owed before spending another slice here.
- S1-P3 | LEAD | PPM-style escape for literal contexts (see S1-R4). First
  slice: S2-A57 (standalone `Ppm` primitive, PPM Method C escape pricing,
  not yet wired). Second slice, S2-R6: measured the most-reasoned fallback
  target (order-0, the mixer's one non-context-keyed bank) via an
  ideal-cost pairing, before committing to a real wiring. Rejected: net
  regression on `bench::baseline` (+0.0458 b/B train average, a severe
  `gradient_image` sealed regression), worst on data where a byte's
  likelihood genuinely depends on context — `markov_h8_2_trap` and every
  structured generator tested — which order-0's global marginal cannot
  represent. Remaining scope: unclear, the same shape S1-P2 reached after
  repeated rejections; a fallback target other than the global marginal is
  owed before spending another slice here.
- S1-P4 | LEAD | LZMA-class windows for large files (xz's remaining edge).
  Several Silesia finals (`mozilla`, `nci`, `samba`, `sao`, `webster`) are
  many times larger than `lz::WINDOW` (1 MiB), so long-range repeats past
  that distance are structurally invisible to the current parse regardless
  of how well it prices what it can see. First slice: S2-A61 (`window` a
  per-instance parameter on `BinaryTreeMatchFinder`, standalone, the wired
  parse still always passes `WINDOW` unchanged). Second slice, S2-A62: the
  `long_range_repeat` corpus generator S2-A61 flagged as possibly needed —
  places a byte-identical repeat at a caller-chosen distance, standalone.
  Third slice, S2-A63: ran the measurement the generator enabled — a
  window under the existing `2^21 - 1` offset-bucket ceiling costs zero
  format change (`bucket()` already covers it) and closes real bpb on a
  planted long-range repeat: train **-0.021443**, sealed **-0.021401**,
  agreeing to four decimal places. Fourth slice, S2-A65: closed the
  `parse_greedy` half of S2-A63's remaining-scope note — its hash-chain
  `MatchFinder` gained the same per-instance `window` parameter
  `BinaryTreeMatchFinder` got in S2-A61, via a new
  `parse_greedy_with_window`; `parse_greedy` and the wired
  `parse_optimal_with_window` seed pass both still pass `WINDOW`
  unchanged, so nothing currently encoded moves. Remaining S1-P4 scope: a
  window past `2^21 - 1` needs `OFFSET_BUCKETS`/`bucket()` widened and a
  `FORMAT_VERSION` bump before it is measurable this way, which still
  leaves the Silesia finals named above (several 10s of MiB) out of
  reach; decide whether the wired `WINDOW` itself should grow toward (or
  to) that free `2^21 - 1` ceiling, now that both match finders on the
  wired path accept a window parameter, and the encode-time cost of a
  larger tree (SPEED, ROADMAP M5, untouched).
- S1-P5 | LEAD | Per-column modeling after transpose (filter-aware coder,
  OpenZL direction). Target: sao. First slice: S2-A64 (standalone
  `column::column_of`, not yet wired). Second slice, S2-A66: `column::
  column_bank`, wrapping `column_of`'s unbounded result into a fixed-size
  bank space so a future expert's storage sizes from a constant rather
  than the frame's declared `columns` (CLAUDE.md hard rule 2). Remaining
  scope: an actual column-index-keyed expert bank in `Literal`, threading
  the `columns` parameter filter selection already knows down to it, a
  `FORMAT_VERSION` bump, and a real bpb measurement.
- S1-P6 | LEAD | Speed tier: bit-decomposed coding (LPAQ-style, ~10×), tANS
  fast path (~100×, zstd-class -1 mode), explicit AVX2 blend (~1.5×).
  Concrete target as of S2-A27: `Literal::decode`'s all-literal worst
  case measures ~1170 ns/byte (~854 KB/s), under the ROADMAP SPEED floor
  (≥1 MB/s decode) — `Literal::mix` rebuilds all 256 cumulative entries
  from scratch every byte instead of an incremental structure.
- S1-P7 | LEAD | Production hardening: streaming mode, frozen format
  spec v1. The fuzzing half landed: targets S2-A25, scheduled CI
  S2-A53, remaining fuzz scope named in S2-A53.
- S1-P8 | LEAD | GLN-style predictors / more experts (2026 AIT Challenge
  entries) — only after SSE.
- S2-A52 | ACCEPTED | Silesia counterpart to S2-A45's Canterbury-facing
  `finals_report`: a new `silesia_report` binary (`bench`'s `corpus-fetch`
  feature) fetches each of Silesia's 12 individually pinned
  `bench/corpus.toml` entries (filtered by `corpus == "silesia"`, not a
  second hardcoded name list), decompresses with the already-tested
  `decompress_silesia`, and would write `docs/benchmarks/silesia.md` via
  the same `finals::format_report` `finals_report` already uses.
  Capability only, not run: `finals_report`'s own module doc measured
  Silesia's smallest file (`xml`, 5.3 MB) at 39s (~0.14 MB/s), so the full
  ~200 MB corpus is on the order of half an hour — too slow for a by-hand
  PR turn, the same call `finals_report` already made for Silesia's
  absence; this slice stops at capability, the same shape `finals_report`
  itself landed in (#252) before `canterbury.md` was generated in a
  follow-up (#253). `format_report` gained a `generator_bin` parameter
  (was hardcoded to say "finals_report" in every report's regeneration
  line, which would have made a Silesia report lie about which binary
  produced it) so both callers name themselves correctly. Also collapsed
  a real duplicate noticed while adding a third copy: `repo_root()`
  existed once in `finals_report.rs` and once in
  `render_baseline_graph.rs`; both now call a single
  `mothergod_bench::repo_root()`. `date -u` timestamp logic similarly
  consolidated into `mothergod_bench::reference::generated_at()`, shared
  by `finals_report` and `silesia_report` (kept out of
  `render_baseline_graph`, which isn't built with `corpus-fetch` and
  shouldn't need to be). | New tests:
  `format_report_names_its_generator_binary`,
  `generated_at_produces_an_iso8601_utc_timestamp`; full `cargo x check`
  clean; `cargo clippy -p mothergod-bench --all-targets --features
  corpus-fetch -- --deny warnings`, `cargo test -p mothergod-bench
  --features corpus-fetch --all-targets -- --include-ignored` (113
  passed, including the real-network
  `fetch_and_cache_smoke_tests_the_real_pins`), and `cargo doc -p
  mothergod-bench --features corpus-fetch --no-deps` all clean, matching
  `corpus-fetch-check.yml`'s exact commands; `baseline_gate check`
  unaffected (11 cases, no regression) since no codec code changed. | No
  bpb measurement: `silesia_report` has never been run, so there is no
  Silesia number to report yet — `progress.jsonl` records this as `kind:
  "patch"` with null bpb deltas, same reason as S2-A45. Remaining
  S1-D2/S2-D1 scope: an actual real Silesia run, by hand or via a
  scheduled workflow (issue #231).
- S2-A53 | ACCEPTED | Scheduled CI for S2-A25's fuzz targets, the
  remaining scope issue #53 named and issue #295 designed:
  `fuzz-check.yml` (#297) runs `decode_arbitrary` and `roundtrip`
  weekly, Sunday 06:13 UTC, offset from the other advisory sweeps, on
  Linux x64 only, 30s per target, nightly installed explicitly so a
  missing toolchain fails loudly instead of silently costing the first
  fuzz step its time budget. A found crasher fails the job, wakes the
  fixer via the alarm (ADR-0036), and uploads `fuzz/artifacts/` for
  promotion into `tests/adversarial/` as a regression seed.
  Deliberately single-OS, not #53's cross-OS `monster` suggestion:
  libFuzzer needs a nightly sanitizer-coverage rebuild per OS, and six
  instrumented rebuilds for a 30-second smoke check buy runner minutes,
  not coverage. | Verified locally before wiring (#295): 36,701
  executions of `decode_arbitrary` and 694 of `roundtrip`, no crashes,
  one 15s slow unit on a decode-amplification input matching the
  bounded-not-fast pattern S2-A25 first measured at 12s. First
  scheduled run 2026-08-30. | No bpb measurement: CI-coverage infra,
  not a ratio experiment; `progress.jsonl` records `kind: "patch"`
  with null deltas. Remaining M4 fuzz scope: cross-OS coverage in
  `monster`, the OSS-Fuzz application (needs an operator contact
  email, `blocked-on-human` when picked up), and an explicit
  allocation-limiter target beyond `MAX_DECODED_LEN`'s existing bound.
- S2-A54 | ACCEPTED | S2-A52 shipped `silesia_report` capability-only:
  Silesia's full corpus at the measured `xml` throughput (~0.14 MB/s
  single-threaded) is on the order of half an hour serially, too slow for
  one PR's by-hand turn. Closed that gap by parallelizing instead of
  waiting for a scheduled workflow: `mothergod_bench::reference::measure_all`
  runs one OS thread per file (`std::thread::scope`, no new dependency) —
  every file's measurement (`mothergod::compress` plus the three
  reference-compressor shells) touches only its own bytes, no shared
  mutable state, so spreading independent CPU-bound work across cores
  changes wall-clock time only, never which bytes get compressed or how.
  `finals_report` and `silesia_report` both call this one function now
  instead of each looping over its files in-process, collapsing a
  near-duplicate measurement loop that existed only because the two
  binaries fetch their corpora differently (tarball vs. 12 independent
  files) into one place that has nothing to do with that difference.
  | Measured: 8m20s wall clock (22m15s total CPU) on 4 cores for the full
  12-file, ~212 MB Silesia corpus, against the serial ~half-hour estimate
  `finals_report`'s module doc carried; `cargo x check` clean; `cargo
  clippy -p mothergod-bench --all-targets --features corpus-fetch --
  --deny warnings`, `cargo test -p mothergod-bench --features
  corpus-fetch --all-targets -- --include-ignored` (114 passed, up from
  113: the new `measure_all_measures_every_file_and_preserves_input_order`
  test), `cargo doc -p mothergod-bench --features corpus-fetch --no-deps`
  all clean, matching `corpus-fetch-check.yml`'s exact commands;
  `baseline_gate check` unaffected (11 cases, no regression) since no
  codec code changed. | Real Silesia numbers, landed in
  `docs/benchmarks/silesia.md`: aggregate 2.069848 bits/byte vs zstd
  -19's 1.996629 and xz -9e's 1.829058 (regret +0.240790 against the
  stronger reference); mothergod already beats both references outright
  on `ooffice` (regret -0.172963). Closes S2-D1/S1-D2's "real Silesia
  finals numbers" line and ROADMAP M2's report line, done by hand;
  nightly/weekly scheduling of either report is still unwired (a
  workflow file, `agent-system` scope, not this session's to land).
- S2-A55 | ACCEPTED | Issue #327: `canterbury.md`/`silesia.md` were
  regenerated by hand and could silently drift stale against the codec
  between hand-runs — confirmed live while landing this, not just
  theorized: they were last generated 2026-08-25, and every codec change
  since (S2-A48's binary-tree parse wiring, S2-A50's `PriceCounts::observe`,
  among others) had moved mothergod's numbers without either report
  following, e.g. `canterbury.md`'s `alice29.txt` read 2.589852 b/B,
  actually 2.587958. Closed without touching `.github/workflows/**`
  (this session's push identity cannot write workflow files; that path is
  the BDFL's alone, `agents/GOVERNANCE.md` "Push identity"): a
  `crate::baseline::fingerprint` (dependency-free 64-bit FNV-1a over
  `format_baseline`'s canonical text) embedded as an HTML comment in every
  report `finals::format_report` writes, naming the `bench/baseline.json`
  it was generated against. `baseline_gate check` (the existing required
  `ratio` job's own binary, no CI YAML changed) now also reads both
  reports and fails when either's embedded fingerprint doesn't match the
  current committed baseline — a content invariant against the committed
  files, not a PR-diff heuristic, so it holds regardless of which commit
  touched which file, and it needs no network fetch to check (only
  `finals_report`/`silesia_report` themselves, run by hand, need the
  real corpora). | `cargo x check` clean; `cargo clippy -p mothergod-bench
  --all-targets --features corpus-fetch -- --deny warnings` clean;
  `cargo run --release --features corpus-fetch --bin finals_report` and
  `--bin silesia_report` re-run to catch both reports up (see the
  Canterbury numbers above; Silesia's aggregate is unchanged within
  measurement, both reference-compressor numbers and mothergod's own,
  since no case in `bench/baseline.json`'s gate touches Silesia-shaped
  data specifically — the drift concentrated in the Canterbury text
  files SSE/binary-tree-parse work targets); `baseline_gate check` green
  against the refreshed reports. | Not a ratio experiment: no codec code
  changed in this PR, only the reporting layer catching up to already-
  recorded changes. Remaining scope this doesn't close: nightly/weekly
  regeneration scheduling is still unwired and still `agent-system`
  scope (a workflow file), same carve-out S2-A54 named.
- S2-A56 | ACCEPTED | S2-A9's own doc comment flagged the archive's
  `lz_opt` structure as "two DP rounds... not iterated to convergence"
  the day it was ported, and no slice had tested that gap since.
  `parse_optimal` now runs a third `dp_round`, reseeding its price table
  from the second round's own token sequence the same way the second
  round already reseeds from the first — pure repetition of an
  already-proven-correct step, not new DP machinery, so it carries none
  of S2-R2/S2-R3's wiring risk (a fresh `MatchFinder`, or intra-round
  price observation racing the forward pass). | `cargo x check` clean;
  the full `lz` module suite (40 tests) including the issue #179 speed
  guard (200,000-byte single-byte run, 0.15s for the whole module, well
  under the 15s bound — a third round adds a constant ~50% more
  `dp_round` work, not a new asymptotic cost); `tests/golden.rs`'s
  `fixtures_decode_and_reencode_to_the_pinned_frame` passed unchanged, no
  fixture regen needed — the pinned `v2-lz-repeated-text` fixture's parse
  already converged by round two, so its re-encode is bit-identical
  either way; this is still an encoder-only change per issue #290's
  ruling (`decode` untouched), just one this particular fixture happens
  not to exercise. | Measured on `bench::baseline`'s 11 train cases and
  the two sealed-only kinds (`access_log`, `gradient_image`), fixed
  seeds, `CASE_LEN` 50,000: net train effect ~−0.039 b/B, ten of eleven
  cases improved (`entropy_ladder_h6` −0.01104, `markov_h8_2_trap`
  −0.00624, `entropy_ladder_h4` −0.00736 carried most of it;
  `entropy_ladder_h8` flat), one regression within `TOLERANCE_BITS`
  (`base64_wrapped` +0.00144). Sealed split both improved: `access_log`
  −0.00112, `gradient_image` −0.00368 — no validation regression, unlike
  every S1-P2 wiring attempt so far (S2-R1's ladder tax, S2-R3's
  `access_log` regression). S1-P2's actual named target moved favorably
  but modestly (`json_records` −0.00256, `sqlite_like_records`
  −0.00096): real, but small enough that this slice does not claim to
  close S1-P2, which stays open at its S2-A51/S2-R3 stopping point — an
  observation rule limited to backtrace survivors, still unbuilt.
  `bench/baseline.json`, `docs/benchmarks/baseline.{md,svg}`, and (issue
  #327's fingerprint gate, S2-A55, now required) `canterbury.md`/
  `silesia.md` all regenerated to match: Canterbury aggregate 1.382712 ->
  1.381605 b/B (regret vs the stronger reference -0.020683 -> -0.021790),
  Silesia aggregate 2.069848 -> 2.068237 b/B (regret +0.240790 ->
  +0.239178) — both finals move the same direction as train/sealed, a
  small real win, not a regression the gate would have caught either way.
  Whether a fourth round keeps paying, and at what compress-time cost,
  is untested and a candidate next slice. Tested: see S2-R4, rejected on a
  sealed-validation regression despite a train win.
- S2-A57 | ACCEPTED | First slice of ROADMAP M3's third standing lead
  (S1-P3, PPM-style escape for literal contexts): a standalone
  `Ppm` primitive (`src/ppm.rs`), not yet wired into
  [`Literal`](crate::literal::Literal) or `codec.rs`, same shape S1-P1's
  first slice (S2-A40) and S1-P2's first slice (S2-A42) both took. Closes
  the gap `JOURNAL` S1-R4's near-miss diagnosis named: every adaptive
  table in this crate (`Model`, `Literal`'s six expert banks) Laplace-
  smooths every symbol to frequency 1 at construction, so "never observed
  in this context" and "observed once, decayed back near the floor" are
  indistinguishable in the table's own state — there is no representable
  escape signal a caller could act on. `Ppm` starts every symbol at
  frequency 0 instead, tracks the count of distinct symbols seen
  (`distinct`), and prices an unseen symbol's context as an explicit
  escape event under classic PPM Method C (escape frequency = distinct
  symbols so far, coding space `total + distinct`), with `encode`/
  `decode`/`encode_escape` driving the real range coder
  (`crate::coder`) the same way `Model::encode`/`decode` do, plus
  advisory `price_symbol`/`price_escape` (`-log2`, off the coding path,
  same `disallowed_methods` carve-out as `lz.rs`'s `PriceCounts::price`).
  Different from `JOURNAL` S1-R5 (rejected): S1-R5 blended every context
  unconditionally toward order-0, damaging the best-trained contexts
  most; `Ppm` only escapes a genuinely never-seen symbol, so a
  well-trained context essentially never pays the escape cost — the
  distinction S1-R4's diagnosis called for. | 12 unit tests: fresh-table
  escape is free and universal, observing a symbol clears only that
  symbol's own escape flag, `distinct` counts each symbol once regardless
  of repeats, a symbol's price falls as it recurs, escape price rises as
  one symbol dominates uncontested and is lower for a context that keeps
  discovering new symbols than one that stopped after its first (Method
  C's qualitative shape, proven, not just asserted), `encode`/
  `encode_escape`/`decode` panic on their documented misuse (coding an
  unseen symbol as real, escaping or decoding an empty table), a mixed
  real-symbol-and-escape sequence round-trips exactly through the real
  coder, and rescaling never turns a zero entry nonzero across 10,000
  repeats. `cargo x check` 4 stages green. | No bpb measurement: this
  primitive is not yet wired to any `Method` variant or reachable from
  `Literal`/`codec.rs`, so there is no champion to diff against —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas,
  same reason S2-A40/S2-A42 did for their own first slices.
  `baseline_gate check` confirms no regression (unaffected, no coding
  path changed). Remaining S1-P3 scope: picking where the escape's
  lower-order fallback lands (order-0? one of `Literal`'s other five
  experts? a fresh dedicated table?) and measuring the wired result
  against `bench::baseline`.
- S2-A58 | ACCEPTED | First implementable slice of S1-P1's own named next
  step: S2-R1's postmortem (`sse.rs` module docs) says the next SSE
  attempt "wants a compound/mixed estimate to calibrate instead (the
  literal mixer's eventual binary decomposition is the obvious one), not
  another raw `Model` split" — this builds that decomposition as a
  standalone primitive, the same pattern S2-A40/S2-A41/S2-A42/S2-A50/
  S2-A57 already used for their own first slices. New `src/bittree.rs`:
  `encode_symbol`/`decode_symbol` code one byte as 8 chained binary
  decisions over a caller-supplied 257-entry cumulative table (shaped
  like `Literal::mix`'s own output), each step splitting the current
  candidate symbol range at its midpoint and asking whether the true
  symbol falls in the upper half, at the probability that split has
  under the table. The chain rule of probability makes the product of
  those 8 binary probabilities equal the direct
  `(cum[symbol+1]-cum[symbol])/cum[ALPHABET]` ratio exactly, so this is
  the same partition `crate::coder::Encoder::encode`/`decode` already
  perform, reshaped into a sequence of binary decisions instead of one
  256-way division — the shape `crate::sse::Sse` calibrates, one context
  per (bit position, decided-prefix) pair, once wired. `ideal_cost_bits`
  checks that identity directly. | 9 unit tests: every symbol round-trips
  exactly on both a uniform and a heavily skewed synthetic table (not
  `Literal`'s own tables — this module is fully standalone, no dependency
  on `literal.rs`), a 2,000/5,000-symbol sequence round-trips through one
  real coded stream, ideal cost matches the direct symbol cost to
  `1e-9`, and real coded length tracks summed ideal cost within 5%
  (looser than `Literal::ideal_cost_bits`'s 1%, since this pays 8 chained
  16-bit-quantized `encode_bit` calls per symbol instead of one direct
  range division); `cargo x check` 4 stages green; `baseline_gate check`
  unaffected (no coding path changed, nothing wired in yet). | No bpb
  measurement, same reason S2-A40/S2-A42/S2-A50/S2-A57 recorded null
  deltas for their own first slices: not yet wired to any `Method`
  variant, no champion to diff against. `research/progress.jsonl` it103.
  Remaining S1-P1 scope after this slice: see S2-A59.
- S2-A59 | ACCEPTED | S2-A58's own remaining-scope note named the first
  open item as picking the `Sse` context keying for a (bit position,
  decided-prefix) pair; this slice decides it, as a standalone function,
  same pattern S2-A40/S2-A58 used for their own first slices rather than
  bundling the decision into the riskier wiring slice. `bittree::sse_context(depth,
  prefix)` maps one step of `encode_symbol`/`decode_symbol`'s walk (tree
  depth `0..8`, decided prefix `0..2^depth`) to a unique index in
  `0..255`, the classic LZMA-literal-coder node numbering
  (`(1 << depth) + prefix`, shifted to be 0-indexed): the cheapest scheme
  that still gives every one of the walk's 255 internal nodes its own
  calibration context, no coarser (folding nodes loses exactly the
  distinction the walk observed) and no finer (nothing more than tree
  position is available per node — the symbol identity is what has not
  been decided yet). Passes on the "256-context-per-bit-position" and
  hashed-context alternatives S2-A58's note raised: both would key on
  more than tree position alone (an order-1 dependency on the previous
  decoded byte), a genuinely different design question from "which
  context does this walk step address," left for the wiring slice to
  raise again if the plain scheme underperforms. | 5 new unit tests (14
  total in `bittree.rs`, up from 9): every `(depth, prefix)` pair the
  walk can reach maps into a bijection onto `0..255` (`SSE_CONTEXTS`);
  every one of the 256 symbols' root-to-leaf paths through
  `encode_symbol`'s own walk visits 8 distinct contexts, checked against
  the walk's real `lo`/width arithmetic rather than asserted in
  isolation; both out-of-range panics (`depth >= LEVELS`, `prefix >=
  2^depth`); `SSE_CONTEXTS == 255`. `cargo x check` 4 stages green;
  `baseline_gate check` unaffected (pure function, no coding path
  touched, nothing wired in yet). | No bpb measurement, same reason
  S2-A58 recorded a null delta: not yet wired to any `Method` variant, no
  champion to diff against. `research/progress.jsonl` it104. Remaining
  S1-P1 scope: wire `bittree::encode_symbol`/`decode_symbol` +
  `sse_context` behind `Literal::encode`/`decode` in place of the direct
  256-way `mix`/scan, with one `Sse` table of `bittree::SSE_CONTEXTS`
  contexts calibrating the mixer's own per-decision probability, bump
  `FORMAT_VERSION`, measure a real bpb delta on the corpus policy's
  train/sealed split — S2-R1's postmortem is still the live risk here:
  the prior wiring attempt showed a raw order-0 binary decision has
  little systematic bias left for SSE to correct, and this slice does
  not yet know whether a mixer-derived decision differs enough to change
  that verdict.
- S2-A60 | ACCEPTED, closes S1-P1 | S2-A59's own remaining-scope note: the
  wiring slice, `FORMAT_VERSION` 3 (ADR-0038). `literal::Literal` gained an
  `sse: Sse` field (`Sse::new(bittree::SSE_CONTEXTS)`) and
  `encode_sse`/`decode_sse`, coding the same mixed `cum` table
  `encode`/`decode` already build through `bittree::encode_symbol_sse`/
  `decode_symbol_sse` (new combinators next to `encode_symbol`/
  `decode_symbol`, reusing their private `check_table_shape`/
  `upper_half_probability` rather than duplicating the chain-rule walk):
  each of the 8 levels refines the raw upper-half probability through
  `sse.refine(sse_context(depth, prefix), raw_p)` before
  `Encoder::encode_bit`/`Decoder::decode_bit`, then updates on the raw
  probability. `Literal::update` still runs unconditionally after every
  symbol regardless of path, so the six-expert mixer keeps adapting
  identically. `codec::decode` gained a `version: u8` parameter (threaded
  from `lib.rs::decompress`, which already had it in scope) and dispatches
  the literal decode call on it (`codec::LITERAL_SSE_MIN_VERSION` = 3);
  `EncodeSink::literal` always calls `encode_sse` (compression targets the
  newest version); `tests/golden/v2-lz-repeated-text.mgdc` still decodes
  unchanged, and a new `tests/golden/v3-lz-repeated-text` pair pins the
  new shape. `codec::ideal_cost_bits`'s `CostSink` also switched to a new
  `Literal::ideal_cost_bits_sse`/`bittree::ideal_cost_bits_sse` (pure
  `-log2` sum through the same SSE-refined chain, no `Encoder`): the old
  `ideal_cost_bits` still exists (`Literal`'s own tests use it against the
  pre-SSE `encode`), but leaving `CostSink` on it would have silently
  desynced ideal-cost pricing from what `EncodeSink` actually codes now,
  exactly the hazard `codec.rs`'s own module docs warn `TokenSink`
  exists to prevent — caught by
  `ideal_cost_bits_tracks_real_encoded_length_within_one_percent` failing
  at 1.33% before this fix. | Measured on `bench::baseline`'s 11
  train-tier cases and the two sealed-only kinds (`access_log`,
  `gradient_image`): net train **-0.36736 b/B**
  (`interleaved_audio16` -0.36368 carried most of it; `base64_wrapped`
  -0.01312, `x86_dense_code` -0.01760, `json_records` -0.01024,
  `entropy_ladder_h1` -0.00512 also improved; `entropy_ladder_h2`
  +0.00016 and `markov_h8_2_trap` +0.00016 flat, `entropy_ladder_h4`
  +0.01040 and `sqlite_like_records` +0.00800 both inside
  `TOLERANCE_BITS`). Sealed split both improved: `access_log` -0.01264,
  `gradient_image` -0.13472. One case regressed past `TOLERANCE_BITS`
  (0.02): `entropy_ladder_h6`, +0.02368 — iid random data at 6 bits/byte,
  where the pre-SSE mixer already pays ~0.156 b/B of modeling noise above
  the 6.0 floor, and SSE's per-context warm-up plus the 8-chained-binary-
  decision path's own quantization add roughly another 0.024 on top, with
  no real systematic bias there for SSE to correct. `research/corpus/
  POLICY.md`'s accept rule (train improvement, no validation regression)
  reads on the net numbers; the `entropy_ladder_h6` regression is declared
  here as the accepted trade `baseline_gate check`'s own message asks
  for, and `bench/baseline.json` updated to the new numbers in the same
  PR (`baseline_gate write` + `cargo x fmt`). | Mechanism: unlike S2-R1's
  lone order-0 `is_copy` counter, the six-expert mixer's blended
  probability at each binary-tree node is a genuinely compound estimate
  with real systematic bias for SSE to find and correct — largest where
  the mixer's own six-way blend is noisiest relative to the true
  structure (`interleaved_audio16`'s two-rate fast/slow byte-interleave
  pattern, `gradient_image`'s smooth low-order drift), smallest to
  negative where there is no structure to find (the entropy ladder, worst
  at `h6`). `cargo x check`: 4 stages green, 206 lib tests (up from 192:
  5 new round-trip/SSE-win tests in `bittree.rs`, 9 in `literal.rs`),
  golden and adversarial suites green including a new
  `tests/adversarial/lz-v3-truncated-literal-stream` seed (a real
  `FORMAT_VERSION` 3 frame truncated mid-literal-stream, exercising
  `decode_sse`'s panic-free-on-truncation path the same way
  `literal::tests::decoding_truncated_stream_does_not_panic_through_sse`
  already does at the `Literal` layer). `docs/adr/0038-wire-sse-into-the-
  literal-mixer.md` records the decision; `docs/format/SPEC.md` updated
  for the version-gated literal sub-stream shape. `bittree.rs`'s and
  `sse.rs`'s "remaining scope" docs updated to point here instead of
  restating the now-closed wiring question. Not S1-P1's originally named
  target even now in a directly measured sense: the five zstd text
  holdouts are held-out finals, never inside the experiment loop
  (`research/corpus/POLICY.md`) — this PR's Canterbury/Silesia report
  regeneration is a mechanical fingerprint refresh
  (`bench/baseline.json` changed), not an accept signal.
  `research/progress.jsonl` it105.
- S2-A61 | ACCEPTED | First slice of ROADMAP M3's fourth standing lead
  (S1-P4, LZMA-class windows for large files): `lz::BinaryTreeMatchFinder`
  now takes `window: usize` as a constructor parameter instead of reading
  the crate-wide `lz::WINDOW` constant (1 MiB) directly, the same
  standalone-primitive-first order S1-P1/S1-P2/S1-P3 each opened with
  (S2-A40, S2-A42, S2-A57) — except here the primitive already exists and
  is wired; this slice only frees the one hardcoded bound
  `insert_and_find`'s eviction check (`distance > WINDOW`) used, so a
  larger window is measurable on the standalone finder before any of the
  real decisions (offset-bucket count, `Model` alphabet size,
  `FORMAT_VERSION`) get made. `dp_round`, the wired parse `codec.rs`
  actually calls, still constructs its finder with `WINDOW` explicitly —
  bit-for-bit identical output, no format or ratio effect. Motivation:
  several Silesia finals (`mozilla`, `nci`, `samba`, `sao`, `webster`) are
  many times larger than 1 MiB, so long-range repeats past that distance
  are currently invisible to the parse regardless of pricing quality —
  `research/corpus/POLICY.md` already names enwik8/9 as "relevant once
  large windows land (M3+)". | 1 new unit test
  (`binary_tree_larger_window_finds_matches_the_default_window_would_miss`,
  207 lib tests total, up from 206): a finder constructed with `WINDOW * 2`
  finds a match at that distance, and a finder constructed with the
  default `WINDOW` on the same data does not — proving the parameter
  actually gates reach rather than being threaded through unused. The 13
  existing call sites (the wired `dp_round` plus 12 test finders) all pass
  `WINDOW` explicitly, so none of them changed behavior. `cargo x check`:
  4 stages green (the doc stage first caught a private-intra-doc-link
  warning from `WINDOW`'s public doc comment naming the now-parameterized
  private `BinaryTreeMatchFinder::new`, fixed by dropping the link, same
  class as S2-A41/S2-A42). | No bpb measurement: `dp_round`'s own call is
  unchanged, so there is no champion to diff against —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas,
  same reason as S2-A40/S2-A42/S2-A57. Remaining S1-P4 scope: measure
  whether a larger window actually helps on train-tier data shaped like
  the named Silesia targets (a long-range-repeat generator does not yet
  exist in `bench/`'s corpus — may itself need a capability slice first,
  `research/corpus/POLICY.md`'s "our own" generator class); decide how a
  larger window reaches the coder — `lz::OFFSET_BUCKETS`/`bucket()` and
  `codec.rs`'s `Model::new(lz::OFFSET_BUCKETS)` currently assume 21
  buckets (`bucket(WINDOW) == 20`) sized for a 1 MiB bound, and `to_u32`'s
  `u32`-fits assumption caps any window this scheme could ever use at
  `u32::MAX`; wire the chosen window behind a real parse pass and bump
  `FORMAT_VERSION`. `research/progress.jsonl` it107.
- S2-A62 | ACCEPTED | Second slice of ROADMAP M3's fourth standing lead
  (S1-P4, LZMA-class windows for large files): a `long_range_repeat`
  corpus generator added to `bench/` (`bench/src/lib.rs`), the capability
  S2-A61's own remaining-scope note flagged as possibly needed first — no
  existing generator in `bench`'s corpus could place a repeat at a
  caller-chosen distance, so a larger window's train-tier effect (several
  Silesia finals many times larger than `lz::WINDOW`) had nothing to
  measure against. Not ported from the founding session (`corpus.py`
  predates this need); a new "our own" generator per
  `research/corpus/POLICY.md`. Fills `len` bytes at 6 bits of order-0
  entropy (dense enough that a planted repeat is the only long-range
  structure to find), then copies the first 4,096 bytes
  (`LONG_RANGE_REPEAT_TEMPLATE_LEN`) to a caller-chosen `distance`, so the
  two occurrences are byte-identical and exactly `distance` apart by
  construction — no separate template draw, the filler's own first block
  doubles as the template. | 7 new unit tests (108 bench tests total, up
  from 101): exact length across three (len, distance) pairs, determinism,
  seed independence, both panics (`distance` shorter than the template,
  `len` too short for both occurrences), the planted-pair placement across
  three distances including one past `lz::WINDOW` (1,052,672 bytes), and a
  full 4,096-byte sliding-window scan over the output proving exactly two
  occurrences exist — no incidental collision from the filler at that
  entropy and length. A new case also joined
  `generators_round_trip_through_the_frame_format`, the other nine
  generators' existing lossless check. `cargo x check`: 4 stages green
  (the doc stage first caught the same private-intra-doc-link class as
  S2-A41/S2-A42/S2-A61 — two doc comments linked the private
  `LONG_RANGE_REPEAT_TEMPLATE_LEN` constant via `` [`...`] ``, fixed by
  dropping to plain code-span text). | No bpb measurement: not wired into
  `DatasetKind`/`bench::baseline`'s CI ratio gate — that gate's `CASE_LEN`
  is 50,000 bytes, far below `lz::WINDOW`, and folding a >1 MiB case into
  every PR's regression gate is a separate sizing decision this capability
  slice does not make — `progress.jsonl` records this as `kind: "patch"`
  with null bpb deltas, same reason as S2-A40/S2-A42/S2-A57/S2-A61.
  Remaining S1-P4 scope, unchanged from S2-A61 except the generator gap
  now closed: run the actual experiment (a real `dp_round` pass at a
  larger window against `long_range_repeat`-shaped train-tier data), decide
  how a larger window reaches the coder (`lz::OFFSET_BUCKETS`/`bucket()`,
  `codec.rs`'s `Model::new(lz::OFFSET_BUCKETS)`, both sized for the 1 MiB
  bound today, and `to_u32`'s `u32`-fits ceiling on any window this scheme
  could ever use), wire the chosen window behind a real parse pass, bump
  `FORMAT_VERSION`. `research/progress.jsonl` it108.
- S2-A63 | ACCEPTED | Third slice of ROADMAP M3's fourth standing lead
  (S1-P4, LZMA-class windows for large files): ran the experiment
  S2-A62's own remaining-scope note named, and closed the piece of it
  that needs no wiring or `FORMAT_VERSION` decision. `bucket()` is
  `floor(log2(v))`, so every distance up to `2^21 - 1` already falls
  inside `OFFSET_BUCKETS`'s existing 21 slots (`bucket(WINDOW)` is 20,
  the same slot every value up to `2^21 - 1` shares); a window anywhere
  under that ceiling is measurable through the crate's real adaptive
  models today, no bitstream change needed. Two new functions carry
  `window` down to where `dp_round` was hardcoded to the wired `WINDOW`:
  `lz::parse_optimal_with_window(data, window)` (`parse_optimal` is now a
  thin wrapper passing `WINDOW`; `parse_greedy`'s seed pass stays bound by
  the wired `WINDOW` regardless, since it only shapes the first round's
  starting price table, not correctness) and
  `codec::ideal_cost_bits_with_window(data, window)` (`ideal_cost_bits`
  likewise now wraps it), mirroring S2-A61's parameterize-the-primitive
  pattern one level up the call stack. `dp_round` itself gained a
  `debug_assert!` that `window`'s bucket stays inside `OFFSET_BUCKETS`,
  since `prices.offset` has exactly that many entries and an
  out-of-range bucket panics on the index rather than mispricing
  silently. | Measured with a throwaway `#[ignore]`d test (run locally,
  not committed, per S2-A62's own note that folding a >1 MiB case into
  every PR's gate is a separate sizing decision): `bench::long_range_repeat(len:
  1,222,672, seed, distance: 1,198,576)` (template 4,096 B past
  `lz::WINDOW`, well under the `2^21 - 1` ceiling), `old_window =
  lz::WINDOW` (1,048,576) vs `new_window = 1,248,576`. Train seed
  (`0xC0FFEE123456789A`): 6.295301 -> 6.273858 bpb, **-0.021443**. Sealed
  seed (`sealed_seed` of the same key): 6.295546 -> 6.274145 bpb,
  **-0.021401**. Both seeds agree to four decimal places: no
  seed-specific fluke. | Mechanism: at `old_window` the far occurrence of
  the 4,096-byte template is invisible to the parse (evicted past
  `WINDOW`), so those bytes cost the dense 6-bit-entropy filler's own
  floor; at `new_window` `dp_round`'s `BinaryTreeMatchFinder` reaches the
  first occurrence and a single match token replaces roughly
  `4,096 * 6` bits of literal coding, matching the measured delta's
  order of magnitude (`24,576 bits / 1,222,672 bytes ~= 0.0201 bpb`,
  close to the ~0.0214 measured once match/flag overhead is included).
  The win scales with the repeat's share of the file: proportionally
  larger on a file with more long-range structure than this one
  generator config plants, proportionally smaller on one with less.
  `cargo x check`: 4 stages green, 209 lib tests (up from 207: one new
  `lz::tests::optimal_with_window_reaches_a_repeat_a_smaller_window_would_miss`
  proving `window` gates `parse_optimal_with_window`'s reach end to end,
  round-trip included; one new
  `codec::tests::ideal_cost_bits_with_window_drops_once_a_repeat_becomes_reachable`
  proving the real Models pipeline reports the cost drop, not just the DP's
  own price heuristic). Both new public functions are additive: the
  wired `parse_optimal`/`ideal_cost_bits` call sites are unchanged, so
  this ships zero effect on any currently-encoded bitstream and no
  `FORMAT_VERSION` bump. Remaining S1-P4 scope: a window past `2^21 - 1`
  needs `OFFSET_BUCKETS`/`bucket()` widened and a `FORMAT_VERSION` bump
  before it is measurable this way, which still leaves the Silesia
  finals named in S2-A61 (several 10s of MiB) far out of reach; decide
  whether the wired `WINDOW` itself should grow to (or toward) the
  `2^21 - 1` ceiling this slice proved free of format cost, including
  `parse_greedy`'s own hash-chain finder (still hardcoded to `WINDOW`,
  unexamined here) and the encode-time cost of a larger tree (SPEED,
  ROADMAP M5, untouched by this slice). `research/progress.jsonl` it109.
- S2-A64 | ACCEPTED | First slice of ROADMAP M3's fifth standing lead
  (S1-P5, per-column modeling after transpose): a standalone
  `column::column_of(position, columns, len)` in a new `src/column.rs`,
  the same "pure mapping function, standalone, not yet wired" shape
  S2-A59's `bittree::sse_context` took for its own lead. `filters::
  transpose::encode` already regroups a row-major stream into column-major
  order (`JOURNAL` S1-A2) so a downstream model with only short-range
  context can see a column's own regularity as byte adjacency, but
  `literal.rs`'s "alignment" expert only keys on `position & 3`, a fixed
  period-4 phase useful for interleaved fixed-width records — it has no
  notion of *which* transposed column a byte belongs to, so it cannot
  give a column its own distribution at the instant a column boundary is
  crossed, only after re-adapting from a few bytes of the wrong column's
  evidence. `column_of` is the arithmetic a future column-index-keyed
  expert needs: `transpose::encode` groups column `c` into `len /
  columns` bytes (`rows`), plus one more for the first `len % columns`
  columns (`long_columns`, the row remainder's leftover bytes) — the
  first `long_columns` columns (each `rows + 1` wide) sit at the front of
  the output, the rest (each `rows` wide) after, so a closed-form
  division locates any position's column without replaying the filter's
  own loop. | 6 unit tests (215 lib tests total, up from 209): a property
  test across 11 lengths (0 to 1000) times 11 column counts comparing
  every position's `column_of` result against
  `naive_column_of_each_position`, an independent replay of `transpose::
  encode`'s own nested loop that records column index instead of copying
  a byte (so a divergence would mean the closed form disagrees with the
  filter it describes, not just with itself); single-column identity;
  exact-division equal-width columns; the remainder case matched directly
  against `filters::transpose`'s own `encode_groups_by_column` fixture
  (`[a,A,b,B,c]` -> `[a,b,c,A,B]` under 2 columns); columns wider than the
  data (every position its own column, mirroring `transpose`'s own
  `roundtrip_fewer_rows_than_columns`); the documented
  `position >= len` panic. `cargo x check`: 4 stages green;
  `baseline_gate check` unaffected (11 cases, no regression — pure
  function, no coding path touched, nothing wired in yet). | No bpb
  measurement, same reason as every other lead's first slice
  (S2-A40/S2-A42/S2-A57/S2-A58/S2-A61/S2-A62): not yet wired to any
  `Method` variant or reachable from `Literal`/`codec.rs`, no champion to
  diff against — `progress.jsonl` records this as `kind: "patch"` with
  null bpb deltas. `research/progress.jsonl` it110. Remaining S1-P5
  scope: see the updated S1-P5 entry above — an actual column-index-keyed
  expert bank in `Literal`, threading `columns` down from filter
  selection, a `FORMAT_VERSION` bump, and a real bpb measurement (`sao`'s
  regret is currently +0.656965 b/B on the Silesia held-out final,
  `docs/benchmarks/silesia.md`, the largest of any file there — named
  here for target framing only, per `research/corpus/POLICY.md` held-out
  finals are never an accept/reject signal inside the experiment loop).
- S2-A65 | ACCEPTED | Fourth slice of ROADMAP M3's fourth standing lead
  (S1-P4, LZMA-class windows for large files): closed S2-A63's own
  remaining-scope note that `parse_greedy`'s hash-chain finder was "still
  hardcoded to WINDOW, unexamined", the one match finder on the wired
  path S2-A61 (`BinaryTreeMatchFinder`) and S2-A63
  (`parse_optimal_with_window`) had not yet parameterized. `MatchFinder`
  (the hash chain `parse_greedy` and `parse_optimal`'s seed pass both
  use) gained a `window: usize` field, stored by a new `MatchFinder::new
  (data, window)` and read by `find_best` in place of the crate-wide
  `WINDOW` constant. A new `parse_greedy_with_window(data, window)` is
  the real function body; `parse_greedy` is now a thin wrapper passing
  `WINDOW` unchanged, mirroring `parse_optimal`/`parse_optimal_with_window`'s
  own split (S2-A63). `parse_optimal_with_window` does not call it: its
  own docs already record, as a deliberate choice, that the seed pass
  stays bound to the wired `WINDOW` regardless of the DP rounds' window —
  this slice makes that choice measurable on its own, it does not revisit
  it. | 1 new unit test (216 lib tests total, up from 215):
  `greedy_with_window_reaches_a_repeat_a_smaller_window_would_miss`, the
  same planted-repeat shape as S2-A63's own
  `optimal_with_window_reaches_a_repeat_a_smaller_window_would_miss`,
  proving a small window never reports a match past it and a large one
  finds the planted repeat, round-tripping either way. `cargo x check`:
  4 stages green (lint first caught a `clippy::doc_markdown` finding on
  an unbacktick'd `parse_greedy` in the new function's own doc comment,
  fixed). `baseline_gate check`: 11 cases, no regression — `parse_greedy`
  and `parse_optimal_with_window` both still pass `WINDOW` unchanged, so
  no currently-encoded bitstream moves and no `FORMAT_VERSION` bump is
  owed. | No bpb measurement, same reason as every other lead's
  parameterize-the-primitive slice (S2-A40/S2-A42/S2-A57/S2-A58/S2-A61/
  S2-A62/S2-A64): standalone, not yet wired to a different value than
  today's — `progress.jsonl` records this as `kind: "patch"` with null
  bpb deltas. `research/progress.jsonl` it111.
- S2-A66 | ACCEPTED | Second slice of ROADMAP M3's fifth standing lead
  (S1-P5, per-column modeling after transpose): `column::column_bank
  (column, max_banks)` (`column % max_banks.get()`), the bound
  `column_of` (S2-A64) still lacked. A future column-index-keyed expert's
  bank storage must size from a constant alone, never from the frame's
  declared `columns`: a decoder reads `columns` from untrusted compressed
  input, so sizing bank count to it directly would let a hostile frame
  drive unbounded allocation, CLAUDE.md hard rule 2. `literal.rs`'s
  existing five non-order0 experts already solve the identical unbounded-
  context problem the same way — `ORDER2_BASE`'s `& 0xFFF`, `WORD_BASE`'s
  `& 0xFFF`, `ALIGN_BASE`'s `position & 3` — so this borrows that
  convention rather than inventing one: real per-column separation for
  the common case this lead targets (structured data with a modest
  column count), aliasing distant columns onto the same bank rather than
  allocating one per column for an adversarial `columns` value. | 4 new
  unit tests (220 lib tests total, up from 216): identity when `columns`
  fits within `max_banks`; explicit wraparound arithmetic for 10 columns
  into 4 banks; every result of 100 columns stays under a 3-bank cap; an
  end-to-end check piping every position of a 37-column, 200-byte stream
  through `column_of` then `column_bank` into a 5-bank space. `cargo x
  check`: 4 stages green; `baseline_gate check`: 11 cases, no regression
  — pure function, nothing wired in yet. | No bpb measurement, same
  reason as S2-A64 and every other lead's non-wired slice: no `Method`
  variant or mixer reads this yet, so there is no champion to diff
  against — `progress.jsonl` records this as `kind: "patch"` with null
  bpb deltas. `research/progress.jsonl` it113 (it112 is PR #353's S2-R7,
  still open at this writing; picking it113 avoids a same-id collision
  regardless of merge order). Remaining S1-P5 scope,
  unchanged from S2-A64: an actual column-index-keyed expert bank wired
  into `Literal`, threading `columns` down from filter selection, a
  `FORMAT_VERSION` bump, and a real bpb measurement — `column_of` and
  `column_bank` together are the arithmetic that wiring needs, still
  neither one is called from `literal.rs` or `codec.rs`.
