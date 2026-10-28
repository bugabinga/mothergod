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
    // Rate::DEFAULT's limit is 65536 and every update adds its
    // increment, 12, so a two-symbol alphabet coded
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

#[test]
#[should_panic(expected = "Model alphabet must be non-empty")]
fn new_over_no_symbols_panics() {
    let _ = Model::new(0);
}

// `try_new` reaches the assert through `try_filled_vec`, `new` through
// `vec!`: two paths into one assert, so each is pinned.
#[test]
#[should_panic(expected = "Model alphabet must be non-empty")]
fn try_new_over_no_symbols_panics() {
    let _ = Model::try_new(0);
}
