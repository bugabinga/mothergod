//! Binary decomposition of a 256-symbol cumulative-frequency table into a
//! sequence of top-down binary split decisions ("bit tree"): the shape
//! ROADMAP M3's oldest standing lead needed to calibrate a compound
//! estimate instead of a lone counter (`research/JOURNAL.md` S1-P1,
//! S2-R1's postmortem, `crate::sse`'s module docs).
//!
//! Not a port: no archive precedent (`crate::sse`'s module docs record the
//! same grep-clean result for S1-P1 generally). [`encode_symbol`]/
//! [`decode_symbol`] (the non-SSE pair) stay standalone, exercised only by
//! this module's own tests; [`encode_symbol_sse`]/[`decode_symbol_sse`]
//! are wired (`research/JOURNAL.md` S2-A60, ADR-0038,
//! [`crate::literal::Literal::encode_sse`]/`decode_sse`).
//!
//! [`encode_symbol`]/[`decode_symbol`] code one byte as 8 chained binary
//! decisions instead of [`crate::coder::Encoder::encode`]/
//! [`crate::coder::Decoder::decode`]'s single 256-way range division:
//! each step splits the current candidate symbol range `[lo, hi)` at its
//! midpoint and asks whether the true symbol falls in the upper half, at
//! the probability that split has under a caller-supplied cumulative
//! table shaped like [`crate::model::Model`]'s or
//! `crate::literal::Literal::mix`'s own output. The chain rule of
//! probability makes the product of those 8 binary probabilities along a
//! symbol's path equal `(cum[symbol + 1] - cum[symbol]) /
//! cum[ALPHABET]` exactly — the same partition, reshaped into a sequence
//! of binary decisions instead of one 256-way division. That reshaping
//! is the point: a binary decision, not a 256-ary one, is what
//! [`crate::sse::Sse`] calibrates. [`ideal_cost_bits`] checks the
//! identity directly; the round-trip tests below check it end to end
//! through the real coder.
//!
//! [`sse_context`] answers S1-P1's other named prerequisite: which
//! [`crate::sse::Sse`] context a given walk step should key on.
//!
//! [`encode_symbol_sse`]/[`decode_symbol_sse`] compose the two: the same
//! chain-rule walk as [`encode_symbol`]/[`decode_symbol`], but each level's
//! raw `cum`-derived probability is first refined through a caller-supplied
//! [`crate::sse::Sse`] table (keyed by [`sse_context`]) before it reaches
//! [`crate::coder::Encoder::encode_bit`]/[`crate::coder::Decoder::decode_bit`],
//! and the table is updated on the raw probability afterward — the shape
//! [`crate::sse::Sse`]'s own test suite already proves round-trips
//! (`calibrated_probability_round_trips_and_costs_less_than_a_fixed_split`).
//! `crate::literal::Literal::encode_sse`/`decode_sse` are the only callers,
//! closing `research/JOURNAL.md` S1-P1.

use crate::coder::{Decoder, Encoder};
use crate::sse::Sse;

/// Byte alphabet a cumulative table spans, matching
/// [`crate::literal::Literal`]'s own alphabet size.
const ALPHABET: usize = 256;

/// `log2(ALPHABET)`: number of binary decisions that pin down one
/// symbol out of 256.
const LEVELS: u32 = 8;

/// Number of distinct `(depth, prefix)` pairs [`sse_context`] can be
/// called with: one per internal node of the depth-`LEVELS` binary tree
/// [`encode_symbol`]/[`decode_symbol`] walk, `2^LEVELS - 1` (255 for
/// `LEVELS = 8`) — a full binary tree with `2^LEVELS` leaves has exactly
/// that many internal nodes. `crate::sse::Sse::new`'s `contexts`
/// argument for a table keyed on this scheme.
pub const SSE_CONTEXTS: usize = (1 << LEVELS) - 1;

