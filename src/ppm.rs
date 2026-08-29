//! PPM-style escape table: [`Ppm`], a standalone adaptive frequency
//! primitive for ROADMAP M3's third standing lead (`JOURNAL` S1-P3, "PPM
//! escape for literal contexts"). Not a port: the founding session never
//! implemented PPM-style escape coding (grepped
//! `research/imports/session-1/mothergod.rs` clean of any escape/PPM code),
//! so there is no archive behavior to carry forward, same situation
//! [`crate::sse`] documented for S1-P1 (ADR-0006).
//!
//! **The gap this closes.** [`crate::model::Model`] and
//! [`crate::literal::Literal`]'s six expert banks all Laplace-smooth every
//! symbol to a starting frequency of 1, so "this symbol has never occurred
//! in this exact context" and "this symbol occurred once, long enough ago
//! to decay back near the floor" are indistinguishable from the table's own
//! state — there is no representable "unseen" signal for a caller to act
//! on. `JOURNAL` S1-R4's near-miss diagnosis named the fix directly: a
//! context should be able to say "I have never seen this symbol; escape to
//! a lower-order model" as its own explicit, separately-priced event,
//! classic PPM Method C: an escape's frequency is the number of *distinct*
//! symbols already observed at this order, so contexts with a longer track
//! record of adding new symbols escape more readily than contexts that keep
//! recoding the same few. Untried territory, and different from `JOURNAL`
//! S1-R5 (rejected): S1-R5 blended every context unconditionally toward
//! order-0, damaging exactly the well-trained contexts that most need to
//! stay confident; this primitive escapes only a genuinely never-seen
//! symbol; a well-trained context essentially never pays the escape cost.
//!
//! **Remaining scope.** This module is standalone and not yet reachable
//! from [`crate::literal`] or [`crate::codec`]: wiring it in means picking
//! where the escape's lower-order fallback lands (order-0? one of
//! [`crate::literal::Literal`]'s other five experts? a fresh dedicated
//! order-0 table?) and measuring the result against `bench::baseline`,
//! left to the next slice.

use crate::coder::{Decoder, Encoder};

/// Frequency increment applied to a symbol every time it is observed.
///
/// Same increment/rescale idiom as [`crate::model::Model`] (`INCREMENT`),
/// so this table decays stale evidence the same way every other adaptive
/// table in this crate does, rather than introducing a second tuning
/// constant with no stated reason to differ.
const INCREMENT: u32 = 12;

