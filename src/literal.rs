//! Six-expert context-mixing literal model: [`Literal`], the S2-D2
//! entropy stage for the bytes an LZ parse ([`crate::lz`]) leaves as
//! literals, once the length/offset/flag streams (each a plain
//! [`crate::model::Model`]) have claimed everything else.
//!
//! Ported from the archive's `Lit`
//! (`research/imports/session-1/mothergod.rs`), not the code, per
//! ADR-0006: six order-N context predictors ("experts") each keep their
//! own order-0 frequency table over the 256 byte values in whatever
//! context they're keyed on (a rolling two-rate fast/slow order-1 hash,
//! an order-0 catch-all, a 12-bit order-2 hash, a position/nibble
//! "alignment" hash, and a 12-bit alnum-only rolling word hash), and
//! their per-symbol counts blend under weights the coder adapts after
//! every symbol via an exponentiated-gradient rule (Mahoney 2005),
//! context-sensitive: the weight vector itself is selected by a small
//! `(prev-byte nibble, after-copy)` key, so the mixer favors different
//! experts after a match than after a run of literals (`JOURNAL` S1-A4).
//!
//! [`Literal::ideal_cost_bits`] is [`crate::model::Model::ideal_cost_bits`]'s
//! counterpart for this six-expert mixer, ROADMAP M2's ideal-cost
//! accounting mode (`JOURNAL` S2-A31, completing S2-A30's remaining
//! scope): sums `-log2(p)` against the mixed distribution instead of
//! driving [`crate::coder::Encoder`].
//!
//! **SSE-calibrated coding (`JOURNAL` S1-P1, S2-A58, S2-A59, `FORMAT_VERSION`
//! 3).** [`Literal::encode_sse`]/[`Literal::decode_sse`] code the six-expert
//! mixer's blended `cum` table through
//! [`crate::bittree::encode_symbol_sse`]/[`crate::bittree::decode_symbol_sse`]:
//! 8 chained binary decisions, each refined by [`Literal`]'s own [`Sse`]
//! table (`crate::bittree::SSE_CONTEXTS` contexts, keyed by tree position
//! only, `crate::bittree::sse_context`) before it reaches the coder. This
//! calibrates the six-expert mixer's own blended probability at each
//! binary-tree node, a compound estimate — unlike `JOURNAL` S2-R1's
//! rejected attempt, which SSE-calibrated an already order-0-adaptive lone
//! frequency counter (the flag model's `is_copy` bit) and found nothing to
//! correct. Every `FORMAT_VERSION` this build decodes (`codec::LZ_MIN_VERSION`
//! (3) and up) codes through this path; the older direct 256-way range
//! division that coded `FORMAT_VERSION` 2's literal sub-stream was deleted
//! with that version
//! (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`, no release
//! having ever written it).
//!
//! **Decode-path determinism (`JOURNAL` S2-D3, resolved by ADR-0024).**
//! The exponentiated-gradient weight update runs on both the encode and
//! decode path, so anything it calls must produce a bit-identical result
//! on every platform, or an encoder and a decoder desync mid-frame (hard
//! rule 1). `f64::exp()` is libm's and not guaranteed bit-identical
//! across implementations; `exp` replaces it with an `e^x` built from
//! IEEE-754 basic operations only (range reduction plus a polynomial,
//! `2^k` by exact repeated doubling), enforced crate-wide by
//! `clippy.toml`'s `disallowed-methods`. `JOURNAL` S1-A5's full
//! integer-only mixer is no longer a prerequisite here; ADR-0024
//! demotes it to an M5 speed lead, since its speed claim is unmeasured
//! in this codebase.
//!
//! **Logit-domain mixing (`JOURNAL` S1-P8, S2-A101, `FORMAT_VERSION`
//! `codec::LOGISTIC_MIN_VERSION` (5), ADR-0052).**
//! [`Literal::encode_logistic`]/[`Literal::decode_logistic`] replace the
//! SSE-calibrated path above with a second mixer, [`LogisticMix`]: the six
//! real experts' own probability estimates are [`crate::logistic::stretch`]ed
//! into the logit domain, blended under a weight vector `logistic_rate`'s
//! annealed schedule adapts, [`crate::logistic::squash`]ed back, then
//! calibrated through `LogisticMix`'s own [`Sse`] table, independent of
//! `Literal`'s own — the same separation [`Literal::encode_column`] already
//! keeps for its seventh expert. The six real experts still adapt exactly as
//! [`Literal::encode_sse`] leaves them (`Literal::update` still runs,
//! unperturbed): only the literal path's own coding and calibration change.
//! `LogisticMix` reuses `stretch`/`squash` rather than `exp`/`ln` directly;
//! both are built from IEEE-754 basic operations only, the same
//! ADR-0024 determinism this module's `exp` already provides
//! (`crate::logistic`'s own module docs).
//!
//! **Learned-baseline rate schedule (`JOURNAL` S2-A104/S2-A105,
//! `FORMAT_VERSION` `codec::SURPRISE_MIN_VERSION` (6), ADR-0054).**
//! [`Literal::encode_logistic_surprise`]/[`Literal::decode_logistic_surprise`]
//! replace [`Literal::encode_logistic`]/`decode_logistic` for every
//! candidate except `Candidate::Transpose`: the same six-expert
//! logit-domain walk, but [`SurpriseLogisticMix`]'s per-key step size reads
//! a fast EMA of that key's own squared prediction error against a slower
//! EMA of the identical signal (its own learned baseline) instead of
//! [`LogisticMix`]'s step-count-derived schedule, so a converged-but-noisy
//! context's stable residual reads as "no surprise" while genuine drift
//! still raises the rate.
//!
//! **Logit-domain SSE bins (`JOURNAL` S2-A106/S2-A108, `FORMAT_VERSION`
//! `codec::LOGIT_SSE_MIN_VERSION` (7), ADR-0055).**
//! [`Literal::encode_logit_sse`]/[`Literal::decode_logit_sse`] replace
//! [`Literal::encode_logistic_surprise`]/`decode_logistic_surprise` for
//! every candidate except `Candidate::Transpose`: the identical walk,
//! rate schedule and gradient step, but [`SurpriseLogisticMixLogitSse`]'s
//! calibration table spaces its bins evenly in
//! [`crate::logistic::stretch`]-space instead of [`Sse`]'s own linear
//! probability space, concentrating resolution near 0/1 where a
//! calibration error costs most.

use std::num::NonZeroUsize;

use crate::bittree;
use crate::coder::{Decoder, Encoder};
use crate::logistic::{squash, stretch};
use crate::ppm::Ppm;
use crate::sse::{Calibrate, Sse};

/// Number of context predictors blended for every literal byte.
const EXPERTS: usize = 6;

/// Number of distinct mixing-weight vectors: one per `(prev-byte nibble,
/// after-copy)` key (`JOURNAL` S1-A4's "context-sensitive MIX weights").
const WEIGHT_CONTEXTS: usize = 32;

/// Byte alphabet every context bank models.
const ALPHABET: usize = 256;
/// [`ALPHABET`] as a `u32`, spelled as its own literal instead of a cast
/// so no truncation lint applies to a compile-time-obvious value.
const ALPHABET_U32: u32 = 256;

// Bank layout, ported from the archive's `O_CF`/`O_CS`/`O_O0`/`O_O2`/
// `O_AL`/`O_WD`/`NB` (`research/imports/session-1/mothergod.rs`): one
// contiguous bank space, sliced per expert so [`banks`] can address any
// of them with a single base offset.
const FAST_BASE: usize = 0;
const FAST_BANKS: usize = 512;
const SLOW_BASE: usize = FAST_BASE + FAST_BANKS;
const SLOW_BANKS: usize = 512;
const ORDER0_BASE: usize = SLOW_BASE + SLOW_BANKS;
const ORDER0_BANKS: usize = 1;
const ORDER2_BASE: usize = ORDER0_BASE + ORDER0_BANKS;
const ORDER2_BANKS: usize = 4096;
const ALIGN_BASE: usize = ORDER2_BASE + ORDER2_BANKS;
const ALIGN_BANKS: usize = 64;
const WORD_BASE: usize = ALIGN_BASE + ALIGN_BANKS;
const WORD_BANKS: usize = 4096;
/// Total context banks across all six experts.
const BANKS: usize = WORD_BASE + WORD_BANKS;

