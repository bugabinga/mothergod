//! A context-mixing path over every byte, no LZ tokens: charter #1763's
//! candidate (`research/JOURNAL.md` S2-A133 to S2-A138, ADR-0077), its
//! contexts widened by S2-A150 (ADR-0081), S2-A151 (ADR-0082) and S2-A152
//! (ADR-0083), coded through the real range coder.
//!
//! Per bit-tree node of each byte, an outer logistic mixer blends, and five
//! calibration tables refine what it says:
//!
//! ```text
//!   Literal (six experts, SurpriseLogisticMix, LogitSse) ─ p ─┐
//!   match model: predicted byte, confirmed length ───────── p ─┤
//!   hashed orders 2 to 6 and 8 ─────────────────────────── p ─┤
//!   hashed word: its letters and the byte before ───────── p ─┤
//!   hashed record column: 1 and 2 records above ────────── p ─┤
//!   hashed record row: 1 record above, the last byte ───── p ─┤ stretch, and
//!   hashed word pair: the previous word's and this one's ─ p ─┤ less 1/2 too
//!   hashed sparse: bytes 3 and 2 back ──────────────────── p ─┤ for each hashed
//!   hashed sparse: bytes 2 and 4 back ──────────────────── p ─┤ context; weighted
//!   hashed sparse: bytes 4 and 8 back ──────────────────── p ─┤ sum plus a bias,
//!   hashed indirect: what followed the last byte ───────── p ─┤ weights selected
//!   hashed indirect: what followed the last two ────────── p ─┤ by match class,
//!   hashed sparse: the last four bytes' high nibbles ───── p ─┤ seen order and
//!   hashed indirect: what followed the last three ──────── p ─┤ nibble; squash
//!   hashed record position, 1 record above ─────────────── p ─┤ ─> p
//!   hashed record position ─────────────────────────────── p ─┤
//!   hashed record position, 1 and 2 records above ──────── p ─┘
//!
//!   p ─┬─ APM by node ───────────────── a0 ─┐
//!      ├─ APM by last byte, node ─────── a1 ─┤ ((p + 3 a0) / 4
//!      ├─ APM by order-2 hash, node ──── a2 ─┤   + a1 + a2 + a3 + a4) / 5
//!      ├─ APM by match state, node ───── a3 ─┤   ─> coder
//!      └─ APM by seen order, node ────── a4 ─┘
//! ```
//!
//! The first input is the shipped wire model's own refined probability,
//! [`Literal::walk_logit_sse`], and a fresh mixer weighs it 1 and every
//! other input 0, so the mixer starts as the wire model and only adds what
//! it learns pays; a fresh calibration table maps a probability to itself
//! within its interpolation error.
//!
//! A [`crate::Method::Cm`] frame carries it, written at
//! [`crate::Level::Extreme`] when smaller than the hybrid's (ADR-0079).
//! [`encode_payload`] and [`decode_payload`] are that frame's payload;
//! [`encode`] and [`decode`] its coded stream alone, and
//! `mothergod_bench::cm_path` prices through [`CmPath::walk`].
//! Every value both sides compute goes through IEEE basic operations and
//! the codec's own [`logistic::stretch`] and [`logistic::squash`], never
//! libm (ADR-0024, S2-A134).
//!
//! # Payload layout
//!
//! ```text
//! offset  size  field
//! 0       2     selector: [kind, param] (`filters::select::Candidate`)
//! 2       4     declared length of the filtered bytes, u32 LE
//! 6       8     coded length: the stream's byte count, u64 LE
//! 14      ...   the stream [`encode`] codes the FILTERED bytes to
//! ```

use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::codec::{self, Extent, MAX_DECODED_LEN, SELECTOR_LEN};
use crate::coder::{Decoder, Encoder};
use crate::filters::{self, select::Candidate};
use crate::literal::{Context, Literal, SurpriseLogisticMix, logistic_gradient_step};
use crate::sse::LogitSse;
use crate::{Error, fan_out, logistic, try_filled_vec, try_with_capacity};

/// The most bytes one [`crate::Method::Cm`] frame, and one [`decode`],
/// outputs: a ceiling below [`MAX_DECODED_LEN`], because every output byte
/// pays a full walk and none is a copy. A hostile frame at this ceiling
/// decodes no slower than a hostile `Method::Lz` frame at
/// [`MAX_DECODED_LEN`] (ADR-0079): the bench crate's `hostile_decode`
/// times both on one CPU and exits non-zero when that inverts (issue
/// #1915). Its read at version 22, release build, AMD EPYC 7763: 267 s
/// here, 32% of `Lz`'s 843 s. The walk's cost does not depend on the
/// stream, so the worst case is linear in the declared length
/// (`research/JOURNAL.md` S2-A152). It equals the block length
/// [`crate::compress`] cuts at, so no frame this crate writes is refused.
pub const MAX_OUTPUT_LEN: u32 = 16 * 1024 * 1024;

