use super::*;

#[test]
fn copy_tokens_interleave_with_literals_round_trip() {
    // The shape Method-wiring will actually drive this with: literal
    // runs broken up by simulated LZ copy tokens, each shifting
    // `after_copy` and re-deriving `prev1`/`prev2` from the copied
    // bytes rather than from `after_literal`'s single-byte update.
    let mut model = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let mut context = Context::default();
    let mut enc = Encoder::new();
    let literal_runs: &[&[u8]] = &[b"hello ", b"world", b" repeat repeat repeat"];
    let copy_runs: &[&[u8]] = &[b"repeat repeat", b"o", b""];
    for (lits, copy) in literal_runs.iter().zip(copy_runs.iter()) {
        for &b in *lits {
            model.encode_logit_sse(&mut enc, context, b, &mut mixer);
            context = context.after_literal(b);
        }
        context = context.after_copy(copy);
    }
    let encoded = enc.finish();
    let total_literals: usize = literal_runs.iter().map(|r| r.len()).sum();

    let mut model = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let mut context = Context::default();
    let mut dec = Decoder::new(&encoded);
    let mut got = Vec::with_capacity(total_literals);
    for (lits, copy) in literal_runs.iter().zip(copy_runs.iter()) {
        for _ in *lits {
            let b = model.decode_logit_sse(&mut dec, context, &mut mixer);
            context = context.after_literal(b);
            got.push(b);
        }
        context = context.after_copy(copy);
    }
    let expected: Vec<u8> = literal_runs
        .iter()
        .flat_map(|r| r.iter().copied())
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn context_after_literal_tracks_previous_bytes_and_position() {
    let context = Context::default();
    let context = context.after_literal(b'a');
    assert_eq!(context.prev1, b'a');
    assert_eq!(context.prev2, 0);
    assert_eq!(context.position, 1);
    assert!(!context.after_copy);
    let context = context.after_literal(b'b');
    assert_eq!(context.prev1, b'b');
    assert_eq!(context.prev2, b'a');
    assert_eq!(context.position, 2);
}

#[test]
fn context_after_copy_of_zero_bytes_keeps_previous_bytes() {
    let context = Context::default().after_literal(b'a').after_literal(b'b');
    let after = context.after_copy(&[]);
    assert_eq!(after.prev1, context.prev1);
    assert_eq!(after.prev2, context.prev2);
    assert_eq!(after.position, context.position);
    assert!(after.after_copy);
}

#[test]
fn context_after_copy_of_one_byte_shifts_prev1_into_prev2() {
    let context = Context::default().after_literal(b'a');
    let after = context.after_copy(b"z");
    assert_eq!(after.prev1, b'z');
    assert_eq!(after.prev2, b'a');
    assert_eq!(after.position, 2);
}

#[test]
fn context_after_copy_of_many_bytes_uses_last_two() {
    let after = Context::default().after_copy(b"hello");
    assert_eq!(after.prev1, b'o');
    assert_eq!(after.prev2, b'l');
    assert_eq!(after.position, 5);
}

#[test]
fn word_hash_extends_on_alnum_and_resets_on_punctuation() {
    let hash_a = advance_word_hash(0, b'a');
    let hash_ab = advance_word_hash(hash_a, b'b');
    assert_ne!(hash_a, 0);
    assert_ne!(hash_ab, hash_a);
    assert_eq!(advance_word_hash(hash_ab, b' '), 0);
    assert_eq!(advance_word_hash(hash_ab, b'.'), 0);
}

#[test]
fn ideal_cost_drops_as_a_byte_gets_more_likely() {
    // Coding the same byte repeatedly raises its own frequency across
    // every expert bank it touches, so its ideal cost must strictly
    // decrease call over call as the model adapts toward it. Context
    // stabilizes after the first call (prev1 becomes 'a' and stays
    // there), so this isolates the adaptation, not a context change.
    let mut model = Literal::new();
    let context = Context::default().after_literal(b'a');
    let first = model.ideal_cost_bits(context, b'a');
    let second = model.ideal_cost_bits(context, b'a');
    let third = model.ideal_cost_bits(context, b'a');
    assert!(second < first);
    assert!(third < second);
}

#[test]
fn ideal_cost_bits_sse_drops_as_a_byte_gets_more_likely() {
    // Same shape as ideal_cost_drops_as_a_byte_gets_more_likely, for
    // the SSE-calibrated path.
    let mut model = Literal::new();
    let context = Context::default().after_literal(b'a');
    let first = model.ideal_cost_bits_sse(context, b'a');
    let second = model.ideal_cost_bits_sse(context, b'a');
    let third = model.ideal_cost_bits_sse(context, b'a');
    assert!(second < first);
    assert!(third < second);
}

/// `f64::exp`, the pre-ADR-0024 reference this test diffs `exp`
/// against. `#[cfg(test)]`-gated, so it never reaches the decode
/// path this crate ships.
fn reference_exp(x: f64) -> f64 {
    #[allow(
        clippy::disallowed_methods,
        reason = "test-only oracle for ADR-0024's 1% accuracy claim (issue #161); #[cfg(test)] keeps it off the decode path"
    )]
    {
        x.exp()
    }
}

