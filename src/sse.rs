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
//! **Wired (`JOURNAL` S1-P1 closed, ADR-0038).** This slice also closed a
//! prerequisite: [`crate::coder::Encoder::encode_bit`]/
//! [`crate::coder::Decoder::decode_bit`] let a caller drive the range
//! coder from an arbitrary probability instead of only a
//! [`crate::model::Model`]-derived frequency, and this module's own test
//! suite proves the two work together — an `Sse`-calibrated probability,
//! coded through that primitive, round-trips exactly and costs far fewer
//! bits than a fixed 50/50 split on the same skewed sequence. Decomposing
//! the flag model's binary "is this a copy, not a literal" sub-decision
//! and wiring `Sse` behind it was tried and reverted (`research/JOURNAL.md`
//! S2-R1): an `Sse` stage over an already order-0-adaptive binary decision
//! showed no train improvement and one sealed regression — SSE earns its
//! keep calibrating a compound estimate, not a lone counter already
//! tracking its own rate. The next attempt calibrated exactly that: the
//! six-expert literal mixer's own blended probability at each node of
//! [`crate::bittree`]'s binary decomposition
//! ([`crate::literal::Literal::encode_sse`]/`decode_sse`, `research/JOURNAL.md`
//! S2-A60), which won on the train/sealed split (net train -0.36736 b/B,
//! both sealed kinds improved). `research/JOURNAL.md` S2-A60 has the full
//! numbers and mechanism read.

use std::marker::PhantomData;

use crate::logistic::{squash, stretch};

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

/// How a calibration table spaces its `BINS` bins across `[0.0, 1.0]`:
/// the only thing [`Sse`] and [`LogitSse`] differ in. Construction, the
/// interpolating read and the nudging write are [`Table`]'s, once.
pub trait Spacing {
    /// Names the table in its caller-bug panic messages.
    const NAME: &'static str;

    /// The two adjacent bin indices `p` falls between, and how far past
    /// the lower one it sits (`0.0` at the lower bin, `1.0` at the upper).
    /// `p` outside `[0.0, 1.0]` is clamped rather than treated as an
    /// error: a primary model's probability estimate is a caller-computed
    /// float that floating-point rounding could nudge a hair past either
    /// end, and clamping is a strictly better response than a panic or an
    /// out-of-bounds bin index for that case.
    fn position(p: f64) -> (usize, f64);

    /// One context's fresh bins: the identity mapping under this spacing,
    /// so a fresh table is (approximately) a no-op.
    fn identity() -> [f64; BINS];
}

/// Splits `scaled`, a position in `[0.0, BINS - 1]` bin units, into the
/// lower bin index and the fraction past it. The last bin pairs with its
/// predecessor (fraction `1.0`) so `lower + 1` is always a valid index.
fn split_position(scaled: f64) -> (usize, f64) {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "scaled is in [0.0, 32.0], so floor(scaled) always fits usize"
    )]
    let lower = (scaled.floor() as usize).min(BINS - 2);
    #[allow(
        clippy::cast_precision_loss,
        reason = "lower < BINS - 1 (32): exact in f64"
    )]
    let fraction = scaled - lower as f64;
    (lower, fraction)
}

/// `bin / (BINS - 1)`: bin `bin`'s position as a fraction of `[0.0, 1.0]`.
fn bin_fraction(bin: usize) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "bin < BINS (33) and BINS - 1 (32): both exact in f64"
    )]
    {
        bin as f64 / (BINS - 1) as f64
    }
}

/// Evenly spaced bins in linear probability space.
#[derive(Debug, Clone)]
pub struct Linear;

impl Spacing for Linear {
    const NAME: &'static str = "Sse";

    fn position(p: f64) -> (usize, f64) {
        #[allow(
            clippy::cast_precision_loss,
            reason = "BINS is 33: exact in f64 well inside its 53-bit mantissa"
        )]
        let scaled = p.clamp(0.0, 1.0) * (BINS - 1) as f64;
        split_position(scaled)
    }

    fn identity() -> [f64; BINS] {
        std::array::from_fn(bin_fraction)
    }
}

