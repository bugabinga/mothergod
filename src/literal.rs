//! Six-expert context-mixing literal model: [`Literal`], the entropy stage
//! for the bytes an LZ parse ([`crate::lz`]) leaves as literals.
//!
//! Six experts each keep an order-0 frequency table over the 256 byte values
//! in their own context: a fast and a slow order-1 hash, an order-0
//! catch-all, a 12-bit order-2 hash, a position/nibble "alignment" hash, and
//! a 12-bit alphanumeric rolling word hash. Their per-symbol counts blend
//! under weights adapted after every symbol by an exponentiated-gradient
//! rule (Mahoney 2005). The weight vector is selected by a small
//! `(prev-byte nibble, after-copy)` key, so the mixer favors different
//! experts after a match than after a run of literals.
//!
//! The coding paths every frame this build decodes takes:
//!
//! - [`Literal::encode_logit_sse`]/[`Literal::decode_logit_sse`]: each
//!   expert's estimate `crate::logistic::stretch`ed into the logit domain
//!   and blended by [`SurpriseLogisticMix`], whose step size comes from a
//!   fast EMA of a key's squared error against a slower one, the result
//!   calibrated through [`SurpriseLogisticMix`]'s bins, even in
//!   `crate::logistic::stretch`-space. The wire path of every frame
//!   except a transposed one.
//! - [`Literal::encode_column`]/[`Literal::decode_column`]: a seventh,
//!   column-keyed expert ([`ColumnExpertState`]) blended into the six
//!   before `Sse` calibration. The wire path of a transposed frame.
//!
//! [`Literal::ideal_cost_bits_sse`] prices the blended `cum` table as 8
//! calibrated binary decisions and is no frame's wire path; the benchmark
//! harness keeps it as the pricing baseline.
//!
//! Determinism: the weight update runs on the encode and the decode path,
//! so everything it calls must be bit-identical on every platform, or the
//! two desync mid-frame. `f64::exp` is libm's and is not, so `exp` builds
//! `e^x` from IEEE-754 basic operations only, and `clippy.toml` forbids the
//! libm transcendentals crate-wide (ADR-0024).

use std::num::NonZeroUsize;

use crate::Rate;
use crate::bittree;
use crate::coder::{Decoder, Encoder};
use crate::logistic::{squash, stretch};
use crate::sse::{LogitSse, MIN_PROBABILITY, Sse};

/// Number of context predictors blended for every literal byte.
const EXPERTS: usize = 6;

/// Width of a byte's high nibble, the part of the previous byte the weight
/// key and the alignment expert read.
const NIBBLE_BITS: u32 = 4;

/// Values of a byte's high nibble.
const NIBBLES: usize = 1 << NIBBLE_BITS;

/// Number of distinct mixing-weight vectors: one per `(prev-byte nibble,
/// after-copy)` key.
const WEIGHT_CONTEXTS: usize = NIBBLES * 2;

/// Byte alphabet every context bank models: the one [`bittree`] walks.
const ALPHABET: usize = bittree::ALPHABET;
/// [`ALPHABET`] as a `u32`, the type a bank's `total` counts in.
#[allow(
    clippy::cast_possible_truncation,
    reason = "ALPHABET is the 256 byte values, which fit u32"
)]
const ALPHABET_U32: u32 = ALPHABET as u32;

// Bank layout: one contiguous bank space, sliced per expert so [`banks`]
// can address any of them with a single base offset. A slice holds one bank
// per value of the key [`banks`] packs for that expert.
// The order-2 and word slices are masked by their bank count in [`banks`], so
// each holds a power of two, written as the 12 bits of key it keeps.
const FAST_BASE: usize = 0;
const FAST_BANKS: usize = ALPHABET * 2;
const SLOW_BASE: usize = FAST_BASE + FAST_BANKS;
const SLOW_BANKS: usize = ALPHABET * 2;
const ORDER0_BASE: usize = SLOW_BASE + SLOW_BANKS;
const ORDER0_BANKS: usize = 1;
const ORDER2_BASE: usize = ORDER0_BASE + ORDER0_BANKS;
const ORDER2_BANKS: usize = 1 << 12;
const ALIGN_BASE: usize = ORDER2_BASE + ORDER2_BANKS;
/// Positions the alignment expert tells apart: `position % ALIGN_PHASES`.
const ALIGN_PHASES: usize = 4;
const ALIGN_BANKS: usize = ALIGN_PHASES * NIBBLES;
const WORD_BASE: usize = ALIGN_BASE + ALIGN_BANKS;
const WORD_BANKS: usize = 1 << 12;
/// Total context banks across all six experts.
const BANKS: usize = WORD_BASE + WORD_BANKS;

/// Where `bank`'s [`ALPHABET`] frequencies sit in a bank-major `freq` table,
/// [`Literal`]'s and [`ColumnExpertState`]'s alike.
fn bank_row(bank: usize) -> std::ops::Range<usize> {
    bank * ALPHABET..(bank + 1) * ALPHABET
}

/// Where `symbol`'s frequency sits in a bank-major `freq` table.
fn bank_slot(bank: usize, symbol: usize) -> usize {
    bank * ALPHABET + symbol
}

/// Rate of the fast-rate context expert (bank 0): higher increment and lower
/// ceiling than the other five, so it tracks recent bytes hardest and
/// forgets fastest.
const FAST_RATE: Rate = Rate::new(32, 6144);

/// Exponentiated-gradient learning rate and weight clamp.
const LEARNING_RATE: f64 = 0.05;
const MIN_WEIGHT: f64 = 1e-4;
const MAX_WEIGHT: f64 = 1e4;
/// Floor under the mixed-probability denominator so a weight update
/// never divides by (near) zero.
const MIN_DENOMINATOR: f64 = 1e-9;