#[test]
fn vendored_exp_keeps_bits_per_byte_within_one_percent_of_f64_exp() {
    // Named corpus (CLAUDE.md hard rule 4): the founding session's
    // archived codec, real structured Rust source, 25,524 bytes.
    let corpus: &[u8] = include_bytes!("../../research/imports/session-1/mothergod.rs");

    let encoded_len = |exp_fn: fn(f64) -> f64| -> usize {
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in corpus {
            let (bank_indices, weight_index, cum) = model.banks_and_cum(context);
            let symbol = usize::from(b);
            enc.encode(cum[symbol], cum[symbol + 1], cum[ALPHABET]);
            model.update(&bank_indices, weight_index, symbol, exp_fn);
            context = context.after_literal(b);
        }
        enc.finish().len()
    };

    let vendored_bytes = encoded_len(exp);
    let reference_bytes = encoded_len(reference_exp);

    #[allow(
        clippy::cast_precision_loss,
        reason = "encoded length is far below f64's exact integer range (2^53)"
    )]
    let relative_diff =
        (vendored_bytes as f64 - reference_bytes as f64).abs() / reference_bytes as f64;

    assert!(
        relative_diff <= 0.01,
        "vendored exp: {vendored_bytes} bytes vs f64::exp reference: {reference_bytes} \
         bytes, {relative_diff:.4} relative difference exceeds the 1% budget (ADR-0024)"
    );
}

#[test]
fn column_expert_state_new_starts_at_the_laplace_floor() {
    let state = ColumnExpertState::new(crate::test_support::nz(4));
    assert_eq!(state.freq.len(), 4 * ALPHABET);
    assert!(state.freq.iter().all(|&f| f == 1));
    assert_eq!(state.total, vec![ALPHABET_U32; 4]);
    assert_eq!(state.weight, vec![1.0; WEIGHT_CONTEXTS]);
}

/// [`ColumnExpertState::try_new`] must produce the exact same state
/// [`ColumnExpertState::new`] does: the real decode path's fallible
/// constructor is not a second, independently-written source of the
/// same starting state ([`Self::try_new`]'s own docs).
#[test]
fn column_expert_state_try_new_matches_new() {
    let via_new = ColumnExpertState::new(crate::test_support::nz(4));
    let via_try_new = ColumnExpertState::try_new(crate::test_support::nz(4)).unwrap();
    assert_eq!(via_new.freq, via_try_new.freq);
    assert_eq!(via_new.total, via_try_new.total);
    assert_eq!(via_new.weight, via_try_new.weight);
    assert_eq!(
        format!("{:?}", via_new.sse),
        format!("{:?}", via_try_new.sse)
    );
}

/// Round-trips bytes through [`Literal::encode_column`]/`decode_column`
/// under a fixed `column_bank` per byte (`columns` cycles through
/// `banks`), the real-wiring counterpart of `roundtrip_bytes_sse`
/// above.
fn roundtrip_bytes_column(bytes: &[u8], banks: usize) {
    let mut model = Literal::new();
    let mut column_state = ColumnExpertState::new(crate::test_support::nz(banks));
    let mut context = Context::default();
    let mut enc = Encoder::new();
    for (i, &b) in bytes.iter().enumerate() {
        model.encode_column(&mut enc, context, b, i % banks, &mut column_state);
        context = context.after_literal(b);
    }
    let encoded = enc.finish();

    let mut model = Literal::new();
    let mut column_state = ColumnExpertState::new(crate::test_support::nz(banks));
    let mut context = Context::default();
    let mut dec = Decoder::new(&encoded);
    let mut got = Vec::with_capacity(bytes.len());
    for i in 0..bytes.len() {
        let b = model.decode_column(&mut dec, context, i % banks, &mut column_state);
        context = context.after_literal(b);
        got.push(b);
    }
    assert_eq!(got, bytes);
}