const _: () = assert!(
    MAX_OUTPUT_LEN <= MAX_DECODED_LEN,
    "a Cm frame is a coded frame, under the coded frames' ceiling"
);

/// The reciprocal of `PROBABILITY_FLOOR`, the outer mixer's clamp, and so
/// the most bytes one stream byte decodes to: [`decode`]'s bomb guard.
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
pub const MAX_EXPANSION: u32 = 4096;

/// Every probability the outer mixer stretches or codes is clamped to
/// `[PROBABILITY_FLOOR, 1 - PROBABILITY_FLOOR]`, bounding a stretch to
/// `ln(4095)` and a node's cost to 12 bits.
#[allow(
    clippy::cast_lossless,
    reason = "f64::from is not const; a u32 converts to f64 exactly"
)]
const PROBABILITY_FLOOR: f64 = 1.0 / MAX_EXPANSION as f64;

/// The outer mixer's gradient step. S2-A151 kept 0.007 over 0.005 and
/// 0.0035; with S2-A152's inputs, about twice as many, 0.005 priced lower
/// than 0.007 and 0.004 (`research/JOURNAL.md`).
const MIXER_RATE: f64 = 0.005;

/// Scale of a context's second mixer input, its probability less one half,
/// paq8's linear-domain input beside the stretched one: 1 and 4 priced
/// worse (`research/JOURNAL.md` S2-A152).
const LINEAR_SCALE: f64 = 2.0;

/// The outer mixer's last input, constant, so each weight set learns a
/// bias (`research/JOURNAL.md` S2-A152).
const BIAS: f64 = 1.0;

/// A [`Counter`] slot's rate is `1 / (count + 1.5)`, its count capped
/// here, so a slot's adaptation slows to this floor and no further.
const COUNTER_LIMIT: u16 = 1023;

/// The hashed context orders, each one table, in mixer order, ascending,
/// the first tables of [`Contexts`]. Order 1 is the wire model's own, and
/// a table of it priced worse; dropping order 5, lpaq1's set, priced worse
/// on the structured cases (`research/JOURNAL.md` S2-A150); order 8 priced
/// lower (S2-A152).
const ORDERS: [usize; 6] = [2, 3, 4, 5, 6, 8];

/// Values [`Contexts::longest_seen_order`] returns: `0`, no order's slot
/// has learned at the node, or one past the index into [`ORDERS`] of the
/// longest that has.
const ORDER_CLASSES: usize = ORDERS.len() + 1;

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
/// nibble's root, one [`child`] per step, as the byte's tree is; so
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

/// The mixer's coarse view of a [`length_bucket`]: `0` no match speaking,
/// `1` a match of 1 to 15 bytes, `2` a longer one.
fn match_class(bucket: usize) -> usize {
    match bucket {
        0 => 0,
        1..=15 => 1,
        _ => 2,
    }
}

/// Values [`match_class`] returns.
const MATCH_CLASSES: usize = 3;

/// Weight sets in the outer mixer: one per [`ORDER_CLASSES`] and
/// [`MATCH_CLASSES`] pair, split by whether the node is in the byte's high
/// or low nibble. The 21 length buckets alone priced worse once the order
/// class joined them, their sets learning slower (`research/JOURNAL.md`
/// S2-A152).
const WEIGHT_SETS: usize = ORDER_CLASSES * MATCH_CLASSES * 2;

/// Inputs to the outer mixer, in its order: the wire model, the match
/// model, each hashed context stretched, each again in the linear domain
/// ([`LINEAR_SCALE`]), then [`BIAS`].
const INPUTS: usize = 3 + 2 * CONTEXTS;

/// Hashed context tables, in mixer order: the [`ORDERS`], [`Words::word`],
/// the record column and row ([`Records`]), [`Words::bigram`], the three
/// [`SPARSE_SEEDS`] contexts, the byte and pair [`Indirect`] contexts, the
/// [`HIGH_NIBBLES_SEED`] context, the triple [`Indirect`] context, then the
/// three [`RECORD_POSITION_SEEDS`] contexts. The order is part of the
/// format: the mixer's dot product sums its terms in it.
const CONTEXTS: usize = ORDERS.len() + 14;