/// Beyond this magnitude, `weights[expert] * exp(gradient)` already
/// saturates the `[MIN_WEIGHT, MAX_WEIGHT]` clamp every caller applies
/// next, regardless of which weight it started from: `MAX_WEIGHT /
/// MIN_WEIGHT` is `1e8`, and `exp(20) > 1e8`. `30` keeps a wide margin,
/// so `exp`'s approximation error can never flip which side of that
/// clamp a borderline gradient lands on.
const EXP_ARG_LIMIT: f64 = 30.0;

/// `e^x`, built from IEEE-754 basic operations only (`+ - * /`,
/// comparisons, and [`f64::round`]): no libm transcendental call, so
/// [`Literal::update`] computes the identical mixing weight on every
/// platform whether it runs on the encode or the decode path (ADR-0024).
///
/// Not a general-purpose `exp`: the argument is clamped to
/// `[-EXP_ARG_LIMIT, EXP_ARG_LIMIT]` first (see that constant's doc for
/// why the clamp never changes the caller's outcome). Classic range
/// reduction from there: `x = k*ln(2) + r` with `|r| <= ln(2)/2`, so the
/// polynomial only ever evaluates near zero, where a degree-7 Taylor
/// series is accurate to within `~2.5e-8`; `2^k` is exact repeated
/// doubling (`pow2`), never a `powi` call.
pub(crate) fn exp(x: f64) -> f64 {
    let x = x.clamp(-EXP_ARG_LIMIT, EXP_ARG_LIMIT);
    let k = (x / std::f64::consts::LN_2).round();
    let r = x - k * std::f64::consts::LN_2;
    let poly = 1.0
        + r * (1.0
            + r * (1.0 / 2.0
                + r * (1.0 / 6.0
                    + r * (1.0 / 24.0
                        + r * (1.0 / 120.0 + r * (1.0 / 720.0 + r * (1.0 / 5040.0)))))));
    #[allow(
        clippy::cast_possible_truncation,
        reason = "k = round(x / ln2) with |x| <= EXP_ARG_LIMIT (30), so |k| <= 44: always fits i32"
    )]
    let k = k as i32;
    poly * pow2(k)
}

/// `2^k` for integer `k`, by exact repeated doubling (multiplying by
/// exactly `2.0`, which has no rounding error) instead of a `powi` call.
/// `exp` only ever passes `|k| <= 44`, far below where this would
/// overflow.
fn pow2(k: i32) -> f64 {
    if k < 0 {
        return 1.0 / pow2(-k);
    }
    let mut result = 1.0;
    let mut base = 2.0;
    let mut remaining = k;
    while remaining > 0 {
        if remaining & 1 == 1 {
            result *= base;
        }
        base *= base;
        remaining >>= 1;
    }
    result
}

/// `1 << 32` as a float, the fixed-point scale [`Literal::mix`] blends
/// expert probabilities under. Spelled as a literal instead of a cast so
/// no `u64 -> f64` conversion lint applies to what is, exactly, a power
/// of two well inside `f64`'s 53-bit mantissa.
const FIXED_POINT_SCALE: f64 = 4_294_967_296.0;

/// `weight`'s share of `weight_sum`, scaled into [`FIXED_POINT_SCALE`]'s
/// fixed-point space and normalized by `bank_total` so an expert's raw
/// frequency count converts into that same fixed-point unit before
/// summing. Shared by every blend ([`Literal::mix`], [`Literal::scale6`],
/// [`Literal::mix7`]); only the `weight_sum` they normalize against
/// differs (six experts' worth vs. seven).
fn fixed_point_scale(weight: f64, weight_sum: f64, bank_total: f64) -> u64 {
    let normalized = weight / weight_sum;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "fixed-point scale factor: normalized weight is in (0,1], bank_total > 0, the product is always non-negative and truncation is the intended floor"
    )]
    {
        ((normalized * FIXED_POINT_SCALE) / bank_total) as u64
    }
}

/// The cumulative-frequency table over `mixed`, one [`FIXED_POINT_SCALE`]
/// fixed-point mass per symbol: each symbol's count is its mass shifted down
/// 16 bits, plus a Laplace floor of 1 so no symbol is impossible to code.
/// The tail of every blend ([`Literal::mix`], [`Literal::mix7`]).
fn cumulate(mixed: impl IntoIterator<Item = u64>) -> [u64; ALPHABET + 1] {
    let mut cum = [0u64; ALPHABET + 1];
    let mut acc = 0u64;
    for (symbol, mass) in mixed.into_iter().enumerate() {
        acc += (mass >> 16) + 1;
        cum[symbol + 1] = acc;
    }
    cum
}

/// Exponentiated-gradient weight update (Mahoney 2005): how far `weight`
/// moves given `estimate` (this expert's own prediction) versus `mixed`
/// (the blend's prediction), clamped to `[MIN_WEIGHT, MAX_WEIGHT]`.
/// Shared by [`Literal::update`]'s six real experts and
/// [`Literal::update_column_expert`]'s extra one, so both adapt by the same
/// rule.
fn adapt_weight(weight: f64, estimate: f64, mixed: f64, exp_fn: fn(f64) -> f64) -> f64 {
    let denominator = mixed.max(MIN_DENOMINATOR);
    let gradient = LEARNING_RATE * (estimate - mixed) / denominator;
    (weight * exp_fn(gradient)).clamp(MIN_WEIGHT, MAX_WEIGHT)
}

/// Per-byte modeling context every [`Literal`] coding path reads to select
/// which banks blend at this position: the previous two bytes (`0` before
/// the start of output), the output position, whether the previous token was
/// a copy (LZ match or rep) rather than a literal, and the rolling
/// alnum-only word hash ([`advance_word_hash`]).
///
/// [`Self::after_literal`]/[`Self::after_copy`] compute the next context,
/// so an encode pass and a decode pass over the same token stream share one
/// update rule instead of two copies that could drift apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Context {
    /// The byte immediately before `position`, or `0` at the start of
    /// output.
    pub prev1: u8,
    /// The byte two before `position`, or `0` near the start of output.
    pub prev2: u8,
    /// How many bytes precede this one in the output.
    pub position: usize,
    /// Whether the token immediately before this byte was a copy (match
    /// or rep) rather than a literal.
    pub after_copy: bool,
    /// Rolling hash over the alnum run leading up to this position
    /// (`JOURNAL` S1-A4's word-hash expert).
    pub word_hash: u32,
}