#[test]
fn empty_stream_round_trips_through_column_expert() {
    roundtrip_bytes_column(&[], 4);
}

#[test]
fn single_byte_round_trips_through_column_expert() {
    roundtrip_bytes_column(b"x", 4);
}

#[test]
fn tabular_columns_round_trip_through_column_expert() {
    // The shape this path targets (research/JOURNAL.md S1-P5): fixed-
    // width records, each column cycling through its own small period,
    // the same class tests/golden/v11-tabular-columns pins.
    let columns = 8;
    let rows: Vec<u8> = (0..600u32)
        .map(|i| u8::try_from((i * 7 + i / u32::try_from(columns).unwrap()) % 251).unwrap())
        .collect();
    roundtrip_bytes_column(&rows, columns);
}

#[test]
fn pseudo_random_bytes_round_trip_through_column_expert() {
    let bytes: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
        .take(5000)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();
    roundtrip_bytes_column(&bytes, 5);
}

#[test]
fn decoding_truncated_stream_does_not_panic_through_column_expert() {
    let bytes: Vec<u8> = (0..200).map(|i| u8::try_from(i % 5).unwrap()).collect();
    let banks = 4;
    let mut model = Literal::new();
    let mut column_state = ColumnExpertState::new(crate::test_support::nz(banks));
    let mut context = Context::default();
    let mut enc = Encoder::new();
    for (i, &b) in bytes.iter().enumerate() {
        model.encode_column(&mut enc, context, b, i % banks, &mut column_state);
        context = context.after_literal(b);
    }
    let encoded = enc.finish();
    let truncated = &encoded[..encoded.len() / 2];

    let mut model = Literal::new();
    let mut column_state = ColumnExpertState::new(crate::test_support::nz(banks));
    let mut context = Context::default();
    let mut dec = Decoder::new(truncated);
    for i in 0..bytes.len() {
        let b = model.decode_column(&mut dec, context, i % banks, &mut column_state);
        context = context.after_literal(b);
    }
    // No panic is the assertion, same as decoding_truncated_stream_does_not_panic.
}

/// [`Literal::encode_column`] must leave the six real experts' weights
/// exactly where [`Literal::ideal_cost_bits`] would, per the layering
/// [`Literal::encode_column`]'s own docs claim: `update_column_expert`
/// touches only `column_state`, never `self.weights`/`self.freq`.
#[test]
fn encode_column_updates_the_six_real_experts_same_as_ideal_cost_bits() {
    let bytes = b"hello world hello again";
    let mut via_column = Literal::new();
    let mut column_state = ColumnExpertState::new(crate::test_support::nz(4));
    let mut context_column = Context::default();
    let mut enc_column = Encoder::new();
    for &b in bytes {
        via_column.encode_column(&mut enc_column, context_column, b, 2, &mut column_state);
        context_column = context_column.after_literal(b);
    }

    let mut via_ideal_cost = Literal::new();
    let mut context_ideal_cost = Context::default();
    for &b in bytes {
        let _ = via_ideal_cost.ideal_cost_bits(context_ideal_cost, b);
        context_ideal_cost = context_ideal_cost.after_literal(b);
    }

    assert_eq!(via_column.freq, via_ideal_cost.freq);
    assert_eq!(via_column.total, via_ideal_cost.total);
    assert_eq!(via_column.weights, via_ideal_cost.weights);
}