/// Seeds of the record column context, the bytes one and two records
/// above, and of the record row context, one record above and the last
/// byte, so the two never share a hash.
const RECORD_COLUMN_SEED: u64 = 0x5ecb;
const RECORD_ROW_SEED: u64 = 0x5eca;

/// The contexts keyed on the position within a record,
/// [`Records::column`] (`research/JOURNAL.md` S2-A152), each its seed, in
/// mixer order: the column with one record above; the column alone; the
/// column with one and two records above.
const RECORD_POSITION_SEEDS: [u64; 3] = [0x5ecc, 0x5ecd, 0x5ece];

/// The sparse contexts, paq8's sparse model (`research/JOURNAL.md`
/// S2-A151): bytes 3 and 2 back, skipping the last; bytes 2 and 4 back, a
/// 16-bit column; bytes 4 and 8 back, a 32-bit column. Each its seed, in
/// mixer order.
const SPARSE_SEEDS: [u64; 3] = [0x5ba7, 0x5ba8, 0x5ba9];

/// Seed of the sparse context of the last four bytes' high nibbles, paq8l's
/// (`research/JOURNAL.md` S2-A152).
const HIGH_NIBBLES_SEED: u64 = 0x5baa;

/// Seeds of the [`Indirect`] contexts: after one byte, after two, after
/// three; in that order, which is not the mixer's.
const INDIRECT_SEEDS: [u64; 3] = [0x1d1, 0x1d2, 0x1d3];

/// Entries in [`Indirect`]'s table of what followed three bytes, as a power
/// of two: the three bytes are hashed to an index.
const INDIRECT_TRIPLE_BITS: u32 = 16;

/// Contexts of [`CmPath`]'s first calibration table: the byte's bit-tree
/// node, `1..BYTE_NODES`.
const BYTE_NODES: usize = 256;

/// Bits of the order-2 hash that key the third calibration table, beside
/// the node.
const APM_ORDER2_BITS: u32 = 8;

/// Match states keying the fourth calibration table, beside the node: `0`
/// while no prediction speaks at the node, else one past the match
/// model's confidence slot, `bucket * 2 + predicted bit`.
const MATCH_STATES: usize = LENGTH_BUCKETS * 2 + 1;

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

/// A table of [`adapt`]ed bit probabilities, one per slot, each starting at
/// one half. The match model's confidence and
/// [`crate::literal::bit_mix`]'s experts.
#[derive(Debug, Clone)]
pub(crate) struct Counter {
    probability: Vec<f32>,
    count: Vec<u16>,
}

