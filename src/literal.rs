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
//! 3).** [`Literal::encode_sse`]/[`Literal::decode_sse`] code the same mixed
//! `cum` table [`Literal::encode`]/[`Literal::decode`] do, but through
//! [`crate::bittree::encode_symbol_sse`]/[`crate::bittree::decode_symbol_sse`]
//! instead of one direct 256-way range division: 8 chained binary decisions,
//! each refined by [`Literal`]'s own [`Sse`] table
//! (`crate::bittree::SSE_CONTEXTS` contexts, keyed by tree position only,
//! `crate::bittree::sse_context`) before it reaches the coder. This
//! calibrates the six-expert mixer's own blended probability at each
//! binary-tree node, a compound estimate — unlike `JOURNAL` S2-R1's
//! rejected attempt, which SSE-calibrated an already order-0-adaptive lone
//! frequency counter (the flag model's `is_copy` bit) and found nothing to
//! correct. The old [`Literal::encode`]/[`Literal::decode`] pair stays,
//! unchanged, for decoding `FORMAT_VERSION` 2 frames
//! (`tests/golden/v2-lz-repeated-text.mgdc` pins that forever); `codec.rs`
//! picks between the two paths by the frame's declared version.
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

use std::num::NonZeroUsize;

use crate::bittree;
use crate::coder::{Decoder, Encoder};
use crate::sse::Sse;

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

/// Frequency increment and rescale ceiling for the other five experts.
/// Ported unchanged from the archive's `INC`/`LIM`, the same values
/// [`crate::model::Model`] uses for the flag/length/offset stages.
const DEFAULT_INCREMENT: u32 = 12;
const DEFAULT_LIMIT: u32 = 65536;

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
fn exp(x: f64) -> f64 {
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

/// Per-byte modeling context [`Literal::encode`]/[`Literal::decode`]
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
    let rate_context = prev1 | (after_copy * 256);
    let order2 = ((prev1 << 8) | prev2) & 0xFFF;
    let align = ((context.position & 3) << 4) | (prev1 >> 4);
    let word_hash =
        usize::try_from(context.word_hash & 0xFFF).expect("masked to 12 bits, always fits usize");
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

/// Measurement-only seventh expert for `research/JOURNAL.md` S1-P5's "does
/// column identity help, blended in alongside the shipped six" hypothesis
/// (`crate::codec::ideal_cost_bits_column_expert_experiment`'s only
/// caller, via [`Literal::ideal_cost_bits_column_expert_pair`]). Not part
/// of [`Literal`]'s own persisted state and never constructed by
/// `encode`/`decode`: a column-keyed frequency bank plus its own single
/// mixing weight per weight-context key (the same key `Literal`'s own six
/// weight vectors are indexed by), adapting on its own trajectory
/// alongside, never inside, the six real experts' weights.
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
}

