//! CI speed gate over mothergod's single-thread decode cost on the
//! [`crate::baseline`] cases (issue #907: ROADMAP's SPEED floor was a
//! sentence nothing enforced).
//!
//! A raw MB/s number is a property of the runner as much as of the code:
//! the CI pool spans at least 1.2 to 1.7 MB/s on near-identical code
//! (`docs/benchmarks/silesia.md` records the CPU per snapshot). So the gate
//! never compares MB/s. [`decode_cost`] divides the measured decode time per
//! byte by the measured time of a fixed [`calibration_seconds`] kernel run on
//! the same machine in the same process: the result counts calibration
//! steps per decoded byte, and a slower CPU slows both terms.
//!
//! The committed number is [`BASELINE_DECODE_COST`]; [`slowdown`] reports a
//! measurement more than [`MAX_SLOWDOWN`] above it. The check is a ratchet on
//! the hermetic gate cases, not a direct test of "1 MB/s on Silesia": no
//! hermetic case reproduces Silesia's mix (its per-file decode speeds span
//! 0.25 to 6.6 MB/s), so what the gate holds is that no accepted slice
//! makes the codec slower by more than the margin without updating the
//! constant and saying why.

use crate::baseline::Case;
use std::hint::black_box;
use std::time::Instant;

/// Steps of the [`calibration_seconds`] kernel per run.
const CALIBRATION_STEPS: u32 = 4_000_000;

/// Entries in the calibration table: 512 KiB of `u32`, past L1 and inside
/// every runner CPU's L2, so the kernel pays a cache latency per step the
/// way the decoder's model tables do. A power of two, so the index is a mask.
const CALIBRATION_TABLE_LEN: u32 = 1 << 17;

/// Runs per timing (calibration and each case's decode); the minimum is
/// kept, because interference only ever adds time.
const REPEATS: u32 = 5;

/// [`decode_cost`] of the committed codec: calibration steps per decoded
/// byte, measured by `baseline_gate -- speed` on an Intel Xeon Platinum 8573C
/// CI runner. Update it in the PR of an accepted slowdown, and say why in the
/// PR body, the way `bench/baseline.json` moves for an accepted ratio trade.
pub const BASELINE_DECODE_COST: f64 = 120.0;

/// Fraction [`decode_cost`] may exceed [`BASELINE_DECODE_COST`] before
/// [`slowdown`] reports it. Measured: back-to-back runs on one runner agree
/// within 3%. Not measured: how far the cost moves between the pool's CPU
/// models, because a hermetic run sees one. The margin is wide for that
/// reason; `baseline_gate` prints the CPU beside every cost so the pool's
/// spread accrues in the CI logs, and the margin tightens once those show it.
pub const MAX_SLOWDOWN: f64 = 0.25;

/// Wall time of a fixed dependent-load kernel: `CALIBRATION_STEPS` steps of
/// a read-modify-write walk over a `CALIBRATION_TABLE_LEN`-entry table,
/// each index depending on the previous step's value. Best of `REPEATS`.
#[must_use]
pub fn calibration_seconds() -> f64 {
    let mut table: Vec<u32> = (0..CALIBRATION_TABLE_LEN)
        .map(|index| index.wrapping_mul(0x9E37_79B1))
        .collect();
    let mut best = f64::MAX;
    for _ in 0..REPEATS {
        let start = Instant::now();
        let mut state = 1u32;
        for _ in 0..CALIBRATION_STEPS {
            let index = (state % CALIBRATION_TABLE_LEN) as usize;
            state = table[index]
                .wrapping_add(state.rotate_left(5))
                .wrapping_mul(0x9E37_79B1);
            table[index] = state;
        }
        black_box(state);
        best = best.min(start.elapsed().as_secs_f64());
    }
    best
}

/// Decode time summed over `cases`, each compressed once outside the timer
/// and decoded `REPEATS` times with the minimum kept, plus the total
/// original bytes.
///
/// # Panics
///
/// Panics if a case's own compressed form fails to decode, which would be a
/// round-trip bug the `ratio` gate's tests catch first.
#[must_use]
pub fn decode_seconds(cases: &[Case]) -> (f64, usize) {
    let mut seconds = 0.0;
    let mut bytes = 0;
    for case in cases {
        let compressed = mothergod::compress(&case.data);
        let mut best = f64::MAX;
        for _ in 0..REPEATS {
            let start = Instant::now();
            let decoded = mothergod::decompress(black_box(&compressed))
                .expect("a case's own compressed form decodes");
            black_box(decoded);
            best = best.min(start.elapsed().as_secs_f64());
        }
        seconds += best;
        bytes += case.data.len();
    }
    (seconds, bytes)
}

/// Calibration steps per decoded byte: decode seconds per byte over
/// calibration seconds per step. Lower is faster.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "byte counts here stay far below 2^53"
)]
pub fn decode_cost(decode_seconds: f64, bytes: usize, calibration_seconds: f64) -> f64 {
    (decode_seconds / bytes as f64) / (calibration_seconds / f64::from(CALIBRATION_STEPS))
}

/// A cost past [`BASELINE_DECODE_COST`]'s margin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slowdown {
    /// The committed cost, [`BASELINE_DECODE_COST`].
    pub baseline: f64,
    /// The measured cost.
    pub measured: f64,
}

impl Slowdown {
    /// How far above the baseline the measurement sits, as a fraction.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        self.measured / self.baseline - 1.0
    }
}

/// `Some` when `measured` exceeds `baseline` by more than `max_slowdown`
/// (a fraction). Faster than the baseline never reports.
#[must_use]
pub fn slowdown(baseline: f64, measured: f64, max_slowdown: f64) -> Option<Slowdown> {
    (measured > baseline * (1.0 + max_slowdown)).then_some(Slowdown { baseline, measured })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_is_calibration_steps_per_decoded_byte() {
        // 2 s for 1000 bytes is 2 ms per byte; a 4 s calibration of
        // 4_000_000 steps is 1 us per step; 2000 us per byte / 1 us = 2000.
        let cost = decode_cost(2.0, 1000, 4.0);
        assert!((cost - 2000.0).abs() < 1e-9, "{cost}");
    }

    #[test]
    fn a_slower_machine_leaves_the_cost_unchanged() {
        let fast = decode_cost(1.0, 1000, 1.0);
        let slow = decode_cost(3.0, 1000, 3.0);
        assert!((fast - slow).abs() < 1e-9, "{fast} vs {slow}");
    }

    #[test]
    fn slowdown_reports_only_past_the_margin() {
        assert_eq!(
            slowdown(100.0, 125.0, 0.25),
            None,
            "the margin is inclusive"
        );
        assert_eq!(slowdown(100.0, 90.0, 0.25), None, "faster never reports");
        let reported = slowdown(100.0, 130.0, 0.25).expect("30% is past a 25% margin");
        assert!((reported.fraction() - 0.30).abs() < 1e-9);
    }

    #[test]
    fn decode_seconds_counts_every_case_byte() {
        let cases = [Case {
            name: "zeros",
            data: vec![0; 1000],
        }];
        let (seconds, bytes) = decode_seconds(&cases);
        assert_eq!(bytes, 1000);
        assert!(seconds > 0.0);
    }
}