/// Frequency increment for the fast-rate context expert (bank 0):
/// higher increment and lower rescale ceiling than the other five, so
/// it tracks recent bytes hardest and forgets fastest. Ported unchanged
/// from the archive's inline `(32u32, 6144u32)` special case for `e==0`.
const FAST_INCREMENT: u32 = 32;
const FAST_LIMIT: u32 = 6144;

/// Exponentiated-gradient learning rate and weight clamp, ported
/// unchanged from the archive's inline `0.05`/`1e-4`/`1e4`.
const LEARNING_RATE: f64 = 0.05;
const MIN_WEIGHT: f64 = 1e-4;
const MAX_WEIGHT: f64 = 1e4;
/// Floor under the mixed-probability denominator so a weight update
/// never divides by (near) zero. Ported unchanged from the archive's
/// inline `1e-9`.
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
/// platform whether it runs on the encode or the decode path
/// (ADR-0024, `JOURNAL` S2-D3).
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
/// summing. Shared by [`Literal::mix`] and [`Literal::mix7`]'s own
/// seven-wide blend (`research/JOURNAL.md` S1-P5): both need the
/// identical scale-factor formula, only the `weight_sum` they normalize
/// against differs (six experts' worth vs. seven).
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

/// Exponentiated-gradient weight update (Mahoney 2005): how far `weight`
/// moves given `estimate` (this expert's own prediction) versus `mixed`
/// (the blend's prediction), clamped to `[MIN_WEIGHT, MAX_WEIGHT]`.
/// Shared by [`Literal::update`]'s six real experts and
/// [`Literal::update_column_expert`]'s seventh: that method's own docs
/// already claim its column weight adapts "the same ... rule
/// `Self::update` uses" — this makes that claim true by construction
/// instead of by two independently written copies.
fn adapt_weight(weight: f64, estimate: f64, mixed: f64, exp_fn: fn(f64) -> f64) -> f64 {
    let denominator = mixed.max(MIN_DENOMINATOR);
    let gradient = LEARNING_RATE * (estimate - mixed) / denominator;
    (weight * exp_fn(gradient)).clamp(MIN_WEIGHT, MAX_WEIGHT)
}

/// Per-byte modeling context [`Literal::encode_sse`]/[`Literal::decode_sse`]
/// read to select which banks blend at this position: the previous two
/// bytes (`0` before the start of output, matching the archive's
/// `fd[pos-1]`/`fd[pos-2]` boundary convention), the output position,
/// whether the previous token was a copy (LZ match or rep) rather than
/// a literal, and the rolling alnum-only word hash
/// ([`advance_word_hash`]).
///
/// [`Self::after_literal`]/[`Self::after_copy`] compute the next context
/// the same way the archive's `encode_body`/`decode` update
/// `(b1, b2, pos, am, wh)` after every token, so a caller driving an
/// encode pass and a decode pass from the same token stream reuses one
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

/// Advances a rolling word hash by one byte. Ported unchanged from the
/// archive's `whup`: only alphanumeric bytes extend the hash, anything
/// else resets it to `0`, so the hash tracks how far into the current
/// alnum run a position is, never text spanning punctuation.
#[must_use]
pub fn advance_word_hash(word_hash: u32, byte: u8) -> u32 {
    if byte.is_ascii_alphanumeric() {
        word_hash.wrapping_mul(61).wrapping_add(u32::from(byte))
    } else {
        0
    }
}

/// Bank indices for [`Context`]'s six experts, and the mixing-weight
/// index that goes with them. Ported unchanged from the archive's
/// `Lit::banks`.
fn banks(context: Context) -> ([usize; EXPERTS], usize) {
    let prev1 = usize::from(context.prev1);
    let prev2 = usize::from(context.prev2);
    let after_copy = usize::from(context.after_copy);
    let rate_context = prev1 | (after_copy * ALPHABET);
    let order2 = ((prev1 << 8) | prev2) & (ORDER2_BANKS - 1);
    let align = ((context.position & 3) << 4) | (prev1 >> 4);
    let word_hash = usize::try_from(context.word_hash)
        .expect("word_hash is a u32, always fits usize")
        & (WORD_BANKS - 1);
    let weight_index = (prev1 >> 4) | (after_copy * 16);
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
/// frame's literal sub-stream (`research/JOURNAL.md` S1-P5,
/// `docs/adr/0046-wire-the-column-expert-into-the-literal-mixer.md`,
/// `FORMAT_VERSION` 4). Not part of [`Literal`]'s own persisted state and
/// never touched by the plain `encode`/`decode` (or `encode_sse`/
/// `decode_sse`) pair: `codec.rs` constructs it separately, per trial, and
/// threads it through only the column-coding path. A column-keyed
/// frequency bank plus its own single mixing weight per weight-context key
/// (the same key `Literal`'s own six weight vectors are indexed by),
/// adapting on its own trajectory alongside, never inside, the six real
/// experts' weights.
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
    /// [`Literal::sse`]'s real one (`research/JOURNAL.md` S1-P5's
    /// SSE-interaction question). [`Literal::encode_column`]/
    /// `decode_column` calibrate the seven-expert mix through this table
    /// instead, so the column signal's own calibration never perturbs the
    /// six-expert model's shipped calibration trajectory.
    sse: Sse,
}

impl ColumnExpertState {
    /// A fresh column-expert state: every bank starts at frequency 1 per
    /// symbol (the same Laplace floor [`Literal::new`] starts its six
    /// experts at), every weight starts at 1.0 (equally trusted), and its
    /// own [`Sse`] table starts at the identity mapping, same as
    /// [`Literal::new`]'s.
    #[must_use]
    pub fn new(max_banks: NonZeroUsize) -> Self {
        Self {
            freq: vec![1u32; max_banks.get() * ALPHABET],
            total: vec![ALPHABET_U32; max_banks.get()],
            weight: vec![1.0; WEIGHT_CONTEXTS],
            sse: Sse::new(bittree::SSE_CONTEXTS),
        }
    }

    /// Fallible counterpart to [`Self::new`], the same shape
    /// [`Literal::try_new`] gives the six real experts: `codec::decode`'s
    /// real decode path constructs a `ColumnExpertState` from a fixed,
    /// decoder-chosen `max_banks` (never a value read off untrusted input),
    /// but the allocation itself can still fail, and hard rule 2 requires
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

/// Bank count for [`PpmExpertState`]: the previous byte's high nibble,
/// the same coarse key `research/JOURNAL.md` S2-R16/S2-R17's
/// `NibbleFallback` used for its own (rejected, substitutive) fallback
/// table.
const PPM_EXPERT_BANKS: usize = 16;

/// `research/JOURNAL.md` S1-P3's own remaining scope after S2-R17: a
/// genuinely additive, [`Ppm`]-backed expert, never substituted into any
/// of [`Literal`]'s six real experts' own banks the way every prior S1-P3
/// slice (S2-R6, S2-R16, S2-R17) tried and had rejected. Same
/// "own bank space, own adaptive weight, own additive contribution" shape
/// as [`ColumnExpertState`] (S1-P5) — the difference is what each bank is:
/// a [`Ppm`] table, not a plain Laplace-smoothed frequency bank, so a
/// symbol this bank has never observed contributes exactly zero mass
/// (Method C's own "genuinely unseen" floor is 0, not 1), never a false
/// floor competing with the six real experts' own Laplace-smoothed ones.
/// Keyed the same coarse way `research/JOURNAL.md` S2-R16/S2-R17's
/// `NibbleFallback` was (the previous byte's high nibble, `PPM_EXPERT_BANKS`
/// contexts), deliberately reusing that context signal so this slice
/// isolates the mechanism question (additive vs. substitutive) from the
/// keying question those two entries already answered.
#[derive(Debug, Clone)]
pub struct PpmExpertState {
    /// One [`Ppm`] table per bank ([`PPM_EXPERT_BANKS`] of them).
    tables: Vec<Ppm>,
    /// This expert's own mixing weight, one per [`WEIGHT_CONTEXTS`] key.
    weight: Vec<f64>,
}

impl PpmExpertState {
    /// A fresh PPM-expert state: every bank starts as a fresh [`Ppm`]
    /// table (every symbol unseen), every weight starts at 1.0 (equally
    /// trusted), the same convention [`ColumnExpertState::new`] uses for
    /// its own fresh state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tables: (0..PPM_EXPERT_BANKS).map(|_| Ppm::new(ALPHABET)).collect(),
            weight: vec![1.0; WEIGHT_CONTEXTS],
        }
    }

    /// Which of this state's [`PPM_EXPERT_BANKS`] tables `context` keys:
    /// the previous byte's high nibble alone.
    #[must_use]
    fn bank_of(context: Context) -> usize {
        usize::from(context.prev1) >> 4
    }
}

