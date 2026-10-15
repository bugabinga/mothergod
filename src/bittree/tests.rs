use super::*;

/// A uniform table: every symbol carries mass 1, matching
/// [`crate::coder::Encoder::encode_bits`]'s fixed 50/50 split at
/// every level.
fn uniform_table() -> Vec<u64> {
    (0..=ALPHABET as u64).collect()
}

/// A geometrically skewed table: symbol `s` gets mass `2^(255 - s)`
/// (clamped so symbol 255 still keeps mass 1), heavily favoring low
/// symbol values — the shape a real adaptive model converges toward
/// on repetitive data.
fn skewed_table() -> Vec<u64> {
    let mut cum = vec![0u64; ALPHABET + 1];
    let mut acc = 0u64;
    for symbol in 0..ALPHABET {
        let shift = (ALPHABET - 1 - symbol).min(40);
        acc += 1u64 << shift;
        cum[symbol + 1] = acc;
    }
    cum
}

#[test]
#[should_panic(expected = "exactly ALPHABET + 1")]
fn wrong_length_table_panics() {
    let mut enc = Encoder::new();
    encode_symbol_sse(&mut enc, &[0, 1, 2], 0, &mut Sse::new(SSE_CONTEXTS));
}

#[test]
#[should_panic(expected = "strictly increasing")]
fn non_increasing_table_panics() {
    let mut cum = uniform_table();
    cum[5] = cum[4];
    let mut enc = Encoder::new();
    encode_symbol_sse(&mut enc, &cum, 4, &mut Sse::new(SSE_CONTEXTS));
}

#[test]
fn ideal_cost_matches_the_direct_symbol_cost() {
    // Chain-rule identity the module docs name, checked directly:
    // the product of the 8 binary decision probabilities along a
    // symbol's path must equal the direct
    // (cum[symbol+1]-cum[symbol])/cum[ALPHABET] ratio, so their
    // -log2 costs must match to near float precision.
    let cum = skewed_table();
    for symbol in 0..=u8::MAX {
        #[allow(
            clippy::cast_precision_loss,
            clippy::disallowed_methods,
            reason = "test-only oracle diffed against the bit-tree's own decomposition; \
                      cum entries are bounded well under 2^53, and no bitstream depends on \
                      this log2 call"
        )]
        let direct = {
            let num = (cum[usize::from(symbol) + 1] - cum[usize::from(symbol)]) as f64;
            let den = cum[ALPHABET] as f64;
            -(num / den).log2()
        };
        let mut decomposed = 0.0;
        walk_nodes(|_depth, _prefix, lo, mid, hi| {
            let bit = usize::from(symbol) >= mid;
            decomposed += ideal_cost_bit(bit, upper_half_probability(&cum, lo, hi));
            bit
        });
        assert!(
            (direct - decomposed).abs() < 1e-9,
            "symbol {symbol}: direct {direct} bits vs decomposed {decomposed} bits"
        );
    }
}

/// `#810`: pins [`ideal_cost_bit`]'s log2 cost formula against
/// directly-chosen `p` values, independent of the bit-tree walk that
/// otherwise makes `p` itself hard to hand-derive.
#[test]
fn ideal_cost_bit_matches_hand_computed_log2_cost() {
    // -log2(0.5) = 1.0, both branches.
    assert!((ideal_cost_bit(true, 0.5) - 1.0).abs() < 1e-12);
    assert!((ideal_cost_bit(false, 0.5) - 1.0).abs() < 1e-12);
    // -log2(0.25) = 2.0: bit = true reads p directly, bit = false
    // reads 1 - p, so p = 0.75 exercises the same value through the
    // subtraction instead.
    assert!((ideal_cost_bit(true, 0.25) - 2.0).abs() < 1e-12);
    assert!((ideal_cost_bit(false, 0.75) - 2.0).abs() < 1e-12);
}

#[test]
fn real_sse_coded_length_tracks_sse_ideal_cost_within_a_few_percent() {
    // Summed ideal cost is an estimate, not the real coder's bit-exact
    // output (8 chained 16-bit quantized encode_bit calls per symbol plus
    // flush bits), so this checks closeness, not equality.
    let cum = skewed_table();
    let symbols: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED_CAFE)
        .take(5000)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();

    let mut cost_sse = Sse::new(SSE_CONTEXTS);
    let ideal_bits: f64 = symbols
        .iter()
        .map(|&s| ideal_cost_bits_sse(&cum, s, &mut cost_sse))
        .sum();

    let mut coder_sse = Sse::new(SSE_CONTEXTS);
    let mut enc = Encoder::new();
    for &symbol in &symbols {
        encode_symbol_sse(&mut enc, &cum, symbol, &mut coder_sse);
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "encoded length is far below f64's exact integer range (2^53)"
    )]
    let real_bits = (enc.finish().len() * 8) as f64;

    let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
    assert!(
        relative_diff <= 0.05,
        "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
         {relative_diff:.4} relative difference exceeds the 5% budget"
    );
}