impl Context {
    /// The context for the byte after `byte` was coded as a literal.
    #[must_use]
    pub fn after_literal(self, byte: u8) -> Self {
        Self {
            prev1: byte,
            prev2: self.prev1,
            position: self.position + 1,
            after_copy: false,
            word_hash: advance_word_hash(self.word_hash, byte),
        }
    }

    /// The context for the byte after a copy (match or rep) token that
    /// replayed `bytes`.
    #[must_use]
    pub fn after_copy(self, bytes: &[u8]) -> Self {
        let word_hash = bytes
            .iter()
            .fold(self.word_hash, |wh, &b| advance_word_hash(wh, b));
        let (prev1, prev2) = match bytes.len() {
            0 => (self.prev1, self.prev2),
            1 => (bytes[0], self.prev1),
            n => (bytes[n - 1], bytes[n - 2]),
        };
        Self {
            prev1,
            prev2,
            position: self.position + bytes.len(),
            after_copy: true,
            word_hash,
        }
    }
}

/// Advances a rolling word hash by one byte: only alphanumeric bytes extend
/// the hash, anything else resets it to `0`, so the hash tracks how far into
/// the current alnum run a position is, never text spanning punctuation.
#[must_use]
pub fn advance_word_hash(word_hash: u32, byte: u8) -> u32 {
    if byte.is_ascii_alphanumeric() {
        word_hash.wrapping_mul(61).wrapping_add(u32::from(byte))
    } else {
        0
    }
}

/// Bank indices for [`Context`]'s six experts, and the mixing-weight
/// index that goes with them.
fn banks(context: Context) -> ([usize; EXPERTS], usize) {
    let prev1 = usize::from(context.prev1);
    let prev2 = usize::from(context.prev2);
    let after_copy = usize::from(context.after_copy);
    let rate_context = prev1 | (after_copy * ALPHABET);
    let order2 = ((prev1 << u8::BITS) | prev2) & (ORDER2_BANKS - 1);
    let nibble = prev1 >> NIBBLE_BITS;
    let align = ((context.position % ALIGN_PHASES) * NIBBLES) | nibble;
    let word_hash = context.word_hash as usize & (WORD_BANKS - 1);
    let weight_index = nibble | (after_copy * NIBBLES);
    (
        [
            FAST_BASE + rate_context,
            SLOW_BASE + rate_context,
            ORDER0_BASE,
            ORDER2_BASE + order2,
            ALIGN_BASE + align,
            WORD_BASE + word_hash,
        ],
        weight_index,
    )
}

/// The seventh, column-keyed expert [`Literal::encode_column`]/
/// `decode_column` blend into the mix for a [`Candidate::Transpose`]
/// frame's literal sub-stream
/// (`docs/adr/0046-wire-the-column-expert-into-the-literal-mixer.md`).
/// Not part of [`Literal`]'s own state: `codec.rs` constructs it per trial
/// and threads it through only the column-coding path. A column-keyed
/// frequency bank plus its own single mixing weight per weight-context key
/// (the key [`Literal`]'s six weight vectors are indexed by), adapting
/// alongside, never inside, the six real experts' weights.
///
/// [`Candidate::Transpose`]: crate::filters::select::Candidate::Transpose
#[derive(Debug, Clone)]
pub struct ColumnExpertState {
    /// `max_banks * ALPHABET` frequencies, bank-major (same convention as
    /// [`Literal::freq`]).
    freq: Vec<u32>,
    /// Per-bank frequency totals, same invariant as [`Literal::total`].
    total: Vec<u32>,
    /// This one expert's own mixing weight, one per [`WEIGHT_CONTEXTS`]
    /// key.
    weight: Vec<f64>,
    /// This expert's own SSE calibration trajectory, independent of
    /// [`Literal::sse`]'s. [`Literal::encode_column`]/`decode_column`
    /// calibrate the seven-expert mix through this table instead, so the
    /// column signal's calibration never perturbs the six-expert model's.
    sse: Sse,
}

impl ColumnExpertState {
    /// A fresh column-expert state, started as [`Literal::new`] starts its
    /// six experts: every bank at frequency 1 per symbol (the Laplace floor),
    /// every weight at 1.0, the [`Sse`] table at the identity mapping.
    #[must_use]
    pub fn new(max_banks: NonZeroUsize) -> Self {
        crate::abort_on_oom(Self::try_new(max_banks))
    }

    /// Fallible counterpart to [`Self::new`], as [`Literal::try_new`] is for
    /// the six real experts: `codec::decode` builds this from a fixed,
    /// decoder-chosen `max_banks` (never a value read off untrusted input),
    /// but the allocation can still fail, and hard rule 2 requires
    /// `Error::OutOfMemory` there instead of an abort.
    pub(crate) fn try_new(
        max_banks: NonZeroUsize,
    ) -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            freq: crate::try_filled_vec(max_banks.get() * ALPHABET, 1u32)?,
            total: crate::try_filled_vec(max_banks.get(), ALPHABET_U32)?,
            weight: crate::try_filled_vec(WEIGHT_CONTEXTS, 1.0)?,
            sse: Sse::try_new(bittree::SSE_CONTEXTS)?,
        })
    }
}

/// [`SurpriseLogisticMix`]'s step size at maximum surprise:
/// `research/JOURNAL.md` S2-R25's train-optimal constant rate.
const LOGISTIC_INITIAL_RATE: f64 = 0.006;

