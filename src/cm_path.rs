//! A context-mixing path over every byte, no LZ tokens: charter #1763's
//! candidate (`research/JOURNAL.md` S2-A133 to S2-A138, ADR-0077), coded
//! through the real range coder.
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
//! other input 0, so the path starts as the wire model and the mixer only
//! adds what it learns pays.
//!
//! Unwired: no [`crate::Method`] names this path, so no frame carries it
//! (issue #1775). [`encode`] and [`decode`] are the payload's coded stream
//! alone, and `mothergod_bench::cm_path` prices through [`CmPath::walk`].
//! Every value both sides compute goes through IEEE basic operations and
//! the codec's own [`logistic::stretch`] and [`logistic::squash`], never
//! libm (ADR-0024, S2-A134).

use crate::coder::{Decoder, Encoder};
use crate::literal::{Context, Literal, SurpriseLogisticMix};
use crate::{Error, logistic, try_filled_vec};

/// The reciprocal of [`PROBABILITY_FLOOR`], and so the most bytes one
/// stream byte decodes to: [`decode`]'s bomb guard.
///
/// Invariant: every node codes at a probability inside the floor, so it
/// costs at least `-log2(1 - PROBABILITY_FLOOR)`, which exceeds
/// `PROBABILITY_FLOOR / ln 2` bits; the coder's 16-bit quantization
/// rounds `1 - PROBABILITY_FLOOR` exactly (`2^16` is a multiple of this
/// reciprocal), and its narrowing rounds a range of more than `2^30`, so
/// the `1 / ln 2` margin absorbs both. A byte is 8 nodes, more than
/// `8 / MAX_EXPANSION` bits, so `L` stream bytes hold fewer than
/// `MAX_EXPANSION * L` output bytes. The coder writes at most one bit
/// fewer than its cost, its interval starting at `2^32` and ending above
/// `2^30`, plus its final bit; the margin covers that bit past 1157
/// output bytes, and below them [`encode`]'s one byte at least does.
const MAX_EXPANSION: u32 = 4096;

/// Every probability the outer mixer stretches or codes is clamped to
/// `[PROBABILITY_FLOOR, 1 - PROBABILITY_FLOOR]`, bounding a stretch to
/// `ln(4095)` and a node's cost to 12 bits.
#[allow(
    clippy::cast_lossless,
    reason = "f64::from is not const; a u32 converts to f64 exactly"
)]
const PROBABILITY_FLOOR: f64 = 1.0 / MAX_EXPANSION as f64;

/// The outer mixer's gradient step.
const MIXER_RATE: f64 = 0.005;

/// A [`Counter`] slot's rate is `1 / (count + 1.5)`, its count capped
/// here, so a slot's adaptation slows to this floor and no further.
const COUNTER_LIMIT: u16 = 1023;

/// The hashed context orders, each one input, in mixer order.
const ORDERS: [usize; 4] = [3, 4, 5, 6];

/// [`Bucket`]s per order table, as a power of two, the size the train
/// mean picked at `CASE_LEN` (`research/JOURNAL.md` S2-A138). Under
/// [`claim`]'s replacement a smaller table forgets rarely seen contexts
/// sooner, which priced lower at that length. A size that grows with the
/// stream's length, one bucket per byte under a cap, was rejected on the
/// sealed set (S2-R39), so the size is the same for every input.
const ORDER_BUCKET_BITS: u32 = 16;

/// Buckets in the set a context may occupy: the aligned run of this many
/// holding its hashed index.
const ORDER_WAYS: usize = 4;

const _: () = assert!(
    ORDER_WAYS.is_power_of_two() && ORDER_WAYS <= 1 << ORDER_BUCKET_BITS,
    "a table splits into whole sets"
);

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

/// `MurmurHash3`'s `fmix64` finalizer: the one hash every table index goes
/// through.
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

/// One step of lpaq1's `StateMap` rule: `p` moves `1 / (count + 1.5)` of
/// the way to `bit`, so a probability learns fast when fresh and settles
/// as it fills. Returns the probability and the count after `bit`.
fn adapt(p: f64, count: u16, bit: bool) -> (f64, u16) {
    let target = f64::from(u8::from(bit));
    (
        p + (target - p) / (f64::from(count) + 1.5),
        (count + 1).min(COUNTER_LIMIT),
    )
}