/// Maps one step of [`encode_symbol`]/[`decode_symbol`]'s walk — the
/// decision at tree depth `depth` (`0..LEVELS`, `0` is the first,
/// coarsest split) having already decided `prefix` (the `depth` bits
/// chosen so far, i.e. `lo` divided by that depth's range width,
/// `0..2^depth`) — to a unique index in `0..SSE_CONTEXTS`, for
/// [`crate::sse::Sse::refine`]/[`crate::sse::Sse::update`] to key on.
/// `S1-P1`'s own remaining-scope decision (`research/JOURNAL.md`):
/// keying purely on tree position, the cheapest scheme that still gives
/// every node its own calibration, no coarser (folding two nodes
/// together loses the distinction the walk actually observed) and no
/// finer (there is no more context available per node than its position
/// — the symbol identity itself is exactly what has not been decided
/// yet at that node). Same numbering LZMA's literal coder uses for its
/// own binary-tree probability array (`probs[(1 << depth) | prefix]`,
/// 1-indexed there; `- 1` here to land in `0..SSE_CONTEXTS` instead).
///
/// # Panics
///
/// Panics if `depth >= LEVELS` or `prefix >= 1 << depth`: both are
/// derived from this module's own walk (`depth` counts loop iterations
/// bounded by `LEVELS`, `prefix` is `lo` divided by the current range
/// width), never from adversarial input — the same caller-code
/// invariant `cum`'s own shape check documents for [`encode_symbol`]/
/// [`decode_symbol`].
#[must_use]
pub fn sse_context(depth: u32, prefix: usize) -> usize {
    assert!(
        depth < LEVELS,
        "depth must be < LEVELS ({LEVELS}), got {depth}"
    );
    assert!(
        prefix < (1usize << depth),
        "prefix must be < 2^depth (2^{depth}), got {prefix}"
    );
    (1usize << depth) + prefix - 1
}

/// Panics if `cum` is not shaped like a cumulative-frequency table over
/// `ALPHABET` symbols: `ALPHABET + 1` entries, monotonically
/// non-decreasing, every symbol carrying strictly positive mass (`cum[i]
/// < cum[i + 1]`). A caller-code invariant, not something adversarial
/// input can trigger: every cumulative table this crate builds already
/// keeps it, the same "nothing is ever impossible to code" guarantee
/// [`crate::model::Model`] and `crate::literal::Literal::mix` give by
/// Laplace-flooring every symbol to at least mass 1.
fn check_table_shape(cum: &[u64]) {
    assert!(
        cum.len() == ALPHABET + 1,
        "cum must have exactly ALPHABET + 1 (257) entries, got {}",
        cum.len()
    );
    assert!(
        cum.windows(2).all(|w| w[0] < w[1]),
        "cum must be strictly increasing: every symbol needs positive mass"
    );
}

/// Probability the true symbol lies in the upper half of `[lo, hi)`,
/// read off `cum`: `(cum[hi] - cum[mid]) / (cum[hi] - cum[lo])`, `mid`
/// the midpoint. Never zero or one: `check_table_shape`'s strictly-
/// increasing invariant keeps both `cum[hi] - cum[mid]` and `cum[mid] -
/// cum[lo]` positive whenever `hi > lo`, which every call site keeps by
/// construction (`hi - lo` halves from 256 down to 1 and never reaches
/// 0 first, since 256 is a power of two).
fn upper_half_probability(cum: &[u64], lo: usize, hi: usize) -> f64 {
    let mid = lo + (hi - lo) / 2;
    #[allow(
        clippy::cast_precision_loss,
        reason = "cum entries are fixed-point sums bounded well under 2^53 \
                  (crate::literal::Literal::mix's own cast carries the same bound)"
    )]
    {
        (cum[hi] - cum[mid]) as f64 / (cum[hi] - cum[lo]) as f64
    }
}

/// Shared skeleton behind [`encode_symbol`], [`decode_symbol`], and
/// [`ideal_cost_bits`]: walks the same `LEVELS`-level binary-tree
/// decomposition of `cum`. `code_bit` receives each level's midpoint and
/// raw `upper_half_probability` and returns the bit that level resolved
/// to — already known from a caller's own `symbol` for [`encode_symbol`]
/// and [`ideal_cost_bits`], decoded from [`Decoder::decode_bit`] for
/// [`decode_symbol`] — so the three callers differ only in what they do
/// with that bit and probability, never in the walk itself. Mirrors
/// [`walk_sse`] for this module's non-SSE trio.
///
/// Returns the final `lo`, which after `LEVELS` halvings of `[0, ALPHABET)`
/// is exactly the coded symbol.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
fn walk(cum: &[u64], mut code_bit: impl FnMut(usize, f64) -> bool) -> u8 {
    check_table_shape(cum);
    let mut lo = 0usize;
    let mut hi = ALPHABET;
    for _ in 0..LEVELS {
        let mid = lo + (hi - lo) / 2;
        let bit = code_bit(mid, upper_half_probability(cum, lo, hi));
        if bit {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "lo is bounded to [0, ALPHABET) after LEVELS halvings of a 256-wide range, \
                  always fits u8"
    )]
    {
        lo as u8
    }
}

