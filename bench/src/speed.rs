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
//! to streaming 256-entry frequency rows through a multiply-accumulate
//! (`Literal::mix`), which a dependent-load walk does not model.
//! [`Kernel::Rows`] is the candidate, measured beside the gating
//! [`Kernel::Walk`] in the same run and gating nothing, so
//! `baseline_gate -- spread --rows` can build its per-CPU table from CI logs
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
    /// The #1184 candidate, [`rows_seconds`].
    Rows,
}

impl Kernel {
    /// Steps of this kernel per timed run.
    const fn steps(self) -> u32 {
        match self {
            Self::Walk => CALIBRATION_STEPS,
            Self::Rows => ROW_STEPS,
        }
    }

    /// Wall time of one run of this kernel, best of `REPEATS`.
    #[must_use]
    pub fn seconds(self) -> f64 {
        match self {
            Self::Walk => calibration_seconds(),
            Self::Rows => rows_seconds(),
        }
    }

    /// Text before the cost in this kernel's cost line. The walk's is the
    /// format the CI logs of main already hold, so it stays as it was.
    const fn line_prefix(self) -> &'static str {
        match self {
            Self::Walk => "bench speed gate: decode cost ",
            Self::Rows => "bench speed gate: row kernel decode cost ",
        }
    }

    /// Text between the cost and the CPU name in this kernel's cost line.
    const fn line_infix(self) -> &'static str {
        match self {
            Self::Walk => " calibration steps/byte on ",
            Self::Rows => " row steps/byte on ",
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

/// Entries per row of the [`rows_seconds`] table: the literal model's
/// alphabet, one `u16` frequency per symbol.
const ROW_LEN: usize = 256;

/// Rows in the [`rows_seconds`] table, 4.6 MiB of `u16`: the literal model's
/// bank count (`literal::BANKS`) at the time of writing. The kernel is a
/// ruler, so it keeps this figure when the model's moves; a change here
/// moves every cost and retires the baseline.
const ROW_COUNT: usize = 9281;

/// Experts blended per step of [`row_walk`], the literal model's `EXPERTS`.
const ROW_EXPERTS: u32 = 6;

/// Blends per [`rows_seconds`] run; a step is one row accumulated, so a run
/// is `ROW_BLENDS * ROW_EXPERTS` steps.
const ROW_BLENDS: u32 = 20_000;

/// Steps of the [`rows_seconds`] kernel per run.
const ROW_STEPS: u32 = ROW_BLENDS * ROW_EXPERTS;

/// The [`rows_seconds`] table: each `u16` a pseudorandom frequency.
fn row_table() -> Vec<u16> {
    (0..ROW_COUNT * ROW_LEN)
        .map(|index| {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "the shift keeps the top 16 bits of a u32"
            )]
            let frequency = ((index as u32).wrapping_mul(0x9E37_79B1) >> 16) as u16;
            frequency
        })
        .collect()
}

/// `value` reduced into `0..modulus`.
fn reduce(value: u64, modulus: usize) -> usize {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the remainder is below `modulus`, a usize"
    )]
    let reduced = (value % modulus as u64) as usize;
    reduced
}

/// `blends` steps of a `Literal::mix`-shaped walk over `table`, returning
/// its final state. Each blend accumulates [`ROW_EXPERTS`] rows, chosen from
/// the state, into 256 `u64` masses weighted by a state-derived scale, takes
/// their running sum as the cumulative table, bumps one frequency the way
/// `rescale_bank` does, and folds the total into the state, so the next
/// blend's rows depend on this one's result.
fn row_walk(table: &mut [u16], blends: u32) -> u64 {
    let mut state = 1u64;
    for _ in 0..blends {
        let mut mixed = [0u64; ROW_LEN];
        for expert in 0..ROW_EXPERTS {
            let draw = state.rotate_left(expert * 9);
            let row = reduce(draw, ROW_COUNT) * ROW_LEN;
            let scale = (draw >> 40) | 1;
            for (mass, &frequency) in mixed.iter_mut().zip(&table[row..row + ROW_LEN]) {
                *mass += scale * u64::from(frequency);
            }
        }
        let mut total = 0u64;
        for mass in mixed {
            total += (mass >> 16) + 1;
        }
        let bumped = reduce(state, ROW_COUNT) * ROW_LEN + reduce(total, ROW_LEN);
        table[bumped] = table[bumped].wrapping_add(32);
        state = state
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(total);
    }
    state
}

/// Wall time of the candidate kernel, `row_walk` over a fresh
/// `row_table`: the access pattern of the decoder's mix, rows streamed
/// from a table past L2 and a multiply-accumulate across them. Best of
/// `REPEATS`.
#[must_use]
pub fn rows_seconds() -> f64 {
    let mut table = row_table();
    let mut best = f64::MAX;
    for _ in 0..REPEATS {
        let start = Instant::now();
        black_box(row_walk(&mut table, ROW_BLENDS));
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
    fn a_cost_line_parses_back_to_its_cost_and_cpu() {
        let cpu = "Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz";
        for kernel in [Kernel::Walk, Kernel::Rows] {
            let line = cost_line(kernel, 132.28, cpu);
            assert_eq!(parse_cost_line(kernel, &line), Some((132.28, cpu)));
        }
    }

    #[test]
    fn a_cost_line_is_read_by_its_own_kernel_only() {
        let walk = cost_line(Kernel::Walk, 120.0, "A");
        let rows = cost_line(Kernel::Rows, 9.5, "A");
        assert_eq!(parse_cost_line(Kernel::Rows, &walk), None);
        assert_eq!(parse_cost_line(Kernel::Walk, &rows), None);
    }

    #[test]
    fn the_walk_line_keeps_the_format_main_s_ci_logs_hold() {
        assert_eq!(
            cost_line(Kernel::Walk, 99.5, "AMD EPYC 7763"),
            "bench speed gate: decode cost 99.50 calibration steps/byte on AMD EPYC 7763"
        );
    }

    #[test]
    fn a_row_walk_is_deterministic_and_reads_its_table() {
        let mut first = row_table();
        let mut second = row_table();
        let state = row_walk(&mut first, 8);
        assert_eq!(state, row_walk(&mut second, 8));
        assert_ne!(state, row_walk(&mut row_table(), 9), "blends fold in");
        let mut flipped = row_table();
        for frequency in &mut flipped {
            *frequency ^= 0x8000;
        }
        assert_ne!(
            state,
            row_walk(&mut flipped, 8),
            "the walk reads the table, not only its own state"
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
    fn spread_groups_the_row_kernel_lines_apart_from_the_walk_lines() {
        let log = "\
bench speed gate: decode cost 120.00 calibration steps/byte on A
bench speed gate: row kernel decode cost 9.00 row steps/byte on A
bench speed gate: row kernel decode cost 10.00 row steps/byte on A
";
        assert_eq!(spread(Kernel::Walk, log)[0].runs, 1);
        let rows = spread(Kernel::Rows, log);
        assert_eq!((rows[0].runs, rows[0].min, rows[0].max), (2, 9.0, 10.0));
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
