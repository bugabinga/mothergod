//! Binary decomposition of a 256-symbol cumulative-frequency table into a
//! sequence of top-down binary split decisions ("bit tree"): the shape
//! ROADMAP M3's oldest standing lead needed to calibrate a compound
//! estimate instead of a lone counter (`research/JOURNAL.md` S1-P1,
//! S2-R1's postmortem, `crate::sse`'s module docs).
//!
//! Not a port: no archive precedent (`crate::sse`'s module docs record the
//! same grep-clean result for S1-P1 generally). [`encode_symbol`]/
//! [`decode_symbol`] (the non-SSE pair) stay standalone, exercised only by
//! this module's own tests; [`encode_symbol_sse`]/[`decode_symbol_sse`]
//! are wired (`research/JOURNAL.md` S2-A60, ADR-0038,
//! [`crate::literal::Literal::encode_sse`]/`decode_sse`).
//!
//! [`encode_symbol`]/[`decode_symbol`] code one byte as 8 chained binary
//! decisions instead of [`crate::coder::Encoder::encode`]/
//! [`crate::coder::Decoder::decode`]'s single 256-way range division:
//! each step splits the current candidate symbol range `[lo, hi)` at its
//! midpoint and asks whether the true symbol falls in the upper half, at
//! the probability that split has under a caller-supplied cumulative
//! table shaped like [`crate::model::Model`]'s or
//! `crate::literal::Literal::mix`'s own output. The chain rule of
//! probability makes the product of those 8 binary probabilities along a
//! symbol's path equal `(cum[symbol + 1] - cum[symbol]) /
//! cum[ALPHABET]` exactly — the same partition, reshaped into a sequence
//! of binary decisions instead of one 256-way division. That reshaping
//! is the point: a binary decision, not a 256-ary one, is what
//! [`crate::sse::Sse`] calibrates. [`ideal_cost_bits`] checks the
//! identity directly; the round-trip tests below check it end to end
//! through the real coder.
//!
//! [`sse_context`] answers S1-P1's other named prerequisite: which
//! [`crate::sse::Sse`] context a given walk step should key on.
//!
//! [`encode_symbol_sse`]/[`decode_symbol_sse`] compose the two: the same
//! chain-rule walk as [`encode_symbol`]/[`decode_symbol`], but each level's
//! raw `cum`-derived probability is first refined through a caller-supplied
//! [`crate::sse::Sse`] table (keyed by [`sse_context`]) before it reaches
//! [`crate::coder::Encoder::encode_bit`]/[`crate::coder::Decoder::decode_bit`],
//! and the table is updated on the raw probability afterward — the shape
//! [`crate::sse::Sse`]'s own test suite already proves round-trips
//! (`calibrated_probability_round_trips_and_costs_less_than_a_fixed_split`).
//! `crate::literal::Literal::encode_sse`/`decode_sse` are the only callers,
//! closing `research/JOURNAL.md` S1-P1.

use crate::coder::{Decoder, Encoder};
use crate::sse::Sse;

/// Byte alphabet a cumulative table spans, matching
/// [`crate::literal::Literal`]'s own alphabet size.
const ALPHABET: usize = 256;

/// `log2(ALPHABET)`: number of binary decisions that pin down one
/// symbol out of 256.
const LEVELS: u32 = 8;

/// Number of distinct `(depth, prefix)` pairs [`sse_context`] can be
/// called with: one per internal node of the depth-`LEVELS` binary tree
/// [`encode_symbol`]/[`decode_symbol`] walk, `2^LEVELS - 1` (255 for
/// `LEVELS = 8`) — a full binary tree with `2^LEVELS` leaves has exactly
/// that many internal nodes. `crate::sse::Sse::new`'s `contexts`
/// argument for a table keyed on this scheme.
pub const SSE_CONTEXTS: usize = (1 << LEVELS) - 1;

