//! Adaptive range coder: [`Encoder`] and [`Decoder`], driven by
//! caller-supplied cumulative-frequency ranges (`JOURNAL` S2-D2).
//!
//! A 32-bit interval renormalizes in three cases: top half fixed, bottom
//! half fixed, straddling the middle with the carry deferred via `pending`.
//!
//! [`Decoder`] reads a stream shorter than it expects as implicit trailing
//! zero bits, never a panic (hard rule 2, `CLAUDE.md`).

/// The coder's `[low, high]` interval is `[0, TOP)`. `u64` arithmetic keeps
/// every intermediate (`range * total`) from overflowing as long as `total`
/// fits in 32 bits.
const TOP: u64 = 1 << 32;
const HALF: u64 = TOP >> 1;
const QUARTER: u64 = TOP >> 2;
const THREE_QUARTERS: u64 = 3 * (TOP >> 2);
const MASK: u64 = TOP - 1;

/// Total the binary coders split: [`Encoder::encode_bits`] and
/// [`Decoder::decode_bits`] take each bit as symbol 0 or 1 out of it, exactly
/// 50/50; [`Encoder::encode_bit`] and [`Decoder::decode_bit`] split it at a
/// quantized probability.
const BIT_SCALE: u64 = 1 << 16;

/// Narrows `[low, high]` to the sub-range `[cum_low, cum_high)` out of
/// `total`. Shared by [`Encoder::encode`] and [`Decoder::decode`] so their
/// rounding cannot drift apart and desync the stream (hard rule 1).
///
/// # Panics
///
/// Panics if `total` is zero (integer division by zero).
fn narrow(low: u64, high: u64, cum_low: u64, cum_high: u64, total: u64) -> (u64, u64) {
    let range = high - low + 1;
    let new_high = low + range * cum_high / total - 1;
    let new_low = low + range * cum_low / total;
    (new_low, new_high)
}

/// Splits `[0, BIT_SCALE)` at `threshold`: `bit` takes the low side
/// `[0, threshold)`, `!bit` the high side `[threshold, BIT_SCALE)`. Shared
/// by [`Encoder::encode_bit`] and [`Decoder::decode_bit`], like [`narrow`].
fn threshold_range(bit: bool, threshold: u64) -> (u64, u64) {
    if bit {
        (0, threshold)
    } else {
        (threshold, BIT_SCALE)
    }
}

/// Splits `[0, BIT_SCALE)` in half: `bit` `0` takes the low half, `1` the
/// high half. Shared by [`Encoder::encode_bits`] and [`Decoder::decode_bits`],
/// like [`narrow`].
fn uniform_range(bit: u64) -> (u64, u64) {
    (bit * (BIT_SCALE / 2), (bit + 1) * (BIT_SCALE / 2))
}

