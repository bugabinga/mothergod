use super::*;

#[test]
#[should_panic(expected = "alphabet must be non-empty")]
fn zero_alphabet_panics() {
    let _ = Ppm::new(0);
}

#[test]
fn fresh_table_escapes_every_symbol_for_free() {
    let ppm = Ppm::new(4);
    for symbol in 0..4 {
        assert!(ppm.is_escape(symbol));
        assert_eq!(ppm.price_symbol(symbol), None);
    }
    assert!((ppm.price_escape() - 0.0).abs() < 1e-9);
    assert_eq!(ppm.distinct(), 0);
}

#[test]
fn observing_a_symbol_clears_its_own_escape_flag_only() {
    let mut ppm = Ppm::new(4);
    ppm.observe(1);
    assert!(!ppm.is_escape(1));
    assert!(ppm.is_escape(0));
    assert!(ppm.is_escape(2));
    assert!(ppm.is_escape(3));
    assert_eq!(ppm.distinct(), 1);
}

#[test]
fn distinct_counts_each_symbol_once_regardless_of_repeats() {
    let mut ppm = Ppm::new(4);
    ppm.observe(1);
    ppm.observe(1);
    ppm.observe(1);
    ppm.observe(2);
    assert_eq!(ppm.distinct(), 2);
}

#[test]
fn price_symbol_drops_as_it_is_observed_more() {
    let mut ppm = Ppm::new(4);
    ppm.observe(0);
    let first = ppm.price_symbol(0).unwrap();
    ppm.observe(0);
    let second = ppm.price_symbol(0).unwrap();
    ppm.observe(0);
    let third = ppm.price_symbol(0).unwrap();
    assert!(second < first, "second={second} first={first}");
    assert!(third < second, "third={third} second={second}");
}

#[test]
fn probability_is_zero_unobserved_and_the_freq_over_space_ratio_once_observed() {
    let mut ppm = Ppm::new(4);
    assert!((ppm.probability(0) - 0.0).abs() < 1e-12);

    ppm.observe(0);
    ppm.observe(1);
    // Both symbols observed once: freq[0] == freq[1] ==
    // DEFAULT_RESCALE_INCREMENT, total == 2 * that, distinct == 2.
    let increment = f64::from(crate::DEFAULT_RESCALE_INCREMENT);
    let expected = increment / (2.0 * increment + 2.0);
    assert!((ppm.probability(0) - expected).abs() < 1e-12);
}

#[test]
fn escape_price_rises_as_the_same_symbol_keeps_recurring() {
    // Method C: repeatedly observing one symbol without ever adding a
    // new one grows total while distinct stays at 1, so escape's own
    // share of the coding space shrinks and its price climbs — this
    // table becomes more confident it has seen everything relevant.
    let mut ppm = Ppm::new(4);
    ppm.observe(0);
    let first = ppm.price_escape();
    for _ in 0..20 {
        ppm.observe(0);
    }
    let later = ppm.price_escape();
    assert!(
        later > first,
        "later={later} first={first}: escape should get more expensive as one symbol \
         dominates uncontested"
    );
}

#[test]
fn escape_price_is_lower_when_observations_keep_introducing_new_symbols() {
    // Two tables, same observation count (20), different mix: one all
    // repeats of a single symbol (distinct stays 1, matching the
    // "rises" test above), the other all-new symbols every time
    // (distinct grows in lockstep with total). Method C's escape share
    // is distinct / (total + distinct); constant novelty keeps that
    // share far larger than a context that stopped discovering
    // anything new after its first symbol.
    let mut all_repeats = Ppm::new(64);
    for _ in 0..20 {
        all_repeats.observe(0);
    }

    let mut all_new = Ppm::new(64);
    for symbol in 0..20 {
        all_new.observe(symbol);
    }

    assert!(
        all_new.price_escape() < all_repeats.price_escape(),
        "all_new={} all_repeats={}: a context that keeps discovering new symbols should \
         escape more cheaply than one that stopped after its first",
        all_new.price_escape(),
        all_repeats.price_escape()
    );
}

#[test]
#[should_panic(expected = "never-observed symbol")]
fn encode_on_unseen_symbol_panics() {
    let mut ppm = Ppm::new(4);
    let mut enc = Encoder::new();
    ppm.encode(&mut enc, 0);
}

#[test]
#[should_panic(expected = "empty table")]
fn encode_escape_on_empty_table_panics() {
    let mut ppm = Ppm::new(4);
    let mut enc = Encoder::new();
    ppm.encode_escape(&mut enc);
}

#[test]
#[should_panic(expected = "empty table")]
fn decode_on_empty_table_panics() {
    let mut ppm = Ppm::new(4);
    let mut dec = Decoder::new(&[]);
    let _ = ppm.decode(&mut dec);
}

/// Round-trips a mixed sequence of real symbols and escapes: the
/// caller decides on the encode side whether a symbol is present via
/// `is_escape`, exactly the decision a future wired-in caller (a lower
/// order's coder) would make, and the decode side must recover both
/// which symbols were coded and which positions escaped.
#[test]
fn mixed_symbols_and_escapes_round_trip() {
    let alphabet_len = 8;
    // First occurrence of every symbol here is deliberately an escape
    // (nothing to code yet); later occurrences of 0..3 are real.
    let sequence = [0usize, 1, 2, 0, 1, 0, 3, 0, 1, 2];

    let mut ppm = Ppm::new(alphabet_len);
    let mut enc = Encoder::new();
    let mut expect_escape = Vec::new();
    for &symbol in &sequence {
        let escape = ppm.is_escape(symbol);
        expect_escape.push(escape);
        if escape {
            if ppm.distinct() > 0 {
                ppm.encode_escape(&mut enc);
            }
            ppm.observe(symbol);
        } else {
            ppm.encode(&mut enc, symbol);
        }
    }
    let bytes = enc.finish();

    let mut ppm = Ppm::new(alphabet_len);
    let mut dec = Decoder::new(&bytes);
    let mut got = Vec::new();
    for (&symbol, &escape) in sequence.iter().zip(expect_escape.iter()) {
        if escape {
            if ppm.distinct() > 0 {
                let decoded = ppm.decode(&mut dec);
                assert_eq!(decoded, None, "expected an escape decode");
            }
            ppm.observe(symbol);
            got.push(symbol);
        } else {
            let decoded = ppm.decode(&mut dec).expect("expected a real symbol decode");
            got.push(decoded);
        }
    }
    assert_eq!(got, sequence);
}

#[test]
fn rescale_triggers_and_round_trip_still_holds() {
    // DEFAULT_RESCALE_LIMIT is 65536 and every observe adds
    // DEFAULT_RESCALE_INCREMENT=12, so 10,000 repeats of one
    // already-known symbol crosses the halving
    // threshold several times over; round-trip must still hold on the
    // far side of every halving, and the symbol's frequency must never
    // decay back to zero (is_escape must stay false throughout).
    let mut ppm = Ppm::new(2);
    ppm.observe(0); // bootstrap: first occurrence is always a free escape
    let mut enc = Encoder::new();
    for _ in 0..10_000 {
        ppm.encode(&mut enc, 0);
        assert!(!ppm.is_escape(0));
    }
    let bytes = enc.finish();

    let mut ppm = Ppm::new(2);
    ppm.observe(0);
    let mut dec = Decoder::new(&bytes);
    for _ in 0..10_000 {
        assert_eq!(ppm.decode(&mut dec), Some(0));
    }
}