/// The step size [`surprise_rate`] decays toward: `research/JOURNAL.md`
/// S2-R25's 0.002 sweep point (S2-A101 names why).
const LOGISTIC_FLOOR_RATE: f64 = 0.002;

/// Every probability [`SurpriseLogisticMix`] stretches or codes is clamped to
/// `[LOGISTIC_PROBABILITY_FLOOR, 1 - LOGISTIC_PROBABILITY_FLOOR]`: an
/// expert's upper-half probability before [`stretch`], bounding each
/// stretch to `ln(4095)` in magnitude, and the squashed mix before the
/// [`Sse`] and the gradient, so a saturated mix still leaves an error to
/// learn from. [`Sse::refine`]'s own floor, [`MIN_PROBABILITY`].
const LOGISTIC_PROBABILITY_FLOOR: f64 = MIN_PROBABILITY;

/// `p` clamped to [`LOGISTIC_PROBABILITY_FLOOR`]'s range.
fn clamp_logistic_probability(p: f64) -> f64 {
    p.clamp(LOGISTIC_PROBABILITY_FLOOR, 1.0 - LOGISTIC_PROBABILITY_FLOOR)
}

/// Starting weight of every expert in every [`SurpriseLogisticMix`] weight
/// vector: the six weights sum to one, so a fresh mixer squashes the
/// experts' mean stretch.
const LOGISTIC_INITIAL_WEIGHT: f64 = 1.0 / 6.0;

/// One gradient step of [`SurpriseLogisticMix`]'s weight vector for a single
/// bit-tree node: the node's error, target bit minus the pre-refine mix
/// `p`, nudges each expert's weight by `rate` scaled by that expert's own
/// stretched input. Split out of [`Literal::code_bit_walk`] so this
/// arithmetic is checkable against the walk's chained state.
fn logistic_gradient_step(
    weights: &mut [f64; EXPERTS],
    stretched: &[f64; EXPERTS],
    rate: f64,
    bit: bool,
    p: f64,
) {
    let error = f64::from(u8::from(bit)) - p;
    for (weight, &s) in weights.iter_mut().zip(stretched) {
        *weight += rate * error * s;
    }
}

/// One bit-tree node's six-expert logit-domain mix: `prefix`'s per-expert
/// prefix sums at `(lo, mid, hi)` give each expert's upper-half
/// probability, [`stretch`]ed and weighted-summed against `weights`.
/// [`Literal::code_bit_walk`]'s first step at each node, before its rate
/// schedule. Split out so it is checkable against directly-chosen prefix
/// sums, independent of the walk's chained state.
fn logistic_mix_node(
    prefix: &[[u64; ALPHABET + 1]; EXPERTS],
    weights: &[f64; EXPERTS],
    lo: usize,
    mid: usize,
    hi: usize,
) -> ([f64; EXPERTS], f64) {
    let mut stretched = [0f64; EXPERTS];
    let mut dot = 0.0f64;
    for (expert, sums) in prefix.iter().enumerate() {
        #[allow(
            clippy::cast_precision_loss,
            reason = "bank totals are bounded by the rescale limit, far below 2^53"
        )]
        let p = (sums[hi] - sums[mid]) as f64 / (sums[hi] - sums[lo]) as f64;
        stretched[expert] = stretch(clamp_logistic_probability(p));
        dot += weights[expert] * stretched[expert];
    }
    (stretched, dot)
}

/// [`SurpriseLogisticMix`]'s per-node error bookkeeping: squares the node's
/// prediction error (target bit minus the pre-refine mix `p`)
/// and advances `recent` at `recent_decay` and `baseline` at
/// [`SURPRISE_BASELINE_DECAY`], both through [`surprise_ema_update`].
/// Split out so it is checkable against directly-chosen `bit`/`p` pairs,
/// independent of the walk's chained state.
fn surprise_error_tracking_step(
    recent: &mut f64,
    baseline: &mut f64,
    recent_decay: f64,
    bit: bool,
    p: f64,
) {
    let error = f64::from(u8::from(bit)) - p;
    let error_sq = error * error;
    *recent = surprise_ema_update(*recent, error_sq, recent_decay);
    *baseline = surprise_ema_update(*baseline, error_sq, SURPRISE_BASELINE_DECAY);
}

/// [`SurpriseLogisticMix`]'s slow timescale: the EMA decay its per-key
/// `baseline_sq_error` uses, fixed rather than swept. This is the
/// "learned floor" `research/JOURNAL.md` S2-R26's own remaining-scope note
/// names, standing for a key's typical residual error once converged; only
/// the fast side ([`SURPRISE_RECENT_DECAY`]) was swept, in S2-A104's own
/// scratch binary.
const SURPRISE_BASELINE_DECAY: f64 = 0.9995;

/// [`SurpriseLogisticMix`]'s per-key step size reads `recent_sq_error`
/// against `baseline_sq_error` at this decay: `research/JOURNAL.md`
/// S2-A104's own accept point, the best-supported point of the
/// nine-value sweep that entry's own scratch binary ran (train mean best at 0.98, both sealed
/// cases improving well inside the qualifying range).
const SURPRISE_RECENT_DECAY: f64 = 0.98;

/// Below this, [`surprise_rate`] treats a key's `baseline_sq_error` as
/// "not yet established" rather than dividing by a near-zero number.
const SURPRISE_BASELINE_EPSILON: f64 = 1e-12;