/// A table of [`adapt`]ed bit probabilities, one per slot.
#[derive(Debug, Clone)]
struct Counter {
    probability: Vec<f32>,
    count: Vec<u16>,
}

impl Counter {
    fn try_new(slots: usize) -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            probability: try_filled_vec(slots, 0.5)?,
            count: try_filled_vec(slots, 0)?,
        })
    }

    fn p(&self, slot: usize) -> f64 {
        f64::from(self.probability[slot])
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "a probability in [0, 1] narrows to f32 by rounding only"
    )]
    fn update(&mut self, slot: usize, bit: bool) {
        let (p, count) = adapt(self.p(slot), self.count[slot], bit);
        self.probability[slot] = p as f32;
        self.count[slot] = count;
    }
}

/// Low bits of a [`Slot`] holding its count; the high bits hold its
/// probability in units of `2^-PROBABILITY_BITS`.
const COUNT_BITS: u32 = 10;

/// Fraction bits of a [`Slot`]'s probability.
const PROBABILITY_BITS: u32 = u32::BITS - COUNT_BITS;

const _: () = assert!(
    COUNTER_LIMIT < 1 << COUNT_BITS,
    "a count at the limit fits its field"
);

/// One [`adapt`]ed probability and its count packed in a `u32`, lpaq1's
/// `StateMap` entry, so a [`Bucket`] fits one cache line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot(u32);

impl Slot {
    /// Probability one half, count zero.
    const FRESH: Self = Self(1 << (u32::BITS - 1));

    fn p(self) -> f64 {
        f64::from(self.0 >> COUNT_BITS) / f64::from(1u32 << PROBABILITY_BITS)
    }

    fn count(self) -> u16 {
        // A remainder below 2^COUNT_BITS, which the const assert above
        // holds under u16's range.
        u16::try_from(self.0 % (1 << COUNT_BITS)).expect("COUNT_BITS fits u16")
    }

    /// Applies [`adapt`], rounding the probability to the nearest unit and
    /// holding it below one, which the field cannot represent.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "adapt keeps p in [0, 1], so the rounded scale is in [0, 2^PROBABILITY_BITS)"
    )]
    fn update(&mut self, bit: bool) {
        let (p, count) = adapt(self.p(), self.count(), bit);
        let max = (1u32 << PROBABILITY_BITS) - 1;
        let units = (p * f64::from(1u32 << PROBABILITY_BITS))
            .round()
            .min(f64::from(max)) as u32;
        self.0 = (units << COUNT_BITS) | u32::from(count);
    }
}

/// One order's [`Slot`]s for one nibble context, a slot per node
/// `1..NIBBLE_NODES`, lpaq1's layout: a nibble's nodes share one 64-byte
/// cache line, so an order reads one aligned set of [`ORDER_WAYS`] lines
/// per nibble instead of a line per node (`research/JOURNAL.md` S2-A136,
/// S2-A138).
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
struct Bucket {
    /// The owning context's [`check_of`]; [`claim`] finds a context by it.
    check: u16,
    slots: [Slot; NIBBLE_NODES - 1],
}

impl Bucket {
    const FRESH: Self = Self {
        check: 0,
        slots: [Slot::FRESH; NIBBLE_NODES - 1],
    };
}

