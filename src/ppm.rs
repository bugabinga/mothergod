//! [`Ppm`]: an adaptive frequency table in which a never-observed symbol is
//! a separately priced escape event, not a Laplace floor of 1 (`JOURNAL`
//! S1-P3).
//!
//! PPM Method C: the escape's frequency is the number of *distinct* symbols
//! already observed, so a context that keeps adding new symbols escapes
//! more readily than one that recodes the same few.
//!
//! The only live consumer is [`crate::literal::PpmExpertState`], which reads
//! `Ppm::probability` and [`Ppm::observe`]. Escape coding
//! ([`Ppm::encode_escape`], [`Ppm::decode`]) has no bitstream caller.

use crate::coder::{Decoder, Encoder};

/// An order-0 adaptive frequency table over `0..alphabet_len` symbols.
///
/// Unlike [`crate::model::Model`], every symbol starts at frequency **0**.
/// The coding space is `total + distinct`: `total` slots split among the
/// symbols already seen, `distinct` slots for the escape event.
#[derive(Debug, Clone)]
pub struct Ppm {
    freq: Vec<u32>,
    total: u32,
    distinct: u32,
}

impl Ppm {
    /// A fresh table over `alphabet_len` symbols, all unseen.
    ///
    /// # Panics
    ///
    /// Panics if `alphabet_len` is zero: a caller bug, never reachable from
    /// adversarial input.
    #[must_use]
    pub fn new(alphabet_len: usize) -> Self {
        assert!(alphabet_len > 0, "Ppm alphabet must be non-empty");
        Self {
            freq: vec![0; alphabet_len],
            total: 0,
            distinct: 0,
        }
    }

    /// Count of symbols observed at least once: the escape's frequency.
    #[must_use]
    pub fn distinct(&self) -> u32 {
        self.distinct
    }

    /// The coding space: every price and coder width here divides by it.
    fn space(&self) -> u32 {
        self.total + self.distinct
    }

    /// `true` if `symbol` has never been observed: [`Self::encode`] must not
    /// code it.
    #[must_use]
    pub fn is_escape(&self, symbol: usize) -> bool {
        self.freq[symbol] == 0
    }

    /// Records one occurrence of `symbol`. Rescaling preserves zero entries
    /// exactly (`(0 + 1) >> 1 == 0`), so [`Self::is_escape`] never flips from
    /// `true` to `false` on its own and [`Self::distinct`] only grows.
    pub fn observe(&mut self, symbol: usize) {
        if self.freq[symbol] == 0 {
            self.distinct += 1;
        }
        crate::rescale_bank(
            &mut self.freq,
            &mut self.total,
            symbol,
            crate::DEFAULT_RESCALE_INCREMENT,
            crate::DEFAULT_RESCALE_LIMIT,
        );
    }

    /// `-log2` price in bits of `symbol`, or [`None`] if it is an escape
    /// ([`Self::price_escape`] prices that instead).
    ///
    /// Advisory only: never drives [`Encoder`]/[`Decoder`], so libm's `log2`
    /// last-ulp behavior cannot desync a bitstream (ADR-0024).
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "advisory price estimate only, never drives an Encoder or Decoder (ADR-0024's determinism rule doesn't apply off the coding path), same carve-out as lz.rs's PriceCounts::price"
    )]
    pub fn price_symbol(&self, symbol: usize) -> Option<f64> {
        if self.is_escape(symbol) {
            return None;
        }
        Some(-self.probability(symbol).log2())
    }

    /// `symbol`'s share of [`Self::space`], `0.0` if never observed: the
    /// linear-space inverse of [`Self::price_symbol`].
    #[must_use]
    pub(crate) fn probability(&self, symbol: usize) -> f64 {
        if self.freq[symbol] == 0 {
            0.0
        } else {
            f64::from(self.freq[symbol]) / f64::from(self.space())
        }
    }

    /// `-log2` price in bits of the escape event. A table that has observed
    /// nothing (`total == 0`) escapes for free: `0.0` bits.
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "advisory price estimate only, never drives an Encoder or Decoder, same carve-out as Self::price_symbol"
    )]
    pub fn price_escape(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let denom = f64::from(self.space());
        -(f64::from(self.distinct) / denom).log2()
    }

    /// Codes `symbol` in the `total`-wide non-escape band, then
    /// [`Self::observe`]s it.
    ///
    /// # Panics
    ///
    /// Panics if `symbol` is an escape ([`Self::is_escape`]): the caller
    /// must call [`Self::encode_escape`] instead. Also panics if
    /// `symbol >= alphabet_len`.
    pub fn encode(&mut self, encoder: &mut Encoder, symbol: usize) {
        assert!(
            !self.is_escape(symbol),
            "Ppm::encode called on a never-observed symbol; use encode_escape"
        );
        crate::encode_symbol(encoder, &self.freq, symbol, u64::from(self.space()));
        self.observe(symbol);
    }

    /// Codes the escape event: the band `[total, total + distinct)`. Does
    /// not [`Self::observe`]; the caller does once a lower order has decoded
    /// the real symbol.
    ///
    /// # Panics
    ///
    /// Panics if `distinct == 0`: the band has zero width, and
    /// [`Self::price_escape`] prices that state at `0.0` bits.
    pub fn encode_escape(&mut self, encoder: &mut Encoder) {
        assert!(
            self.distinct > 0,
            "Ppm::encode_escape called on an empty table; escaping an empty table costs \
             nothing and needs no coded event"
        );
        let total = u64::from(self.total);
        let width = u64::from(self.distinct);
        encoder.encode(total, total + width, total + width);
    }

    /// Decodes one event: [`Some`] symbol, [`Self::observe`]d, matching
    /// [`Self::encode`]; or [`None`] for the escape band, state untouched,
    /// matching [`Self::encode_escape`].
    ///
    /// # Panics
    ///
    /// Panics if `distinct == 0`, same bound as [`Self::encode_escape`].
    /// Never panics on adversarial `decoder` state otherwise:
    /// [`Decoder::target`] is bounded to `[0, total + distinct)`, so the scan
    /// always finds a symbol or the escape band inside the table.
    #[must_use]
    pub fn decode(&mut self, decoder: &mut Decoder) -> Option<usize> {
        assert!(
            self.distinct > 0,
            "Ppm::decode called on an empty table; nothing was ever coded into it"
        );
        let denom = u64::from(self.space());
        let target = decoder.target(denom);
        if target >= u64::from(self.total) {
            let total = u64::from(self.total);
            decoder.decode(total, denom, denom);
            return None;
        }
        let (symbol, low, high) = crate::scan_for_target(&self.freq, target);
        decoder.decode(low, high, denom);
        self.observe(symbol);
        Some(symbol)
    }
}

#[cfg(test)]
mod tests;