/// #705: `update_column_expert`'s own probability estimate,
/// `column_state.freq[column_bank * ALPHABET + symbol] / column_total`,
/// read from a freq slot the `+`/`/` mutants at the `*` cannot reach:
/// bank 2's slot for `symbol` gets a frequency no other bank's slot for
/// `symbol` shares, so a wrong index reads back the untouched Laplace
/// floor instead and adapts the weight to a value other than the one
/// computed here from the intended index's estimate.
#[test]
fn update_column_expert_estimates_from_bank_times_alphabet_plus_symbol() {
    let model = Literal::new();
    let context = Context::default();
    let (bank_indices, weight_index) = banks(context);
    let column_bank = 2;
    let symbol = 5;

    let mut column_state = ColumnExpertState::new(crate::test_support::nz(3));
    column_state.freq[column_bank * ALPHABET + symbol] = 100;
    let w7 = column_state.weight[weight_index];
    let (weights6, weight_sum) = model.weights6_and_sum(weight_index, w7);
    let column_total = f64::from(column_state.total[column_bank]);
    let column_estimate =
        f64::from(column_state.freq[column_bank * ALPHABET + symbol]) / column_total;
    let expected_weight = model.adapt_seventh_weight(
        &bank_indices,
        &weights6,
        weight_sum,
        symbol,
        w7,
        column_estimate,
    );

    model.update_column_expert(
        &bank_indices,
        weight_index,
        symbol,
        column_bank,
        &mut column_state,
    );

    assert!((column_state.weight[weight_index] - expected_weight).abs() < 1e-12);
}

/// `research/JOURNAL.md` S1-P3: the pair's baseline side is
/// `Self::ideal_cost_bits` verbatim
/// (`Self::ideal_cost_bits_ppm_expert_pair`'s own docs), the same
/// claim S1-P5's `column_expert_pair_baseline_matches_plain_ideal_cost_bits`
/// made for `ideal_cost_bits_column_expert_pair`.
#[test]
fn ppm_expert_pair_baseline_matches_plain_ideal_cost_bits() {
    let mut paired = Literal::new();
    let mut plain = Literal::new();
    let mut ppm_state = PpmExpertState::new();
    let mut context = Context::default();
    for &b in b"the quick brown fox jumps over the lazy dog" {
        let (baseline, _) = paired.ideal_cost_bits_ppm_expert_pair(context, b, &mut ppm_state);
        let expected = plain.ideal_cost_bits(context, b);
        assert!(
            (baseline - expected).abs() < 1e-9,
            "byte {b:?}: paired baseline {baseline} vs plain {expected}"
        );
        context = context.after_literal(b);
    }
}

#[test]
fn ppm_expert_pair_updates_only_its_own_ppm_bank() {
    let mut model = Literal::new();
    let mut ppm_state = PpmExpertState::new();
    // prev1 = 0x25: high nibble 2, so bank_of must select bank 2.
    let context = Context {
        prev1: 0x25,
        ..Context::default()
    };
    let bank = PpmExpertState::bank_of(context);
    assert_eq!(bank, 2);

    let symbol = usize::from(b'x');
    let _ = model.ideal_cost_bits_ppm_expert_pair(context, b'x', &mut ppm_state);

    assert!(
        !ppm_state.tables[bank].is_escape(symbol),
        "the observed bank must clear its own escape flag for this symbol"
    );
    for other in 0..PPM_EXPERT_BANKS {
        if other != bank {
            assert!(
                ppm_state.tables[other].is_escape(symbol),
                "bank {other} must stay untouched"
            );
        }
    }
}

#[test]
fn ppm_expert_pair_costs_stay_finite_and_positive() {
    let mut model = Literal::new();
    let mut ppm_state = PpmExpertState::new();
    let mut context = Context::default();
    for &b in b"0123456789abcdefghijklmnopqrstuvwxyz" {
        let (baseline, with_ppm) =
            model.ideal_cost_bits_ppm_expert_pair(context, b, &mut ppm_state);
        assert!(
            baseline.is_finite() && baseline > 0.0,
            "baseline={baseline}"
        );
        assert!(
            with_ppm.is_finite() && with_ppm > 0.0,
            "with_ppm={with_ppm}"
        );
        context = context.after_literal(b);
    }
}

/// #697: a concrete `(weight, weight_sum, probability)` triple whose
/// expected fixed-point result distinguishes `/` from `%`/`*` in the
/// normalization step and `*` from `+`/`/` in either multiplication,
/// so every arithmetic-operator mutant in the function produces a
/// different `u64`.
#[test]
fn fixed_point_contribution_from_probability_matches_the_normalized_product() {
    assert_eq!(
        fixed_point_contribution_from_probability(2.0, 4.0, 0.5),
        1_073_741_824
    );
}