/// The bucket `check`'s context owns in the [`ORDER_WAYS`] set holding
/// `index`, lpaq1's replacement: a context already in the set keeps its
/// bucket; otherwise the bucket whose nibble root was seen least is reset
/// to [`Bucket::FRESH`] under `check`, the first such on a tie. A
/// collision costs the evicted context its history instead of sharing it
/// with a stranger.
fn claim(table: &mut [Bucket], index: usize, check: u16) -> usize {
    let set = index - index % ORDER_WAYS..index - index % ORDER_WAYS + ORDER_WAYS;
    if let Some(owned) = set.clone().find(|&i| table[i].check == check) {
        return owned;
    }
    let victim = set
        .min_by_key(|&i| table[i].slots[0].count())
        .expect("ORDER_WAYS is at least one");
    table[victim] = Bucket {
        check,
        ..Bucket::FRESH
    };
    victim
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
/// predicted, so a prediction only ever reads the prefix. Every index it
/// reads derives from its own table and the prefix's length, never from
/// a coded bit, so on a decode the prefix is the output so far and no
/// input can steer an index out of it.
#[derive(Debug, Clone)]
struct MatchModel {
    /// Position just after each hashed `MATCH_MIN_LEN`-byte context; `0`
    /// is empty, which no stored position can be. A position past
    /// `u32::MAX` is never stored, on both sides alike; [`decode`] never
    /// reaches one, its length being a `u32`.
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
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            table: try_filled_vec(1 << MATCH_TABLE_BITS, 0)?,
            pointer: 0,
            length: 0,
            confidence: Counter::try_new(LENGTH_BUCKETS * 2)?,
        })
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
            // An empty slot verifies nothing: `0..0` counts zero bytes.
            if self.length == 0 {
                let verified = (0..candidate.min(MATCH_VERIFY_LIMIT))
                    .take_while(|&back| prefix[candidate - 1 - back] == prefix[position - 1 - back])
                    .count();
                if verified >= MATCH_MIN_LEN {
                    self.length = verified;
                    self.pointer = candidate;
                }
            }
            if let Ok(stored) = u32::try_from(position) {
                self.table[entry] = stored;
            }
        }
        (self.length > 0).then(|| (prefix[self.pointer], self.length))
    }
}

/// The hashed orders: one [`Bucket`] table per [`ORDERS`] entry, and the
/// bucket each currently owns for the nibble being walked.
#[derive(Debug, Clone)]
struct Orders {
    tables: Vec<Vec<Bucket>>,
    claimed: [usize; ORDERS.len()],
}

impl Orders {
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        let mut tables = Vec::new();
        tables.try_reserve_exact(ORDERS.len())?;
        for _ in ORDERS {
            tables.push(try_filled_vec(1 << ORDER_BUCKET_BITS, Bucket::FRESH)?);
        }
        Ok(Self {
            tables,
            claimed: [0; ORDERS.len()],
        })
    }

    /// [`claim`]s each order's bucket for the nibble rooted at `node`, from
    /// the byte's context `hashes`. A nibble starts at node 1 or at 16 to
    /// 31, which names the high nibble, so the low nibble's buckets key on
    /// it.
    fn claim_nibble(&mut self, hashes: &[u64; ORDERS.len()], node: usize) {
        let nibble_key = (node as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for ((claimed, &h), table) in self.claimed.iter_mut().zip(hashes).zip(&mut self.tables) {
            let h = mix64(h ^ nibble_key);
            *claimed = claim(table, slot(h, ORDER_BUCKET_BITS), check_of(h));
        }
    }

    /// Each order's stretched probability at `nibble_node`.
    fn stretches(&self, nibble_node: usize) -> [f64; ORDERS.len()] {
        let i = nibble_node - 1;
        let mut out = [0.0; ORDERS.len()];
        for ((out, table), &b) in out.iter_mut().zip(&self.tables).zip(&self.claimed) {
            *out = stretch(table[b].slots[i].p());
        }
        out
    }

    fn update(&mut self, nibble_node: usize, bit: bool) {
        for (table, &b) in self.tables.iter_mut().zip(&self.claimed) {
            table[b].slots[nibble_node - 1].update(bit);
        }
    }
}

/// The path's model: [`Literal`]'s wire walk plus the match model and the
/// hashed orders under one outer logistic mixer. Fresh per stream.
#[derive(Debug, Clone)]
pub struct CmPath {
    literal: Literal,
    literal_mixer: SurpriseLogisticMix,
    match_model: MatchModel,
    orders: Orders,
    weights: Vec<[f64; INPUTS]>,
}

impl Default for CmPath {
    fn default() -> Self {
        Self::new()
    }
}

impl CmPath {
    /// A fresh model: every counter at one half, the outer mixer weighing
    /// the wire model 1 and every other input 0.
    #[must_use]
    pub fn new() -> Self {
        crate::abort_on_oom(Self::try_new())
    }