/// Codes `symbol` through `encoder` as `LEVELS` chained binary
/// decisions over `cum`. See the module docs for the identity this
/// implements.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
pub fn encode_symbol(encoder: &mut Encoder, cum: &[u64], symbol: u8) {
    let symbol_index = usize::from(symbol);
    let landed = walk(cum, |mid, p| {
        let bit = symbol_index >= mid;
        encoder.encode_bit(bit, p);
        bit
    });
    debug_assert_eq!(
        landed, symbol,
        "8 halvings of [0, 256) must land exactly on symbol"
    );
}

/// Decodes one byte from `decoder` as `LEVELS` chained binary
/// decisions over `cum`, the exact inverse of [`encode_symbol`].
///
/// Never panics on adversarial `decoder` state: [`Decoder::decode_bit`]
/// is total over any coded bit pattern, same as every other decode path
/// in this crate.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`. `cum` is
/// caller-supplied local state, never derived from `decoder`'s bytes, so
/// this is the same caller-code invariant [`encode_symbol`] documents,
/// not an adversarial-input hazard.
#[must_use]
pub fn decode_symbol(decoder: &mut Decoder, cum: &[u64]) -> u8 {
    walk(cum, |_mid, p| decoder.decode_bit(p))
}

/// Shared skeleton behind [`encode_symbol_sse`], [`decode_symbol_sse`], and
/// [`ideal_cost_bits_sse`]: walks the same `LEVELS`-level binary-tree
/// decomposition of `cum`, computing each level's SSE context and refined
/// probability and updating `sse` on the raw one, identically for all three
/// callers. `code_bit` receives the level's midpoint and refined
/// probability and returns the bit that level resolved to — already known
/// from a caller's own `symbol` for [`encode_symbol_sse`] and
/// [`ideal_cost_bits_sse`], decoded from [`Decoder::decode_bit`] for
/// [`decode_symbol_sse`] — so the three callers differ only in what they do
/// with that bit and probability, never in the SSE walk itself. Matches
/// [`crate::codec::walk_tokens`]'s reasoning: keeping the walk in one place
/// is what stops the encode, decode, and cost-pricing paths from silently
/// drifting apart.
///
/// Returns the final `lo`, which after `LEVELS` halvings of `[0, ALPHABET)`
/// is exactly the coded symbol.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
fn walk_sse(cum: &[u64], sse: &mut Sse, mut code_bit: impl FnMut(usize, f64) -> bool) -> u8 {
    check_table_shape(cum);
    let mut lo = 0usize;
    let mut hi = ALPHABET;
    for depth in 0..LEVELS {
        let mid = lo + (hi - lo) / 2;
        let context = sse_context(depth, lo / (hi - lo));
        let raw_p = upper_half_probability(cum, lo, hi);
        let refined_p = sse.refine(context, raw_p);
        let bit = code_bit(mid, refined_p);
        sse.update(context, raw_p, bit);
        if bit {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "lo is bounded to [0, ALPHABET) after LEVELS halvings of a 256-wide range, \
                  always fits u8"
    )]
    {
        lo as u8
    }
}

