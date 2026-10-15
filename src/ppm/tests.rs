use super::*;

#[test]
#[should_panic(expected = "alphabet must be non-empty")]
fn zero_alphabet_panics() {
    let _ = Ppm::new(0);
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
fn distinct_counts_each_symbol_once_regardless_of_repeats() {
    let mut ppm = Ppm::new(4);
    for symbol in [1, 1, 1, 2] {
        ppm.observe(symbol);
    }
    // freq[1] == 3 * increment, total == 4 * increment, distinct == 2.
    let increment = f64::from(crate::DEFAULT_RESCALE_INCREMENT);
    let expected = 3.0 * increment / (4.0 * increment + 2.0);
    assert!((ppm.probability(1) - expected).abs() < 1e-12);
}

#[test]
fn an_observed_symbol_never_decays_back_to_unseen_across_rescales() {
    // DEFAULT_RESCALE_LIMIT is 65536 and every observe adds
    // DEFAULT_RESCALE_INCREMENT=12, so 10,000 repeats cross the halving
    // threshold several times over.
    let mut ppm = Ppm::new(2);
    for _ in 0..10_000 {
        ppm.observe(0);
        assert!(ppm.probability(0) > 0.0);
    }
    assert!((ppm.probability(1) - 0.0).abs() < 1e-12);
}