/// Maps one step of [`encode_symbol`]/[`decode_symbol`]'s walk — the
/// decision at tree depth `depth` (`0..LEVELS`, `0` is the first,
/// coarsest split) having already decided `prefix` (the `depth` bits
/// chosen so far, i.e. `lo` divided by that depth's range width,
/// `0..2^depth`) — to a unique index in `0..SSE_CONTEXTS`, for
/// [`crate::sse::Sse::refine`]/[`crate::sse::Sse::update`] to key on.
/// `S1-P1`'s own remaining-scope decision (`research/JOURNAL.md`):
/// keying purely on tree position, the cheapest scheme that still gives
/// every node its own calibration, no coarser (folding two nodes
/// together loses the distinction the walk actually observed) and no
/// finer (there is no more context available per node than its position
/// — the symbol identity itself is exactly what has not been decided
/// yet at that node). Same numbering LZMA's literal coder uses for its
/// own binary-tree probability array (`probs[(1 << depth) | prefix]`,
/// 1-indexed there; `- 1` here to land in `0..SSE_CONTEXTS` instead).
///
/// # Panics
///
/// Panics if `depth >= LEVELS` or `prefix >= 1 << depth`: both are
/// derived from this module's own walk (`depth` counts loop iterations
/// bounded by `LEVELS`, `prefix` is `lo` divided by the current range
/// width), never from adversarial input — the same caller-code
/// invariant `cum`'s own shape check documents for [`encode_symbol`]/
/// [`decode_symbol`].
#[must_use]
pub fn sse_context(depth: u32, prefix: usize) -> usize {
    assert!(
        depth < LEVELS,
        "depth must be < LEVELS ({LEVELS}), got {depth}"
    );
    assert!(
        prefix < (1usize << depth),
        "prefix must be < 2^depth (2^{depth}), got {prefix}"
    );
    (1usize << depth) + prefix - 1
}

/// Panics if `cum` is not shaped like a cumulative-frequency table over
/// `ALPHABET` symbols: `ALPHABET + 1` entries, monotonically
/// non-decreasing, every symbol carrying strictly positive mass (`cum[i]
/// < cum[i + 1]`). A caller-code invariant, not something adversarial
/// input can trigger: every cumulative table this crate builds already
/// keeps it, the same "nothing is ever impossible to code" guarantee
/// [`crate::model::Model`] and `crate::literal::Literal::mix` give by
/// Laplace-flooring every symbol to at least mass 1.
fn check_table_shape(cum: &[u64]) {
    assert!(
        cum.len() == ALPHABET + 1,
        "cum must have exactly ALPHABET + 1 (257) entries, got {}",
        cum.len()
    );
    assert!(
        cum.windows(2).all(|w| w[0] < w[1]),
        "cum must be strictly increasing: every symbol needs positive mass"
    );
}

/// Probability the true symbol lies in the upper half of `[lo, hi)`,
/// read off `cum`: `(cum[hi] - cum[mid]) / (cum[hi] - cum[lo])`, `mid`
/// the midpoint. Never zero or one: `check_table_shape`'s strictly-
/// increasing invariant keeps both `cum[hi] - cum[mid]` and `cum[mid] -
/// cum[lo]` positive whenever `hi > lo`, which every call site keeps by
/// construction (`hi - lo` halves from 256 down to 1 and never reaches
/// 0 first, since 256 is a power of two).
fn upper_half_probability(cum: &[u64], lo: usize, hi: usize) -> f64 {
    let mid = lo + (hi - lo) / 2;
    #[allow(
        clippy::cast_precision_loss,
        reason = "cum entries are fixed-point sums bounded well under 2^53 \
                  (crate::literal::Literal::mix's own cast carries the same bound)"
    )]
    {
        (cum[hi] - cum[mid]) as f64 / (cum[hi] - cum[lo]) as f64
    }
}

/// Shared halving loop behind [`walk`] and [`walk_sse`]: the `LEVELS`-level
/// binary-tree decomposition of `[0, ALPHABET)`, computing each level's
/// `depth`, `prefix` (`lo / width`, [`sse_context`]'s own argument), midpoint,
/// and raw `upper_half_probability` before handing them to `step`, which
/// returns the bit that level resolved to. [`walk`] ignores `depth`/`prefix`;
/// [`walk_sse`] uses them to key its `Sse` context — the only difference
/// between the two, so this is the one place that difference lives.
///
/// Returns the final `lo`, which after `LEVELS` halvings of `[0, ALPHABET)`
/// is exactly the coded symbol.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
fn walk_steps(cum: &[u64], mut step: impl FnMut(u32, usize, usize, f64) -> bool) -> u8 {
    check_table_shape(cum);
    walk_nodes(|depth, prefix, lo, mid, hi| {
        step(depth, prefix, mid, upper_half_probability(cum, lo, hi))
    })
}

