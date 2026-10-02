use super::*;

/// `f64::ln`, the libm reference [`ln`] is diffed against.
/// `#[cfg(test)]`-gated, so it never reaches any coding path.
fn reference_ln(x: f64) -> f64 {
    #[allow(
        clippy::disallowed_methods,
        reason = "test-only oracle for the vendored ln's accuracy claim; #[cfg(test)] keeps it off every coding path"
    )]
    {
        x.ln()
    }
}

#[test]
fn vendored_ln_matches_f64_ln_within_1e_12_relative_across_1e_minus_6_to_1e6() {
    // 2,000 evenly spaced points in each of the twelve decades of
    // [1e-6, 1e6], plus points straddling 1, where ln(x) -> 0 and
    // relative accuracy is hardest to keep.
    let mut points = Vec::new();
    let mut decade_start = 1e-6;
    for _ in 0..12 {
        for step in 0..2_000 {
            points.push(decade_start * (1.0 + 9.0 * f64::from(step) / 2_000.0));
        }
        decade_start *= 10.0;
    }
    points.extend([
        1.0,
        1.0 + 1e-15,
        1.0 - 1e-15,
        1.0 + 1e-9,
        1.0 - 1e-9,
        0.999_999,
        1.000_001,
        std::f64::consts::SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
        1e-6,
        1e6,
    ]);
    for x in points {
        assert!(
            (1e-6 * (1.0 - 1e-12)..=1e6 * (1.0 + 1e-12)).contains(&x),
            "sweep left its range: {x}"
        );
        let (ours, reference) = (ln(x), reference_ln(x));
        assert!(
            (ours - reference).abs() <= 1e-12 * reference.abs(),
            "x={x}: vendored {ours} vs f64::ln {reference}"
        );
    }
}

#[test]
fn stretch_of_one_half_is_zero_and_odd_around_it() {
    assert!(stretch(0.5).abs() < 1e-15);
    for p in [0.01, 0.1, 0.3, 0.45] {
        assert!((stretch(p) + stretch(1.0 - p)).abs() < 1e-12, "p={p}");
        assert!(stretch(p) < 0.0, "p={p}");
    }
}

#[test]
fn squash_inverts_stretch() {
    for p in [1e-4, 0.01, 0.2, 0.5, 0.7, 0.99, 1.0 - 1e-4] {
        let back = squash(stretch(p));
        assert!((back - p).abs() <= 1e-7 * p.min(1.0 - p), "p={p}: {back}");
    }
}

#[test]
fn squash_stays_strictly_inside_zero_one() {
    assert!((squash(0.0) - 0.5).abs() < 1e-15);
    for x in [-1e6, -40.0, 40.0, 1e6] {
        let p = squash(x);
        assert!(p > 0.0 && p < 1.0, "x={x}: {p}");
    }
    assert!(squash(-1.0) < squash(1.0));
}