/// Total frequency past which [`Ppm::observe`] halves every count.
///
/// Same value as [`crate::model::Model::update`]'s `RESCALE_LIMIT`, for the
/// same reason: keeps `total` comfortably inside `u32`, and bounds how long
/// stale evidence can outweigh recent symbols. `(freq + 1) >> 1` rounds up,
/// but a zero entry stays zero (`(0 + 1) >> 1 == 0`), which is exactly the
/// invariant [`Ppm`] needs: rescaling must never turn "never seen" into
/// "seen".
const RESCALE_LIMIT: u32 = 65536;

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
        if self.freq[symbol] == 0 {
            return None;
        }
        let denom = f64::from(self.total + self.distinct);
        Some(-(f64::from(self.freq[symbol]) / denom).log2())
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
        let denom = f64::from(self.total + self.distinct);
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
            self.freq[symbol] > 0,
            "Ppm::encode called on a never-observed symbol; use encode_escape"
        );
        let low: u32 = self.freq[..symbol].iter().sum();
        let high = low + self.freq[symbol];
        encoder.encode(
            u64::from(low),
            u64::from(high),
            u64::from(self.total + self.distinct),
        );
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
        let denom = u64::from(self.total + self.distinct);
        let target = decoder.target(denom);
        if target >= u64::from(self.total) {
            let total = u64::from(self.total);
            decoder.decode(total, denom, denom);
            return None;
        }
        let mut symbol = 0;
        let mut low = 0u64;
        while low + u64::from(self.freq[symbol]) <= target {
            low += u64::from(self.freq[symbol]);
            symbol += 1;
        }
        let high = low + u64::from(self.freq[symbol]);
        decoder.decode(low, high, denom);
        self.observe(symbol);
        Some(symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "alphabet must be non-empty")]
    fn zero_alphabet_panics() {
        let _ = Ppm::new(0);
    }

    #[test]
    fn fresh_table_escapes_every_symbol_for_free() {
        let ppm = Ppm::new(4);
        for symbol in 0..4 {
            assert!(ppm.is_escape(symbol));
            assert_eq!(ppm.price_symbol(symbol), None);
        }
        assert!((ppm.price_escape() - 0.0).abs() < 1e-9);
        assert_eq!(ppm.distinct(), 0);
    }

    #[test]
    fn observing_a_symbol_clears_its_own_escape_flag_only() {
        let mut ppm = Ppm::new(4);
        ppm.observe(1);
        assert!(!ppm.is_escape(1));
        assert!(ppm.is_escape(0));
        assert!(ppm.is_escape(2));
        assert!(ppm.is_escape(3));
        assert_eq!(ppm.distinct(), 1);
    }

    #[test]
    fn distinct_counts_each_symbol_once_regardless_of_repeats() {
        let mut ppm = Ppm::new(4);
        ppm.observe(1);
        ppm.observe(1);
        ppm.observe(1);
        ppm.observe(2);
        assert_eq!(ppm.distinct(), 2);
    }

    #[test]
    fn price_symbol_drops_as_it_is_observed_more() {
        let mut ppm = Ppm::new(4);
        ppm.observe(0);
        let first = ppm.price_symbol(0).unwrap();
        ppm.observe(0);
        let second = ppm.price_symbol(0).unwrap();
        ppm.observe(0);
        let third = ppm.price_symbol(0).unwrap();
        assert!(second < first, "second={second} first={first}");
        assert!(third < second, "third={third} second={second}");
    }

    #[test]
    fn escape_price_rises_as_the_same_symbol_keeps_recurring() {
        // Method C: repeatedly observing one symbol without ever adding a
        // new one grows total while distinct stays at 1, so escape's own
        // share of the coding space shrinks and its price climbs — this
        // table becomes more confident it has seen everything relevant.
        let mut ppm = Ppm::new(4);
        ppm.observe(0);
        let first = ppm.price_escape();
        for _ in 0..20 {
            ppm.observe(0);
        }
        let later = ppm.price_escape();
        assert!(
            later > first,
            "later={later} first={first}: escape should get more expensive as one symbol \
             dominates uncontested"
        );
    }

    #[test]
    fn escape_price_is_lower_when_observations_keep_introducing_new_symbols() {
        // Two tables, same observation count (20), different mix: one all
        // repeats of a single symbol (distinct stays 1, matching the
        // "rises" test above), the other all-new symbols every time
        // (distinct grows in lockstep with total). Method C's escape share
        // is distinct / (total + distinct); constant novelty keeps that
        // share far larger than a context that stopped discovering
        // anything new after its first symbol.
        let mut all_repeats = Ppm::new(64);
        for _ in 0..20 {
            all_repeats.observe(0);
        }

        let mut all_new = Ppm::new(64);
        for symbol in 0..20 {
            all_new.observe(symbol);
        }

        assert!(
            all_new.price_escape() < all_repeats.price_escape(),
            "all_new={} all_repeats={}: a context that keeps discovering new symbols should \
             escape more cheaply than one that stopped after its first",
            all_new.price_escape(),
            all_repeats.price_escape()
        );
    }

    #[test]
    #[should_panic(expected = "never-observed symbol")]
    fn encode_on_unseen_symbol_panics() {
        let mut ppm = Ppm::new(4);
        let mut enc = Encoder::new();
        ppm.encode(&mut enc, 0);
    }

    #[test]
    #[should_panic(expected = "empty table")]
    fn encode_escape_on_empty_table_panics() {
        let mut ppm = Ppm::new(4);
        let mut enc = Encoder::new();
        ppm.encode_escape(&mut enc);
    }

    #[test]
    #[should_panic(expected = "empty table")]
    fn decode_on_empty_table_panics() {
        let mut ppm = Ppm::new(4);
        let mut dec = Decoder::new(&[]);
        let _ = ppm.decode(&mut dec);
    }

    /// Round-trips a mixed sequence of real symbols and escapes: the
    /// caller decides on the encode side whether a symbol is present via
    /// `is_escape`, exactly the decision a future wired-in caller (a lower
    /// order's coder) would make, and the decode side must recover both
    /// which symbols were coded and which positions escaped.
    #[test]
    fn mixed_symbols_and_escapes_round_trip() {
        let alphabet_len = 8;
        // First occurrence of every symbol here is deliberately an escape
        // (nothing to code yet); later occurrences of 0..3 are real.
        let sequence = [0usize, 1, 2, 0, 1, 0, 3, 0, 1, 2];

        let mut ppm = Ppm::new(alphabet_len);
        let mut enc = Encoder::new();
        let mut expect_escape = Vec::new();
        for &symbol in &sequence {
            let escape = ppm.is_escape(symbol);
            expect_escape.push(escape);
            if escape {
                if ppm.distinct() > 0 {
                    ppm.encode_escape(&mut enc);
                }
                ppm.observe(symbol);
            } else {
                ppm.encode(&mut enc, symbol);
            }
        }
        let bytes = enc.finish();

        let mut ppm = Ppm::new(alphabet_len);
        let mut dec = Decoder::new(&bytes);
        let mut got = Vec::new();
        for (&symbol, &escape) in sequence.iter().zip(expect_escape.iter()) {
            if escape {
                if ppm.distinct() > 0 {
                    let decoded = ppm.decode(&mut dec);
                    assert_eq!(decoded, None, "expected an escape decode");
                }
                ppm.observe(symbol);
                got.push(symbol);
            } else {
                let decoded = ppm.decode(&mut dec).expect("expected a real symbol decode");
                got.push(decoded);
            }
        }
        assert_eq!(got, sequence);
    }

    #[test]
    fn rescale_triggers_and_round_trip_still_holds() {
        // RESCALE_LIMIT is 65536 and every observe adds INCREMENT=12, so
        // 10,000 repeats of one already-known symbol crosses the halving
        // threshold several times over; round-trip must still hold on the
        // far side of every halving, and the symbol's frequency must never
        // decay back to zero (is_escape must stay false throughout).
        let mut ppm = Ppm::new(2);
        ppm.observe(0); // bootstrap: first occurrence is always a free escape
        let mut enc = Encoder::new();
        for _ in 0..10_000 {
            ppm.encode(&mut enc, 0);
            assert!(!ppm.is_escape(0));
        }
        let bytes = enc.finish();

        let mut ppm = Ppm::new(2);
        ppm.observe(0);
        let mut dec = Decoder::new(&bytes);
        for _ in 0..10_000 {
            assert_eq!(ppm.decode(&mut dec), Some(0));
        }
    }
}
