//! Order-0 adaptive frequency table: [`Model`], the piece that turns
//! [`crate::coder`]'s range coder into an actual entropy coder by supplying
//! data-derived cumulative-frequency ranges instead of the fixed ones
//! `coder`'s own tests use as a stand-in (`JOURNAL` S2-A10, S2-D2).
//!
//! Ported from the archive's `Model`
//! (`research/imports/session-1/mothergod.rs`), not the code, per ADR-0006:
//! same increment-then-halve update rule, same linear cumulative-frequency
//! scan. This is the flag/length/offset stage of S2-D2 (each of those is one
//! `Model` instance); the six-expert `Lit` literal mixer is a separate,
//! larger slice built on top of the same coder, not on top of this type.
//!
//! [`Model::ideal_cost_bits`] is ROADMAP M2's ideal-cost accounting mode
//! (first slice, `JOURNAL` S2-A30): sums `-log2(p)` against this table's
//! adaptive state instead of driving [`crate::coder::Encoder`], so an
//! experiment loop can price a distribution without paying for real
//! arithmetic coding. [`crate::literal::Literal::ideal_cost_bits`]
//! (`JOURNAL` S2-A31) is the same mode's counterpart for the six-expert
//! mixer; [`crate::codec::ideal_cost_bits`] (`JOURNAL` S2-A38) sums both
//! together into the whole-codec pass.

use crate::coder::{Decoder, Encoder};

/// Frequency increment applied to a symbol every time it is coded.
///
/// Ported unchanged from the archive's `INC`
/// (`research/imports/session-1/mothergod.rs`): higher favors recent
/// symbols, lower favors long-run accuracy.
const INCREMENT: u32 = 12;

/// Total frequency past which [`Model::update`] halves every count.
///
/// Ported unchanged from the archive's `LIM`. Keeps `total`, and every
/// per-symbol frequency, comfortably inside `u32` with room to spare, and
/// bounds how long stale evidence can outweigh recent symbols.
const RESCALE_LIMIT: u32 = 65536;

/// An order-0 adaptive frequency table over `0..alphabet_len` symbols.
///
/// Every symbol starts at frequency 1 (nothing is ever impossible to code),
/// and each coded occurrence raises its own frequency by a fixed increment
/// until the running total crosses a fixed limit, at which point every
/// frequency is halved, rounding up so no symbol's frequency can decay to
/// zero. `total` always equals the sum of `freq`: [`Self::decode`] leans on
/// that invariant to never run off the table regardless of what bytes the
/// [`Decoder`] it reads from was built from.
#[derive(Debug, Clone)]
pub struct Model {
    freq: Vec<u32>,
    total: u32,
}

impl Model {
    /// A fresh table over `alphabet_len` symbols, each starting at
    /// frequency 1.
    ///
    /// # Panics
    ///
    /// Panics if `alphabet_len` is zero: a model over no symbols could
    /// encode nothing, which is a caller bug fixed at construction, never
    /// something adversarial input can trigger.
    #[must_use]
    pub fn new(alphabet_len: usize) -> Self {
        assert!(alphabet_len > 0, "Model alphabet must be non-empty");
        Self {
            freq: vec![1; alphabet_len],
            total: u32::try_from(alphabet_len).expect("alphabet_len fits u32"),
        }
    }

    /// Fallible counterpart to [`Self::new`]: the same fresh table, but
    /// returns `Err` instead of aborting if the allocator cannot satisfy
    /// `alphabet_len` entries. [`crate::codec::decode`]'s real decode path
    /// uses this (hard rule 2, `rust-craft` skill's allocation-discipline,
    /// `tests/torture.rs`, #453); [`Self::new`] stays the panicking
    /// constructor the encoder and every test use, where `alphabet_len` is
    /// always one of this crate's own small fixed constants.
    ///
    /// # Panics
    ///
    /// Same as [`Self::new`]: `alphabet_len` zero is a caller bug, never
    /// something adversarial input can trigger.
    pub(crate) fn try_new(alphabet_len: usize) -> Result<Self, std::collections::TryReserveError> {
        assert!(alphabet_len > 0, "Model alphabet must be non-empty");
        Ok(Self {
            freq: crate::try_filled_vec(alphabet_len, 1u32)?,
            total: u32::try_from(alphabet_len).expect("alphabet_len fits u32"),
        })
    }