/// Codes `symbol` through `encoder` as `LEVELS` chained binary decisions
/// over `cum`, same as [`encode_symbol`], except each level's raw
/// `upper_half_probability` is first refined through `sse` (keyed by
/// [`sse_context`]) before it drives [`Encoder::encode_bit`], and `sse` is
/// updated on the raw probability afterward — the calibration step S1-P1
/// exists for, applied to the mixer's own compound per-decision estimate
/// rather than a lone frequency counter (`research/JOURNAL.md` S2-R1's
/// mechanism reading).
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
pub fn encode_symbol_sse(encoder: &mut Encoder, cum: &[u64], symbol: u8, sse: &mut Sse) {
    let symbol_index = usize::from(symbol);
    let landed = walk_sse(cum, sse, |mid, refined_p| {
        let bit = symbol_index >= mid;
        encoder.encode_bit(bit, refined_p);
        bit
    });
    debug_assert_eq!(
        landed, symbol,
        "8 halvings of [0, 256) must land exactly on symbol"
    );
}

/// Decodes one byte from `decoder` as `LEVELS` chained binary decisions
/// over `cum`, the exact inverse of [`encode_symbol_sse`].
///
/// Never panics on adversarial `decoder` state: [`Decoder::decode_bit`] is
/// total over any coded bit pattern, same as [`decode_symbol`].
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`. `cum` and `sse` are both
/// caller-supplied local state, never derived from `decoder`'s bytes.
#[must_use]
pub fn decode_symbol_sse(decoder: &mut Decoder, cum: &[u64], sse: &mut Sse) -> u8 {
    walk_sse(cum, sse, |_mid, refined_p| decoder.decode_bit(refined_p))
}

/// Sum of the ideal (`-log2`) cost of each of the `LEVELS` binary
/// decisions [`encode_symbol`] would pay coding `symbol` under `cum`,
/// without driving a coder — the chain-rule identity the module docs
/// name, checked directly. Equal to `-log2((cum[symbol + 1] -
/// cum[symbol]) / cum[ALPHABET])` up to floating-point rounding (proven
/// by test, not just asserted), [`crate::literal::Literal::ideal_cost_bits`]'s
/// counterpart for this decomposition.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
#[must_use]
#[allow(
    clippy::disallowed_methods,
    reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends \
              on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't \
              apply off the coding path) — crate::literal::Literal::ideal_cost_bits takes the \
              same exemption"
)]
pub fn ideal_cost_bits(cum: &[u64], symbol: u8) -> f64 {
    let symbol_index = usize::from(symbol);
    let mut bits = 0.0f64;
    walk(cum, |mid, p| {
        let bit = symbol_index >= mid;
        bits -= if bit { p.log2() } else { (1.0 - p).log2() };
        bit
    });
    bits
}

