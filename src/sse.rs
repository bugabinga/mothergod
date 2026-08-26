//! Secondary symbol estimation: [`Sse`], a standalone adaptive probability
//! calibration primitive for ROADMAP M3's oldest standing lead (`JOURNAL`
//! S1-P1, "SSE", targeting the five zstd text holdouts). Not a port: S1-P1
//! is a literature lead the founding session never implemented (grepped
//! `research/imports/session-1/` clean of any SSE/APM code), so there is no
//! archive behavior to carry forward, unlike every other module in this
//! crate (ADR-0006).
//!
//! An SSE (secondary symbol estimation, Mahoney 2005; also called an APM,
//! adaptive probability map) stage takes a primary model's probability
//! estimate for one outcome plus a small side context, and looks up a
//! separately-adapted, better-calibrated probability for that same
//! `(context, estimate)` pair — it corrects a primary model's systematic
//! bias ("when the mixer says 70%, the true rate in this context is
//! actually 85%") rather than predicting from raw symbol history itself.
//!
//! **Design deviation from the classic APM.** PAQ's APM warps its bin
//! spacing through a logit transform (`stretch = ln(p / (1 - p))`,
//! `squash` its inverse) so bins concentrate resolution near 0 and 1,
//! where calibration errors cost the most. This crate's `clippy.toml`
//! forbids every libm transcendental crate-wide (ADR-0024): a mixing
//! weight or probability the encoder computes and the decoder must
//! reproduce bit-for-bit cannot depend on a function libm implementations
//! disagree on in the last ulp. [`Literal`](crate::literal::Literal)
//! solved the identical problem for its own weight update by vendoring a
//! deterministic `exp`; this module sidesteps it instead, by choosing
//! *linear*-domain bins (evenly spaced across `[0.0, 1.0]`) rather than
//! log-domain ones. The calibration mechanism — quantize into two
//! neighboring bins, interpolate for a read, nudge both toward the
//! observed outcome on a write — is the same idea `stretch`/`squash`
//! serves; only the bin spacing is simpler, at the cost of coarser
//! resolution near the extremes than a production APM would want. Bit-
//! exact reproducibility (`+ - * /` and [`f64::clamp`] only) matters more
//! here than that resolution, so this crate takes the trade.
//!
//! **Remaining scope.** This module is not yet reachable from
//! [`crate::codec`]: nothing in this crate has a binary (two-outcome)
//! probability stream to calibrate yet. The flag stream `codec.rs` codes
//! (literal / match / rep) is three-ary, and the six-expert literal mixer
//! ([`crate::literal::Literal`]) codes a 256-ary symbol directly rather
//! than a sequence of binary decisions, so wiring [`Sse`] against either
//! needs a decomposition this port does not build. This slice also closes
//! a second prerequisite: [`crate::coder::Encoder::encode_bit`]/
//! [`crate::coder::Decoder::decode_bit`] now let a caller drive the range
//! coder from an arbitrary probability instead of only a
//! [`crate::model::Model`]-derived frequency, and this module's own test
//! suite proves the two work together — an `Sse`-calibrated probability,
//! coded through that primitive, round-trips exactly and costs far fewer
//! bits than a fixed 50/50 split on the same skewed sequence.
//!
//! Decomposing the flag model's binary "is this a copy, not a literal"
//! sub-decision and wiring `Sse` behind it was tried and reverted
//! (`research/JOURNAL.md` S2-R1): measured on the train/sealed corpus, an
//! `Sse` stage over an already order-0-adaptive binary decision showed no
//! train improvement and one sealed regression, rather than the hoped-for
//! move toward the five zstd text holdouts S1-P1 names. `research/JOURNAL.md`
//! S1-P1's entry has the numbers and the mechanism read; the next attempt,
//! if any, wants a compound estimate to calibrate (the literal mixer's own
//! eventual binary decomposition is the obvious candidate), not another raw
//! `Model` split.

