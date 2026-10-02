use super::*;

pub(super) fn roundtrip_symbols(symbols: &[usize], alphabet_len: usize) {
    let mut model = Model::new(alphabet_len);
    let mut enc = Encoder::new();
    for &s in symbols {
        model.encode(&mut enc, s);
    }
    let bytes = enc.finish();

    let mut model = Model::new(alphabet_len);
    let mut dec = Decoder::new(&bytes);
    let got: Vec<usize> = symbols.iter().map(|_| model.decode(&mut dec)).collect();
    assert_eq!(got, symbols);
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
    // Symbol 0 dominates: exercises the near-degenerate coder
    // intervals hardest, same shape as coder.rs's own coverage but
    // now driven by the real adaptive table, not a test stand-in.
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
fn rescale_triggers_and_round_trip_still_holds() {
    // DEFAULT_RESCALE_LIMIT is 65536 and every update adds
    // DEFAULT_RESCALE_INCREMENT=12, so a two-symbol alphabet coded
    // 10,000 times crosses the halving threshold several times over;
    // round-trip must still hold on the far side of every halving.
    let symbols: Vec<usize> = (0..10_000).map(|i| usize::from(i % 3 == 0)).collect();
    roundtrip_symbols(&symbols, 2);
}

#[test]
fn independent_models_interleave_on_one_coder() {
    // The real use this type exists for (JOURNAL S2-D2): a flag model
    // and a length model, each with their own alphabet and state,
    // alternating on the same coder stream. Each model instance must
    // only ever see its own symbols, never the other's.
    let flags = [1usize, 0, 0, 1, 1, 0, 1];
    let lengths = [3usize, 15, 0, 7, 2, 9, 1];

    let mut flag_model = Model::new(2);
    let mut length_model = Model::new(16);
    let mut enc = Encoder::new();
    for (&flag, &len) in flags.iter().zip(lengths.iter()) {
        flag_model.encode(&mut enc, flag);
        length_model.encode(&mut enc, len);
    }
    let bytes = enc.finish();

    let mut flag_model = Model::new(2);
    let mut length_model = Model::new(16);
    let mut dec = Decoder::new(&bytes);
    for (&flag, &len) in flags.iter().zip(lengths.iter()) {
        assert_eq!(flag_model.decode(&mut dec), flag);
        assert_eq!(length_model.decode(&mut dec), len);
    }
}

#[test]
fn ideal_cost_matches_fresh_table_uniform_distribution() {
    // A fresh 4-symbol table starts uniform (every freq is 1, total 4),
    // so every symbol's ideal cost is exactly -log2(1/4) = 2 bits.
    let mut model = Model::new(4);
    assert!((model.ideal_cost_bits(0) - 2.0).abs() < 1e-9);
}

#[test]
fn ideal_cost_drops_as_a_symbol_gets_more_likely() {
    // Coding the same symbol repeatedly raises its own frequency
    // (DEFAULT_RESCALE_INCREMENT), so its ideal cost must strictly
    // decrease call over call as the table adapts toward it.
    let mut model = Model::new(4);
    let first = model.ideal_cost_bits(0);
    let second = model.ideal_cost_bits(0);
    let third = model.ideal_cost_bits(0);
    assert!(second < first);
    assert!(third < second);
}

#[test]
fn ideal_cost_updates_state_same_as_encode() {
    // ideal_cost_bits must leave the table in the same state encode
    // would have: fork two identical tables, drive one through each
    // path over the same symbols, then confirm they agree from here by
    // coding one more symbol on top of each and comparing cost.
    let symbols = [0usize, 1, 0, 2, 0, 1, 3, 0];
    let mut via_encode = Model::new(4);
    let mut enc = Encoder::new();
    for &s in &symbols {
        via_encode.encode(&mut enc, s);
    }
    let mut via_ideal_cost = Model::new(4);
    for &s in &symbols {
        let _ = via_ideal_cost.ideal_cost_bits(s);
    }
    assert!((via_encode.ideal_cost_bits(2) - via_ideal_cost.ideal_cost_bits(2)).abs() < 1e-9);
}

#[test]
fn ideal_cost_sum_tracks_real_encoded_length() {
    // Named corpus (CLAUDE.md hard rule 4): 5000 pseudo-random symbols
    // over a 32-wide alphabet, the same fixture
    // pseudo_random_symbols_round_trip above uses. Summed ideal cost is
    // an estimate, not the real coder's bit-exact output (integer
    // cumulative-frequency division rounds; the coder also pays a
    // handful of flush bits at the very end), so this checks closeness,
    // not equality — the same tolerance shape as literal.rs's vendored
    // `exp` accuracy check (ADR-0024).
    let symbols: Vec<usize> = crate::test_support::Xorshift32::new(0x1234_5678)
        .take(5000)
        .map(|state| (state % 32) as usize)
        .collect();

    let mut ideal_cost_model = Model::new(32);
    let ideal_bits: f64 = symbols
        .iter()
        .map(|&s| ideal_cost_model.ideal_cost_bits(s))
        .sum();

    let mut real_model = Model::new(32);
    let mut enc = Encoder::new();
    for &s in &symbols {
        real_model.encode(&mut enc, s);
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "encoded length is far below f64's exact integer range (2^53)"
    )]
    let real_bits = (enc.finish().len() * 8) as f64;

    let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
    assert!(
        relative_diff <= 0.01,
        "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
         {relative_diff:.4} relative difference exceeds the 1% budget"
    );
}

#[test]
fn decoding_truncated_stream_does_not_panic() {
    let symbols: Vec<usize> = (0..200).map(|i| i % 5).collect();
    let mut model = Model::new(5);
    let mut enc = Encoder::new();
    for &s in &symbols {
        model.encode(&mut enc, s);
    }
    let bytes = enc.finish();
    let truncated = &bytes[..bytes.len() / 2];

    let mut model = Model::new(5);
    let mut dec = Decoder::new(truncated);
    for _ in &symbols {
        let _ = model.decode(&mut dec);
    }
    // No panic is the assertion: decoded symbols past the real data
    // are whatever implicit-zero bits produce, never treated as
    // ground truth here.
}
