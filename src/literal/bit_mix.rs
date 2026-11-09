//! Charter #1922's literal coder class (b), unwired: three bit-level
//! experts mixed in the logit domain, lpaq1's shape cut to three small
//! contexts and one APM (`research/JOURNAL.md` S2-A159).
//!
//! ```text
//!   order 1: previous byte, node ─────────────────── p ─┐ stretch; weighted
//!   byte two back, node ──────────────────────────── p ─┤ sum with the bias,
//!   alignment: position phase, high nibble, node ─── p ─┤ weights selected by
//!                                         bias 1/4 ─────┘ depth and weight key;
//!                                                         squash ─> p
//!   p ─ APM by node ─ a ─> (p + 3a) / 4 ─> coder
//! ```
//!
//! Each expert is an adaptive probability per bit-tree node of its key,
//! lpaq1's `StateMap` rule, no hashing: every key is a byte or less, so the
//! three tables hold 576 keys of 255 nodes. Per byte the walk reads eight
//! slots per expert and builds no per-symbol table. Its per-bit
//! [`stretch`] and [`squash`] are still `crate::logistic`'s series, so it
//! does not yet fit M9's literal budget (`research/JOURNAL.md` S2-A159).
//!
//! No frame this build writes codes through it; `mothergod_bench` prices
//! [`BitMix::walk`] against [`super::Literal::walk_logit_sse`] on the
//! literal stream. Every value goes through IEEE basic operations and
//! [`crate::logistic`]'s vendored [`stretch`] and [`squash`], never libm
//! (ADR-0024), so the walk an encoder and a decoder run is bit-identical.

use super::{
    ALIGN_BANKS, Context, WEIGHT_CONTEXTS, align_key, clamp_logistic_probability,
    logistic_gradient_step, weight_index,
};
use crate::bittree::{self, SSE_CONTEXTS};
use crate::cm_path::Counter;
use crate::logistic::{squash, stretch};

/// The experts, in mixer order: order 1, the byte two back, alignment.
const EXPERTS: usize = 3;

/// Mixer inputs: each expert's stretch, then [`BIAS`].
const INPUTS: usize = EXPERTS + 1;

/// The mixer's constant input, so each weight set learns an offset.
const BIAS: f64 = 0.25;

/// Every weight's starting value, the bias's included: 0.3 and 0.5
/// priced higher on train (`research/JOURNAL.md` S2-A159).
const INITIAL_WEIGHT: f64 = 0.2;

/// The mixer's gradient step: 0.01 and 0.02 priced higher on train.
const MIXER_RATE: f64 = 0.04;

/// Bit-tree depths, the first half of a weight set's key.
const DEPTHS: usize = 8;

/// Keys of the two byte-keyed experts.
const BYTE_KEYS: usize = 256;

/// Bins of the APM, over stretch `[-APM_RANGE, APM_RANGE]` at a spacing of
/// `1 / APM_STEPS_PER_UNIT`, lpaq1's shape.
const APM_BINS: usize = 33;

/// Half the width of the APM's stretch domain.
const APM_RANGE: f64 = 8.0;

/// APM bins per unit of stretch.
const APM_STEPS_PER_UNIT: f64 = 2.0;

/// The largest stretch magnitude the APM reads, just inside
/// [`APM_RANGE`], so the bin pair it interpolates between stays inside the
/// row.
const APM_HELD: f64 = 7.999;

/// The APM's update step, each bin moved by its interpolation weight times
/// this: 0.02 and 0.04 priced higher on train.
const APM_RATE: f64 = 0.01;

/// Charter #1922's class (b) literal model, one per literal stream.
#[derive(Debug, Clone)]
pub struct BitMix {
    /// Order 1: a slot per previous byte and node.
    order1: Counter,
    /// A slot per byte two back and node, skipping the previous byte.
    back2: Counter,
    /// A slot per [`align_key`] and node.
    align: Counter,
    /// One weight vector per depth and [`weight_index`] key.
    weights: Vec<[f64; INPUTS]>,
    /// One bin row per node, each bin a probability.
    apm: Vec<[f64; APM_BINS]>,
}

impl Default for BitMix {
    fn default() -> Self {
        Self::new()
    }
}