impl Default for PpmExpertState {
    fn default() -> Self {
        Self::new()
    }
}

/// `table`'s own linear-space probability estimate for `symbol`
/// ([`Ppm::probability`]). Shared by [`Literal::mix_ppm`] and
/// [`Literal::update_ppm_expert`] so both price the identical number.
fn ppm_probability(table: &Ppm, symbol: usize) -> f64 {
    table.probability(symbol)
}

/// [`fixed_point_scale`]'s own shape, but for an expert like
/// [`PpmExpertState`] whose own per-symbol estimate is already a
/// probability in `[0, 1]` ([`ppm_probability`]), not a raw frequency over
/// a bank total: an escaped symbol's probability is `0.0` by construction,
/// so there is no bank total this could sensibly divide by.
fn fixed_point_contribution_from_probability(
    weight: f64,
    weight_sum: f64,
    probability: f64,
) -> u64 {
    let normalized = weight / weight_sum;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "fixed-point contribution: normalized weight and probability are both in [0,1], the product is always non-negative and truncation is the intended floor"
    )]
    {
        (normalized * probability * FIXED_POINT_SCALE) as u64
    }
}

/// [`LogisticMix`]'s step size at the start of a weight vector's life:
/// `research/JOURNAL.md` S2-R25's train-optimal constant rate.
const LOGISTIC_INITIAL_RATE: f64 = 0.006;

/// The step size [`logistic_rate`] decays toward: `research/JOURNAL.md`
/// S2-R25's 0.002 sweep point (S2-A101 names why).
const LOGISTIC_FLOOR_RATE: f64 = 0.002;

/// [`LogisticMix`]'s rate schedule's `decay`, fixed at the train-only sweep
/// optimum [`Literal::encode_logistic`]/`decode_logistic` code every byte
/// under (`research/JOURNAL.md` S2-A101).
const LOGISTIC_RATE_DECAY: f64 = 4e-4;

/// Every probability [`LogisticMix`] stretches or codes is clamped to
/// `[LOGISTIC_PROBABILITY_FLOOR, 1 - LOGISTIC_PROBABILITY_FLOOR]`: an
/// expert's upper-half probability before [`stretch`], bounding each
/// stretch to `ln(4095)` in magnitude, and the squashed mix before the
/// [`Sse`] and the gradient, so a saturated mix still leaves an error to
/// learn from. The same bound [`Sse::refine`] clamps its own output to.
const LOGISTIC_PROBABILITY_FLOOR: f64 = 1.0 / 4096.0;

/// `p` clamped to [`LOGISTIC_PROBABILITY_FLOOR`]'s range.
fn clamp_logistic_probability(p: f64) -> f64 {
    p.clamp(LOGISTIC_PROBABILITY_FLOOR, 1.0 - LOGISTIC_PROBABILITY_FLOOR)
}

/// Starting weight of every expert in every [`LogisticMix`] weight vector:
/// the six weights sum to one, so a fresh mixer squashes the experts'
/// mean stretch.
const LOGISTIC_INITIAL_WEIGHT: f64 = 1.0 / 6.0;

/// [`LogisticMix`]'s step size for a weight vector that has already taken
/// `steps` gradient steps: `FLOOR + (INITIAL - FLOOR) / (1 + steps *
/// decay)`, [`LOGISTIC_INITIAL_RATE`] at `steps = 0`, falling
/// monotonically toward [`LOGISTIC_FLOOR_RATE`] for any `decay > 0`, and
/// constant at the initial rate for `decay = 0` (`research/JOURNAL.md`
/// S2-A101).
fn logistic_rate(steps: u64, decay: f64) -> f64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "a step count far below 2^53 per measured stream; precision past that changes the rate by under one ulp"
    )]
    let steps = steps as f64;
    LOGISTIC_FLOOR_RATE + (LOGISTIC_INITIAL_RATE - LOGISTIC_FLOOR_RATE) / steps.mul_add(decay, 1.0)
}

/// One gradient step of [`LogisticMix`]'s weight vector for a single
/// bit-tree node: the node's error, target bit minus the pre-refine mix
/// `p`, nudges each expert's weight by `rate` scaled by that expert's own
/// stretched input. Split out of [`Literal::logistic_cost_bits`] so this
/// arithmetic is checkable against the walk's chained state (#783's
/// mutants outlived the walk-level tests; issue's shape is
/// `test-craft`'s survivor-triage).
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
/// Shared by every [`Literal::code_bit_walk`] instantiation, the one step
/// they all take before diverging into their own rate schedules. Split out
/// on [`logistic_gradient_step`]'s own grounds (#783): checkable against
/// directly-chosen prefix sums, independent of the walk's chained state
/// (`test-craft`'s survivor-triage, #810).
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

/// [`SurpriseLogisticMix`]'s [`LogisticMixer::advance`]: squares the node's
/// prediction error (target bit minus the pre-refine mix `p`)
/// and advances `recent` at `recent_decay` and `baseline` at
/// [`SURPRISE_BASELINE_DECAY`], both through [`surprise_ema_update`].
/// Split out on [`logistic_gradient_step`]'s own grounds (#783): checkable
/// against directly-chosen `bit`/`p` pairs, independent of the walk's
/// chained state (`test-craft`'s survivor-triage, #810).
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

/// A fresh weight vector, one per [`WEIGHT_CONTEXTS`] key, every expert at
/// [`LOGISTIC_INITIAL_WEIGHT`]: the shape [`LogisticMix::new`] and
/// [`SurpriseLogisticMix::new`] both start from, named once so a future
/// change to either constant cannot update one copy and miss the other.
fn fresh_logistic_weights() -> Vec<[f64; EXPERTS]> {
    vec![[LOGISTIC_INITIAL_WEIGHT; EXPERTS]; WEIGHT_CONTEXTS]
}

/// Fallible counterpart to [`fresh_logistic_weights`], the same relationship
/// [`crate::try_filled_vec`] bears to a plain `vec![]` everywhere else in
/// this crate: [`LogisticMix::try_new`] and [`SurpriseLogisticMix::try_new`]
/// both start from this.
fn try_fresh_logistic_weights() -> Result<Vec<[f64; EXPERTS]>, std::collections::TryReserveError> {
    crate::try_filled_vec(WEIGHT_CONTEXTS, [LOGISTIC_INITIAL_WEIGHT; EXPERTS])
}

/// Logit-domain mixer over [`Literal`]'s own six expert banks
/// (`research/JOURNAL.md` S1-P8, S2-A101): per bit-tree node, each expert's
/// probability of the upper half is [`stretch`]ed, the stretches are
/// summed under one weight vector per `WEIGHT_CONTEXTS` key (the same
/// key `banks` selects [`Literal`]'s linear weights by), and the sum is
/// [`squash`]ed back, then refined through this mixer's own [`Sse`] over
/// the same [`bittree::SSE_CONTEXTS`] contexts the shipped coder uses.
/// Reads [`Literal`]'s banks, never writes them: [`Literal::encode_logistic`]/
/// `decode_logistic` are the real coding path (`codec::LOGISTIC_MIN_VERSION`).
#[derive(Debug, Clone)]
pub struct LogisticMix {
    /// One weight vector per [`WEIGHT_CONTEXTS`] key.
    weights: Vec<[f64; EXPERTS]>,
    /// Gradient steps each key's weight vector has taken, one per
    /// bit-tree node coded under that key: [`logistic_rate`]'s `steps`.
    update_count: Vec<u64>,
    /// This mixer's own calibration table, keyed by
    /// [`bittree::sse_context`].
    sse: Sse,
}

