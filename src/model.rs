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

    fn update(&mut self, symbol: usize) {
        self.freq[symbol] += INCREMENT;
        self.total += INCREMENT;
        if self.total > RESCALE_LIMIT {
            let mut total = 0u32;
            for f in &mut self.freq {
                *f = (*f + 1) >> 1;
                total += *f;
            }
            self.total = total;
        }
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

    fn roundtrip_symbols(symbols: &[usize], alphabet_len: usize) {
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