/// The pure tree traversal under [`walk_steps`], with no cumulative table:
/// hands `node` each level's `depth`, `prefix`, and the `[lo, mid, hi)`
/// split it decides, and follows the bit `node` returns. Factored out so a
/// caller pricing the same decisions from its own probabilities
/// ([`crate::literal::LogisticMix`], `research/JOURNAL.md` S2-A101) walks
/// the identical tree the shipped coder does, never a copy of it.
///
/// Returns the final `lo`, the symbol the `LEVELS` decisions resolved to.
pub(crate) fn walk_nodes(mut node: impl FnMut(u32, usize, usize, usize, usize) -> bool) -> u8 {
    let mut lo = 0usize;
    let mut hi = ALPHABET;
    for depth in 0..LEVELS {
        let width = hi - lo;
        let mid = lo + width / 2;
        let prefix = lo / width;
        let bit = node(depth, prefix, lo, mid, hi);
        if bit {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "lo is bounded to [0, ALPHABET) after LEVELS halvings of a 256-wide range, \
                  always fits u8"
    )]
    {
        lo as u8
    }
}

/// Shared skeleton behind [`encode_symbol`], [`decode_symbol`], and
/// [`ideal_cost_bits`]: walks [`walk_steps`], keying on nothing beyond each
/// level's midpoint and raw probability. `code_bit` receives those two and
/// returns the bit that level resolved to — already known from a caller's
/// own `symbol` for [`encode_symbol`] and [`ideal_cost_bits`], decoded from
/// [`Decoder::decode_bit`] for [`decode_symbol`] — so the three callers
/// differ only in what they do with that bit and probability, never in the
/// walk itself. Mirrors [`walk_sse`] for this module's non-SSE trio.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
fn walk(cum: &[u64], mut code_bit: impl FnMut(usize, f64) -> bool) -> u8 {
    walk_steps(cum, |_depth, _prefix, mid, p| code_bit(mid, p))
}

/// Codes `symbol` through `encoder` as `LEVELS` chained binary
/// decisions over `cum`. See the module docs for the identity this
/// implements.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
pub fn encode_symbol(encoder: &mut Encoder, cum: &[u64], symbol: u8) {
    let symbol_index = usize::from(symbol);
    let landed = walk(cum, |mid, p| {
        let bit = symbol_index >= mid;
        encoder.encode_bit(bit, p);
        bit
    });
    debug_assert_eq!(
        landed, symbol,
        "8 halvings of [0, 256) must land exactly on symbol"
    );
}

/// Decodes one byte from `decoder` as `LEVELS` chained binary
/// decisions over `cum`, the exact inverse of [`encode_symbol`].
///
/// Never panics on adversarial `decoder` state: [`Decoder::decode_bit`]
/// is total over any coded bit pattern, same as every other decode path
/// in this crate.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`. `cum` is
/// caller-supplied local state, never derived from `decoder`'s bytes, so
/// this is the same caller-code invariant [`encode_symbol`] documents,
/// not an adversarial-input hazard.
#[must_use]
pub fn decode_symbol(decoder: &mut Decoder, cum: &[u64]) -> u8 {
    walk(cum, |_mid, p| decoder.decode_bit(p))
}

/// Shared skeleton behind [`walk_sse`]: walks [`walk_steps`], computing
/// each level's SSE context via caller-supplied `context_of`, refining and
/// updating `sse` on the raw probability. [`walk_sse`] below keys on
/// exactly [`sse_context`].
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
fn walk_sse_keyed(
    cum: &[u64],
    sse: &mut Sse,
    mut context_of: impl FnMut(u32, usize) -> usize,
    mut code_bit: impl FnMut(usize, f64) -> bool,
) -> u8 {
    walk_steps(cum, |depth, prefix, mid, raw_p| {
        let context = context_of(depth, prefix);
        let refined_p = sse.refine(context, raw_p);
        let bit = code_bit(mid, refined_p);
        sse.update(context, raw_p, bit);
        bit
    })
}

/// Shared skeleton behind [`encode_symbol_sse`], [`decode_symbol_sse`], and
/// [`ideal_cost_bits_sse`]: [`walk_sse_keyed`] keyed on plain [`sse_context`].
/// `code_bit` receives the level's midpoint and refined probability and
/// returns the bit that level resolved to — already known from a caller's
/// own `symbol` for [`encode_symbol_sse`] and [`ideal_cost_bits_sse`],
/// decoded from [`Decoder::decode_bit`] for [`decode_symbol_sse`] — so the
/// three callers differ only in what they do with that bit and probability,
/// never in the SSE walk itself. Matches [`crate::codec::walk_tokens`]'s
/// reasoning: keeping the walk in one place is what stops the encode,
/// decode, and cost-pricing paths from silently drifting apart.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
fn walk_sse(cum: &[u64], sse: &mut Sse, code_bit: impl FnMut(usize, f64) -> bool) -> u8 {
    walk_sse_keyed(cum, sse, sse_context, code_bit)
}

