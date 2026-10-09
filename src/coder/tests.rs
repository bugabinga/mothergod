use super::*;

/// Minimal order-0 adaptive frequency table: just enough to drive
/// [`Encoder`]/[`Decoder`] round-trip tests without depending on the
/// real entropy models this coder supports ([`crate::model::Model`],
/// [`crate::literal::Literal`], `JOURNAL` S2-D2). Mirrors the shape the
/// archive's own `Model` uses (`research/imports/session-1/mothergod.rs`):
/// a frequency per symbol, updated after every code.
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

pub(super) fn roundtrip_symbols(symbols: &[usize], alphabet: usize) {
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
    let symbols: Vec<usize> = crate::test_support::Xorshift32::new(0x1234_5678)
        .take(5000)
        .map(|state| (state % 32) as usize)
        .collect();
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

#[test]
fn encode_bit_round_trips_across_a_range_of_probabilities() {
    let plan = [
        (true, 0.5),
        (false, 0.5),
        (true, 0.99),
        (true, 0.01),
        (false, 0.99),
        (false, 0.01),
        (true, 0.999_999),
        (false, 0.000_001),
    ];
    let mut enc = Encoder::new();
    for &(bit, p) in &plan {
        enc.encode_bit(bit, p);
    }
    let bytes = enc.finish();

    let mut dec = Decoder::new(&bytes);
    for &(bit, p) in &plan {
        assert_eq!(dec.decode_bit(p), bit);
    }
}

#[test]
fn encode_bit_round_trips_the_unlikely_outcome() {
    // The expensive but load-bearing case: a bit coded against a
    // probability that says it is almost impossible must still decode
    // exactly, not just the likely bit at the same probability.
    let mut enc = Encoder::new();
    enc.encode_bit(false, 0.999);
    let bytes = enc.finish();
    let mut dec = Decoder::new(&bytes);
    assert!(!dec.decode_bit(0.999));
}

#[test]
fn encode_bit_out_of_range_probability_clamps_not_panics() {
    let mut enc = Encoder::new();
    enc.encode_bit(true, 2.0);
    enc.encode_bit(false, -1.0);
    let bytes = enc.finish();

    let mut dec = Decoder::new(&bytes);
    assert!(dec.decode_bit(2.0));
    assert!(!dec.decode_bit(-1.0));
}

#[test]
fn encode_bit_at_a_skewed_probability_costs_far_fewer_bits_than_fixed_50_50() {
    // 2000 bits, true 99% of the time: coding them at the matching
    // skewed probability should compress far below encode_bits' fixed,
    // unmodeled 50/50 split on the same sequence.
    let bits: Vec<bool> = crate::test_support::Xorshift32::new(0x0BAD_F00D)
        .take(2000)
        .map(|state| state % 100 != 0)
        .collect();

    let mut skewed = Encoder::new();
    for &bit in &bits {
        skewed.encode_bit(bit, 0.99);
    }
    let skewed_bytes = skewed.finish();

    let mut fixed = Encoder::new();
    for &bit in &bits {
        fixed.encode_bits(u32::from(bit), 1);
    }
    let fixed_bytes = fixed.finish();

    assert!(
        skewed_bytes.len() < fixed_bytes.len() / 4,
        "skewed {} bytes should be far below fixed-50/50 {} bytes for a 99%-true sequence",
        skewed_bytes.len(),
        fixed_bytes.len()
    );
}

/// Codes each `(cum_low, cum_high, total)` range, then checks the decoder's
/// `target` lands inside the range it was coded from.
fn roundtrip_ranges(ranges: &[(u64, u64, u64)]) {
    let mut enc = Encoder::new();
    for &(lo, hi, tot) in ranges {
        enc.encode(lo, hi, tot);
    }
    let bytes = enc.finish();

    let mut dec = Decoder::new(&bytes);
    for &(lo, hi, tot) in ranges {
        let target = dec.target(tot);
        assert!((lo..hi).contains(&target), "{target} outside {lo}..{hi}");
        dec.decode(lo, hi, tot);
    }
}

/// A binary tail after the boundary symbol: it reads the interval the
/// boundary symbol left behind, so a coder that renormalized it wrongly
/// desyncs here.
const BINARY_TAIL: [(u64, u64, u64); 6] = [
    (0, 1, 2),
    (1, 2, 2),
    (1, 2, 2),
    (0, 1, 2),
    (0, 1, 2),
    (1, 2, 2),
];

fn roundtrip_boundary_then_tail(boundary: (u64, u64, u64)) {
    let mut ranges = vec![boundary];
    ranges.extend_from_slice(&BINARY_TAIL);
    roundtrip_ranges(&ranges);
}

/// The first symbol narrows `[0, MASK]` to `[0, HALF]`: `high` equals
/// `HALF` exactly, so the interval still straddles the midpoint and no
/// leading bit is fixed. Renormalizing it as "top half empty"
/// (`high <= HALF`) emits a 0 that `HALF` itself contradicts.
#[test]
fn interval_ending_exactly_at_half_is_not_renormalized() {
    roundtrip_boundary_then_tail((0, HALF + 1, MASK));
}

/// The first symbol narrows `[0, MASK]` to `[QUARTER, THREE_QUARTERS]`:
/// `high` equals `THREE_QUARTERS` exactly, one past the middle-straddle
/// test's range, so the underflow shift must not fire. The mutated
/// comparison (`<=`) shifts, and the decoder desyncs from the encoder.
#[test]
fn interval_ending_exactly_at_three_quarters_is_not_shifted() {
    roundtrip_boundary_then_tail((QUARTER, THREE_QUARTERS + 1, MASK));
}