/// #697: `mix_ppm`'s own accumulation step, isolated from
/// [`Literal::six_expert_mixed`] and [`fixed_point_contribution_from_probability`]
/// (each already proven correct on their own): recomputes symbol 0's
/// `mixed` total from those same building blocks with the intended
/// `+=`/`>>`/`+1` operators and checks `mix_ppm`'s own first cumulative
/// entry against it, so a swap of any of those three operators inside
/// `mix_ppm` itself is visible even though the sub-computations it
/// calls are unchanged.
#[test]
fn mix_ppm_first_entry_matches_the_six_expert_mix_plus_ppm_contribution() {
    let model = Literal::new();
    let mut ppm_state = PpmExpertState::new();
    let context = Context::default();
    let (bank_indices, weight_index) = banks(context);
    let ppm_bank = PpmExpertState::bank_of(context);

    // Give the PPM table's own estimate for symbol 0 real weight, so
    // its contribution is not the degenerate zero every fresh table
    // starts every symbol at.
    for _ in 0..8 {
        ppm_state.tables[ppm_bank].observe(0);
    }

    let w7 = ppm_state.weight[weight_index];
    let (weights6, weight_sum) = model.weights6_and_sum(weight_index, w7);
    let scale6 = model.scale6(&bank_indices, &weights6, weight_sum);
    let mut expected_mixed = model.six_expert_mixed(&bank_indices, &scale6, 0);
    expected_mixed += fixed_point_contribution_from_probability(
        w7,
        weight_sum,
        ppm_state.tables[ppm_bank].probability(0),
    );
    let expected_first_entry = (expected_mixed >> 16) + 1;

    let cum = model.mix_ppm(&bank_indices, weight_index, ppm_bank, &ppm_state);
    assert_eq!(cum[1], expected_first_entry);
}

/// `#783`: pins the gradient step's arithmetic against hand-computed
/// values (`error = bit - p`, `weight += rate * error * stretched`),
/// independent of the bit-tree walk, so a mutated `-`, `+=`, or `*`
/// in [`logistic_gradient_step`] lands on a wrong number instead of
/// surviving under the walk's chained, hard-to-hand-check state.
#[test]
fn logistic_gradient_step_matches_hand_computed_error_and_delta() {
    let stretched = [1.0, -1.0, 2.0, 0.0, 0.5, -0.25];

    // bit = true: error = 1.0 - p = 0.7, delta = rate * error * s.
    let mut weights = [0.0; EXPERTS];
    logistic_gradient_step(&mut weights, &stretched, 0.1, true, 0.3);
    let expected = [0.07, -0.07, 0.14, 0.0, 0.035, -0.0175];
    for (got, want) in weights.iter().zip(&expected) {
        assert!((got - want).abs() < 1e-12, "{weights:?}");
    }

    // bit = false: error = 0.0 - p = -0.4, delta = rate * error * s,
    // the opposite sign from the bit = true case above.
    let mut weights = [0.0; EXPERTS];
    logistic_gradient_step(&mut weights, &stretched, 0.2, false, 0.4);
    let expected = [-0.08, 0.08, -0.16, 0.0, -0.04, 0.02];
    for (got, want) in weights.iter().zip(&expected) {
        assert!((got - want).abs() < 1e-12, "{weights:?}");
    }

    // Nonzero starting weights: `+=` accumulates onto them rather
    // than replacing or subtracting.
    let mut weights = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    logistic_gradient_step(&mut weights, &[1.0; EXPERTS], 0.05, true, 0.25);
    let expected = [1.0375, 2.0375, 3.0375, 4.0375, 5.0375, 6.0375];
    for (got, want) in weights.iter().zip(&expected) {
        assert!((got - want).abs() < 1e-12, "{weights:?}");
    }
}