/// Range-codes a byte stream from caller-supplied cumulative-frequency
/// ranges. Holds no model: [`Self::encode`] takes `[cum_low, cum_high)` out
/// of `total` on every call.
#[derive(Debug)]
pub struct Encoder {
    low: u64,
    high: u64,
    pending: u32,
    out: Vec<u8>,
    bit_buf: u8,
    bit_count: u8,
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder {
    /// A fresh coder over the full interval `[0, TOP)`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            low: 0,
            high: MASK,
            pending: 0,
            out: Vec::new(),
            bit_buf: 0,
            bit_count: 0,
        }
    }

    fn emit(&mut self, bit: u8) {
        self.bit_buf = (self.bit_buf << 1) | bit;
        self.bit_count += 1;
        if self.bit_count == 8 {
            self.out.push(self.bit_buf);
            self.bit_buf = 0;
            self.bit_count = 0;
        }
    }

    /// Emits `bit`, then the `pending` deferred bits it resolves, each its
    /// opposite: the carry-resolution step of both renormalization and
    /// [`Self::finish`].
    fn emit_resolved(&mut self, bit: u8) {
        self.emit(bit);
        for _ in 0..self.pending {
            self.emit(bit ^ 1);
        }
        self.pending = 0;
    }

    /// Narrows the coder's interval to the sub-range `[cum_low, cum_high)`
    /// out of `total`, then renormalizes, emitting whichever leading bits
    /// of the interval are now fixed (deferring bits made ambiguous by a
    /// possible future carry via `pending`, the standard range-coder
    /// underflow handling).
    ///
    /// Invariant callers must hold, the same trust boundary
    /// [`Decoder::decode`] extends to its own caller: `cum_low < cum_high
    /// <= total`, `total` fits in 32 bits.
    ///
    /// # Panics
    ///
    /// Panics if `total` is zero (integer division by zero).
    pub fn encode(&mut self, cum_low: u64, cum_high: u64, total: u64) {
        (self.low, self.high) = narrow(self.low, self.high, cum_low, cum_high, total);
        loop {
            if self.high < HALF {
                self.emit_resolved(0);
            } else if self.low >= HALF {
                self.emit_resolved(1);
                self.low -= HALF;
                self.high -= HALF;
            } else if self.low >= QUARTER && self.high < THREE_QUARTERS {
                self.pending += 1;
                self.low -= QUARTER;
                self.high -= QUARTER;
            } else {
                break;
            }
            self.low = (self.low << 1) & MASK;
            self.high = ((self.high << 1) | 1) & MASK;
        }
    }

    /// Encodes the low `bits` bits of `value`, most-significant first, each
    /// at a fixed 50/50 probability: the residual bits below an
    /// adaptively-coded length or offset bucket.
    pub fn encode_bits(&mut self, value: u32, bits: u32) {
        for k in (0..bits).rev() {
            let bit = u64::from((value >> k) & 1);
            let (cum_low, cum_high) = uniform_range(bit);
            self.encode(cum_low, cum_high, BIT_SCALE);
        }
    }

    /// Codes `bit` at `probability_of_one` (the caller's estimate that the
    /// bit is `1`), quantized into a fixed-point threshold: the primitive an
    /// [`crate::sse::Sse`]-calibrated binary decision needs.
    pub fn encode_bit(&mut self, bit: bool, probability_of_one: f64) {
        let threshold = quantize_probability(probability_of_one);
        let (cum_low, cum_high) = threshold_range(bit, threshold);
        self.encode(cum_low, cum_high, BIT_SCALE);
    }

    /// Flushes the final interval and any deferred carry bits, pads the
    /// last byte with zero bits, and returns the coded stream.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        self.pending += 1;
        self.emit_resolved(u8::from(self.low >= QUARTER));
        while self.bit_count != 0 {
            self.emit(0);
        }
        self.out
    }
}

/// Reads a byte stream coded by [`Encoder`], driven by the same ranges in the
/// same order.
#[derive(Debug)]
pub struct Decoder<'a> {
    low: u64,
    high: u64,
    value: u64,
    data: &'a [u8],
    bit_pos: usize,
}