impl Counter {
    pub(crate) fn try_new(slots: usize) -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            probability: try_filled_vec(slots, 0.5)?,
            count: try_filled_vec(slots, 0)?,
        })
    }

    pub(crate) fn p(&self, slot: usize) -> f64 {
        f64::from(self.probability[slot])
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "a probability in [0, 1] narrows to f32 by rounding only"
    )]
    pub(crate) fn update(&mut self, slot: usize, bit: bool) {
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

/// The child of bit-tree `node` that `bit` selects: root `1`, children
/// `2n` and `2n + 1`, so a node id holds the path's bits below its top
/// `1`.
fn child(node: usize, bit: bool) -> usize {
    node * 2 + usize::from(bit)
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

/// Seed of [`Words::word`]'s fold.
const WORD_SEED: u64 = 0x776f_7264;

/// Seed of [`Words::bigram`]'s fold.
const BIGRAM_SEED: u64 = 0x6269_6772_616d;

/// The two text contexts (`research/JOURNAL.md` S2-A150), each a
/// [`mix64`] fold over bytes, ASCII case folded, kept up to date one byte
/// at a time so a walk costs the same however long a word runs. A word is
/// a maximal run of ASCII letters.
///
/// Invariant: [`Self::push`] has seen every byte of the prefix in order,
/// once, which [`CmPath::walk`]'s one call per byte provides, the contract
/// [`MatchModel`] already holds it to.
#[derive(Debug, Clone, Copy)]
struct Words {
    /// lpaq1's word context: the letters of the current word, after the
    /// byte before it when one exists, folded from `WORD_SEED`.
    word: u64,
    /// The previous word's letters, a space, then the current word's,
    /// folded from `BIGRAM_SEED`; the non-letters between them are skipped.
    bigram: u64,
    /// The current word's letters alone, folded from `BIGRAM_SEED`: the
    /// previous word once a non-letter ends it.
    current: u64,
    /// Whether the last byte pushed was a letter.
    in_word: bool,
}

impl Words {
    fn new() -> Self {
        let current = mix64(BIGRAM_SEED);
        Self {
            word: mix64(WORD_SEED),
            bigram: mix64(current ^ u64::from(b' ')),
            current,
            in_word: false,
        }
    }

    /// Folds in the prefix's next byte.
    fn push(&mut self, byte: u8) {
        if byte.is_ascii_alphabetic() {
            let letter = u64::from(byte.to_ascii_lowercase());
            self.word = mix64(self.word ^ letter);
            self.bigram = mix64(self.bigram ^ letter);
            self.current = mix64(self.current ^ letter);
        } else {
            self.word = mix64(mix64(WORD_SEED) ^ u64::from(byte));
            if self.in_word {
                self.bigram = mix64(self.current ^ u64::from(b' '));
                self.current = mix64(BIGRAM_SEED);
            }
        }
        self.in_word = byte.is_ascii_alphabetic();
    }
}

/// The byte `back` positions before the end of `prefix`, plus one, or `0`
/// when the prefix is shorter: a context key in which absence is its own
/// value. `back` is at least one.
fn byte_back(prefix: &[u8], back: usize) -> u64 {
    prefix
        .len()
        .checked_sub(back)
        .map_or(0, |i| u64::from(prefix[i]) + 1)
}

/// paq8's record model (`research/JOURNAL.md` S2-A151): a record length
/// read off the stream, the gap at which one byte value has recurred
/// three times running, so a fixed-width record's bytes can be predicted
/// from the same column of the records above.
///
/// Invariant: as for [`Words`], [`Self::push`] has seen every byte of the
/// prefix in order, once. `length` never exceeds the bytes pushed, so it
/// indexes the prefix wherever [`byte_back`] reads with it.
#[derive(Debug, Clone)]
struct Records {
    /// Bytes pushed so far: the 1-based position of the last one.
    seen: usize,
    /// Each byte value's last 1-based position, `0` while unseen.
    last: [usize; 256],
    /// Each byte value's last gap between two occurrences, `0` until it
    /// has one.
    gap: [usize; 256],
    /// The gap before [`Self::gap`].
    gap_before: [usize; 256],
    /// The latest gap above one that a byte value repeated at three times
    /// running, four evenly spaced occurrences; `0` until one has.
    length: usize,
}

impl Records {
    fn new() -> Self {
        Self {
            seen: 0,
            last: [0; 256],
            gap: [0; 256],
            gap_before: [0; 256],
            length: 0,
        }
    }

    /// Folds in the prefix's next byte.
    fn push(&mut self, byte: u8) {
        self.seen += 1;
        let b = usize::from(byte);
        let gap = if self.last[b] > 0 {
            self.seen - self.last[b]
        } else {
            0
        };
        if gap > 1 && gap == self.gap[b] && gap == self.gap_before[b] {
            self.length = gap;
        }
        self.gap_before[b] = self.gap[b];
        self.gap[b] = gap;
        self.last[b] = self.seen;
    }

    /// [`byte_back`] of the byte `records` record lengths before the one
    /// being predicted, `0` while no length is known.
    fn above(&self, prefix: &[u8], records: usize) -> u64 {
        match self.length {
            0 => 0,
            length => byte_back(prefix, length.saturating_mul(records)),
        }
    }

    /// The position of the byte being predicted within a record of the
    /// current length, counted from the stream's start, so it shifts when
    /// the length changes; `0` while no length is known.
    fn column(&self) -> u64 {
        (self.seen % self.length.max(1)) as u64
    }
}

/// paq8l's indirect model (`research/JOURNAL.md` S2-A152): for the last
/// one, two and three bytes, the two bytes that followed them the last two
/// times they occurred, so a context predicts from what came after its
/// earlier occurrences rather than from the occurrences themselves.
///
/// Invariant: as for [`Words`], [`Self::push`] has seen every byte of the
/// prefix in order, once. Every index is masked or hashed into its table,
/// whatever the bytes.
#[derive(Debug, Clone)]
struct Indirect {
    /// The last four bytes pushed, the latest lowest, zero before the start:
    /// a stream's first bytes are keyed as if zero bytes preceded them.
    recent: u32,
    /// Per byte value, the two bytes that followed it, the latest lowest.
    after_byte: [u16; 256],
    /// Per two-byte value, as [`Self::after_byte`].
    after_pair: Vec<u16>,
    /// Per hashed three-byte value, as [`Self::after_byte`].
    after_triple: Vec<u16>,
}

impl Indirect {
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            recent: 0,
            after_byte: [0; 256],
            after_pair: try_filled_vec(1 << 16, 0)?,
            after_triple: try_filled_vec(1 << INDIRECT_TRIPLE_BITS, 0)?,
        })
    }

    /// The latest byte pushed, the latest two as one `u16`, and the
    /// latest three's slot in [`Self::after_triple`].
    fn keys(&self) -> (usize, usize, usize) {
        let [b0, b1, ..] = self.recent.to_le_bytes();
        (
            usize::from(b0),
            usize::from(u16::from_le_bytes([b0, b1])),
            slot(
                mix64(u64::from(self.recent & 0xff_ffff)),
                INDIRECT_TRIPLE_BITS,
            ),
        )
    }

    /// Folds in the prefix's next byte.
    fn push(&mut self, byte: u8) {
        let (one, two, three) = self.keys();
        let b = u16::from(byte);
        for entry in [
            &mut self.after_byte[one],
            &mut self.after_pair[two],
            &mut self.after_triple[three],
        ] {
            *entry = (*entry << 8) | b;
        }
        self.recent = (self.recent << 8) | u32::from(byte);
    }

    /// The three contexts, after one, two and three bytes: each its key
    /// and what followed it.
    fn hashes(&self) -> [u64; 3] {
        let (one, two, three) = self.keys();
        let [s1, s2, s3] = INDIRECT_SEEDS;
        [
            mix64(mix64(s1 ^ one as u64) ^ u64::from(self.after_byte[one])),
            mix64(mix64(s2 ^ two as u64) ^ u64::from(self.after_pair[two])),
            mix64(
                mix64(s3 ^ u64::from(self.recent & 0xff_ffff))
                    ^ u64::from(self.after_triple[three]),
            ),
        ]
    }

    /// paq8l's sparse context of the last four bytes' high nibbles.
    fn high_nibbles(&self) -> u64 {
        mix64(HIGH_NIBBLES_SEED ^ u64::from(self.recent & 0xf0f0_f0f0))
    }
}

