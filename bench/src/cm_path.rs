//! Charter #1763's candidate: a context-mixing path over every byte, no
//! LZ tokens, priced by [`crate::whole_stream`] against the hybrid's real
//! bitstream (`research/JOURNAL.md` S2-A133, ADR-0077).
//!
//! Per bit-tree node of each byte, an outer logistic mixer blends:
//!
//! ```text
//!   Literal (six experts, SurpriseLogisticMix, LogitSse) ─ p ─┐
//!   match model: predicted byte, confirmed length ───────── p ─┤ stretch,
//!   hashed order-3 context, per node ────────────────────── p ─┤ weighted sum,
//!   hashed order-4 ─────────────────────────────────────── p ─┤ squash ─> p
//!   hashed order-5 ─────────────────────────────────────── p ─┤
//!   hashed order-6 ─────────────────────────────────────── p ─┘
//! ```
//!
//! The first input is the shipped wire model's own refined probability,
//! [`Literal::walk_logit_sse`], and a fresh mixer weighs it 1 and every
//! other input 0, so the candidate starts as the wire model and the mixer
//! only adds what it learns pays.
//!
//! Unwired: nothing here reaches a bitstream. Every value a decoder would
//! recompute goes through the codec's own [`logistic::stretch`] and
//! [`logistic::squash`] (ADR-0024, S2-A134); `cost_bits` stays libm
//! `log2` (`bench/clippy.toml`) because it prices a decision, and a frame
//! codes that decision instead.

use crate::literal_pairing::PairingReport;
use crate::whole_stream;
use mothergod::literal::{Context, Literal, SurpriseLogisticMix};
use mothergod::logistic;

/// Every probability the outer mixer stretches or prices is clamped to
/// `[PROBABILITY_FLOOR, 1 - PROBABILITY_FLOOR]`, bounding a stretch to
/// `ln(4095)` and a node's cost to 12 bits.
const PROBABILITY_FLOOR: f64 = 1.0 / 4096.0;

/// The outer mixer's gradient step.
const MIXER_RATE: f64 = 0.005;

/// A [`Counter`] slot's rate is `1 / (count + 1.5)`, its count capped
/// here, so a slot's adaptation slows to this floor and no further.
const COUNTER_LIMIT: u16 = 1023;

/// The hashed context orders, each one input, in mixer order.
const ORDERS: [usize; 4] = [3, 4, 5, 6];

/// [`Bucket`]s per order table, as a power of two. A 50,000-byte case
/// makes at most 100,000 nibble contexts per order, so collisions stay
/// rare at the case size every recorded number used; a [`Bucket`]'s check
/// detects one and resets the bucket instead of sharing it.
const ORDER_BUCKET_BITS: u32 = 18;

/// Bit-tree nodes per nibble, numbered `1..NIBBLE_NODES` from the
/// nibble's root, `node * 2 + bit` per step, as the byte's tree is; so
/// also the byte's node id at which the low nibble starts.
const NIBBLE_NODES: usize = 16;

/// Bytes the match model hashes to find a candidate, and the shortest
/// verified match it follows.
const MATCH_MIN_LEN: usize = 6;

/// Slots in the match model's hash table, as a power of two.
const MATCH_TABLE_BITS: u32 = 20;

/// The longest a lookup verifies a candidate backward; a match found
/// longer grows past it one byte at a time.
const MATCH_VERIFY_LIMIT: usize = 400;

/// Values [`length_bucket`] returns.
const LENGTH_BUCKETS: usize = 21;

/// Weight sets in the outer mixer: one per length bucket, split by
/// whether the node is in the byte's high or low nibble.
const WEIGHT_SETS: usize = LENGTH_BUCKETS * 2;

/// Inputs to the outer mixer: the wire model, the match model, the orders.
const INPUTS: usize = 2 + ORDERS.len();

fn clamp(p: f64) -> f64 {
    p.clamp(PROBABILITY_FLOOR, 1.0 - PROBABILITY_FLOOR)
}

/// [`logistic::stretch`] of `p` clamped, so its odds ratio stays normal,
/// the precondition [`logistic::ln`] states.
fn stretch(p: f64) -> f64 {
    logistic::stretch(clamp(p))
}

