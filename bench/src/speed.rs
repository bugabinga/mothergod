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
//! Issue #1184 replaces that kernel. Its hypothesis: the decoder's time goes
//! to the literal model's floating-point mix, a serial chain of divides and
//! multiplies (JOURNAL S2-R36's profile), which a dependent-load walk does
//! not model. The row kernel that guessed table streaming instead failed
//! the finish line, so it is gone.
//! [`Kernel::Fp`] is the candidate, measured beside the gating
//! [`Kernel::Walk`] in the same run and gating nothing, so
//! `baseline_gate -- spread --fp` can build its per-CPU table from CI logs
//! before a second PR swaps the gate or drops the candidate.
//!
//! The committed number is [`BASELINE_DECODE_COST`]; [`slowdown`] reports a
//! measurement more than [`MAX_SLOWDOWN`] above it. The check is a ratchet on
//! the hermetic gate cases, not a direct test of "1 MB/s on Silesia": no
//! hermetic case reproduces Silesia's mix (its per-file decode speeds span
//! 0.25 to 6.6 MB/s), so what the gate holds is that no accepted slice
//! makes the codec slower by more than the margin without updating the
//! constant and saying why.

use crate::baseline::Case;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;

/// A calibration kernel a decode cost divides by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kernel {
    /// The gating kernel, [`calibration_seconds`].
    Walk,
    /// The #1184 candidate, [`fp_seconds`].
    Fp,
}

impl Kernel {
    /// Steps of this kernel per timed run.
    const fn steps(self) -> u32 {
        match self {
            Self::Walk => CALIBRATION_STEPS,
            Self::Fp => FP_STEPS,
        }
    }

    /// Wall time of one run of this kernel, best of `REPEATS`.
    #[must_use]
    pub fn seconds(self) -> f64 {
        match self {
            Self::Walk => calibration_seconds(),
            Self::Fp => fp_seconds(),
        }
    }

    /// Text before the cost in this kernel's cost line. The walk's is the
    /// format the CI logs of main already hold, so it stays as it was.
    const fn line_prefix(self) -> &'static str {
        match self {
            Self::Walk => "bench speed gate: decode cost ",
            Self::Fp => "bench speed gate: fp kernel decode cost ",
        }
    }

    /// Text between the cost and the CPU name in this kernel's cost line.
    const fn line_infix(self) -> &'static str {
        match self {
            Self::Walk => " calibration steps/byte on ",
            Self::Fp => " fp steps/byte on ",
        }
    }
}

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
/// [`slowdown`] reports it. Measured from the `bench speed gate` line of 136
/// `ci.yml` push runs on main, 2026-10-08 to 2026-10-14 (issue #918): the
/// pool's six CPU models have median costs 103.70 to 132.28 and the whole
/// range is 94.87 to 132.95, so the worst run is +10.8% over the baseline.
/// Within one model the range is up to 33.6% (AMD EPYC 7763, 69 runs, 94.87
/// to 126.76), main's code changes included, so that figure bounds the noise
/// from above. The kernel therefore does not track the decoder across
/// models (the 8370C holds 130.39 to 132.95 across the window while the
/// 7763 sits near 100 to 115); within one model it is unresolved. A margin
/// under about 0.11 would redden main on the Intel Xeon Platinum 8370C
/// alone. The margin stays wide until the calibration kernel is replaced by
/// one closer to the decoder's access pattern; `baseline_gate` prints the
/// CPU beside every cost, and `baseline_gate -- spread` rebuilds the
/// per-CPU table above from the logs of those runs.
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

/// Index width of the [`fp_seconds`] table: the top bits of a hash.
const FP_INDEX_BITS: u32 = 8;

/// Entries of the [`fp_seconds`] table.
const FP_TABLE_LEN: usize = 1 << FP_INDEX_BITS;