/// [`SurpriseLogisticMix`]'s step size: reads `recent_sq_error` against
/// `baseline_sq_error`,
/// the same key's own slower EMA of the identical signal, so a converged
/// context's stable residual noise (`recent` tracking `baseline`, ratio
/// near 1) decays toward [`LOGISTIC_FLOOR_RATE`] while a real shift
/// (`recent` pulling away from `baseline`) pushes back toward
/// [`LOGISTIC_INITIAL_RATE`] instead of decaying forever
/// (`research/JOURNAL.md` S2-R26's own remaining-scope note, in contrast
/// to that entry's own rejected mechanism, one shared
/// `ERROR_RATE_MAX_VARIANCE` normalizer). `baseline_sq_error <=
/// SURPRISE_BASELINE_EPSILON` (a fresh key, nothing observed yet) reads as
/// maximum surprise.
fn surprise_rate(recent_sq_error: f64, baseline_sq_error: f64) -> f64 {
    let fraction = if baseline_sq_error <= SURPRISE_BASELINE_EPSILON {
        1.0
    } else {
        (recent_sq_error / baseline_sq_error - 1.0).clamp(0.0, 1.0)
    };
    LOGISTIC_FLOOR_RATE + (LOGISTIC_INITIAL_RATE - LOGISTIC_FLOOR_RATE) * fraction
}

/// One EMA step shared by [`SurpriseLogisticMix`]'s `recent_sq_error` and
/// `baseline_sq_error`, differing only in `decay`: `decay * previous + (1 -
/// decay) * error_sq`.
fn surprise_ema_update(previous: f64, error_sq: f64, decay: f64) -> f64 {
    decay.mul_add(previous, (1.0 - decay) * error_sq)
}

/// Logit-domain mixer over [`Literal`]'s own six expert banks
/// (`research/JOURNAL.md` S1-P8, S2-A101, S2-A104): per bit-tree node,
/// each expert's probability of the upper half is `stretch`ed, the
/// stretches are summed under one weight vector per `WEIGHT_CONTEXTS`
/// key (the same key `banks` selects [`Literal`]'s linear weights by), and
/// the sum is `squash`ed back and refined through `LogitSse`. The per-key
/// step size separates a context's stable residual uncertainty from genuine
/// drift by comparing a fast EMA of squared
/// prediction error (`recent_sq_error`) against a slow EMA of the
/// identical signal (`baseline_sq_error`, that key's own learned floor),
/// through `surprise_rate`, rather than reading magnitude against one
/// shared constant the way S2-R26's own rejected mechanism did. Reads
/// [`Literal`]'s banks, never writes them.
///
/// The real coding path (`research/JOURNAL.md` S2-A106, S2-A108,
/// `docs/adr/0055-wire-logit-domain-sse-bins-into-the-literal-model.md`):
/// [`Literal::encode_logit_sse`]/`decode_logit_sse`.
#[derive(Debug, Clone)]
pub struct SurpriseLogisticMix {
    /// One weight vector per [`WEIGHT_CONTEXTS`] key.
    weights: Vec<[f64; EXPERTS]>,
    /// Each key's fast EMA of squared prediction error.
    recent_sq_error: Vec<f64>,
    /// Each key's slow EMA of the identical signal: its own learned
    /// baseline.
    baseline_sq_error: Vec<f64>,
    /// This mixer's own calibration table, keyed by
    /// [`bittree::sse_context`].
    sse: LogitSse,
}

impl SurpriseLogisticMix {
    /// A fresh mixer: every weight at [`LOGISTIC_INITIAL_WEIGHT`], every
    /// error EMA at zero (read by [`surprise_rate`] as "not yet
    /// established," the maximum-surprise starting point).
    #[must_use]
    pub fn new() -> Self {
        crate::abort_on_oom(Self::try_new())
    }

    /// Fallible counterpart to [`Self::new`], the same shape
    /// [`ColumnExpertState::try_new`] gives the seventh expert:
    /// [`crate::codec::decode`]'s real decode path constructs a
    /// `SurpriseLogisticMix` per frame, and the allocation can still fail,
    /// so hard rule 2 requires `Error::OutOfMemory` there instead of an
    /// abort.
    pub(crate) fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            weights: crate::try_filled_vec(WEIGHT_CONTEXTS, [LOGISTIC_INITIAL_WEIGHT; EXPERTS])?,
            recent_sq_error: crate::try_filled_vec(WEIGHT_CONTEXTS, 0.0)?,
            baseline_sq_error: crate::try_filled_vec(WEIGHT_CONTEXTS, 0.0)?,
            sse: LogitSse::try_new(bittree::SSE_CONTEXTS)?,
        })
    }
}

impl Default for SurpriseLogisticMix {
    fn default() -> Self {
        Self::new()
    }
}

/// Six-expert context-mixing model over literal bytes. See the module
/// docs for the port source and the open `f64` determinism question.
#[derive(Debug, Clone)]
pub struct Literal {
    /// `BANKS * ALPHABET` per-symbol frequencies, bank-major.
    freq: Vec<u32>,
    /// Per-bank frequency totals; always equals the sum of that bank's
    /// 256 `freq` entries, the same invariant [`crate::model::Model`]
    /// leans on for panic-free decoding.
    total: Vec<u32>,
    /// Per-weight-context mixing weights, one `[f64; EXPERTS]` per
    /// [`WEIGHT_CONTEXTS`] key.
    weights: Vec<[f64; EXPERTS]>,
    /// Calibrates [`Self::ideal_cost_bits_sse`]'s per-node binary
    /// decisions, keyed by [`bittree::sse_context`]
    /// (`research/JOURNAL.md` S1-P1's remaining scope).
    sse: Sse,
}

impl Default for Literal {
    fn default() -> Self {
        Self::new()
    }
}

impl Literal {
    /// A fresh model: every bank starts at frequency 1 per symbol (total
    /// 256, nothing ever impossible to code), every mixing weight starts
    /// at 1.0 (experts start equally trusted).
    #[must_use]
    pub fn new() -> Self {
        crate::abort_on_oom(Self::try_new())
    }

