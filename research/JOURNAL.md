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
- S2-R7 | REJECTED | S1-P4's own remaining scope (S2-A63's closing note):
  decide whether the wired `WINDOW` (`lz::WINDOW`, 1 MiB) should itself
  grow toward the `2^21 - 1` offset-bucket ceiling S2-A63 proved free of
  format cost, given both a real bpb effect and the encode-time cost of a
  larger tree named as still open. Measured with a throwaway (uncommitted)
  scratch binary, `bench/src/bin/window_experiment.rs` (deleted after this
  measurement, per S2-R6's own convention), `codec::
  ideal_cost_bits_with_window` at `old_window = lz::WINDOW` (1,048,576)
  vs `new_window = 2,097,151` (`2^21 - 1`), release build, at 4,000,000
  bytes per case (`bench::baseline`'s own `CASE_LEN`, 50,000, cannot show
  any window effect at all — every distance in a case that small sits
  far under `WINDOW` itself regardless of which window is wired, so this
  needed its own, much larger, ad hoc length): train seed
  `0xC0FFEE123456789A` (S2-A63's own key) across `bench::baseline`'s four
  train-eligible non-ladder kinds, mean **-0.007797 b/B**
  (`sqlite_like_records` -0.006542, `markov_h8_2_trap` -0.025424,
  `x86_dense_code` +0.000505, `json_records` +0.000273 — the two
  improvements come from incidental multi-megabyte-scale recurrence
  `long_range_repeat` engineers on purpose, `markov_h8_2_trap`'s bounded
  low-conditional-entropy state space revisiting old states, `sqlite_like_
  records`'s synthetic record structure repeating widely by chance at this
  length; neither is the kind of structure the two-generator regression
  represents). Sealed check at `sealed_seed(0xC0FFEE123456789A)`, the same
  pairing S2-A63/S2-R4 use, on both actual sealed-only kinds
  (`bench::baseline` excludes `access_log`/`gradient_image` from the train
  gate for exactly this reason): both regressed — `access_log` +0.000189
  b/B, `gradient_image` +0.000092 b/B. Small in absolute terms — roughly
  0.6x and 0.3x of S2-R4's own deciding regression (`access_log`
  +0.00032), not an order of magnitude below it as an earlier draft of
  this entry miscalculated (a stray byte-to-bit conversion inflated every
  delta in this paragraph 8x; corrected 2026-08-29, PR #353 review) — but
  real and consistent in direction across both sealed kinds, not a coin
  flip.
  Encode time (the same `ideal_cost_bits_with_window` calls, wall clock)
  grew 1.05x-1.41x at `new_window` across every case measured, consistent
  with `insert_and_find`'s own docs: a smaller window lets the
  distance-eviction check (`distance > self.window`) cut a tree walk short
  before `max_depth` every time it hits an out-of-window node, so growing
  the window mostly removes that early exit rather than adding new work —
  bounded by `MAX_TREE_DEPTH_OPTIMAL`/`NICE_LEN_OPTIMAL` regardless (no
  case here reached 1.5x), but real, and paid on every position, not just
  ones near a genuine repeat. Also measured, unchanged from an earlier
  pass of this same slice: a `bench::long_range_repeat` case with its
  planted repeat moved out to `new_window - 4,096` (near the candidate
  ceiling itself, rather than S2-A63's 150,000-byte-past-`WINDOW` case),
  **-0.012543 b/B**, confirming the win still holds at the far edge of the
  candidate window. **Rejected**, applying the same rule S2-R3/S2-R4
  named explicitly: a validation regression fails corpus policy's accept
  rule independent of the net train number, and both sealed-only kinds
  regressed, consistently, not as noise — despite a net-improving train
  mean, itself driven by incidental recurrence rather than the structure
  the lead's own named targets (Silesia's `mozilla`/`nci`/`samba`/`sao`/
  `webster`) actually have. A blanket bump of the wired `WINDOW` costs
  the sealed set real bpb and every measured case real encode time to win
  only on content with recurrence the current window already can't see,
  which is exactly the trade a global constant cannot make conditionally.
  `research/progress.jsonl` it112. Remaining S1-P4 scope: growing
  `WINDOW` unconditionally is closed off by this result; what is left is
  either a content-adaptive trigger (grow the window only when a cheap
  pre-pass suggests recurrence past it exists, paying the encode-time
  cost only where the bpb win is real) or accepting the bpb/speed cost as
  a deliberate large-file mode distinct from the default (ROADMAP M5
  SPEED territory, not this lead's own scope) — neither designed nor
  measured here.

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
  any, is not a third variant of the same idea. Fourteenth slice, S2-R10:
  answered the differently-shaped question this entry itself posed —
  whether the match finder's *search quality* (`NICE_LEN_OPTIMAL`,
  `MAX_TREE_DEPTH_OPTIMAL`, both frozen at their issue-#179-speed-guard
  values since S2-A46/S2-A48 and never independently tested as ratio
  levers) rather than pricing was the unmoved target's real bottleneck.
  Rejected: raising either constant, alone or measured up to a 4,000,000-byte
  scale on `json_records`/`sqlite_like_records` directly, changed the
  compressed output by at most one byte, because neither bound ever binds
  on this target's own structure (fixed 20-byte rows, short inter-digit
  literal runs) — the parse already sees every candidate worth seeing at
  the current values. Full mechanism and numbers: S2-R10's own entry.
  Remaining S1-P2 scope: unclear, more so than before — both the
  intra-round pricing angle (S2-A51 through S2-R5) and now the
  match-finder search-quality angle (S2-R10) are closed on this evidence.
  What's left is either a modeling primitive that reaches structure a
  binary-tree parse cannot (e.g. a schema-aware/typed-field literal model,
  in the spirit of S1-P5's per-column direction but for pre-transpose or
  mixed-type records) or accepting that sqlite/json/jsonl residue sits
  near this architecture's ceiling absent one. Fifteenth slice, S2-A99:
  took that remaining scope's own first branch — a standalone primitive
  for a schema-aware/typed-field signal, the same "first slice, not yet
  wired" shape every other lead opened with (S2-A42, S2-A57, S2-A61,
  S2-A64). `fieldtype::FieldState`/`FieldClass`/`field_bank`: a rolling
  state machine over JSON-shaped syntax (quote-toggling with
  backslash-escape absorption, then digit/structural/whitespace/text
  classification of the byte outside any open string) that answers "what
  kind of field is this byte in" without the fixed stride
  `column::column_of` needs `transpose::encode` to have already imposed —
  the "for pre-transpose or mixed-type records" half of this entry's own
  framing, since JSON/JSONL/sqlite-dump row-major bytes carry no such
  stride. Standalone: proven only by 17 unit tests (a realistic JSON
  record byte-by-byte, escape/double-escape edge cases, the bank space
  bound), not measured against `bench::baseline` and not blended into
  `Literal`'s mix. Remaining S1-P2 scope, at the time: the ideal-cost
  pairing `S2-A69`'s methodology used for the column expert's own
  equivalent question — blend a field-class-keyed eighth expert into the
  shipped six under `sqlite_like_records`/`json_records` and the sealed
  set, before any real wiring — still unbuilt. Sixteenth slice, S2-R15:
  built and measured that pairing. Rejected: a sealed-only-kind regression
  (`gradient_image` +0.001598 b/B), despite train improving both named
  targets and the other sealed-only kind (`access_log`) also improving —
  numbers and mechanism in S2-R15's own entry. Remaining S1-P2 scope: back
  to unclear, the same shape this entry's own fourteenth-slice note
  already reached — both the pricing angles (intra-round and match-finder
  search-quality) and now the field-class modeling angle are closed on
  this evidence; what is left is either a different modeling primitive
  entirely or accepting that sqlite/json/jsonl residue sits near this
  architecture's ceiling absent one.
- S1-P3 | LEAD | PPM-style escape for literal contexts (see S1-R4). First
  slice: S2-A57 (standalone `Ppm` primitive, PPM Method C escape pricing,
  not yet wired), whose own module doc named three candidate fallback
  targets: order-0, one of `Literal`'s other five experts, or a fresh
  dedicated table. Second slice, S2-R6: measured the first (order-0, the
  mixer's one non-context-keyed bank) via an ideal-cost pairing, before
  committing to a real wiring. Rejected: net regression on
  `bench::baseline` (+0.0458 b/B train average, a severe `gradient_image`
  sealed regression), worst on data where a byte's likelihood genuinely
  depends on context — `markov_h8_2_trap` and every structured generator
  tested — which order-0's global marginal cannot represent; also ruled
  out the second candidate on the reasoning that a context-specific bank
  is exactly as likely to be sparse as whichever one is escaping. Third
  slice, S2-R16: measured the third and last named candidate, a fresh
  dedicated 16-context table keyed on the previous byte's high nibble,
  the same before-wiring ideal-cost-pairing methodology. Rejected too, on
  a different failure shape: train net *regressed* (+0.0348 b/B) even
  though both sealed-only kinds improved this time (`gradient_image`
  −0.1517, the exact case order-0 hurt worst), and S1-P3's own named
  target `sqlite_like_records` moved the wrong direction again, further
  than order-0's own miss. Mechanism: this fallback target genuinely
  fixes order-0's own blind spot (coarse, but not zero, context), but its
  own rescale-onto-bank-total step drifts a genuinely-unobserved symbol's
  floor away from 1 whenever a bank's total diverges from the fallback
  table's own (which ordinary training does, at different rates per
  expert), costing the most on exactly the fixed-record/short-period data
  this project's generators exist to probe. Full numbers and mechanism:
  S2-R16's own entry. Remaining scope, at the time: all three of
  `src/ppm.rs`'s own named fallback candidates were tried and rejected,
  the same "unclear, no further named branch" shape S1-P2 reached after
  its own repeated rejections; what was left was either a substitution
  rule that needs no cross-table rescaling, or accepting the ceiling.
  Fourth slice, S2-R17: built exactly that substitution rule, reviving
  `NibbleFallback` (S2-R16's own 16-context table) with a mechanism that
  never rescales a probability onto another table's total at all, instead
  of S2-R16's "convert to an effective frequency, rescaled onto the
  target bank's own total" rounding step. Rejected anyway, on nearly the
  same shape S2-R16 reached: train net regressed (+0.0278 b/B, 5
  improved, 6 regressed, the same split count as S2-R16), both
  sealed-only kinds improved, and the same three fixed-record/
  short-period generators (`interleaved_audio16`, `sqlite_like_records`,
  `x86_dense_code`) regressed worst. Full numbers and mechanism: S2-R17's
  own entry. Remaining scope: every substitution mechanism this doc's own
  module named, or S2-R17 could construct, rescale-based and
  rescale-free alike, now fails on the same data, for what looks like the
  same underlying reason: not a rounding artifact specific to one
  mechanism, but the six-expert mixer's own adaptive weights, tuned to
  expect an honest Laplace floor from a sparse expert, mishandling
  whatever a foreign substitute reports in its place instead (S2-R17's
  own entry names this the likely mechanism, itself untested). What is
  left, if anything, is a fallback that never enters the six-expert
  mix's per-expert floor at all: an escape event priced and blended as a
  separate, eighth expert-like signal, the same additive (not
  substitutive) architectural shape S1-P5's column expert used, or
  accepting that this lead's ceiling, absent one, sits where S1-P2's own
  repeated-rejection shape already landed. Fifth slice, S2-A100: built
  exactly that additive shape and it worked. `PpmExpertState`
  (`src/literal.rs`) wraps one `Ppm` table per bank, keyed the same
  coarse way `NibbleFallback` was (previous byte's high nibble, 16
  contexts, deliberately reused to isolate the mechanism question from
  the keying question S2-R16/S2-R17 already answered), plus its own
  mixing weight, blended into the mix as a genuinely new term via
  `Literal::mix_ppm`/`update_ppm_expert` — never written into any of the
  six real experts' own banks or totals, unlike every prior slice here.
  Measured the same before-wiring ideal-cost pairing S2-R16/S2-R17 used:
  train net **-0.005530 b/B** (8 of 11 cases improved; `markov_h8_2_trap`
  -0.038598, `interleaved_audio16` -0.007128, `x86_dense_code` -0.005872,
  `json_records` -0.003686 led; `entropy_ladder_h8` +0.000861,
  `base64_wrapped` +0.000031, and S1-P3's own named target
  `sqlite_like_records` +0.000944 were the three regressions, all an
  order of magnitude or more smaller than any regression in
  S2-R6/S2-R16/S2-R17). Sealed: `access_log` **-0.006145**,
  `gradient_image` **-0.176711** — both improved, `gradient_image` by the
  widest margin of any S1-P3 slice measured so far (the same case every
  substitution mechanism helped most when it helped at all, S2-R6/S2-R16/
  S2-R17). Corpus policy's accept rule (train improvement AND no
  validation regression) passes outright. **Accepted.** Mechanism: a
  `Ppm` bank starts every symbol at frequency 0, so on data where its
  16-context key carries no real signal (`entropy_ladder_h8`) it stays
  sparse and contributes near-zero mass everywhere rather than a
  confidently-wrong one (`JOURNAL` S2-R6's order-0 substitution and
  S2-R16/S2-R17's `NibbleFallback` substitution both went the other way:
  a Laplace-smoothed or rescaled substitute injects a specific, often
  wrong, non-zero value straight into an existing expert's own floor).
  More fundamentally, this slice never touches an existing expert's own
  reported estimate at all: the six real experts keep reporting their own
  honest numbers exactly as `Self::update` already calibrates their
  weights to expect, and the new expert's own weight is free to fall
  toward zero wherever it is unhelpful, the same "mixer discounts what
  doesn't help" mechanism `JOURNAL` S2-A69's column expert and S2-A76's
  SSE-survival follow-up both used to explain their own worst case
  merely breaking even. That is the structural difference from every
  rejected substitution in this lead: S2-R17's own diagnosis named the
  six-expert mixer's weights as "tuned to expect an honest Laplace floor
  from a sparse expert, mishandling whatever a foreign substitute reports
  in its place instead" — an additive slot never puts a foreign number in
  another expert's place, so that mishandling has nothing to act on.
  `research/progress.jsonl` it158. Remaining S1-P3 scope: the real
  wiring slice — a `Method`/`FORMAT_VERSION` bump and an ADR (hard rule
  5), decode support for every earlier version, and a real-bitstream
  measurement through `mothergod::compress`/`decode`, the same shape
  S1-P5's own `ADR-0046` took after S2-A69's pre-SSE accept — plus the
  still-open, deliberately deferred SSE-interaction question S2-A76 asked
  for the column expert (does this win survive `encode_sse`'s
  calibration stage): both untested for this expert.
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
  unchanged, so nothing currently encoded moves. Fifth slice, S2-R7:
  answered that decision, rejected — despite a net-improving train mean
  (-0.007797 b/B, driven by incidental multi-megabyte-scale recurrence,
  not the Silesia-shaped structure this lead targets), both real sealed-
  only kinds regressed (`access_log` +0.000189, `gradient_image`
  +0.000092), the same "any real validation regression fails accept, net
  number or not" rule S2-R3/S2-R4 applied, on top of a real encode-time
  cost (1.05x-1.41x across every case measured). Remaining S1-P4 scope: a
  blanket `WINDOW` bump is closed off;
  what is left is either a content-adaptive trigger (grow only when a
  cheap pre-pass suggests recurrence past the current window exists, so
  the encode-time cost is paid only where the bpb win is real) or a
  deliberate large-file mode distinct from the default (ROADMAP M5 SPEED
  territory). Sixth slice, S2-A89: built the content-adaptive trigger's own
  first piece — `lz::likely_benefits_from_larger_window`, a cheap sampled
  detector, standalone, not yet wired to `parse_optimal`'s window choice.
  Seventh slice, S2-R9: measured the wiring decision S2-A89 left open —
  gate the window bump on the detector's verdict instead of applying it
  unconditionally — and rejected it: the detector both misses the exact
  scenario this lead targets and still lets through part of the sealed
  regression it exists to prevent. Remaining S1-P4 scope: the detector's
  fixed-position sampling needs replacing with content-defined anchor
  selection before this trigger is trustworthy, or the large-file-mode
  alternative, either still open. Eighth slice, S2-A91: built that
  replacement, `lz::likely_benefits_from_larger_window_content_defined`,
  standalone; proved directly that it closes the alignment blind spot
  S2-R9 found (a planted repeat off the fixed-stride grid is missed by
  the old detector and found by the new one, same test) but left the
  other half of S2-R9's rejection — incidental-collision false positives
  on densely-sampled real data — untested and open. Remaining S1-P4
  scope: answer the false-positive question (a longer anchor, or
  requiring agreement past `DETECTOR_ANCHOR_LEN`) before either detector
  is worth gating a window bump on, or the large-file-mode alternative,
  still open. Ninth slice, S2-A92: answered that question — built
  `lz::likely_benefits_from_larger_window_content_defined_confirmed`,
  requiring `DETECTOR_CONFIRM_LEN` (24) bytes past each anchor to also
  agree byte-for-byte before reporting `true`, which the exact S2-R9
  divergent-tail shape now fails while a real extending recurrence still
  passes. Tenth slice, S2-A93: did that wiring — `lz::
  parse_optimal_adaptive_window`, gated by the confirmed detector between
  `WINDOW` and `ADAPTIVE_WINDOW` (`2^21 - 1`) — and found the regression
  S2-R9 blamed on the detector was actually `parse_optimal_with_window`'s
  own seed pass staying bound to `WINDOW` regardless of the DP's search
  window; binding both to the same chosen window turned every previously
  regressing gated case into an improvement, closing both of S2-R9's
  failure modes for real. Eleventh slice, S2-A94: built the real-bitstream
  counterpart to `ideal_cost_bits_with_window`/`ideal_cost_bits_adaptive_window`
  (`codec::compressed_len_with_window`/`compressed_len_adaptive_window`),
  still not wired to `encode_tokens` itself, so the next slice's
  real-bitstream measurement would not be flying blind on the
  ideal-vs-real coder gap ADR-0038 documents. Twelfth slice, S2-R11: used
  it to re-measure S2-A93's wiring decision for real, across every
  train-eligible kind this crate defines rather than S2-R9's own four —
  and rejected it: `Base64Wrapped(train)` regresses **+0.005874** b/B
  even though the confirmed detector fires on genuine, non-colliding far
  matches there, a third distinct failure mode neither S2-A91's nor
  S2-A92's fix addresses. Remaining S1-P4 scope: isolate why
  `Base64Wrapped`'s confirmed far matches net-lose before either
  tightening the detector with a fourth condition or accepting that
  closing this lead needs gating per-kind local-match density, not just
  a byte-content anchor; `parse_optimal_adaptive_window` stays unwired
  from `compress`/`encode`. A window past `2^21 - 1` itself still
  separately needs `OFFSET_BUCKETS`/`bucket()` widened and a
  `FORMAT_VERSION` bump before it is measurable at all, which still
  leaves the Silesia finals named above (several 10s of MiB) out of
  reach regardless of any of this. Thirteenth slice, S2-A95: isolated
  the mechanism directly — 484 of the 1,529 far matches (32%) displace
  an active `Token::Rep` at the same position, and 463 of those 484
  (96%) are a genuine length win over the rep they replaced, not a tie
  or a shorter trade, ruling out "same length, needlessly pricier" and
  ruling out detector error (S2-R11 already confirmed the matches are
  real, non-colliding recurrences). The real driver is
  `relax_match_candidate`'s fixed bucket-20 distance tax (20 raw bits
  plus header, paid identically 1 byte or 1 MiB past `WINDOW`) against
  `relax_rep_candidates`' near-zero marginal cost for the same-slot
  continuation it displaces: a longer match is not automatically a
  cheaper one once it has to pay that tax instead of reusing a live rep.
  Remaining S1-P4 scope: a gate or price adjustment that accounts for
  what a far match displaces (an active rep, in particular) rather than
  only whether the detector confirms a real recurrence, or the
  large-file-mode alternative, still open. Fourteenth slice, S2-A96:
  standalone capability for the next question this scope note raised —
  `codec::compressed_len_with_seed_and_search_window` exposes the seed
  and search windows as independent parameters, the real-bitstream
  counterpart `parse_optimal_adaptive_window`'s own matched-window
  contract does not let through. Fifteenth slice, S2-R12: used it to
  re-test whether S2-R9's own rejected seed/search mismatch (a possible
  self-reinforcement fix, not this scope note's own displacement
  question) explains S2-A95's tax — rejected, it recovers roughly 73% of
  `Base64Wrapped(train)`'s loss but regresses both sealed-only kinds and
  still leaves a genuine ~0.0016 b/B residual, closing the seed-window
  lever for this lead. Sixteenth slice, S2-A97: standalone primitive for
  this scope note's own displacement question — `lz::best_active_rep_len`
  factors the per-slot carry-aware rep scan `relax_rep_candidates`
  already runs into a reusable query, the longest active rep continuation
  at a position, that `relax_match_candidate`'s eventual displacement-aware
  pricing needs before it can weigh a far match's fixed bucket-20 tax
  against what it would cost to keep extending a live rep instead; not
  yet called from `dp_round`. Remaining S1-P4 scope: wire it into
  `relax_match_candidate` behind a threshold (how much of a length edge a
  far match needs over the rep it displaces to earn its tax) and measure
  against S2-R12's own residual, or accept the large-file-mode
  alternative if no threshold clears every prior slice's per-case bar.
  Seventeenth slice, S2-R13: did that wiring and swept the threshold —
  rejected, `base64_wrapped`'s regression barely moved across the swept
  range while `access_log`'s already-accepted win eroded well before
  that; a length-only displacement gate cannot separate the two.
  Remaining S1-P4 scope: the residual needs a signal neither the
  seed-window lever nor a length-based gate used, plausibly how many
  future positions the displaced rep slot would still have served.
  Eighteenth slice, S2-A98: tested exactly that signal across every
  case this lead has an accept/reject verdict on — falsified, future
  service left on the table tracks no consistent relationship to
  accept/reject status (`access_log`'s accepted win leaves the least on
  the table, `json_records`'s accepted win leaves the most).
  Nineteenth slice, S2-R14: spent that last untried pricing angle:
  amortizing a fresh match's distance tax across the near-future reuses
  of the distance it introduces. Rejected, and hard: every cell of a
  4x3 parameter grid regressed every case, train mean +0.144495 b/B at
  the primary point and still +0.085804 at the gentlest, monotone in
  the discount. A token-mix diagnostic named the mechanism. The
  rebate double-counts a saving `relax_rep_candidates` already books
  where it actually occurs, so the DP buys mispriced fresh matches by
  spending the rep tokens that justified the discount (`json_records`
  match tokens 2.9x, rep-covered bytes 33,343 -> 19,621). That result
  also reframes the lead: S2-A95's asymmetry is correct accounting, not
  a defect, so the four slices since have been hunting a bug in a
  number that was never wrong. Remaining S1-P4 scope: no pricing or
  gating signal proposed for this lead since S2-R11 survives contact.
  What is left is the large-file-mode alternative (ROADMAP M5
  territory, a deliberate mode distinct from the default rather than a
  parse heuristic), or accepting that far-match residue on this data is
  not a pricing problem at all.
- S1-P5 | RESOLVED 2026-09-17, closed by S2-A79 (`FORMAT_VERSION` 4,
  ADR-0046) | Per-column modeling after transpose (filter-aware coder,
  OpenZL direction). Target: sao. First slice: S2-A64 (standalone
  `column::column_of`, not yet wired). Second slice, S2-A66: `column::
  column_bank`, wrapping `column_of`'s unbounded result into a fixed-size
  bank space so a future expert's storage sizes from a constant rather
  than the frame's declared `columns` (CLAUDE.md hard rule 2). Third
  slice, S2-R8: measured whether a per-column-bank model would even beat
  the shipped mixer before spending the wiring slice — rejected, a
  column-keyed model *replacing* `Literal` loses more from discarding
  order-1/SSE adjacency than a column-boundary signal alone gains.
  Remaining scope, at the time: whether column identity helps as one
  *more* expert blended alongside the existing six, not a replacement for
  them, is still untested — that needs the actual column-index-keyed
  expert bank in `Literal`, threading the `columns` parameter filter
  selection already knows down to it, and a `FORMAT_VERSION` bump.
  Fourth slice, S2-A69: answered that question via the same
  before-wiring ideal-cost pairing methodology S2-R6/S2-R8 used, this
  time blending a column-keyed seventh expert *into* the shipped mix
  (`Literal::ideal_cost_bits_column_expert_pair`) instead of replacing
  it, sharing the six real experts' one adaptation trajectory
  ([`Literal::ideal_cost_bits`] runs unmodified inside the paired call)
  and letting the seventh expert's own bank and single mixing weight
  adapt on an independent trajectory. Deliberately scoped pre-SSE: this
  measures the raw seven-expert mix, not its interaction with the
  `FORMAT_VERSION` 3 SSE calibration stage (S1-P1), a separable question
  for the real wiring slice. Accepted, at this pre-wiring, pre-SSE
  measurement layer: train improved on both cases, sealed did not
  regress (numbers in S2-A69's own entry). Fifth slice, S2-A76: answered
  the SSE-interaction question this entry named as remaining scope —
  accepted, net train improved and sealed did not regress, but the win is
  smaller than the pre-SSE numbers suggested and uneven across cases: SSE
  calibration already recovers part of what the column signal was worth
  (`sqlite_like_records`'s pre-SSE −0.032494 shrinks to −0.012452
  post-SSE), and fully absorbs it for `interleaved_audio16` (pre-SSE
  −0.021819 flips to a +0.000604 wash post-SSE); `gradient_image`
  (sealed) improved more post-SSE (−0.005562 vs pre-SSE's −0.001672).
  Remaining S1-P5 scope: the real wiring itself (`FORMAT_VERSION` bump,
  threading `columns` from filter selection into `Literal`) and a
  real-bitstream measurement to confirm this ideal-cost signal survives
  contact with the actual coder — expect a real-bitstream win closer to
  S2-A76's post-SSE numbers than S2-A69's pre-SSE ones, and no
  improvement from audio-shaped column structure specifically.
- S2-A69 | ACCEPTED | Fourth slice of ROADMAP M3's fifth standing lead
  (S1-P5, per-column modeling after transpose): before spending the real
  `Literal`/`Method`/`FORMAT_VERSION` wiring slice S2-A64/S2-A66 left as
  remaining scope, and after S2-R8 falsified "column identity alone beats
  the shipped mixer" (not "column identity, blended in as a seventh
  expert alongside the other six, ever helps"), measured the narrower
  hypothesis S2-R8's own entry named. Hypothesis: blending a
  `column::column_bank(column::column_of(position, columns, len),
  max_banks)`-keyed expert into `Literal`'s six-expert mix as a seventh,
  rather than replacing it, reduces ideal-cost bits/byte on
  already-transposed column-structured data without regressing the
  sealed case. New `Literal::ideal_cost_bits_column_expert_pair`: prices
  each literal byte twice from the same pre-update six-expert state —
  once as `Self::ideal_cost_bits` exactly (shared trajectory, including
  its own `update` call, so the six real experts adapt identically to
  production regardless of this method ever running), once with a new
  `ColumnExpertState`'s bank blended in via a seven-wide fixed-point mix
  mirroring `mix`'s own shape. `ColumnExpertState` adapts on its own
  trajectory: its bank observes the byte the same `rescale_bank` free
  function (extracted from `Literal::update`'s previously inline rescale
  loop, behavior-preserving, existing tests unchanged) uses for the five
  default-rate real experts, and its one mixing weight adapts via the
  same continuous-probability-space exponentiated-gradient rule
  `Literal::update` uses, restricted to this one component, never written
  back into the six real weights. `codec::
  ideal_cost_bits_column_expert_experiment` pairs this against the shared
  flag/length/offset/slot costs the same way `CostSink` prices them, so
  any delta is attributable to the literal model alone. Deliberately
  pre-SSE (measures the raw seven-expert mix, not its interaction with
  `FORMAT_VERSION` 3's SSE calibration stage): a separable question left
  for the real wiring slice, not this one. | Measured (model-cost, not
  real-bitstream; `bench` crate generators; git revision at time of run):
  train, the same rotated-window shape S2-R8 used (150,000 generated
  bytes, 50,000-byte window at offset 50,000, train seed
  `0xC01D_BEEF_1234_5678`) — `sqlite_like_records` (columns=20): baseline
  3.285756 -> with-column 3.253262 bpb, **-0.032494**. `interleaved_audio16`
  (columns=2): baseline 5.796946 -> with-column 5.775128 bpb,
  **-0.021819**. Sealed: `gradient_image` (columns=200, `sealed_seed` of
  the same train seed, not rotated, 50,000 bytes): baseline 6.069871 ->
  with-column 6.068199 bpb, **-0.001672** (an improvement, not a
  regression). Every case improved; corpus policy's accept rule (train
  improvement AND no validation regression) passes on both halves.
  Baseline numbers here are not directly comparable to S2-R8's own quoted
  baseline (that measurement went through the SSE-calibrated whole-codec
  `codec::ideal_cost_bits`; this one is deliberately pre-SSE, per the
  scoping note above) — same order of magnitude (sqlite_like_records
  3.285756 here vs 3.266457 there), which is a sanity check on the
  harness, not a claim of equivalence. | Mechanism: unlike S2-R8's
  histogram-only replacement, this seventh expert never displaces the
  six real experts' order-1/SSE-adjacent evidence — it only adds one more
  signal the exponentiated-gradient mixer is free to downweight if it is
  not useful at a given context. The gradient_image case, which paid the
  worst tax in S2-R8's replacement (+2.075129 bpb), essentially breaks
  even here (-0.001672), consistent with "the column-boundary signal is
  real but narrow" — small enough to add for free once it does not have
  to compete for the whole prediction, not large enough to move a case
  dominated by within-column order-1 drift. Candidate code (`ColumnExpertState`,
  `Literal::ideal_cost_bits_column_expert_pair`, the `rescale_bank`
  extraction, `codec::ideal_cost_bits_column_expert_experiment`, their
  unit tests) kept: this is an accepted step at the ideal-cost-pairing
  layer, the same status S2-A58 held for S1-P1 before its own SSE wiring
  landed. The manual driver
  (`bench/src/bin/scratch_column_expert_experiment.rs`) that ran this
  measurement is not: deleted after recording these numbers, same as
  every prior scratch driver regardless of verdict, since the numbers it
  produced are now this entry, not a rerunnable tool. `research/
  progress.jsonl` it117. Remaining S1-P5 scope: see the updated S1-P5
  entry above.
- S2-A76 | ACCEPTED | Fifth slice of ROADMAP M3's fifth standing lead
  (S1-P5, per-column modeling after transpose): S2-A69's own deliberately
  deferred question, whether the seventh column-keyed expert's win
  survives once the mixed probability is calibrated by
  `Literal::encode_sse`'s SSE stage instead of priced directly, the
  refinement every real byte already pays under `FORMAT_VERSION` 3.
  Hypothesis: blending `column_state`'s bank into the seven-expert mix
  and calibrating the result through the same bittree/SSE decomposition
  `Literal::encode_sse` uses reduces ideal-cost bits/byte on
  already-transposed column-structured data without regressing the
  sealed case, the same accept bar S2-A69 cleared pre-SSE. New
  `Literal::ideal_cost_bits_column_expert_pair_sse`: prices `byte` twice
  through the SSE-calibrated path — once as `Self::ideal_cost_bits_sse`
  exactly (so `self.sse`'s one real trajectory adapts identically to
  production regardless of this method running), once with the same
  seven-expert `cum` table `ideal_cost_bits_column_expert_pair` builds
  (factored out as `Self::mix7`, shared by both, behavior-preserving),
  calibrated through `bittree::ideal_cost_bits_sse` against a new
  independent `Sse` table on `ColumnExpertState` — its own trajectory,
  never `self.sse`'s, so probing the with-column path never perturbs the
  six-expert model's real calibration state. The column expert's own
  weight/bank adaptation (`Self::update_column_expert`, the other half of
  the same extraction) is unchanged from S2-A69, shared verbatim by both
  pairing methods. `codec::ideal_cost_bits_column_expert_experiment_sse`
  mirrors `ideal_cost_bits_column_expert_experiment`'s flag/length/
  offset/slot pass-through, differing only in routing the literal price
  through the SSE-paired method. | Measured (model-cost, not
  real-bitstream; `bench` crate generators; same rotated-window shape,
  train seed `0xC01D_BEEF_1234_5678`, and generators S2-A69 used, this
  time transposing each window with `filters::transpose::encode` before
  measuring — S2-A69's own "already-transposed column-structured data"
  precondition, made explicit here): train, `sqlite_like_records`
  (columns=20): baseline (SSE) 3.266457 -> with-column (SSE) 3.254005
  bpb, **-0.012452** (baseline reproduces S2-R8's own SSE-calibrated
  baseline number exactly, a harness sanity check).
  `interleaved_audio16` (columns=2): baseline 5.765049 -> with-column
  5.765653 bpb, **+0.000604**, a wash inside noise. Sealed:
  `gradient_image` (columns=200, `sealed_seed` of the same train seed,
  not rotated): baseline 5.916617 -> with-column 5.911055 bpb,
  **-0.005562** (an improvement, larger than S2-A69's pre-SSE
  -0.001672). Net train (mean of the two cases): **-0.005924**. Corpus
  policy's accept rule (train improvement AND no validation regression)
  passes: net train improved, sealed improved, the same "individual
  cases may mix, net and sealed decide" reading S1-P1's own accept
  applied to its one in-trade regression. | Mechanism: SSE calibrates the
  six-expert mix's own blended probability at each binary-tree node
  using only tree-position context (`bittree::sse_context`), not column
  identity — it corrects *systematic* bias in that probability, and some
  of the bias a missing column signal causes is systematic enough for
  SSE to partially correct without ever seeing the column index
  directly. That explains why the pre-SSE win shrinks rather than
  vanishing on `sqlite_like_records` (a real column-boundary effect SSE
  cannot fully substitute for) and why it fully vanishes on
  `interleaved_audio16` (a two-column low/high-byte split apparently is
  exactly the kind of bias SSE's own calibration already captures).
  `gradient_image` improving *more* post-SSE than pre-SSE suggests SSE
  and the column signal interact favorably there rather than competing —
  not explained further here, a candidate thread for a future slice, not
  this one. Candidate code (`ColumnExpertState`'s new `sse` field,
  `Literal::mix7`, `Literal::update_column_expert`,
  `Literal::ideal_cost_bits_column_expert_pair_sse`,
  `codec::ideal_cost_bits_column_expert_experiment_sse`, their unit
  tests) kept, same status as S2-A69's own candidate code: an accepted
  step at the ideal-cost-pairing layer. `Literal::mix7`/
  `update_column_expert` also replace `ideal_cost_bits_column_expert_pair`'s
  previously-inline blend and weight-update logic, behavior-preserving
  (its own existing unit tests pass unchanged). The manual driver
  (`bench/src/bin/scratch_column_expert_sse_experiment.rs`) that ran this
  measurement is not: deleted after recording these numbers, same as
  every prior scratch driver regardless of verdict. `research/
  progress.jsonl` it125. Remaining S1-P5 scope: see the updated S1-P5
  entry above.
- S2-A77 | ACCEPTED | S1-P6's own first slice (issue #447), the
  `Literal::mix` rebuild S2-A27/S2-A67/S2-A68 already named as the
  mechanism behind decode's floor violation. Hypothesis: `mix`'s single
  fused loop over `symbol` computes two things at once — a per-symbol
  multiply-add across the six experts (independent across symbols) and
  the running `cum` prefix sum (a genuine loop-carried dependency) — and
  the second forces the optimizer to serialize the first too. Splitting
  the loop into pass one (per-symbol multiply-add into a plain `[u64;
  ALPHABET]`, no cross-symbol dependency) and pass two (the same
  `(mixed >> 16) + 1` prefix sum, now over already-computed values) frees
  pass one to autovectorize without changing a single computed value:
  for a fixed symbol, pass one still sums the same six terms in the same
  expert order (0..EXPERTS) the old fused loop did, and pass two performs
  the identical running sum. Bit-for-bit identical output by
  construction, not by testing alone — an encoder-only-shaped argument
  that happens to apply to a decode-side hot loop too, so no
  `FORMAT_VERSION` bump or golden fixture update (hard rule 5). | Measured:
  `baseline_gate check` — 11 cases, no regression, bits/byte identical
  on every case (a pure speed change). Decode throughput,
  `crate::codec::decode` on 8 MiB of `test_support::Xorshift32`
  pseudo-random bytes forced through `Method::Lz` via
  `crate::codec::encode` directly (the same incompressible-data/forced-Lz
  shape `MAX_DECODED_LEN`'s doc comment already used for its own 1780
  ns/byte figure), this run's sandbox, two runs each side: before
  1818.0/1820.9 ns/byte (mean 1819.5), after 1554.7/1594.5 ns/byte (mean
  1574.6) — a 13.5% reduction, about 1.16×. Canterbury's held-out finals
  (`docs/benchmarks/canterbury.md`, real bitstreams): not regenerated by
  this slice, since bits/byte identity is already established by
  `baseline_gate check` above; the last committed regeneration (as of
  2026-09-01T09:32:52Z) has `xargs.1` (the file S2-A67 first flagged
  under the floor) at 0.596 MB/s decode, still under the 1 MB/s floor —
  cited as the standing figure this slice narrows the gap against, not
  a claim this slice re-measured it. | Mechanism: full argument lives in
  `Literal::mix`'s own
  updated doc comment (`src/literal.rs`), so it stays in one place
  instead of drifting between there and here. `Literal::decode`'s
  O(ALPHABET) linear symbol-scan is untouched, a smaller and separate
  bottleneck (at most 256 comparisons per byte against `mix`'s 1536
  multiply-add-shift operations) — left for a later slice if it's ever
  worth its own measurement. | Remaining S1-P6 scope: this slice alone
  does not reach the floor. Bit-decomposed coding already shipped
  (S2-A58/S2-A59, wired as SSE at S2-A60, `FORMAT_VERSION` 3) without
  removing the per-byte rebuild this slice targets — `bittree` is
  handed the same 257-entry `cum` `mix` still rebuilds from scratch, per
  issue #447's curator note (2026-09-03). The lead's remaining open
  directions: the incremental cumulative structure (S1-P6's original
  ~10× target, not yet spent), tANS fast path (~100×), explicit AVX2
  blend (~1.5×). Issue #447 stays open, updated with these numbers.
  `research/progress.jsonl` it126.
- S2-A78 | REJECTED | S1-P6's remaining incremental-cumulative-structure
  direction (issue #447, following S2-A77's autovectorization slice).
  Hypothesis: a per-bank order-statistics structure (a Fenwick/
  binary-indexed tree over each of the six experts' 256-entry frequency
  tables) could let `mix` answer the handful of point queries
  `bittree::walk_sse` actually reads (one per tree level, `LEVELS` = 8)
  in `O(EXPERTS * log ALPHABET)` instead of today's full `O(EXPERTS *
  ALPHABET)` array build, leaving every coded bit identical to today's
  output. No candidate was implemented: the blocking fact is an exact
  arithmetic identity, not a corpus-dependent outcome, so no train/val
  measurement applies (kind `wild`, same shape as it68's decode-time
  characterization). | Mechanism: `mix` computes `cum[symbol + 1] =
  cum[symbol] + (mixed[symbol] >> 16) + 1`, a per-symbol floor division
  of the combined six-expert sum, applied before the running total ever
  sees the next symbol. Answering `cum` at one position needs the sum of
  `(mixed[i] >> 16) + 1` over every `i` below it, and floor does not
  distribute over addition: `mixed = [3*65536 - 1, 2]` gives per-symbol
  floors `2, 0` (sum 2), but `(196607 + 2) >> 16 = 3`, a whole unit off
  (verified directly, not asserted). A Fenwick tree over each bank's raw
  (pre-shift) frequencies answers `O(log n)` range-sum queries against
  `mixed` itself, not against `cum`'s floored, per-symbol-summed form —
  getting the latter still means visiting every `i` below the query
  position individually, the same `O(ALPHABET)` work `mix` already does.
  The one reformulation that does answer in `O(log ALPHABET)` — floor
  the aggregate once instead of every symbol, `cum(position) =
  (prefix_sum_mixed(position) >> 16) + position` — computes a materially
  different number per symbol than today's `mix`, a bitstream change
  (`FORMAT_VERSION` bump, golden fixtures, a real ratio re-measurement
  per hard rule 5), not the same-output speed change this lead's
  remaining budget was scoped for. Separately, and independent of the
  floor problem: `banks()` re-derives a context-hash-keyed bank per
  expert every call, and `update()` re-adapts every mixing weight every
  call (S1-A4's context-sensitive weights, exponentiated gradient), so
  the six-bank *combination* `mix` blends essentially never repeats from
  one literal to the next — only the six single-bank raw-frequency
  tables persist meaningfully across calls, and those alone cannot
  answer the per-symbol-floor query below the `O(ALPHABET)` bound just
  shown. | The incremental-cumulative direction closes here as
  infeasible without a `FORMAT_VERSION` bump; S1-P6's remaining scope
  narrows to the tANS fast path and the explicit AVX2 blend.
  `research/progress.jsonl` it127.
- S2-A79 | ACCEPTED | S1-P5's real-wiring slice, closing the lead
  (`FORMAT_VERSION` 4, ADR-0046): `Literal::encode_column`/`decode_column`
  code a `Candidate::Transpose` frame's literals through the seven-expert
  mix (`Literal::mix7`) instead of the six-expert `encode_sse`/`decode_sse`,
  reusing `Literal::update`/`update_column_expert` verbatim rather than a
  new coupled update (the six real experts adapt exactly as `encode_sse`
  leaves them, `ColumnExpertState` adapts separately against the seven-way
  mixed estimate that was actually coded — the same order S2-A76's ideal-
  cost pairing already measured, not a new design). `codec::decode` reads
  its already-parsed filter selector alongside the declared version to
  pick the path; every other candidate is byte-for-byte unchanged at
  version 4. | Measured, real bitstreams (`mothergod::compress`, this run's
  sandbox): `bench/baseline.json`'s 11 fixed train-tier cases show no
  regression (`baseline_gate check` passes); 10 of 11 are byte-identical
  (none select `Candidate::Transpose` in the real encoder trial at these
  fixed seeds/lengths). `entropy_ladder_h6` does select
  `Candidate::Transpose(96)` on both the unpatched and patched build (a
  `filters::select::pick` entropy-margin artifact on iid noise, unrelated
  to this slice's target shape) and moves 6.179200 -> 6.178240 bpb
  (**-0.000960**), two orders of magnitude inside `TOLERANCE_BITS` (0.02);
  not written back to `bench/baseline.json` (see ADR-0046's Consequences
  for why). Two purpose-built train/sealed pairs (synthetic fixed-width
  tabular data, each column cycling through its own period with ~20% of
  bytes jittered off the clean pattern so the literal model carries real
  weight, not just LZ repeats — both real encoder trials reduce to
  `Candidate::Transpose(96)`): 8-column shape, train 8464 -> 8298 bytes
  (**-0.055333 bpb**), sealed 8400 -> 8240 bytes (**-0.053333 bpb**);
  20-column shape, train 22556 -> 22344 bytes (**-0.028267 bpb**), sealed
  22643 -> 22432 bytes (**-0.028134 bpb**). All four improve; corpus
  policy's accept rule (train improvement, no validation regression)
  passes on both pairs. Sealed-only `gradient_image`/`access_log`: both
  select `Candidate::Identity` on both codec versions at this generator's
  default parameters, byte-identical — `filters::select::TRANSPOSE_COLUMNS`'s
  fixed candidate list does not include `gradient_image`'s true 200-column
  width, so this slice's real corpus impact is narrower than S1-P5's
  original "target: sao" framing hoped; widening that candidate list is
  separate scope (a `filters::select` heuristic question), not reopened
  here. | Mechanism: full argument in ADR-0046's Decision section, not
  duplicated here. New golden fixture `tests/golden/v4-tabular-columns`
  (same plaintext as `v3-tabular-columns`, re-encoded); that pair stays
  committed, decode-only, forever. `research/progress.jsonl` it128.
- S2-A80 | REJECTED | S1-P6's remaining explicit-AVX2-blend direction
  (issue #447), following S2-A77/S2-A78. Hypothesis: hand-written AVX2
  intrinsics for `mix`'s per-symbol multiply-add pass (the pass S2-A77
  split out for autovectorization) could beat the compiler's own
  vectorization by controlling lane width and instruction selection
  directly, with output staying bit-for-bit identical (integer
  multiply-add-then-sum over `u64` is associative regardless of lane
  order, no precision loss). No candidate implemented: every
  `std::arch` SIMD intrinsic is an `unsafe fn`, and `src/lib.rs:4`/
  `src/bin/mothergod.rs:1` carry `#![forbid(unsafe_code)]` crate-wide —
  a documented quality-boundary choice (ADR-0017), load-bearing for the
  weekly Miri lane's guarantee that no undefined behavior reaches the
  decode path (ADR-0043) — and `forbid`, unlike `deny`, cannot be
  locally lifted by a nested `#[allow(unsafe_code)]`; verified against
  `rustc`'s own lint semantics, not asserted. The one route to SIMD
  that stays in safe code, `std::simd` (portable SIMD), is nightly-only
  and unavailable here too: `rust-toolchain.toml` pins `channel =
  "stable"`. | Mechanism: this is a policy conflict, not a
  corpus-dependent outcome (kind `wild`, same shape as S2-A78/it68: no
  candidate to keep or delete, nothing to measure). Lifting
  `forbid(unsafe_code)` is a quality-boundary change squarely in
  ADR-0017's own territory, not a call this slice makes on its own
  authority. | S1-P6's remaining scope narrows to the tANS fast path
  alone; explicit AVX2 blend is closed until a future ADR revisits
  `forbid(unsafe_code)`. `research/progress.jsonl` it129.
- S2-A81 | ACCEPTED | First slice of S1-P6's remaining tANS fast path
  (issue #447, after S2-A80 closed AVX2): `src/tans.rs`'s
  `normalize_frequencies`, a standalone, not-yet-wired primitive every
  tANS/FSE-family coder needs before it can build encode or decode
  tables — rescaling raw symbol counts onto a power-of-two total
  (`1 << table_log2`) so a future coder's state transform can use
  shifts and masks instead of division. Largest-remainder rounding:
  floor each symbol's ideal share (bumping a nonzero count's floor-to-
  zero up to 1, since nothing downstream could ever code a symbol at
  frequency 0), then settle the exact total by adding to or removing
  from the entries whose rounding was least faithful to their true
  share first, ranked by `(counts[i] * table_target) % total` and tied
  by ascending index, this slice's own tie-break choice for determinism.
  No archive precedent (grepped `research/imports/session-1/mothergod.rs`
  clean of any ANS-family code, same check S2-A57/S2-A64 ran for their
  own leads). Review (PR #576) measured the surplus-removal branch
  O(nonzero.len() * surplus) on a shape with one dominant entry and
  the rest pinned at the forced-nonzero floor (750ms at 20,000 distinct
  symbols): the round-robin loop re-scanned every already-exhausted
  entry once per single-slot removal. Rewritten to a single forward
  pass over the same remainder-sorted order, taking each entry's full
  headroom before advancing instead of one slot per revisit — O(n log n)
  overall (the sort dominates), 4.2s -> well under 200ms on that shape.
  | 12 unit tests: sum equals the target total across 6 distinct
  count/`table_log2` shapes, every originally-nonzero symbol keeps a
  nonzero share, zero entries stay zero, exact-power-of-two counts
  pass through unchanged, a dominant symbol claims most of the table,
  determinism (same input twice), the `distinct == target` edge
  (forces every entry to exactly 1), `u32::MAX`-scale counts do not
  overflow, the dominant-outlier surplus-removal shape stays under a
  200ms bound, and two panics (`table_log2 >= 32`, more distinct
  symbols than table slots); `cargo x check` (fmt, clippy pedantic +
  missing_docs, test, doc) clean. | No bpb measurement: this primitive
  has no coder around it yet to produce a bitstream, same reason
  S2-A42/S2-A57/S2-A64
  gave for their own first slices — `progress.jsonl` records this as
  `kind: "patch"` with null bpb deltas; `baseline_gate check` confirms
  the existing 11 cases are unaffected (nothing wired). Remaining S1-P6
  scope: the encode/decode state-transform tables built from a
  normalized frequency table (the "spread" step), the coder's state
  machine itself, wiring behind a new fast `Method` variant, and the
  `FORMAT_VERSION` bump and real-bitstream measurement that wiring
  needs.
- S2-A82 | ACCEPTED | Second slice of S1-P6's remaining tANS fast path
  (issue #447, after S2-A81's `normalize_frequencies`): `src/tans.rs`'s
  `spread_symbols`, the classic FSE/tANS "spread" step — assigning each
  of a `1 << table_log2`-slot table to exactly one symbol from a
  normalized frequency table, symbol `s` taking exactly `freq[s]` slots.
  Scattered by a fixed stride (`spread_stride`, private) instead of
  packed contiguously per symbol: any stride odd relative to the
  power-of-two table size is coprime to it, so its additive orbit mod
  `table_size` visits every slot exactly once before repeating, the only
  property placement correctness needs. Magnitude borrowed from FSE's
  own constant (half the table plus an eighth plus three) for the
  scattering quality real FSE/tANS decode tables rely on, with the low
  bit forced on: the bare constant lands on `table_size` itself (hence
  even) at `table_size == 8`, which would collapse every placement onto
  slot 0. No archive precedent, same check S2-A81 ran. | 9 unit tests:
  two panics (`table_log2 >= 32`, a `freq` that does not sum to
  `1 << table_log2`), a hand-computed 4-slot table, per-symbol placement
  counts across the same 6 shapes S2-A81's sum-total test used, an
  independent reimplementation of the stride walk asserting no slot is
  ever written twice and every slot is written once (a per-symbol-count
  check alone cannot rule out a slot collision that happens to land on
  the default fill value, a real symbol id), determinism, a single-slot
  table, and the `table_size == 8` even-constant edge; `cargo x check`
  (fmt, clippy pedantic + missing_docs, test, doc) clean. | No bpb
  measurement, same reason as S2-A81: no coder yet to produce a
  bitstream — `progress.jsonl` records this as `kind: "patch"` with null
  bpb deltas; `baseline_gate check` confirms the existing 11 cases are
  unaffected (nothing wired). Remaining S1-P6 scope: the encode/decode
  transition tables built from a spread assignment (state deltas and
  next-state arithmetic), the coder's state machine itself, wiring
  behind a new fast `Method` variant, and the `FORMAT_VERSION` bump and
  real-bitstream measurement that wiring needs.
- S2-A83 | ACCEPTED | Third slice of S1-P6's remaining tANS fast path
  (issue #447, after S2-A82's `spread_symbols`): `src/tans.rs`'s
  `build_decode_table`, the classic FSE/tANS decode-table construction
  step — turning a spread assignment into the per-slot entries a real
  decoder indexes by state (which symbol, how many bits to read next,
  and the baseline those bits are added to for the next state).
  `FSE_buildDTable`'s own construction, no archive precedent, same check
  S2-A81/S2-A82 ran: each symbol's occurrences in the spread table,
  visited in slot order, are numbered consecutively starting at that
  symbol's own normalized frequency (the state range a canonical tANS
  table reserves for it), and a given occurrence number's bit count and
  baseline fall straight out of that number's highest set bit
  (`table_log2` minus the bit position for the count, that many bits
  shifted in and `1 << table_log2` subtracted back off for the
  baseline). A private `highbit32` helper wraps `u32::ilog2` (clippy's
  `manual_ilog2` caught a hand-rolled `leading_zeros` reimplementation in
  one of this slice's own tests during review, applied to both). | 12
  unit tests: three panics (`table_log2 >= 32`, a `freq`/`table_symbol`
  length mismatch, an out-of-range symbol id in `table_symbol`), a
  hand-computed 4-slot table (worked by hand against the construction
  formula), the decode table's symbol column matching the spread table
  it was built from exactly, bit-count/baseline range bounds across the
  same 6 shapes S2-A81/S2-A82's tests used, an independent cross-check
  recomputing each occurrence's expected bit count and baseline from its
  position among that symbol's own occurrences rather than reusing the
  function's own running counter, determinism, and a single-symbol
  whole-table case (needs 0 bits per decode and is an identity map
  slot-to-slot, the trivial-probability-1 sanity check); `cargo x check`
  (fmt, clippy pedantic + missing_docs, test, doc) clean. | No bpb
  measurement, same reason as S2-A81/S2-A82: no coder yet to produce a
  bitstream — `progress.jsonl` records this as `kind: "patch"` with null
  bpb deltas; `baseline_gate check` confirms the existing 11 cases are
  unaffected (nothing wired). Remaining S1-P6 scope: the mirroring
  encode-table construction, the coder's actual read/write state machine
  over both tables, wiring behind a new fast `Method` variant, and the
  `FORMAT_VERSION` bump and real-bitstream measurement that wiring
  needs.
- S2-A84 | ACCEPTED | Fourth slice of S1-P6's remaining tANS fast path
  (issue #447, after S2-A83's `build_decode_table`): `src/tans.rs`'s
  `build_encode_table`, the mirroring encode-table construction
  S2-A83 named as remaining scope. Inverts `build_decode_table`'s own
  per-symbol occurrence numbering rather than recomputing it: a decoder
  answers "slot `p` decodes to which symbol, at which occurrence"; an
  encoder already knows the symbol it is about to emit and needs the
  opposite lookup, "symbol `s`'s occurrence `n` sits at which slot," so
  this function makes one pass over the same spread table
  `build_decode_table` walks and records each symbol's occurrences (in
  slot order, which is also ascending order since the pass is a single
  forward scan) into a per-symbol `Vec<u32>` of slot positions, instead
  of a per-slot struct. `FSE`'s own real encode tables (`symbolTT`,
  `deltaFindState`/`deltaNbBits`) fold this lookup into a division-free
  closed form; this slice keeps the plain table because nothing yet
  reads it inside a hot loop to make that optimization pay for itself —
  the coder's state machine (remaining scope below) is what would tell
  whether the closed form is worth the extra complexity, not a decision
  to make ahead of having one. | 12 unit tests: three panics
  (`table_log2 >= 32`, a `freq`/`table_symbol` length mismatch, an
  out-of-range symbol id), a hand-computed 4-slot table (the same
  worked example `build_decode_table`'s own hand-computed test uses,
  read in the opposite direction), each symbol's row length matching
  its `freq` entry, rows sorted ascending by construction, an
  independent cross-check filtering the spread table for each symbol
  and comparing the resulting slot list directly (not reusing
  `build_encode_table`'s own scan), a round-trip property test feeding
  every `(symbol, slot)` pair the encode table names back through
  `build_decode_table` and asserting the decoded symbol matches,
  determinism, and a zero-count symbol getting an empty row rather than
  a missing one or a panic; `cargo x check` (fmt, clippy pedantic +
  missing_docs, test, doc) clean. | No bpb measurement, same reason as
  S2-A81 through S2-A83: no coder yet to produce a bitstream —
  `progress.jsonl` records this as `kind: "patch"` with null bpb
  deltas; `baseline_gate check` confirms the existing 11 cases are
  unaffected (nothing wired). Remaining S1-P6 scope: the coder's actual
  read/write state machine over both tables, wiring behind a new fast
  `Method` variant, and the `FORMAT_VERSION` bump and real-bitstream
  measurement that wiring needs.
- S2-A85 | ACCEPTED | Sixth slice of S1-P6's remaining tANS fast path
  (issue #447, after S2-A84's `build_encode_table`): `src/tans.rs`'s
  `encode_symbol`, `encode_message`, `decode_message` — the coder's
  actual read/write state machine over both tables. `decode_message`
  needs no new algorithm: `DecodeSlot` already fully specifies a decode
  step (index by state, read `nb_bits` bits, add `new_state_base`), so
  it is a loop over that plus real bit reads. Encoding has no closed
  form yet (`build_encode_table`'s own docs deferred one, "nothing yet
  reads it inside a hot loop to make that optimization pay for itself"):
  `encode_symbol` instead searches `encode_table[symbol]`'s occurrences
  for the one whose decode range covers the target state, relying on
  the property that an FSE/tANS decode table's per-symbol occurrence
  ranges partition `[0, table_size)` exactly and contiguously (an
  `encode_symbol_is_the_exact_inverse_of_a_decode_step` test walks every
  slot's every admissible bits value and confirms the search recovers
  it exactly). `encode_message` threads `encode_symbol` backward over a
  whole symbol sequence — reverse order, the direction a stack-like ANS
  state actually threads through — collecting bits in encode order,
  reversing once, then packing into a real byte buffer (a private
  `BitWriter`, LSB-first) that `decode_message`'s counterpart
  (`BitReader`) reads forward from byte 0; no existing bit-I/O
  abstraction in the crate fit (`coder.rs`'s is an arithmetic-coding
  range coder, not a plain bit packer), so both are new and scoped to
  this module. | 15 unit tests (`BitWriter`/`BitReader` round trip and
  bounds panic, `encode_symbol` panics/hand-computed example/exact
  inverse-of-decode-step property, `encode_message`/`decode_message`
  round trip including the single-symbol-table and empty-message edges,
  an out-of-range `initial_state` panic) plus a
  `encode_decode_message_round_trips_for_arbitrary_symbol_sequences`
  proptest over arbitrary alphabets, table sizes, and symbol streams (a
  separate `#[cfg(not(miri))] mod proptests`, mirroring `coder.rs`'s own
  layout and its reasoning for excluding Miri: interpretation cost
  multiplied by the case count, with the deterministic examples already
  walking the same paths for UB observation); `cargo x check` clean. |
  No bpb measurement, same reason as S2-A81 through S2-A84: no `Method`
  wiring yet, so no real bitstream to measure — `progress.jsonl` records
  this as `kind: "patch"` with null bpb deltas; `baseline_gate check`
  confirms the existing 11 cases are unaffected (nothing wired). This
  closes the "read/write state machine" item. Remaining S1-P6 scope:
  wiring this coder behind a new fast `Method` variant, and the
  `FORMAT_VERSION` bump and real-bitstream measurement that wiring
  needs.
- S2-A86 | ACCEPTED | S1-P6's remaining tANS fast path (issue #447, after
  S2-A85's `encode_symbol`/`encode_message`/`decode_message`): the first
  real measurement of `src/tans.rs`'s standalone primitives, still
  without any `Method` wiring or `FORMAT_VERSION` change. A new
  `bench/src/bin/tans_measure.rs` binary builds an order-0 static tANS
  coder per buffer (`table_log2=10`: `normalize_frequencies`,
  `spread_symbols`, `build_decode_table`, `build_encode_table`,
  `encode_message`, `decode_message`), asserts an exact round trip, and
  compares its bits/byte against `order0_entropy_bits` (the theoretical
  floor) and against `mothergod::compress`/`decompress`'s (`Method::Lz`,
  the champion) real bits/byte and MB/s on the identical buffer — no
  network fetch, four `mothergod_bench` generator buffers at 200,000
  bytes, seed `0xC0FFEE123456789A` (S2-A1's own convention):
  `entropy_ladder(h=2)`, `entropy_ladder(h=6)`, `markov_h8_2_trap`,
  `access_log`. | Real numbers (`cargo run -p mothergod-bench --release
  --bin tans_measure`): tANS lands within 0.0005–0.2546 bits of the
  entropy floor on every buffer (`entropy_ladder(h=2)`: 2.0058 vs 1.9980;
  `entropy_ladder(h=6)`: 6.2504 vs 5.9958, the widest gap, `table_log2=10`
  granularity coarsening at a flatter/higher-entropy distribution;
  `markov_h8_2_trap`: 8.0012 vs 7.9982; `access_log`: 4.7910 vs 4.7905) —
  the primitives are a tight, efficient order-0 coder, not merely a
  correct one. Against the champion, tANS wins only on pure iid noise
  (`entropy_ladder(h=2)`: 2.0058 vs 2.3432 bpb, the champion's
  context-mixing overhead costing it on data with no context to mix) and
  loses everywhere order-1+ structure exists (`entropy_ladder(h=6)`:
  6.2504 vs 6.1345; `markov_h8_2_trap`: 8.0012 vs 2.3432, the trap's
  whole point — order-1 correlation no order-0 histogram can see;
  `access_log`: 4.7910 vs 0.7951, LZ matches an order-0 coder cannot
  exploit) — expected for an order-0 coder measured against a
  context-mixing one, not a competition. Speed is the actual finding: on
  the same buffers, tANS's `encode_message`/`decode_message` ran
  23×–342× faster encode and 21×–305× faster decode than
  `mothergod::compress`/`decompress` in every single case (e.g.
  `access_log`: 26.29 vs 0.53 MB/s encode, 213.79 vs 10.12 MB/s decode).
  | 5 focused unit tests (histogram counting, a small-fixed-buffer round
  trip, the MB/s arithmetic, a single-symbol buffer's near-zero bpb) plus
  a round-trip assertion inside the measurement itself for every
  generated buffer; `cargo x check`: 4 stages green; `cargo clippy
  --all-targets -- --deny warnings` clean; `cargo doc --no-deps` clean;
  `baseline_gate check`: 11 cases, no regression (nothing wired,
  unaffected). No bpb measurement against the champion in
  `progress.jsonl`: this characterizes a different tool for a different
  tier, not a champion-replacement candidate, same reason S2-A81 through
  S2-A85 used null deltas; `research/progress.jsonl` it135. Verdict on
  continuing toward the wiring slice: numbers support it. tANS sits
  within noise of its own theoretical floor and runs one to two orders of
  magnitude faster than the champion on every buffer measured, exactly
  the ratio-for-speed trade a `level -1` fast path needs — but only as an
  additional low tier alongside the champion, never a replacement: the
  ratio gap on any data with order-1+ structure (the common case) is
  large and structural, not a bug to fix. Remaining S1-P6 scope
  unchanged: `Method` wiring, the `FORMAT_VERSION` bump, and a real-
  bitstream sealed-validation measurement, still open (issue #447).
- S2-A87 | ACCEPTED | S1-P6's remaining tANS fast path (issue #447, after
  S2-A86's measurement): the piece every prior slice left implicit by
  threading `freq` straight from `normalize_frequencies` into
  `spread_symbols` inside one process. A decoder has no access to the
  original byte counts `normalize_frequencies` was computed from, so a
  real bitstream needs the normalized frequency table itself on the
  wire. `src/tans.rs` gained `write_freq_table`/`read_freq_table`
  (varint `table_log2`, varint alphabet length, then one varint per
  entry) plus the private `write_varint_u32`/`read_varint_u32` pair they
  ride on. `read_freq_table` is this coder's first byte-level contact
  with untrusted input, so it is fully validating, not merely correct:
  it rejects a `table_log2` past a new `MAX_TABLE_LOG2` (16, generous
  headroom past S2-A86's `table_log2=10`, capping the eventual decode
  table's slot count regardless of how small the coded payload is), an
  alphabet length past the bytes actually remaining (bounding the
  returned `Vec`'s allocation by the input's own size, never by the
  untrusted length field alone), a malformed varint (an overlong
  encoding past the 5 bytes a `u32` ever needs, or one that overflows
  `u32`), and a table whose entries do not sum to exactly
  `1 << table_log2`. Still standalone: nothing calls either function
  outside this module's own tests. | 19 unit tests (varint round trips
  and rejections, freq-table round trips, and one rejection per
  validation clause above, including a truncate-at-every-prefix sweep
  that only pins "never `Ok`, never a panic" since which error variant a
  given cut lands on depends on exactly where it falls) plus a proptest
  extending S2-A85's own `freq_table_log2_and_symbols` strategy to cover
  `write_freq_table`/`read_freq_table` round trips over arbitrary
  alphabets and table sizes; `cargo x check`: 4 stages green; `baseline_gate
  check`: 11 cases, no regression (nothing wired, unaffected). Remaining
  S1-P6 scope unchanged: `Method` wiring, the `FORMAT_VERSION` bump, and
  a real-bitstream sealed-validation measurement, still open (issue
  #447).
- S2-A88 | ACCEPTED | S1-P6's remaining tANS fast path (issue #447, after
  S2-A87's `write_freq_table`/`read_freq_table`): the "real-bitstream
  sealed-validation measurement" S2-A87 left as remaining scope, still
  without wiring anything to a `Method`. S2-A86 measured 4 hand-picked
  buffers and used `access_log`'s raw generator seed directly even though
  `DatasetKind::AccessLog` is sealed-only (`research/corpus/POLICY.md`,
  "held-out seeds AND held-out dataset kinds") — a policy gap, not just
  thin coverage. `bench/src/bin/tans_measure.rs` now covers all 9
  `DatasetKind`s (the entropy ladder at two points bracketing the class,
  one buffer per remaining kind), every sealed-only kind (`AccessLog`,
  `GradientImage`) generated at `sealed_seed` of the shared seed instead
  of the seed directly, and charges `write_freq_table`'s serialized bytes
  into the tANS frame size, not just `encode_message`'s bare output — a
  real decoder needs that table on the wire (S2-A87), so leaving it
  uncharged would understate every case's real cost. Each measurement now
  also reports `tans_would_win`: whether `tans_frame_len <
  champion_bytes.len()`, the literal question a `Method`-wiring decision
  needs, not a bits/byte comparison in the abstract. | Real numbers
  (`cargo run -p mothergod-bench --release --bin tans_measure`, 200,000-byte
  samples, `table_log2=10`): tANS-as-third-candidate would win on exactly
  **1 of 10** buffers — `entropy_ladder(h=2)` (2.0162 vs champion 2.3432
  bpb, tANS's own frame-size accounting now including its ~258-byte fixed
  freq-table overhead, one varint per byte-alphabet slot regardless of how
  few symbols actually occur). Every other buffer loses, including two
  kinds S2-A86 never measured (`sqlite_like_records`: 6.1234 vs 3.4004;
  `x86_dense_code`: 5.7047 vs 2.5534) and the two sealed-only kinds under
  their policy-correct seed (`access_log`: 4.8003 vs 0.7960; `gradient_image`:
  7.9075 vs 5.7550). A run of one repeated byte (a case wiring would also
  need to survive) does not flip either: the champion's match-based
  redundancy elimination crushes it far below tANS's fixed table overhead.
  | 2 new unit tests pinning the win/lose computation itself as a
  regression guard (`tans_wins_over_the_champion_on_skewed_iid_noise`,
  `tans_does_not_win_over_the_champion_on_a_run_of_one_byte`), 1 new test
  confirming every `DatasetKind` is represented in `cases()`, the existing
  entropy-floor test's buffer size raised (1,000 → 50,000 bytes) since the
  freq-table charge added above meant 1,000 bytes no longer amortized it
  to the sub-0.5-bpb bar that test asserts; `cargo x check`: 4 stages
  green; `baseline_gate check`: 11 cases, no regression (nothing wired,
  unaffected). | Verdict on the wiring question S2-A86 left open: **park
  it.** A 1/10 win rate, on a class (skewed-but-not-flat iid byte
  distributions) narrow enough that `compress`'s existing `Stored`/`Lz`
  choice already handles it acceptably (2.3432 bpb, not 8), does not
  justify a `FORMAT_VERSION` bump, an ADR, and a permanent new public
  `Method` variant (SIMPLICITY: public API surface grows for a case that
  loses on 9 of 10 realistic buffers). The "zstd-class -1 mode" the LEAD
  entry below names is a different, larger design than what S2-A81
  through S2-A87 built: zstd's own fast levels keep LZ matching and swap
  only the entropy stage, where every slice here built a whole-buffer
  order-0 coder with no match stage at all — S2-A86's `access_log` result
  (4.79 vs 0.80 bpb, a 6× ratio loss) is LZ's matches disappearing
  entirely, not an entropy-coder gap tANS could close. A genuine fast tier
  needs tANS threaded into the LZ token stream's literal coding, not a
  parallel whole-frame `Method`; that is a materially different, bigger
  integration this issue's remaining checklist item ("Method wiring")
  undersold. Recorded here so the next session does not re-measure the
  same question: standalone tANS-as-automatic-candidate is closed,
  negative; the LZ-preserving fast-literal-stage direction is untried and
  is S1-P6's actual remaining scope.
- S2-A90 | REJECTED | S1-P6's remaining LZ-preserving fast-literal-stage
  direction (issue #447, after S2-A88): before building the integration
  (`Method` wiring, `FORMAT_VERSION` bump, decoder support) S2-A88 flagged
  as materially bigger than any prior slice, measure whether it is even
  bit-cost-plausible by pricing only the literal-byte stream a real
  optimal parse produces, so the redundancy LZ's matches already remove
  is out of the comparison on both sides. | New
  `bench/src/bin/tans_literal_measure.rs`: for each of `tans_measure`'s 10
  buffers (mothergod_bench generators, 200,000-byte samples, seed
  0xC0FFEE123456789A, sealed-only kinds at `sealed_seed`), runs
  `lz::parse_optimal`, walks the tokens exactly as `codec`'s private
  `walk_tokens` does (`Context::after_literal`/`after_copy`, same order),
  and splits the walk into the literal-byte stream plus the champion's
  real per-literal cost (`Literal::ideal_cost_bits_sse`, the `encode_sse`
  path every non-`Transpose` candidate actually codes literals through).
  An order-0 static tANS coder (`normalize_frequencies` through
  `encode_message`, `table_log2=10`) then codes that same literal-byte
  stream, freq-table bytes charged in (S2-A87/S2-A88's accounting). Real
  numbers: tANS loses on **10 of 10** buffers, not just most — even
  `entropy_ladder(h=2)`, the one buffer where whole-buffer tANS won in
  S2-A88, loses here (2.7727 vs 2.7374 bits/literal-byte) because the
  champion's context state (word-hash bank, SSE calibration) keeps
  adapting across a match boundary while an order-0 coder starts fresh
  every buffer. The loss is small in whole-buffer terms on the four
  kinds where literals are a small fraction of the parse (`access_log`
  6.3% literal: +0.0256 bits/byte of the whole buffer; `json_records`
  4.5%: +0.0187; `base64_wrapped` 3.8%: +0.0709; `entropy_ladder(h=2)`
  17.6%: +0.0062) but severe on the kinds a genuine fast tier would most
  need to survive, where literals dominate the parse: `gradient_image`
  95.5% literal, +2.1217 bits/byte of the whole buffer; `interleaved_
  audio16` 89.8%, +1.5386; `entropy_ladder(h=6)` 93.1%, +0.2141;
  `markov_h8_2_trap` only 13.6% literal but the champion's context model
  crushes this trap by design (3.1938 bits/literal-byte) while an
  order-0 coder cannot see past its flat marginal histogram at all
  (8.0403), the widest per-literal gap measured (+4.8466). 4 unit tests
  (literal extraction preserves order and byte-for-byte content on an
  all-literal buffer, a long single-byte run leaves only a handful of
  leading literals, the empty-stream `None` case, a round-trip-verified
  positive measurement on a real buffer) plus the existing
  `cases_cover_every_dataset_kind` shape from `tans_measure`; `cargo x
  check`: 4 stages green; `baseline_gate check`: 11 cases, no regression
  (nothing wired, unaffected). | Closes the LZ-preserving fast-literal-
  stage direction, negative: an order-0 static coder loses to the
  context-mixing champion on every buffer tested, catastrophically on
  literal-heavy data, because the champion's context keeps adapting
  through match boundaries in a way a fresh-per-buffer order-0 table
  cannot match regardless of how few bytes it prices. This was S1-P6's
  last open branch that respects the crate's own constraints (S2-A78
  closed the incremental-cumulative direction as `FORMAT_VERSION`-bump-
  class, S2-A80 closed explicit AVX2 as blocked by `forbid(unsafe_code)`
  independent of measurement). S1-P6 has no further small-slice avenue
  left unmeasured; reopening it needs either a genuinely new idea (an
  adaptive/context-conditioned fast coder, which reintroduces the
  per-byte adaptation cost this lead exists to amortize away) or lifting
  `forbid(unsafe_code)` (ADR-0017's call, not this lead's). CHANGELOG.md
  untouched: nothing wired to a `Method`, no public API or user-visible
  behavior changed.
- S1-P6 | LEAD | Speed tier: bit-decomposed coding (LPAQ-style, ~10×), tANS
  fast path (~100×, zstd-class -1 mode). Concrete target as of S2-A27:
  `Literal::decode`'s all-literal worst case measures ~1170 ns/byte
  (~854 KB/s), under the ROADMAP SPEED floor (≥1 MB/s decode) —
  `Literal::mix` rebuilds all 256 cumulative entries from scratch every
  byte instead of an incremental structure. S2-A67/S2-A68 later measured
  the same floor violation on real corpus files (Canterbury's `xargs.1`
  0.775 MB/s, Silesia's `x-ray` 0.392 MB/s and `sao` 0.428 MB/s), a
  finding that sat unlinked until issue #447 gave it a queue entry.
  S2-A77 took the first slice: splitting `mix`'s fused loop cut decode's
  measured ns/byte by about 13.5% on this sandbox (`xargs.1` still under
  the floor after it), autovectorization-shaped, not an incremental
  structure. Bit-decomposed coding above already shipped (S2-A58/S2-A59/
  S2-A60, `FORMAT_VERSION` 3) without reaching the floor, since `bittree`
  is handed the same from-scratch `cum`. S2-A78 closed the incremental-
  cumulative direction as infeasible without a `FORMAT_VERSION` bump
  (`mix`'s per-symbol floor does not distribute over a prefix sum, and
  the six-expert combination itself never repeats call to call). S2-A80
  closed the explicit-AVX2-blend direction too, blocked by the crate's
  own `forbid(unsafe_code)` (ADR-0017/ADR-0043) independent of any
  corpus measurement; the tANS fast path is the sole remaining open
  scope. S2-A81 took tANS's own first slice: `src/tans.rs`'s
  `normalize_frequencies`, standalone and not yet wired. S2-A82 took the
  next slice, the "spread" step (`spread_symbols`), also standalone and
  not yet wired. S2-A83 took the decode-table construction step
  (`build_decode_table`), also standalone and not yet wired. S2-A84 took
  the mirroring encode-table construction step (`build_encode_table`),
  also standalone and not yet wired. S2-A85 took the coder's actual
  read/write state machine over both tables (`encode_symbol`,
  `encode_message`, `decode_message`), standalone over a real (if
  scratch) byte buffer and still not wired to any `Method`. S2-A86 then
  measured the whole standalone coder for real without wiring it to
  anything: within 0.0005–0.2546 bits of the order-0 entropy floor on
  every buffer tested (`bench/src/bin/tans_measure.rs`), 23×–342× faster
  encode and 21×–305× faster decode than the champion
  (`mothergod::compress`/`decompress`) on the same buffers, losing on
  ratio wherever order-1+ structure exists (expected, not a defect).
  S2-A88 then ran the real-bitstream sealed-validation measurement this
  entry's own "remaining scope" line used to name: full 9-`DatasetKind`
  coverage, policy-correct sealed seeding, freq-table bytes charged into
  the frame size. Verdict: **park whole-buffer tANS-as-automatic-`Method`-
  candidate**, closed negative — it would win on only 1 of 10 realistic
  buffers, a class `compress`'s existing `Stored`/`Lz` choice already
  handles acceptably, not worth a `FORMAT_VERSION` bump and a permanent
  public API addition. This lead's own "zstd-class -1 mode" framing named
  a different design than what got built: zstd's fast levels keep LZ
  matching and swap only the entropy stage, where S2-A81 through S2-A87
  built a whole-buffer order-0 coder with no match stage, which is why
  S2-A86's structured-data losses were so large (LZ's matches vanishing
  entirely, not an entropy-coder gap). S2-A90 then measured that
  remaining branch directly, pricing only the literal-byte stream a real
  optimal parse produces (so LZ's own redundancy removal is out of the
  comparison on both sides): an order-0 static tANS coder loses to the
  champion on 10 of 10 buffers, catastrophically on literal-dominated
  ones (`gradient_image` 95.5% literal: +2.12 bits/byte of the whole
  buffer), because the champion's context state keeps adapting across
  match boundaries in a way a fresh-per-buffer order-0 table cannot
  match. Closed, negative. S1-P6 has no further small-slice avenue left
  that respects the crate's own constraints: bit-decomposed coding
  shipped without reaching the floor, the incremental-cumulative
  direction is `FORMAT_VERSION`-bump-class, explicit AVX2 is blocked by
  `forbid(unsafe_code)` (ADR-0017), and both tANS directions (whole-
  buffer automatic candidate, LZ-preserving literal-stage swap) lose on
  bits/byte. Reopening this lead needs a genuinely new idea, not a next
  slice of what is already here.
- S1-P7 | RESOLVED 2026-09-01, closed by it124/ADR-0041 | Production
  hardening: streaming mode, frozen format spec v1. The fuzzing half landed: targets S2-A25, scheduled CI
  S2-A53, remaining fuzz scope named in S2-A53. First slice toward the
  streaming-mode half: S2-A70 (a precondition, not the streaming API
  itself). First slice of the bounded-memory-decode half, a separate
  thread from S2-A70's ring-buffer precondition: S2-A71
  (`mothergod::decompress_bounded`, a caller-configurable ceiling below
  `codec::MAX_DECODED_LEN`, not the streaming/block API itself either).
  Streaming mode and frozen format spec v1 remain untouched; streaming
  mode's ring-buffer design also depends on how filter-undo interacts
  with a bounded output buffer, a second obstacle beyond S2-A70's LZ-loop
  precondition, per S2-D4 — decided 2026-08-30: scope the ring-buffer
  decoder to `Identity`/`Delta`/`Bcj` frames, `Transpose` keeps today's
  whole-buffer path, no `transpose` redesign required. The decision's own
  prerequisite (a public predicate so a caller can reason about the split,
  not a silent fallback) landed as S2-A72. Scoping the ring-buffer decoder
  itself before writing it found it is not decomposable into a small
  standalone primitive the way S2-A70/S2-A71/S2-A72 were: S2-D5. S2-D5's
  combined sink-abstraction-plus-real-caller slice, narrowed to
  `Candidate::Identity` (the one candidate needing no filter-chunking work
  first), landed as S2-A73: `mothergod::decompress_to_writer`. `Delta`'s
  turn, S2-A74: `filters::delta::Undo` gives the delta transform its own
  incremental decode form, wired into the same streaming path Identity
  uses. `Bcj`'s turn, S2-A75: `filters::bcj::Undo` gives the bcj transform
  its own incremental decode form (lookahead, not lookback, unlike
  `Delta`), wired into the same streaming path. Streaming-mode scope is now
  closed for every candidate but `Transpose`, excluded by design per S2-D4.
  Remaining S1-P7 scope, closed here: the frozen format spec v1 half.
  ADR-0041 rules on issue #422's two questions — the format is ready to
  freeze now (a live research lead never gates it, since a model-class
  change lands as a new `FORMAT_VERSION` under hard rule 5 regardless),
  and freezing means a stability commitment at the current version, not
  reverting the wire byte to 1. `docs/format/SPEC.md`'s status flips from
  "DRAFT/unstable" to "stable, frozen": every version it documents (2, 3)
  decodes forever, and CLAUDE.md hard rule 5's "unless an ADR drops one"
  carve-out is retired for those versions (version 1's retirement,
  ADR-0026/ADR-0028, predates the freeze and stands). No `FORMAT_VERSION`
  bump, no code change — a policy commitment about future ADRs, not a
  wire change. Both halves of this lead are now closed.
- S2-A70 | ACCEPTED | `codec::decode` now rejects a match distance beyond
  `lz::WINDOW`, not just beyond the output written so far
  (ROADMAP M4's bounded-memory decode guarantee, a precondition for
  S1-P7's streaming-mode half). `OFFSET_BUCKETS` (21) lets the offset
  model's bucket-20 residual bits represent a decoded value up to
  `2 * WINDOW - 1`; the encoder's match finder (`insert_and_find`) never
  searches past `WINDOW`, so any real bitstream's distance already fits,
  but nothing in `copy_checked`'s `distance <= output.len()` check ever
  enforced the tighter bound. Left unrejected, an adversarial payload
  could force this decoder to keep output far past what any real
  bitstream needs to reference — the actual blocker for ever making
  decode's memory O(window) instead of O(declared_len): a future
  ring-buffer decoder physically cannot satisfy a request the wire
  format didn't already guarantee stays in-window. Checked once, at the
  match-token's own distance decode (before `RepCache::push_front`), not
  again on the rep path: `RepCache::initial()`'s defaults ([1, 4, 8]) and
  every value `push_front`/`promote` ever place in it originate from a
  match distance that already passed this check, so the invariant holds
  by construction. | No bpb measurement: rejects input no real encoder
  produces, so `bench::baseline`'s 11 cases are unchanged (`baseline_gate
  check`: no regression, finals reports fresh); `research/progress.jsonl`
  records this as `kind: "patch"` with null deltas, it118. | New unit
  test `match_distance_beyond_window_is_rejected`, hand-crafted the same
  way `bad_match_distance_is_rejected_not_panicking` already is: a single
  match token coded through a real `Encoder`, distance one past `WINDOW`,
  asserting `Error::Corrupt`. `cargo x check`: 4 stages green. Remaining
  S1-P7 streaming-mode scope: the ring-buffer decoder itself (replacing
  `output: Vec<u8>` with a fixed `WINDOW`-sized buffer and a sink for
  bytes that fall out of it) and the frozen format spec v1 half, both
  untouched by this slice.
- S2-A71 | ACCEPTED | First slice of ROADMAP M3's seventh standing lead
  (S1-P7, production hardening: streaming mode, frozen format spec v1)
  addressing the bounded-memory-decode half by a different mechanism than
  S2-A70's ring-buffer precondition: `mothergod::decompress_bounded(input,
  max_len)`, a new public function alongside the unchanged
  `mothergod::decompress`, lets a caller supply its own output-size
  ceiling instead of always accepting `codec::MAX_DECODED_LEN`'s 256 MiB.
  `max_len` is clamped to `MAX_DECODED_LEN`, never raised past it: that
  constant is the only ceiling this decoder's worst-case decode time has
  been measured against (S2-A27), so a caller can tighten its own memory
  budget but cannot use this function to relax the safety-tested bound.
  `codec::decode` gained the same `max_len: u32` parameter, replacing its
  internal use of the `MAX_DECODED_LEN` constant directly (`decode`'s
  sole external call site, `decompress_bounded`, passes the
  already-clamped value); its one internal call site inside
  `mothergod::decompress` is now a thin wrapper calling `decompress_bounded`
  with `codec::MAX_DECODED_LEN`, so existing behavior is unchanged
  bit-for-bit. `Method::Stored`, which `codec::decode` never sees
  (`decompress`/`decompress_bounded` handle it directly, since it has no
  declared-length field of its own), gained its own `max_len` check
  against the payload's own byte count, but only when a caller's `max_len`
  is strictly tighter than `MAX_DECODED_LEN`: unlike `Method::Lz`'s
  declared-length field, a stored payload's length is read straight from
  `input` and is never spoofable past it, so `MAX_DECODED_LEN` itself buys
  it no safety margin, only a compatibility break — review caught a first
  version that bounded it unconditionally, which would have made
  `decompress` reject any incompressible input at or past 256 MiB that
  round-tripped on `main`, violating hard rule 1 for a real, reachable
  input class (PR #377 review thread). No `FORMAT_VERSION` bump and no ADR:
  `docs/format/SPEC.md` already names this ceiling "a decoder policy, not
  a wire-format field," and this slice only makes that policy
  caller-configurable within the one value already proven safe, changing
  no bit on the wire. A second review round caught two more gaps in the
  same PR: `Error::TooLarge(u32)`'s `Display` impl still named
  `codec::MAX_DECODED_LEN` unconditionally as "this decoder's maximum,"
  false the moment `decompress_bounded` rejects at a caller's tighter
  bound instead; and `codec::decode`'s `max_len` parameter went
  unclamped internally, relying entirely on `decompress_bounded` (its
  only in-crate caller) to have clamped first, a guarantee that lived
  only in a doc comment for a `#[doc(hidden)]`-but-`pub` function any
  dependent crate can call directly. Fixed by widening `Error::TooLarge`
  to `{ len: u32, max: u32 }` (source-level break, pre-1.0, no
  `FORMAT_VERSION` implication) so the message names the bound actually
  violated, and by adding `let max_len = max_len.min(MAX_DECODED_LEN);`
  as `decode`'s first line, a no-op at its one sanctioned call site and
  a real guarantee for any other. | 7 new unit tests (`codec.rs`'s
  `caller_supplied_max_len_below_max_decoded_len_is_honored`,
  `decode_clamps_a_max_len_above_max_decoded_len` (the second round's
  finding); `lib.rs`'s
  `decompress_matches_decompress_bounded_at_the_max`,
  `decompress_bounded_rejects_an_lz_frame_over_its_own_tighter_bound`,
  `decompress_bounded_rejects_a_stored_frame_over_its_own_tighter_bound`,
  `decompress_bounded_clamps_a_max_len_above_max_decoded_len`,
  `decompress_roundtrips_a_stored_payload_past_max_decoded_len` (the
  first round's boundary finding, built via `build_frame` directly
  rather than `compress` to avoid running the LZ encoder over 256+ MiB),
  plus every existing `codec::decode` call site and `Error::TooLarge`
  construction updated for the new parameter/shape, including S2-A70's
  own new `match_distance_beyond_window_is_rejected` test landed just
  ahead of this slice); the bench crate's
  `baseline_gate check` still reports the existing 11 cases with no
  regression and both finals reports fresh, confirming this change
  touches no encoded bitstream. `cargo x check`: 4 stages green. | No bpb
  measurement: `research/progress.jsonl` records this as `kind: "patch"`
  with null deltas, it119. Remaining S1-P7 scope: the streaming/block API
  itself (this slice is bounded-memory-decode only, still a single-shot
  whole-buffer call, same as S2-A70's ring-buffer precondition being
  unwired) and the frozen format spec v1.
- S2-D4 | DEBT | Issue #380: streaming decode (S1-P7's streaming-mode
  half, ROADMAP M4) is blocked by more than the LZ token loop. `codec::
  decode` calls `undo_filter(candidate, output)` once, after the token
  loop finishes, over the complete buffer. S2-A70's `WINDOW` guard makes
  the token loop itself close to streamable — nothing before
  `output.len() - WINDOW` can ever be referenced again — but
  `undo_filter`'s four candidates aren't uniformly streaming-compatible:
  `filters::delta::decode` and `filters::bcj::decode` are sequential with
  small fixed lookback (a stride, or the 5-byte call/jmp instruction),
  but `filters::transpose::decode` writes `out[i]` scattered across the
  *entire* buffer in column-major order (`for start in 0..columns { ...
  out[i] = data[pos]; i += columns }`) — output isn't produced in address
  order even though the total length is known upfront, so a sequential
  `Write` sink can't consume it incrementally; it needs either full
  buffering or a seekable sink. `Candidate::Transpose` isn't a rare
  corner case: `filters::select::pick` shortlists it whenever a probe's
  column-wise order-1 entropy beats identity/delta/BCJ (fixed-record-width
  data, `JOURNAL` S1-A2's x-ray dataset), so a streaming design that only
  handles the LZ loop would silently stop bounding memory whenever the
  encoder picked `Transpose` — a leaky guarantee a caller can't reason
  about, since filter choice is an internal encoder heuristic. Named fix,
  either: (1) redesign `transpose` to decode against a seekable/blocked
  sink, or (2) scope true streaming decode to `Identity`/`Delta`/`Bcj`
  frames only, falling back to today's whole-buffer path for `Transpose`
  and documenting the split precisely in the public API. No code changed;
  investigation only, while scoping S1-P7's top-of-list slice before
  committing to one.
  **Decision (2026-08-30): (2).** Redesigning `transpose` around a
  seekable or blocked sink (option 1) makes every future streaming caller
  either provide seekable output or accept the block-size/complexity cost
  of a second decode strategy for one filter, to buy a guarantee real
  frames may not even need — `filters::select::pick` chooses `Transpose`
  only when it wins probe entropy, so most frames already qualify for
  streaming under option 2 with no redesign. Option 2 also decomposes:
  the ring-buffer decoder (S2-A70's named remaining piece) can be built
  and tested against `Identity`/`Delta`/`Bcj` alone, with `Transpose`
  unchanged, rather than gating that work on a `transpose` rewrite first.
  The public streaming API MUST surface this split explicitly (e.g. a
  queryable "does this frame decode incrementally" predicate derived from
  the frame's stored `Candidate`, not a silent fallback) so a caller can
  reason about its own memory bound; no code implements this yet. |
  Remaining S1-P7 streaming-mode scope, decision made, implementation
  still open: the ring-buffer decoder itself, scoped to
  `Identity`/`Delta`/`Bcj` per the decision above, `Transpose` frames
  keeping today's whole-buffer `undo_filter` path.
- S2-A72 | ACCEPTED | S2-D4's own named prerequisite: `mothergod::
  decodes_incrementally(input)`, a new public function that reads a
  frame's header and (for `Method::Lz`) its 2-byte filter selector, and
  reports whether the frame's filter choice is one the future ring-buffer
  decoder will handle (`Identity`/`Delta`/`Bcj`) or not (`Transpose`) —
  without decoding any of the payload. Ported no archive behavior; this
  predicate has no precedent, since the archive never bounded decode
  memory at all. `Method::Stored` always answers `true`: it has no filter
  step, so nothing about it depends on the split S2-D4 decided. Shares its
  header parsing with `decompress_bounded` via a new private `parse_header`
  helper (`MAGIC`/`FORMAT_VERSION`/`Method` dispatch, previously inlined
  in `decompress_bounded` alone), so the two functions' idea of a
  well-formed header cannot drift apart. No `FORMAT_VERSION` bump: reads
  an existing wire field, writes nothing, changes no bit any encoder
  produces. | 6 new unit tests (`lib.rs`): a `Method::Stored` frame, each
  of `Identity`/`Delta`/`Bcj` hand-built via a new `lz_frame_header` test
  helper (which also de-duplicated two existing tests' identical inline
  header-building), `Transpose`, a malformed filter selector
  (`Error::Corrupt`), a payload truncated before the filter selector
  (`Error::Truncated`), and a bad-magic frame proving the shared
  `parse_header` errors propagate. `cargo x check`: 4 stages green,
  240 lib tests (up from 234); `baseline_gate check`: 11 cases, no
  regression, both finals reports fresh — no codec or bitstream path
  touched. | No bpb measurement: `research/progress.jsonl` records this
  as `kind: "patch"` with null deltas, it120. Remaining S1-P7
  streaming-mode scope, unchanged: the ring-buffer decoder itself.
- S2-D5 | DEBT | Scoped S1-P7's named remaining piece (the ring-buffer
  decoder itself) before writing it, the same "primitive first" order
  S2-A64/S2-A70/S2-A71/S2-A72 each took for their own leads. It does not
  decompose the same way. Built a standalone `Window` type in `src/lz.rs`
  (fixed-capacity ring buffer over `WINDOW` bytes, tracking an absolute
  write position, handing each byte to a caller-supplied sink the instant
  it is produced so nothing is retained past what a future match/rep
  distance could still reference) plus differential unit tests against
  `copy_checked`'s overlapping-run semantics (distance shorter than
  length, the max-distance boundary case, before-the-start rejection,
  many wraps past a small test capacity). `cargo x lint` rejected it
  outright: `dead_code` fires on the plain `lib` clippy target (not just
  `--all-targets`, which also runs the `lib test` target where
  `#[cfg(test)]` usage does count) because nothing in non-test code ever
  constructs a `Window` — S2-A2's escape (`pub mod filters`, reachable as
  external library surface without any in-crate caller) does not apply
  here, since an internal ring buffer is not a defensible standalone
  surface for 0.1 the way a reversible filter transform is (`library-
  surface-0-1`, PR #360), and every other lead's "unwired" first slice
  (S2-A2/A3/A4's filters aside) was in fact wired to a real caller
  immediately: S2-A70 modified `decode`'s existing check in place,
  S2-A71/S2-A72 are new `pub` functions `decompress`/`decompress_bounded`
  or a real embedder can call directly and this crate's own tests
  exercise as such. `Window` had neither.
  Tried making it real by wiring it into today's `codec::decode` as a
  drop-in replacement for `copy_checked`, reading match/rep copy sources
  through the ring buffer instead of `output[start + k]`. This is worse
  than dead code, not better: `decode`'s public contract still returns a
  fully-resident `Vec<u8>` (nothing about this slice changes that), so
  every call would carry `output` (already retaining everything) *and* a
  redundant `WINDOW`-sized (1 MiB) `Window` buffer duplicating output's
  own tail, for zero benefit — a straight memory regression on the one
  path real callers use today, the opposite of `rust-craft`'s mechanical-
  sympathy discipline. A ring buffer only pays for itself once something
  downstream actually stops retaining the bytes it evicts; short of that
  pairing, it is pure overhead.
  Mechanism, stated generally: S1-P7's remaining piece is not "a ring
  buffer" plus "wire it in" as two separate slices, because the first
  half has no honest standalone value — the value only exists at the
  seam between the ring buffer and a real sink that discards old bytes,
  and today's `decode`/`decompress`/`decompress_bounded` all promise a
  complete `Vec<u8>` result, so none of them has such a seam yet. The
  next real slice has to land the ring buffer and a genuine bounded-
  memory caller together, not as a tested-but-unused primitive first.
  Narrowest honest shape for that combined slice, given `filters::delta`
  and `filters::bcj` are still whole-slice functions today (S2-A2/S2-A4's
  own "forward accumulation"/"never re-examined" descriptions say the
  *algorithms* are streaming-compatible, but the current code is not
  chunked): a new sink-based decode path scoped to `Candidate::Identity`
  only (`undo_filter`'s no-op case — the LZ loop's own output already
  is the final byte stream, nothing to buffer after it), exposed as a
  real public function real callers can reach (e.g. a `Write`-targeting
  `decompress_to_writer`, falling back to today's whole-buffer `decode`
  then one bulk write for every other `Candidate`, matching S2-D4's
  `Identity`/`Delta`/`Bcj`-streams/`Transpose`-buffers split at least for
  the one candidate that needs no filter-chunking work first). Streaming
  `Delta`/`Bcj` is separate, smaller follow-on scope once each filter
  gains an incremental decode form; `Transpose` stays excluded per
  S2-D4. No code merged from this investigation: the `Window` type and
  its tests were reverted in full, `research/progress.jsonl` gains no
  row (an obstacle-only investigation, same as S2-D4's own). Remaining
  S1-P7 streaming-mode scope: the combined sink-abstraction-plus-real-
  caller slice above, sized for its own dedicated PR rather than a
  heartbeat aside.
- S2-A73 | ACCEPTED | S2-D5's combined sink-abstraction-plus-real-caller
  slice, narrowed to the one candidate it named as needing no
  filter-chunking work first: `mothergod::decompress_to_writer(input,
  max_len, writer)`, writing decoded bytes to any `std::io::Write`
  incrementally instead of collecting a `Vec<u8>`. `lz::Window`, a
  fixed-`WINDOW`-capacity ring buffer (absolute write position mod
  `WINDOW`, so a byte older than `WINDOW` pushes is silently overwritten —
  sound because `ensure_within_window` already rejects any legitimate
  reference to it), replaces `codec::decode`'s `output: Vec<u8>` on a new
  `codec::decode_identity_streaming` path taken only when the frame's
  filter selector is `Candidate::Identity`: the one candidate whose
  `undo_filter` step is the identity transform, so the raw LZ token stream
  this path replays *is* the final output, nothing left to buffer
  afterward. Every other candidate (and `Method::Stored`) falls back to a
  whole-buffer `decode` plus one bulk `write_all`, matching S2-D4's
  `Identity`/`Delta`/`Bcj`-streams/`Transpose`-buffers split for exactly
  the slice that needs no further filter work — `Delta`/`Bcj` streaming
  remains open, named in the S1-P7 lead entry above. | No bpb measurement:
  capability patch, no codec/bitstream change (`encode`/`decode` both
  untouched; `decode_to_writer`'s Identity path is new code alongside
  them, not a modification to either) — `baseline_gate check`: 11 cases,
  no regression, finals reports fresh, confirming it. | Mechanism proving
  the ring buffer's per-byte updates match `decode`'s batch ones: a
  match/rep copy's `Context::after_copy(&output[start..])` folds
  `word_hash` over the whole copied run and takes `prev1`/`prev2` from its
  last two bytes; calling the existing public `after_copy` once per byte
  with a one-element slice, in order, is exactly equivalent (each call's
  output feeds the next call's `self`, so the fold and the last-two-bytes
  tracking both telescope to the same final state a single multi-byte call
  would reach) — verified by inspection, not by adding a second
  parallel-but-separate implementation to keep in sync. A copy's source
  distance from the *current* write position stays constant through its
  own loop even for an overlapping run (distance < length): both the
  current and source position advance by one per byte, so their
  difference never changes, the same identity `copy_checked`'s
  one-byte-at-a-time loop already relies on. `cargo x lint`'s
  `dead_code` objection that sank S2-D5's standalone `Window` does not
  apply here: `Window` has a real in-crate caller
  (`decode_identity_streaming`) from the same commit that introduces it.
  | 21 new tests: `lz::Window`'s own (push/get, an overlapping-run replay
  matching `copy_checked`'s doc-comment shape, wraparound past `WINDOW`
  capacity leaving in-range bytes undisturbed); `codec`'s existing
  `roundtrip` fixtures (empty, single byte, all-literals, a long
  single-byte run, cyclic data, pseudo-random bytes, binary data with
  zero bytes, the founding archive source, the 50x-repeat shrink case)
  extended to also assert `decode_to_writer`'s output byte-for-byte
  against `decode`'s, plus the columnar-drift fixture proving the
  non-Identity fallback; `decode_to_writer`-level adversarial tests
  (bad match distance, distance beyond `lz::WINDOW`, declared length past
  `MAX_DECODED_LEN` on both the Identity and fallback paths, a writer
  that always fails proving its error comes back unwrapped rather than
  folded into the decode-error wrapping); `decompress_to_writer`-level
  tests mirroring `decompress_bounded`'s own (Stored/Lz parity with
  `decompress`, a Stored frame over a tighter bound, an unsupported
  version, a failing writer). None of the 1 MiB wraparound path is
  exercised through the full codec (a real round trip that size is
  minutes-scale per S2-A17's own note on `parse_optimal`); it is
  unit-tested directly on `Window` instead. `research/progress.jsonl`
  it121. Full record: this entry, S1-P7 lead entry updated above.
- S2-A74 | ACCEPTED | `Delta`'s turn at S2-A73's remaining scope
  (`Delta`/`Bcj` streaming, `Transpose` staying excluded per S2-D4).
  Unlike `Identity`, `Delta`'s `undo_filter` step is not a no-op, but it
  is a small fixed-lookback accumulate (`out[i] = out[i-stride] +
  data[i]`), so it undoes one byte at a time with no more state than the
  last `stride` bytes it has itself produced — new `filters::delta::Undo`,
  differentially tested against the batch `decode` it shadows (feeding it
  `encode`'s output one byte at a time must reproduce the original
  input, across strides 1 through 255, the full range an adversarial
  header byte can request, not just `pick`'s own `MAX_DELTA_STRIDE` (96)
  ceiling). Wired into `codec::decode_to_writer`'s existing streaming
  path (S2-A73's `decode_identity_streaming`, renamed
  `decode_undoable_streaming` and generalized over a new `StreamUndo`
  enum) rather than adding a parallel path: `window` still holds the
  *filtered* byte stream throughout, since match/rep distances reference
  positions in what the encoder's LZ pass actually saw (`apply_filter`
  runs before `encode_tokens`) — only the byte handed to `writer` differs
  per candidate, threaded through both the literal branch and
  `copy_streamed`'s per-byte loop, never `window` or `context`, which stay
  on the filtered stream unchanged. `Bcj` and `Transpose` are unaffected,
  still routed to `decode`'s whole-buffer path. | `cargo x check`: 4
  stages green. `baseline_gate check`: 11 cases, no regression, both
  finals reports fresh — `encode`/`decode` both untouched, this only adds
  a second streaming path alongside them. 8 new `Undo` unit tests
  (differential against `decode`, matching its own empty/shorter-than-
  stride/wrapping-overflow/various-strides cases); one existing
  integration test renamed and tightened
  (`roundtrip_columnar_drift_data_selects_delta_and_streams_it`, now
  pinning the kind byte to 1 (`Delta`) rather than just non-identity,
  since this fixture is the only regression coverage of
  `decode_undoable_streaming`'s `Delta` path through a real `encode()`
  trial); one new crafted test
  (`decode_undoable_streaming_delta_path_covers_copy_streamed_too`)
  building a `Candidate::Delta` frame directly from a short repeating
  filtered pattern, bypassing `pick`'s trial selection, because
  `columnar_drift_data`'s random walk gives `parse_optimal` no repeat
  long enough to price a match over literals — without it, `copy_streamed`'s
  own `undo.apply` call would have shipped unexercised. | No bpb
  measurement: capability patch, no codec/bitstream change.
  `research/progress.jsonl` it122. Full record: this entry, S1-P7 lead
  entry updated above.
- S2-A75 | ACCEPTED | `Bcj`'s turn at S2-A74's remaining scope (`Transpose`
  staying excluded per S2-D4). Unlike `Delta`'s fixed lookback, `Bcj`'s
  `undo_filter` step needs fixed *lookahead*: whether a filtered byte starts
  an instruction is decidable the instant it arrives (`0xE8`/`0xE9` or not),
  but rewriting that instruction's operand needs the four bytes that follow
  it — not yet available when the opcode byte itself reaches the streaming
  decoder. New `filters::bcj::Undo`: `apply(filtered_byte) -> Resolved`, a
  fixed-capacity (`INSTRUCTION_LEN`, 5) buffer type avoiding a heap
  allocation per byte in the hot loop (`rust-craft` skill), resolving to
  zero bytes while buffering a candidate instruction's still-incoming
  operand, one byte immediately for a non-opcode byte, or all 5 the instant
  an operand completes; `finish() -> Resolved` flushes a trailing opcode
  byte seen too close to the stream's end to resolve, the same
  too-short-for-any-instruction case `rewrite`'s own scan bound leaves
  untouched. Differentially tested against `decode` by feeding `encode`'s
  output through `apply` one byte at a time (plus a trailing `finish`)
  across empty/too-short/no-opcode/adjacent-instructions/wrapping-overflow/
  jmp fixtures. Wired into `codec.rs`: `StreamUndo` gained a
  `Bcj(bcj::Undo)` variant, and its `apply` method changed shape — it now
  takes the writer directly (`apply(byte, writer) -> io::Result<()>`)
  rather than returning a byte for the caller to write, since `Bcj`'s
  output length per call varies (0/1/5) where `Identity`/`Delta`'s is
  always 1; this unifies all three under one call shape without a shared
  buffer type crossing the `filters`/`codec` boundary. A new
  `StreamUndo::finish(writer)` call after `decode_undoable_streaming`'s
  token loop flushes `Bcj`'s trailing bytes (a no-op for
  `Identity`/`Delta`). `decode_to_writer` now dispatches `Candidate::Bcj`
  to the streaming path instead of the whole-buffer fallback; only
  `Transpose` still falls back. `decompress_to_writer`'s and
  `decodes_incrementally`'s doc comments, stale since S2-A74 shipped Delta
  streaming without updating them, now name all three streaming candidates.
  | `cargo x check`: 4 stages green, 276 lib tests (up from 267).
  `baseline_gate check`: 11 cases, no regression, both finals reports fresh
  — `encode`/`decode` both untouched. 2 new codec-level tests: a call-dense
  fixture (many `call rel32` instructions targeting the same absolute
  address, mirroring `filters::select::pick`'s own opcode-density fixture)
  proving `encode()` actually selects `Candidate::Bcj` and the streaming
  path reproduces `decode()`'s output; a hand-built `Candidate::Bcj` frame
  from a repeating filtered instruction, exercising `copy_streamed`'s own
  undo call the same way S2-A74's `Delta` fixture did.
  `streaming_falls_back_to_decode_for_non_identity_declared_length_over_the_max`
  renamed and repointed at `Candidate::Transpose`, the one candidate still
  on the fallback path, since `Bcj` no longer is. | No bpb measurement:
  capability patch, no codec/bitstream change. `research/progress.jsonl`
  it123. Full record: this entry, S1-P7 lead entry updated above.
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
- S2-A89 | ACCEPTED | S1-P4's own remaining scope after S2-R7's rejection of
  an unconditional `WINDOW` bump: "grow the window only when a cheap
  pre-pass suggests recurrence past it exists, paying the encode-time cost
  only where the bpb win is real," named as one of two untried directions
  and, unlike the other (a deliberate large-file mode, ROADMAP M5
  territory), buildable as a standalone primitive ahead of any wiring
  decision — the same first-slice shape every other lead opened with
  (S2-A42 for S1-P2, S2-A57 for S1-P3, S2-A64 for S1-P5, S2-A81 for S1-P6).
  `lz::likely_benefits_from_larger_window(data, base_window)` samples an
  8-byte anchor (`DETECTOR_ANCHOR_LEN`) every 64 bytes (`DETECTOR_STRIDE`),
  remembers the most recent position each exact anchor was seen at, and
  reports `true` the first time two occurrences sit more than `base_window`
  bytes apart — never measuring match length or cost, never choosing a
  window itself, and never touching `dp_round`, `parse_optimal_with_window`,
  or any wired call site. Anchors compare byte-for-byte through a `HashMap`
  key, so a `true` result names a real recurrence, never a hash collision
  (S2-R7's own blanket-bump measurement paid real sealed-set bpb and
  1.05x-1.41x encode time on data without this kind of recurrence at all;
  this function exists so a future wiring slice can pay that cost
  conditionally instead). | 4 new unit tests: false below and exactly at
  `base_window` (data cannot contain a wider gap, the early-return case),
  false on stride-aligned noise with no repeated anchor, true when a
  planted repeat sits just past `base_window`, false when the same planted
  repeat sits at or under `base_window` (the actual filtering logic, not
  just non-sampling) — all four share a `planted_repeat` test helper that
  fills each 64-byte block with a distinct byte so no sampled anchor
  collides by construction. `cargo x check`: 4 stages green, 361 lib tests
  (up from 357). `baseline_gate check`: 11 cases, no regression (nothing
  wired, unaffected). | No bpb measurement: this is a detector, not a parse
  change, so there is no champion to diff against — `progress.jsonl`
  records this as `kind: "patch"` with null bpb deltas. Remaining S1-P4
  scope: the wiring decision itself — call this detector before choosing
  `parse_optimal`'s window, decide the actual candidate window and
  threshold (this slice picked `DETECTOR_ANCHOR_LEN`/`DETECTOR_STRIDE` for
  cheapness, not tuned against any corpus), and measure the resulting
  train/sealed bpb and encode-time cost on the conditional path, which
  S2-R7 never did because nothing before this slice could tell the wired
  parse when to pay the larger window's cost. `research/progress.jsonl`
  it138.
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
- S2-R8 | REJECTED | Third slice of ROADMAP M3's fifth standing lead
  (S1-P5, per-column modeling after transpose): before committing to the
  real `Literal`/`Method`/`FORMAT_VERSION` wiring slice S2-A64/S2-A66 both
  left as remaining scope, measured whether column identity would even
  help, the same "pair a hypothetical against the shipped model via
  ideal-cost accounting, before wiring" methodology S2-R6 used for S1-P3.
  Hypothesis: replacing `Literal`'s six-expert mix with `max_banks`
  independent order-0 `Model`s, one per `column::column_bank(column::
  column_of(position, columns, len), max_banks)`, reduces ideal-cost
  bits/byte on already-transposed column-structured data. A new
  `codec::ideal_cost_bits_column_bank_experiment(data, columns,
  max_banks)` walks the same `lz::parse_optimal` token stream
  `codec::ideal_cost_bits` prices, coding flag/length/offset/slot symbols
  identically and pricing only literal bytes differently, so any delta
  between the two is attributable to the literal model alone, not a
  confound from also changing the parse. | Measured (all model-cost, not
  real-bitstream, numbers; bench crate generators, git revision
  `5aaf0314`; `CASE_LEN` 50,000): train, a rotated window (offset 50,000
  into 150,000 generated bytes, row-width-aligned so the rotation
  doesn't cut a record in half) at train seed `0xC01D_BEEF_1234_5678`,
  distinct from `bench::baseline::CASE_SEED` — `sqlite_like_records`
  (columns=20, S1-P5's own "sao"-shaped train target): 3.266457 ->
  3.347801 bpb, **+0.081344**. `interleaved_audio16` (columns=2): 5.765049
  -> 5.811079 bpb, **+0.046029**. Sealed: `gradient_image` (columns=200,
  `DatasetKind::GradientImage` is sealed-only, `sealed_seed` of the same
  train seed, not rotated): 5.916617 -> 7.991745 bpb, **+2.075129**, the
  worst of the three by a wide margin. Every case regressed; corpus
  policy's accept rule (train improvement AND no validation regression)
  fails on both halves at once, not just one. | Mechanism: an order-0
  histogram, even a fresh one per column, only ever knows a symbol's
  marginal frequency inside its own bank — it cannot represent "this
  byte's value depends on the previous byte in this same column," which
  is exactly the correlation `transpose::encode` was built to expose
  (`JOURNAL` S1-A2) and exactly what `Literal`'s fast/slow order-1 experts
  already spend most of their weight on once a column run gets going. The
  regression scales with how locally smooth/correlated the data is:
  `gradient_image`'s pixels drift gradually down each column (a strong
  order-1 signal a histogram cannot use at all) and paid the worst tax;
  `sqlite_like_records`' mixed categorical/numeric fields are the most
  histogram-friendly of the three (a few fields have only a handful of
  distinct values) and paid the least. The column-boundary signal S2-A64's
  own module doc named as the real gap (a fresh context's first few bytes
  after a column boundary, before order-1 adjacency has re-adapted) is real
  but narrow — one boundary per column, a few dozen columns per case here
  — and discarding the other five experts entirely to get it costs far
  more than that narrow boundary is worth. This does not fully retire
  S1-P5: it falsifies "column identity, alone, beats the shipped mixer,"
  not "column identity, blended in as a seventh expert alongside the
  other six, ever helps at a boundary" — S2-R6 hit the same
  narrower-than-hoped-for shape for S1-P3's escape fallback and drew the
  same distinction. Candidate code (`codec::
  ideal_cost_bits_column_bank_experiment`, its four unit tests, and the
  scratch driver `bench/src/bin/scratch_column_bank_experiment.rs`)
  reverted in full, same as S2-R6; `column::column_of`/`column_bank`
  (S2-A64/S2-A66) are unaffected, standalone primitives outside this
  slice's scope. `research/progress.jsonl` it114. Remaining S1-P5 scope:
  see the updated S1-P5 entry above.
- S2-A67 | ACCEPTED | ROADMAP SPEED scorecard (issue #364): unmeasurable
  on two weekly scorecards because neither `docs/benchmarks/silesia.md`
  nor `canterbury.md` carried a throughput column and nothing measured
  decode at all. `bench::finals::FileMeasurement` gained `encode_secs`/
  `decode_secs`, timed with `std::time::Instant` around
  `mothergod::compress`/`mothergod::decompress` inside
  `reference::measure_one` — single-thread, the same measurement thread
  `measure_all` already spawns one-per-file, not a new source of
  parallelism. `mothergod::decompress` is now called on this path for the
  first time (previously only `mothergod_len` was read off the compressed
  `Vec`); its output is compared against the original bytes, so a corpus
  round-trip failure now surfaces as a measurement error instead of an
  unverified compressed length silently entering a report. `format_report`
  gained two decimal-MB/s columns (`mothergod encode MB/s`, `mothergod
  decode MB/s`), per-file and aggregate — the aggregate is total original
  bytes over total wall-clock seconds, the same byte-weighted-not-averaged
  shape `aggregate_bpb` already uses (CLAUDE.md rule 4). `canterbury.md`
  regenerated with real numbers (11 files, 11.5s wall). `silesia.md`'s
  regeneration is a fast-follow: the full ~200 MB corpus is on the order
  of half an hour even parallelized across cores (S2-A54), bounded below
  by its single largest file — too slow for one agent turn, the same call
  S2-A52 already made for Silesia. | `cargo x check`: 4 stages green (129
  `mothergod-bench` tests under `--features corpus-fetch
  --include-ignored`, up from 108 default-feature, including the
  real-network `fetch_and_cache_smoke_tests_the_real_pins`); `cargo
  clippy -p mothergod-bench --all-targets --features corpus-fetch --
  --deny warnings` clean; `baseline_gate check`: 11 cases, no regression,
  finals reports fresh — no codec code changed. | Finding, not fixed
  here: Canterbury's smallest file, `xargs.1` (4227 bytes), decodes at
  0.775 MB/s, under the ROADMAP SPEED floor (`>=1 MB/s`) — consistent
  with S1-P6's already-recorded `Literal::decode` worst case (~854 KB/s),
  amplified on a small file where fixed per-call overhead dominates. |
  No bpb measurement: measurement-capability patch, not a ratio
  experiment; `research/progress.jsonl` records this as `kind: "patch"`
  with null deltas, it115. Remaining scope: run `silesia_report` and
  commit `silesia.md`'s regeneration (issue to file).
- S2-A68 | ACCEPTED | Issue #366: S2-A67's fast-follow closed. Ran
  `cargo run -p mothergod-bench --release --features corpus-fetch --bin
  silesia_report`, the same S2-A54-parallelized (`measure_all`, one OS
  thread per file) command that produced `silesia.md`'s existing numbers,
  now exercising the `encode_secs`/`decode_secs` timing S2-A67 added. No
  code changed; this is a report regeneration only. | This entry originally
  claimed "well under 10 minutes wall clock on 4 cores," copied from
  S2-A54's 8m20s without a fresh stopwatch. Review (PR #367) caught the
  claim as arithmetically impossible against this same run's own numbers:
  211,938,580 bytes at the reported 0.062 MB/s aggregate encode alone is
  ~3418s of work, ~880s best case spread over 4 cores, before fetch or
  reference-compressor time is counted. A same-session re-run attempt
  confirmed it empirically: `fetch_silesia_files` fetches one file at a
  time (serial, network-bound), and 10 of 12 files alone took 6m48s before
  a connection reset, with `measure_all`'s compute not yet started.
  S2-A54's 8m20s measured compress-only, pre-dating both S2-A67's decode
  timing and this fetch instrumentation, so it is not a valid stand-in for
  this run's total; S2-A52's original "half an hour" estimate is the
  right order of magnitude, not the stale number this entry accused it of
  being. Corrected: no wall-clock number is asserted for this run. |
  bits/byte unchanged
  within measurement (aggregate 2.060705, identical to the pre-existing
  report): confirms the new timing instrumentation reads but does not
  perturb the compressed bytes. New columns: aggregate 0.062 MB/s encode,
  2.084 MB/s decode; slowest decode `x-ray` at 0.392 MB/s and `sao` at
  0.428 MB/s, both under the ROADMAP SPEED floor (`>=1 MB/s`), same
  small/dense-file amplification pattern S2-A67 found on Canterbury's
  `xargs.1`. Finding, not fixed here. No bpb measurement:
  `research/progress.jsonl` records this as `kind: "patch"` with null
  deltas, it116.
- S2-R9 | REJECTED | S1-P4's own remaining scope after S2-A89 (issue-less
  standing lead): wire `lz::likely_benefits_from_larger_window` as a gate
  on whether `parse_optimal` grows its window from `lz::WINDOW`
  (1,048,576) to the `2^21 - 1` (2,097,151) ceiling S2-A63 proved free of
  format cost, so S2-R7's blanket-bump sealed-set regression is paid only
  where a real recurrence past `WINDOW` exists. Measured with a throwaway
  (uncommitted) scratch binary, `bench/src/bin/window_detector_experiment.rs`
  (deleted after this measurement, per S2-R6/S2-R7's own convention),
  `codec::ideal_cost_bits_with_window` at `old_window = lz::WINDOW` vs
  `new_window = 2,097,151`, gated per-case by
  `likely_benefits_from_larger_window(data, old_window)`, release build:
  S2-R7's own four train-eligible non-ladder kinds at 4,000,000 bytes,
  train seed `0xC0FFEE123456789A`, plus both sealed-only kinds at
  `sealed_seed` of the same key, plus S2-A63's own planted
  `long_range_repeat` case (len 1,222,672, distance 1,198,576) at both
  seeds. Every non-gated number reproduces S2-R7/S2-A63 exactly to six
  decimal places (cross-check that this harness measures the same thing
  they did): `markov_h8_2_trap` -0.025424, `sqlite_like_records` -0.006542,
  `x86_dense_code` +0.000505, `json_records` +0.000273, `access_log`
  +0.000189, `gradient_image` +0.000092, `long_range_repeat` -0.021443
  (train) / -0.021401 (sealed). | **Rejected**, two independent failure
  modes, either one sufficient on its own: (1) the detector fires `false`
  on both `long_range_repeat` seeds — the exact scenario this lead
  targets — missing a real -0.0214 b/B win entirely, and (2) it fires
  `true` on the sealed-only `access_log`, letting through +0.000189 b/B of
  exactly the regression this trigger exists to prevent (`gradient_image`
  is the one sealed kind it correctly gates away). Mechanism, confirmed by
  a direct manipulation, not just inferred from the miss: `long_range_repeat`'s
  distance (1,198,576) is not a multiple of `DETECTOR_STRIDE` (64) —
  `1,198,576 mod 64 = 48` — so no two of the detector's fixed
  absolute-position samples (`0, 64, 128, ...`) ever land on matching
  offsets into the planted 4,096-byte repeat; re-running the identical
  case with the distance rounded down to the nearest multiple of 64
  (1,198,528) flips the detector to `true` and recovers the win
  (-0.021446 b/B, matching the -0.021443 unrounded number to three
  decimal places). `likely_benefits_from_larger_window` can therefore
  only ever see a repeat whose distance happens to be a multiple of
  `DETECTOR_STRIDE` — a ~1/64 chance for a real-world repeat's distance,
  not a property any actual recurrence has reason to satisfy — while
  `access_log`'s false-positive comes from the opposite direction:
  sampling ~62,500 positions over 4,000,000 bytes makes an incidental
  8-byte anchor collision likely by the birthday bound even with no
  exploitable long-range structure at all (S2-R7 already named this shape
  for `markov_h8_2_trap`'s real win; here the same sampling-volume effect
  fires without a matching win behind it). Both S2-A89's own unit tests
  and this rejection's own `long_range_repeat` case used a
  stride-aligned or unaligned offset respectively by construction, which
  is why S2-A89's tests never exposed this: `planted_repeat`
  (`src/lz.rs`) deliberately rounds `far_offset` to a `DETECTOR_STRIDE`
  multiple before asserting `true`, so the detector's contract was tested
  exactly on the one alignment case it handles and never on the general
  one. S2-A89 itself is not wrong: its doc comment never claimed anchor
  selection is alignment-invariant, and the primitive still does exactly
  what it says. This slice found the assumption that would have made it
  useful for this wiring, and falsified it, before any code reached
  `parse_optimal`. No source under `src/` changed; the scratch binary
  is not committed. `research/progress.jsonl` it139. Remaining S1-P4
  scope: a content-defined anchor selection (Rabin-fingerprint-style,
  choosing sample positions by a rolling hash of local content instead of
  fixed absolute offsets, the way rsync/content-defined chunking picks
  chunk boundaries) would make detection distance-invariant and is the
  next concrete idea; a fixed-stride detector is not salvageable by
  tuning `DETECTOR_STRIDE` alone, since any fixed stride has the same
  alignment blind spot at a different period. The large-file-mode
  alternative (ROADMAP M5 SPEED territory) remains the other untried
  branch.
- S2-R10 | REJECTED | S1-P2's own closing note (S2-R5's entry, restated in
  the lead's own summary): "a differently-shaped idea, not a
  pricing-cadence or observation-rule tweak, is owed before spending
  another slice here." Hypothesis: the binary-tree match finder's *search
  quality*, not its pricing, was the untried lever — `dp_round` has passed
  `NICE_LEN_OPTIMAL` (128) and `MAX_TREE_DEPTH_OPTIMAL` (640) to
  `BinaryTreeMatchFinder::insert_and_find` unchanged since S2-A46/S2-A48
  wired them, and both were tuned there purely to satisfy the issue #179
  speed guard and a near-duplicate-block micro-benchmark (S2-A44/S2-A46),
  never independently measured as a bits/byte lever on S1-P2's own named
  target (sqlite/json/jsonl-shaped structured records, unmoved by every
  pricing-cadence slice so far, S2-A47 through S2-R5). Raising either
  constant — more candidates considered (`MAX_TREE_DEPTH_OPTIMAL`) or a
  longer match reportable/scannable before the early exit
  (`NICE_LEN_OPTIMAL`) — is a match-quality change, not a pricing one, so
  it is a genuinely different shape. Measured directly (constants edited
  locally, never committed) rather than assumed: at `bench::baseline`'s
  own `CASE_LEN` (50,000), `NICE_LEN_OPTIMAL` 128→273 (LZMA's own
  `nice_len` ceiling, `MAX_TREE_DEPTH_OPTIMAL` held at 640) changed zero
  of the 11 train cases' compressed bytes and zero of the two sealed-only
  kinds' (`access_log`, `gradient_image`, measured the way S2-A48's
  `measure_sealed_tmp.rs` did), byte-for-byte; separately,
  `MAX_TREE_DEPTH_OPTIMAL` 640→5,000 (`NICE_LEN_OPTIMAL` held at 128)
  changed zero of the same 13 cases too. Recognized the same trap S2-R7's
  own text names ("`CASE_LEN` 50,000... cannot show any window effect at
  all... needed its own, much larger, ad hoc length") could apply here as
  well, so re-measured `json_records`/`sqlite_like_records` directly (S1-P2's
  own named target, train seed `0xC0FFEE123456789A`, no sealed-set peeking
  since this is feasibility exploration) at an ad hoc 4,000,000 bytes: at
  that scale, `NICE_LEN_OPTIMAL` 273 alone left both cases' compressed
  length byte-for-byte unchanged (284,234 and 1,678,319 bytes) and wall
  clock unchanged (47.4s vs 47.5s baseline, both release builds, `dp_round`
  called through `mothergod::compress`); `MAX_TREE_DEPTH_OPTIMAL` 5,000
  alone left `sqlite_like_records` byte-for-byte unchanged and moved
  `json_records` by exactly one byte (284,234 → 284,233, ~3.5e-6
  relative), at roughly double the wall clock (96.9s). The two issue #179
  speed-guard tests
  (`optimal_roundtrip_long_run_of_one_repeated_byte_stays_linear`,
  `binary_tree_near_duplicate_blocks_benefit_from_prefix_reuse`) stayed
  well inside their budgets at every combination tried (debug build, both
  together: 1.73s baseline, 2.03s at `nice_len` 273, 1.72s at `max_depth`
  5,000; budgets 15s and 3s respectively) — the speed side of this
  hypothesis was never in doubt, only the ratio side. **Rejected**:
  neither constant binds on this target's own structure at either scale,
  so there is nothing for a higher bound to find. Mechanism:
  `sqlite_like_records`' fixed 8+4+8-byte rows and `json_records`'
  literal-text-between-incrementing-digits shape both produce match runs
  far shorter than even the current 128-byte `nice_len`, let alone 273 —
  the record layout itself caps match length, not the search bound, so
  raising the cap changes nothing regardless of scale. `HASH_BITS` (17,
  131,072 buckets) keeps average per-bucket candidate counts (≈30 at
  4,000,000 bytes) far under even the *current* 640-deep budget, so a
  5,000-deep budget has essentially nothing extra, in-window, to visit;
  the one-byte `json_records` change is consistent with a single
  unusually deep bucket's tie-break flipping, not a systematic quality
  gain, and does not begin to pay for the ~2x encode-time cost measured
  alongside it. This falsifies the search-quality hypothesis cleanly: the
  parse already sees every candidate worth seeing at today's constants,
  on today's target data, at both the CI-gate scale and a realistic
  multi-megabyte scale. Candidate code: none committed. Two scratch
  binaries used for measurement, `bench/src/bin/measure_sealed_tmp.rs`
  (S2-A48's own naming convention, re-created and re-deleted here) and
  `bench/src/bin/nice_len_depth_scale_tmp.rs`, both deleted after
  measurement per S2-R6/S2-R7/S2-R9's convention; `src/lz.rs`'s constants
  reverted to their committed values (128/640) in every case.
  `research/progress.jsonl` it141. Remaining S1-P2 scope: see the lead's
  own entry above, updated — both the intra-round pricing angle and now
  the match-finder search-quality angle are closed on the evidence
  gathered so far; a modeling primitive that reaches structure a
  binary-tree parse cannot see at all (schema-aware/typed-field literal
  contexts, or accepting the residue as near this architecture's ceiling)
  is what is owed next, not another parse-level lever.
- S2-A91 | ACCEPTED | S1-P4's own remaining scope after S2-R9: "the
  detector's fixed-position sampling needs replacing with content-defined
  anchor selection before this trigger is trustworthy." Built that
  replacement as a standalone primitive, same first-slice shape S2-A89
  itself took, not yet wired to any parse call site.
  `lz::likely_benefits_from_larger_window_content_defined(data,
  base_window)` answers the identical question S2-A89's detector does
  (does an exact-byte recurrence past `base_window` exist) but selects
  candidate anchor positions with `is_content_defined_anchor`, a
  polynomial hash (`CONTENT_HASH_BASE`, wrapping `u64` arithmetic) over
  each `DETECTOR_ANCHOR_LEN`-byte window, firing on low
  `DETECTOR_STRIDE`-1 bits of the hash — a pure function of a window's own
  bytes, so two identical windows anywhere in `data` always agree on
  whether they are sampled, unlike S2-A89's fixed absolute-position grid.
  Directly demonstrates the fix S2-R9 called for: a repeat planted at
  `5 * DETECTOR_STRIDE + 7` (deliberately off the fixed-stride grid) is
  missed by `likely_benefits_from_larger_window` and found by the new
  function, both assertions in the same test
  (`content_defined_detector_finds_a_repeat_the_fixed_stride_detector_misses`),
  the same "a direct manipulation, not just inferred" standard S2-R9's own
  rejection used. Does not address S2-R9's other, independent finding
  (false positives from incidental anchor collisions on data sampled
  densely enough to hit the birthday bound, e.g. `access_log`):
  content-defined selection changes *where* anchors fall, not how many,
  so that failure mode is untested by this slice and stays open — this
  closes only the alignment blind spot, not the whole wiring question.
  Recomputes the rolling hash from scratch per position rather than
  incrementally (documented as a known follow-up, not a rolling-hash
  bug): correctness over cost for a detector-only primitive not yet on
  any wired path. Review round (PR #634) caught the public function
  underflowing on `base_window < data.len() < DETECTOR_ANCHOR_LEN`: the
  `for i in 0..=data.len() - DETECTOR_ANCHOR_LEN` loop assumed a length
  the early return did not guarantee, panicking on ordinary short input
  the sibling detector's `while i + DETECTOR_ANCHOR_LEN <= data.len()`
  shape handles by degrading to zero iterations. Fixed to match the
  sibling's loop-condition shape; the "rolling polynomial hash" language
  above was also imprecise (it recomputes from scratch, as the next
  paragraph already said correctly) and is now just "polynomial hash". |
  6 new unit tests (367 lib tests total, up from 361):
  early-return at/under `base_window` (mirroring S2-A89's own), the
  underflow regression at `data.len() = 3 < DETECTOR_ANCHOR_LEN`, false on
  a splitmix64 pseudorandom stream with no accidental 8-byte collision
  (the zero-padded counter pattern S2-A89's own noise test used is not
  noise enough here — this detector examines every position, not just
  `DETECTOR_STRIDE`-aligned ones, and a zero-heavy window recurs
  legitimately across the padding; verified by direct construction before
  concluding it was a real recurrence and not a bug, then replaced with
  data that has none), true on a stride-aligned planted repeat, false when
  the same repeat sits inside `base_window`, and the off-grid
  find/miss contrast above. `cargo x check`: 4 stages green.
  `baseline_gate check`: 11 cases, no regression (nothing wired,
  unaffected). | No bpb measurement: this is a detector, not a parse
  change, so there is no champion to diff against — `progress.jsonl`
  records this as `kind: "patch"` with null bpb deltas, same reason
  S2-A89 did for its own first slice. Remaining S1-P4 scope: the
  birthday-bound false-positive question S2-R9 raised is still
  unaddressed by either detector; wiring either one into `parse_optimal`'s
  window choice still needs that answered (a longer anchor, or requiring
  agreement past `DETECTOR_ANCHOR_LEN` before reporting `true`, are the
  two untried ideas) before a gated window bump is worth re-measuring
  against `bench::baseline`'s sealed set the way S2-R9 did. `research/
  progress.jsonl` it142.
- S2-A92 | ACCEPTED | S1-P4's own remaining scope after S2-A91: S2-R9's
  second, still-open failure mode — an incidental `DETECTOR_ANCHOR_LEN`-byte
  agreement (a real 8-byte recurrence, not a hash collision) firing `true`
  with no exploitable long match behind it, measured on `access_log` — and
  S2-A91's own closing note naming "requiring agreement past
  `DETECTOR_ANCHOR_LEN` before reporting `true`" as one of two untried
  fixes (the other, a longer anchor, only lowers the incidence without
  changing the mechanism). Built that fix as
  `lz::likely_benefits_from_larger_window_content_defined_confirmed`, a
  third sibling alongside the fixed-stride and content-defined-only
  detectors (same first-slice, not-yet-wired shape). It runs the identical
  content-defined sampling S2-A91 built, then additionally requires
  `DETECTOR_CONFIRM_LEN` (24, a 32-byte total confirmed run, chosen as a
  starting value pending the wiring slice's own measurement, not tuned
  against any corpus) bytes past each anchor to also agree byte-for-byte
  before reporting `true` (`confirms_past_anchor`), the same
  agrees-past-the-anchor test a real long-range recurrence passes almost
  for free and a merely-common short substring usually does not.
  Deliberately still just a bounds-checked byte comparison, not a match-
  length measurement: `dp_round`'s job stays `dp_round`'s. A confirmation
  window that would run past `data`'s end reports `false` for that
  candidate rather than panicking, symmetric with the anchor step's own
  end-of-data handling. | 3 new unit tests (370 lib tests total, up from
  367): a real extending recurrence (same anchor and same confirmation
  tail at both occurrences) stays `true`, the exact S2-R9 failure shape —
  same anchor, deliberately divergent confirmation tail — is `true` on the
  unconfirmed content-defined detector and `false` on this one (the direct
  A/B proof S2-A91's own off-grid test used, applied to this failure mode
  instead), and a candidate whose confirmation window runs past `data`'s
  end returns `false` without panicking. `cargo x check`: 4 stages green.
  `baseline_gate check`: 11 cases, no regression (nothing wired,
  unaffected). | No bpb measurement, same reason as S2-A89/S2-A91: not yet
  wired to any parse call site, so there is no champion to diff against —
  `progress.jsonl` records this as `kind: "patch"` with null bpb deltas.
  Remaining S1-P4 scope: both of S2-R9's failure modes now have a
  candidate fix (S2-A91 for the alignment blind spot, this entry for the
  incidental-agreement false positive); wiring
  `likely_benefits_from_larger_window_content_defined_confirmed` into
  `parse_optimal`'s window choice and re-measuring against
  `bench::baseline`'s sealed set the way S2-R9 did — including whether
  `DETECTOR_CONFIRM_LEN`'s starting value of 24 is the right one, untested
  here — is the next slice; a large-file-mode alternative (ROADMAP M5
  SPEED territory) remains the other untried branch, unaffected by this
  entry either way. `research/progress.jsonl` it143.
- S2-A93 | ACCEPTED | S1-P4's own remaining scope after S2-A92: wire
  `likely_benefits_from_larger_window_content_defined_confirmed` into
  `parse_optimal`'s window choice and re-measure against
  `bench::baseline`'s sealed set the way S2-R9 did. Built
  `lz::parse_optimal_adaptive_window(data)`: `WINDOW` unless the confirmed
  detector finds a real recurrence past it, in which case
  `lz::ADAPTIVE_WINDOW` (`2^21 - 1`, S2-A63's proven-free-of-format-cost
  ceiling); `codec::ideal_cost_bits_adaptive_window` measures it through
  the real adaptive models the way `ideal_cost_bits_with_window` measures
  a fixed window. First measurement reproduced S2-R9's own rejection
  almost exactly: `access_log` (sealed) still regressed (+0.000189 b/B)
  even gated on the confirmed detector, and tracing the exact anchor pair
  it fired on found a genuine, if narrow, 41-byte recurrence (a repeated
  IP address plus fixed log-line preamble recurring at a rare joint
  distance past `WINDOW`, not a birthday-bound collision as S2-R9's
  framing assumed) — the detector was *right*, and gating on it still
  cost bits. Falsified the working hypothesis (S2-A92's framing: "an
  incidental agreement... with no exploitable long match behind it")
  before accepting or rejecting: the confirmed detector already finds
  only real recurrences, so a still-regressing gated case meant the
  regression's cause was elsewhere. Traced it to
  `parse_optimal_with_window`'s own documented contract: `parse_greedy`'s
  seed pass stays bound to the wired `WINDOW` regardless of the `dp_round`
  rounds' window (S2-A61's original design, never load-bearing-tested
  until now), so growing only the search window leaves the DP's first
  price table blind to the far match it is about to reconsider, a worse
  initial guess the iterative reseeding never fully recovers from.
  Confirmed by direct manipulation, not inference: binding both the seed
  pass (via `parse_greedy_with_window`, S2-A65's own primitive) and every
  `dp_round` to the SAME chosen window flipped `access_log` from +0.000189
  to **-0.001109** and `json_records` from +0.000273 to **-0.000383**,
  the exact two cases S2-R9 and this slice's first pass both regressed on.
  Every detector=`true` case improved (`long_range_repeat`: train
  -0.021482, sealed -0.021422, both slightly past even S2-A63's
  unconditional-window numbers; `json_records` -0.000383; `access_log`
  sealed -0.001109); every detector=`false` case stayed exactly at zero
  (`markov_h8_2_trap`, `sqlite_like_records`, `x86_dense_code`,
  `gradient_image` sealed), correctly declining the diffuse,
  no-single-exact-recurrence wins S2-R7 traced to incidental
  multi-megabyte-scale structure rather than a real long-range repeat —
  this gate is deliberately narrower than "any window growth helps," and
  that narrowness is why it never regresses. `DETECTOR_CONFIRM_LEN`'s
  starting value of 24 needed no change: nothing in this measurement
  motivated one. | 3 new unit tests (373 lib tests total, up from 370):
  two in `src/lz.rs` (`parse_optimal_adaptive_window` reproduces
  `parse_optimal`'s output byte-for-byte for `data.len() <= WINDOW`, the
  gate's own early return making this unconditional; a planted confirmed
  repeat just past `WINDOW` round-trips through the real three-round DP
  with at least one token carrying a distance beyond `WINDOW`, the first
  round-trip test in this crate to exercise `bucket()`/`dp_round` against
  a real out-of-`WINDOW` distance end-to-end rather than `dp_round`
  called directly with a wider window parameter), one in `src/codec.rs`
  (the adaptive and wired ideal-cost entry points agree exactly within
  `WINDOW`). `cargo x check`:
  4 stages green. `baseline_gate check`: 11 cases, no regression (nothing
  wired to `compress`/`encode`, unaffected). | Measured with a throwaway
  (uncommitted) scratch binary, `bench/src/bin/
  window_wiring_confirmed_experiment.rs` (deleted after this measurement,
  per S2-R6/S2-R7/S2-R9's convention), on the same cases S2-R9 used so
  the numbers are directly comparable: S2-R9's own four train-eligible
  non-ladder kinds at 4,000,000 bytes (train seed
  `0xC0FFEE123456789A`), both sealed-only kinds at the same length
  (`sealed_seed` of that key), plus S2-A63's own planted
  `long_range_repeat` (len 1,222,672, distance 1,198,576) at both seeds;
  every unconditional-window number reproduced S2-R9's own cross-check
  values to six decimal places. `research/progress.jsonl` it144.
  Remaining S1-P4 scope: this closes the standalone-primitive arc S2-A61
  opened — every slice from here needs to touch `compress`/`encode`
  itself. Wiring `parse_optimal_adaptive_window` into the real codec
  (`encode`'s `Method::Lz` path) and re-measuring through a real
  bitstream against `bench::baseline`'s sealed set (not just ideal cost)
  is the next slice; that wiring is encoder-only (`dp_round`'s
  `Token`/`Move` shape and `bucket()`'s ceiling are unchanged, so decode
  needs no new case and no `FORMAT_VERSION` bump, CLAUDE.md hard rule 5's
  encoder-only carve-out) but regenerates the current-version golden
  fixture per `tests/golden.rs` (issue #290's ruling) and updates
  `bench/baseline.json` for whichever gate cases the detector newly
  fires true on. The several-10s-of-MiB Silesia finals this lead targets
  (`mozilla`, `nci`, `samba`, `sao`, `webster`) stay out of reach either
  way: `ADAPTIVE_WINDOW` is still under 2 MiB.
- S2-A94 | ACCEPTED | S1-P4's own remaining scope after S2-A93: before
  wiring `parse_optimal_adaptive_window` into `encode`'s real
  `Method::Lz` path, built the real-bitstream counterpart to
  `ideal_cost_bits_with_window`/`ideal_cost_bits_adaptive_window`, so
  the next slice's measurement would not be flying blind on the
  ideal-vs-real coder gap ADR-0038 documents. `codec::
  encode_tokens` factored into a thin wrapper over a new
  `encode_tokens_with(data, columns, tokens)` taking already-parsed
  tokens, letting two new `pub` entry points reuse its real `Encoder`
  body: `compressed_len_with_window(data, window)` (parses with
  `lz::parse_optimal_with_window`) and `compressed_len_adaptive_window(data)`
  (parses with `lz::parse_optimal_adaptive_window`), returning the real
  payload byte length rather than a modeled cost sum. `encode_tokens`
  itself is untouched — still `lz::parse_optimal(data)` unconditionally
  — so this is purely additive: `compress`/`decompress` behavior does
  not change. Standalone primitive, same shape every earlier S1-P4 slice
  took. | 1 new unit test (374 lib tests, up from 373):
  `compressed_len_adaptive_window` matches `encode_tokens`'s own wired
  length exactly for data at or under `WINDOW`, the gate being
  unconditionally false there (same reason
  `ideal_cost_bits_adaptive_window_matches_wired_window_within_window`
  holds at the ideal-cost layer). `cargo x check`: 4 stages green.
  `baseline_gate check`: 11 cases, no regression (nothing wired to
  `compress`/`encode`, unaffected). | No bpb measurement of its own:
  this is infra enabling the next slice's real-bitstream measurement,
  same as S2-A1/S2-A50/S2-A61 before it; `research/progress.jsonl`
  records it as `kind: "patch"` with null bpb deltas.
- S2-R11 | REJECTED | S1-P4's own remaining scope after S2-A94: used
  `compressed_len_with_window`/`compressed_len_adaptive_window` to
  re-measure S2-A93's wiring decision through a real bitstream, not
  just ideal cost, at S2-R9/S2-A93's exact scale (4,000,000 bytes, train
  seed `0xC0FFEE123456789A`) but across every train-eligible non-ladder
  `DatasetKind` this crate currently defines — five, not the four S2-R9
  happened to test: `JsonRecords`, `Base64Wrapped`, `InterleavedAudio16`,
  `SqliteLikeRecords`, `X86DenseCode` — plus both sealed-only kinds and
  `long_range_repeat` at both seeds. Every case S2-A93 already tested
  reproduced its ideal-cost numbers closely through the real coder too
  (`access_log` sealed -0.001108 vs S2-A93's -0.001109; `json_records`
  -0.000384 vs -0.000383; `long_range_repeat` train -0.021481/sealed
  -0.021422, both matching to five decimal places), confirming the
  ideal-vs-real gap never flips an accept/reject verdict on any of them.
  But `Base64Wrapped(train)`, outside S2-R9's own four-kind sweep,
  regressed: 0.470048 -> 0.475922 b/B, **+0.005874**, a third distinct
  confirmed-detector failure mode after S2-R9's alignment blind spot
  (fixed by S2-A91) and incidental-collision false positive (fixed by
  S2-A92). Traced by direct manipulation, not inference: the detector
  fires `true` on this data, and the 1,529 resulting match tokens
  landing past `WINDOW` (lengths 60-81 bytes) are confirmed genuine
  recurrences, not hash collisions — the same "detector right, gate
  still costs bits" shape S2-A93 diagnosed for `access_log`, but this
  time not explained by the seed/search window mismatch S2-A93 fixed
  (both are already bound to the same window here). Mechanism not
  isolated further: whether these far matches displace cheaper local
  matches/reps outright, or just carry bucket 20's flat 20-raw-bits
  price (paid identically whether the match lands 1 byte or 1 MiB past
  `WINDOW`) for too little length to earn it, needs a slice of its own.
  `research/corpus/POLICY.md`'s accept bar is per-case by this lead's
  own precedent (S2-A93: "every detector=false case stayed exactly at
  zero"); a new real regression on a previously-untested train kind
  fails it regardless of the aggregate direction. `encode_tokens` stays
  unwired from the adaptive gate. Remaining S1-P4 scope: isolate why
  `Base64Wrapped`'s confirmed far matches net-lose before either adding
  a fourth detector condition or accepting that closing this lead needs
  gating on a kind's own local-match density, not just a byte-content
  anchor. | Measured with a throwaway, uncommitted scratch binary
  (`bench/src/bin/adaptive_window_wiring_experiment.rs`, deleted after
  this measurement, same convention as S2-A93's own).
  `research/progress.jsonl` it146.
- S2-A95 | ACCEPTED | S1-P4's own remaining scope after S2-R11: isolate
  why `Base64Wrapped(train)`'s confirmed far matches net-lose, left open
  as "mechanism not isolated further" by S2-R11. Standalone token-level
  diagnostic, no wiring change: ran both `lz::parse_optimal_with_window(data,
  WINDOW)` and `lz::parse_optimal_adaptive_window(data)` on S2-R11's own
  case (`base64_wrapped`, 4,000,000 bytes, train seed
  `0xC0FFEE123456789A`), then classified each of the adaptive parse's
  1,529 far-distance `Match` tokens (`distance > WINDOW`) by what the
  `WINDOW`-capped parse chose at the identical byte position: 484 (32%)
  displaced an active `Token::Rep`, 581 (38%) displaced an in-window
  `Token::Match`, 464 (30%) displaced a literal run. Of the 484
  rep-displacements, 463 (96%) are a real length win over the rep they
  replaced (median far length 60 bytes vs median displaced-rep length
  30), not a tie or a shorter trade — ruling out "same length, needlessly
  pricier" as the mechanism, and S2-R11 already ruled out detector error
  (the far matches are real, non-colliding recurrences). Traced instead
  to `relax_match_candidate`'s own fixed distance-bucket tax: any
  `distance > WINDOW` lands in `bucket` 20 (`ADAPTIVE_WINDOW` is
  `2^21 - 1`, one bucket wide), so `offset_cost` pays `extra_bits(20)`
  (20 raw bits) plus `EXTRA_HEADER_BITS_PRICE` (1.6) on top of the
  modeled offset-symbol price — a tax paid identically whether the match
  lands 1 byte or 1 MiB past `WINDOW` — while `relax_rep_candidates`
  prices the same-slot continuation with no distance cost at all. The DP
  is optimal against its own reseeded ideal price table; the real coder
  still nets a loss because that fixed tax, amortized over roughly 2x
  more literal-equivalent bytes at base64's near-uniform alphabet, does
  not out-earn a rep's near-zero marginal cost 484 times over. Confirms
  a pricing/gating mechanism, not a detector one: rules out a fourth
  detector-confirmation condition as this lead's next move and points at
  gating or pricing that accounts for what a far match displaces, not
  just whether it is a real recurrence. | Measured with a throwaway,
  uncommitted scratch binary (`bench/src/bin/adaptive_window_base64_diag.rs`,
  deleted after this measurement, same convention as
  S2-A93/S2-A94/S2-R11's own). No new bpb number: this diagnostic
  classifies S2-R11's already-recorded regression rather than measuring a
  new candidate; `research/progress.jsonl` records it as `kind: "patch"`
  with null bpb deltas, the same shape as S2-A89/S2-A91/S2-A92/S2-A94
  (standalone, not wired, no champion to diff against). Full record:
  `research/progress.jsonl` it147.
- S2-A96 | ACCEPTED | S1-P4's own remaining scope after S2-A95: before
  re-testing S2-R9's own rejected seed/search window mismatch on
  `base64_wrapped`, exposed
  `lz::parse_optimal_with_seed_and_search_window` as `pub(crate)` (was
  private to `lz.rs`) and added `codec::
  compressed_len_with_seed_and_search_window(data, seed_window,
  search_window)`, the real-`Encoder` counterpart to
  `compressed_len_with_window`/`compressed_len_adaptive_window` that
  exposes both windows as independent parameters instead of only the two
  matched policies those two already wire. `encode_tokens` itself is
  untouched, so `compress`/`decompress` behavior does not change.
  Standalone capability, same shape S2-A94 took for the matched-window
  case. | 1 new unit test (375 lib tests, up from 374):
  `compressed_len_with_seed_and_search_window(data, WINDOW, w)` matches
  `compressed_len_with_window(data, w)` exactly, the identity
  `parse_optimal_with_window`'s own contract guarantees. `cargo x check`:
  4 stages green. `baseline_gate check`: 11 cases, no regression (nothing
  wired). | No bpb measurement of its own: infra enabling the next
  slice's measurement, same reason as S2-A1/S2-A50/S2-A61/S2-A94;
  `research/progress.jsonl` records it as `kind: "patch"` with null bpb
  deltas. Full record: `research/progress.jsonl` it148.
- S2-R12 | REJECTED | S1-P4's own remaining scope after S2-A96: S2-A95
  traced `Base64Wrapped(train)`'s far-match net-loss to the fixed
  bucket-20 distance tax outearning a rep's near-zero marginal cost, but
  left open whether that tax is itself inflated by the price table's own
  self-reinforcement — the seed pass and every `dp_round` round share the
  SAME window under `parse_optimal_adaptive_window`'s matched-window
  contract (S2-A93), so a round-1 far match (chosen by
  `parse_greedy_with_window`, not cost-aware) cheapens bucket 20's
  reseeded price for rounds 2 and 3, entrenching more far matches whether
  or not they earn their keep. Hypothesis: decoupling the seed pass from
  the search window (S2-R9's own original, since-rejected mismatch:
  seed capped at `WINDOW`, `dp_round` searching `ADAPTIVE_WINDOW`) starves
  that reinforcement loop of its round-1 seed and should recover most, if
  not all, of `Base64Wrapped(train)`'s loss. | Measured with a
  throwaway, uncommitted scratch binary
  (`bench/src/bin/adaptive_window_seed_ablation.rs`, deleted after this
  measurement, same convention as S2-A93/S2-A94/S2-R11/S2-A95's own),
  using S2-A96's new capability: `base64_wrapped(4_000_000,
  0xC0FFEE123456789A)`, baseline `compressed_len_with_window(data,
  WINDOW)` = 235,024 bytes (0.470048 b/B, reproducing S2-R11's own number
  exactly, confirming this harness measures the identical thing), matched
  (`compressed_len_with_seed_and_search_window(data, ADAPTIVE_WINDOW,
  ADAPTIVE_WINDOW)`) = 237,961 bytes (0.475922 b/B, **+0.005874**,
  reproducing S2-R11's own regression exactly), mismatched
  (`compressed_len_with_seed_and_search_window(data, WINDOW,
  ADAPTIVE_WINDOW)`) = 235,826 bytes (0.471652 b/B, **+0.001604**). Same
  capability against both sealed-only kinds at the same length and
  `sealed_seed` of the same key, as a fresh cross-check rather than a
  reference to a different slice's numbers: `access_log` mismatched
  +0.000190 (matched -0.001108, both reproducing S2-R9's +0.000189 and
  S2-A93's -0.001109 to the fifth decimal place) and `gradient_image`
  mismatched +0.000092 (exactly reproducing S2-R9's own unconditional-bump
  number, itself `ideal_cost_bits_with_window`'s seed-always-`WINDOW`
  contract — S2-R9's "unconditional bump" was already the mismatched
  policy throughout this lead, never the matched one); matched forces
  +0.056180 on `gradient_image`, far worse than either, confirming why
  `parse_optimal_adaptive_window`'s detector gate declining to fire on it
  at all (S2-A93: "every detector=false case stayed exactly at zero")
  is load-bearing, not incidental. **Rejected**: the mismatch recovers
  most of `Base64Wrapped(train)`'s loss (+0.005874 -> +0.001604, roughly
  73% smaller) but does not flip it to an improvement, and it regresses
  both sealed-only kinds versus their matched numbers (`access_log`
  +0.000190 vs -0.001108, `gradient_image` +0.000092 vs the gate's actual
  0.0) — failing this lead's own per-case accept bar on every axis
  measured. Confirms the self-reinforcement mechanism is real and
  explains most, not all, of S2-A95's diagnosed tax: a genuine residual
  (~0.0016 b/B, roughly a quarter of the original loss) survives even
  with round-1's price table blind to the far match, so bucket 20's tax
  outearning a rep's cost is not solely an artifact of adaptive
  reseeding. No single global seed-window policy serves every case:
  matched is strictly better for every previously-accepted detector=true
  case (and for the gate itself, since it is what the gate actually
  runs), mismatched is merely less-bad for `Base64Wrapped` alone. Closes
  the seed-window lever as a viable fix for this lead: the seed/search
  mismatch stays rejected globally (S2-R9 stands), and
  `parse_optimal_adaptive_window`'s matched-window contract is unchanged.
  Remaining S1-P4 scope: the residual bucket-20-tax-vs-rep-cost gap
  S2-A95 named is now quantified (~0.0016 b/B on this case) and confirmed
  independent of seed reinforcement; a pricing or gating change that
  accounts for what a far match displaces — not a fourth detector
  condition, not a seed-window policy — is what is owed next.
  `research/progress.jsonl` it149.
- S2-A97 | ACCEPTED | S1-P4's own remaining scope after S2-R12: the
  pricing/gating fix S2-R12 named — weighing a far match against the rep
  continuation it displaces — needs to know that continuation's length
  before it can price anything, and nothing in `dp_round`'s current shape
  exposes it without a fresh scan. Standalone primitive, no wiring:
  `lz::best_active_rep_len(data, i, reps, rep_carry)` factors the
  per-slot `rep_match_len` scan `relax_rep_candidates` already runs into
  a reusable query, the single longest active rep continuation at `i` (or
  `None` below `MIN_REP_LEN`), reusing the same carry so a call across
  consecutive positions on one run stays O(1) — the same issue #179
  hazard that makes `best_rep`'s plain `match_len` loop safe only for
  `parse_greedy`'s once-per-token caller, not a per-position one.
  `#[allow(dead_code)]`, justified inline: `RepCache` is `pub(crate)`, so
  this can't dodge `dead_code` the way `lib.rs`'s doc-hidden `pub`
  modules do for a whole unwired research primitive (`src/lib.rs`'s own
  comment on that pattern, S2-A2). Deliberately leaves the actual wiring
  decision open: how much of a length edge a far match needs over the rep
  it displaces to be worth bucket 20's fixed tax is exactly the kind of
  threshold S2-R3/S2-R5 warn against picking by eye rather than
  measuring. | 4 new unit tests (380 lib tests, up from 376): no cached
  distance reaches back at `i = 0`;
  the one matching cached distance on a planted 5-byte repeat; the
  longest of two matching distances at the same position (not just
  whichever slot is checked first — the losing distance's shorter length
  is independently verified); a length-1 match (below `MIN_REP_LEN`)
  filtered to `None`. `cargo x check`: 4 stages green. `baseline_gate
  check`: 11 cases, no regression (nothing wired). | No bpb measurement:
  standalone capability, not yet a priced candidate — `research/
  progress.jsonl` records this as `kind: "patch"` with null bpb deltas,
  same reason as S2-A50/S2-A89/S2-A94/S2-A96. Remaining S1-P4 scope: wire
  this into `relax_match_candidate` behind a length-edge threshold and
  measure against S2-R12's own residual (~0.0016 b/B on
  `Base64Wrapped(train)`), or accept the large-file-mode alternative if
  no threshold clears every prior slice's per-case accept bar.
  `research/progress.jsonl` it150.
- S2-R13 | REJECTED | S1-P4's own remaining scope after S2-A97: wired
  `best_active_rep_len` into `relax_match_candidate` exactly as S2-A97
  named — a beyond-`WINDOW` match distance (only reachable through
  `parse_optimal_adaptive_window`'s matched-window policy, never the
  wired `compress`/`encode` path, since the wired search never exceeds
  `WINDOW`) is skipped entirely when an active rep exists at the same
  position and the match's real length does not clear that rep's length
  by a new `FAR_MATCH_REP_LEN_EDGE` constant, gating on the exact
  displacement S2-R12 named. | Swept `FAR_MATCH_REP_LEN_EDGE` at 0, 4, 8,
  16, 24 bytes with a throwaway, uncommitted scratch binary
  (`bench/src/bin/far_match_edge_sweep.rs`, deleted after this
  measurement, per S2-R9/S2-A93/S2-R12's own convention), same corpus and
  seeds as those three (`base64_wrapped`/`markov_h8_2_trap`/
  `sqlite_like_records`/`x86_dense_code`/`json_records` at 4,000,000
  bytes, train seed `0xC0FFEE123456789A`; `access_log`/`gradient_image`
  at `sealed_seed` of that key; `long_range_repeat` at both). Sanity-
  checked the harness before sweeping by forcing `active_rep_len` to
  `None` (gate never fires): every case reproduced S2-A93/S2-R12's own
  published numbers to five decimal places
  (`base64_wrapped` +0.005874, `access_log` -0.001108, `json_records`
  -0.000384, `long_range_repeat` -0.021481/-0.021422 train/sealed, every
  detector-false case exactly 0.0). | **Rejected**: the gate does not
  discriminate S1-P4's two outcomes by length-over-displaced-rep alone.
  `Base64Wrapped(train)`'s regression barely moved across the entire
  swept range (edge 0: +0.005874, edge 4: +0.005886 — briefly *worse*,
  a DP-reachability side effect of removing an option rather than a
  mispriced one — edge 8/16: +0.005878, edge 24: +0.005800, recovering
  at most ~4.6% of S2-R12's ~0.0016 b/B residual at the largest edge
  measured), while `access_log`'s already-accepted matched-window win
  eroded well before that: unchanged at edge 0/4 (-0.001108), then
  -0.001020 at edge 8 and -0.000910 at edge 16, roughly half its win
  gone. `json_records` moved by 0.00001 b/B across the same range
  (-0.000384 to -0.000394), noise-scale and non-monotonic with edge, not
  this lead's target either way; `long_range_repeat` (both seeds) stayed
  fixed at its S2-A93 numbers through edge 16, the only tested case
  confirming the gate can leave a genuine long-range win completely
  alone. Edge 24's own `access_log`/`json_records`/`long_range_repeat`
  values were not collected: the sweep was stopped once the trend
  through edge 16 was unambiguous, an honest gap in this record rather
  than a claim about edge 24's full case set. Mechanism: `access_log`'s
  one real far-match win (S2-A93's confirmed 41-byte IP/preamble
  recurrence) and `base64_wrapped`'s bad far matches (S2-A95's classified
  rep-displacements) evidently overlap in length-over-active-rep profile
  — a single fixed edge that suppresses enough of the latter to matter
  also suppresses some of the former, faster than it recovers anything.
  Generalizes S2-R12's own "no single global policy serves every case"
  finding from the seed-window lever to this one: a length-only
  displacement gate is not the differently-shaped pricing/gating change
  this lead still owes; separating a far match worth its bucket-20 tax
  from one that is not needs a signal this DP does not yet compute
  (plausibly how many future positions the displaced rep slot would
  still have served, not just its length at the one position it is
  compared at). No source under `src/` changed: `lz::best_active_rep_len`
  was wired into `relax_match_candidate` and `dp_round` for this
  measurement, then reverted in full (`git checkout -- src/lz.rs`),
  matching S2-D4/S2-D5's own convention for an investigation that finds
  no honest slice worth keeping. `research/progress.jsonl` it151.
  Remaining S1-P4 scope: the ~0.0016 b/B residual on
  `Base64Wrapped(train)` is still open; both the seed-window lever
  (S2-R12) and a length-based displacement gate (this entry) are now
  closed, so the next idea needs a signal neither used.
- S2-A98 | ACCEPTED | S1-P4's own remaining scope after S2-R13: tested
  the signal S2-R13's own closing paragraph named — "how many future
  positions the displaced rep slot would still have served, not just
  its length at the one position it is compared at" — before wiring
  anything. Standalone token-level diagnostic, no `src/` change: for
  every far-distance (`distance > lz::WINDOW`) `Token::Match` the
  adaptive parse chose where the `WINDOW`-capped parse's token covering
  that same byte position was a `Token::Rep` (S2-A95's own
  classification, extended to cover a far match starting mid-token, not
  only at a small-parse token boundary — a boundary-only version of this
  same classification reproduced the literal bucket exactly (464) but
  undercounted match (372 of 581) and rep (196 of 484) by conflating
  "no small-parse token starts here" with "no small-parse token covers
  here"), checked whether the displaced rep's distance still
  matches at least 2 bytes starting exactly where the far match ends.
  Ran across every case this lead has measured a real accept/reject
  verdict on: `base64_wrapped` (train, the bad case), `access_log`
  (sealed, the one genuine win), `json_records` (train, also an
  accepted win), `long_range_repeat` (both seeds, the strongest genuine
  win — zero far matches displace a rep there, the repeat is reached
  from a cold cache). **Falsified**: `base64_wrapped`'s bad far matches
  leave future rep service on the table 27.7% of the time (134 of 484);
  `access_log`'s genuinely-accepted far matches do so *less* often,
  5.1% (4 of 78), and `json_records`'s own accepted win does so *more*
  often than either, 47.8% (66 of 138). A signal that tracked "far match
  is bad" would show `base64_wrapped` highest and the two accepted wins
  lowest; the actual ordering is `access_log` (accepted) <
  `base64_wrapped` (rejected) < `json_records` (accepted), no monotonic
  relationship to accept/reject status at all. Mechanism: this
  falsifies "far match is bad because it cuts off a rep that still had
  more to give" as a per-case story. S2-A95's own diagnosis stands
  unchallenged instead — the fixed bucket-20 distance tax (20 raw bits
  plus `EXTRA_HEADER_BITS_PRICE`) is an amortized-cost problem, paid
  identically whether the displaced rep was about to run dry or not, so
  whether it *would* have kept serving is the wrong axis to gate on.
  Remaining S1-P4 scope: both signals S2-R12/S2-R13/this entry tried
  (seed reinforcement, raw displaced length, future displaced service)
  are closed; what is left is either a pricing model that actually
  amortizes bucket 20's fixed tax against expected reuse of the *new*
  distance a far match introduces (an untried angle: a far match's
  price should fall the more likely its own distance is to recur, the
  same reasoning `relax_rep_candidates` already gives an existing rep,
  which nothing in `relax_match_candidate` currently does) or the
  large-file-mode alternative. | Measured with a throwaway, uncommitted
  scratch binary (`bench/src/bin/far_match_future_service_diag.rs`,
  deleted after this measurement, same convention as
  S2-A93/S2-A94/S2-R11/S2-A95/S2-R12/S2-R13's own). Harness
  cross-checked against S2-A95's own published counts before adding the
  future-service check: reproduced 1,529 total far-distance tokens,
  464/581/484 literal/match/rep displacement split, exactly. | No bpb
  measurement: this diagnostic classifies existing parses rather than
  measuring a new candidate; `research/progress.jsonl` records it as
  `kind: "patch"` with null bpb deltas, the same shape as
  S2-A89/S2-A91/S2-A92/S2-A94/S2-A95. Full record: `research/
  progress.jsonl` it152.
- S2-R14 | REJECTED | S1-P4's sole remaining pricing angle after S2-A98,
  named there as untried: amortize a fresh match's distance tax against
  the expected reuse of the *new* distance that match introduces, so a
  far match's price falls the more likely its own distance is to recur,
  "the same reasoning `relax_rep_candidates` already gives an existing
  rep, which nothing in `relax_match_candidate` currently does."
  Implemented as `lz::distance_reuse_count(data, end, distance)`, a
  bounded forward walk over the `REUSE_LOOKAHEAD` positions past a
  match's end counting how many times that distance matches again for at
  least `MIN_REP_LEN` bytes (capped at `MAX_COUNTED_REUSES`; the walk
  advances by at least one position per step and stops at the lookahead,
  so it is O(1) per DP position, not O(remaining data)), with
  `relax_match_candidate`'s `offset_cost` divided by `1 + reuses`. Taken
  once per position from the full match's end and applied to every
  `LENGTH_STEPS` breakpoint, since the quantity estimated is a property
  of the distance and its neighbourhood, not of where the DP cuts the
  match. Encoder-only by construction: prices feed `dp_round`'s ranking
  and never the token stream's own bookkeeping, so `replay`/`decode` are
  untouched and any count whatsoever still yields a round-trippable
  parse, confirmed (not assumed) by a `decompress(compress(x)) == x`
  assertion on all 13 buffers at every measured point (169 round-trips
  across the grid below, none failed).
  Measured on `bench::baseline`'s 11 train-tier cases and the two
  sealed-only kinds (`access_log`, `gradient_image` at
  `sealed_seed(CASE_SEED)`), `CASE_LEN` 50,000, real bitstreams through
  `mothergod::compress`, baseline and candidate from the same binary on
  the same host so the per-case deltas are paired. **Train mean
  +0.144495 b/B** at `REUSE_LOOKAHEAD` 64 / `MAX_COUNTED_REUSES` 3:
  `entropy_ladder_h4` +0.384960, `x86_dense_code` +0.308960,
  `markov_h8_2_trap` +0.223680, `entropy_ladder_h2` +0.223520,
  `base64_wrapped` +0.134080, `json_records` +0.123040,
  `sqlite_like_records` +0.109760, `entropy_ladder_h1` +0.080640,
  `entropy_ladder_h6` +0.000800, `entropy_ladder_h8` and
  `interleaved_audio16` exactly flat. Sealed: `access_log` +0.089760,
  `gradient_image` flat. **Not a tuning failure.** Swept the full 4x3
  grid (`REUSE_LOOKAHEAD` 8/16/64/256 x `MAX_COUNTED_REUSES` 1/2/3) on
  train only, as `research/corpus/POLICY.md` permits: no case improved
  at any of the twelve points, and the damage is monotone in the
  discount at fixed lookahead (`entropy_ladder_h1` +0.023520 / +0.051520
  / +0.080640 at lookahead 64 for cap 1 / 2 / 3). The gentlest point in
  the grid, lookahead 8 with cap 1, a discount of at most 2x looking
  8 bytes ahead, still costs **+0.085804 b/B** train mean. Dose-response
  in the wrong direction across a grid with no improving cell is the
  signature of a wrong rule, not an undertuned one.
  Mechanism, measured rather than reasoned: a token-mix diagnostic over
  `parse_optimal`'s output, same cases, baseline vs candidate. The
  discount converts rep tokens into fresh matches, which is precisely
  backwards. `json_records`: `Token::Match` 415 -> 1,208 (2.9x),
  `Token::Rep` 1,483 -> 1,003, rep-covered bytes 33,343 -> 19,621.
  `sqlite_like_records`: match 971 -> 4,955 (5.1x), rep 3,636 -> 1,363,
  rep bytes 20,178 -> 2,737. `base64_wrapped`: match 369 -> 1,120, rep
  1,316 -> 664. `access_log`: match 1,379 -> 1,921, rep 152 -> 26. The
  four cases whose distances genuinely recur, exactly the four the
  reuse signal fires hardest on, are the four that shed the most free
  rep coverage. `entropy_ladder_h1` states it most sharply: total tokens
  *fell* 5,945 -> 3,984 (literals 3,198 -> 571 absorbed into longer
  matches) while bits/byte *rose* 6.4%, so the parse got shorter in
  tokens and more expensive in bits. On `json_records` the +0.123040
  b/B is 6,152 bits over 793 extra match tokens, ~7.8 bits each, the
  same order as a fresh distance at this data's bucket range (10-12 raw
  bits plus modeled price plus `EXTRA_HEADER_BITS_PRICE`) net of the
  literals and reps those matches absorbed.
  So the rebate double-counts a saving that already exists. Every reuse
  the lookahead counts is a rep the DP *already* prices at near-zero
  where it actually occurs, via `relax_rep_candidates`. Crediting the
  introducing match for those same reps books the saving twice: once at
  the cheap rep token that really earns it, and again as a discount on a
  token that will pay the full distance on the wire. `dp_round`'s price
  then stops estimating emitted bits, so its shortest path stops being
  the shortest bitstream, and it buys that mispriced match by spending
  the very rep tokens whose existence justified the discount. This also
  corrects the framing S2-A95 bequeathed to this lead: the asymmetry it
  measured (a match pays for its distance, a following rep does not) is
  *correct accounting*, not a mispricing. The distance genuinely costs
  what it costs, once, at the token that emits it, and the reps that
  follow genuinely cost near nothing. The DP was right; four slices
  (S2-R12, S2-R13, S2-A98, this one) have now looked for a defect in a
  number that was never wrong.
  Encode time also rose, modestly and as designed-for rather than as a
  surprise (the lookahead is O(1) per position but not free):
  compress-plus-decompress wall clock at the primary point grew
  `markov_h8_2_trap` 0.235s -> 0.299s and `x86_dense_code` 0.176s ->
  0.226s; at lookahead 256, `json_records` 0.116s -> 0.181s.
  Candidate code (`distance_reuse_count`, `REUSE_LOOKAHEAD`,
  `MAX_COUNTED_REUSES`, and `relax_match_candidate`'s `data` parameter
  and divided `offset_cost`) reverted in full, per the
  `compression-experiment` skill; measured with throwaway, uncommitted
  scratch binaries (`bench/src/bin/reuse_amortization_measure.rs`,
  `bench/src/bin/reuse_token_diag.rs`, a sweep driver), deleted after
  measurement, the same convention as S2-A93 through S2-A98. One
  incidental honesty note, which is why the baseline column above is
  measured rather than read from the committed file: this host measures
  `entropy_ladder_h6` at 6.178240 against `bench/baseline.json`'s
  6.1792, a 0.00096 b/B *improvement* some past encoder change earned
  without regenerating the file. `baseline_gate check` is green on a
  clean tree, correctly: it fails on regression past `TOLERANCE_BITS`
  and has no reason to chase an improvement. But a paired comparison
  against a stale column would have credited this candidate 0.00096 b/B
  it did not earn. `research/progress.jsonl` it153. Remaining S1-P4
  scope: with reuse amortization closed, no pricing or gating signal
  proposed for this lead since S2-R11 survives contact. What is left is
  the large-file-mode alternative (ROADMAP M5 territory, a deliberate
  mode distinct from the default rather than a parse heuristic), or
  accepting that far-match residue on this data is not a pricing
  problem at all.
- S2-A99 | ACCEPTED | S1-P2's own remaining scope after S2-R10: its own
  closing paragraph named two branches once both the intra-round pricing
  angle and the match-finder search-quality angle closed — "a modeling
  primitive that reaches structure a binary-tree parse cannot (e.g. a
  schema-aware/typed-field literal model, in the spirit of S1-P5's
  per-column direction but for pre-transpose or mixed-type records)" or
  accepting the ceiling. Also weighed against S1-P8 (GLN-style
  predictors/more experts), unblocked since S1-P1's SSE resolution but a
  one-line lead with zero design decisions recorded anywhere in this
  repo about what a gated linear network should look like here: a first
  slice there would have meant inventing the gating architecture from
  scratch under this session's time budget, real risk of the "half
  understood" failure this lead's own selection criteria warn against.
  S1-P2's own named branch, by contrast, already has a worked precedent
  (S1-P5's column expert) and a concretely specified target
  (sqlite/json/jsonl), so it is the better-grounded pick. New module
  `src/fieldtype.rs`: `FieldClass` (`Whitespace`/`Structural`/`Numeric`/
  `Text`), `FieldState` (`in_quotes`, `escaped`, `class`), and
  `FieldState::advance` folding one byte at a time into the next state —
  the same "cheap rolling state, no lookahead" shape
  `literal::advance_word_hash` already uses for its own alnum-run signal,
  applied to JSON-shaped syntax instead: outside a quoted string, a byte
  classifies as `Structural` (`{ } [ ] : , "`), `Whitespace`
  (` \t\n\r`), `Numeric` (`- . 0-9`), or `Text` (everything else);
  inside a quoted string every byte is `Text` regardless of shape (a
  digit or brace inside a string is string content, not a numeric or
  structural field), with `escaped` absorbing the byte immediately after
  an unescaped `\` unconditionally so `\"` never closes the string and
  `\\` never leaves a dangling escape. `field_bank` maps a `FieldState`
  into a fixed `0..FIELD_BANKS` (8: four classes times in/out of quotes,
  so quoted-string `Text` and unclassified `Text` occupy different banks)
  the same way `column::column_bank` wraps `column_of`'s unbounded result
  — moot for overflow here since every input is already bounded by
  construction, kept as the one named constant a future bank-sizing
  caller reads instead of re-deriving `4 * 2`. Hypothesis this slice
  itself does not test: unwired, so no bits/byte claim. Standalone,
  same "first slice, not yet wired" shape as S2-A42/S2-A57/S2-A61/S2-A64:
  17 unit tests (structural/whitespace/numeric/text classification
  outside quotes, digits and braces reclassified to `Text` inside
  quotes, single and double backslash escape handling, a realistic JSON
  record classified byte-by-byte end to end, the bank-space bound, and
  that `field_bank` is a pure function of `(class, in_quotes)` alone, not
  of byte history). `cargo x check`: 4 stages green. `baseline_gate
  check`: 11 cases, no regression (nothing wired). Remaining S1-P2 scope:
  the ideal-cost pairing S2-A69's methodology used for the column
  expert's own equivalent question — blend a field-class-keyed eighth
  expert into the shipped six under `sqlite_like_records`/`json_records`
  and the sealed set, before any real wiring — still unbuilt.
  `research/progress.jsonl` it154.
- S2-R15 | REJECTED | S1-P2's own remaining scope after S2-A99: the
  ideal-cost pairing S2-A69's methodology used for S1-P5's column expert
  (`Literal::ideal_cost_bits_column_expert_pair`), applied to S2-A99's
  field-class classifier instead. Hypothesis: blending a
  `fieldtype::field_bank(field_state)`-keyed expert into `Literal`'s
  six-expert mix as a seventh, where `field_state` is the
  `fieldtype::FieldState` folded over every byte strictly before the one
  being priced (never the byte itself — folding that in would leak the
  answer being priced into its own context), reduces ideal-cost bits/byte
  on `bench::baseline`'s `sqlite_like_records`/`json_records` cases
  without regressing the sealed set. New `Literal::
  ideal_cost_bits_fieldtype_expert_pair` and `FieldExpertState`: same
  shape as S2-A69's `ideal_cost_bits_column_expert_pair`/
  `ColumnExpertState` before their own SSE field landed (deliberately
  pre-SSE, same scoping note S2-A69 gave — a separable question for a real
  wiring slice, not this one), reusing this codebase's own
  already-extracted `fixed_point_scale`/`adapt_weight`/
  `expert_estimates`/`crate::rescale_bank` helpers rather than duplicating
  their logic inline the way S2-A69's original commit had to before its
  own later deslop (#396) pulled them out. `codec::
  ideal_cost_bits_fieldtype_expert_experiment` pairs this against the
  shared flag/length/offset/slot costs the same way `CostSink` does, via a
  `FieldExpertCostSink` that folds `field_state` incrementally
  (`bank_before`): since `walk_tokens` visits literal and copy positions
  in strictly increasing `context.position` order and a copy token's
  replayed bytes are always `data`'s own bytes at that span, folding the
  span since the last position visited straight from `data` needs no
  separate per-copy-byte hook on `TokenSink`, unlike `column::column_of`'s
  pure-position formula which needed no fold at all. | Measured
  (model-cost, not real-bitstream; `bench` crate generators; via an
  uncommitted scratch driver, `bench/src/bin/
  scratch_fieldtype_expert_experiment.rs`, deleted after this measurement,
  same convention as every prior scratch driver): train, the same
  rotated-window shape S2-A69/S2-R8 used (150,000 generated bytes,
  50,000-byte window at offset 50,000, train seed
  `0xC01D_BEEF_1234_5678`) — `sqlite_like_records`: baseline 4.070786 ->
  with-field 4.069077 bpb, **-0.001709**. `json_records`: baseline
  0.604557 -> with-field 0.603621 bpb, **-0.000936**. Sealed, both
  sealed-only kinds (`DatasetKind::sealed_only`) at `CASE_LEN` 50,000,
  `sealed_seed` of the same train seed, not rotated — measured both,
  unlike S2-A69's own column expert, which measured only `gradient_image`
  because its `columns` parameter had no principled value for
  `access_log`'s line-oriented, non-fixed-width format; this classifier
  needs no such parameter, so both are equally applicable, the same
  "measure both sealed-only kinds when the mechanism applies
  unconditionally" practice S2-R6/S2-R7/S2-A63 followed: `access_log`
  baseline 0.914719 -> with-field 0.910645 bpb, **-0.004075** (an
  improvement). `gradient_image` baseline 6.059044 -> with-field
  6.060642 bpb, **+0.001598** (a regression). Train improved on both
  named targets and one sealed-only kind improved, but `gradient_image`
  regressed — corpus policy's accept rule (train improvement AND no
  validation regression) fails, the same binary reading S2-R3/S2-R4/S2-R7
  established: "no validation regression" draws no tolerance line for
  magnitude, and a carve-out here that those entries did not get would be
  tuning the accept rule against the outcome, not applying it. |
  Mechanism: `gradient_image` is raw gradient pixel data with no
  JSON-shaped syntax at all — `FieldState::advance`'s
  quote/structural/numeric/whitespace classes have nothing real to key on
  there, so the bank a given position lands in is close to arbitrary
  (occasional digit-valued bytes land in the `Numeric` bank by coincidence
  of their numeric byte value, not because the data has numeric-literal
  structure), and blending that near-noise in as a seventh expert costs a
  small but real, not-fully-suppressed mixing tax even though the mixer is
  free to downweight it — the same "the mixer is free to downweight it
  where it is not useful" framing S2-A69's own entry used to explain why
  its column expert's worst case merely broke even, not a guarantee every
  case breaks even. The two named train targets (`sqlite_like_records`,
  `json_records`) and `access_log` (log-line text with quoted path
  segments and bracketed timestamps, JSON-adjacent enough for the
  classifier to track something real) all improved, consistent with the
  signal being genuine on data that actually has the syntax this
  classifier reads — but S1-P2's own selection criteria, and this
  project's corpus policy, both require every measured sealed case to
  hold, not just the ones the hypothesis was aimed at. Candidate code
  (`FieldExpertState`, `Literal::ideal_cost_bits_fieldtype_expert_pair`,
  their four unit tests, `codec::FieldExpertCostSink`/
  `ideal_cost_bits_fieldtype_expert_experiment`, their two unit tests, the
  scratch driver) reverted in full, per the `compression-experiment`
  skill's "delete rejected candidate code"; `fieldtype::FieldState`/
  `field_bank` themselves (S2-A99) are unaffected, standalone and unwired,
  same status as `column::column_of`/`column_bank` after S2-R8.
  `research/progress.jsonl` it155.
- S2-R16 | REJECTED | S1-P3's own remaining scope (`src/ppm.rs`'s module
  doc, restated in S2-A57/S2-R6's own text): the third of its three named
  fallback-target candidates, "a fresh dedicated table," after S2-R6
  rejected the first (order-0's context-free global marginal) and ruled
  out the second (any of `Literal`'s other five experts, on the reasoning
  that a context-specific bank is exactly as likely to be sparse as
  whichever one is escaping). Before spending a real `Ppm`/`Method`/
  `FORMAT_VERSION` wiring slice, measured the same before-wiring ideal-cost
  pairing S2-R6/S2-A69 both used. `NibbleFallback` (`src/literal.rs`): a
  new, dedicated 16-context table keyed only by the previous byte's high
  nibble — deliberately coarser than every one of `Literal`'s six banks
  (the sparsest, the alignment expert, still has 64), so it converges on
  real local structure faster than any of them while still respecting
  *some* context, unlike order-0's total blindness.
  `Literal::ideal_cost_bits_nibble_fallback_pair` prices a literal byte
  twice from the same pre-update six-expert state — once as
  `Self::ideal_cost_bits` exactly, once with any expert whose own bank has
  never observed this symbol beyond its initial Laplace floor (`freq ==
  1`) substituting an *effective frequency* rescaled from
  `NibbleFallback`'s own estimate onto that expert's own bank total —
  updating `self` from the real frequencies exactly once (the shared-
  trajectory principle S2-A69/S2-R6 both used) and `NibbleFallback`
  unconditionally, every literal byte. Deliberately reuses `Literal::mix`'s
  exact two-pass fixed-point arithmetic rather than a hand-rolled
  probability-space recomputation the way an earlier draft of this slice
  first tried: `mix` adds its own `+1`-per-symbol fixed-point floor on top
  of every bank's already-Laplace-smoothed counts, and a separate
  floating-point reimplementation did not reproduce that second floor,
  making a "nothing is sparse, so nothing should differ from baseline"
  unit test fail by construction on a real difference of about 0.08 bits
  even with no substitution firing on the priced symbol — caught by that
  test itself before landing, not by the corpus measurement below, fixed
  by rebuilding the candidate distribution through `mix`'s own scale/`+1`-
  floor code path with only the effective-frequency source changed per
  expert, after which the same test's tolerance tightened from 1e-3 back
  to 1e-9 and passed. A whole-file `TokenSink` (`codec::
  NibbleFallbackCostSink`/`ideal_cost_bits_nibble_fallback_experiment`)
  accumulates both totals over one `lz::parse_optimal` token walk, sharing
  the identical flag/length/offset/slot costs on both sides so only the
  literal term can differ, run via an uncommitted scratch binary
  (`bench/src/bin/scratch_nibble_fallback_experiment.rs`, deleted after
  this measurement, same convention as every prior scratch driver).
  Deliberately pre-SSE, matching S2-R6's own methodology exactly (a
  separable question for a real wiring slice, not this one, per S2-A69's
  own scoping note) — `codec::ideal_cost_bits`'s own `CostSink` already
  prices literals through the SSE-calibrated `ideal_cost_bits_sse` today,
  so this measurement's absolute bits/byte run below the real shipped
  path's, the same gap S2-A69's own pre-SSE numbers had against S2-A76's
  post-SSE follow-up; only the *train/sealed accept-rule verdict* is
  claimed here, not an absolute-bpb comparison to `bench/baseline.json`.
  | Measured on `bench::baseline`'s 11 train-tier cases (`CASE_LEN`
  50,000, `CASE_SEED` 0xBA5E11E5BA5E11E5) and the two sealed-only kinds at
  `sealed_seed(CASE_SEED)`: train net **+0.034795 b/B** (5 improved, 6
  regressed) — `entropy_ladder_h1` −0.002681, `h2` −0.001457, `h6`
  −0.000639, `markov_h8_2_trap` −0.067481, `json_records` −0.002422
  improved; `entropy_ladder_h4` +0.000854, `h8` +0.080206, `base64_wrapped`
  +0.008129, `interleaved_audio16` +0.166017, `sqlite_like_records`
  +0.095198, `x86_dense_code` +0.107023 regressed. S1-P3's own named
  target, `sqlite_like_records`, moved the wrong direction again, further
  than order-0's own +0.039703 (S2-R6). Sealed: `access_log` **−0.002250**,
  `gradient_image` **−0.151653** — both improved, `gradient_image` by a
  wide margin (the same case order-0's own substitution regressed worst,
  +0.541159, S2-R6). **Rejected**: corpus policy's accept rule needs train
  improvement AND no validation regression; train net regressed, so this
  fails on the train side even though both sealed-only kinds improved — a
  different failure shape than every prior S1-P2/S1-P3/S1-P4 rejection in
  this journal, which failed on a sealed regression despite an improving
  train mean. Mechanism, traced rather than assumed: this fallback target
  does fix order-0's own specific failure (`markov_h8_2_trap` and
  `gradient_image`, S2-R6's two worst regressions, are this candidate's
  best improvements — both are cases where the global/coarse structure a
  16-bucket table can see genuinely helps over order-0's total blindness).
  But the "rescale onto this bank's own total" step needed to combine a
  differently-normalized fallback source with a sparse expert's own bank
  introduces its own bias whenever the two totals diverge, which they do
  by ordinary training, not just by substitution: the fast-rate expert's
  `FAST_INCREMENT` (32) grows its own bank total roughly 2.7x faster than
  `NibbleFallback`'s `DEFAULT_INCREMENT` (12)-driven one, so a genuinely
  never-observed symbol's rescaled "floor" drifts away from a true count
  of 1 in that expert specifically, the moment *any* other symbol has been
  observed there — confirmed directly in this slice's own unit tests (a
  50-repeat same-context fixture left every relevant bank's own total
  diverged from `NibbleFallback`'s, and a naive rescale reproduced a
  floor of 2, not 1, in the fast-rate bank alone). That drift is exactly
  the mechanism behind the regressions: `interleaved_audio16`,
  `sqlite_like_records`, and `x86_dense_code` are this project's three
  fixed-record/short-period generators, precisely the data the fast-rate
  expert is best placed to track well once trained, and precisely where a
  silently-inflated floor costs the most. Candidate code
  (`literal::NibbleFallback`, `Literal::ideal_cost_bits_nibble_fallback_pair`,
  `Literal::substituted_cost_bits`, their nine unit tests,
  `codec::NibbleFallbackCostSink`/`ideal_cost_bits_nibble_fallback_experiment`,
  the scratch driver) reverted in full, per the `compression-experiment`
  skill's "delete rejected candidate code"; `Ppm` itself (S2-A57) is
  unaffected, same basis as S2-R6. `research/progress.jsonl` it156.
  Remaining S1-P3 scope: all three of `src/ppm.rs`'s own named fallback
  candidates (order-0, another of `Literal`'s six experts, a fresh
  dedicated table) are now tried and rejected. What is left is either a
  substitution rule that does not need cross-table rescaling (so it cannot
  reintroduce this slice's own drift mechanism) or accepting that this
  lead's ceiling, absent one, sits at the same "unclear, no further named
  branch" shape S1-P2 reached after its own repeated rejections.
- S2-R17 | REJECTED | S1-P3's own remaining scope after S2-R16: a
  substitution rule that never rescales one table's estimate onto
  another's total. Revived S2-R16's own `NibbleFallback` (16 contexts
  keyed on the previous byte's high nibble) but replaced its substitution
  mechanism entirely: instead of converting `NibbleFallback`'s estimate
  into an integer "effective frequency" against the target expert's own
  bank total (S2-R16's own rounding step, diagnosed there as the drift
  source), a new `fixed_point_contribution_from_probability` plugs
  `NibbleFallback`'s own probability directly into the exact fixed-point
  unit `fixed_point_scale` already lands every real contribution in
  (`weight_fraction * probability * FIXED_POINT_SCALE`), no bank total
  of any kind entering this formula, so there is nothing for two tables'
  totals to disagree about. `Literal::ideal_cost_bits_nibble_fallback_direct_pair`
  prices a literal twice from the same pre-update state: the baseline
  side reuses `Self::mix` directly (unlike S2-R16's own first, rejected
  draft, a hand-rolled float recomputation that missed `mix`'s own
  per-symbol `+1` fixed-point floor and diverged from the true baseline
  by about 0.08 bits even with nothing substituted; this slice avoids
  that specific bug by construction, calling `mix` verbatim for one side
  of the pair); the substituted side rebuilds the identical two-pass
  `u64` scale/`+1`-floor structure, branching per `(expert, symbol)` only
  where that expert's own bank has never observed that symbol
  (`freq == 1`). Two unit tests exercise this pairing method's own
  control cases before any corpus measurement: a fresh model paired with
  a fresh, equally-uniform `NibbleFallback` must match baseline exactly
  on the very first call (both tables start in genuine agreement, so
  even though the substitution branch fires everywhere, the value it
  computes is the same number the floor already gave); a `NibbleFallback`
  trained to favor one symbol must make that symbol strictly cheaper than
  the unsubstituted floor. Both held. | Measured the same way S2-R16 did:
  `mothergod_bench::baseline`'s 11 train cases (`CASE_LEN` 50,000,
  `CASE_SEED` 0xBA5E11E5BA5E11E5) plus `access_log`/`gradient_image` at
  `sealed_seed(CASE_SEED)`, via an uncommitted scratch binary
  (`bench/src/bin/scratch_nibble_fallback_direct_experiment.rs`, deleted
  after this measurement). Train net **+0.027803 b/B** (5 improved:
  `entropy_ladder_h1` -0.002353, `h2` -0.001276, `h6` -0.000612,
  `markov_h8_2_trap` -0.053073, `json_records` -0.001710; 6 regressed:
  `entropy_ladder_h4` +0.000850, `h8` +0.057303, `base64_wrapped`
  +0.009312, `interleaved_audio16` +0.147638, `sqlite_like_records`
  +0.068857, `x86_dense_code` +0.080897). Sealed: `access_log`
  **-0.001629**, `gradient_image` **-0.155227**, both improved. **Rejected**:
  corpus policy's accept rule needs train improvement AND no validation
  regression; train net regressed, the same failure side S2-R16 fell on,
  even though the specific rounding bug S2-R16 named is provably absent
  here (the unit tests above rule it out directly, and this slice's
  formula never computes an integer frequency against a foreign total at
  all). Mechanism: the near-identical shape of this rejection to
  S2-R16's own (same 5/6 train split, same three worst regressions
  `interleaved_audio16`/`sqlite_like_records`/`x86_dense_code`, same two
  sealed-only kinds improving) despite a genuinely different substitution
  formula falsifies the hypothesis this slice was built to test: the
  drift S2-R16 diagnosed was not the real cause of those regressions, or
  at best only a minor contributor. The more likely mechanism, untested
  as its own claim: `Literal`'s six mixing weights adapt per
  weight-context assuming each expert reports its *own* honest estimate,
  including an honest Laplace floor when it has nothing to say; a
  foreign substitute in that slot, however it is computed, is a
  different number than the one the weight was calibrated against,
  and the fixed-record generators this project probes are exactly the
  data where the fast-rate expert (bank 0) most reliably earns a high
  weight once trained, making its floor entries the ones a substitute
  most disrupts. Candidate code (`literal::NibbleFallback`,
  `fixed_point_contribution_from_probability`,
  `Literal::ideal_cost_bits_nibble_fallback_direct_pair`, their seven
  unit tests, `codec::NibbleFallbackDirectCostSink`/
  `ideal_cost_bits_nibble_fallback_direct_experiment`, the scratch
  binary) reverted in full, per the `compression-experiment` skill's
  "delete rejected candidate code"; `Ppm` itself (S2-A57) is unaffected,
  same basis as S2-R6/S2-R16. `research/progress.jsonl` it157. Remaining
  S1-P3 scope: every substitution mechanism named or attempted so far
  fails on the same data regardless of its rescale arithmetic, pointing
  at the six-expert mix's own weight calibration rather than any one
  formula; what is left, if anything, is an escape signal that never
  enters a real expert's own floor at all (a separate, additive
  eighth-expert-style blend, S1-P5's own architectural shape) or
  accepting the ceiling.
- S2-A100 | ACCEPTED | Fifth slice of ROADMAP M3's third standing lead
  (S1-P3, PPM-style escape for literal contexts), S2-R17's own closing
  note: build the one untried shape, an escape signal blended as a
  genuinely additive expert rather than substituted into any of
  `Literal`'s six real experts' own banks. Hypothesis: `src/ppm.rs`'s
  `Ppm` primitive (S2-A57, accepted but unwired since), given its own
  bank space and its own adaptive mixing weight and blended into
  `Literal`'s mix as one more additive term — never touching the six
  real experts' own banks or totals — reduces ideal-cost bits/byte on
  data with genuine local structure without regressing the sealed set,
  because an additive slot's own weight can fall toward zero wherever it
  is unhelpful (the mechanism `JOURNAL` S2-A69's column expert already
  relies on), unlike a substitute that overwrites an existing,
  already-calibrated expert's own floor value (S2-R17's own diagnosed
  failure mode). New `Literal::PpmExpertState` (`src/literal.rs`): one
  `Ppm` table per bank, `PPM_EXPERT_BANKS` (16) of them, keyed by
  `PpmExpertState::bank_of` on the previous byte's high nibble alone —
  deliberately the same coarse key S2-R16/S2-R17's `NibbleFallback` used,
  reused on purpose so this slice isolates the mechanism question
  (additive vs. substitutive) from the keying question those two entries
  already answered — plus one mixing weight per `WEIGHT_CONTEXTS` key,
  same shape `ColumnExpertState` (S1-P5) uses. `Literal::mix_ppm` is
  `Self::mix7`'s own shape, but `ppm_probability` (new, private) reads a
  bank's per-symbol contribution as `0.0` when `Ppm::is_escape` is `true`
  and as the linear-space inverse of `Ppm::price_symbol`'s `-log2(p)`
  bits otherwise (`probability_from_price_bits`, the one place a caller
  outside `ppm.rs` undoes that log, since `Ppm` exposes no raw
  frequency/total pair): unlike every real expert's own bank, which
  starts Laplace-smoothed to a floor of 1, a `Ppm` bank starts every
  symbol at frequency 0, so a genuinely unobserved symbol contributes
  exactly zero mass here, never a false floor. `Literal::update_ppm_expert`
  adapts `PpmExpertState`'s own weight via the same `adapt_weight` rule
  `Self::update`'s six real weights use, restricted to this one
  component, and advances its own bank via `Ppm::observe` (Method C's own
  bookkeeping, not `crate::rescale_bank` — this bank is a `Ppm` table,
  never a plain frequency array). `Literal::ideal_cost_bits_ppm_expert_pair`
  prices a literal twice from the same pre-update six-expert state, the
  identical paired methodology S2-A69/S2-R15 both used: once as
  `Self::ideal_cost_bits` exactly (so the six real experts adapt on
  their one real trajectory regardless of this method running), once
  with `PpmExpertState` blended in via `mix_ppm`. `codec::
  ideal_cost_bits_ppm_expert_experiment` pairs this against the shared
  flag/length/offset/slot costs the same way `CostSink` does, via a new
  `PpmExpertCostSink`, run from an uncommitted scratch binary
  (`bench/src/bin/scratch_ppm_expert_experiment.rs`, deleted after this
  measurement, same convention as every prior scratch driver). 6 new unit
  tests in `literal.rs` (paired baseline matches plain `ideal_cost_bits`
  exactly; a `PpmExpertState` update touches only its own keyed bank,
  every other bank stays untouched; costs stay finite and positive over a
  mixed byte stream), 2 in `codec.rs` (zero on empty input, finite and
  positive on a real file). Deliberately pre-SSE, matching S2-A69's own
  methodology exactly (a separable question for a real wiring slice, not
  this one, per that entry's own scoping note). | Measured on
  `bench::baseline`'s 11 train-tier cases (`CASE_LEN` 50,000, `CASE_SEED`
  0xBA5E11E5BA5E11E5) and both sealed-only kinds at
  `sealed_seed(CASE_SEED)`, matching S2-R16/S2-R17's own convention: train
  net **-0.005530 b/B** (8 of 11 improved: `entropy_ladder_h1` -0.001557,
  `h2` -0.002289, `h4` -0.003062, `h6` -0.000473, `markov_h8_2_trap`
  -0.038598, `json_records` -0.003686, `interleaved_audio16` -0.007128,
  `x86_dense_code` -0.005872; 3 regressed: `entropy_ladder_h8` +0.000861,
  `base64_wrapped` +0.000031, and S1-P3's own named target
  `sqlite_like_records` +0.000944 — all roughly two orders of magnitude
  smaller than any train regression S2-R6/S2-R16/S2-R17 measured on the
  same cases). Sealed: `access_log` **-0.006145**, `gradient_image`
  **-0.176711** — both improved, `gradient_image` by the widest margin any
  S1-P3 slice has measured, the same case every rejected substitution
  mechanism also improved most when it improved at all (S2-R6's order-0
  -inapplicable there, it regressed +0.541159; S2-R16/S2-R17's
  `NibbleFallback` improved it -0.1517/-0.1552). Corpus policy's accept
  rule (train improvement AND no validation regression) passes outright,
  the first S1-P3 slice to clear it. **Accepted.** | Mechanism: a `Ppm`
  bank's frequency-0 start means it contributes nothing on data where its
  16-context key carries no real signal (`entropy_ladder_h8`'s iid noise)
  rather than a confidently wrong value the way a Laplace-smoothed or
  rescaled substitute could — the residual +0.000861 there is the
  bounded cost of one more mixing weight adapting away from an unhelpful
  signal, not a corrupted floor. More fundamentally, this slice never
  writes into any of the six real experts' own reported estimates: `Self::update`
  still calibrates their weights against their own honest numbers exactly
  as before, untouched, and `PpmExpertState`'s own weight is free to fall
  toward zero wherever it is unhelpful, the same "the mixer is free to
  downweight it where it is not useful" mechanism `JOURNAL` S2-A69's
  column expert's own entry used to explain its own worst case merely
  breaking even. That is the structural difference from S2-R6/S2-R16/
  S2-R17: S2-R17's own diagnosis named the six-expert mixer's weights as
  "tuned to expect an honest Laplace floor from a sparse expert,
  mishandling whatever a foreign substitute reports in its place
  instead" — an additive slot never puts a foreign number in another
  expert's place, so that mishandling has nothing to act on here.
  `sqlite_like_records`'s tiny regression (S1-P3's own named target,
  again moving the wrong direction, but by 0.000944 against S2-R16's
  +0.095198 and S2-R17's +0.068857 in the same case) is consistent with
  this reading: whatever residual cost an additive expert imposes on
  already-well-modeled fixed-record data is the bounded cost of adapting
  one more weight, not the unbounded cost of corrupting an existing,
  load-bearing expert's own calibration. Candidate code
  (`literal::PpmExpertState`, `literal::PPM_EXPERT_BANKS`,
  `literal::probability_from_price_bits`, `literal::ppm_probability`,
  `literal::fixed_point_contribution_from_probability`,
  `Literal::mix_ppm`/`update_ppm_expert`/`ideal_cost_bits_ppm_expert_pair`,
  their six unit tests, `codec::PpmExpertCostSink`/
  `ideal_cost_bits_ppm_expert_experiment`, their two unit tests) kept:
  this is an accepted step at the ideal-cost-pairing layer, the same
  status S2-A69's own candidate code held before its real wiring landed.
  The scratch binary that ran this measurement is not: deleted after
  recording these numbers, same as every prior scratch driver regardless
  of verdict. `src/ppm.rs`'s own module doc updated to record this
  outcome. `research/progress.jsonl` it158. Remaining S1-P3 scope: see
  the updated S1-P3 entry above (the real `Method`/`FORMAT_VERSION`
  wiring slice, an ADR, and the still-open SSE-interaction question
  S2-A76 answered for the column expert but this expert has not yet been
  asked).
