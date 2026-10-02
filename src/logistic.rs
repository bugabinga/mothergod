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
mod tests;
