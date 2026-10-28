//! Binary decomposition of a 256-symbol cumulative-frequency table into 8
//! top-down split decisions ("bit tree"), the shape [`crate::sse::Sse`]
//! calibrates (`research/JOURNAL.md` S1-P1).
//!
//! Each step splits the candidate range `[lo, hi)` at its midpoint and codes
//! whether the symbol lies in the upper half, at the probability `cum` gives
//! that split. By the chain rule the 8 probabilities along a symbol's path
//! multiply to `(cum[symbol + 1] - cum[symbol]) / cum[ALPHABET]`: the same
//! partition as [`crate::coder::Encoder::encode`]'s one 256-way division.
//! The round-trip tests check the identity through the real coder.
//!
//! Each level's probability is refined through a [`crate::sse::Sse`] keyed by
//! [`sse_context`], and the `Sse` is updated on the raw probability;
//! `crate::literal::Literal::encode_column`/`decode_column` are the callers
//! (`research/JOURNAL.md` S2-A60, ADR-0038).

use crate::coder::{Decoder, Encoder};
use crate::sse::Sse;

/// Byte alphabet a cumulative table spans, and
/// [`crate::literal::Literal`]'s own alphabet size.
pub(crate) const ALPHABET: usize = 256;

/// `log2(ALPHABET)`: number of binary decisions that pin down one
/// symbol out of 256.
const LEVELS: u32 = 8;

/// One context per internal node of the depth-`LEVELS` tree: `2^LEVELS - 1`.
/// `crate::sse::Sse::try_new`'s `contexts` argument for a table keyed by
/// [`sse_context`].
pub const SSE_CONTEXTS: usize = (1 << LEVELS) - 1;

/// Maps the walk step at tree depth `depth` (`0..LEVELS`, `0` the coarsest
/// split) having already decided `prefix` (the `depth` bits chosen so far,
/// `0..2^depth`) to a unique index in `0..SSE_CONTEXTS`.
///
/// Keyed on tree position alone: folding two nodes together loses what the
/// walk observed, and nothing finer exists, since the symbol is exactly what
/// is still undecided. LZMA's literal coder numbers its nodes the same way
/// (`probs[(1 << depth) | prefix]`, 1-indexed there).
///
/// # Panics
///
/// Panics if `depth >= LEVELS` or `prefix >= 1 << depth`. The module's own walk
/// derives both, never adversarial input.
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

/// Panics unless `cum` is a cumulative-frequency table over `ALPHABET`
/// symbols: `ALPHABET + 1` entries, strictly increasing so every symbol has
/// positive mass. A caller-code invariant, never adversarial input:
/// [`crate::model::Model`] and `crate::literal::Literal::mix` Laplace-floor
/// every symbol to mass 1.
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

/// Probability the symbol lies in the upper half of `[lo, hi)`:
/// `(cum[hi] - cum[mid]) / (cum[hi] - cum[lo])`. Never zero or one:
/// `check_table_shape` keeps both halves' mass positive, and `hi - lo` halves
/// from 256 to 1 without reaching 0.
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

/// The tree traversal under [`walk_sse`], with no cumulative table: hands
/// `node` each level's `depth`, `prefix` (`lo / width`) and `[lo, mid, hi)`
/// split, and follows the bit it returns. A caller pricing
/// from its own probabilities ([`crate::literal::SurpriseLogisticMix`],
/// `research/JOURNAL.md` S2-A101) walks the identical tree the coder does.
///
/// Returns the symbol the decisions resolved to.
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

/// [`walk_nodes`] over `cum`, with each level's raw probability refined
/// through `sse` (keyed by [`sse_context`]) before `code_bit` sees it, and
/// `sse` updated on the raw probability afterward. Shared by [`encode_symbol_sse`],
/// [`decode_symbol_sse`] and [`ideal_cost_bits_sse`], so encode, decode and
/// pricing cannot drift apart.
///
/// Panics if `cum` fails `check_table_shape`.
fn walk_sse(cum: &[u64], sse: &mut Sse, mut code_bit: impl FnMut(usize, f64) -> bool) -> u8 {
    check_table_shape(cum);
    walk_nodes(|depth, prefix, lo, mid, hi| {
        let raw_p = upper_half_probability(cum, lo, hi);
        let context = sse_context(depth, prefix);
        let refined_p = sse.refine(context, raw_p);
        let bit = code_bit(mid, refined_p);
        sse.update(context, raw_p, bit);
        bit
    })
}

/// Codes `symbol` through `encoder` as `LEVELS` binary decisions over `cum`,
/// each probability refined through `sse`, which is then updated on the raw
/// probability: the calibration S1-P1 exists for, applied to the mixer's
/// compound estimate (`research/JOURNAL.md` S2-R1).
///
/// # Panics
///
/// Panics if `cum` fails `check_table_shape`.
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

/// Decodes one byte from `decoder`, the exact inverse of
/// [`encode_symbol_sse`].
///
/// Never panics on adversarial `decoder` state: [`Decoder::decode_bit`] is
/// total over any coded bit pattern.
///
/// # Panics
///
/// Panics if `cum` fails `check_table_shape`. `cum` and `sse` are caller
/// state, never derived from `decoder`'s bytes.
#[must_use]
pub fn decode_symbol_sse(decoder: &mut Decoder, cum: &[u64], sse: &mut Sse) -> u8 {
    walk_sse(cum, sse, |_mid, refined_p| decoder.decode_bit(refined_p))
}

/// One node's ideal cost: `-log2(p)` if it resolved to `bit`, `-log2(1 - p)`
/// otherwise. [`ideal_cost_bits_sse`] sums it over a symbol's nodes.
pub(crate) fn ideal_cost_bit(bit: bool, p: f64) -> f64 {
    crate::surprisal(if bit { p } else { 1.0 - p })
}

/// Prices what [`encode_symbol_sse`] would pay for `symbol`: each
/// `sse`-refined decision without driving a coder, updating `sse` exactly as
/// [`encode_symbol_sse`] does, so later prices see the same adaptation a real
/// pass would leave.
///
/// # Panics
///
/// Panics if `cum` fails `check_table_shape`.
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