/// The hashed contexts: one [`Bucket`] table per context [`CONTEXTS`]
/// lists, and the bucket each currently owns for the nibble being walked.
#[derive(Debug, Clone)]
struct Contexts {
    tables: Vec<Vec<Bucket>>,
    claimed: [usize; CONTEXTS],
}

impl Contexts {
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        let mut tables = try_with_capacity(CONTEXTS)?;
        for _ in 0..CONTEXTS {
            tables.push(try_filled_vec(1 << ORDER_BUCKET_BITS, Bucket::FRESH)?);
        }
        Ok(Self {
            tables,
            claimed: [0; CONTEXTS],
        })
    }

    /// [`claim`]s each context's bucket for the nibble rooted at `node`, from
    /// the byte's context `hashes`. A nibble starts at node 1 or at 16 to
    /// 31, which names the high nibble, so the low nibble's buckets key on
    /// it.
    fn claim_nibble(&mut self, hashes: &[u64; CONTEXTS], node: usize) {
        let nibble_key = (node as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for ((claimed, &h), table) in self.claimed.iter_mut().zip(hashes).zip(&mut self.tables) {
            let h = mix64(h ^ nibble_key);
            *claimed = claim(table, slot(h, ORDER_BUCKET_BITS), check_of(h));
        }
    }

    /// Each context's probability at `nibble_node` through `transform`.
    fn map(&self, nibble_node: usize, transform: impl Fn(f64) -> f64) -> [f64; CONTEXTS] {
        let i = nibble_node - 1;
        let mut out = [0.0; CONTEXTS];
        for ((out, table), &b) in out.iter_mut().zip(&self.tables).zip(&self.claimed) {
            *out = transform(table[b].slots[i].p());
        }
        out
    }

    /// The longest of the [`ORDERS`] whose slot at `nibble_node` has learned
    /// a bit, as one of [`ORDER_CLASSES`]: paq8's mixer context, how much
    /// history the hashed orders hold here.
    fn longest_seen_order(&self, nibble_node: usize) -> usize {
        (0..ORDERS.len())
            .rev()
            .find(|&i| self.tables[i][self.claimed[i]].slots[nibble_node - 1].count() > 0)
            .map_or(0, |i| i + 1)
    }

    fn update(&mut self, nibble_node: usize, bit: bool) {
        for (table, &b) in self.tables.iter_mut().zip(&self.claimed) {
            table[b].slots[nibble_node - 1].update(bit);
        }
    }
}

