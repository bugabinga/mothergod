//! Adaptive range coder: [`Encoder`] and [`Decoder`], the arithmetic-coding
//! primitive the entropy models (`JOURNAL` S2-D2) will drive.
//!
//! Ported from the archive's `Enc`/`Dec`
//! (`research/imports/session-1/mothergod.rs`), not the code, per ADR-0006:
//! same 32-bit interval, same three renormalization cases (top-half fixed,
//! bottom-half fixed, straddling the middle with carry deferred via
//! `pending`), same byte-oriented bit packer. This slice is the coder
//! alone, driven directly by caller-supplied cumulative-frequency ranges;
//! [`crate::model::Model`] is the order-0 adaptive frequency table that
//! supplies those ranges from real data. The six-expert `Lit` literal mixer
//! is a separate, larger slice still to come.
//! [`Decoder`] already treats a stream shorter than the coder expects as
//! implicit trailing zero bits rather than panicking (hard rule 2,
//! `CLAUDE.md`): the archive's own decoder relied on the same behavior.

/// Width of the coder's `[low, high]` interval, in bits. `u64` arithmetic
/// throughout keeps every intermediate (`range * total`) from overflowing
/// as long as `total` also fits in 32 bits, the same bound the archive's
/// frequency tables (`LIM`, `research/imports/session-1/mothergod.rs`)
/// stay far under.
const TOP: u64 = 1 << 32;
const HALF: u64 = TOP >> 1;
const QUARTER: u64 = TOP >> 2;
const THREE_QUARTERS: u64 = 3 * (TOP >> 2);
const MASK: u64 = TOP - 1;

/// Uniform-probability scale [`Encoder::encode_bits`]/[`Decoder::decode_bits`]
/// use to code raw bits with no model: each bit is symbol 0 or 1 out of this
/// total, i.e. exactly 50/50. Used for the residual bits below an
/// adaptively-coded length/offset bucket.
const BIT_SCALE: u64 = 1 << 16;

/// Range-codes a byte stream from a sequence of caller-supplied
/// cumulative-frequency ranges.
///
/// Holds no model of its own: [`Self::encode`] takes `[cum_low, cum_high)`
/// out of `total` on every call, so any adaptive frequency table (or a
/// fixed one) can drive it.
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
        let range = self.high - self.low + 1;
        self.high = self.low + range * cum_high / total - 1;
        self.low += range * cum_low / total;
        loop {
            if self.high < HALF {
                self.emit(0);
                for _ in 0..self.pending {
                    self.emit(1);
                }
                self.pending = 0;
            } else if self.low >= HALF {
                self.emit(1);
                for _ in 0..self.pending {
                    self.emit(0);
                }
                self.pending = 0;
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
    /// at a fixed 50/50 probability (no model). For residual bits below an
    /// adaptively-coded bucket, e.g. a match length's low bits once its
    /// bucket symbol is coded.
    pub fn encode_bits(&mut self, value: u32, bits: u32) {
        for k in (0..bits).rev() {
            let bit = u64::from((value >> k) & 1);
            self.encode(
                bit * (BIT_SCALE / 2),
                (bit + 1) * (BIT_SCALE / 2),
                BIT_SCALE,
            );
        }
    }

    /// Flushes the final interval and any deferred carry bits, pads the
    /// last byte with zero bits, and returns the coded stream.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        self.pending += 1;
        if self.low < QUARTER {
            self.emit(0);
            for _ in 0..self.pending {
                self.emit(1);
            }
        } else {
            self.emit(1);
            for _ in 0..self.pending {
                self.emit(0);
            }
        }
        while self.bit_count != 0 {
            self.emit(0);
        }
        self.out
    }
}