/// `#810`: pins [`logistic_mix_node`]'s per-expert probability and dot
/// product against directly-chosen prefix sums, independent of the
/// bit-tree walk, so a mutated `-`, `/`, or `+=` there lands on a wrong
/// number instead of surviving under the walk's chained state.
#[test]
fn logistic_mix_node_matches_hand_computed_stretch_and_dot() {
    let (lo, mid, hi) = (0, 128, 256);
    // Six distinct upper-half probabilities, one per expert, chosen so
    // `sums[hi] - sums[mid]` and `sums[hi] - sums[lo]` are never equal
    // (ruling out a `-`/`/` mixup that happens to agree at p = 0.5).
    let p = [0.75, 0.25, 0.5, 0.875, 0.125, 0.625];
    let mut prefix = [[0u64; ALPHABET + 1]; EXPERTS];
    for (sums, &p) in prefix.iter_mut().zip(&p) {
        sums[hi] = 256;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mid_sum = (256.0 * (1.0 - p)) as u64;
        sums[mid] = mid_sum;
    }
    let weights = [0.1, -0.2, 0.3, 0.05, 0.4, -0.15];

    let expected_stretched: [f64; EXPERTS] =
        std::array::from_fn(|i| stretch(clamp_logistic_probability(p[i])));
    let expected_dot: f64 = weights
        .iter()
        .zip(&expected_stretched)
        .map(|(w, s)| w * s)
        .sum();

    let (stretched, dot) = logistic_mix_node(&prefix, &weights, lo, mid, hi);
    for (got, want) in stretched.iter().zip(&expected_stretched) {
        assert!((got - want).abs() < 1e-12, "{stretched:?}");
    }
    assert!((dot - expected_dot).abs() < 1e-12, "{dot} {expected_dot}");
}

#[test]
fn clamp_logistic_probability_bounds_both_tails_and_passes_the_middle() {
    assert!((clamp_logistic_probability(0.0) - LOGISTIC_PROBABILITY_FLOOR).abs() < 1e-18);
    assert!((clamp_logistic_probability(1.0) - (1.0 - LOGISTIC_PROBABILITY_FLOOR)).abs() < 1e-18);
    assert!((clamp_logistic_probability(0.3) - 0.3).abs() < 1e-18);
}

#[test]
fn surprise_rate_is_maximal_at_a_fresh_key_and_floor_at_a_converged_one() {
    // A fresh key (baseline not yet established) reads as maximum
    // surprise.
    assert!((surprise_rate(0.0, 0.0) - LOGISTIC_INITIAL_RATE).abs() < 1e-15);
    // `recent` tracking `baseline` (a converged, stable context) decays
    // to the floor regardless of the shared magnitude.
    for level in [1e-6, 0.01, 0.25, 1.0] {
        assert!((surprise_rate(level, level) - LOGISTIC_FLOOR_RATE).abs() < 1e-12);
    }
    // `recent` at twice `baseline` or more (this schedule's cap) reads
    // as maximum surprise again.
    assert!((surprise_rate(0.02, 0.01) - LOGISTIC_INITIAL_RATE).abs() < 1e-12);
    assert!((surprise_rate(0.05, 0.01) - LOGISTIC_INITIAL_RATE).abs() < 1e-12);
    // Monotonic between the two endpoints.
    let low = surprise_rate(0.010, 0.01);
    let mid = surprise_rate(0.015, 0.01);
    let high = surprise_rate(0.019, 0.01);
    assert!(low < mid && mid < high, "{low} {mid} {high}");
}

#[test]
fn surprise_ema_update_matches_hand_computed_blend() {
    // 0.9 * 0.1 + 0.1 * 0.5 = 0.09 + 0.05 = 0.14.
    assert!((surprise_ema_update(0.1, 0.5, 0.9) - 0.14).abs() < 1e-15);
    // decay = 0.0 replaces the previous value outright.
    assert!((surprise_ema_update(0.1, 0.5, 0.0) - 0.5).abs() < 1e-15);
    // decay = 1.0 never moves.
    assert!((surprise_ema_update(0.3, 0.9, 1.0) - 0.3).abs() < 1e-15);
}

/// `#810`: pins [`surprise_error_tracking_step`]'s error and error²
/// arithmetic against directly-chosen `bit`/`p` pairs, independent of
/// the bit-tree walk, comparing against [`surprise_ema_update`] (a
/// trusted, already-tested function) fed the hand-computed error².
#[test]
fn surprise_error_tracking_step_matches_hand_computed_error_sq() {
    // bit = true: error = 1.0 - 0.3 = 0.7, error_sq = 0.49.
    let mut recent = 0.2;
    let mut baseline = 0.1;
    surprise_error_tracking_step(&mut recent, &mut baseline, 0.8, true, 0.3);
    let expected_recent = surprise_ema_update(0.2, 0.49, 0.8);
    let expected_baseline = surprise_ema_update(0.1, 0.49, SURPRISE_BASELINE_DECAY);
    assert!((recent - expected_recent).abs() < 1e-15);
    assert!((baseline - expected_baseline).abs() < 1e-15);

    // bit = false: error = 0.0 - 0.3 = -0.3, error_sq = 0.09, the
    // opposite sign from the bit = true case but the same square.
    let mut recent = 0.2;
    let mut baseline = 0.1;
    surprise_error_tracking_step(&mut recent, &mut baseline, 0.8, false, 0.3);
    let expected_recent = surprise_ema_update(0.2, 0.09, 0.8);
    let expected_baseline = surprise_ema_update(0.1, 0.09, SURPRISE_BASELINE_DECAY);
    assert!((recent - expected_recent).abs() < 1e-15);
    assert!((baseline - expected_baseline).abs() < 1e-15);
}