    fn update(&mut self, symbol: usize) {
        crate::rescale_bank(
            &mut self.freq,
            &mut self.total,
            symbol,
            INCREMENT,
            RESCALE_LIMIT,
        );
    }

    /// Codes `symbol` through `encoder` under this table's current
    /// distribution, then updates the table.
    ///
    /// # Panics
    ///
    /// Panics if `symbol >= alphabet_len`: the caller's encoder and decoder
    /// share one fixed alphabet by construction, so an out-of-range symbol
    /// here is our own bug, not adversarial input (nothing on the decode
    /// path calls this method).
    pub fn encode(&mut self, encoder: &mut Encoder, symbol: usize) {
        let low: u32 = self.freq[..symbol].iter().sum();
        let high = low + self.freq[symbol];
        encoder.encode(u64::from(low), u64::from(high), u64::from(self.total));
        self.update(symbol);
    }

    /// Bits it would cost to code `symbol` under this table's current
    /// distribution — `-log2(freq[symbol] / total)` — then updates the
    /// table the same way [`Self::encode`] does. No [`Encoder`] involved:
    /// this is the ideal-cost accounting mode ROADMAP M2 and ADR-0006 call
    /// for, the Rust-native replacement for the archive's Python model-cost
    /// proxy (`sum -log2(p)` instead of emitting bits), for experiment
    /// loops that want a distribution's cost without paying for real
    /// arithmetic coding.
    ///
    /// # Panics
    ///
    /// Panics if `symbol >= alphabet_len`, same bound as [`Self::encode`].
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't apply off the coding path)"
    )]
    pub fn ideal_cost_bits(&mut self, symbol: usize) -> f64 {
        let probability = f64::from(self.freq[symbol]) / f64::from(self.total);
        self.update(symbol);
        -probability.log2()
    }

    /// Decodes one symbol from `decoder` under this table's current
    /// distribution, then updates the table the same way [`Self::encode`]
    /// did on the encoding side, keeping both in lockstep.
    ///
    /// Never panics on adversarial `decoder` state: [`Decoder::target`] is
    /// mathematically bounded to `[0, total)`, and `total` is exactly the
    /// sum of `freq` by construction, so the scan below always finds a
    /// symbol before running past the end of the table, regardless of what
    /// bytes produced `decoder`'s internal value.
    #[must_use]
    pub fn decode(&mut self, decoder: &mut Decoder) -> usize {
        let target = decoder.target(u64::from(self.total));
        let mut symbol = 0;
        let mut low = 0u64;
        while low + u64::from(self.freq[symbol]) <= target {
            low += u64::from(self.freq[symbol]);
            symbol += 1;
        }
        let high = low + u64::from(self.freq[symbol]);
        decoder.decode(low, high, u64::from(self.total));
        self.update(symbol);
        symbol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn roundtrip_symbols(symbols: &[usize], alphabet_len: usize) {
        let mut model = Model::new(alphabet_len);
        let mut enc = Encoder::new();
        for &s in symbols {
            model.encode(&mut enc, s);
        }
        let bytes = enc.finish();

        let mut model = Model::new(alphabet_len);
        let mut dec = Decoder::new(&bytes);
        let got: Vec<usize> = symbols.iter().map(|_| model.decode(&mut dec)).collect();
        assert_eq!(got, symbols);
    }

    #[test]
    fn empty_stream_round_trips() {
        roundtrip_symbols(&[], 4);
    }

    #[test]
    fn single_symbol_round_trips() {
        roundtrip_symbols(&[0], 2);
    }

    #[test]
    fn skewed_frequencies_round_trip() {
        // Symbol 0 dominates: exercises the near-degenerate coder
        // intervals hardest, same shape as coder.rs's own coverage but
        // now driven by the real adaptive table, not a test stand-in.
        let symbols: Vec<usize> = (0..500).map(|i| usize::from(i % 17 == 0)).collect();
        roundtrip_symbols(&symbols, 2);
    }

    #[test]
    fn full_alphabet_cycles_round_trip() {
        let symbols: Vec<usize> = (0..2000).map(|i| i % 256).collect();
        roundtrip_symbols(&symbols, 256);
    }

    #[test]
    fn pseudo_random_symbols_round_trip() {
        let symbols: Vec<usize> = crate::test_support::Xorshift32::new(0x1234_5678)
            .take(5000)
            .map(|state| (state % 32) as usize)
            .collect();
        roundtrip_symbols(&symbols, 32);
    }

    #[test]
    fn rescale_triggers_and_round_trip_still_holds() {
        // RESCALE_LIMIT is 65536 and every update adds INCREMENT=12, so a
        // two-symbol alphabet coded 10,000 times crosses the halving
        // threshold several times over; round-trip must still hold on the
        // far side of every halving.
        let symbols: Vec<usize> = (0..10_000).map(|i| usize::from(i % 3 == 0)).collect();
        roundtrip_symbols(&symbols, 2);
    }

    #[test]
    fn independent_models_interleave_on_one_coder() {
        // The real use this type exists for (JOURNAL S2-D2): a flag model
        // and a length model, each with their own alphabet and state,
        // alternating on the same coder stream. Each model instance must
        // only ever see its own symbols, never the other's.
        let flags = [1usize, 0, 0, 1, 1, 0, 1];
        let lengths = [3usize, 15, 0, 7, 2, 9, 1];

        let mut flag_model = Model::new(2);
        let mut length_model = Model::new(16);
        let mut enc = Encoder::new();
        for (&flag, &len) in flags.iter().zip(lengths.iter()) {
            flag_model.encode(&mut enc, flag);
            length_model.encode(&mut enc, len);
        }
        let bytes = enc.finish();

        let mut flag_model = Model::new(2);
        let mut length_model = Model::new(16);
        let mut dec = Decoder::new(&bytes);
        for (&flag, &len) in flags.iter().zip(lengths.iter()) {
            assert_eq!(flag_model.decode(&mut dec), flag);
            assert_eq!(length_model.decode(&mut dec), len);
        }
    }

    #[test]
    fn ideal_cost_matches_fresh_table_uniform_distribution() {
        // A fresh 4-symbol table starts uniform (every freq is 1, total 4),
        // so every symbol's ideal cost is exactly -log2(1/4) = 2 bits.
        let mut model = Model::new(4);
        assert!((model.ideal_cost_bits(0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn ideal_cost_drops_as_a_symbol_gets_more_likely() {
        // Coding the same symbol repeatedly raises its own frequency
        // (INCREMENT), so its ideal cost must strictly decrease call over
        // call as the table adapts toward it.
        let mut model = Model::new(4);
        let first = model.ideal_cost_bits(0);
        let second = model.ideal_cost_bits(0);
        let third = model.ideal_cost_bits(0);
        assert!(second < first);
        assert!(third < second);
    }

    #[test]
    fn ideal_cost_updates_state_same_as_encode() {
        // ideal_cost_bits must leave the table in the same state encode
        // would have: fork two identical tables, drive one through each
        // path over the same symbols, then confirm they agree from here by
        // coding one more symbol on top of each and comparing cost.
        let symbols = [0usize, 1, 0, 2, 0, 1, 3, 0];
        let mut via_encode = Model::new(4);
        let mut enc = Encoder::new();
        for &s in &symbols {
            via_encode.encode(&mut enc, s);
        }
        let mut via_ideal_cost = Model::new(4);
        for &s in &symbols {
            let _ = via_ideal_cost.ideal_cost_bits(s);
        }
        assert!((via_encode.ideal_cost_bits(2) - via_ideal_cost.ideal_cost_bits(2)).abs() < 1e-9);
    }

    #[test]
    fn ideal_cost_sum_tracks_real_encoded_length() {
        // Named corpus (CLAUDE.md hard rule 4): 5000 pseudo-random symbols
        // over a 32-wide alphabet, the same fixture
        // pseudo_random_symbols_round_trip above uses. Summed ideal cost is
        // an estimate, not the real coder's bit-exact output (integer
        // cumulative-frequency division rounds; the coder also pays a
        // handful of flush bits at the very end), so this checks closeness,
        // not equality — the same tolerance shape as literal.rs's vendored
        // `exp` accuracy check (ADR-0024).
        let symbols: Vec<usize> = crate::test_support::Xorshift32::new(0x1234_5678)
            .take(5000)
            .map(|state| (state % 32) as usize)
            .collect();

        let mut ideal_cost_model = Model::new(32);
        let ideal_bits: f64 = symbols
            .iter()
            .map(|&s| ideal_cost_model.ideal_cost_bits(s))
            .sum();

        let mut real_model = Model::new(32);
        let mut enc = Encoder::new();
        for &s in &symbols {
            real_model.encode(&mut enc, s);
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "encoded length is far below f64's exact integer range (2^53)"
        )]
        let real_bits = (enc.finish().len() * 8) as f64;

        let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
        assert!(
            relative_diff <= 0.01,
            "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
             {relative_diff:.4} relative difference exceeds the 1% budget"
        );
    }

    #[test]
    fn decoding_truncated_stream_does_not_panic() {
        let symbols: Vec<usize> = (0..200).map(|i| i % 5).collect();
        let mut model = Model::new(5);
        let mut enc = Encoder::new();
        for &s in &symbols {
            model.encode(&mut enc, s);
        }
        let bytes = enc.finish();
        let truncated = &bytes[..bytes.len() / 2];

        let mut model = Model::new(5);
        let mut dec = Decoder::new(truncated);
        for _ in &symbols {
            let _ = model.decode(&mut dec);
        }
        // No panic is the assertion: decoded symbols past the real data
        // are whatever implicit-zero bits produce, never treated as
        // ground truth here.
    }
}

// `mod tests` above is a wall of `roundtrip*` examples that each vary one
// dimension (alphabet size, symbol distribution, stream length): the
// escalation ladder's rung-2 trigger (`test-craft`'s escalation-ladder
// reference, #452 scope item 1). The examples stay as named anchors for
// the specific edge cases they document (rescale crossing, interleaved
// model instances); this property sweeps arbitrary alphabets and symbol
// streams instead of one example per shape. `roundtrip_symbols` is
// `mod tests`' own helper, reused here rather than duplicated
// (single source of truth for the encode/decode/assert sequence).
// Not under Miri: interpretation costs 300-5000x per case on this
// crate (measured, issue #456), the storm multiplies that by its case
// count, and the deterministic example tests already walk the same
// paths for UB observation.
#[cfg(test)]
#[cfg(not(miri))]
mod proptests {
    use proptest::prelude::*;

    use super::tests::roundtrip_symbols;

    /// Alphabet size 2..64 (every hand-written example above falls in this
    /// range), symbol stream length 0..300 (`Model::decode`'s scan is
    /// `O(alphabet)` per symbol, so this stays cheap at proptest's default
    /// case count without needing a `PROPTEST_CASES`-scaled profile the way
    /// `lib.rs`'s heavier `compress`-driven property does).
    fn symbols_and_alphabet() -> impl Strategy<Value = (usize, Vec<usize>)> {
        (2usize..64).prop_flat_map(|alphabet| {
            proptest::collection::vec(0..alphabet, 0..300)
                .prop_map(move |symbols| (alphabet, symbols))
        })
    }

    proptest! {
        /// Every symbol decoded back matches what was encoded, swept over
        /// arbitrary alphabets and streams instead of one example per shape
        /// (mirrors `mod tests`' `roundtrip_symbols`-based examples).
        #[test]
        fn roundtrip_holds_for_arbitrary_symbol_streams((alphabet, symbols) in symbols_and_alphabet()) {
            roundtrip_symbols(&symbols, alphabet);
        }
    }
}