/// Bits a decision costs: `bit` coded at `probability_of_one`, clamped.
fn cost_bits(bit: bool, probability_of_one: f64) -> f64 {
    let p = clamp(probability_of_one);
    -(if bit { p } else { 1.0 - p }).log2()
}

/// The `SplitMix64` finalizer: the one hash every table index goes through.
fn mix64(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// `mix64` folded over `bytes`, starting from `seed`.
fn hash_bytes(seed: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(mix64(seed), |h, &b| mix64(h ^ u64::from(b)))
}

/// `h` reduced to a table index under `bits`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "masked to at most 20 bits first, which fits usize on every target"
)]
fn slot(h: u64, bits: u32) -> usize {
    (h & ((1u64 << bits) - 1)) as usize
}

/// One adaptive bit probability with a count-scaled rate, lpaq1's
/// `StateMap` rule: it moves `1 / (count + 1.5)` of the way to each bit it
/// sees, so it learns fast when fresh and settles as it fills.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a probability in [0, 1] narrows to f32 by rounding only"
)]
fn adapt(probability: &mut f32, count: &mut u16, bit: bool) {
    let p = f64::from(*probability);
    let target = f64::from(u8::from(bit));
    *probability = (p + (target - p) / (f64::from(*count) + 1.5)) as f32;
    if *count < COUNTER_LIMIT {
        *count += 1;
    }
}

/// A table of [`adapt`]ed bit probabilities, one per slot.
#[derive(Debug, Clone)]
struct Counter {
    probability: Vec<f32>,
    count: Vec<u16>,
}

impl Counter {
    fn new(slots: usize) -> Self {
        Self {
            probability: vec![0.5; slots],
            count: vec![0; slots],
        }
    }

    fn p(&self, slot: usize) -> f64 {
        f64::from(self.probability[slot])
    }

    fn update(&mut self, slot: usize, bit: bool) {
        adapt(&mut self.probability[slot], &mut self.count[slot], bit);
    }
}

/// One order's [`adapt`]ed probabilities for one nibble context, a slot
/// per node `1..NIBBLE_NODES`, lpaq1's layout: a nibble's nodes share 96
/// bytes at a 32-byte alignment, so exactly two cache lines, and an order
/// costs one table miss per nibble instead of one per node
/// (`research/JOURNAL.md` S2-A136).
#[derive(Debug, Clone, Copy)]
#[repr(C, align(32))]
struct Bucket {
    /// The claiming context's [`check_of`]; a lookup under another check
    /// resets the bucket, so a collision costs the evicted context its
    /// history instead of sharing it with a stranger.
    check: u16,
    count: [u16; NIBBLE_NODES - 1],
    probability: [f32; NIBBLE_NODES - 1],
}

impl Bucket {
    const FRESH: Self = Self {
        check: 0,
        count: [0; NIBBLE_NODES - 1],
        probability: [0.5; NIBBLE_NODES - 1],
    };

    /// Claims the bucket for `check`: kept when `check` already owns it,
    /// reset to [`Bucket::FRESH`] otherwise.
    fn claim(&mut self, check: u16) {
        if self.check != check {
            *self = Self {
                check,
                ..Self::FRESH
            };
        }
    }
}

/// The bits of `h` a [`Bucket`] keeps as its check: the top 16, disjoint
/// from the low [`ORDER_BUCKET_BITS`] that index it.
fn check_of(h: u64) -> u16 {
    u16::try_from(h >> 48).expect("a u64 shifted right 48 fits u16")
}

/// Bucket of a confirmed match length: `0` is no match, `1..=15` exact,
/// then one bucket per doubling up to 256 and one past it.
fn length_bucket(length: usize) -> usize {
    match length {
        0..=15 => length,
        16..=31 => 16,
        32..=63 => 17,
        64..=127 => 18,
        128..=255 => 19,
        _ => 20,
    }
}

/// lpaq1's match model over the reconstructed stream: the most recent
/// earlier position whose `MATCH_MIN_LEN` bytes equal the current ones,
/// verified byte by byte (a hash collision never predicts), followed while
/// its predictions hold.
///
/// Invariant: `length > 0` implies `pointer < position` of the byte being
/// predicted, so a prediction only ever reads the prefix.
#[derive(Debug, Clone)]
struct MatchModel {
    /// Position just after each hashed `MATCH_MIN_LEN`-byte context; `0`
    /// is empty, which no stored position can be.
    table: Vec<u32>,
    /// Position of the predicted byte, valid while `length > 0`.
    pointer: usize,
    /// Bytes confirmed equal before `pointer`.
    length: usize,
    /// Probability that the node's bit is the predicted byte's, per
    /// `(length_bucket, predicted bit)`.
    confidence: Counter,
}