/// Multiplier of the hashes in [`fp_walk`]: 2^64 over the golden ratio.
const FP_HASH: u64 = 0x9E37_79B9_7F4A_7C15;

/// Steps of the [`fp_seconds`] kernel per run.
const FP_STEPS: u32 = 2_000_000;

/// The [`fp_seconds`] table: every entry in `[1, 2)`, so [`fp_walk`]'s
/// divisor never nears zero.
fn fp_table() -> [f64; FP_TABLE_LEN] {
    let mut table = [0.0; FP_TABLE_LEN];
    for (entry, index) in table.iter_mut().zip(0u32..) {
        *entry = 1.0 + f64::from(index) / f64::from(1u32 << FP_INDEX_BITS);
    }
    table
}

/// Bits of `1.0`: OR-ing 52 mantissa bits under them gives a value in
/// `[1, 2)`.
const FP_ONE_BITS: u64 = 0x3FF0_0000_0000_0000;

/// `steps` of a serial floating-point chain over `table`, returning its
/// final state. Each step multiplies, adds and divides the state by a table
/// entry the state itself chose, rehashes the quotient's bits into a fresh
/// full-mantissa state, and writes that back into the entry: the divide,
/// the multiplies and the table index of `literal::logistic_mix_node` and
/// `sse::Table::locate`, each waiting on the one before.
///
/// The rehash is load-bearing: `(x * y + 1) / (x + y)` alone has the fixed
/// point 1.0, reached in a few steps, after which the divide sees operands
/// no real mix has.
///
/// Invariant: the state and every entry stay in `[1, 2)`, so the quotient
/// lies in `[1, 1.25]`: no zero divisor, no infinity, no denormal, on any
/// CPU.
fn fp_walk(table: &mut [f64; FP_TABLE_LEN], steps: u32) -> f64 {
    let mut state = 1.5f64;
    for _ in 0..steps {
        let index = (state.to_bits().wrapping_mul(FP_HASH) >> (u64::BITS - FP_INDEX_BITS)) as usize;
        let entry = table[index];
        let quotient = (state * entry + 1.0) / (state + entry);
        state = f64::from_bits(FP_ONE_BITS | (quotient.to_bits().wrapping_mul(FP_HASH) >> 12));
        table[index] = state;
    }
    state
}

/// Wall time of the candidate kernel, `fp_walk` over a fresh `fp_table`,
/// best of `REPEATS`. Shaped like the decode profile of #1184 (the literal
/// model's mix, FP divide and multiply latency, not table loads), and fit
/// to the hybrid's decoder alone: whether it tracks `cm_path`'s decode is
/// unmeasured.
#[must_use]
pub fn fp_seconds() -> f64 {
    let mut table = fp_table();
    let mut best = f64::MAX;
    for _ in 0..REPEATS {
        let start = Instant::now();
        black_box(fp_walk(&mut table, FP_STEPS));
        best = best.min(start.elapsed().as_secs_f64());
    }
    best
}

/// Wall time of one `decode` call, best of `REPEATS`. The last output is
/// compared with `expected` outside the timer, so no time is reported for
/// a decode that does not round-trip.
///
/// # Panics
///
/// Panics if the output differs from `expected`.
pub fn best_decode_seconds(mut decode: impl FnMut() -> Vec<u8>, expected: &[u8]) -> f64 {
    let mut best = f64::MAX;
    let mut decoded = Vec::new();
    for _ in 0..REPEATS {
        let start = Instant::now();
        decoded = black_box(decode());
        best = best.min(start.elapsed().as_secs_f64());
    }
    assert!(decoded == expected, "a timed decode must round-trip");
    best
}

