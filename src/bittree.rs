//! Binary decomposition of a 256-symbol cumulative-frequency table into a
//! sequence of top-down binary split decisions ("bit tree"): ROADMAP M3's
//! oldest standing lead's own next step (`research/JOURNAL.md` S1-P1,
//! `crate::sse`'s module docs — "the literal mixer's own eventual binary
//! decomposition is the obvious calibration candidate, not another raw
//! `Model` split").
//!
//! Not a port: no archive precedent (`crate::sse`'s module docs record the
//! same grep-clean result for S1-P1 generally). Standalone, like
//! [`crate::sse::Sse`] and [`crate::coder::Encoder::encode_bit`]/
//! [`crate::coder::Decoder::decode_bit`] before it (S2-A40/S2-A41): no
//! caller in this crate drives this yet, and it is not wired into
//! [`crate::literal::Literal`] or `crate::codec`.
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

use crate::coder::{Decoder, Encoder};

/// Byte alphabet a cumulative table spans, matching
/// [`crate::literal::Literal`]'s own alphabet size.
const ALPHABET: usize = 256;

/// `log2(ALPHABET)`: number of binary decisions that pin down one
/// symbol out of 256.
const LEVELS: u32 = 8;

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

/// Codes `symbol` through `encoder` as `LEVELS` chained binary
/// decisions over `cum`. See the module docs for the identity this
/// implements.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
pub fn encode_symbol(encoder: &mut Encoder, cum: &[u64], symbol: u8) {
    check_table_shape(cum);
    let symbol = usize::from(symbol);
    let mut lo = 0usize;
    let mut hi = ALPHABET;
    for _ in 0..LEVELS {
        let mid = lo + (hi - lo) / 2;
        let bit = symbol >= mid;
        encoder.encode_bit(bit, upper_half_probability(cum, lo, hi));
        if bit {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    debug_assert_eq!(
        lo, symbol,
        "8 halvings of [0, 256) must land exactly on symbol"
    );
    debug_assert_eq!(hi, symbol + 1);
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
    check_table_shape(cum);
    let mut lo = 0usize;
    let mut hi = ALPHABET;
    for _ in 0..LEVELS {
        let mid = lo + (hi - lo) / 2;
        let bit = decoder.decode_bit(upper_half_probability(cum, lo, hi));
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
    check_table_shape(cum);
    let symbol = usize::from(symbol);
    let mut lo = 0usize;
    let mut hi = ALPHABET;
    let mut bits = 0.0f64;
    for _ in 0..LEVELS {
        let mid = lo + (hi - lo) / 2;
        let bit = symbol >= mid;
        let p = upper_half_probability(cum, lo, hi);
        bits -= if bit { p.log2() } else { (1.0 - p).log2() };
        if bit {
            lo = mid;
        } else {
            hi = mid;
        }
    }
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
