use super::*;
use crate::coder::{Decoder, Encoder};

#[test]
#[should_panic(expected = "at least one context")]
fn zero_contexts_panics() {
    let _ = Sse::new(0);
}

#[test]
fn fresh_table_is_near_identity() {
    let sse = Sse::new(1);
    for tenth in 0..=10 {
        let p = f64::from(tenth) / 10.0;
        let refined = sse.refine(0, p);
        assert!(
            (refined - p).abs() < 0.02,
            "p={p}, refined={refined}, expected near-identity on a fresh table"
        );
    }
}

#[test]
fn output_is_always_clamped_away_from_extremes() {
    let mut sse = Sse::new(1);
    for _ in 0..10_000 {
        sse.update(0, 1.0, true);
    }
    let refined = sse.refine(0, 1.0);
    assert!(
        (MIN_PROBABILITY..1.0).contains(&refined),
        "refined={refined} must stay inside (0.0, 1.0) even after {} updates \
         all pushing toward 1.0",
        10_000
    );

    let mut sse = Sse::new(1);
    for _ in 0..10_000 {
        sse.update(0, 0.0, false);
    }
    let refined = sse.refine(0, 0.0);
    assert!(
        refined > 0.0 && refined <= MAX_PROBABILITY,
        "refined={refined} must stay inside (0.0, 1.0) even after {} updates \
         all pushing toward 0.0",
        10_000
    );
}

#[test]
fn converges_toward_the_true_observed_rate() {
    // The primary model is uninformative (always claims p=0.5), but
    // the true outcome rate at this context is 90%: a working SSE
    // stage must learn to correct the primary estimate toward 0.9,
    // which is exactly the systematic-bias correction S1-P1 is for.
    let mut sse = Sse::new(1);
    let rng = crate::test_support::Xorshift32::new(0xA5A5_5A5A);
    for state in rng.take(20_000) {
        let outcome = state % 10 != 0; // true 90% of the time
        sse.update(0, 0.5, outcome);
    }
    let refined = sse.refine(0, 0.5);
    assert!(
        (refined - 0.9).abs() < 0.03,
        "refined={refined}, expected convergence near the true rate 0.9"
    );
}

#[test]
fn contexts_adapt_independently() {
    let mut sse = Sse::new(2);
    for _ in 0..5000 {
        sse.update(0, 0.5, true);
        sse.update(1, 0.5, false);
    }
    let refined0 = sse.refine(0, 0.5);
    let refined1 = sse.refine(1, 0.5);
    assert!(
        refined0 > 0.8,
        "context 0 saw only true outcomes, refined={refined0}"
    );
    assert!(
        refined1 < 0.2,
        "context 1 saw only false outcomes, refined={refined1}"
    );
}

#[test]
fn refine_is_monotonic_in_input_probability_on_a_fresh_table() {
    let sse = Sse::new(1);
    let mut previous = sse.refine(0, 0.0);
    for hundredth in 1..=100 {
        let p = f64::from(hundredth) / 100.0;
        let refined = sse.refine(0, p);
        assert!(
            refined >= previous,
            "refine must be non-decreasing in p on an untrained table: \
             p={p}, refined={refined}, previous={previous}"
        );
        previous = refined;
    }
}

#[test]
fn out_of_range_probability_is_clamped_not_a_panic() {
    let sse = Sse::new(1);
    let low = sse.refine(0, -1.0);
    let high = sse.refine(0, 2.0);
    assert!((low - MIN_PROBABILITY).abs() < 1e-6);
    assert!((high - MAX_PROBABILITY).abs() < 1e-6);
}

#[test]
#[should_panic(expected = "context out of range")]
fn refine_out_of_range_context_panics() {
    let sse = Sse::new(2);
    let _ = sse.refine(2, 0.5);
}

#[test]
#[should_panic(expected = "context out of range")]
fn update_out_of_range_context_panics() {
    let mut sse = Sse::new(2);
    sse.update(2, 0.5, true);
}

#[test]
fn contexts_reports_the_constructed_count() {
    assert_eq!(Sse::new(5).contexts(), 5);
}

