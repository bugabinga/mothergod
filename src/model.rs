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
            crate::DEFAULT_RESCALE_INCREMENT,
            crate::DEFAULT_RESCALE_LIMIT,
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
        crate::encode_symbol(encoder, &self.freq, symbol, u64::from(self.total));
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
        let (symbol, low, high) = crate::scan_for_target(&self.freq, target);
        decoder.decode(low, high, u64::from(self.total));
        self.update(symbol);
        symbol
    }
}

#[cfg(test)]
mod tests;

// `mod tests` above is a wall of `roundtrip*` examples that each vary one
// dimension (alphabet size, symbol distribution, stream length): the
// escalation ladder's rung-2 trigger (`test-craft`'s escalation-ladder
// reference, #452 scope item 1). The examples stay as named anchors for
// the specific edge cases they document (rescale crossing, interleaved
// model instances); this property sweeps arbitrary alphabets and symbol
// streams instead of one example per shape. `roundtrip_symbols` is
// `mod tests`' own helper, reused here rather than duplicated
// (single source of truth for the encode/decode/assert sequence).
// Not under Miri: same rationale as `coder.rs`'s `mod proptests` header
// comment (interpretation cost, issue #456).
#[cfg(test)]
#[cfg(not(miri))]
mod proptests;