/// The five calibration tables after the outer mixer, lpaq1's APM stage
/// widened (`research/JOURNAL.md` S2-A151, S2-A152): each refines the
/// mixer's probability in its own context beside the byte's bit-tree node.
#[derive(Debug, Clone)]
struct Calibration {
    /// `[node, last byte, order-2 hash, match state, longest seen order]`,
    /// in the order [`Self::refine`] blends them.
    tables: [LogitSse; 5],
}

impl Calibration {
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            tables: [
                LogitSse::try_new(BYTE_NODES)?,
                LogitSse::try_new(256 * BYTE_NODES)?,
                LogitSse::try_new((1 << APM_ORDER2_BITS) * BYTE_NODES)?,
                LogitSse::try_new(MATCH_STATES * BYTE_NODES)?,
                LogitSse::try_new(ORDER_CLASSES * BYTE_NODES)?,
            ],
        })
    }

    /// The probability coded at a node: `p` refined by each table at its
    /// row in `rows`, blended `((p + 3 a0) / 4 + a1 + a2 + a3 + a4) / 5`.
    fn refine(&self, rows: [usize; 5], p: f64) -> f64 {
        let [a0, a1, a2, a3, a4] = [0, 1, 2, 3, 4].map(|i| self.tables[i].refine(rows[i], p));
        clamp(((p + 3.0 * a0) / 4.0 + a1 + a2 + a3 + a4) / 5.0)
    }

    fn update(&mut self, rows: [usize; 5], p: f64, bit: bool) {
        for (table, row) in self.tables.iter_mut().zip(rows) {
            table.update(row, p, bit);
        }
    }
}

/// The path's model: [`Literal`]'s wire walk plus the match model and the
/// hashed contexts under one outer logistic mixer, then five calibration
/// tables. Fresh per stream.
#[derive(Debug, Clone)]
pub struct CmPath {
    literal: Literal,
    literal_mixer: SurpriseLogisticMix,
    match_model: MatchModel,
    words: Words,
    records: Records,
    indirect: Indirect,
    contexts: Contexts,
    weights: Vec<[f64; INPUTS]>,
    calibration: Calibration,
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
            words: Words::new(),
            records: Records::new(),
            indirect: Indirect::try_new()?,
            contexts: Contexts::try_new()?,
            weights: try_filled_vec(WEIGHT_SETS, start)?,
            calibration: Calibration::try_new()?,
        })
    }

    /// Every hashed context's key for the byte after `prefix`, in mixer
    /// order ([`CONTEXTS`]), once [`Self::words`] and [`Self::records`]
    /// have seen the prefix's last byte.
    fn context_hashes(&self, prefix: &[u8]) -> [u64; CONTEXTS] {
        let mut hashes = [0; CONTEXTS];
        for (h, order) in hashes.iter_mut().zip(ORDERS) {
            *h = hash_bytes(order as u64, &prefix[prefix.len().saturating_sub(order)..]);
        }
        let length = self.records.length as u64;
        let above = self.records.above(prefix, 1);
        let n = prefix.len();
        let above_two = self.records.above(prefix, 2);
        let column = self.records.column();
        let [skip_one, stride_two, stride_four] = SPARSE_SEEDS;
        let [after_byte, after_pair, after_triple] = self.indirect.hashes();
        let [column_above, column_alone, column_two_above] = RECORD_POSITION_SEEDS;
        let derived = [
            self.words.word,
            mix64(mix64(mix64(RECORD_COLUMN_SEED ^ length) ^ above) ^ above_two),
            mix64(mix64(mix64(RECORD_ROW_SEED ^ length) ^ above) ^ byte_back(prefix, 1)),
            self.words.bigram,
            hash_bytes(skip_one, &prefix[n.saturating_sub(3)..n.saturating_sub(1)]),
            mix64(mix64(stride_two ^ byte_back(prefix, 2)) ^ byte_back(prefix, 4)),
            mix64(mix64(stride_four ^ byte_back(prefix, 4)) ^ byte_back(prefix, 8)),
            after_byte,
            after_pair,
            self.indirect.high_nibbles(),
            after_triple,
            mix64(mix64(mix64(column_above ^ length) ^ column) ^ above),
            mix64(mix64(column_alone ^ length) ^ column),
            mix64(mix64(mix64(mix64(column_two_above ^ length) ^ column) ^ above) ^ above_two),
        ];
        hashes[ORDERS.len()..].copy_from_slice(&derived);
        hashes
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
        if let Some(&last) = prefix.last() {
            self.words.push(last);
            self.records.push(last);
            self.indirect.push(last);
        }
        let hashes = self.context_hashes(prefix);
        let Self {
            literal,
            literal_mixer,
            match_model,
            contexts,
            weights,
            calibration,
            ..
        } = self;
        let last_byte_row = usize::from(prefix.last().copied().unwrap_or(0)) * BYTE_NODES;
        let order2_row = slot(hashes[0], APM_ORDER2_BITS) * BYTE_NODES;

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
                contexts.claim_nibble(&hashes, node);
            }
            inputs[2..2 + CONTEXTS].copy_from_slice(&contexts.map(nibble_node, stretch));
            // Each probability less one half, times [`LINEAR_SCALE`].
            inputs[2 + CONTEXTS..INPUTS - 1]
                .copy_from_slice(&contexts.map(nibble_node, |p| (p - 0.5) * LINEAR_SCALE));
            inputs[INPUTS - 1] = BIAS;

            let seen_order = contexts.longest_seen_order(nibble_node);
            let set = (seen_order * MATCH_CLASSES + match_class(bucket)) * 2
                + usize::from(node >= NIBBLE_NODES);
            let w = &mut weights[set];
            let dot: f64 = w.iter().zip(&inputs).map(|(w, x)| w * x).sum();
            let p = clamp(logistic::squash(dot));
            let match_state = confidence_slot.map_or(0, |s| s + 1);
            let rows = [
                node,
                last_byte_row + node,
                order2_row + node,
                match_state * BYTE_NODES + node,
                seen_order * BYTE_NODES + node,
            ];
            let bit = code_bit(mid, calibration.refine(rows, p));
            calibration.update(rows, p, bit);

            logistic_gradient_step(w, &inputs, MIXER_RATE, bit, p);
            if let Some(s) = confidence_slot {
                match_model.confidence.update(s, bit);
            }
            contexts.update(nibble_node, bit);

            if bit {
                lo = mid;
            } else {
                hi = mid;
            }
            node = child(node, bit);
            nibble_node = child(nibble_node, bit);
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
/// against `max_len` capped at [`MAX_OUTPUT_LEN`], which
/// bounds the output buffer and the decode work, one walk per byte, and
/// against `MAX_EXPANSION` bytes per stream byte, the most any encoding
/// holds, which bounds both by the input. The model's tables are constants and
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
    let max_len = max_len.min(MAX_OUTPUT_LEN);
    Error::ensure_within(u64::from(len), max_len)?;
    if u64::from(len) > u64::from(MAX_EXPANSION).saturating_mul(stream.len() as u64) {
        return Err(Error::Truncated);
    }
    let len = len as usize;
    let mut output = try_with_capacity(len)?;
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