#[test]
fn calibrated_probability_round_trips_and_costs_less_than_a_fixed_split() {
    // The mechanism S1-P1 names as this primitive's reason to exist,
    // proven end to end: an uninformative primary estimate (constant
    // 0.5, same as converges_toward_the_true_observed_rate above) that
    // Sse calibrates toward a skewed context's true rate, fed through
    // coder::Encoder::encode_bit/Decoder::decode_bit instead of just
    // compared against refine()'s return value. Exercises the
    // primitive directly against the coder, independent of
    // crate::literal::Literal::encode_sse/decode_sse, this module doc's
    // "Wired" ADR-0038 path that later composed the same two pieces
    // into the real bitstream.
    let outcomes: Vec<bool> = crate::test_support::Xorshift32::new(0x5EED_5EED)
        .take(2000)
        .map(|state| state % 10 != 0) // true 90% of the time
        .collect();

    let mut sse = Sse::new(1);
    let mut enc = Encoder::new();
    for &outcome in &outcomes {
        let p = sse.refine(0, 0.5);
        enc.encode_bit(outcome, p);
        sse.update(0, 0.5, outcome);
    }
    let calibrated_bytes = enc.finish();

    let mut fixed = Encoder::new();
    for &outcome in &outcomes {
        fixed.encode_bits(u32::from(outcome), 1);
    }
    let fixed_bytes = fixed.finish();

    assert!(
        calibrated_bytes.len() < fixed_bytes.len() * 2 / 3,
        "Sse-calibrated {} bytes should be well below fixed-50/50 {} bytes for a \
         90%-skewed sequence",
        calibrated_bytes.len(),
        fixed_bytes.len()
    );

    let mut sse = Sse::new(1);
    let mut dec = Decoder::new(&calibrated_bytes);
    for &outcome in &outcomes {
        let p = sse.refine(0, 0.5);
        let bit = dec.decode_bit(p);
        assert_eq!(bit, outcome, "round-trip mismatch");
        sse.update(0, 0.5, bit);
    }
}

#[test]
#[should_panic(expected = "at least one context")]
fn logit_sse_zero_contexts_panics() {
    let _ = LogitSse::new(0);
}

#[test]
fn logit_sse_fresh_table_is_near_identity() {
    let sse = LogitSse::new(1);
    for tenth in 1..10 {
        // Excludes the extremes (0.0, 1.0): stretch-domain bins bunch
        // tightly there, so a fresh table's interpolation error is
        // largest at exactly the points Sse's own test also has
        // loosest tolerance for real signal, not a bug in this
        // candidate's identity fill.
        let p = f64::from(tenth) / 10.0;
        let refined = sse.refine(0, p);
        assert!(
            (refined - p).abs() < 0.02,
            "p={p}, refined={refined}, expected near-identity on a fresh table"
        );
    }
}

#[test]
fn logit_sse_output_is_always_clamped_away_from_extremes() {
    let mut sse = LogitSse::new(1);
    for _ in 0..10_000 {
        sse.update(0, 1.0, true);
    }
    let refined = sse.refine(0, 1.0);
    assert!(
        (MIN_PROBABILITY..1.0).contains(&refined),
        "refined={refined} must stay inside (0.0, 1.0) even after 10_000 updates \
         all pushing toward 1.0"
    );

    let mut sse = LogitSse::new(1);
    for _ in 0..10_000 {
        sse.update(0, 0.0, false);
    }
    let refined = sse.refine(0, 0.0);
    assert!(
        refined > 0.0 && refined <= MAX_PROBABILITY,
        "refined={refined} must stay inside (0.0, 1.0) even after 10_000 updates \
         all pushing toward 0.0"
    );
}

#[test]
fn logit_sse_converges_toward_the_true_observed_rate() {
    let mut sse = LogitSse::new(1);
    let rng = crate::test_support::Xorshift32::new(0xA5A5_5A5A);
    for state in rng.take(20_000) {
        let outcome = state % 10 != 0; // true 90% of the time
        sse.update(0, 0.5, outcome);
    }
    let refined = sse.refine(0, 0.5);
    assert!(
        (refined - 0.9).abs() < 0.03,
        "refined={refined}, expected convergence near the true rate 0.9"
    );
}

