//! [`Ppm`]: an adaptive frequency table in which a never-observed symbol has
//! probability 0 and the escape event owns a share of the space, not a
//! Laplace floor of 1 (`JOURNAL` S1-P3).
//!
//! PPM Method C: the escape's frequency is the number of *distinct* symbols
//! already observed, so a context that keeps adding new symbols reserves
//! more for escape than one that recodes the same few.
//!
//! The only consumer is [`crate::literal::PpmExpertState`], which reads
//! `Ppm::probability` and [`Ppm::observe`]; the table codes nothing itself.

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

    /// `true` if `symbol` has never been observed.
    #[must_use]
    pub fn is_escape(&self, symbol: usize) -> bool {
        self.freq[symbol] == 0
    }

    /// Records one occurrence of `symbol`. Rescaling preserves zero entries
    /// exactly (`(0 + 1) >> 1 == 0`), so a symbol once observed never drops
    /// back to unseen and `distinct` only grows.
    pub fn observe(&mut self, symbol: usize) {
        if self.is_escape(symbol) {
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

    /// `symbol`'s share of the `total + distinct` space, `0.0` if never
    /// observed.
    #[must_use]
    pub(crate) fn probability(&self, symbol: usize) -> f64 {
        if self.is_escape(symbol) {
            0.0
        } else {
            f64::from(self.freq[symbol]) / f64::from(self.total + self.distinct)
        }
    }
}

#[cfg(test)]
mod tests;