/// Byte length of a payload's fields between its selector and its stream:
/// the declared length of the filtered bytes, a `u32` as `Method::Lz`'s is,
/// then the coded length, a `u64` for the reason `Method::Lz`'s is.
const LENGTHS_LEN: usize = size_of::<u32>() + size_of::<u64>();

/// Bytes at the front of a [`crate::Method::Cm`] payload that
/// [`declared_extent`] reads: the selector and both lengths.
pub(crate) const EXTENT_PREFIX_LEN: usize = SELECTOR_LEN + LENGTHS_LEN;

/// The fields of a [`crate::Method::Cm`] payload before its stream.
struct Header {
    candidate: Candidate,
    declared_len: u32,
    coded_len: u64,
}

/// Reads the [`EXTENT_PREFIX_LEN`] header off the front of `payload`,
/// returning it and the bytes after it. The selector is read first, so a bad
/// one in a prefix cut short is [`Error::Corrupt`], as `Method::Lz`'s is.
fn split_header(payload: &[u8]) -> Result<(Header, &[u8]), Error> {
    let (&selector, rest) = payload.split_first_chunk().ok_or(Error::Truncated)?;
    // `Method::Lz` carries its reach in the kind byte's top bits; this path
    // has no reach, so a set bit names no kind and is rejected here.
    let candidate = Candidate::from_header_bytes(selector).ok_or(Error::Corrupt)?;
    let (&declared_len, rest) = rest.split_first_chunk().ok_or(Error::Truncated)?;
    let (&coded_len, rest) = rest.split_first_chunk().ok_or(Error::Truncated)?;
    let header = Header {
        candidate,
        declared_len: u32::from_le_bytes(declared_len),
        coded_len: u64::from_le_bytes(coded_len),
    };
    Ok((header, rest))
}