#[test]
fn logit_sse_update_advances_the_lower_and_upper_bin_separately() {
    // `0.5` (every other `LogitSse` test's training input) lands
    // exactly on a bin boundary (`position`'s fraction is `0.0`), so
    // the neighbor bin's update term is multiplied by zero and its
    // index never matters. `0.6` lands strictly between two bins,
    // which is what it takes to tell `lower + 1` apart from a
    // mutant `lower * 1` (`lower + 1 == lower * 1` only at `lower ==
    // 1`, never true here): both update terms must land, one per
    // bin, for this test to match the independent hand simulation
    // below.
    let p = 0.6;
    let (lower_index, fraction) = LogitSse::position(p);
    assert!(
        fraction > 0.05 && fraction < 0.95,
        "need p=0.6 to split non-trivially between two neighboring bins, \
         got lower_index={lower_index}, fraction={fraction}"
    );

    let bound = stretch_bound();
    #[allow(
        clippy::cast_precision_loss,
        reason = "lower_index < BINS - 1 (32): exact in f64"
    )]
    let lower_s = -bound + lower_index as f64 / (BINS - 1) as f64 * (2.0 * bound);
    #[allow(
        clippy::cast_precision_loss,
        reason = "lower_index + 1 <= BINS - 1 (32): exact in f64"
    )]
    let upper_s = -bound + (lower_index + 1) as f64 / (BINS - 1) as f64 * (2.0 * bound);
    // Oracle: `fill_identity_logit`'s own starting values for these
    // two bins, then update()'s documented per-bin recurrence
    // applied to each in isolation. If `update` really writes two
    // distinct slots, this must match `refine` reading the real
    // table back exactly, float for float.
    let mut lower_value = squash(lower_s);
    let mut upper_value = squash(upper_s);
    for _ in 0..1000 {
        lower_value += LEARNING_RATE * (1.0 - fraction) * (1.0 - lower_value);
        upper_value += LEARNING_RATE * fraction * (1.0 - upper_value);
    }
    let expected = lower_value
        .mul_add(1.0 - fraction, upper_value * fraction)
        .clamp(MIN_PROBABILITY, MAX_PROBABILITY);

    let mut sse = LogitSse::new(1);
    for _ in 0..1000 {
        sse.update(0, p, true);
    }
    let actual = sse.refine(0, p);

    assert!(
        (actual - expected).abs() < 1e-12,
        "actual={actual}, expected={expected}: update must advance the \
         lower bin (index {lower_index}) and the upper bin (index {}) \
         as two distinct slots, not collapse them into one",
        lower_index + 1
    );
}

#[test]
fn logit_sse_contexts_adapt_independently() {
    let mut sse = LogitSse::new(2);
    for _ in 0..5000 {
        sse.update(0, 0.5, true);
        sse.update(1, 0.5, false);
    }
    let refined0 = sse.refine(0, 0.5);
    let refined1 = sse.refine(1, 0.5);
    assert!(
        refined0 > 0.8,
        "context 0 saw only true outcomes, refined={refined0}"
    );
    assert!(
        refined1 < 0.2,
        "context 1 saw only false outcomes, refined={refined1}"
    );
}

#[test]
fn logit_sse_refine_is_monotonic_in_input_probability_on_a_fresh_table() {
    let sse = LogitSse::new(1);
    let mut previous = sse.refine(0, 0.0);
    for hundredth in 1..=100 {
        let p = f64::from(hundredth) / 100.0;
        let refined = sse.refine(0, p);
        assert!(
            refined >= previous,
            "refine must be non-decreasing in p on an untrained table: \
             p={p}, refined={refined}, previous={previous}"
        );
        previous = refined;
    }
}

#[test]
fn logit_sse_out_of_range_probability_is_clamped_not_a_panic() {
    let sse = LogitSse::new(1);
    let low = sse.refine(0, -1.0);
    let high = sse.refine(0, 2.0);
    assert!((low - MIN_PROBABILITY).abs() < 1e-6);
    assert!((high - MAX_PROBABILITY).abs() < 1e-6);
}

#[test]
#[should_panic(expected = "context out of range")]
fn logit_sse_refine_out_of_range_context_panics() {
    let sse = LogitSse::new(2);
    let _ = sse.refine(2, 0.5);
}

#[test]
#[should_panic(expected = "context out of range")]
fn logit_sse_update_out_of_range_context_panics() {
    let mut sse = LogitSse::new(2);
    sse.update(2, 0.5, true);
}

#[test]
fn logit_sse_contexts_reports_the_constructed_count() {
    assert_eq!(LogitSse::new(5).contexts(), 5);
}

#[test]
fn logit_sse_bins_concentrate_resolution_near_the_extremes() {
    // The mechanism this candidate exists to test: two probabilities
    // close together near 0.5 should fall in the same or adjacent
    // bins (coarse there), while two probabilities equally far apart
    // near 1.0 should land in more widely separated bins (fine
    // there) than Sse's own linear spacing would give them.
    #[allow(
        clippy::cast_precision_loss,
        reason = "bin indices here stay under BINS (33): exact in f64"
    )]
    fn gap(lo: (usize, f64), hi: (usize, f64)) -> f64 {
        (hi.0 as f64 + hi.1) - (lo.0 as f64 + lo.1)
    }
    let mid_gap = gap(LogitSse::position(0.50), LogitSse::position(0.52));
    let ext_gap = gap(LogitSse::position(0.96), LogitSse::position(0.98));

    assert!(
        ext_gap > mid_gap,
        "the same 0.02 probability step should cross more bin-space near 1.0 \
         (gap={ext_gap}) than near 0.5 (gap={mid_gap})"
    );
}
