//! Order-0 adaptive frequency table: [`Model`], the data-derived
//! cumulative-frequency source for [`crate::coder`]'s range coder. One
//! instance per coded alphabet; the literal mixer in [`crate::literal`] is
//! built on the coder directly, not on this type.

use crate::Rate;
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
    /// frequency 1, returning `Err` instead of aborting if the allocator
    /// cannot satisfy `alphabet_len` entries (hard rule 2, #453).
    ///
    /// # Panics
    ///
    /// Panics if `alphabet_len` is zero: a model over no symbols could
    /// encode nothing, which is a caller bug fixed at construction, never
    /// something adversarial input can trigger.
    pub(crate) fn try_new(alphabet_len: usize) -> Result<Self, std::collections::TryReserveError> {
        let freq = crate::try_filled_vec(alphabet_len, 1u32)?;
        assert!(!freq.is_empty(), "Model alphabet must be non-empty");
        let total = u32::try_from(freq.len()).expect("alphabet_len fits u32");
        Ok(Self { freq, total })
    }

    fn update(&mut self, symbol: usize) {
        crate::rescale_bank(&mut self.freq, &mut self.total, symbol, Rate::DEFAULT);
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
        // The cumulative range [low, low + freq[symbol]) out of `total`;
        // `decode` scans for the same range.
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
        let total = u64::from(self.total);
        let target = decoder.target(total);
        let mut symbol = 0;
        let mut low = 0u64;
        while low + u64::from(self.freq[symbol]) <= target {
            low += u64::from(self.freq[symbol]);
            symbol += 1;
        }
        decoder.decode(low, low + u64::from(self.freq[symbol]), total);
        self.update(symbol);
        symbol
    }
}

#[cfg(test)]
mod tests;

// Not under Miri: same rationale as `coder.rs`'s `mod proptests` header
// comment (interpretation cost, issue #456).
#[cfg(test)]
#[cfg(not(miri))]
mod proptests;