impl LogisticMix {
    /// A fresh mixer: every weight at [`LOGISTIC_INITIAL_WEIGHT`], every
    /// step count at zero, its [`Sse`] at the identity mapping.
    #[must_use]
    pub fn new() -> Self {
        Self {
            weights: fresh_logistic_weights(),
            update_count: vec![0; WEIGHT_CONTEXTS],
            sse: Sse::new(bittree::SSE_CONTEXTS),
        }
    }

    /// Fallible counterpart to [`Self::new`], the same shape
    /// [`ColumnExpertState::try_new`] gives the seventh expert:
    /// [`crate::codec::decode`]'s real decode path constructs a
    /// `LogisticMix` per frame now that [`Literal::decode_logistic`] reaches
    /// it, and the allocation can still fail, so hard rule 2 requires
    /// `Error::OutOfMemory` there instead of an abort.
    pub(crate) fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            weights: try_fresh_logistic_weights()?,
            update_count: crate::try_filled_vec(WEIGHT_CONTEXTS, 0)?,
            sse: Sse::try_new(bittree::SSE_CONTEXTS)?,
        })
    }
}

impl Default for LogisticMix {
    fn default() -> Self {
        Self::new()
    }
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
/// S2-A104's own accept point (mirrors [`LOGISTIC_RATE_DECAY`]'s role for
/// the champion), the best-supported point of the nine-value sweep that
/// entry's own scratch binary ran (train mean best at 0.98, both sealed
/// cases improving well inside the qualifying range).
const SURPRISE_RECENT_DECAY: f64 = 0.98;

/// Below this, [`surprise_rate`] treats a key's `baseline_sq_error` as
/// "not yet established" rather than dividing by a near-zero number.
const SURPRISE_BASELINE_EPSILON: f64 = 1e-12;

/// [`SurpriseLogisticMix`]'s step size: unlike [`logistic_rate`]'s step
/// count, this schedule reads `recent_sq_error` against `baseline_sq_error`,
/// the same key's own slower EMA of the identical signal, so a converged
/// context's stable residual noise (`recent` tracking `baseline`, ratio
/// near 1) decays toward [`LOGISTIC_FLOOR_RATE`] while a real shift
/// (`recent` pulling away from `baseline`) pushes back toward
/// [`LOGISTIC_INITIAL_RATE`] instead of decaying forever
/// (`research/JOURNAL.md` S2-R26's own remaining-scope note, in contrast
/// to that entry's own rejected mechanism, one shared
/// `ERROR_RATE_MAX_VARIANCE` normalizer). `baseline_sq_error <=
/// SURPRISE_BASELINE_EPSILON` (a fresh key, nothing observed yet) reads as
/// maximum surprise, matching `logistic_rate`'s own `steps = 0` behavior.
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

/// A second rate schedule for a logit-domain literal mixer's per-key step
/// size, replacing [`LogisticMix`]'s step-count-derived
/// `logistic_rate` (`research/JOURNAL.md` S2-A104, S2-R26's own
/// remaining-scope note fixed): separates a context's stable residual
/// uncertainty from genuine drift by comparing a fast EMA of squared
/// prediction error (`recent_sq_error`) against a slow EMA of the
/// identical signal (`baseline_sq_error`, that key's own learned floor),
/// through `surprise_rate`, rather than reading magnitude against one
/// shared constant the way S2-R26's own rejected mechanism did. The real
/// coding path (`codec::SURPRISE_MIN_VERSION`):
/// [`Literal::encode_logistic_surprise`]/`decode_logistic_surprise`.
///
/// Generic over its own calibration table `C` ([`Calibrate`]): the mixing
/// weights, error EMAs and rate schedule never depend on which table
/// calibrates them, only the walk's `refine`/`update` calls do.
/// [`SurpriseLogisticMixLogitSse`] is this same type with
/// [`crate::sse::LogitSse`] in `C`'s place (`research/JOURNAL.md` S2-A106):
/// isolates the bin-spacing change that type measures without a second
/// struct and a second walk that would differ from this one in exactly one
/// line. The real coding path for `C = Sse`
/// (`codec::SURPRISE_MIN_VERSION`): [`Literal::encode_logistic_surprise`]/
/// `decode_logistic_surprise`; for `C = `[`crate::sse::LogitSse`]
/// (`codec::LOGIT_SSE_MIN_VERSION`, `research/JOURNAL.md` S2-A108,
/// `docs/adr/0055-wire-logit-domain-sse-bins-into-the-literal-model.md`):
/// [`Literal::encode_logit_sse`]/`decode_logit_sse`.
#[derive(Debug, Clone)]
pub struct SurpriseLogisticMix<C = Sse> {
    /// One weight vector per [`WEIGHT_CONTEXTS`] key, same shape and
    /// starting point as [`LogisticMix::weights`].
    weights: Vec<[f64; EXPERTS]>,
    /// Each key's fast EMA of squared prediction error.
    recent_sq_error: Vec<f64>,
    /// Each key's slow EMA of the identical signal: its own learned
    /// baseline.
    baseline_sq_error: Vec<f64>,
    /// This mixer's own calibration table, same shape as
    /// [`LogisticMix::sse`].
    sse: C,
}

/// [`SurpriseLogisticMix`] over [`crate::sse::LogitSse`] instead of
/// [`Sse`]'s default: see [`SurpriseLogisticMix`]'s own docs.
pub type SurpriseLogisticMixLogitSse = SurpriseLogisticMix<crate::sse::LogitSse>;

impl<C: Calibrate> SurpriseLogisticMix<C> {
    /// A fresh mixer: every weight at [`LOGISTIC_INITIAL_WEIGHT`], every
    /// error EMA at zero (read by [`surprise_rate`] as "not yet
    /// established," the same maximum-surprise starting point
    /// [`logistic_rate`]'s `steps = 0` gives the champion).
    #[must_use]
    pub fn new() -> Self {
        Self {
            weights: fresh_logistic_weights(),
            recent_sq_error: vec![0.0; WEIGHT_CONTEXTS],
            baseline_sq_error: vec![0.0; WEIGHT_CONTEXTS],
            sse: C::new(bittree::SSE_CONTEXTS),
        }
    }

    /// Fallible counterpart to [`Self::new`], the same shape
    /// [`LogisticMix::try_new`] gives its own mixer:
    /// [`crate::codec::decode`]'s real decode path constructs a
    /// `SurpriseLogisticMix` per frame now that
    /// [`Literal::decode_logistic_surprise`]/[`Literal::decode_logit_sse`]
    /// reach it, and the allocation can still fail, so hard rule 2
    /// requires `Error::OutOfMemory` there instead of an abort.
    pub(crate) fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            weights: try_fresh_logistic_weights()?,
            recent_sq_error: crate::try_filled_vec(WEIGHT_CONTEXTS, 0.0)?,
            baseline_sq_error: crate::try_filled_vec(WEIGHT_CONTEXTS, 0.0)?,
            sse: C::try_new(bittree::SSE_CONTEXTS)?,
        })
    }
}

impl<C: Calibrate> Default for SurpriseLogisticMix<C> {
    fn default() -> Self {
        Self::new()
    }
}

/// [`Literal::code_bit_walk`]'s per-mixer interface: the weight vector and
/// calibration table both mixers keep per [`WEIGHT_CONTEXTS`] key, plus the
/// one place [`LogisticMix`]'s step-count rate and [`SurpriseLogisticMix`]'s
/// error-EMA rate diverge ([`logistic_mix_node`]'s own docs), named once so
/// the walk between them is not duplicated.
trait LogisticMixer {
    /// This mixer's own calibration table type.
    type Calibrator: Calibrate;