/// Decode time summed over `cases`, each compressed once outside the timer
/// and timed by [`best_decode_seconds`], plus the total original bytes.
///
/// # Panics
///
/// Panics if a case's own compressed form fails to decode to the case,
/// which would be a round-trip bug the `ratio` gate's tests catch first.
#[must_use]
pub fn decode_seconds(cases: &[Case]) -> (f64, usize) {
    let mut seconds = 0.0;
    let mut bytes = 0;
    for case in cases {
        let compressed = mothergod::compress(&case.data);
        seconds += best_decode_seconds(
            || {
                mothergod::decompress(black_box(&compressed))
                    .expect("a case's own compressed form decodes")
            },
            &case.data,
        );
        bytes += case.data.len();
    }
    (seconds, bytes)
}

/// `kernel` steps per decoded byte: decode seconds per byte over
/// `kernel_seconds` per step. Lower is faster.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "byte counts here stay far below 2^53"
)]
pub fn decode_cost(kernel: Kernel, decode_seconds: f64, bytes: usize, kernel_seconds: f64) -> f64 {
    (decode_seconds / bytes as f64) / (kernel_seconds / f64::from(kernel.steps()))
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

/// The line `baseline_gate` prints once per kernel per run: `cost` in
/// `kernel` steps per byte, then the CPU it was measured on. [`Kernel`]
/// owns the text around the cost, so [`parse_cost_line`] reads what this
/// prints.
#[must_use]
pub fn cost_line(kernel: Kernel, cost: f64, cpu: &str) -> String {
    format!(
        "{}{cost:.2}{}{cpu}",
        kernel.line_prefix(),
        kernel.line_infix()
    )
}

/// The cost and CPU of a [`cost_line`] for `kernel` inside `line`, or `None`
/// when `line` holds none. A CI log prefixes each line with a job, step and
/// timestamp, so the line is found anywhere in `line`, not only at its
/// start. The slowdown report shares the walk's prefix and carries an arrow
/// where the cost ends, so it does not match.
#[must_use]
pub fn parse_cost_line(kernel: Kernel, line: &str) -> Option<(f64, &str)> {
    let (_, rest) = line.split_once(kernel.line_prefix())?;
    let (cost, cpu) = rest.split_once(kernel.line_infix())?;
    let cost: f64 = cost.parse().ok().filter(|cost: &f64| cost.is_finite())?;
    let cpu = cpu.trim();
    (!cpu.is_empty()).then_some((cost, cpu))
}

/// One CPU model's costs over many runs, from [`spread`].
#[derive(Debug, Clone, PartialEq)]
pub struct CpuSpread {
    /// The CPU model name as the cost lines print it.
    pub cpu: String,
    /// Number of cost lines for this CPU.
    pub runs: usize,
    /// Smallest cost.
    pub min: f64,
    /// Median cost; the mean of the two middle costs when `runs` is even.
    pub median: f64,
    /// Largest cost.
    pub max: f64,
}

impl CpuSpread {
    /// The within-model range as a fraction of the smallest cost, the
    /// figure #1184 finishes on.
    #[must_use]
    pub fn range(&self) -> f64 {
        self.max / self.min - 1.0
    }
}

/// Groups every [`parse_cost_line`] match for `kernel` in `text` by CPU,
/// sorted by CPU name.
#[must_use]
pub fn spread(kernel: Kernel, text: &str) -> Vec<CpuSpread> {
    let mut by_cpu: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for (cost, cpu) in text
        .lines()
        .filter_map(|line| parse_cost_line(kernel, line))
    {
        by_cpu.entry(cpu).or_default().push(cost);
    }
    by_cpu
        .into_iter()
        .map(|(cpu, mut costs)| {
            costs.sort_by(f64::total_cmp);
            let runs = costs.len();
            let median = if runs % 2 == 1 {
                costs[runs / 2]
            } else {
                f64::midpoint(costs[runs / 2 - 1], costs[runs / 2])
            };
            CpuSpread {
                cpu: cpu.to_string(),
                runs,
                min: costs[0],
                median,
                max: costs[runs - 1],
            }
        })
        .collect()
}

/// The [`spread`] table as text, one line per CPU after a header, columns
/// aligned.
#[must_use]
pub fn format_spread(rows: &[CpuSpread]) -> String {
    let width = rows
        .iter()
        .map(|row| row.cpu.len())
        .fold("cpu".len(), usize::max);
    let mut out = String::new();
    writeln!(
        out,
        "{:<width$}  {:>5}  {:>8}  {:>8}  {:>8}  {:>7}",
        "cpu", "runs", "min", "median", "max", "range"
    )
    .expect("writing to a String never fails");
    for row in rows {
        writeln!(
            out,
            "{:<width$}  {:>5}  {:>8.2}  {:>8.2}  {:>8.2}  {:>+6.1}%",
            row.cpu,
            row.runs,
            row.min,
            row.median,
            row.max,
            row.range() * 100.0
        )
        .expect("writing to a String never fails");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_is_calibration_steps_per_decoded_byte() {
        // 2 s for 1000 bytes is 2 ms per byte; a 4 s calibration of
        // 4_000_000 steps is 1 us per step; 2000 us per byte / 1 us = 2000.
        let cost = decode_cost(Kernel::Walk, 2.0, 1000, 4.0);
        assert!((cost - 2000.0).abs() < 1e-9, "{cost}");
    }

    #[test]
    fn a_slower_machine_leaves_the_cost_unchanged() {
        let fast = decode_cost(Kernel::Walk, 1.0, 1000, 1.0);
        let slow = decode_cost(Kernel::Walk, 3.0, 1000, 3.0);
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

    #[test]
    #[should_panic(expected = "a timed decode must round-trip")]
    fn a_decode_that_does_not_round_trip_reports_no_time() {
        let _ = best_decode_seconds(|| vec![1, 2, 3], &[1, 2, 4]);
    }

    #[test]
    fn a_cost_line_parses_back_to_its_cost_and_cpu() {
        let cpu = "Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz";
        for kernel in [Kernel::Walk, Kernel::Fp] {
            let line = cost_line(kernel, 132.28, cpu);
            assert_eq!(parse_cost_line(kernel, &line), Some((132.28, cpu)));
        }
    }

    #[test]
    fn a_cost_line_is_read_by_its_own_kernel_only() {
        let walk = cost_line(Kernel::Walk, 120.0, "A");
        let fp = cost_line(Kernel::Fp, 9.5, "A");
        assert_eq!(parse_cost_line(Kernel::Fp, &walk), None);
        assert_eq!(parse_cost_line(Kernel::Walk, &fp), None);
    }

    #[test]
    fn the_walk_line_keeps_the_format_main_s_ci_logs_hold() {
        assert_eq!(
            cost_line(Kernel::Walk, 99.5, "AMD EPYC 7763"),
            "bench speed gate: decode cost 99.50 calibration steps/byte on AMD EPYC 7763"
        );
    }

    #[test]
    fn an_fp_walk_is_deterministic_and_stays_in_its_invariant() {
        let state = fp_walk(&mut fp_table(), 1000);
        assert_eq!(state, fp_walk(&mut fp_table(), 1000));
        assert_ne!(state, fp_walk(&mut fp_table(), 999), "steps fold in");
        assert!((1.0..2.0).contains(&state), "{state}");
        let mut table = fp_table();
        fp_walk(&mut table, 1000);
        assert!(table.iter().all(|entry| (1.0..2.0).contains(entry)));
        assert_ne!(table, fp_table(), "the walk writes its table back");
        let distinct = (1..=2000u32)
            .map(|steps| fp_walk(&mut fp_table(), steps).to_bits())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            distinct.len() > 1900,
            "the chain collapsed: {}",
            distinct.len()
        );
    }

    #[test]
    fn a_ci_log_prefix_and_other_lines_do_not_hide_or_fake_a_cost_line() {
        let logged = "ratio\tRun gate\t2026-10-08T12:00:00.0000000Z \
                      bench speed gate: decode cost 99.50 calibration steps/byte on AMD EPYC 7763\r";
        assert_eq!(
            parse_cost_line(Kernel::Walk, logged),
            Some((99.5, "AMD EPYC 7763"))
        );
        for other in [
            "bench speed gate: decode cost 120.00 -> 130.00 calibration steps/byte (+8.3%, margin 25%)",
            "bench speed gate: decode cost NaN calibration steps/byte on AMD EPYC 7763",
            "bench speed gate: decode cost 99.50 calibration steps/byte on ",
            "bench baseline gate: 12 cases, no regression",
        ] {
            assert_eq!(parse_cost_line(Kernel::Walk, other), None, "{other}");
        }
    }

    #[test]
    fn spread_averages_the_two_middle_costs_of_an_even_run_count() {
        // The pairs #1186 names: the lower middle value, 132.17 and 111.64,
        // was the wrong median.
        let log = "\
bench speed gate: decode cost 132.95 calibration steps/byte on Xeon 8370C
bench speed gate: decode cost 132.17 calibration steps/byte on Xeon 8370C
bench speed gate: decode cost 111.80 calibration steps/byte on EPYC 9V45
bench speed gate: decode cost 130.39 calibration steps/byte on Xeon 8370C
bench speed gate: decode cost 132.39 calibration steps/byte on Xeon 8370C
bench speed gate: decode cost 111.64 calibration steps/byte on EPYC 9V45
unrelated line
";
        let rows = spread(Kernel::Walk, log);
        assert_eq!(rows.len(), 2);
        let (epyc, xeon) = (&rows[0], &rows[1]);
        assert_eq!((epyc.cpu.as_str(), epyc.runs), ("EPYC 9V45", 2));
        assert!((epyc.median - 111.72).abs() < 1e-9, "{}", epyc.median);
        assert_eq!((xeon.cpu.as_str(), xeon.runs), ("Xeon 8370C", 4));
        assert!((xeon.median - 132.28).abs() < 1e-9, "{}", xeon.median);
        assert_eq!((xeon.min, xeon.max), (130.39, 132.95));
        assert!((xeon.range() - (132.95 / 130.39 - 1.0)).abs() < 1e-12);
    }

    #[test]
    fn spread_groups_the_fp_kernel_lines_apart_from_the_walk_lines() {
        let log = "\
bench speed gate: decode cost 120.00 calibration steps/byte on A
bench speed gate: fp kernel decode cost 9.00 fp steps/byte on A
bench speed gate: fp kernel decode cost 10.00 fp steps/byte on A
";
        assert_eq!(spread(Kernel::Walk, log)[0].runs, 1);
        let fp = spread(Kernel::Fp, log);
        assert_eq!((fp[0].runs, fp[0].min, fp[0].max), (2, 9.0, 10.0));
    }

    #[test]
    fn spread_takes_the_middle_cost_of_an_odd_run_count() {
        let log = "\
bench speed gate: decode cost 3.00 calibration steps/byte on A
bench speed gate: decode cost 1.00 calibration steps/byte on A
bench speed gate: decode cost 2.00 calibration steps/byte on A
";
        assert_eq!(spread(Kernel::Walk, log)[0].median, 2.0);
        assert_eq!(spread(Kernel::Walk, "no cost lines here\n").len(), 0);
    }

    #[test]
    fn format_spread_prints_one_aligned_line_per_cpu() {
        let rows = spread(
            Kernel::Walk,
            "bench speed gate: decode cost 100.00 calibration steps/byte on AMD EPYC 7763\n\
             bench speed gate: decode cost 125.00 calibration steps/byte on AMD EPYC 7763\n",
        );
        assert_eq!(
            format_spread(&rows),
            "cpu             runs       min    median       max    range\n\
             AMD EPYC 7763      2    100.00    112.50    125.00   +25.0%\n"
        );
    }
}