/// A fresh APM row: each bin holds the [`squash`] of its own stretch, so
/// a fresh table maps a probability to itself within its interpolation
/// error.
fn fresh_apm_row() -> [f64; APM_BINS] {
    let mut row = [0.0; APM_BINS];
    for (bin, value) in (0u32..).zip(&mut row) {
        *value = squash(f64::from(bin) / APM_STEPS_PER_UNIT - APM_RANGE);
    }
    row
}

/// `p`'s lower APM bin and the upper bin's interpolation weight, in
/// `[0, 1)`: the lower bin is at most `APM_BINS - 2`, so both read inside
/// the row.
fn apm_position(p: f64) -> (usize, f64) {
    let held = stretch(p).clamp(-APM_HELD, APM_HELD);
    let scaled = (held + APM_RANGE) * APM_STEPS_PER_UNIT;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "scaled is in (0, APM_BINS - 1), so truncation is the floor and fits usize"
    )]
    let lower = scaled as usize;
    #[allow(
        clippy::cast_precision_loss,
        reason = "lower is below APM_BINS, exact in f64"
    )]
    let fraction = scaled - lower as f64;
    (lower, fraction)
}

impl BitMix {
    /// A fresh model: every slot at one half and count zero, every weight
    /// at [`INITIAL_WEIGHT`], every APM row the identity.
    #[must_use]
    pub fn new() -> Self {
        crate::abort_on_oom(Self::try_new())
    }

    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            order1: Counter::try_new(BYTE_KEYS * SSE_CONTEXTS)?,
            back2: Counter::try_new(BYTE_KEYS * SSE_CONTEXTS)?,
            align: Counter::try_new(ALIGN_BANKS * SSE_CONTEXTS)?,
            weights: crate::try_filled_vec(DEPTHS * WEIGHT_CONTEXTS, [INITIAL_WEIGHT; INPUTS])?,
            apm: crate::try_filled_vec(SSE_CONTEXTS, fresh_apm_row())?,
        })
    }

    /// One byte's walk under `context`, the shape of
    /// [`super::Literal::walk_logit_sse`]: `code_bit` receives each node's
    /// midpoint and the probability that the node's bit is `1` and returns
    /// the bit the node resolved to; every table learns that bit before
    /// the next node. Returns the landed byte.
    ///
    /// The coded probability lies in
    /// [`clamp_logistic_probability`]'s range, so a node costs at most 12
    /// bits.
    pub fn walk(&mut self, context: Context, mut code_bit: impl FnMut(usize, f64) -> bool) -> u8 {
        let bases = [
            usize::from(context.prev1) * SSE_CONTEXTS,
            usize::from(context.prev2) * SSE_CONTEXTS,
            align_key(context) * SSE_CONTEXTS,
        ];
        let key = weight_index(context);
        bittree::walk_nodes(|depth, prefix, _lo, mid, _hi| {
            let node = bittree::sse_context(depth, prefix);
            let slots = bases.map(|base| base + node);
            let stretched = [
                stretch(clamp_logistic_probability(self.order1.p(slots[0]))),
                stretch(clamp_logistic_probability(self.back2.p(slots[1]))),
                stretch(clamp_logistic_probability(self.align.p(slots[2]))),
                BIAS,
            ];
            let set = depth as usize * WEIGHT_CONTEXTS + key;
            let weights = &mut self.weights[set];
            let mut dot = 0.0;
            for (w, s) in weights.iter().zip(&stretched) {
                dot += w * s;
            }
            let p = clamp_logistic_probability(squash(dot));

            let row = &mut self.apm[node];
            let (lower, fraction) = apm_position(p);
            let calibrated = row[lower] * (1.0 - fraction) + row[lower + 1] * fraction;
            // The APM carries three quarters of the coded probability,
            // lpaq1's blend. Plain operations, not `mul_add`, which is a
            // libm call on baseline x86-64 (issue #1979).
            let coded = clamp_logistic_probability((p + 3.0 * calibrated) / 4.0);
            let bit = code_bit(mid, coded);

            let target = f64::from(u8::from(bit));
            row[lower] += APM_RATE * (1.0 - fraction) * (target - row[lower]);
            row[lower + 1] += APM_RATE * fraction * (target - row[lower + 1]);
            logistic_gradient_step(weights, &stretched, MIXER_RATE, bit, p);
            self.order1.update(slots[0], bit);
            self.back2.update(slots[1], bit);
            self.align.update(slots[2], bit);
            bit
        })
    }
}

#[cfg(test)]
mod tests;