impl MatchModel {
    fn new() -> Self {
        Self {
            table: vec![0; 1 << MATCH_TABLE_BITS],
            pointer: 0,
            length: 0,
            confidence: Counter::new(LENGTH_BUCKETS * 2),
        }
    }

    /// The byte at `prefix.len()` this model predicts and the match length
    /// behind it, after folding in `prefix`'s last byte; `None` when no
    /// verified match is live.
    fn predict(&mut self, prefix: &[u8]) -> Option<(u8, usize)> {
        let position = prefix.len();
        if self.length > 0 {
            if prefix[self.pointer] == prefix[position - 1] {
                self.length += 1;
                self.pointer += 1;
            } else {
                self.length = 0;
            }
        }
        if position >= MATCH_MIN_LEN {
            let h = hash_bytes(0, &prefix[position - MATCH_MIN_LEN..]);
            let entry = slot(h, MATCH_TABLE_BITS);
            let candidate = self.table[entry] as usize;
            if self.length == 0 && candidate > 0 {
                let verified = (0..candidate.min(MATCH_VERIFY_LIMIT))
                    .take_while(|&back| prefix[candidate - 1 - back] == prefix[position - 1 - back])
                    .count();
                if verified >= MATCH_MIN_LEN {
                    self.length = verified;
                    self.pointer = candidate;
                }
            }
            self.table[entry] = u32::try_from(position).expect("a priced case is far below 4 GiB");
        }
        (self.length > 0).then(|| (prefix[self.pointer], self.length))
    }
}

/// The candidate: [`Literal`]'s wire walk plus the match model and the
/// hashed orders under one outer logistic mixer. Fresh per case.
#[derive(Debug, Clone)]
pub struct CmPath {
    literal: Literal,
    literal_mixer: SurpriseLogisticMix,
    match_model: MatchModel,
    orders: Vec<Vec<Bucket>>,
    weights: Vec<[f64; INPUTS]>,
}

impl Default for CmPath {
    fn default() -> Self {
        Self::new()
    }
}

impl CmPath {
    /// A fresh candidate: every counter at one half, the outer mixer
    /// weighing the wire model 1 and every other input 0.
    #[must_use]
    pub fn new() -> Self {
        let mut start = [0.0; INPUTS];
        start[0] = 1.0;
        Self {
            literal: Literal::new(),
            literal_mixer: SurpriseLogisticMix::new(),
            match_model: MatchModel::new(),
            orders: ORDERS
                .iter()
                .map(|_| vec![Bucket::FRESH; 1 << ORDER_BUCKET_BITS])
                .collect(),
            weights: vec![start; WEIGHT_SETS],
        }
    }