    /// [`Self::new`], returning `Err` if the allocator cannot satisfy a
    /// table: [`decode`]'s constructor (hard rule 2). Every size is a
    /// constant, none read from a stream.
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        let mut start = [0.0; INPUTS];
        start[0] = 1.0;
        Ok(Self {
            literal: Literal::try_new()?,
            literal_mixer: SurpriseLogisticMix::try_new()?,
            match_model: MatchModel::try_new()?,
            orders: Orders::try_new()?,
            weights: try_filled_vec(WEIGHT_SETS, start)?,
        })
    }

    /// One byte's walk after `prefix` under `context`, the step [`encode`],
    /// [`decode`] and the benchmark harness's pricing share so they cannot
    /// drift apart: `code_bit` receives each node's midpoint and the mixed
    /// probability, clamped, that the node's bit is `1` (the byte is at or
    /// above the midpoint) and returns the bit the node resolved to; every
    /// part of the model then learns it. Returns the landed byte.
    pub fn walk(
        &mut self,
        context: Context,
        prefix: &[u8],
        mut code_bit: impl FnMut(usize, f64) -> bool,
    ) -> u8 {
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

        let (mut lo, mut hi, mut node) = (0usize, 256usize, 1usize);
        let mut nibble_node = 1usize;
        literal.walk_logit_sse(context, literal_mixer, |mid, literal_p| {
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

            if nibble_node == 1 {
                orders.claim_nibble(&order_hashes, node);
            }
            inputs[2..].copy_from_slice(&orders.stretches(nibble_node));

            let set = bucket * 2 + usize::from(node >= NIBBLE_NODES);
            let w = &mut weights[set];
            let dot: f64 = w.iter().zip(&inputs).map(|(w, x)| w * x).sum();
            let p = clamp(logistic::squash(dot));
            let bit = code_bit(mid, p);

            let error = f64::from(u8::from(bit)) - p;
            for (w, x) in w.iter_mut().zip(&inputs) {
                *w += MIXER_RATE * error * x;
            }
            if let Some(s) = confidence_slot {
                match_model.confidence.update(s, bit);
            }
            orders.update(nibble_node, bit);

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
        })
    }
}

/// `data` coded byte by byte through a fresh [`CmPath`] and the range
/// coder: the stream [`decode`] inverts given `data.len()`. Carries no
/// length and no header; the frame that wires it declares both.
#[must_use]
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut model = CmPath::new();
    let mut encoder = Encoder::new();
    let mut context = Context::default();
    for (position, &byte) in data.iter().enumerate() {
        let symbol = usize::from(byte);
        let landed = model.walk(context, &data[..position], |mid, p| {
            let bit = symbol >= mid;
            encoder.encode_bit(bit, p);
            bit
        });
        debug_assert_eq!(landed, byte, "the walk must land on the coded byte");
        context = context.after_literal(byte);
    }
    encoder.finish()
}

/// The `len` bytes [`encode`] coded into `stream`.
///
/// `len` is the caller's declared length, checked before any allocation
/// against `max_len` capped at [`crate::codec::MAX_DECODED_LEN`], which
/// bounds the output buffer, and against `MAX_EXPANSION` bytes per stream
/// byte, the most any encoding holds, which bounds the decode work, one
/// walk per byte, by the input. The model's tables are constants and
/// allocate fallibly.
///
/// # Errors
///
/// [`Error::TooLarge`] when `len` exceeds the capped `max_len`,
/// [`Error::Truncated`] when `stream` is too short for any encoding of
/// `len` bytes, [`Error::OutOfMemory`] when the allocator refuses the
/// output or a table. Any other stream decodes to some `len` bytes: the
/// coder reads past the end as zero bits and every probability is the
/// model's own, so a damaged stream is wrong output, which the frame's
/// check value catches, never a panic.
pub fn decode(stream: &[u8], len: u32, max_len: u32) -> Result<Vec<u8>, Error> {
    let max_len = max_len.min(crate::codec::MAX_DECODED_LEN);
    Error::ensure_within(u64::from(len), max_len)?;
    if u64::from(len) > u64::from(MAX_EXPANSION).saturating_mul(stream.len() as u64) {
        return Err(Error::Truncated);
    }
    let len = usize::try_from(len).map_err(|_| Error::TooLarge { len, max: max_len })?;
    let mut output = Vec::new();
    output.try_reserve_exact(len)?;
    let mut model = CmPath::try_new()?;
    let mut decoder = Decoder::new(stream);
    let mut context = Context::default();
    while output.len() < len {
        let byte = model.walk(context, &output, |_mid, p| decoder.decode_bit(p));
        output.push(byte);
        context = context.after_literal(byte);
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