/// [`stretch`]'s value at [`MAX_PROBABILITY`], the positive half of the
/// bounded logit-domain range [`Logit`] spaces its bins across (`stretch`
/// is odd, so [`MIN_PROBABILITY`]'s value is its negation). Recomputed
/// rather than a `const`: [`stretch`] calls [`crate::logistic::ln`], not
/// itself `const fn`.
fn stretch_bound() -> f64 {
    stretch(MAX_PROBABILITY)
}

/// Evenly spaced bins in [`stretch`]-space, concentrating resolution near
/// 0 and 1 the way [`Sse`]'s own module doc says a production APM wants.
#[derive(Debug, Clone)]
pub struct Logit;

impl Spacing for Logit {
    const NAME: &'static str = "LogitSse";

    /// `p` is clamped to [`MIN_PROBABILITY`]/[`MAX_PROBABILITY`] first
    /// (same range [`Table::refine`]'s own output is clamped to) so
    /// `stretch` never sees an input outside the domain its own bound was
    /// computed from.
    fn position(p: f64) -> (usize, f64) {
        let bound = stretch_bound();
        let s = stretch(p.clamp(MIN_PROBABILITY, MAX_PROBABILITY)).clamp(-bound, bound);
        #[allow(
            clippy::cast_precision_loss,
            reason = "BINS is 33: exact in f64 well inside its 53-bit mantissa"
        )]
        let scaled = (s + bound) / (2.0 * bound) * (BINS - 1) as f64;
        split_position(scaled)
    }

    /// Bin `i` starts at [`squash`] of the evenly spaced *stretch*-domain
    /// point, so a fresh table is still (approximately) a no-op under this
    /// spacing.
    fn identity() -> [f64; BINS] {
        let bound = stretch_bound();
        std::array::from_fn(|bin| squash(-bound + bin_fraction(bin) * (2.0 * bound)))
    }
}

/// Adaptive probability calibration table, `BINS` bins per context, spaced
/// by `S`.
///
/// Every context's bins start at `S`'s identity mapping, so a freshly
/// constructed table is a no-op: [`Self::refine`] returns (approximately)
/// its input `p` until [`Self::update`] has adapted that context's bins
/// away from identity.
#[derive(Debug, Clone)]
pub struct Table<S> {
    contexts: usize,
    /// `contexts * BINS` calibrated probabilities, context-major.
    bins: Vec<f64>,
    spacing: PhantomData<S>,
}

/// [`Table`] over [`Linear`] bins.
pub type Sse = Table<Linear>;

/// Logit-domain counterpart to [`Sse`]'s bin spacing, a research candidate
/// (`research/JOURNAL.md` S2-A106): [`Sse`]'s own module doc records a
/// deliberate deviation from the classic APM (Mahoney 2005) because this
/// crate had no deterministic transcendental pair to spend on it at the
/// time (S2-A40). [`crate::logistic`] (S2-A101) built exactly that pair for
/// [`crate::literal::LogisticMix`]'s own mixing step and it already ships
/// on the real coding path (`FORMAT_VERSION` 5+), so the blocker no longer
/// holds. This is the untried side of that deviation: [`Table`] over
/// [`Logit`] bins, everything else identical to [`Sse`].
pub type LogitSse = Table<Logit>;

impl<S: Spacing> Table<S> {
    /// A fresh table over `contexts` independent contexts, every bin
    /// initialized to `S`'s identity mapping (see the struct docs).
    ///
    /// # Panics
    ///
    /// Panics if `contexts` is zero: a table with no contexts could
    /// calibrate nothing, which is a caller bug fixed at construction,
    /// never something adversarial input can trigger.
    #[must_use]
    pub fn new(contexts: usize) -> Self {
        assert!(contexts > 0, "{} must have at least one context", S::NAME);
        Self::filled(vec![0.0; contexts * BINS], contexts)
    }