#[test]
fn surprise_logistic_mix_logit_sse_new_starts_at_uniform_weights_and_zero_error_state() {
    let mix = SurpriseLogisticMixLogitSse::new();
    assert_eq!(
        mix.weights,
        vec![[LOGISTIC_INITIAL_WEIGHT; EXPERTS]; WEIGHT_CONTEXTS]
    );
    assert_eq!(mix.recent_sq_error, vec![0.0; WEIGHT_CONTEXTS]);
    assert_eq!(mix.baseline_sq_error, vec![0.0; WEIGHT_CONTEXTS]);
    assert_eq!(mix.sse.contexts(), bittree::SSE_CONTEXTS);
}

#[test]
fn ideal_cost_bits_logistic_surprise_logit_sse_steps_only_its_own_weight_key_once_per_node() {
    let mut model = Literal::new();
    let context = Context {
        prev1: 0x25,
        ..Context::default()
    };
    for _ in 0..4 {
        let _ = model.ideal_cost_bits_logistic_surprise_logit_sse(
            context,
            b'x',
            &mut SurpriseLogisticMixLogitSse::new(),
        );
    }
    let (_, weight_index) = banks(context);
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let _ = model.ideal_cost_bits_logistic_surprise_logit_sse(context, b'x', &mut mixer);
    for key in 0..WEIGHT_CONTEXTS {
        if key == weight_index {
            assert_ne!(mixer.recent_sq_error[key].to_bits(), 0.0f64.to_bits());
            assert_ne!(mixer.baseline_sq_error[key].to_bits(), 0.0f64.to_bits());
            assert_ne!(
                mixer.weights[key].map(f64::to_bits),
                [LOGISTIC_INITIAL_WEIGHT.to_bits(); EXPERTS]
            );
        } else {
            assert_eq!(
                mixer.recent_sq_error[key].to_bits(),
                0.0f64.to_bits(),
                "key {key}"
            );
            assert_eq!(
                mixer.baseline_sq_error[key].to_bits(),
                0.0f64.to_bits(),
                "key {key}"
            );
            assert_eq!(
                mixer.weights[key].map(f64::to_bits),
                [LOGISTIC_INITIAL_WEIGHT.to_bits(); EXPERTS]
            );
        }
    }
}

fn roundtrip_bytes_logit_sse(bytes: &[u8]) {
    let mut model = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let mut context = Context::default();
    let mut enc = Encoder::new();
    for &b in bytes {
        model.encode_logit_sse(&mut enc, context, b, &mut mixer);
        context = context.after_literal(b);
    }
    let encoded = enc.finish();

    let mut model = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let mut context = Context::default();
    let mut dec = Decoder::new(&encoded);
    let mut got = Vec::with_capacity(bytes.len());
    for _ in bytes {
        let b = model.decode_logit_sse(&mut dec, context, &mut mixer);
        context = context.after_literal(b);
        got.push(b);
    }
    assert_eq!(got, bytes);
}

#[test]
fn empty_stream_round_trips_through_logit_sse() {
    roundtrip_bytes_logit_sse(&[]);
}

#[test]
fn single_byte_round_trips_through_logit_sse() {
    roundtrip_bytes_logit_sse(b"x");
}

#[test]
fn skewed_repeat_round_trips_through_logit_sse() {
    roundtrip_bytes_logit_sse(&b"aaaaaaaaaaaaaaaaaaaaaaaaaab".repeat(20));
}

#[test]
fn full_alphabet_cycles_round_trip_through_logit_sse() {
    let bytes: Vec<u8> = (0..2000).map(|i| u8::try_from(i % 256).unwrap()).collect();
    roundtrip_bytes_logit_sse(&bytes);
}