    /// Bits `byte` costs after `prefix` under `context`, then every part
    /// of the model learns `byte`: the shape
    /// [`whole_stream::whole_stream_bits`] hands a candidate.
    pub fn price(&mut self, context: Context, prefix: &[u8], byte: u8) -> f64 {
        let symbol = usize::from(byte);
        let predicted = self.match_model.predict(prefix);
        let order_hashes = ORDERS
            .map(|order| hash_bytes(order as u64, &prefix[prefix.len().saturating_sub(order)..]));
        let Self {
            literal,
            literal_mixer,
            match_model,
            orders,
            weights,
        } = self;

        let mut bits = 0.0;
        let (mut lo, mut hi, mut node) = (0usize, 256usize, 1usize);
        let mut nibble_node = 1usize;
        let mut order_buckets = [0usize; ORDERS.len()];
        literal.walk_logit_sse(context, literal_mixer, |mid, literal_p| {
            let bit = symbol >= mid;
            let mut inputs = [0.0; INPUTS];
            inputs[0] = stretch(literal_p);

            // The match input speaks only while the predicted byte is still
            // inside this node's range, so a diverged prediction is silent.
            let mut bucket = 0;
            let mut confidence_slot = None;
            if let Some((predicted_byte, length)) = predicted {
                let predicted_symbol = usize::from(predicted_byte);
                if (lo..hi).contains(&predicted_symbol) {
                    bucket = length_bucket(length);
                    let slot = bucket * 2 + usize::from(predicted_symbol >= mid);
                    inputs[1] = stretch(match_model.confidence.p(slot));
                    confidence_slot = Some(slot);
                }
            }

            // A nibble starts at node 1 or at 16 to 31, which names the
            // high nibble, so the low nibble's buckets key on it.
            if nibble_node == 1 {
                let nibble_key = (node as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
                for ((index, &h), table) in order_buckets
                    .iter_mut()
                    .zip(&order_hashes)
                    .zip(&mut *orders)
                {
                    let h = mix64(h ^ nibble_key);
                    *index = slot(h, ORDER_BUCKET_BITS);
                    table[*index].claim(check_of(h));
                }
            }
            let i = nibble_node - 1;
            for ((input, table), &b) in inputs[2..].iter_mut().zip(&*orders).zip(&order_buckets) {
                *input = stretch(f64::from(table[b].probability[i]));
            }

            let set = bucket * 2 + usize::from(node >= NIBBLE_NODES);
            let w = &mut weights[set];
            let dot: f64 = w.iter().zip(&inputs).map(|(w, x)| w * x).sum();
            let p = logistic::squash(dot);
            bits += cost_bits(bit, p);

            let error = f64::from(u8::from(bit)) - clamp(p);
            for (w, x) in w.iter_mut().zip(&inputs) {
                *w += MIXER_RATE * error * x;
            }
            if let Some(s) = confidence_slot {
                match_model.confidence.update(s, bit);
            }
            for (table, &b) in orders.iter_mut().zip(&order_buckets) {
                let entry = &mut table[b];
                adapt(&mut entry.probability[i], &mut entry.count[i], bit);
            }

            if bit {
                lo = mid;
            } else {
                hi = mid;
            }
            node = node * 2 + usize::from(bit);
            nibble_node = nibble_node * 2 + usize::from(bit);
            if nibble_node >= NIBBLE_NODES {
                nibble_node = 1;
            }
            bit
        });
        bits
    }
}

/// The recorded experiment: a fresh [`CmPath`] per case over the 11
/// `baseline::cases()` and both sealed kinds, against `compress`'s real
/// bitstream (`research/JOURNAL.md` S2-A133).
#[must_use]
pub fn train_and_sealed_vs_real_bitstream() -> PairingReport {
    whole_stream::train_and_sealed_vs_real_bitstream(|| {
        let mut candidate = CmPath::new();
        move |context, prefix, byte| candidate.price(context, prefix, byte)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prices `data` through `model`, byte by byte, as the driver does.
    fn price_all(model: &mut CmPath, data: &[u8]) -> Vec<f64> {
        let mut context = Context::default();
        data.iter()
            .enumerate()
            .map(|(position, &byte)| {
                let bits = model.price(context, &data[..position], byte);
                context = context.after_literal(byte);
                bits
            })
            .collect()
    }

    #[test]
    fn a_fresh_candidate_prices_the_first_byte_as_the_wire_model_does() {
        let byte = b'q';
        let mut wire = Literal::new();
        let mut wire_mixer = SurpriseLogisticMix::new();
        let mut wire_bits = 0.0;
        wire.walk_logit_sse(Context::default(), &mut wire_mixer, |mid, p| {
            let bit = usize::from(byte) >= mid;
            wire_bits += cost_bits(bit, p);
            bit
        });

        let bits = CmPath::new().price(Context::default(), &[], byte);

        assert!(
            (bits - wire_bits).abs() < 1e-9,
            "weight 1 on the wire model, 0 elsewhere: {bits} vs {wire_bits}"
        );
    }

    #[test]
    fn every_byte_costs_finite_positive_bits_and_a_rerun_prices_identically() {
        let data: Vec<u8> = (0u32..3000)
            .map(|i| u8::try_from(i * 7 % 251).expect("below 251"))
            .collect();
        let first = price_all(&mut CmPath::new(), &data);
        let second = price_all(&mut CmPath::new(), &data);
        assert!(first.iter().all(|b| b.is_finite() && *b > 0.0));
        assert_eq!(first, second, "no state leaks between fresh candidates");
    }

    /// The match model is what a whole-stream path has that the wire model
    /// alone lacks: on a long verbatim repeat of random bytes, which no
    /// short context predicts, the second copy must cost far less than
    /// the first.
    #[test]
    fn a_verbatim_repeat_of_random_bytes_costs_a_fraction_of_its_first_copy() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let half: Vec<u8> = (0..4000)
            .map(|_| {
                state = mix64(state);
                state.to_le_bytes()[0]
            })
            .collect();
        let data = [half.as_slice(), half.as_slice()].concat();

        let bits = price_all(&mut CmPath::new(), &data);
        let first: f64 = bits[..half.len()].iter().sum();
        let second: f64 = bits[half.len()..].iter().sum();

        assert!(second < first / 8.0, "first {first}, second {second}");
    }

    #[test]
    fn the_match_model_predicts_only_from_a_verified_earlier_context() {
        let mut model = MatchModel::new();
        let data = b"abcdefgh--abcdefgX";
        let predictions: Vec<_> = (0..data.len()).map(|i| model.predict(&data[..i])).collect();

        assert!(
            predictions[..16].iter().all(Option::is_none),
            "{predictions:?}"
        );
        assert_eq!(
            predictions[16],
            Some((b'g', 6)),
            "abcdef recurs, g follows it"
        );
        assert_eq!(
            predictions[17],
            Some((b'h', 7)),
            "the hit extends the match"
        );
        assert_eq!(
            model.predict(data),
            None,
            "X broke the match, nothing recurs"
        );
    }

    #[test]
    fn length_buckets_are_monotone_and_in_range() {
        let buckets: Vec<usize> = (0..1000).map(length_bucket).collect();
        assert!(buckets.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(buckets[0], 0);
        assert_eq!(*buckets.last().unwrap(), LENGTH_BUCKETS - 1);
    }

    /// An input at or past either end of `[0, 1]` would hand
    /// [`logistic::ln`] a zero or infinite odds ratio; the clamp holds it
    /// to the floor's logit instead.
    #[test]
    fn stretch_of_a_certain_input_is_the_floors_finite_logit() {
        let bound = (1.0 / PROBABILITY_FLOOR - 1.0).ln();
        for (p, expected) in [(0.0, -bound), (-1.0, -bound), (1.0, bound), (2.0, bound)] {
            let x = stretch(p);
            assert!((x - expected).abs() < 1e-12, "stretch({p}) = {x}");
        }
        assert!(stretch(0.5).abs() < 1e-15);
    }

    /// 96 bytes at a 32-byte alignment start at offset 0 or 32 of a
    /// 64-byte line, so a bucket spans exactly two lines, the miss count
    /// [`Bucket`]'s doc promises.
    #[test]
    fn a_bucket_spans_exactly_two_cache_lines() {
        assert_eq!(std::mem::size_of::<Bucket>(), 96);
        assert_eq!(std::mem::align_of::<Bucket>(), 32);
    }

    #[test]
    fn a_bucket_keeps_its_owners_history_and_resets_for_another_check() {
        let mut bucket = Bucket::FRESH;
        bucket.claim(7);
        adapt(&mut bucket.probability[3], &mut bucket.count[3], true);
        let learned = bucket.probability[3];
        assert!(learned > 0.5);

        bucket.claim(7);
        assert_eq!(
            (bucket.probability[3], bucket.count[3]),
            (learned, 1),
            "same owner"
        );

        bucket.claim(8);
        assert_eq!(bucket.check, 8);
        assert_eq!(bucket.count, Bucket::FRESH.count, "a collision evicts");
        assert_eq!(bucket.probability, Bucket::FRESH.probability);
    }

    #[test]
    fn a_counter_learns_fast_when_fresh_and_its_count_stops_at_the_limit() {
        let mut counter = Counter::new(1);
        counter.update(0, true);
        let after_one = counter.p(0);
        assert!((after_one - (0.5 + 0.5 / 1.5)).abs() < 1e-6, "{after_one}");
        for _ in 0..2 * usize::from(COUNTER_LIMIT) {
            counter.update(0, true);
        }
        assert_eq!(counter.count[0], COUNTER_LIMIT);
        assert!(counter.p(0) > 0.99);
    }
}
