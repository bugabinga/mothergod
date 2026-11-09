use super::*;
use crate::coder::{Decoder, Encoder};
use crate::sse::MIN_PROBABILITY;

/// Literal runs split by copies, the shape a parse hands the literal
/// coder: text, a 16-bit ramp the alignment and two-back experts key on,
/// and every byte value once.
fn runs() -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let text = b"the literal coder sees what the parse leaves behind ".repeat(4);
    let ramp: Vec<u8> = (0u16..300).flat_map(|s| (s * 37).to_le_bytes()).collect();
    let every: Vec<u8> = (0..=255).collect();
    let copies = vec![b"the ".to_vec(), vec![0x41], Vec::new()];
    (vec![text, ramp, every], copies)
}

#[test]
fn encode_then_decode_round_trips_literal_runs_between_copies() {
    let (literals, copies) = runs();
    let mut model = BitMix::new();
    let mut context = Context::default();
    let mut encoder = Encoder::new();
    for (run, copy) in literals.iter().zip(&copies) {
        for &byte in run {
            let symbol = usize::from(byte);
            let landed = model.walk(context, |mid, p| {
                let bit = symbol >= mid;
                encoder.encode_bit(bit, p);
                bit
            });
            assert_eq!(landed, byte, "the walk lands on the byte its bits spell");
            context = context.after_literal(byte);
        }
        context = context.after_copy(copy);
    }
    let stream = encoder.finish();

    let mut model = BitMix::new();
    let mut context = Context::default();
    let mut decoder = Decoder::new(&stream);
    for (run, copy) in literals.iter().zip(&copies) {
        let landed: Vec<u8> = run
            .iter()
            .map(|_| {
                let byte = model.walk(context, |_mid, p| decoder.decode_bit(p));
                context = context.after_literal(byte);
                byte
            })
            .collect();
        assert_eq!(&landed, run);
        context = context.after_copy(copy);
    }
}

#[test]
fn walk_lands_on_every_byte_value_with_each_probability_inside_the_clamp() {
    let mut model = BitMix::new();
    let mut context = Context::default();
    for byte in 0..=255u8 {
        let symbol = usize::from(byte);
        let landed = model.walk(context, |mid, p| {
            assert!(
                (MIN_PROBABILITY..=1.0 - MIN_PROBABILITY).contains(&p),
                "coded probability {p} left the clamp"
            );
            symbol >= mid
        });
        assert_eq!(landed, byte);
        context = context.after_literal(byte);
    }
}

#[allow(
    clippy::disallowed_methods,
    reason = "test oracle: the cost of a decision, which no stream depends on"
)]
fn cost_bits(bit: bool, p: f64) -> f64 {
    -(if bit { p } else { 1.0 - p }).log2()
}

#[test]
fn a_byte_repeated_under_one_context_gets_cheaper() {
    let mut model = BitMix::new();
    let context = Context {
        prev1: b'q',
        prev2: b'e',
        position: 8,
        after_copy: false,
        word_hash: 0,
    };
    let symbol = usize::from(b'u');
    let mut price = || {
        let mut bits = 0.0;
        model.walk(context, |mid, p| {
            let bit = symbol >= mid;
            bits += cost_bits(bit, p);
            bit
        });
        bits
    };
    let first = price();
    let later = (0..40).map(|_| price()).last().expect("forty prices");
    assert!(
        first > 6.0,
        "a fresh model prices a byte near 8 bits: {first}"
    );
    assert!(
        later < 1.0,
        "forty repeats leave the byte under a bit: {later}"
    );
}

#[test]
fn apm_position_keeps_both_bins_inside_the_row_at_the_clamp_edges() {
    for p in [MIN_PROBABILITY, 0.5, 1.0 - MIN_PROBABILITY] {
        let (lower, fraction) = apm_position(p);
        assert!(lower + 1 < APM_BINS, "p {p} read past the row at {lower}");
        assert!((0.0..1.0).contains(&fraction), "p {p} fraction {fraction}");
    }
    assert_eq!(apm_position(0.5), (16, 0.0), "one half is the middle bin");
}

#[test]
fn a_fresh_apm_row_maps_a_probability_to_itself_within_interpolation_error() {
    let row = fresh_apm_row();
    for p in [0.01, 0.2, 0.5, 0.7, 0.99] {
        let (lower, fraction) = apm_position(p);
        let calibrated = row[lower] * (1.0 - fraction) + row[lower + 1] * fraction;
        assert!(
            (calibrated - p).abs() < 0.02,
            "p {p} calibrated {calibrated}"
        );
    }
}
