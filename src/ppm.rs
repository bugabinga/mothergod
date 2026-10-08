//! PPM-style escape table: [`Ppm`], an adaptive frequency primitive in which
//! a symbol never observed is a distinct, separately priced event rather
//! than a Laplace floor of 1 (`JOURNAL` S1-P3, "PPM escape for literal
//! contexts"). Not a port: the founding session never implemented escape
//! coding, so there is no archive behavior to carry forward (ADR-0006).
//!
//! Classic PPM Method C: an escape's frequency is the number of *distinct*
//! symbols already observed, so a context with a longer record of adding new
//! symbols escapes more readily than one that keeps recoding the same few.
//!
//! The only live consumer is [`crate::literal::PpmExpertState`], which blends
//! one table per bank into [`crate::literal`]'s mix as an additive term and
//! reads `Ppm::probability`; escape coding ([`Ppm::encode_escape`],
//! [`Ppm::decode`]) has no bitstream caller. [`Ppm::distinct`] only ever
//! grows: [`Ppm::observe`] has no decrement or reset path, so "has this
//! table seen anything" never goes back to `false`.
//!
//! Every fallback, wiring and calibration slice tried on this lead, with its
//! numbers and mechanism, is in `JOURNAL` S2-R6, S2-R16, S2-R17, S2-A100,
//! S2-R18, S2-R19 and S2-R21.

use crate::coder::{Decoder, Encoder};