/// Number of probability bins per context: 33 evenly spaced points across
/// `[0.0, 1.0]` (32 intervals), the classic PAQ/APM bin count (Mahoney
/// 2005) — one bin per point, `1.0 / 32.0` apart, so bin `i` starts life
/// at exactly `i / 32.0`.
const BINS: usize = 33;

/// Learning rate for [`Sse::update`]: how far a bin's calibrated
/// probability moves toward each observed outcome, per update. An
/// exponential moving average, not a running count-based mean — matching
/// this crate's other adaptive tables ([`crate::model::Model`]'s
/// increment-then-halve rule, [`crate::literal::Literal`]'s mixing-weight
/// update), which all favor recent evidence over an unweighted lifetime
/// average, because compressible data is rarely stationary (`JOURNAL`
/// S1-L4).
const LEARNING_RATE: f64 = 1.0 / 32.0;

/// The smallest probability [`Sse::refine`] ever returns, and `1.0` minus
/// this is the largest. A probability of exactly `0.0` or `1.0` costs
/// infinite or zero bits under `-log2`, and declares an outcome
/// impossible — a claim no adaptive table fed by finite, noisy evidence
/// should ever make (the same "nothing is ever impossible to code"
/// guarantee [`crate::model::Model`] gives by starting every frequency at
/// 1, applied to a continuous probability instead of a discrete count).
const MIN_PROBABILITY: f64 = 1.0 / 4096.0;
const MAX_PROBABILITY: f64 = 1.0 - MIN_PROBABILITY;

/// Adaptive probability calibration table, `BINS` bins per context.
///
/// Every context's bins start at the identity mapping (bin `i`'s value is
/// its own position, `i / (BINS - 1)`), so a freshly constructed [`Sse`]
/// is a no-op: [`Self::refine`] returns (approximately) its input `p`
/// until [`Self::update`] has adapted that context's bins away from
/// identity.
#[derive(Debug, Clone)]
pub struct Sse {
    contexts: usize,
    /// `contexts * BINS` calibrated probabilities, context-major.
    table: Vec<f64>,
}