#[test]
fn ascii_text_round_trips_through_logit_sse() {
    let text = b"the quick brown fox jumps over the lazy dog, again and again.".repeat(50);
    roundtrip_bytes_logit_sse(&text);
}

#[test]
fn pseudo_random_bytes_round_trip_through_logit_sse() {
    let bytes: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
        .take(5000)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();
    roundtrip_bytes_logit_sse(&bytes);
}

#[test]
fn decoding_truncated_stream_does_not_panic_through_logit_sse() {
    let bytes: Vec<u8> = (0..200).map(|i| u8::try_from(i % 5).unwrap()).collect();
    let mut model = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let mut context = Context::default();
    let mut enc = Encoder::new();
    for &b in &bytes {
        model.encode_logit_sse(&mut enc, context, b, &mut mixer);
        context = context.after_literal(b);
    }
    let encoded = enc.finish();
    let truncated = &encoded[..encoded.len() / 2];

    let mut model = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let mut context = Context::default();
    let mut dec = Decoder::new(truncated);
    for _ in &bytes {
        let b = model.decode_logit_sse(&mut dec, context, &mut mixer);
        context = context.after_literal(b);
    }
    // No panic is the assertion, same as decoding_truncated_stream_does_not_panic.
}

/// [`Literal::encode_logit_sse`] must leave the six real experts' banks
/// and linear weights exactly where [`Literal::ideal_cost_bits`] would, per
/// the layering [`Literal::encode_logit_sse`]'s mixer reads the banks but
/// never perturbs them or `self.weights`, the same guarantee
/// `encode_column_updates_the_six_real_experts_same_as_ideal_cost_bits`
/// checks for the seventh expert.
#[test]
fn encode_logit_sse_updates_the_six_real_experts_same_as_ideal_cost_bits() {
    let bytes = b"hello world hello again";
    let mut via_logit_sse = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let mut context_logit_sse = Context::default();
    let mut enc_logit_sse = Encoder::new();
    for &b in bytes {
        via_logit_sse.encode_logit_sse(&mut enc_logit_sse, context_logit_sse, b, &mut mixer);
        context_logit_sse = context_logit_sse.after_literal(b);
    }

    let mut via_ideal_cost = Literal::new();
    let mut context_ideal_cost = Context::default();
    for &b in bytes {
        let _ = via_ideal_cost.ideal_cost_bits(context_ideal_cost, b);
        context_ideal_cost = context_ideal_cost.after_literal(b);
    }

    assert_eq!(via_logit_sse.freq, via_ideal_cost.freq);
    assert_eq!(via_logit_sse.total, via_ideal_cost.total);
    assert_eq!(via_logit_sse.weights, via_ideal_cost.weights);
}

#[test]
fn encode_logit_sse_steps_only_its_own_weight_key_once_per_node() {
    let mut model = Literal::new();
    let mut mixer = SurpriseLogisticMixLogitSse::new();
    let context = Context {
        prev1: 0x25,
        ..Context::default()
    };
    // Give the banks a skew first, so the stretches (and therefore the
    // gradient step) are nonzero.
    for _ in 0..4 {
        model.encode_logit_sse(
            &mut Encoder::new(),
            context,
            b'x',
            &mut SurpriseLogisticMixLogitSse::new(),
        );
    }
    let (_, weight_index) = banks(context);
    model.encode_logit_sse(&mut Encoder::new(), context, b'x', &mut mixer);
    for key in 0..WEIGHT_CONTEXTS {
        if key == weight_index {
            assert_ne!(mixer.recent_sq_error[key].to_bits(), 0.0f64.to_bits());
            assert_ne!(mixer.baseline_sq_error[key].to_bits(), 0.0f64.to_bits());
            assert_ne!(
                mixer.weights[key].map(f64::to_bits),
                [LOGISTIC_INITIAL_WEIGHT.to_bits(); EXPERTS]
            );
        } else {
            assert_eq!(
                mixer.recent_sq_error[key].to_bits(),
                0.0f64.to_bits(),
                "key {key}"
            );
            assert_eq!(
                mixer.baseline_sq_error[key].to_bits(),
                0.0f64.to_bits(),
                "key {key}"
            );
            assert_eq!(
                mixer.weights[key].map(f64::to_bits),
                [LOGISTIC_INITIAL_WEIGHT.to_bits(); EXPERTS]
            );
        }
    }
}