    /// Fallible counterpart to [`Self::new`]: the same fresh, identity-
    /// mapped table, but returns `Err` instead of aborting if the
    /// allocator cannot satisfy `contexts * BINS` entries.
    /// [`crate::literal::Literal::try_new`] uses this on the real decode
    /// path (hard rule 2, `rust-craft` skill's allocation-discipline,
    /// `tests/torture.rs`, #453); [`Self::new`] stays the panicking
    /// constructor the encoder and every test use.
    ///
    /// # Panics
    ///
    /// Same as [`Self::new`]: `contexts` zero is a caller bug, never
    /// something adversarial input can trigger.
    pub(crate) fn try_new(contexts: usize) -> Result<Self, std::collections::TryReserveError> {
        assert!(contexts > 0, "{} must have at least one context", S::NAME);
        Ok(Self::filled(
            crate::try_filled_vec(contexts * BINS, 0.0)?,
            contexts,
        ))
    }

    /// Writes `S`'s identity row into every context of `table`, already
    /// sized to `contexts * BINS`. [`Self::new`] and [`Self::try_new`]
    /// differ only in how `table` was allocated, never in what fills it.
    fn filled(mut table: Vec<f64>, contexts: usize) -> Self {
        let identity = S::identity();
        for row in table.as_chunks_mut::<BINS>().0 {
            *row = identity;
        }
        Self {
            contexts,
            bins: table,
            spacing: PhantomData,
        }
    }

    /// The number of independent contexts this table calibrates.
    #[must_use]
    pub fn contexts(&self) -> usize {
        self.contexts
    }

    /// [`Spacing::position`] under this table's `S`.
    fn position(p: f64) -> (usize, f64) {
        S::position(p)
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
        assert!(context < self.contexts, "{} context out of range", S::NAME);
        let (lower_index, fraction) = Self::position(p);
        let lower = context * BINS + lower_index;
        self.bins[lower]
            .mul_add(1.0 - fraction, self.bins[lower + 1] * fraction)
            .clamp(MIN_PROBABILITY, MAX_PROBABILITY)
    }

    /// Adapts `context`'s two bins nearest `p` toward the observed
    /// `outcome` (`1.0` if true, `0.0` if false) by `LEARNING_RATE`,
    /// weighted by how close `p` sits to each bin (`position`'s
    /// `fraction`). Independent of [`Self::refine`]: a caller decides for
    /// itself whether to refine before observing the outcome, same shape
    /// as [`crate::model::Model::encode`] coding a symbol and updating its
    /// table in the same call.
    ///
    /// # Panics
    ///
    /// Panics if `context >= self.contexts()`, same bound as
    /// [`Self::refine`].
    pub fn update(&mut self, context: usize, p: f64, outcome: bool) {
        assert!(context < self.contexts, "{} context out of range", S::NAME);
        let (lower_index, fraction) = Self::position(p);
        let target = if outcome { 1.0 } else { 0.0 };
        let lower = context * BINS + lower_index;
        self.bins[lower] += LEARNING_RATE * (1.0 - fraction) * (target - self.bins[lower]);
        self.bins[lower + 1] += LEARNING_RATE * fraction * (target - self.bins[lower + 1]);
    }
}

/// [`Sse`] and [`LogitSse`] in exactly the shape a generic mixer needs:
/// both differ only in [`Spacing`], never in construction or the two public
/// methods' own signatures. [`crate::literal::SurpriseLogisticMix`] is
/// generic over this trait so its one walk serves both calibration tables,
/// rather than two structs and two walks that differ only in which table
/// they call.
pub trait Calibrate: Sized {
    /// Same contract as [`Table::new`].
    fn new(contexts: usize) -> Self;
    /// Same contract as `Table::try_new`.
    fn try_new(contexts: usize) -> Result<Self, std::collections::TryReserveError>;
    /// Same contract as [`Table::refine`].
    fn refine(&self, context: usize, p: f64) -> f64;
    /// Same contract as [`Table::update`].
    fn update(&mut self, context: usize, p: f64, outcome: bool);
}

impl<S: Spacing> Calibrate for Table<S> {
    fn new(contexts: usize) -> Self {
        Self::new(contexts)
    }

    fn try_new(contexts: usize) -> Result<Self, std::collections::TryReserveError> {
        Self::try_new(contexts)
    }

    fn refine(&self, context: usize, p: f64) -> f64 {
        Self::refine(self, context, p)
    }

    fn update(&mut self, context: usize, p: f64, outcome: bool) {
        Self::update(self, context, p, outcome);
    }
}

#[cfg(test)]
mod tests;
