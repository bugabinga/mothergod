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
//! [`stretch`] and [`squash`] are table lookups, lpaq1's, built once per
//! model from `crate::logistic`'s series (`research/JOURNAL.md` S2-A160);
//! the series cost S2-A159's walk most of its time.
//!
//! No frame this build writes codes through it; `mothergod_bench` prices
//! [`BitMix::walk`] against [`super::Literal::walk_logit_sse`] on the
//! literal stream. Every value goes through IEEE basic operations and
//! tables built from [`crate::logistic`]'s vendored [`stretch`] and
//! [`squash`], never libm (ADR-0024), so the walk an encoder and a decoder
//! run is bit-identical.

use super::{
    ALIGN_BANKS, Context, WEIGHT_CONTEXTS, align_key, clamp_probability, logistic_gradient_step,
    weight_index,
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

/// Entries of the stretch table's `[0, 1]` domain, lpaq1's 12 bits.
const STRETCH_STEPS: u32 = 4096;

/// [`stretch`] of `i / STRETCH_STEPS`, clamped like every coded
/// probability.
fn fresh_stretch_table() -> Vec<f32> {
    (0..=STRETCH_STEPS)
        .map(|i| {
            let p = f64::from(i) / f64::from(STRETCH_STEPS);
            #[allow(
                clippy::cast_possible_truncation,
                reason = "narrowing a stretch to f32"
            )]
            let value = stretch(clamp_probability(p)) as f32;
            value
        })
        .collect()
}

/// Half the width of the squash table's domain: past it the coded
/// probability sits on its clamp.
const SQUASH_RANGE: f64 = 9.0;

/// Squash table entries per unit of stretch.
const SQUASH_STEPS_PER_UNIT: f64 = 32.0;

/// Entries of the squash table.
const SQUASH_ENTRIES: usize = 577;

/// [`squash`] at each table point.
fn fresh_squash_table() -> Vec<f64> {
    (0..SQUASH_ENTRIES)
        .map(|i| {
            #[allow(clippy::cast_precision_loss, reason = "i is at most 576")]
            let x = i as f64 / SQUASH_STEPS_PER_UNIT - SQUASH_RANGE;
            squash(x)
        })
        .collect()
}

/// [`squash`] of `x` by linear interpolation in `table`.
///
/// `#[inline]` here and on [`apm_position`] and [`table_stretch`]: a
/// builder of [`BitMix::walk`] in another crate sees a non-generic helper
/// as a call, and this walk makes each of them per bit
/// (`research/JOURNAL.md` S2-A164).
#[inline]
fn table_squash(table: &[f64], x: f64) -> f64 {
    let scaled =
        (x.clamp(-SQUASH_RANGE, SQUASH_RANGE - 1e-9) + SQUASH_RANGE) * SQUASH_STEPS_PER_UNIT;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "scaled is in [0, SQUASH_ENTRIES - 1), truncation is the floor"
    )]
    let lower = scaled as usize;
    #[allow(clippy::cast_precision_loss, reason = "lower is below 577")]
    let fraction = scaled - lower as f64;
    table[lower] * (1.0 - fraction) + table[lower + 1] * fraction
}

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
    /// [`stretch`] at each multiple of `1 / STRETCH_STEPS`.
    stretch_table: Vec<f32>,
    /// [`squash`] at each multiple of `1 / SQUASH_STEPS_PER_UNIT` over
    /// `[-SQUASH_RANGE, SQUASH_RANGE]`.
    squash_table: Vec<f64>,
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
#[inline]
fn apm_position(table: &[f32], p: f64) -> (usize, f64) {
    let held = table_stretch(table, p).clamp(-APM_HELD, APM_HELD);
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

/// [`stretch`] of `p` read from `table`, `p` rounded to a multiple of
/// `1 / STRETCH_STEPS`. A `p` outside `[0, 1]` reads the nearest end: the
/// float-to-integer cast saturates at zero, and `min` bounds the top (a
/// float clamp here cost about a tenth of the walk).
#[inline]
fn table_stretch(table: &[f32], p: f64) -> f64 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the cast saturates at zero and truncation is the rounding floor"
    )]
    let index = (p * f64::from(STRETCH_STEPS) + 0.5) as usize;
    f64::from(table[index.min(STRETCH_STEPS as usize)])
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
            stretch_table: fresh_stretch_table(),
            squash_table: fresh_squash_table(),
        })
    }

    /// One byte's walk under `context`, the shape of
    /// [`super::Literal::walk_logit_sse`]: `code_bit` receives each node's
    /// midpoint and the probability that the node's bit is `1` and returns
    /// the bit the node resolved to; every table learns that bit before
    /// the next node. Returns the landed byte.
    ///
    /// The coded probability lies in
    /// [`clamp_probability`]'s range, so a node costs at most 12
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
            let table = &self.stretch_table;
            let stretched = [
                table_stretch(table, self.order1.p(slots[0])),
                table_stretch(table, self.back2.p(slots[1])),
                table_stretch(table, self.align.p(slots[2])),
                BIAS,
            ];
            let set = depth as usize * WEIGHT_CONTEXTS + key;
            let weights = &mut self.weights[set];
            let mut dot = 0.0;
            for (w, s) in weights.iter().zip(&stretched) {
                dot += w * s;
            }
            let p = clamp_probability(table_squash(&self.squash_table, dot));

            let row = &mut self.apm[node];
            let (lower, fraction) = apm_position(&self.stretch_table, p);
            let calibrated = row[lower] * (1.0 - fraction) + row[lower + 1] * fraction;
            // The APM carries three quarters of the coded probability,
            // lpaq1's blend. Plain operations, not `mul_add`, which is a
            // libm call on baseline x86-64 (issue #1979).
            let coded = clamp_probability((p + 3.0 * calibrated) / 4.0);
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