/// Reads a byte stream coded by [`Encoder`], resolving cumulative-frequency
/// targets from caller-supplied ranges the same way `Encoder::encode` was
/// driven, symbol by symbol in lockstep.
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
    /// Never panics on short input: a stream under 32 bits reads as
    /// implicit trailing zero bits, the same past-the-end behavior used
    /// throughout decoding (hard rule 2, `CLAUDE.md`) — adversarial
    /// truncation degrades to garbage output, not a crash.
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

    /// Reads one bit from `data`, MSB first within each byte. Past the end
    /// of `data`, reads as `0` rather than panicking or erroring: the
    /// standard range-coder convention of treating a truncated stream as
    /// padded with zero bits.
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
    /// [`Encoder::encode`]'s narrowing and renormalization bit-for-bit so
    /// encoder and decoder stay in lockstep.
    ///
    /// # Panics
    ///
    /// Panics if `total` is zero (integer division by zero).
    pub fn decode(&mut self, cum_low: u64, cum_high: u64, total: u64) {
        let range = self.high - self.low + 1;
        self.high = self.low + range * cum_high / total - 1;
        self.low += range * cum_low / total;
        loop {
            let shift = if self.high < HALF {
                true
            } else if self.low >= HALF {
                self.low -= HALF;
                self.high -= HALF;
                self.value -= HALF;
                true
            } else if self.low >= QUARTER && self.high < THREE_QUARTERS {
                self.low -= QUARTER;
                self.high -= QUARTER;
                self.value -= QUARTER;
                true
            } else {
                false
            };
            if !shift {
                break;
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
            self.decode(
                u64::from(bit) * (BIT_SCALE / 2),
                u64::from(bit + 1) * (BIT_SCALE / 2),
                BIT_SCALE,
            );
            value = (value << 1) | bit;
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal order-0 adaptive frequency table: just enough to drive
    /// [`Encoder`]/[`Decoder`] round-trip tests without depending on the
    /// real entropy models this coder exists to support (still to come,
    /// `JOURNAL` S2-D2). Mirrors the shape the archive's own `Model` uses
    /// (`research/imports/session-1/mothergod.rs`): a frequency per
    /// symbol, updated after every code.
    struct FreqTable {
        freq: Vec<u32>,
        total: u32,
    }

    impl FreqTable {
        fn new(symbols: usize) -> Self {
            Self {
                freq: vec![1; symbols],
                total: u32::try_from(symbols).expect("test alphabets are tiny"),
            }
        }

        fn range(&self, symbol: usize) -> (u64, u64, u64) {
            let low: u32 = self.freq[..symbol].iter().sum();
            (
                u64::from(low),
                u64::from(low + self.freq[symbol]),
                u64::from(self.total),
            )
        }

        fn find(&self, target: u64) -> usize {
            let mut symbol = 0;
            let mut cum = 0u64;
            while cum + u64::from(self.freq[symbol]) <= target {
                cum += u64::from(self.freq[symbol]);
                symbol += 1;
            }
            symbol
        }

        fn update(&mut self, symbol: usize) {
            self.freq[symbol] += 8;
            self.total += 8;
        }
    }

    fn roundtrip_symbols(symbols: &[usize], alphabet: usize) {
        let mut model = FreqTable::new(alphabet);
        let mut enc = Encoder::new();
        for &s in symbols {
            let (lo, hi, tot) = model.range(s);
            enc.encode(lo, hi, tot);
            model.update(s);
        }
        let bytes = enc.finish();

        let mut model = FreqTable::new(alphabet);
        let mut dec = Decoder::new(&bytes);
        let mut got = Vec::with_capacity(symbols.len());
        for _ in symbols {
            let target = dec.target(u64::from(model.total));
            let s = model.find(target);
            let (lo, hi, tot) = model.range(s);
            dec.decode(lo, hi, tot);
            model.update(s);
            got.push(s);
        }
        assert_eq!(got, symbols);
    }

    fn mask_for(bits: u32) -> u32 {
        if bits == 0 {
            0
        } else if bits >= 32 {
            u32::MAX
        } else {
            (1u32 << bits) - 1
        }
    }

    #[test]
    fn empty_stream_round_trips() {
        roundtrip_symbols(&[], 4);
    }

    #[test]
    fn single_symbol_round_trips() {
        roundtrip_symbols(&[0], 2);
    }

    #[test]
    fn skewed_frequencies_round_trip() {
        // Symbol 0 dominates: exercises the near-degenerate intervals that
        // drive the HALF/QUARTER renormalization branches hardest.
        let symbols: Vec<usize> = (0..500).map(|i| usize::from(i % 17 == 0)).collect();
        roundtrip_symbols(&symbols, 2);
    }

    #[test]
    fn full_alphabet_cycles_round_trip() {
        let symbols: Vec<usize> = (0..2000).map(|i| i % 256).collect();
        roundtrip_symbols(&symbols, 256);
    }

    #[test]
    fn pseudo_random_symbols_round_trip() {
        // xorshift32: deterministic, no external RNG dependency.
        let mut state = 0x1234_5678u32;
        let mut symbols = Vec::with_capacity(5000);
        for _ in 0..5000 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            symbols.push((state % 32) as usize);
        }
        roundtrip_symbols(&symbols, 32);
    }

    #[test]
    fn raw_bits_round_trip() {
        let values = [
            (0u32, 0u32),
            (1, 1),
            (0, 8),
            (0xFF, 8),
            (0xDEAD_BEEF, 32),
            (5, 3),
        ];
        let mut enc = Encoder::new();
        for &(v, n) in &values {
            enc.encode_bits(v, n);
        }
        let bytes = enc.finish();

        let mut dec = Decoder::new(&bytes);
        for &(v, n) in &values {
            assert_eq!(dec.decode_bits(n), v & mask_for(n));
        }
    }

    #[test]
    fn mixed_symbols_and_raw_bits_round_trip() {
        // A length/offset model interleaves an adaptively-coded bucket
        // symbol with fixed-probability residual bits; this is that shape.
        let plan = [(3usize, 7u32, 0b101_1010u32), (0, 0, 0), (16, 16, 0xFFFF)];
        let mut model = FreqTable::new(17);
        let mut enc = Encoder::new();
        for &(sym, bits, val) in &plan {
            let (lo, hi, tot) = model.range(sym);
            enc.encode(lo, hi, tot);
            model.update(sym);
            enc.encode_bits(val, bits);
        }
        let bytes = enc.finish();

        let mut model = FreqTable::new(17);
        let mut dec = Decoder::new(&bytes);
        for &(sym, bits, val) in &plan {
            let target = dec.target(u64::from(model.total));
            let s = model.find(target);
            assert_eq!(s, sym);
            let (lo, hi, tot) = model.range(s);
            dec.decode(lo, hi, tot);
            model.update(s);
            assert_eq!(dec.decode_bits(bits), val & mask_for(bits));
        }
    }

    #[test]
    fn decoding_truncated_stream_does_not_panic() {
        let symbols: Vec<usize> = (0..200).map(|i| i % 5).collect();
        let mut model = FreqTable::new(5);
        let mut enc = Encoder::new();
        for &s in &symbols {
            let (lo, hi, tot) = model.range(s);
            enc.encode(lo, hi, tot);
            model.update(s);
        }
        let bytes = enc.finish();
        let truncated = &bytes[..bytes.len() / 2];

        let mut model = FreqTable::new(5);
        let mut dec = Decoder::new(truncated);
        for _ in &symbols {
            let target = dec.target(u64::from(model.total));
            let s = model.find(target);
            let (lo, hi, tot) = model.range(s);
            dec.decode(lo, hi, tot);
            model.update(s);
        }
        // No panic is the assertion: decoded symbols past the real data are
        // whatever implicit-zero bits produce, never treated as ground
        // truth here.
    }
}