/// The [`Extent`] the first [`EXTENT_PREFIX_LEN`] bytes of a
/// [`crate::Method::Cm`] payload declare, trusting nothing else: no decode
/// work and no allocation, as [`codec::declared_extent`] is for `Method::Lz`.
///
/// # Errors
///
/// [`Error::Truncated`] if `bytes` is shorter than the prefix or the coded
/// length overflows `usize`; [`Error::Corrupt`] if the selector names no
/// [`Candidate`].
pub(crate) fn declared_extent(bytes: &[u8]) -> Result<Extent, Error> {
    let (header, _) = split_header(bytes)?;
    let coded_len = usize::try_from(header.coded_len).map_err(|_| Error::Truncated)?;
    Ok(Extent {
        payload_len: EXTENT_PREFIX_LEN
            .checked_add(coded_len)
            .ok_or(Error::Truncated)?,
        output_len: header.candidate.output_len(header.declared_len),
    })
}

/// The [`crate::Method::Cm`] payload for `data`: every filter
/// [`filters::select::pick`] shortlists, applied and coded by [`encode`]
/// through a fresh model, the shortest stream kept and a tie to the earlier
/// candidate, so the payload is the one a serial search writes. Up to
/// `workers` trials run at once, the calling thread being one; a trial holds
/// its filtered copy and one model's constant tables, not the block-sized
/// working set of a `Method::Lz` trial.
///
/// # Panics
///
/// If `data.len()` exceeds `u32::MAX`: [`crate::compress`] stores such input
/// before any trial.
#[must_use]
pub fn encode_payload(data: &[u8], workers: NonZeroUsize) -> Vec<u8> {
    let candidates = filters::select::pick(data);
    let next = AtomicUsize::new(0);
    let (_, candidate, declared_len, stream) = thread::scope(|scope| {
        let work = || {
            let mut kept: Option<(usize, Candidate, usize, Vec<u8>)> = None;
            loop {
                // Relaxed: the counter hands out each index once and orders
                // nothing else.
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&candidate) = candidates.get(index) else {
                    break;
                };
                let filtered = codec::apply_filter(candidate, data);
                let stream = encode(&filtered);
                if kept.as_ref().is_none_or(|(.., incumbent)| {
                    crate::candidate_beats_incumbent(stream.len(), incumbent.len())
                }) {
                    kept = Some((index, candidate, filtered.len(), stream));
                }
            }
            kept
        };
        fan_out(scope, workers.get().min(candidates.len()), work)
            .into_iter()
            .flatten()
            .min_by_key(|(index, .., stream)| (stream.len(), *index))
            .expect("filters::select::pick always returns at least Candidate::Identity")
    });
    let declared_len = u32::try_from(declared_len).expect("a block's filtered length fits u32");
    let coded_len = stream.len() as u64;
    let mut out = Vec::with_capacity(EXTENT_PREFIX_LEN + stream.len());
    out.extend_from_slice(&candidate.to_header_bytes());
    out.extend_from_slice(&declared_len.to_le_bytes());
    out.extend_from_slice(&coded_len.to_le_bytes());
    out.extend(stream);
    out
}

/// The bytes a [`crate::Method::Cm`] `payload` decodes to: its stream
/// through [`decode`] at the declared length, then the selector's filter
/// undone.
///
/// The frame's output, [`Candidate::output_len`] of the declared length, is
/// checked against `max_len` capped at [`MAX_OUTPUT_LEN`] before any
/// allocation; [`decode`] bounds the work by the stream's bytes and
/// allocates fallibly, and so does the filter's undo.
///
/// # Errors
///
/// - [`Error::Truncated`] if `payload` is shorter than its header, than the
///   stream it declares, or than any stream of the declared length.
/// - [`Error::TooLarge`] if the output exceeds the capped `max_len`.
/// - [`Error::Corrupt`] if the selector names no [`Candidate`] or bytes
///   follow the stream.
/// - [`Error::OutOfMemory`] if the allocator refuses the output, a table or
///   the undo buffer.
///
/// A stream damaged past its header decodes to wrong bytes of the declared
/// length, never a panic; the frame's check value is what catches it.
pub fn decode_payload(payload: &[u8], max_len: u32) -> Result<Vec<u8>, Error> {
    let (header, stream) = split_header(payload)?;
    let max_len = max_len.min(MAX_OUTPUT_LEN);
    Error::ensure_within(header.candidate.output_len(header.declared_len), max_len)?;
    codec::ensure_coded_len(stream, header.coded_len)?;
    // Every kind's output is at least its filtered bytes, so `max_len` bounds
    // the declared length too.
    let filtered = decode(stream, header.declared_len, max_len)?;
    codec::undo_filter(header.candidate, filtered)
}

#[cfg(test)]
mod tests;
