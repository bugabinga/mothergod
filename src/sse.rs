//! Secondary symbol estimation (Mahoney 2005, also called an APM): [`Table`],
//! an adaptive probability calibration primitive. It takes a primary model's
//! probability for one outcome plus a small side context and looks up a
//! separately adapted probability for that `(context, estimate)` pair,
//! correcting the primary model's systematic bias ("when the mixer says 70%,
//! the true rate in this context is 85%").
//!
//! Calibration quantizes `p` into two neighboring bins, interpolates for a
//! read, and nudges both toward the observed outcome on a write. [`Spacing`]
//! is the only axis the two tables differ on: [`Sse`] spaces its bins evenly
//! in linear probability, [`LogitSse`] evenly in [`stretch`]-space, which
//! concentrates resolution near 0 and 1.
//!
//! Every probability here feeds the coder, so it must be bit-exact between
//! encoder and decoder: the arithmetic is `+ - * /`, [`f64::clamp`],
//! [`f64::floor`] and [`crate::logistic`]'s deterministic `stretch`/`squash`,
//! never a libm transcendental (ADR-0024).
//!
//! Consumers: [`crate::bittree`]'s binary walk ([`Sse`]) and
//! [`crate::literal::SurpriseLogisticMix`] ([`LogitSse`]).

use std::marker::PhantomData;

use crate::logistic::{squash, stretch};

/// Bins per context: 33 points across `[0.0, 1.0]` (32 intervals, the classic
/// APM count), so under [`Linear`] bin `i` starts at exactly `i / 32.0`.
const BINS: usize = 33;

/// How far a bin moves toward each observed outcome per update: an
/// exponential moving average, favoring recent evidence over a lifetime mean
/// because compressible data is rarely stationary.
const LEARNING_RATE: f64 = 1.0 / 32.0;

/// The smallest probability [`Table::refine`] returns; `1.0` minus this is
/// the largest. Exactly `0.0` or `1.0` costs infinite bits when wrong, a
/// claim no table fed by finite evidence may make.
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
/// 0 and 1.
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

/// [`Table`] over [`Logit`] bins.
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
/// generic over this trait; since ADR-0063 retired its [`Sse`]
/// instantiation, only [`LogitSse`] instantiates it.
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