/// An order-0 adaptive frequency table over `0..alphabet_len` symbols that
/// distinguishes a genuinely unseen symbol from one merely rare, and prices
/// that distinction as an explicit escape event (PPM Method C).
///
/// Unlike [`crate::model::Model`], every symbol starts at frequency **0**,
/// not 1: nothing is coded as "possible but unlikely" until this table has
/// actually observed it. The escape event's own frequency is
/// [`Self::distinct`], the count of symbols observed at least once, so the
/// coding space at any moment is `total + distinct`: `total` slots split
/// among the symbols already seen, `distinct` slots reserved for "escape,
/// try a lower order instead."
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
    /// Panics if `alphabet_len` is zero: a table over no symbols could
    /// escape every input for no reason, which is a caller bug fixed at
    /// construction, never something adversarial input can trigger.
    #[must_use]
    pub fn new(alphabet_len: usize) -> Self {
        assert!(alphabet_len > 0, "Ppm alphabet must be non-empty");
        Self {
            freq: vec![0; alphabet_len],
            total: 0,
            distinct: 0,
        }
    }

    /// Count of symbols observed at least once — the escape event's own
    /// frequency under PPM Method C.
    #[must_use]
    pub fn distinct(&self) -> u32 {
        self.distinct
    }

    /// The full coding space this table currently divides its distribution
    /// over: `total` slots for symbols already seen plus `distinct` slots
    /// for the escape event ([`Self::price_symbol`]/[`Self::price_escape`]'s
    /// shared denominator, and [`Self::encode`]/[`Self::decode`]'s shared
    /// coder width).
    fn space(&self) -> u32 {
        self.total + self.distinct
    }

    /// `true` if `symbol` has never been observed in this table: coding it
    /// now must go through [`Self::encode_escape`]/be reported as
    /// [`None`] by [`Self::decode`], never [`Self::encode`].
    #[must_use]
    pub fn is_escape(&self, symbol: usize) -> bool {
        self.freq[symbol] == 0
    }

    /// Records one occurrence of `symbol`, raising its frequency and, on a
    /// symbol's first occurrence, [`Self::distinct`]. Rescales every count
    /// (symbol's own included) once `total` crosses a fixed limit,
    /// preserving zero entries exactly (`(0 + 1) >> 1 == 0`) so
    /// [`Self::is_escape`] never flips from `true` to `false` on its own.
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

    /// `-log2` price, in bits, of coding `symbol` under this table's
    /// current distribution, or [`None`] if `symbol` has never been
    /// observed here ([`Self::is_escape`] would return `true`) — the
    /// caller must escape instead, priced by [`Self::price_escape`].
    ///
    /// Advisory only, like [`crate::lz`]'s `PriceCounts::price`: never
    /// drives [`Encoder`]/[`Decoder`] itself, so libm's `log2` last-ulp
    /// behavior cannot desync a bitstream (ADR-0024's determinism rule
    /// binds the coding path, not this estimate).
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

    /// `symbol`'s own linear-space share of [`Self::space`], or `0.0` if
    /// `symbol` has never been observed. The same distribution
    /// [`Self::price_symbol`] reports in bits, for a caller that mixes
    /// raw probabilities instead of summing prices
    /// ([`crate::literal::Literal::mix_ppm`]).
    #[must_use]
    pub(crate) fn probability(&self, symbol: usize) -> f64 {
        if self.freq[symbol] == 0 {
            0.0
        } else {
            f64::from(self.freq[symbol]) / f64::from(self.space())
        }
    }

    /// `-log2` price, in bits, of the escape event itself: this context
    /// has nothing to say about the symbol actually coming next, fall back
    /// to a lower order. A table that has observed nothing yet
    /// (`total == 0`) escapes for free (`0.0` bits): there is no evidence
    /// here to weigh against escaping, so the decision costs nothing,
    /// mirroring how [`crate::model::Model`] would need no evidence to
    /// justify its Laplace floor either.
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

    /// Codes `symbol` through `encoder` under this table's current
    /// distribution (the `total`-wide band, PPM Method C's non-escape
    /// space), then [`Self::observe`]s it.
    ///
    /// # Panics
    ///
    /// Panics if `symbol` has never been observed (`price_symbol` would
    /// return [`None`]): coding an unseen symbol as if it had weight is a
    /// caller bug — the caller must have already checked
    /// [`Self::is_escape`] and called [`Self::encode_escape`] instead.
    /// Also panics if `symbol >= alphabet_len`, same bound as
    /// [`crate::model::Model::encode`].
    pub fn encode(&mut self, encoder: &mut Encoder, symbol: usize) {
        assert!(
            !self.is_escape(symbol),
            "Ppm::encode called on a never-observed symbol; use encode_escape"
        );
        crate::encode_symbol(encoder, &self.freq, symbol, u64::from(self.space()));
        self.observe(symbol);
    }

    /// Codes the escape event through `encoder`: the band
    /// `[total, total + distinct)` in this table's `total + distinct`-wide
    /// space. Does not call [`Self::observe`] — escaping says nothing
    /// about which symbol comes next, only that it isn't one already seen
    /// here; the caller observes the real symbol (if learning it into this
    /// table at all) once a lower order has decoded it.
    ///
    /// # Panics
    ///
    /// Panics if this table has observed nothing yet (`distinct == 0`):
    /// escaping from an empty table has zero width to code
    /// (`price_escape` returns `0.0` for exactly this state, meaning the
    /// event needs no bits and this method should not be called at all).
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

    /// Decodes one event from `decoder` under this table's current
    /// distribution: [`Some`] with the symbol and this table's own state
    /// updated via [`Self::observe`], matching [`Self::encode`]; or
    /// [`None`] if the escape band was decoded, state left untouched,
    /// matching [`Self::encode_escape`].
    ///
    /// # Panics
    ///
    /// Panics if this table has observed nothing yet (`distinct == 0`),
    /// same bound as [`Self::encode_escape`]: nothing was ever coded into
    /// an empty table, so nothing should ever be decoded from one either.
    ///
    /// Never panics on adversarial `decoder` state otherwise:
    /// [`Decoder::target`] is mathematically bounded to
    /// `[0, total + distinct)`, and the scan below always finds a symbol
    /// or the escape band before running past the end of the table,
    /// regardless of what bytes produced `decoder`'s internal value.
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
