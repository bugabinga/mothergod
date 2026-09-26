//! The logit-domain transfer pair a logistic mixer is built on:
//! [`stretch`] `= ln(p / (1 - p))` and its inverse [`squash`]
//! `= 1 / (1 + e^-x)` (PAQ7 onward, Mahoney 2005), for
//! [`crate::literal::LogisticMix`] (`research/JOURNAL.md` S1-P8, S2-A101).
//!
//! `clippy.toml` forbids the libm transcendental family crate-wide
//! (ADR-0024), so both are built from IEEE-754 basic operations only:
//! [`squash`] reuses [`crate::literal`]'s vendored `exp`, and [`ln`] is
//! vendored here. Invariant every caller relies on: [`ln`] is only ever
//! called on a positive, finite, normal `f64`; [`stretch`] guarantees that
//! for any `p` strictly inside `(0, 1)` whose odds ratio is itself normal.

/// IEEE-754 binary64 exponent bias, mantissa width, and mantissa mask, for
/// [`ln`]'s exact split of `x` into `m * 2^k`.
const EXPONENT_BIAS: i64 = 1023;
const MANTISSA_BITS: u32 = 52;
const MANTISSA_MASK: u64 = (1 << MANTISSA_BITS) - 1;

/// Natural logarithm of `x`, from IEEE-754 basic operations and bit
/// manipulation only (no libm call).
///
/// `x = m * 2^k` with `m` in `[sqrt(1/2), sqrt(2))`, read off the bit
/// pattern exactly; then `ln(m) = 2 * atanh(s)` with `s = (m - 1) / (m +
/// 1)`, `|s| <= 0.1716`, summed as an odd series to `s^27`, whose first
/// omitted term is below `1e-21`. `m - 1` is exact near `m = 1` (Sterbenz),
/// so the result keeps its relative accuracy as `ln(x)` approaches `0`.
///
/// Precondition: `x` positive, finite and normal (see the module doc);
/// checked in debug builds only, because every caller derives `x` from a
/// probability it already bounds away from `0` and `1`.
#[must_use]
pub fn ln(x: f64) -> f64 {
    debug_assert!(
        x.is_normal() && x > 0.0,
        "ln: x must be positive and normal, got {x}"
    );
    let bits = x.to_bits();
    let biased = i64::try_from(bits >> MANTISSA_BITS).expect("an 11-bit exponent fits i64");
    let mut k = biased - EXPONENT_BIAS;
    // Mantissa in [1, 2): same fraction bits, exponent field set to 0.
    #[allow(
        clippy::cast_sign_loss,
        reason = "EXPONENT_BIAS is a positive constant, the cast is lossless"
    )]
    let mut m = f64::from_bits((bits & MANTISSA_MASK) | ((EXPONENT_BIAS as u64) << MANTISSA_BITS));
    if m > std::f64::consts::SQRT_2 {
        m /= 2.0;
        k += 1;
    }
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let mut term = s;
    let mut series = 0.0;
    let mut denominator = 1.0;
    for _ in 0..14 {
        series += term / denominator;
        term *= s2;
        denominator += 2.0;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "|k| <= 1075 for any normal f64, exact in f64"
    )]
    let k = k as f64;
    k.mul_add(std::f64::consts::LN_2, 2.0 * series)
}

/// `ln(p / (1 - p))`: a probability in `(0, 1)` mapped to the logit
/// domain, where a logistic mixer adds evidence instead of averaging it.
#[must_use]
pub fn stretch(p: f64) -> f64 {
    ln(p / (1.0 - p))
}

/// `1 / (1 + e^-x)`, [`stretch`]'s inverse. `exp` clamps its argument to
/// `[-30, 30]`, so the result stays strictly inside `(0, 1)` for every
/// finite `x`.
#[must_use]
pub fn squash(x: f64) -> f64 {
    1.0 / (1.0 + crate::literal::exp(-x))
}

#[cfg(test)]
mod tests {
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
}