impl Sse {
    /// A fresh table over `contexts` independent contexts, every bin
    /// initialized to the identity mapping (see the struct docs).
    ///
    /// # Panics
    ///
    /// Panics if `contexts` is zero: a table with no contexts could
    /// calibrate nothing, which is a caller bug fixed at construction,
    /// never something adversarial input can trigger.
    #[must_use]
    pub fn new(contexts: usize) -> Self {
        assert!(contexts > 0, "Sse must have at least one context");
        let mut table = vec![0.0; contexts * BINS];
        for context in 0..contexts {
            for bin in 0..BINS {
                #[allow(
                    clippy::cast_precision_loss,
                    reason = "bin < BINS (33) and BINS - 1 (32): both exact in f64"
                )]
                {
                    table[context * BINS + bin] = bin as f64 / (BINS - 1) as f64;
                }
            }
        }
        Self { contexts, table }
    }

    /// The number of independent contexts this table calibrates.
    #[must_use]
    pub fn contexts(&self) -> usize {
        self.contexts
    }

    /// The two adjacent bin indices `p` falls between, and how far past
    /// the lower one it sits (`0.0` at the lower bin, `1.0` at the
    /// upper). `p` outside `[0.0, 1.0]` is clamped rather than treated as
    /// an error: a primary model's probability estimate is a caller-
    /// computed float that floating-point rounding could nudge a hair
    /// past either end, and clamping is a strictly better response than
    /// a panic or an out-of-bounds bin index for that case.
    fn position(p: f64) -> (usize, f64) {
        #[allow(
            clippy::cast_precision_loss,
            reason = "BINS is 33: exact in f64 well inside its 53-bit mantissa"
        )]
        let scaled = p.clamp(0.0, 1.0) * (BINS - 1) as f64;
        let lower = scaled.floor();
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "scaled is clamp(0.0, 1.0) * 32.0, so floor(scaled) is in [0.0, 32.0]: always fits usize"
        )]
        let lower_index = (lower as usize).min(BINS - 2);
        #[allow(
            clippy::cast_precision_loss,
            reason = "lower_index < BINS - 1 (32): exact in f64"
        )]
        let fraction = scaled - lower_index as f64;
        (lower_index, fraction)
    }

    /// Calibrated probability for `context`'s current table at input
    /// probability `p`: linear interpolation between the two bins `p`
    /// falls between, clamped to `[MIN_PROBABILITY, MAX_PROBABILITY]`.
    ///
    /// # Panics
    ///
    /// Panics if `context >= self.contexts()`: contexts come from a
    /// caller's own fixed indexing scheme, the same bound
    /// [`crate::model::Model::encode`] documents for out-of-range
    /// symbols, not from adversarial input.
    #[must_use]
    pub fn refine(&self, context: usize, p: f64) -> f64 {
        assert!(context < self.contexts, "Sse context out of range");
        let base = context * BINS;
        let (lower_index, fraction) = Self::position(p);
        let value = self.table[base + lower_index].mul_add(
            1.0 - fraction,
            self.table[base + lower_index + 1] * fraction,
        );
        value.clamp(MIN_PROBABILITY, MAX_PROBABILITY)
    }

    /// Adapts `context`'s two bins nearest `p` toward the observed
    /// `outcome` (`1.0` if true, `0.0` if false), weighted by how close
    /// `p` sits to each bin (`position`'s `fraction`). Independent
    /// of [`Self::refine`]: a caller decides for itself whether to refine
    /// before observing the outcome, same shape as
    /// [`crate::model::Model::encode`] coding a symbol and updating its
    /// table in the same call.
    ///
    /// # Panics
    ///
    /// Panics if `context >= self.contexts()`, same bound as
    /// [`Self::refine`].
    pub fn update(&mut self, context: usize, p: f64, outcome: bool) {
        assert!(context < self.contexts, "Sse context out of range");
        let base = context * BINS;
        let (lower_index, fraction) = Self::position(p);
        let target = if outcome { 1.0 } else { 0.0 };
        let lower = base + lower_index;
        let upper = lower + 1;
        self.table[lower] += LEARNING_RATE * (1.0 - fraction) * (target - self.table[lower]);
        self.table[upper] += LEARNING_RATE * fraction * (target - self.table[upper]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coder::{Decoder, Encoder};

    #[test]
    #[should_panic(expected = "at least one context")]
    fn zero_contexts_panics() {
        let _ = Sse::new(0);
    }

    #[test]
    fn fresh_table_is_near_identity() {
        let sse = Sse::new(1);
        for tenth in 0..=10 {
            let p = f64::from(tenth) / 10.0;
            let refined = sse.refine(0, p);
            assert!(
                (refined - p).abs() < 0.02,
                "p={p}, refined={refined}, expected near-identity on a fresh table"
            );
        }
    }

    #[test]
    fn output_is_always_clamped_away_from_extremes() {
        let mut sse = Sse::new(1);
        for _ in 0..10_000 {
            sse.update(0, 1.0, true);
        }
        let refined = sse.refine(0, 1.0);
        assert!(
            (MIN_PROBABILITY..1.0).contains(&refined),
            "refined={refined} must stay inside (0.0, 1.0) even after {} updates \
             all pushing toward 1.0",
            10_000
        );

        let mut sse = Sse::new(1);
        for _ in 0..10_000 {
            sse.update(0, 0.0, false);
        }
        let refined = sse.refine(0, 0.0);
        assert!(
            refined > 0.0 && refined <= MAX_PROBABILITY,
            "refined={refined} must stay inside (0.0, 1.0) even after {} updates \
             all pushing toward 0.0",
            10_000
        );
    }

    #[test]
    fn converges_toward_the_true_observed_rate() {
        // The primary model is uninformative (always claims p=0.5), but
        // the true outcome rate at this context is 90%: a working SSE
        // stage must learn to correct the primary estimate toward 0.9,
        // which is exactly the systematic-bias correction S1-P1 is for.
        let mut sse = Sse::new(1);
        let rng = crate::test_support::Xorshift32::new(0xA5A5_5A5A);
        for state in rng.take(20_000) {
            let outcome = state % 10 != 0; // true 90% of the time
            sse.update(0, 0.5, outcome);
        }
        let refined = sse.refine(0, 0.5);
        assert!(
            (refined - 0.9).abs() < 0.03,
            "refined={refined}, expected convergence near the true rate 0.9"
        );
    }

    #[test]
    fn contexts_adapt_independently() {
        let mut sse = Sse::new(2);
        for _ in 0..5000 {
            sse.update(0, 0.5, true);
            sse.update(1, 0.5, false);
        }
        let refined0 = sse.refine(0, 0.5);
        let refined1 = sse.refine(1, 0.5);
        assert!(
            refined0 > 0.8,
            "context 0 saw only true outcomes, refined={refined0}"
        );
        assert!(
            refined1 < 0.2,
            "context 1 saw only false outcomes, refined={refined1}"
        );
    }

    #[test]
    fn refine_is_monotonic_in_input_probability_on_a_fresh_table() {
        let sse = Sse::new(1);
        let mut previous = sse.refine(0, 0.0);
        for hundredth in 1..=100 {
            let p = f64::from(hundredth) / 100.0;
            let refined = sse.refine(0, p);
            assert!(
                refined >= previous,
                "refine must be non-decreasing in p on an untrained table: \
                 p={p}, refined={refined}, previous={previous}"
            );
            previous = refined;
        }
    }

    #[test]
    fn out_of_range_probability_is_clamped_not_a_panic() {
        let sse = Sse::new(1);
        let low = sse.refine(0, -1.0);
        let high = sse.refine(0, 2.0);
        assert!((low - MIN_PROBABILITY).abs() < 1e-6);
        assert!((high - MAX_PROBABILITY).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "context out of range")]
    fn refine_out_of_range_context_panics() {
        let sse = Sse::new(2);
        let _ = sse.refine(2, 0.5);
    }

    #[test]
    #[should_panic(expected = "context out of range")]
    fn update_out_of_range_context_panics() {
        let mut sse = Sse::new(2);
        sse.update(2, 0.5, true);
    }

    #[test]
    fn contexts_reports_the_constructed_count() {
        assert_eq!(Sse::new(5).contexts(), 5);
    }

    #[test]
    fn calibrated_probability_round_trips_and_costs_less_than_a_fixed_split() {
        // The mechanism S1-P1 names as this primitive's reason to exist,
        // proven end to end: an uninformative primary estimate (constant
        // 0.5, same as converges_toward_the_true_observed_rate above) that
        // Sse calibrates toward a skewed context's true rate, fed through
        // coder::Encoder::encode_bit/Decoder::decode_bit instead of just
        // compared against refine()'s return value. Not yet wired into
        // codec.rs (see this module's "Remaining scope" docs), but proves
        // the two pieces compose correctly before that wiring decision.
        let outcomes: Vec<bool> = crate::test_support::Xorshift32::new(0x5EED_5EED)
            .take(2000)
            .map(|state| state % 10 != 0) // true 90% of the time
            .collect();

        let mut sse = Sse::new(1);
        let mut enc = Encoder::new();
        for &outcome in &outcomes {
            let p = sse.refine(0, 0.5);
            enc.encode_bit(outcome, p);
            sse.update(0, 0.5, outcome);
        }
        let calibrated_bytes = enc.finish();

        let mut fixed = Encoder::new();
        for &outcome in &outcomes {
            fixed.encode_bits(u32::from(outcome), 1);
        }
        let fixed_bytes = fixed.finish();

        assert!(
            calibrated_bytes.len() < fixed_bytes.len() * 2 / 3,
            "Sse-calibrated {} bytes should be well below fixed-50/50 {} bytes for a \
             90%-skewed sequence",
            calibrated_bytes.len(),
            fixed_bytes.len()
        );

        let mut sse = Sse::new(1);
        let mut dec = Decoder::new(&calibrated_bytes);
        for &outcome in &outcomes {
            let p = sse.refine(0, 0.5);
            let bit = dec.decode_bit(p);
            assert_eq!(bit, outcome, "round-trip mismatch");
            sse.update(0, 0.5, bit);
        }
    }
}