/// Codes `symbol` through `encoder` as `LEVELS` chained binary decisions
/// over `cum`, same as [`encode_symbol`], except each level's raw
/// `upper_half_probability` is first refined through `sse` (keyed by
/// [`sse_context`]) before it drives [`Encoder::encode_bit`], and `sse` is
/// updated on the raw probability afterward — the calibration step S1-P1
/// exists for, applied to the mixer's own compound per-decision estimate
/// rather than a lone frequency counter (`research/JOURNAL.md` S2-R1's
/// mechanism reading).
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
pub fn encode_symbol_sse(encoder: &mut Encoder, cum: &[u64], symbol: u8, sse: &mut Sse) {
    let symbol_index = usize::from(symbol);
    let landed = walk_sse(cum, sse, |mid, refined_p| {
        let bit = symbol_index >= mid;
        encoder.encode_bit(bit, refined_p);
        bit
    });
    debug_assert_eq!(
        landed, symbol,
        "8 halvings of [0, 256) must land exactly on symbol"
    );
}

/// Decodes one byte from `decoder` as `LEVELS` chained binary decisions
/// over `cum`, the exact inverse of [`encode_symbol_sse`].
///
/// Never panics on adversarial `decoder` state: [`Decoder::decode_bit`] is
/// total over any coded bit pattern, same as [`decode_symbol`].
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`. `cum` and `sse` are both
/// caller-supplied local state, never derived from `decoder`'s bytes.
#[must_use]
pub fn decode_symbol_sse(decoder: &mut Decoder, cum: &[u64], sse: &mut Sse) -> u8 {
    walk_sse(cum, sse, |_mid, refined_p| decoder.decode_bit(refined_p))
}

/// One bit-tree node's ideal-cost contribution: `-log2(p)` if the node
/// resolved to `bit`, `-log2(1 - p)` otherwise. Shared by every ideal-cost
/// accounting path in the crate ([`ideal_cost_bits`], [`ideal_cost_bits_sse`],
/// and [`crate::literal::Literal`]'s logistic-mix pricing) so the formula
/// carries one exemption from `clippy.toml`'s `disallowed-methods` instead
/// of one per call site.
#[allow(
    clippy::disallowed_methods,
    reason = "ideal-cost accounting never drives an Encoder or Decoder, so no bitstream depends \
              on libm's last-ulp behavior here (ADR-0006, ADR-0024's determinism rule doesn't \
              apply off the coding path)"
)]
pub(crate) fn ideal_cost_bit(bit: bool, p: f64) -> f64 {
    -(if bit { p.log2() } else { (1.0 - p).log2() })
}

/// Sum of the ideal (`-log2`) cost of each of the `LEVELS` binary
/// decisions [`encode_symbol`] would pay coding `symbol` under `cum`,
/// without driving a coder — the chain-rule identity the module docs
/// name, checked directly. Equal to `-log2((cum[symbol + 1] -
/// cum[symbol]) / cum[ALPHABET])` up to floating-point rounding (proven
/// by test, not just asserted), [`crate::literal::Literal::ideal_cost_bits`]'s
/// counterpart for this decomposition.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
#[must_use]
pub fn ideal_cost_bits(cum: &[u64], symbol: u8) -> f64 {
    let symbol_index = usize::from(symbol);
    let mut bits = 0.0f64;
    walk(cum, |mid, p| {
        let bit = symbol_index >= mid;
        bits += ideal_cost_bit(bit, p);
        bit
    });
    bits
}

/// [`ideal_cost_bits`]'s counterpart for [`encode_symbol_sse`]: sums the
/// ideal (`-log2`) cost of each `sse`-refined binary decision
/// [`encode_symbol_sse`] would pay coding `symbol` under `cum`, without
/// driving a coder, updating `sse` on the raw probability exactly as
/// [`encode_symbol_sse`] does — so a caller pricing a whole stream this way
/// leaves `sse` in the same state a real `encode_symbol_sse` pass would
/// have, and later prices reflect that adaptation.
/// [`crate::literal::Literal::ideal_cost_bits_sse`]'s counterpart for this
/// decomposition.
///
/// # Panics
///
/// Panics if `cum` is not shaped like a 257-entry cumulative table over
/// `ALPHABET` symbols; see `check_table_shape`.
#[must_use]
pub fn ideal_cost_bits_sse(cum: &[u64], symbol: u8, sse: &mut Sse) -> f64 {
    let symbol_index = usize::from(symbol);
    let mut bits = 0.0f64;
    walk_sse(cum, sse, |mid, refined_p| {
        let bit = symbol_index >= mid;
        bits += ideal_cost_bit(bit, refined_p);
        bit
    });
    bits
}

#[cfg(test)]
mod tests;