/// [`ideal_cost_bits`]'s counterpart for [`encode_symbol_sse`]: sums the
/// ideal (`-log2`) cost of each `sse`-refined binary decision
/// [`encode_symbol_sse`] would pay coding `symbol` under `cum`, without
/// driving a coder, updating `sse` on the raw probability exactly as
/// [`encode_symbol_sse`] does — so a caller pricing a whole stream this way
/// leaves `sse` in the same state a real `encode_symbol_sse` pass would
/// have, and later prices reflect that adaptation.
/// [`crate::literal::Literal::ideal_cost_bits_sse`]'s counterpart for this
/// decomposition.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
#[must_use]
#[allow(
    clippy::disallowed_methods,
    reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends \
              on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't \
              apply off the coding path) — crate::literal::Literal::ideal_cost_bits_sse takes the \
              same exemption"
)]
pub fn ideal_cost_bits_sse(cum: &[u64], symbol: u8, sse: &mut Sse) -> f64 {
    let symbol_index = usize::from(symbol);
    let mut bits = 0.0f64;
    walk_sse(cum, sse, |mid, refined_p| {
        let bit = symbol_index >= mid;
        bits -= if bit {
            refined_p.log2()
        } else {
            (1.0 - refined_p).log2()
        };
        bit
    });
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A uniform table: every symbol carries mass 1, matching
    /// [`crate::coder::Encoder::encode_bits`]'s fixed 50/50 split at
    /// every level.
    fn uniform_table() -> Vec<u64> {
        (0..=ALPHABET as u64).collect()
    }

    /// A geometrically skewed table: symbol `s` gets mass `2^(255 - s)`
    /// (clamped so symbol 255 still keeps mass 1), heavily favoring low
    /// symbol values — the shape a real adaptive model converges toward
    /// on repetitive data.
    fn skewed_table() -> Vec<u64> {
        let mut cum = vec![0u64; ALPHABET + 1];
        let mut acc = 0u64;
        for symbol in 0..ALPHABET {
            let shift = (ALPHABET - 1 - symbol).min(40);
            acc += 1u64 << shift;
            cum[symbol + 1] = acc;
        }
        cum
    }

    #[test]
    #[should_panic(expected = "exactly ALPHABET + 1")]
    fn wrong_length_table_panics() {
        let mut enc = Encoder::new();
        encode_symbol(&mut enc, &[0, 1, 2], 0);
    }

    #[test]
    #[should_panic(expected = "strictly increasing")]
    fn non_increasing_table_panics() {
        let mut cum = uniform_table();
        cum[5] = cum[4];
        let mut enc = Encoder::new();
        encode_symbol(&mut enc, &cum, 4);
    }

    #[test]
    fn every_symbol_round_trips_on_a_uniform_table() {
        let cum = uniform_table();
        for symbol in 0..=u8::MAX {
            let mut enc = Encoder::new();
            encode_symbol(&mut enc, &cum, symbol);
            let encoded = enc.finish();
            let mut dec = Decoder::new(&encoded);
            assert_eq!(decode_symbol(&mut dec, &cum), symbol);
        }
    }

    #[test]
    fn every_symbol_round_trips_on_a_skewed_table() {
        let cum = skewed_table();
        for symbol in 0..=u8::MAX {
            let mut enc = Encoder::new();
            encode_symbol(&mut enc, &cum, symbol);
            let encoded = enc.finish();
            let mut dec = Decoder::new(&encoded);
            assert_eq!(decode_symbol(&mut dec, &cum), symbol);
        }
    }

    #[test]
    fn a_sequence_of_symbols_round_trips_through_one_stream() {
        let cum = skewed_table();
        let symbols: Vec<u8> = crate::test_support::Xorshift32::new(0xB17_7EEE)
            .take(2000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();

        let mut enc = Encoder::new();
        for &symbol in &symbols {
            encode_symbol(&mut enc, &cum, symbol);
        }
        let encoded = enc.finish();

        let mut dec = Decoder::new(&encoded);
        let decoded: Vec<u8> = symbols
            .iter()
            .map(|_| decode_symbol(&mut dec, &cum))
            .collect();
        assert_eq!(decoded, symbols);
    }

    #[test]
    fn ideal_cost_matches_the_direct_symbol_cost() {
        // Chain-rule identity the module docs name, checked directly:
        // the product of the 8 binary decision probabilities along a
        // symbol's path must equal the direct
        // (cum[symbol+1]-cum[symbol])/cum[ALPHABET] ratio, so their
        // -log2 costs must match to near float precision.
        let cum = skewed_table();
        for symbol in 0..=u8::MAX {
            #[allow(
                clippy::cast_precision_loss,
                clippy::disallowed_methods,
                reason = "test-only oracle diffed against ideal_cost_bits's own decomposition; \
                          cum entries are bounded well under 2^53, and no bitstream depends on \
                          this log2 call"
            )]
            let direct = {
                let num = (cum[usize::from(symbol) + 1] - cum[usize::from(symbol)]) as f64;
                let den = cum[ALPHABET] as f64;
                -(num / den).log2()
            };
            let decomposed = ideal_cost_bits(&cum, symbol);
            assert!(
                (direct - decomposed).abs() < 1e-9,
                "symbol {symbol}: direct {direct} bits vs decomposed {decomposed} bits"
            );
        }
    }

    #[test]
    fn ideal_cost_drops_as_a_symbol_gets_more_likely() {
        // Coding a fixed heavily-favored symbol on the skewed table costs
        // far fewer bits than a rare one — the mechanism S1-P1 exists to
        // exploit, checked on this decomposition directly.
        let cum = skewed_table();
        let favored = ideal_cost_bits(&cum, 0);
        let rare = ideal_cost_bits(&cum, 255);
        assert!(
            favored < rare,
            "favored symbol ({favored} bits) should cost less than the rare one ({rare} bits)"
        );
    }

    #[test]
    fn real_coded_length_tracks_ideal_cost_within_a_few_percent() {
        // Same shape as crate::literal's
        // ideal_cost_sum_tracks_real_encoded_length: summed ideal cost is
        // an estimate, not the real coder's bit-exact output (8 chained
        // 16-bit quantized encode_bit calls per symbol instead of one
        // direct range division, plus flush bits), so this checks
        // closeness, not equality. A looser budget than crate::literal's
        // 1% for the same reason: 8x the quantization steps per symbol.
        let cum = skewed_table();
        let symbols: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED_CAFE)
            .take(5000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();

        let ideal_bits: f64 = symbols.iter().map(|&s| ideal_cost_bits(&cum, s)).sum();

        let mut enc = Encoder::new();
        for &symbol in &symbols {
            encode_symbol(&mut enc, &cum, symbol);
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "encoded length is far below f64's exact integer range (2^53)"
        )]
        let real_bits = (enc.finish().len() * 8) as f64;

        let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
        assert!(
            relative_diff <= 0.05,
            "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
             {relative_diff:.4} relative difference exceeds the 5% budget"
        );
    }

    #[test]
    fn real_sse_coded_length_tracks_sse_ideal_cost_within_a_few_percent() {
        // Same shape as real_coded_length_tracks_ideal_cost_within_a_few_percent,
        // for the sse-calibrated path: ideal_cost_bits_sse must track
        // encode_symbol_sse's real output, not just encode_symbol's.
        let cum = skewed_table();
        let symbols: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED_CAFE)
            .take(5000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();

        let mut cost_sse = Sse::new(SSE_CONTEXTS);
        let ideal_bits: f64 = symbols
            .iter()
            .map(|&s| ideal_cost_bits_sse(&cum, s, &mut cost_sse))
            .sum();

        let mut coder_sse = Sse::new(SSE_CONTEXTS);
        let mut enc = Encoder::new();
        for &symbol in &symbols {
            encode_symbol_sse(&mut enc, &cum, symbol, &mut coder_sse);
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "encoded length is far below f64's exact integer range (2^53)"
        )]
        let real_bits = (enc.finish().len() * 8) as f64;

        let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
        assert!(
            relative_diff <= 0.05,
            "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
             {relative_diff:.4} relative difference exceeds the 5% budget"
        );
    }

    #[test]
    fn sse_context_is_a_bijection_onto_0_sse_contexts() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for depth in 0..LEVELS {
            for prefix in 0..(1usize << depth) {
                let context = sse_context(depth, prefix);
                assert!(
                    context < SSE_CONTEXTS,
                    "depth={depth}, prefix={prefix}: context {context} must be < {SSE_CONTEXTS}"
                );
                assert!(
                    seen.insert(context),
                    "depth={depth}, prefix={prefix}: context {context} collides with an earlier pair"
                );
            }
        }
        assert_eq!(
            seen.len(),
            SSE_CONTEXTS,
            "every one of the {SSE_CONTEXTS} contexts must be reachable"
        );
    }

    #[test]
    fn sse_context_along_one_symbol_path_visits_eight_distinct_nodes() {
        // sse_context's documented "prefix = lo / width" identity, exercised
        // against encode_symbol's own walk (not asserted in isolation): every
        // symbol's root-to-leaf path must visit LEVELS distinct tree nodes,
        // one per depth, since a real Sse calibration wired behind this walk
        // must never conflate two different decisions under one context.
        for symbol in 0..=u8::MAX {
            let symbol = usize::from(symbol);
            let mut lo = 0usize;
            let mut hi = ALPHABET;
            let mut path = Vec::with_capacity(LEVELS as usize);
            for depth in 0..LEVELS {
                let width = hi - lo;
                let prefix = lo / width;
                path.push(sse_context(depth, prefix));
                let mid = lo + width / 2;
                if symbol >= mid {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let distinct: std::collections::HashSet<_> = path.iter().copied().collect();
            assert_eq!(
                distinct.len(),
                LEVELS as usize,
                "symbol {symbol}: path {path:?} must visit {LEVELS} distinct contexts"
            );
        }
    }

    #[test]
    #[should_panic(expected = "depth must be < LEVELS")]
    fn sse_context_depth_out_of_range_panics() {
        let _ = sse_context(LEVELS, 0);
    }

    #[test]
    #[should_panic(expected = "prefix must be")]
    fn sse_context_prefix_out_of_range_panics() {
        let _ = sse_context(2, 4);
    }

    #[test]
    fn sse_context_count_is_255() {
        assert_eq!(SSE_CONTEXTS, 255);
    }

    #[test]
    fn every_symbol_round_trips_through_sse_on_a_uniform_table() {
        let cum = uniform_table();
        for symbol in 0..=u8::MAX {
            let mut enc_sse = Sse::new(SSE_CONTEXTS);
            let mut enc = Encoder::new();
            encode_symbol_sse(&mut enc, &cum, symbol, &mut enc_sse);
            let encoded = enc.finish();
            let mut dec_sse = Sse::new(SSE_CONTEXTS);
            let mut dec = Decoder::new(&encoded);
            assert_eq!(decode_symbol_sse(&mut dec, &cum, &mut dec_sse), symbol);
        }
    }

    #[test]
    fn every_symbol_round_trips_through_sse_on_a_skewed_table() {
        let cum = skewed_table();
        for symbol in 0..=u8::MAX {
            let mut enc_sse = Sse::new(SSE_CONTEXTS);
            let mut enc = Encoder::new();
            encode_symbol_sse(&mut enc, &cum, symbol, &mut enc_sse);
            let encoded = enc.finish();
            let mut dec_sse = Sse::new(SSE_CONTEXTS);
            let mut dec = Decoder::new(&encoded);
            assert_eq!(decode_symbol_sse(&mut dec, &cum, &mut dec_sse), symbol);
        }
    }

    #[test]
    fn a_sequence_of_symbols_round_trips_through_sse_over_one_stream() {
        let cum = skewed_table();
        let symbols: Vec<u8> = crate::test_support::Xorshift32::new(0xB17_7EEE)
            .take(2000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();

        let mut sse = Sse::new(SSE_CONTEXTS);
        let mut enc = Encoder::new();
        for &symbol in &symbols {
            encode_symbol_sse(&mut enc, &cum, symbol, &mut sse);
        }
        let encoded = enc.finish();

        let mut sse = Sse::new(SSE_CONTEXTS);
        let mut dec = Decoder::new(&encoded);
        let decoded: Vec<u8> = symbols
            .iter()
            .map(|_| decode_symbol_sse(&mut dec, &cum, &mut sse))
            .collect();
        assert_eq!(decoded, symbols);
    }

    #[test]
    fn sse_calibration_wins_when_the_raw_table_is_systematically_biased() {
        // Mirrors crate::sse's own
        // calibrated_probability_round_trips_and_costs_less_than_a_fixed_split:
        // an uninformative-ish cum (near-uniform) fed a stream that
        // actually favors low symbols hard should cost less through
        // encode_symbol_sse than encode_symbol, once Sse has adapted,
        // because Sse is exactly what corrects a systematic gap between a
        // primary estimate and the true observed rate.
        let cum = uniform_table();
        let symbols: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED_5EED)
            .take(4000)
            .map(|state| u8::try_from(state % 8).unwrap()) // heavily favors symbols 0..8
            .collect();

        let mut plain_enc = Encoder::new();
        for &symbol in &symbols {
            encode_symbol(&mut plain_enc, &cum, symbol);
        }
        let plain_bytes = plain_enc.finish().len();

        let mut sse = Sse::new(SSE_CONTEXTS);
        let mut sse_enc = Encoder::new();
        for &symbol in &symbols {
            encode_symbol_sse(&mut sse_enc, &cum, symbol, &mut sse);
        }
        let sse_bytes = sse_enc.finish().len();

        assert!(
            sse_bytes < plain_bytes,
            "sse-calibrated {sse_bytes} bytes should beat uncalibrated {plain_bytes} bytes \
             once Sse has adapted to the skew a uniform cum table can't see"
        );
    }

    #[test]
    fn boundary_symbols_zero_and_max_round_trip() {
        let cum = skewed_table();
        for &symbol in &[0u8, u8::MAX] {
            let mut enc = Encoder::new();
            encode_symbol(&mut enc, &cum, symbol);
            let encoded = enc.finish();
            let mut dec = Decoder::new(&encoded);
            assert_eq!(decode_symbol(&mut dec, &cum), symbol);
        }
    }
}