    /// `weight_index`'s weight vector.
    fn weights_mut(&mut self, weight_index: usize) -> &mut [f64; EXPERTS];

    /// This mixer's calibration table.
    fn sse_mut(&mut self) -> &mut Self::Calibrator;

    /// `weight_index`'s step size for the node about to be coded, before
    /// that node's bit is known.
    fn rate(&self, weight_index: usize) -> f64;

    /// Bookkeeping once the node's bit and pre-refine probability are
    /// known.
    fn advance(&mut self, weight_index: usize, bit: bool, p: f64);
}

impl LogisticMixer for LogisticMix {
    type Calibrator = Sse;

    fn weights_mut(&mut self, weight_index: usize) -> &mut [f64; EXPERTS] {
        &mut self.weights[weight_index]
    }

    fn sse_mut(&mut self) -> &mut Sse {
        &mut self.sse
    }

    fn rate(&self, weight_index: usize) -> f64 {
        logistic_rate(self.update_count[weight_index], LOGISTIC_RATE_DECAY)
    }

    fn advance(&mut self, weight_index: usize, _bit: bool, _p: f64) {
        self.update_count[weight_index] += 1;
    }
}

impl<C: Calibrate> LogisticMixer for SurpriseLogisticMix<C> {
    type Calibrator = C;

    fn weights_mut(&mut self, weight_index: usize) -> &mut [f64; EXPERTS] {
        &mut self.weights[weight_index]
    }

    fn sse_mut(&mut self) -> &mut C {
        &mut self.sse
    }

    fn rate(&self, weight_index: usize) -> f64 {
        surprise_rate(
            self.recent_sq_error[weight_index],
            self.baseline_sq_error[weight_index],
        )
    }

    fn advance(&mut self, weight_index: usize, bit: bool, p: f64) {
        surprise_error_tracking_step(
            &mut self.recent_sq_error[weight_index],
            &mut self.baseline_sq_error[weight_index],
            SURPRISE_RECENT_DECAY,
            bit,
            p,
        );
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
    /// Calibrates [`Self::encode_sse`]/[`Self::decode_sse`]'s per-node
    /// binary decisions, keyed by [`bittree::sse_context`]
    /// (`research/JOURNAL.md` S1-P1's remaining scope, `FORMAT_VERSION` 3).
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
        Self {
            freq: vec![1u32; BANKS * ALPHABET],
            total: vec![ALPHABET_U32; BANKS],
            weights: vec![[1.0; EXPERTS]; WEIGHT_CONTEXTS],
            sse: Sse::new(bittree::SSE_CONTEXTS),
        }
    }

    /// Fallible counterpart to [`Self::new`]: the same fresh model, but
    /// returns `Err` instead of aborting if the allocator cannot satisfy
    /// any of its tables. [`crate::codec::decode`]'s real decode path
    /// uses this (hard rule 2, `rust-craft` skill's allocation-discipline,
    /// `tests/torture.rs`, #453); [`Self::new`] stays the panicking
    /// constructor the encoder and every test use.
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
    /// Two passes, not one fused loop over `symbol` (`research/JOURNAL.md`
    /// S1-P6, issue #447): the per-symbol multiply-add below is
    /// independent across symbols, but the original single loop also
    /// carried `acc`'s running total through the same iterations, a
    /// loop-carried dependency that keeps the optimizer from
    /// autovectorizing the multiply-add at all. Splitting the prefix sum
    /// into its own pass over already-computed values leaves the
    /// axpy-shaped accumulation (pass one) free of any cross-symbol
    /// dependency. This changes only *when* each addition happens, never
    /// its value: pass one still sums the same six per-expert terms for a
    /// given symbol in the same expert order (0..EXPERTS) the old fused
    /// loop did, and pass two performs the identical `(mixed >> 16) + 1`
    /// running sum. `mix`'s output is therefore bit-for-bit identical to
    /// the pre-split version; this is a speed change, not a format change
    /// (hard rule 5), so no golden fixture or `FORMAT_VERSION` bump is
    /// needed.
    fn mix(&self, bank_indices: &[usize; EXPERTS], weight_index: usize) -> [u64; ALPHABET + 1] {
        let weights = &self.weights[weight_index];
        let weight_sum: f64 = weights.iter().sum();
        let mut scale = [0u64; EXPERTS];
        for (expert, &bank) in bank_indices.iter().enumerate() {
            let bank_total = f64::from(self.total[bank]);
            scale[expert] = fixed_point_scale(weights[expert], weight_sum, bank_total);
        }

        let mut mixed = [0u64; ALPHABET];
        for (&bank, &s) in bank_indices.iter().zip(scale.iter()) {
            let base = bank * ALPHABET;
            for (m, &freq) in mixed.iter_mut().zip(&self.freq[base..base + ALPHABET]) {
                *m += s * u64::from(freq);
            }
        }

        let mut cum = [0u64; ALPHABET + 1];
        let mut acc = 0u64;
        for (symbol, &m) in mixed.iter().enumerate() {
            acc += (m >> 16) + 1;
            cum[symbol + 1] = acc;
        }
        cum
    }