impl<'a> Decoder<'a> {
    /// Starts decoding `data`, priming the coder's window with its first 32
    /// bits.
    ///
    /// Never panics on short input: a stream under 32 bits reads as implicit
    /// trailing zero bits, so truncation degrades to garbage output, not a
    /// crash (hard rule 2).
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        let mut decoder = Self {
            low: 0,
            high: MASK,
            value: 0,
            data,
            bit_pos: 0,
        };
        for _ in 0..32 {
            let bit = decoder.next_bit();
            decoder.value = (decoder.value << 1) | u64::from(bit);
        }
        decoder
    }

    /// Reads one bit from `data`, MSB first within each byte; `0` past the
    /// end.
    fn next_bit(&mut self) -> u8 {
        let byte_index = self.bit_pos >> 3;
        let bit = if byte_index < self.data.len() {
            (self.data[byte_index] >> (7 - (self.bit_pos & 7))) & 1
        } else {
            0
        };
        self.bit_pos += 1;
        bit
    }

    /// Maps the coder's current value into `[0, total)`: the cumulative
    /// frequency the caller's model looks up to find which symbol was
    /// coded, before confirming the match with [`Self::decode`].
    ///
    /// # Panics
    ///
    /// Panics if `total` is zero (integer division by zero).
    #[must_use]
    pub fn target(&self, total: u64) -> u64 {
        let range = self.high - self.low + 1;
        ((self.value - self.low + 1) * total - 1) / range
    }

    /// Consumes the sub-range `[cum_low, cum_high)` out of `total` the
    /// caller determined from [`Self::target`], mirroring
    /// [`Encoder::encode`]'s narrowing and renormalization bit-for-bit.
    ///
    /// # Panics
    ///
    /// Panics if `total` is zero (integer division by zero).
    pub fn decode(&mut self, cum_low: u64, cum_high: u64, total: u64) {
        (self.low, self.high) = narrow(self.low, self.high, cum_low, cum_high, total);
        loop {
            if self.high >= HALF {
                let offset = if self.low >= HALF {
                    HALF
                } else if self.low >= QUARTER && self.high < THREE_QUARTERS {
                    QUARTER
                } else {
                    break;
                };
                self.low -= offset;
                self.high -= offset;
                self.value -= offset;
            }
            self.low = (self.low << 1) & MASK;
            self.high = ((self.high << 1) | 1) & MASK;
            let bit = self.next_bit();
            self.value = ((self.value << 1) | u64::from(bit)) & MASK;
        }
    }

    /// Decodes `bits` bits written by [`Encoder::encode_bits`], most
    /// significant first.
    #[must_use]
    pub fn decode_bits(&mut self, bits: u32) -> u32 {
        let mut value = 0u32;
        for _ in 0..bits {
            let target = self.target(BIT_SCALE);
            let bit = u32::from(target >= BIT_SCALE / 2);
            let (cum_low, cum_high) = uniform_range(u64::from(bit));
            self.decode(cum_low, cum_high, BIT_SCALE);
            value = (value << 1) | bit;
        }
        value
    }

    /// Inverse of [`Encoder::encode_bit`]: resolves which side of the same
    /// threshold the coder's value falls on, consumes that sub-range, and
    /// returns the coded bit. Never panics on adversarial `self` state:
    /// [`Self::target`] is bounded to `[0, total)` whatever bytes produced it.
    #[must_use]
    pub fn decode_bit(&mut self, probability_of_one: f64) -> bool {
        let threshold = quantize_probability(probability_of_one);
        let bit = self.target(BIT_SCALE) < threshold;
        let (cum_low, cum_high) = threshold_range(bit, threshold);
        self.decode(cum_low, cum_high, BIT_SCALE);
        bit
    }
}

/// Quantizes `probability_of_one` into a [`BIT_SCALE`]-wide integer
/// threshold: bit `1` occupies `[0, threshold)`, bit `0` occupies
/// `[threshold, BIT_SCALE)`. Clamped to `1..=BIT_SCALE - 1` so neither
/// outcome is ever assigned zero width, mirroring
/// [`crate::model::Model`]'s "nothing is ever impossible to code".
/// [`Encoder::encode_bit`] and [`Decoder::decode_bit`] both call this with
/// identical inputs and only `+ - * /`, [`f64::clamp`], and rounding, no libm
/// transcendental, so both sides compute the same threshold bit-for-bit
/// (ADR-0024's determinism rule).
fn quantize_probability(probability_of_one: f64) -> u64 {
    #[allow(
        clippy::cast_precision_loss,
        reason = "BIT_SCALE (2^16) is exact in f64, well inside its 53-bit mantissa"
    )]
    let scale = BIT_SCALE as f64;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "probability_of_one.clamp(0.0, 1.0) * scale is in [0.0, scale]: rounds to a \
                  value that always fits u64, and a NaN input (clamp leaves NaN as NaN, per its \
                  docs) casts to 0 under Rust's defined float-to-int cast semantics, which the \
                  clamp below still bounds away from 0 and BIT_SCALE"
    )]
    let scaled = (probability_of_one.clamp(0.0, 1.0) * scale).round() as u64;
    scaled.clamp(1, BIT_SCALE - 1)
}

#[cfg(test)]
mod tests;

// Sweeps arbitrary alphabets and symbol streams where `mod tests` has one
// `roundtrip*` example per shape, through `mod tests`' own `roundtrip_symbols`.
// Not under Miri: interpretation costs 300-5000x per case on this
// crate (measured, issue #456), the storm multiplies that by its case
// count, and the deterministic example tests already walk the same
// paths for UB observation.
#[cfg(test)]
#[cfg(not(miri))]
mod proptests;
