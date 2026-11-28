use super::*;

/// The function [`nearest`] replaces, at its call sites' conversion.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the reference is only fed |x| < 2^52"
)]
pub(super) fn reference(x: f64) -> i64 {
    x.round() as i64
}

#[test]
fn every_tie_and_its_neighbours_round_as_f64_round_does() {
    for half_units in -4001_i32..=4001 {
        let x = f64::from(half_units) / 2.0;
        for probe in [x.next_down(), x, x.next_up()] {
            assert_eq!(nearest(probe), reference(probe), "x = {probe:?}");
        }
    }
}

#[test]
fn the_nearest_float_below_a_tie_rounds_down_in_magnitude() {
    assert_eq!(nearest(0.5_f64.next_down()), 0);
    assert_eq!(nearest((-0.5_f64).next_up()), 0);
    assert_eq!(nearest(0.5), 1);
    assert_eq!(nearest(-0.5), -1);
    assert_eq!(nearest(2.5), 3);
    assert_eq!(nearest(-2.5), -3);
}

#[test]
fn zeros_and_nan_round_to_zero() {
    assert_eq!(nearest(0.0), 0);
    assert_eq!(nearest(-0.0), 0);
    assert_eq!(nearest(f64::NAN), 0);
    assert_eq!(nearest(f64::NAN), reference(f64::NAN));
}

#[test]
fn the_precondition_limit_edge_rounds_as_f64_round_does() {
    let below = EXACT_LIMIT.next_down();
    for x in [below, -below, below - 1.0, 1.0 - below] {
        assert_eq!(nearest(x), reference(x), "x = {x:?}");
    }
}

#[test]
#[should_panic(expected = "nearest: |x| must stay below 2^52")]
fn a_value_past_the_limit_fails_a_debug_build() {
    let _ = nearest(EXACT_LIMIT);
}
