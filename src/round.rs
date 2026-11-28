//! [`nearest`]: `f64::round` into an integer with no libm call.
//!
//! On baseline x86-64 (no SSE4.1) `f64::round` lowers to a call through the
//! GOT, which spills the per-bit model's live values (`research/JOURNAL.md`
//! S2-A157, issue #1979). A truncating cast and one exact subtraction
//! compile inline and give the identical integer, so the decode path's
//! rounding sites share this one function instead of each re-deriving the
//! proof.

/// Largest magnitude [`nearest`] accepts: `2^52`, past which every `f64` is
/// an integer and `x - trunc(x)` stops being the fraction the proof reads.
const EXACT_LIMIT: f64 = 4_503_599_627_370_496.0;

/// `x.round() as i64`, ties away from zero, a NaN to `0`, bit for bit.
///
/// Precondition: `|x| < 2^52` or NaN, checked in debug builds only: every
/// caller rounds a value it already bounds far below that. Inside it `t =
/// trunc(x)` is exact in `f64`, so `x - t` is exact too (the two share an
/// exponent range and `|x - t| < 1`), and its comparison with `0.5` is the
/// tie rule `f64::round` has: half or more away from zero rounds up in
/// magnitude. A NaN casts to `0` and fails both comparisons, matching
/// `f64::round(NaN) as i64`.
#[inline]
#[must_use]
pub(crate) fn nearest(x: f64) -> i64 {
    debug_assert!(
        x.is_nan() || x.abs() < EXACT_LIMIT,
        "nearest: |x| must stay below 2^52, got {x}"
    );
    #[allow(
        clippy::cast_possible_truncation,
        reason = "|x| < 2^52 (see the precondition) always fits i64; a NaN casts to 0"
    )]
    let truncated = x as i64;
    #[allow(
        clippy::cast_precision_loss,
        reason = "|truncated| < 2^52, exact in f64's 53-bit mantissa"
    )]
    let fraction = x - truncated as f64;
    truncated + i64::from(fraction >= 0.5) - i64::from(fraction <= -0.5)
}

#[cfg(test)]
mod tests;

// Sweeps the bit patterns where `mod tests` pins the edges one by one.
// Not under Miri: the sweep's case count is the cost, and the edge tests
// already walk the same code for UB observation.
#[cfg(test)]
#[cfg(not(miri))]
mod proptests;