impl ColumnExpertState {
    /// A fresh column-expert state: every bank starts at frequency 1 per
    /// symbol (the same Laplace floor [`Literal::new`] starts its six
    /// experts at), every weight starts at 1.0 (equally trusted).
    #[must_use]
    pub fn new(max_banks: NonZeroUsize) -> Self {
        Self {
            freq: vec![1u32; max_banks.get() * ALPHABET],
            total: vec![ALPHABET_U32; max_banks.get()],
            weight: vec![1.0; WEIGHT_CONTEXTS],
        }
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

    /// Blends the six experts' banks under the current mixing weights
    /// into a cumulative-frequency table over the 256 byte values.
    /// Ported unchanged from the archive's `Lit::cum`: every symbol's
    /// mixed count gets `+1` (a Laplace floor, the same "nothing is ever
    /// impossible to code" guarantee [`crate::model::Model`] gives by
    /// starting every frequency at 1), so `cum` is always strictly
    /// increasing and `cum[ALPHABET]` is always the true total passed to
    /// the coder.
    fn mix(&self, bank_indices: &[usize; EXPERTS], weight_index: usize) -> [u64; ALPHABET + 1] {
        let weights = &self.weights[weight_index];
        let weight_sum: f64 = weights.iter().sum();
        let mut scale = [0u64; EXPERTS];
        for expert in 0..EXPERTS {
            let normalized = weights[expert] / weight_sum;
            let bank_total = f64::from(self.total[bank_indices[expert]]);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "fixed-point scale factor: normalized weight is in (0,1], bank_total > 0, the product is always non-negative and truncation is the intended floor"
            )]
            {
                scale[expert] = ((normalized * FIXED_POINT_SCALE) / bank_total) as u64;
            }
        }
        let mut cum = [0u64; ALPHABET + 1];
        let mut acc = 0u64;
        for symbol in 0..ALPHABET {
            let mut mixed = 0u64;
            for expert in 0..EXPERTS {
                let freq = u64::from(self.freq[bank_indices[expert] * ALPHABET + symbol]);
                mixed += scale[expert] * freq;
            }
            acc += (mixed >> 16) + 1;
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
        let mut estimate = [0f64; EXPERTS];
        for (expert, bank) in bank_indices.iter().enumerate() {
            estimate[expert] =
                f64::from(self.freq[bank * ALPHABET + symbol]) / f64::from(self.total[*bank]);
        }
        let weights = &mut self.weights[weight_index];
        let weight_sum: f64 = weights.iter().sum();
        let mixed: f64 = (0..EXPERTS)
            .map(|expert| weights[expert] * estimate[expert])
            .sum::<f64>()
            / weight_sum;
        let denominator = mixed.max(MIN_DENOMINATOR);
        for expert in 0..EXPERTS {
            let gradient = LEARNING_RATE * (estimate[expert] - mixed) / denominator;
            weights[expert] = (weights[expert] * exp_fn(gradient)).clamp(MIN_WEIGHT, MAX_WEIGHT);
        }
        for (expert, &bank) in bank_indices.iter().enumerate() {
            let (increment, limit) = if expert == 0 {
                (FAST_INCREMENT, FAST_LIMIT)
            } else {
                (DEFAULT_INCREMENT, DEFAULT_LIMIT)
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

    /// Codes `byte` through `encoder` under `context`, then updates
    /// every expert bank and the mixing weights.
    pub fn encode(&mut self, encoder: &mut Encoder, context: Context, byte: u8) {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        let symbol = usize::from(byte);
        encoder.encode(cum[symbol], cum[symbol + 1], cum[ALPHABET]);
        self.update(&bank_indices, weight_index, symbol, exp);
    }

    /// Codes `byte` through `encoder` under `context`, same as
    /// [`Self::encode`], except the mixed `cum` table is coded as 8 chained
    /// binary decisions through [`bittree::encode_symbol_sse`], each
    /// calibrated by this model's own [`Sse`] table, instead of one direct
    /// 256-way range division (`research/JOURNAL.md` S1-P1, `FORMAT_VERSION`
    /// 3). The underlying six-expert mixer still adapts exactly as
    /// [`Self::encode`] leaves it: `update` runs unconditionally after the
    /// symbol is coded, regardless of which coding path chose it.
    pub fn encode_sse(&mut self, encoder: &mut Encoder, context: Context, byte: u8) {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        bittree::encode_symbol_sse(encoder, &cum, byte, &mut self.sse);
        self.update(&bank_indices, weight_index, usize::from(byte), exp);
    }

    /// Decodes one byte from `decoder` under `context`, the exact inverse
    /// of [`Self::encode_sse`], then updates the model the same way
    /// [`Self::decode`] did.
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
    /// cum[ALPHABET])` — then updates the model the same way
    /// [`Self::encode`] does. No [`Encoder`] involved: this is
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

    /// `research/JOURNAL.md` S1-P5, before-wiring measurement: prices
    /// `byte` twice from the same pre-update six-expert state, the paired
    /// methodology S2-R6 used for S1-P3's escape fallback — once under the
    /// shipped mix ([`Self::ideal_cost_bits`] exactly, including its own
    /// `update` call, so the six real experts adapt on their one real
    /// trajectory regardless of this method ever being called), once with
    /// `column_state`'s bank blended in as a seventh expert. `column_state`
    /// adapts on its own trajectory: its bank observes `byte` the same way
    /// [`Self::update`]'s five default-rate experts do, and its one mixing
    /// weight adapts toward whichever side — its own local estimate vs. the
    /// seven-expert blend — predicted `byte` better, independent of the six
    /// real weights (never written back into `self.weights`).
    ///
    /// Returns `(baseline_bits, with_column_bits)`.
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't apply off the coding path)"
    )]
    pub fn ideal_cost_bits_column_expert_pair(
        &mut self,
        context: Context,
        byte: u8,
        column_bank: usize,
        column_state: &mut ColumnExpertState,
    ) -> (f64, f64) {
        let (bank_indices, weight_index) = banks(context);
        let symbol = usize::from(byte);

        let weights6 = self.weights[weight_index];
        let w7 = column_state.weight[weight_index];
        let weight_sum = weights6.iter().sum::<f64>() + w7;

        // Seven-wide fixed-point blend, mirroring `mix`'s own shape with
        // one more expert, over the pre-update state both prices below
        // share.
        let mut scale6 = [0u64; EXPERTS];
        for expert in 0..EXPERTS {
            let normalized = weights6[expert] / weight_sum;
            let bank_total = f64::from(self.total[bank_indices[expert]]);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "same fixed-point scale factor mix() uses: normalized weight is in (0,1], bank_total > 0"
            )]
            {
                scale6[expert] = ((normalized * FIXED_POINT_SCALE) / bank_total) as u64;
            }
        }
        let column_total = f64::from(column_state.total[column_bank]);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "same fixed-point scale factor mix() uses: normalized weight is in (0,1], column_total > 0"
        )]
        let scale7 = (((w7 / weight_sum) * FIXED_POINT_SCALE) / column_total) as u64;

        let mut total_mixed = 0u64;
        let mut symbol_mixed = 0u64;
        for s in 0..ALPHABET {
            let mut mixed = 0u64;
            for expert in 0..EXPERTS {
                let freq = u64::from(self.freq[bank_indices[expert] * ALPHABET + s]);
                mixed += scale6[expert] * freq;
            }
            let freq7 = u64::from(column_state.freq[column_bank * ALPHABET + s]);
            mixed += scale7 * freq7;
            let contribution = (mixed >> 16) + 1;
            total_mixed += contribution;
            if s == symbol {
                symbol_mixed = contribution;
            }
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "same bound as ideal_cost_bits: fixed-point sums stay well under 2^53"
        )]
        let with_column_probability = symbol_mixed as f64 / total_mixed as f64;
        let with_column_bits = -with_column_probability.log2();

        // The column expert's own weight adapts on the same continuous-
        // probability-space rule Self::update uses for the six real
        // weights, restricted to this one component: how well its own
        // local estimate did against the seven-expert blend.
        let column_estimate =
            f64::from(column_state.freq[column_bank * ALPHABET + symbol]) / column_total;
        let mut estimate6 = [0f64; EXPERTS];
        for (expert, &bank) in bank_indices.iter().enumerate() {
            estimate6[expert] =
                f64::from(self.freq[bank * ALPHABET + symbol]) / f64::from(self.total[bank]);
        }
        let mixed_estimate = (weights6
            .iter()
            .zip(estimate6.iter())
            .map(|(&w, &e)| w * e)
            .sum::<f64>()
            + w7 * column_estimate)
            / weight_sum;
        let denominator = mixed_estimate.max(MIN_DENOMINATOR);
        let gradient = LEARNING_RATE * (column_estimate - mixed_estimate) / denominator;
        column_state.weight[weight_index] = (w7 * exp(gradient)).clamp(MIN_WEIGHT, MAX_WEIGHT);

        crate::rescale_bank(
            &mut column_state.freq[column_bank * ALPHABET..column_bank * ALPHABET + ALPHABET],
            &mut column_state.total[column_bank],
            symbol,
            DEFAULT_INCREMENT,
            DEFAULT_LIMIT,
        );

        let baseline_bits = self.ideal_cost_bits(context, byte);

        (baseline_bits, with_column_bits)
    }

    /// Decodes one byte from `decoder` under `context`, then updates the
    /// model the same way [`Self::encode`] did, keeping both sides in
    /// lockstep.
    ///
    /// Never panics on adversarial `decoder` state: [`Decoder::target`]
    /// is mathematically bounded to `[0, total)`, and the mixed
    /// cumulative-frequency table is built so every symbol contributes
    /// at least `1`, so `cum[ALPHABET]` always exceeds any in-range
    /// target and the scan below always finds a symbol before running
    /// past the table.
    #[must_use]
    pub fn decode(&mut self, decoder: &mut Decoder, context: Context) -> u8 {
        let (bank_indices, weight_index, cum) = self.banks_and_cum(context);
        let total = cum[ALPHABET];
        let target = decoder.target(total);
        let mut symbol = 0usize;
        while cum[symbol + 1] <= target {
            symbol += 1;
        }
        decoder.decode(cum[symbol], cum[symbol + 1], total);
        self.update(&bank_indices, weight_index, symbol, exp);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "symbol is a scan index bounded by ALPHABET (256), always fits u8"
        )]
        {
            symbol as u8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_bytes(bytes: &[u8]) {
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in bytes {
            model.encode(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let encoded = enc.finish();

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(&encoded);
        let mut got = Vec::with_capacity(bytes.len());
        for _ in bytes {
            let b = model.decode(&mut dec, context);
            context = context.after_literal(b);
            got.push(b);
        }
        assert_eq!(got, bytes);
    }

    fn roundtrip_bytes_sse(bytes: &[u8]) {
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in bytes {
            model.encode_sse(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let encoded = enc.finish();

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(&encoded);
        let mut got = Vec::with_capacity(bytes.len());
        for _ in bytes {
            let b = model.decode_sse(&mut dec, context);
            context = context.after_literal(b);
            got.push(b);
        }
        assert_eq!(got, bytes);
    }

    #[test]
    fn empty_stream_round_trips() {
        roundtrip_bytes(&[]);
    }

    #[test]
    fn single_byte_round_trips() {
        roundtrip_bytes(b"x");
    }

    #[test]
    fn skewed_repeat_round_trips() {
        roundtrip_bytes(&b"aaaaaaaaaaaaaaaaaaaaaaaaaab".repeat(20));
    }

    #[test]
    fn full_alphabet_cycles_round_trip() {
        let bytes: Vec<u8> = (0..2000).map(|i| u8::try_from(i % 256).unwrap()).collect();
        roundtrip_bytes(&bytes);
    }

    #[test]
    fn ascii_text_round_trips() {
        let text = b"the quick brown fox jumps over the lazy dog, again and again.".repeat(50);
        roundtrip_bytes(&text);
    }

    #[test]
    fn pseudo_random_bytes_round_trip() {
        // 5000 bytes crosses every bank's rescale threshold repeatedly,
        // including the fast expert's low 6144 ceiling.
        let bytes: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
            .take(5000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();
        roundtrip_bytes(&bytes);
    }

    #[test]
    fn copy_tokens_interleave_with_literals_round_trip() {
        // The shape Method-wiring will actually drive this with: literal
        // runs broken up by simulated LZ copy tokens, each shifting
        // `after_copy` and re-deriving `prev1`/`prev2` from the copied
        // bytes rather than from `after_literal`'s single-byte update.
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        let literal_runs: &[&[u8]] = &[b"hello ", b"world", b" repeat repeat repeat"];
        let copy_runs: &[&[u8]] = &[b"repeat repeat", b"o", b""];
        for (lits, copy) in literal_runs.iter().zip(copy_runs.iter()) {
            for &b in *lits {
                model.encode(&mut enc, context, b);
                context = context.after_literal(b);
            }
            context = context.after_copy(copy);
        }
        let encoded = enc.finish();
        let total_literals: usize = literal_runs.iter().map(|r| r.len()).sum();

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(&encoded);
        let mut got = Vec::with_capacity(total_literals);
        for (lits, copy) in literal_runs.iter().zip(copy_runs.iter()) {
            for _ in *lits {
                let b = model.decode(&mut dec, context);
                context = context.after_literal(b);
                got.push(b);
            }
            context = context.after_copy(copy);
        }
        let expected: Vec<u8> = literal_runs
            .iter()
            .flat_map(|r| r.iter().copied())
            .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn empty_stream_round_trips_through_sse() {
        roundtrip_bytes_sse(&[]);
    }

    #[test]
    fn single_byte_round_trips_through_sse() {
        roundtrip_bytes_sse(b"x");
    }

    #[test]
    fn skewed_repeat_round_trips_through_sse() {
        roundtrip_bytes_sse(&b"aaaaaaaaaaaaaaaaaaaaaaaaaab".repeat(20));
    }

    #[test]
    fn full_alphabet_cycles_round_trip_through_sse() {
        let bytes: Vec<u8> = (0..2000).map(|i| u8::try_from(i % 256).unwrap()).collect();
        roundtrip_bytes_sse(&bytes);
    }

    #[test]
    fn ascii_text_round_trips_through_sse() {
        let text = b"the quick brown fox jumps over the lazy dog, again and again.".repeat(50);
        roundtrip_bytes_sse(&text);
    }

    #[test]
    fn pseudo_random_bytes_round_trip_through_sse() {
        let bytes: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
            .take(5000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();
        roundtrip_bytes_sse(&bytes);
    }

    #[test]
    fn decoding_truncated_stream_does_not_panic_through_sse() {
        let bytes: Vec<u8> = (0..200).map(|i| u8::try_from(i % 5).unwrap()).collect();
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in &bytes {
            model.encode_sse(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let encoded = enc.finish();
        let truncated = &encoded[..encoded.len() / 2];

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(truncated);
        for _ in &bytes {
            let b = model.decode_sse(&mut dec, context);
            context = context.after_literal(b);
        }
        // No panic is the assertion, same as decoding_truncated_stream_does_not_panic.
    }

    #[test]
    fn context_after_literal_tracks_previous_bytes_and_position() {
        let context = Context::default();
        let context = context.after_literal(b'a');
        assert_eq!(context.prev1, b'a');
        assert_eq!(context.prev2, 0);
        assert_eq!(context.position, 1);
        assert!(!context.after_copy);
        let context = context.after_literal(b'b');
        assert_eq!(context.prev1, b'b');
        assert_eq!(context.prev2, b'a');
        assert_eq!(context.position, 2);
    }

    #[test]
    fn context_after_copy_of_zero_bytes_keeps_previous_bytes() {
        let context = Context::default().after_literal(b'a').after_literal(b'b');
        let after = context.after_copy(&[]);
        assert_eq!(after.prev1, context.prev1);
        assert_eq!(after.prev2, context.prev2);
        assert_eq!(after.position, context.position);
        assert!(after.after_copy);
    }

    #[test]
    fn context_after_copy_of_one_byte_shifts_prev1_into_prev2() {
        let context = Context::default().after_literal(b'a');
        let after = context.after_copy(b"z");
        assert_eq!(after.prev1, b'z');
        assert_eq!(after.prev2, b'a');
        assert_eq!(after.position, 2);
    }

    #[test]
    fn context_after_copy_of_many_bytes_uses_last_two() {
        let after = Context::default().after_copy(b"hello");
        assert_eq!(after.prev1, b'o');
        assert_eq!(after.prev2, b'l');
        assert_eq!(after.position, 5);
    }

    #[test]
    fn word_hash_extends_on_alnum_and_resets_on_punctuation() {
        let hash_a = advance_word_hash(0, b'a');
        let hash_ab = advance_word_hash(hash_a, b'b');
        assert_ne!(hash_a, 0);
        assert_ne!(hash_ab, hash_a);
        assert_eq!(advance_word_hash(hash_ab, b' '), 0);
        assert_eq!(advance_word_hash(hash_ab, b'.'), 0);
    }

    #[test]
    fn ideal_cost_drops_as_a_byte_gets_more_likely() {
        // Coding the same byte repeatedly raises its own frequency across
        // every expert bank it touches, so its ideal cost must strictly
        // decrease call over call as the model adapts toward it. Context
        // stabilizes after the first call (prev1 becomes 'a' and stays
        // there), so this isolates the adaptation, not a context change.
        let mut model = Literal::new();
        let context = Context::default().after_literal(b'a');
        let first = model.ideal_cost_bits(context, b'a');
        let second = model.ideal_cost_bits(context, b'a');
        let third = model.ideal_cost_bits(context, b'a');
        assert!(second < first);
        assert!(third < second);
    }

    #[test]
    fn ideal_cost_bits_sse_drops_as_a_byte_gets_more_likely() {
        // Same shape as ideal_cost_drops_as_a_byte_gets_more_likely, for
        // the SSE-calibrated path.
        let mut model = Literal::new();
        let context = Context::default().after_literal(b'a');
        let first = model.ideal_cost_bits_sse(context, b'a');
        let second = model.ideal_cost_bits_sse(context, b'a');
        let third = model.ideal_cost_bits_sse(context, b'a');
        assert!(second < first);
        assert!(third < second);
    }

    #[test]
    fn ideal_cost_bits_sse_updates_state_same_as_encode_sse() {
        // Same shape as ideal_cost_updates_state_same_as_encode, for the
        // SSE-calibrated path: encode_sse and ideal_cost_bits_sse must
        // leave both the mixer and this model's own Sse table in the same
        // state.
        let bytes = b"hello world hello again";
        let mut via_encode = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in bytes {
            via_encode.encode_sse(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let mut via_ideal_cost = Literal::new();
        let mut ideal_context = Context::default();
        for &b in bytes {
            let _ = via_ideal_cost.ideal_cost_bits_sse(ideal_context, b);
            ideal_context = ideal_context.after_literal(b);
        }
        assert_eq!(context, ideal_context);
        assert!(
            (via_encode.ideal_cost_bits_sse(context, b'!')
                - via_ideal_cost.ideal_cost_bits_sse(context, b'!'))
            .abs()
                < 1e-9
        );
    }

    #[test]
    fn ideal_cost_updates_state_same_as_encode() {
        // ideal_cost_bits must leave the model in the same state encode
        // would have: fork two identical models, drive one through each
        // path over the same bytes, then confirm they agree from here by
        // coding one more byte on top of each and comparing cost.
        let bytes = b"hello world hello again";
        let mut via_encode = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in bytes {
            via_encode.encode(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let mut via_ideal_cost = Literal::new();
        let mut ideal_context = Context::default();
        for &b in bytes {
            let _ = via_ideal_cost.ideal_cost_bits(ideal_context, b);
            ideal_context = ideal_context.after_literal(b);
        }
        assert_eq!(context, ideal_context);
        assert!(
            (via_encode.ideal_cost_bits(context, b'!')
                - via_ideal_cost.ideal_cost_bits(context, b'!'))
            .abs()
                < 1e-9
        );
    }

    #[test]
    fn ideal_cost_sum_tracks_real_encoded_length() {
        // Named corpus (CLAUDE.md hard rule 4): the founding session's
        // archived codec, real structured Rust source, the same fixture
        // vendored_exp_keeps_bits_per_byte_within_one_percent_of_f64_exp
        // above uses. Summed ideal cost is an estimate, not the real
        // coder's bit-exact output (integer cumulative-frequency division
        // rounds; the coder also pays a handful of flush bits at the very
        // end), so this checks closeness, not equality — the same
        // tolerance shape as that test and model.rs's counterpart.
        let corpus: &[u8] = include_bytes!("../research/imports/session-1/mothergod.rs");

        let mut ideal_cost_model = Literal::new();
        let mut context = Context::default();
        let ideal_bits: f64 = corpus
            .iter()
            .map(|&b| {
                let cost = ideal_cost_model.ideal_cost_bits(context, b);
                context = context.after_literal(b);
                cost
            })
            .sum();

        let mut real_model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in corpus {
            real_model.encode(&mut enc, context, b);
            context = context.after_literal(b);
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "encoded length is far below f64's exact integer range (2^53)"
        )]
        let real_bits = (enc.finish().len() * 8) as f64;

        let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
        assert!(
            relative_diff <= 0.01,
            "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
             {relative_diff:.4} relative difference exceeds the 1% budget"
        );
    }

    #[test]
    fn decoding_truncated_stream_does_not_panic() {
        let bytes: Vec<u8> = (0..200).map(|i| u8::try_from(i % 5).unwrap()).collect();
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in &bytes {
            model.encode(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let encoded = enc.finish();
        let truncated = &encoded[..encoded.len() / 2];

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(truncated);
        for _ in &bytes {
            let b = model.decode(&mut dec, context);
            context = context.after_literal(b);
        }
        // No panic is the assertion: decoded bytes past the real data are
        // whatever implicit-zero bits produce, never treated as ground
        // truth here.
    }

    /// `f64::exp`, the pre-ADR-0024 reference this test diffs `exp`
    /// against. `#[cfg(test)]`-gated, so it never reaches the decode
    /// path this crate ships.
    fn reference_exp(x: f64) -> f64 {
        #[allow(
            clippy::disallowed_methods,
            reason = "test-only oracle for ADR-0024's 1% accuracy claim (issue #161); #[cfg(test)] keeps it off the decode path"
        )]
        {
            x.exp()
        }
    }

    #[test]
    fn vendored_exp_keeps_bits_per_byte_within_one_percent_of_f64_exp() {
        // Named corpus (CLAUDE.md hard rule 4): the founding session's
        // archived codec, real structured Rust source, 25,524 bytes.
        let corpus: &[u8] = include_bytes!("../research/imports/session-1/mothergod.rs");

        let encoded_len = |exp_fn: fn(f64) -> f64| -> usize {
            let mut model = Literal::new();
            let mut context = Context::default();
            let mut enc = Encoder::new();
            for &b in corpus {
                let (bank_indices, weight_index, cum) = model.banks_and_cum(context);
                let symbol = usize::from(b);
                enc.encode(cum[symbol], cum[symbol + 1], cum[ALPHABET]);
                model.update(&bank_indices, weight_index, symbol, exp_fn);
                context = context.after_literal(b);
            }
            enc.finish().len()
        };

        let vendored_bytes = encoded_len(exp);
        let reference_bytes = encoded_len(reference_exp);

        #[allow(
            clippy::cast_precision_loss,
            reason = "encoded length is far below f64's exact integer range (2^53)"
        )]
        let relative_diff =
            (vendored_bytes as f64 - reference_bytes as f64).abs() / reference_bytes as f64;

        assert!(
            relative_diff <= 0.01,
            "vendored exp: {vendored_bytes} bytes vs f64::exp reference: {reference_bytes} \
             bytes, {relative_diff:.4} relative difference exceeds the 1% budget (ADR-0024)"
        );
    }

    #[test]
    fn column_expert_state_new_starts_at_the_laplace_floor() {
        let state = ColumnExpertState::new(crate::test_support::nz(4));
        assert_eq!(state.freq.len(), 4 * ALPHABET);
        assert!(state.freq.iter().all(|&f| f == 1));
        assert_eq!(state.total, vec![ALPHABET_U32; 4]);
        assert_eq!(state.weight, vec![1.0; WEIGHT_CONTEXTS]);
    }

    #[test]
    fn column_expert_pair_baseline_matches_plain_ideal_cost_bits() {
        // The pair's baseline side is Self::ideal_cost_bits verbatim
        // (`Self::ideal_cost_bits_column_expert_pair`'s own docs): walking
        // a model through the paired method must land on exactly the same
        // per-byte costs and exactly the same six-expert state a model
        // walked through plain `ideal_cost_bits` alone would, byte for
        // byte, with column_state along for the ride.
        let mut paired = Literal::new();
        let mut plain = Literal::new();
        let mut column_state = ColumnExpertState::new(crate::test_support::nz(4));
        let mut context = Context::default();
        for &b in b"the quick brown fox jumps over the lazy dog" {
            let (baseline, _) =
                paired.ideal_cost_bits_column_expert_pair(context, b, 0, &mut column_state);
            let expected = plain.ideal_cost_bits(context, b);
            assert!(
                (baseline - expected).abs() < 1e-9,
                "byte {b:?}: paired baseline {baseline} vs plain {expected}"
            );
            context = context.after_literal(b);
        }
    }

    #[test]
    fn column_expert_pair_updates_only_its_own_column_bank() {
        let mut model = Literal::new();
        let mut column_state = ColumnExpertState::new(crate::test_support::nz(4));
        let context = Context::default();
        let _ = model.ideal_cost_bits_column_expert_pair(context, b'x', 2, &mut column_state);

        let bank2_total: u32 = column_state.freq[2 * ALPHABET..3 * ALPHABET].iter().sum();
        assert_eq!(bank2_total, column_state.total[2]);
        assert_eq!(
            column_state.freq[2 * ALPHABET + usize::from(b'x')],
            1 + DEFAULT_INCREMENT
        );
        for other in [0usize, 1, 3] {
            assert!(
                column_state.freq[other * ALPHABET..(other + 1) * ALPHABET]
                    .iter()
                    .all(|&f| f == 1)
            );
            assert_eq!(column_state.total[other], ALPHABET_U32);
        }
    }

    #[test]
    fn column_expert_pair_costs_stay_finite_and_positive() {
        let mut model = Literal::new();
        let mut column_state = ColumnExpertState::new(crate::test_support::nz(8));
        let mut context = Context::default();
        for (i, &b) in b"0123456789abcdefghijklmnopqrstuvwxyz".iter().enumerate() {
            let bank = i % 8;
            let (baseline, with_column) =
                model.ideal_cost_bits_column_expert_pair(context, b, bank, &mut column_state);
            assert!(
                baseline.is_finite() && baseline > 0.0,
                "baseline={baseline}"
            );
            assert!(
                with_column.is_finite() && with_column > 0.0,
                "with_column={with_column}"
            );
            context = context.after_literal(b);
        }
    }
}