    /// [`banks`] plus [`Self::mix`] in one call: every coding/pricing method
    /// below starts by selecting `context`'s bank indices and weight
    /// context, then blending them into a mixed cumulative-frequency table,
    /// before doing whatever is specific to that method and, eventually,
    /// calling [`Self::update`] with the same `bank_indices`/`weight_index`.
    /// One place for that shared prelude keeps the six call sites from
    /// drifting if `banks` or `mix` ever change.
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
                f64::from(self.freq[bank * ALPHABET + symbol]) / f64::from(self.total[bank]);
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
            let (increment, limit) = if expert == 0 {
                (FAST_INCREMENT, FAST_LIMIT)
            } else {
                (
                    crate::DEFAULT_RESCALE_INCREMENT,
                    crate::DEFAULT_RESCALE_LIMIT,
                )
            };
            crate::rescale_bank(
                &mut self.freq[bank * ALPHABET..bank * ALPHABET + ALPHABET],
                &mut self.total[bank],
                symbol,
                increment,
                limit,
            );
        }
    }

    /// Codes `byte` through `encoder` under `context`, the mixed `cum`
    /// table coded as 8 chained binary decisions through
    /// [`bittree::encode_symbol_sse`], each calibrated by this model's own
    /// [`Sse`] table (`research/JOURNAL.md` S1-P1, `FORMAT_VERSION` 3),
    /// then updates every expert bank and the mixing weights.
    pub fn encode_sse(&mut self, encoder: &mut Encoder, context: Context, byte: u8) {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        bittree::encode_symbol_sse(encoder, &cum, byte, &mut self.sse);
        self.update(&bank_indices, weight_index, usize::from(byte), exp);
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_sse`], then updates every expert bank and the
    /// mixing weights.
    ///
    /// Never panics on adversarial `decoder` state: [`bittree::decode_symbol_sse`]
    /// is total over any coded bit pattern (its own `Decoder::decode_bit`
    /// calls are), and `cum`'s shape is this model's own invariant
    /// (`mix`'s Laplace floor), never derived from `decoder`'s bytes.
    #[must_use]
    pub fn decode_sse(&mut self, decoder: &mut Decoder, context: Context) -> u8 {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        let byte = bittree::decode_symbol_sse(decoder, &cum, &mut self.sse);
        self.update(&bank_indices, weight_index, usize::from(byte), exp);
        byte
    }

    /// Bits it would cost to code `byte` under `context`'s current mixed
    /// distribution — `-log2((cum[symbol+1] - cum[symbol]) /
    /// cum[ALPHABET])` — then updates every expert bank and the mixing
    /// weights, exactly as [`Self::encode_sse`] does. No [`Encoder`]
    /// involved: this is
    /// [`crate::model::Model::ideal_cost_bits`]'s counterpart for the
    /// six-expert mixer, the remaining scope `JOURNAL` S2-A30 flagged for
    /// ROADMAP M2's ideal-cost accounting mode.
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't apply off the coding path)"
    )]
    pub fn ideal_cost_bits(&mut self, context: Context, byte: u8) -> f64 {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        let symbol = usize::from(byte);
        #[allow(
            clippy::cast_precision_loss,
            reason = "cum entries are fixed-point sums bounded well under 2^53 (FIXED_POINT_SCALE is 2^32, ALPHABET is 256), so this loses no precision that matters"
        )]
        let probability = (cum[symbol + 1] - cum[symbol]) as f64 / cum[ALPHABET] as f64;
        self.update(&bank_indices, weight_index, symbol, exp);
        -probability.log2()
    }

    /// [`Self::ideal_cost_bits`]'s counterpart for [`Self::encode_sse`]:
    /// sums the ideal cost of `byte`'s 8 `sse`-refined binary decisions
    /// through [`bittree::ideal_cost_bits_sse`] instead of pricing the
    /// direct 256-way division, so a caller pricing a whole stream this way
    /// reflects what `Self::encode_sse` actually pays, including this
    /// model's own [`Sse`] table adapting call over call
    /// (`crate::codec`'s `CostSink`/`EncodeSink` must price and code the
    /// same thing, per that module's docs). Updates the mixer state the
    /// same way [`Self::ideal_cost_bits`] does.
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't apply off the coding path)"
    )]
    pub fn ideal_cost_bits_sse(&mut self, context: Context, byte: u8) -> f64 {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        let bits = bittree::ideal_cost_bits_sse(&cum, byte, &mut self.sse);
        self.update(&bank_indices, weight_index, usize::from(byte), exp);
        bits
    }

    /// [`Self::ideal_cost_bits_sse`]'s counterpart for
    /// [`Self::encode_logistic_surprise`]: sums the ideal cost of `byte`'s
    /// `LEVELS` `mixer`-refined binary decisions through
    /// [`Self::code_bit_walk`] at the registered [`SURPRISE_RECENT_DECAY`],
    /// so a caller pricing a whole stream this way reflects what
    /// `Self::encode_logistic_surprise` actually pays (`crate::codec`'s
    /// `CostSink`/`EncodeSink` invariant, [`Self::ideal_cost_bits_sse`]'s
    /// own docs). Updates the six real experts' banks and takes the same
    /// gradient step `mixer` would, same as [`Self::ideal_cost_bits_sse`].
    #[must_use]
    pub fn ideal_cost_bits_logistic_surprise(
        &mut self,
        context: Context,
        byte: u8,
        mixer: &mut SurpriseLogisticMix,
    ) -> f64 {
        let (bank_indices, weight_index) = banks(context);
        let symbol = usize::from(byte);
        let mut bits = 0.0f64;
        let landed = self.code_bit_walk(&bank_indices, weight_index, mixer, |mid, p| {
            let bit = symbol >= mid;
            bits += bittree::ideal_cost_bit(bit, p);
            bit
        });
        debug_assert_eq!(landed, byte, "the walk must land on the priced byte");
        self.update(&bank_indices, weight_index, symbol, exp);
        bits
    }

    /// [`Self::ideal_cost_bits_logistic_surprise`]'s counterpart for the
    /// [`SurpriseLogisticMixLogitSse`] research candidate
    /// (`research/JOURNAL.md` S2-A106): identical pricing, over a
    /// [`SurpriseLogisticMix`] keyed to [`crate::sse::LogitSse`] instead of
    /// [`Sse`].
    #[must_use]
    pub fn ideal_cost_bits_logistic_surprise_logit_sse(
        &mut self,
        context: Context,
        byte: u8,
        mixer: &mut SurpriseLogisticMixLogitSse,
    ) -> f64 {
        let (bank_indices, weight_index) = banks(context);
        let symbol = usize::from(byte);
        let mut bits = 0.0f64;
        let landed = self.code_bit_walk(&bank_indices, weight_index, mixer, |mid, p| {
            let bit = symbol >= mid;
            bits += bittree::ideal_cost_bit(bit, p);
            bit
        });
        debug_assert_eq!(landed, byte, "the walk must land on the priced byte");
        self.update(&bank_indices, weight_index, symbol, exp);
        bits
    }

    /// `weights6`, the extra expert's own weight, and their sum: the
    /// shared three-number prelude [`Self::mix7`]/[`Self::mix_ppm`] and
    /// their [`Self::update_column_expert`]/[`Self::update_ppm_expert`]
    /// counterparts each start from, keyed by the same `weight_index`
    /// [`banks`] selected for the six real experts.
    fn weights6_and_sum(&self, weight_index: usize, w7: f64) -> ([f64; EXPERTS], f64) {
        let weights6 = self.weights[weight_index];
        let weight_sum = weights6.iter().sum::<f64>() + w7;
        (weights6, weight_sum)
    }

    /// The six real experts' fixed-point scale factors under `weights6`
    /// and `weight_sum`: the shared middle step [`Self::mix7`] and
    /// [`Self::mix_ppm`] both compute identically before folding in
    /// their own seventh/eighth term.
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

    /// The six real experts' combined fixed-point contribution to
    /// `symbol` under `scale6`: the shared inner-loop term
    /// [`Self::mix7`] and [`Self::mix_ppm`] each add their own
    /// seventh/eighth term to.
    fn six_expert_mixed(
        &self,
        bank_indices: &[usize; EXPERTS],
        scale6: &[u64; EXPERTS],
        symbol: usize,
    ) -> u64 {
        let mut mixed = 0u64;
        for expert in 0..EXPERTS {
            let freq = u64::from(self.freq[bank_indices[expert] * ALPHABET + symbol]);
            mixed += scale6[expert] * freq;
        }
        mixed
    }

    /// The extra expert's own weight, adapted toward how well
    /// `estimate7` did against the six-real-experts-plus-extra mixed
    /// estimate: the shared update rule
    /// [`Self::update_column_expert`] and [`Self::update_ppm_expert`]
    /// each apply to their own bank.
    fn adapt_seventh_weight(
        &self,
        bank_indices: &[usize; EXPERTS],
        weights6: &[f64; EXPERTS],
        weight_sum: f64,
        symbol: usize,
        w7: f64,
        estimate7: f64,
    ) -> f64 {
        let estimate6 = self.expert_estimates(bank_indices, symbol);
        let mixed_estimate = (weights6
            .iter()
            .zip(estimate6.iter())
            .map(|(&w, &e)| w * e)
            .sum::<f64>()
            + w7 * estimate7)
            / weight_sum;
        adapt_weight(w7, estimate7, mixed_estimate, exp)
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

        let mut cum = [0u64; ALPHABET + 1];
        let mut acc = 0u64;
        for s in 0..ALPHABET {
            let mut mixed = self.six_expert_mixed(bank_indices, &scale6, s);
            let freq7 = u64::from(column_state.freq[column_bank * ALPHABET + s]);
            mixed += scale7 * freq7;
            acc += (mixed >> 16) + 1;
            cum[s + 1] = acc;
        }
        cum
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
            f64::from(column_state.freq[column_bank * ALPHABET + symbol]) / column_total;

        column_state.weight[weight_index] = self.adapt_seventh_weight(
            bank_indices,
            &weights6,
            weight_sum,
            symbol,
            w7,
            column_estimate,
        );

        crate::rescale_bank(
            &mut column_state.freq[column_bank * ALPHABET..column_bank * ALPHABET + ALPHABET],
            &mut column_state.total[column_bank],
            symbol,
            crate::DEFAULT_RESCALE_INCREMENT,
            crate::DEFAULT_RESCALE_LIMIT,
        );
    }

    /// Codes `byte` through `encoder` under `context`, blending
    /// `column_state`'s bank in as a seventh expert via [`Self::mix7`] and
    /// coding the result through the SSE-calibrated bittree decomposition
    /// against `column_state`'s own [`Sse`] table (`research/JOURNAL.md`
    /// S1-P5's real-wiring slice, `FORMAT_VERSION` 4). The six real
    /// experts adapt exactly as [`Self::encode_sse`] leaves them —
    /// [`Self::update`] still runs, on the same six-way `mixed` estimate
    /// it always has, unperturbed by the column expert — the same
    /// layering [`bittree`]'s own SSE stage already uses on top of the
    /// six-expert mix, not a new coupling. `column_state`'s own weight and
    /// bank adapt separately via [`Self::update_column_expert`], against
    /// the seven-way mixed estimate that was actually coded. Reproduces
    /// exactly the update order `research/JOURNAL.md` S2-A76's
    /// before-wiring measurement found correct: `update_column_expert`
    /// first (reading the six real experts' pre-update state), then the
    /// six-expert `update`.
    pub fn encode_column(
        &mut self,
        encoder: &mut Encoder,
        context: Context,
        byte: u8,
        column_bank: usize,
        column_state: &mut ColumnExpertState,
    ) {
        let (bank_indices, weight_index) = banks(context);
        let cum7 = self.mix7(&bank_indices, weight_index, column_bank, column_state);
        bittree::encode_symbol_sse(encoder, &cum7, byte, &mut column_state.sse);
        let symbol = usize::from(byte);
        self.update_column_expert(
            &bank_indices,
            weight_index,
            symbol,
            column_bank,
            column_state,
        );
        self.update(&bank_indices, weight_index, symbol, exp);
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_column`]; see that method's docs for the coding
    /// and update shape.
    ///
    /// Never panics on adversarial `decoder` state, the same argument
    /// [`Self::decode_sse`]'s docs give: [`bittree::decode_symbol_sse`] is
    /// total over any coded bit pattern, and [`Self::mix7`]'s `cum7` is
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
        let (bank_indices, weight_index) = banks(context);
        let cum7 = self.mix7(&bank_indices, weight_index, column_bank, column_state);
        let byte = bittree::decode_symbol_sse(decoder, &cum7, &mut column_state.sse);
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

    /// Eighth-expert-style counterpart of [`Self::mix`]/[`Self::mix7`]:
    /// the same fixed-point blend with `ppm_state`'s bank folded in as one
    /// more, genuinely additive term (`research/JOURNAL.md` S1-P3's own
    /// remaining scope after S2-R17: "an escape signal that never enters
    /// a real expert's own floor at all"). Unlike [`Self::mix7`]'s
    /// `column_state` bank (a Laplace-smoothed frequency table, same
    /// shape as the six real experts), `ppm_state`'s own [`Ppm`] table
    /// starts every symbol at frequency 0: [`ppm_probability`] reports
    /// `0.0` for a symbol this bank has never observed, so this expert's
    /// own contribution is silent exactly where it has nothing to say,
    /// never a false floor competing with the six real experts' own
    /// Laplace-smoothed ones.
    fn mix_ppm(
        &self,
        bank_indices: &[usize; EXPERTS],
        weight_index: usize,
        ppm_bank: usize,
        ppm_state: &PpmExpertState,
    ) -> [u64; ALPHABET + 1] {
        let w7 = ppm_state.weight[weight_index];
        let (weights6, weight_sum) = self.weights6_and_sum(weight_index, w7);
        let scale6 = self.scale6(bank_indices, &weights6, weight_sum);

        let table = &ppm_state.tables[ppm_bank];

        let mut cum = [0u64; ALPHABET + 1];
        let mut acc = 0u64;
        for s in 0..ALPHABET {
            let mut mixed = self.six_expert_mixed(bank_indices, &scale6, s);
            mixed += fixed_point_contribution_from_probability(
                w7,
                weight_sum,
                ppm_probability(table, s),
            );
            acc += (mixed >> 16) + 1;
            cum[s + 1] = acc;
        }
        cum
    }

    /// Adapts `ppm_state`'s own weight and bank toward `symbol`, the
    /// PPM-expert counterpart of [`Self::update_column_expert`]: its
    /// weight adapts on the same continuous-probability-space rule
    /// [`Self::update`] uses for the six real weights, restricted to this
    /// one component, and its bank observes `symbol` through [`Ppm::observe`]
    /// (Method C's own bookkeeping, not [`crate::rescale_bank`] — this
    /// bank is a [`Ppm`] table, never a plain frequency array).
    fn update_ppm_expert(
        &self,
        bank_indices: &[usize; EXPERTS],
        weight_index: usize,
        symbol: usize,
        ppm_bank: usize,
        ppm_state: &mut PpmExpertState,
    ) {
        let w7 = ppm_state.weight[weight_index];
        let (weights6, weight_sum) = self.weights6_and_sum(weight_index, w7);
        let ppm_estimate = ppm_probability(&ppm_state.tables[ppm_bank], symbol);

        ppm_state.weight[weight_index] = self.adapt_seventh_weight(
            bank_indices,
            &weights6,
            weight_sum,
            symbol,
            w7,
            ppm_estimate,
        );

        ppm_state.tables[ppm_bank].observe(symbol);
    }

    /// `research/JOURNAL.md` S1-P3's before-wiring measurement, the same
    /// paired methodology S1-P5's column expert (`JOURNAL` S2-A69) and
    /// S1-P2's fieldtype expert (`JOURNAL` S2-R15) both used: prices
    /// `byte` twice from the same pre-update six-expert state — once
    /// under the shipped mix ([`Self::ideal_cost_bits`] exactly,
    /// including its own `update` call, so the six real experts adapt on
    /// their one real trajectory regardless of this method ever running),
    /// once with `ppm_state`'s own [`Ppm`] table blended in as a
    /// genuinely additive expert via [`Self::mix_ppm`], never substituted
    /// into any of the six real experts' own banks (`JOURNAL`
    /// S2-R6/S2-R16/S2-R17's shared failure shape). `ppm_state` adapts on
    /// its own trajectory via [`Self::update_ppm_expert`], independent of
    /// the six real weights.
    ///
    /// Returns `(baseline_bits, with_ppm_bits)`.
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't apply off the coding path)"
    )]
    pub fn ideal_cost_bits_ppm_expert_pair(
        &mut self,
        context: Context,
        byte: u8,
        ppm_state: &mut PpmExpertState,
    ) -> (f64, f64) {
        let (bank_indices, weight_index) = banks(context);
        let symbol = usize::from(byte);
        let ppm_bank = PpmExpertState::bank_of(context);

        let cum7 = self.mix_ppm(&bank_indices, weight_index, ppm_bank, ppm_state);
        #[allow(
            clippy::cast_precision_loss,
            reason = "same bound as ideal_cost_bits: fixed-point sums stay well under 2^53"
        )]
        let with_ppm_probability = (cum7[symbol + 1] - cum7[symbol]) as f64 / cum7[ALPHABET] as f64;
        let with_ppm_bits = -with_ppm_probability.log2();

        self.update_ppm_expert(&bank_indices, weight_index, symbol, ppm_bank, ppm_state);

        let baseline_bits = self.ideal_cost_bits(context, byte);

        (baseline_bits, with_ppm_bits)
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
            for (symbol, &freq) in self.freq[bank * ALPHABET..(bank + 1) * ALPHABET]
                .iter()
                .enumerate()
            {
                acc += u64::from(freq);
                sums[symbol + 1] = acc;
            }
        }
        prefix
    }

    /// Shared skeleton behind [`Self::encode_mix`]/[`Self::decode_mix`],
    /// the [`LogisticMixer`] counterpart of
    /// [`bittree::walk_sse`]: walks [`bittree::walk_nodes`], the shipped
    /// coder's own tree traversal, blending this model's six expert banks
    /// in the logit domain under `mixer`'s `weight_index` vector, refining
    /// through `mixer`'s own calibration table, and taking one gradient
    /// step per node at `mixer`'s own rate ([`LogisticMixer::rate`]) —
    /// [`LogisticMix`]'s step-count decay or [`SurpriseLogisticMix`]'s
    /// error-EMA one, the one place the two mixers differ
    /// ([`logistic_mix_node`]'s own docs). `code_bit` receives each node's
    /// midpoint and refined probability and returns the bit that node
    /// resolved to — already known from a caller's own `symbol` for an
    /// encode, decoded from [`Decoder::decode_bit`] for a decode — so
    /// callers differ only in what they do with that bit and probability,
    /// never in the walk itself. Generic over `mixer`'s own calibration
    /// table ([`LogisticMixer::Calibrator`]) so [`SurpriseLogisticMix<Sse>`]
    /// and [`SurpriseLogisticMixLogitSse`] share this one walk rather than
    /// two that would differ only in which table's `refine`/`update` it
    /// calls.
    ///
    /// Every expert's upper-half probability lies strictly inside `(0, 1)`
    /// before [`clamp_logistic_probability`] even runs: every bank
    /// frequency is at least 1 ([`crate::rescale_bank`] rounds up), and
    /// every node's range holds at least one symbol on each side.
    fn code_bit_walk<M: LogisticMixer>(
        &self,
        bank_indices: &[usize; EXPERTS],
        weight_index: usize,
        mixer: &mut M,
        mut code_bit: impl FnMut(usize, f64) -> bool,
    ) -> u8 {
        let prefix = self.expert_prefix_sums(bank_indices);
        bittree::walk_nodes(|depth, node_prefix, lo, mid, hi| {
            let (stretched, dot) =
                logistic_mix_node(&prefix, mixer.weights_mut(weight_index), lo, mid, hi);
            let p = clamp_logistic_probability(squash(dot));
            let node_context = bittree::sse_context(depth, node_prefix);
            let refined = mixer.sse_mut().refine(node_context, p);
            let bit = code_bit(mid, refined);
            mixer.sse_mut().update(node_context, p, bit);
            let rate = mixer.rate(weight_index);
            logistic_gradient_step(mixer.weights_mut(weight_index), &stretched, rate, bit, p);
            mixer.advance(weight_index, bit, p);
            bit
        })
    }

    /// [`Self::encode_logistic`]/[`Self::encode_logistic_surprise`]/
    /// [`Self::encode_logit_sse`]'s shared body: generic over `M`
    /// ([`LogisticMixer`]) the same way [`Self::code_bit_walk`] is, since
    /// coding `byte` and updating the six expert banks afterward never
    /// depends on which mixer or calibration table refines each node's
    /// probability.
    fn encode_mix<M: LogisticMixer>(
        &mut self,
        encoder: &mut Encoder,
        context: Context,
        byte: u8,
        mixer: &mut M,
    ) {
        let (bank_indices, weight_index) = banks(context);
        let symbol = usize::from(byte);
        let landed = self.code_bit_walk(&bank_indices, weight_index, mixer, |mid, p| {
            let bit = symbol >= mid;
            encoder.encode_bit(bit, p);
            bit
        });
        debug_assert_eq!(landed, byte, "the walk must land on the coded byte");
        self.update(&bank_indices, weight_index, symbol, exp);
    }

    /// [`Self::decode_logistic`]/[`Self::decode_logistic_surprise`]/
    /// [`Self::decode_logit_sse`]'s shared body; see [`Self::encode_mix`]
    /// for why this is generic over the mixer.
    fn decode_mix<M: LogisticMixer>(
        &mut self,
        decoder: &mut Decoder,
        context: Context,
        mixer: &mut M,
    ) -> u8 {
        let (bank_indices, weight_index) = banks(context);
        let byte = self.code_bit_walk(&bank_indices, weight_index, mixer, |_mid, p| {
            decoder.decode_bit(p)
        });
        self.update(&bank_indices, weight_index, usize::from(byte), exp);
        byte
    }

    /// Codes `byte` through `encoder` under `context`, blending this
    /// model's six expert banks through `mixer`'s logit-domain mix under
    /// [`SurpriseLogisticMix`]'s learned-baseline rate schedule instead of
    /// [`Self::encode_logistic`]'s step-count-derived one
    /// (`research/JOURNAL.md` S2-A104, `codec::SURPRISE_MIN_VERSION`), then
    /// updates every expert bank exactly as [`Self::encode_sse`] does. The
    /// six real experts' own linear weights adapt unperturbed
    /// ([`Self::update`] still runs on the same six-way `mixed` estimate it
    /// always has), the same layering [`Self::encode_logistic`] already
    /// uses.
    pub fn encode_logistic_surprise(
        &mut self,
        encoder: &mut Encoder,
        context: Context,
        byte: u8,
        mixer: &mut SurpriseLogisticMix,
    ) {
        self.encode_mix(encoder, context, byte, mixer);
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_logistic_surprise`]; see that method's docs for the
    /// coding and update shape.
    ///
    /// Never panics on adversarial `decoder` state, the same argument
    /// [`Self::decode_sse`]'s docs give: [`Decoder::decode_bit`] is total
    /// over any coded bit pattern, and every probability
    /// [`Self::code_bit_walk`] derives is this model's and `mixer`'s
    /// own invariant, never derived from `decoder`'s bytes.
    #[must_use]
    pub fn decode_logistic_surprise(
        &mut self,
        decoder: &mut Decoder,
        context: Context,
        mixer: &mut SurpriseLogisticMix,
    ) -> u8 {
        self.decode_mix(decoder, context, mixer)
    }

    /// Codes `byte` through `encoder` under `context`, blending this
    /// model's six expert banks through `mixer`'s logit-domain mix under
    /// [`SurpriseLogisticMix`]'s own rate schedule, refined through
    /// [`crate::sse::LogitSse`] instead of [`Sse`]
    /// (`research/JOURNAL.md` S2-A106/S2-A108, `codec::LOGIT_SSE_MIN_VERSION`),
    /// then updates every expert bank exactly as
    /// [`Self::encode_logistic_surprise`] does. The six real experts' own
    /// linear weights adapt unperturbed, the same layering every other
    /// `encode_*` method in this file uses.
    pub fn encode_logit_sse(
        &mut self,
        encoder: &mut Encoder,
        context: Context,
        byte: u8,
        mixer: &mut SurpriseLogisticMixLogitSse,
    ) {
        self.encode_mix(encoder, context, byte, mixer);
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_logit_sse`]; see that method's docs for the coding
    /// and update shape.
    ///
    /// Never panics on adversarial `decoder` state, the same argument
    /// [`Self::decode_sse`]'s docs give: [`Decoder::decode_bit`] is total
    /// over any coded bit pattern, and every probability
    /// [`Self::code_bit_walk`] derives is this model's and `mixer`'s
    /// own invariant, never derived from `decoder`'s bytes.
    #[must_use]
    pub fn decode_logit_sse(
        &mut self,
        decoder: &mut Decoder,
        context: Context,
        mixer: &mut SurpriseLogisticMixLogitSse,
    ) -> u8 {
        self.decode_mix(decoder, context, mixer)
    }

    /// Codes `byte` through `encoder` under `context`, blending this
    /// model's six expert banks through `logistic`'s logit-domain mix
    /// instead of [`Self::mix`]'s linear blend
    /// (`research/JOURNAL.md` S1-P8, S2-A101, `codec::LOGISTIC_MIN_VERSION`),
    /// then updates every expert bank exactly as [`Self::encode_sse`]
    /// does. The six real experts' own linear weights adapt unperturbed
    /// ([`Self::update`] still runs on the same six-way `mixed` estimate it
    /// always has) — the same layering [`Self::encode_column`] already
    /// uses for its seventh expert, not a new coupling.
    pub fn encode_logistic(
        &mut self,
        encoder: &mut Encoder,
        context: Context,
        byte: u8,
        logistic: &mut LogisticMix,
    ) {
        self.encode_mix(encoder, context, byte, logistic);
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_logistic`]; see that method's docs for the coding
    /// and update shape.
    ///
    /// Never panics on adversarial `decoder` state, the same argument
    /// [`Self::decode_sse`]'s docs give: [`Decoder::decode_bit`] is total
    /// over any coded bit pattern, and every probability
    /// [`Self::code_bit_walk`] derives is this model's and `logistic`'s
    /// own invariant, never derived from `decoder`'s bytes.
    #[must_use]
    pub fn decode_logistic(
        &mut self,
        decoder: &mut Decoder,
        context: Context,
        logistic: &mut LogisticMix,
    ) -> u8 {
        self.decode_mix(decoder, context, logistic)
    }
}

#[cfg(test)]
mod tests;