#[test]
fn sse_context_is_a_bijection_onto_0_sse_contexts() {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for depth in 0..LEVELS {
        for prefix in 0..(1usize << depth) {
            let context = sse_context(depth, prefix);
            assert!(
                context < SSE_CONTEXTS,
                "depth={depth}, prefix={prefix}: context {context} must be < {SSE_CONTEXTS}"
            );
            assert!(
                seen.insert(context),
                "depth={depth}, prefix={prefix}: context {context} collides with an earlier pair"
            );
        }
    }
    assert_eq!(
        seen.len(),
        SSE_CONTEXTS,
        "every one of the {SSE_CONTEXTS} contexts must be reachable"
    );
}

#[test]
fn sse_context_along_one_symbol_path_visits_eight_distinct_nodes() {
    // sse_context's documented "prefix = lo / width" identity, exercised
    // against walk_nodes's own walk (not asserted in isolation): every
    // symbol's root-to-leaf path must visit LEVELS distinct tree nodes,
    // one per depth, since a real Sse calibration wired behind this walk
    // must never conflate two different decisions under one context.
    for symbol in 0..=u8::MAX {
        let symbol = usize::from(symbol);
        let mut lo = 0usize;
        let mut hi = ALPHABET;
        let mut path = Vec::with_capacity(LEVELS as usize);
        for depth in 0..LEVELS {
            let width = hi - lo;
            let prefix = lo / width;
            path.push(sse_context(depth, prefix));
            let mid = lo + width / 2;
            if symbol >= mid {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let distinct: std::collections::HashSet<_> = path.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            LEVELS as usize,
            "symbol {symbol}: path {path:?} must visit {LEVELS} distinct contexts"
        );
    }
}

#[test]
#[should_panic(expected = "depth must be < LEVELS")]
fn sse_context_depth_out_of_range_panics() {
    let _ = sse_context(LEVELS, 0);
}

#[test]
#[should_panic(expected = "prefix must be")]
fn sse_context_prefix_out_of_range_panics() {
    let _ = sse_context(2, 4);
}

#[test]
fn sse_context_count_is_255() {
    assert_eq!(SSE_CONTEXTS, 255);
}

#[test]
fn every_symbol_round_trips_through_sse_on_a_uniform_table() {
    let cum = uniform_table();
    for symbol in 0..=u8::MAX {
        let mut enc_sse = Sse::new(SSE_CONTEXTS);
        let mut enc = Encoder::new();
        encode_symbol_sse(&mut enc, &cum, symbol, &mut enc_sse);
        let encoded = enc.finish();
        let mut dec_sse = Sse::new(SSE_CONTEXTS);
        let mut dec = Decoder::new(&encoded);
        assert_eq!(decode_symbol_sse(&mut dec, &cum, &mut dec_sse), symbol);
    }
}

#[test]
fn every_symbol_round_trips_through_sse_on_a_skewed_table() {
    let cum = skewed_table();
    for symbol in 0..=u8::MAX {
        let mut enc_sse = Sse::new(SSE_CONTEXTS);
        let mut enc = Encoder::new();
        encode_symbol_sse(&mut enc, &cum, symbol, &mut enc_sse);
        let encoded = enc.finish();
        let mut dec_sse = Sse::new(SSE_CONTEXTS);
        let mut dec = Decoder::new(&encoded);
        assert_eq!(decode_symbol_sse(&mut dec, &cum, &mut dec_sse), symbol);
    }
}

#[test]
fn a_sequence_of_symbols_round_trips_through_sse_over_one_stream() {
    let cum = skewed_table();
    let symbols: Vec<u8> = crate::test_support::Xorshift32::new(0xB17_7EEE)
        .take(2000)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();

    let mut sse = Sse::new(SSE_CONTEXTS);
    let mut enc = Encoder::new();
    for &symbol in &symbols {
        encode_symbol_sse(&mut enc, &cum, symbol, &mut sse);
    }
    let encoded = enc.finish();

    let mut sse = Sse::new(SSE_CONTEXTS);
    let mut dec = Decoder::new(&encoded);
    let decoded: Vec<u8> = symbols
        .iter()
        .map(|_| decode_symbol_sse(&mut dec, &cum, &mut sse))
        .collect();
    assert_eq!(decoded, symbols);
}

#[test]
fn boundary_symbols_zero_and_max_round_trip() {
    let cum = skewed_table();
    for &symbol in &[0u8, u8::MAX] {
        let mut enc = Encoder::new();
        encode_symbol_sse(&mut enc, &cum, symbol, &mut Sse::new(SSE_CONTEXTS));
        let encoded = enc.finish();
        let mut dec = Decoder::new(&encoded);
        let decoded = decode_symbol_sse(&mut dec, &cum, &mut Sse::new(SSE_CONTEXTS));
        assert_eq!(decoded, symbol);
    }
}