    /// The one statement of a fresh model, returning `Err` if the allocator
    /// cannot satisfy any of its tables. [`crate::codec::decode`]'s real
    /// decode path uses this (hard rule 2, `rust-craft` skill's
    /// allocation-discipline, `tests/torture.rs`, #453); [`Self::new`] is
    /// this with an abort, the constructor the encoder and every test use.
    pub(crate) fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            freq: crate::try_filled_vec(BANKS * ALPHABET, 1u32)?,
            total: crate::try_filled_vec(BANKS, ALPHABET_U32)?,
            weights: crate::try_filled_vec(WEIGHT_CONTEXTS, [1.0; EXPERTS])?,
            sse: Sse::try_new(bittree::SSE_CONTEXTS)?,
        })
    }

    /// Blends the six experts' banks under the current mixing weights
    /// into a cumulative-frequency table over the 256 byte values.
    /// Ported unchanged from the archive's `Lit::cum`: every symbol's
    /// mixed count gets `+1` (a Laplace floor, the same "nothing is ever
    /// impossible to code" guarantee [`crate::model::Model`] gives by
    /// starting every frequency at 1), so `cum` is always strictly
    /// increasing and `cum[ALPHABET]` is always the true total passed to
    /// the coder.
    ///
    /// Pass one, the per-expert multiply-add, has no cross-symbol
    /// dependency, so it autovectorizes; pass two, [`cumulate`], is the
    /// prefix sum.
    fn mix(&self, bank_indices: &[usize; EXPERTS], weight_index: usize) -> [u64; ALPHABET + 1] {
        let weights = &self.weights[weight_index];
        let weight_sum: f64 = weights.iter().sum();
        let scale = self.scale6(bank_indices, weights, weight_sum);

        let mut mixed = [0u64; ALPHABET];
        for (&bank, &s) in bank_indices.iter().zip(scale.iter()) {
            for (m, &freq) in mixed.iter_mut().zip(&self.freq[bank_row(bank)]) {
                *m += s * u64::from(freq);
            }
        }

        cumulate(mixed)
    }

    /// [`banks`] plus [`Self::mix`] in one call: the prelude of
    /// [`Self::ideal_cost_bits_sse`], which then calls [`Self::update`]
    /// with the same `bank_indices`/`weight_index`.
    fn banks_and_cum(&self, context: Context) -> ([usize; EXPERTS], usize, [u64; ALPHABET + 1]) {
        let (bank_indices, weight_index) = banks(context);
        let cum = self.mix(&bank_indices, weight_index);
        (bank_indices, weight_index, cum)
    }

    /// Each real expert's own probability estimate for `symbol`: its bank's
    /// frequency count over its bank's total. Shared by [`Self::update`] and
    /// [`Self::update_column_expert`] so both weight adaptations start from
    /// the identical six numbers.
    fn expert_estimates(&self, bank_indices: &[usize; EXPERTS], symbol: usize) -> [f64; EXPERTS] {
        let mut estimate = [0f64; EXPERTS];
        for (expert, &bank) in bank_indices.iter().enumerate() {
            estimate[expert] =
                f64::from(self.freq[bank_slot(bank, symbol)]) / f64::from(self.total[bank]);
        }
        estimate
    }

    /// Adapts mixing weights toward whichever experts predicted `symbol`
    /// best (exponentiated gradient, Mahoney 2005), then updates every
    /// expert's own frequency table the same way
    /// [`crate::model::Model::encode`]/`decode` do. Ported unchanged
    /// from the archive's `Lit::upd`, except the weight update's `exp`
    /// call is parameterized: production callers pass `exp`, and the
    /// test suite's accuracy check (ADR-0024) passes `f64::exp` as an
    /// independent reference to diff against without duplicating the
    /// rest of this method.
    fn update(
        &mut self,
        bank_indices: &[usize; EXPERTS],
        weight_index: usize,
        symbol: usize,
        exp_fn: fn(f64) -> f64,
    ) {
        let estimate = self.expert_estimates(bank_indices, symbol);
        let weights = &mut self.weights[weight_index];
        let weight_sum: f64 = weights.iter().sum();
        let mixed: f64 = (0..EXPERTS)
            .map(|expert| weights[expert] * estimate[expert])
            .sum::<f64>()
            / weight_sum;
        for expert in 0..EXPERTS {
            weights[expert] = adapt_weight(weights[expert], estimate[expert], mixed, exp_fn);
        }
        for (expert, &bank) in bank_indices.iter().enumerate() {
            let rate = if expert == 0 {
                FAST_RATE
            } else {
                Rate::DEFAULT
            };
            crate::rescale_bank(
                &mut self.freq[bank_row(bank)],
                &mut self.total[bank],
                symbol,
                rate,
            );
        }
    }

    /// Bits it would cost to code `byte` under `context`'s blended `cum`
    /// table as 8 binary decisions: sums the ideal cost of `byte`'s 8
    /// `sse`-refined decisions through [`bittree::ideal_cost_bits_sse`],
    /// including this model's own [`Sse`] table adapting call over call,
    /// then updates every expert bank and the mixing weights. No
    /// [`Encoder`] involved. No frame this build decodes codes this way;
    /// the benchmark harness prices it as a baseline.
    #[must_use]
    pub fn ideal_cost_bits_sse(&mut self, context: Context, byte: u8) -> f64 {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        let bits = bittree::ideal_cost_bits_sse(&cum, byte, &mut self.sse);
        self.update(&bank_indices, weight_index, usize::from(byte), exp);
        bits
    }

    /// `weights6`, the extra expert's own weight, and their sum: the
    /// three-number prelude [`Self::mix7`] and [`Self::update_column_expert`]
    /// each start from, keyed by the same `weight_index` [`banks`] selected
    /// for the six real experts.
    fn weights6_and_sum(&self, weight_index: usize, w7: f64) -> ([f64; EXPERTS], f64) {
        let weights6 = self.weights[weight_index];
        let weight_sum = weights6.iter().sum::<f64>() + w7;
        (weights6, weight_sum)
    }

    /// The six real experts' fixed-point scale factors under `weights6`
    /// and `weight_sum`: the shared middle step [`Self::mix`] and
    /// [`Self::mix7`] compute identically, the latter before folding in its
    /// seventh term.
    fn scale6(
        &self,
        bank_indices: &[usize; EXPERTS],
        weights6: &[f64; EXPERTS],
        weight_sum: f64,
    ) -> [u64; EXPERTS] {
        let mut scale6 = [0u64; EXPERTS];
        for expert in 0..EXPERTS {
            let bank_total = f64::from(self.total[bank_indices[expert]]);
            scale6[expert] = fixed_point_scale(weights6[expert], weight_sum, bank_total);
        }
        scale6
    }

    /// Seven-wide counterpart of [`Self::mix`]: the same fixed-point blend
    /// with `column_state`'s bank folded in as a seventh expert, keyed by
    /// `column_bank`. Shared by [`Self::encode_column`] and
    /// [`Self::decode_column`] so both code the identical seven-expert
    /// distribution (`research/JOURNAL.md` S1-P5).
    fn mix7(
        &self,
        bank_indices: &[usize; EXPERTS],
        weight_index: usize,
        column_bank: usize,
        column_state: &ColumnExpertState,
    ) -> [u64; ALPHABET + 1] {
        let w7 = column_state.weight[weight_index];
        let (weights6, weight_sum) = self.weights6_and_sum(weight_index, w7);

        let scale6 = self.scale6(bank_indices, &weights6, weight_sum);
        let column_total = f64::from(column_state.total[column_bank]);
        let scale7 = fixed_point_scale(w7, weight_sum, column_total);

        cumulate((0..ALPHABET).map(|symbol| {
            let mut mixed = scale7 * u64::from(column_state.freq[bank_slot(column_bank, symbol)]);
            for expert in 0..EXPERTS {
                let freq = u64::from(self.freq[bank_slot(bank_indices[expert], symbol)]);
                mixed += scale6[expert] * freq;
            }
            mixed
        }))
    }

    /// Adapts `column_state`'s own weight and bank toward `symbol`, the
    /// seventh-expert counterpart of [`Self::update`]'s six real experts,
    /// shared by [`Self::encode_column`] and [`Self::decode_column`] so
    /// both adapt `column_state` through the identical rule
    /// (`research/JOURNAL.md` S1-P5). The column expert's own weight adapts
    /// on the same continuous-probability-space rule [`Self::update`] uses
    /// for the six real weights, restricted to this one component: how well
    /// its own local estimate did against the seven-expert blend
    /// ([`Self::mix7`]'s), never written back into `self.weights`.
    fn update_column_expert(
        &self,
        bank_indices: &[usize; EXPERTS],
        weight_index: usize,
        symbol: usize,
        column_bank: usize,
        column_state: &mut ColumnExpertState,
    ) {
        let w7 = column_state.weight[weight_index];
        let (weights6, weight_sum) = self.weights6_and_sum(weight_index, w7);
        let column_total = f64::from(column_state.total[column_bank]);
        let column_estimate =
            f64::from(column_state.freq[bank_slot(column_bank, symbol)]) / column_total;

        let estimate6 = self.expert_estimates(bank_indices, symbol);
        let mixed_estimate = (weights6
            .iter()
            .zip(estimate6.iter())
            .map(|(&w, &e)| w * e)
            .sum::<f64>()
            + w7 * column_estimate)
            / weight_sum;
        column_state.weight[weight_index] = adapt_weight(w7, column_estimate, mixed_estimate, exp);

        crate::rescale_bank(
            &mut column_state.freq[bank_row(column_bank)],
            &mut column_state.total[column_bank],
            symbol,
            Rate::DEFAULT,
        );
    }

    /// One byte's coding under `context`, blending `column_state`'s bank in
    /// as a seventh expert via [`Self::mix7`] and coding the result through
    /// the SSE-calibrated bittree decomposition against `column_state`'s own
    /// [`Sse`] table (`research/JOURNAL.md` S1-P5's real-wiring slice,
    /// `FORMAT_VERSION` 4). `code` takes the seven-expert `cum` table and
    /// that [`Sse`] and returns the byte it coded or decoded, which is
    /// returned. Shared by [`Self::encode_column`] and [`Self::decode_column`]
    /// so the two cannot drift apart.
    ///
    /// The six real experts adapt exactly as [`Self::ideal_cost_bits_sse`]
    /// leaves them, [`Self::update`] on the same six-way `mixed` estimate it
    /// always has, unperturbed by the column expert: the same layering
    /// [`bittree`]'s own SSE stage already uses on top of the six-expert mix,
    /// not a new coupling. `column_state`'s own weight and bank adapt
    /// separately via [`Self::update_column_expert`], against the seven-way
    /// mixed estimate that was actually coded. Reproduces exactly the update
    /// order `research/JOURNAL.md` S2-A76's before-wiring measurement found
    /// correct: `update_column_expert` first (reading the six real experts'
    /// pre-update state), then the six-expert `update`.
    fn code_column(
        &mut self,
        context: Context,
        column_bank: usize,
        column_state: &mut ColumnExpertState,
        code: impl FnOnce(&[u64; ALPHABET + 1], &mut Sse) -> u8,
    ) -> u8 {
        let (bank_indices, weight_index) = banks(context);
        let cum7 = self.mix7(&bank_indices, weight_index, column_bank, column_state);
        let byte = code(&cum7, &mut column_state.sse);
        let symbol = usize::from(byte);
        self.update_column_expert(
            &bank_indices,
            weight_index,
            symbol,
            column_bank,
            column_state,
        );
        self.update(&bank_indices, weight_index, symbol, exp);
        byte
    }

    /// Codes `byte` through `encoder` under `context` with `column_state`'s
    /// bank as a seventh expert; see [`Self::code_column`] for the coding and
    /// update shape.
    pub fn encode_column(
        &mut self,
        encoder: &mut Encoder,
        context: Context,
        byte: u8,
        column_bank: usize,
        column_state: &mut ColumnExpertState,
    ) {
        self.code_column(context, column_bank, column_state, |cum7, sse| {
            bittree::encode_symbol_sse(encoder, cum7, byte, sse);
            byte
        });
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_column`].
    ///
    /// Never panics on adversarial `decoder` state:
    /// [`bittree::decode_symbol_sse`] is total over any coded bit pattern
    /// (its own `Decoder::decode_bit` calls are), and [`Self::mix7`]'s `cum7` is
    /// this model's own invariant (both banks' Laplace floors), never
    /// derived from `decoder`'s bytes.
    #[must_use]
    pub fn decode_column(
        &mut self,
        decoder: &mut Decoder,
        context: Context,
        column_bank: usize,
        column_state: &mut ColumnExpertState,
    ) -> u8 {
        self.code_column(context, column_bank, column_state, |cum7, sse| {
            bittree::decode_symbol_sse(decoder, cum7, sse)
        })
    }

    /// Each expert's cumulative frequency table over its selected bank:
    /// `prefix[expert][s]` is the bank's mass below symbol `s`, so any
    /// `[lo, hi)` range's mass is one subtraction.
    fn expert_prefix_sums(
        &self,
        bank_indices: &[usize; EXPERTS],
    ) -> [[u64; ALPHABET + 1]; EXPERTS] {
        let mut prefix = [[0u64; ALPHABET + 1]; EXPERTS];
        for (sums, &bank) in prefix.iter_mut().zip(bank_indices) {
            let mut acc = 0u64;
            for (symbol, &freq) in self.freq[bank_row(bank)].iter().enumerate() {
                acc += u64::from(freq);
                sums[symbol + 1] = acc;
            }
        }
        prefix
    }

    /// The [`SurpriseLogisticMix`] counterpart of [`bittree::walk_sse`],
    /// shared by encode, decode and pricing so they cannot drift apart:
    /// blends the six expert banks in the logit domain under `mixer`'s
    /// `weight_index` vector, refines through its calibration table, and
    /// takes one gradient step per node at [`surprise_rate`]. `code_bit`
    /// receives each node's midpoint and refined probability and returns the
    /// bit the node resolved to.
    ///
    /// Every expert's upper-half probability lies strictly inside `(0, 1)`
    /// before [`clamp_logistic_probability`] even runs: every bank
    /// frequency is at least 1 ([`crate::rescale_bank`] rounds up), and
    /// every node's range holds at least one symbol on each side.
    fn code_bit_walk(
        &self,
        bank_indices: &[usize; EXPERTS],
        weight_index: usize,
        mixer: &mut SurpriseLogisticMix,
        mut code_bit: impl FnMut(usize, f64) -> bool,
    ) -> u8 {
        let prefix = self.expert_prefix_sums(bank_indices);
        bittree::walk_nodes(|depth, node_prefix, lo, mid, hi| {
            let (stretched, dot) =
                logistic_mix_node(&prefix, &mixer.weights[weight_index], lo, mid, hi);
            let p = clamp_logistic_probability(squash(dot));
            let node_context = bittree::sse_context(depth, node_prefix);
            let refined = mixer.sse.refine(node_context, p);
            let bit = code_bit(mid, refined);
            mixer.sse.update(node_context, p, bit);
            let rate = surprise_rate(
                mixer.recent_sq_error[weight_index],
                mixer.baseline_sq_error[weight_index],
            );
            logistic_gradient_step(&mut mixer.weights[weight_index], &stretched, rate, bit, p);
            surprise_error_tracking_step(
                &mut mixer.recent_sq_error[weight_index],
                &mut mixer.baseline_sq_error[weight_index],
                SURPRISE_RECENT_DECAY,
                bit,
                p,
            );
            bit
        })
    }

    /// Codes `byte` through `encoder` under `context`, mixed by `mixer`'s
    /// logit-domain walk and refined through [`LogitSse`]
    /// (`research/JOURNAL.md` S2-A106, S2-A108). The six expert banks
    /// update by `update` whichever mixer refined the probabilities.
    pub fn encode_logit_sse(
        &mut self,
        encoder: &mut Encoder,
        context: Context,
        byte: u8,
        mixer: &mut SurpriseLogisticMix,
    ) {
        let symbol = usize::from(byte);
        let landed = self.walk_logit_sse(context, mixer, |mid, p| {
            let bit = symbol >= mid;
            encoder.encode_bit(bit, p);
            bit
        });
        debug_assert_eq!(landed, byte, "the walk must land on the coded byte");
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_logit_sse`].
    ///
    /// Never panics on adversarial `decoder` state: [`Decoder::decode_bit`]
    /// is total over any coded bit pattern, and every probability is this
    /// model's and `mixer`'s own invariant, never derived from `decoder`'s
    /// bytes.
    #[must_use]
    pub fn decode_logit_sse(
        &mut self,
        decoder: &mut Decoder,
        context: Context,
        mixer: &mut SurpriseLogisticMix,
    ) -> u8 {
        self.walk_logit_sse(context, mixer, |_mid, p| decoder.decode_bit(p))
    }

    /// One byte's walk under `context` and `mixer`, the step
    /// [`Self::encode_logit_sse`], [`Self::decode_logit_sse`] and the
    /// benchmark harness's whole-stream pricing share: `code_bit` receives
    /// each node's midpoint and refined probability that the node's bit is
    /// `1` (the byte is at or above the midpoint) and returns the bit the
    /// node resolved to; the six expert banks then update on the landed
    /// byte, which is returned.
    pub fn walk_logit_sse(
        &mut self,
        context: Context,
        mixer: &mut SurpriseLogisticMix,
        code_bit: impl FnMut(usize, f64) -> bool,
    ) -> u8 {
        let (bank_indices, weight_index) = banks(context);
        let byte = self.code_bit_walk(&bank_indices, weight_index, mixer, code_bit);
        self.update(&bank_indices, weight_index, usize::from(byte), exp);
        byte
    }
}

#[cfg(test)]
mod tests;
