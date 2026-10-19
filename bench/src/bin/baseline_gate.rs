//! CI entry point for [`mothergod_bench::baseline`]: measures mothergod's
//! bits/byte on the fixed regression-gate cases and either checks them
//! against the committed `bench/baseline.json` (exit non-zero on a
//! regression past `baseline::TOLERANCE_BITS`) or rewrites that file with
//! today's measurements, holding any case that grew at its old value. Wired
//! into `.github/workflows/ci.yml` as the required `ratio` check.
//!
//! `check` also fails when a case compresses to different bytes than the
//! digest `bench/baseline.json` commits (issue #1182), whatever the size:
//! a refactor's "output unchanged" is this check's result, and an output
//! change is declared by regenerating the baseline in the same PR, the
//! rule `tests/golden.rs` sets for an encoder-only change. The digests stay
//! out of the fingerprint below.
//!
//! `check` also verifies the held-out-final reports
//! (`docs/benchmarks/canterbury.md`, `docs/benchmarks/silesia.md`) still
//! embed this exact `bench/baseline.json`'s fingerprint (issue #327): a
//! baseline change is a deliberate signal the codec's measured behavior
//! changed, so those two reports (real Silesia/Canterbury numbers, not the
//! synthetic gate cases here) can now be stale. A content check against the
//! committed reports, not a regeneration: `finals_report`/`silesia_report`
//! fetch real corpora over the network, too slow and non-hermetic for this
//! required check to run on every PR.
//!
//! `check` also gates decode speed (issue #907): [`mothergod_bench::speed`]
//! measures the cost in calibration steps per decoded byte and fails past
//! `BASELINE_DECODE_COST`'s margin, with the CPU printed beside the number.
//!
//! `measure` and `delta <file>` are the research loop's arithmetic
//! (issue #1169): `measure` prints the gate cases' bits/byte in
//! `bench/baseline.json`'s own format, so a champion measured in the
//! session is a file; `delta <file>` prints every case's move against such
//! a file and the train mean, and exits 0 whatever the sign, because it
//! reports and `check` gates. Neither touches `bench/baseline.json`, and
//! both measure [`mothergod_bench::baseline::cases`] only, never a sealed
//! case.
//!
//! `spread` is the table #1184 validates on (issue #1186): it reads CI log
//! text on stdin and prints per-CPU runs, min, median, max and range of the
//! cost lines `speed` prints, so a claim about the pool's spread cites a
//! command a reviewer reruns. The line format is owned by
//! [`mothergod_bench::speed::cost_line`], which the parser sits beside.
//!
//! Usage: `cargo run -p mothergod-bench --release --bin baseline_gate --
//! check` (the default) or `... -- write` (after a codec change, to
//! record the shrunk and new cases; a grown case is held at its old value,
//! so an accepted loosening is a hand edit of the JSON) or `... -- speed` (print the
//! decode cost, to commit as `BASELINE_DECODE_COST` after an accepted
//! slowdown) or `... -- measure` or `... -- delta <file>` or
//! `... -- spread [--rows] < logs` (see [`SPREAD_USAGE`]).

use mothergod_bench::baseline::{
    Snapshot, deltas, fingerprint, format_baseline, hold_at_best, mean_delta, measure_all,
    output_changes, parse_baseline, regressions,
};
use mothergod_bench::finals::stale_reason;
use mothergod_bench::reference::cpu_model;
use mothergod_bench::repo_root;
use mothergod_bench::speed;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// `bench/baseline.json`, located relative to this crate's manifest so the
/// path is correct regardless of the caller's working directory.
fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("baseline.json")
}

/// Held-out-final reports that must track `bench/baseline.json`'s
/// fingerprint (issue #327), repo-root-relative.
const FINALS_REPORTS: [&str; 2] = [
    "docs/benchmarks/canterbury.md",
    "docs/benchmarks/silesia.md",
];

/// Reports a reason for every [`FINALS_REPORTS`] entry whose embedded
/// fingerprint does not match `baseline`'s. A report this checkout does
/// not have (unreadable) counts as stale too, named as such rather than
/// silently skipped.
fn stale_finals_reports(baseline: &std::collections::BTreeMap<String, f64>) -> Vec<String> {
    let current = fingerprint(baseline);
    FINALS_REPORTS
        .iter()
        .filter_map(|relative_path| {
            let path = repo_root().join(relative_path);
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    stale_reason(&text, &current).map(|reason| format!("{relative_path} {reason}"))
                }
                Err(err) => Some(format!("{relative_path}: failed to read: {err}")),
            }
        })
        .collect()
}

/// Measures [`speed::decode_cost`] on the gate cases once, prints it under
/// each [`speed::Kernel`] with the CPU it ran on, and returns the gating
/// [`speed::Kernel::Walk`] cost. The candidate's line gates nothing (#1184).
fn measure_decode_cost() -> f64 {
    let (seconds, bytes) = speed::decode_seconds(&mothergod_bench::baseline::cases());
    let cpu = cpu_model().unwrap_or_else(|| "unknown CPU".to_string());
    let print_cost = |kernel: speed::Kernel| {
        let cost = speed::decode_cost(kernel, seconds, bytes, kernel.seconds());
        println!("{}", speed::cost_line(kernel, cost, &cpu));
        cost
    };
    let gating = print_cost(speed::Kernel::Walk);
    print_cost(speed::Kernel::Rows);
    gating
}

/// Names a speed regression and the way to accept it.
fn report_slowdown(slow: &speed::Slowdown) {
    eprintln!(
        "bench speed gate: decode cost {:.2} -> {:.2} calibration steps/byte \
         (+{:.1}%, margin {:.0}%)",
        slow.baseline,
        slow.measured,
        slow.fraction() * 100.0,
        speed::MAX_SLOWDOWN * 100.0
    );
    eprintln!(
        "if this slowdown is an accepted trade, set `speed::BASELINE_DECODE_COST` to the \
         `-- speed` output in the same PR and say why in the PR body."
    );
}

/// Every case's move of `measured` against the baseline file at `path`,
/// one line per case, then the train mean. `Err` only when the file cannot
/// be read or parsed: a regression is a line in the report, never an `Err`,
/// because `delta` reports and `check` gates.
fn delta_report(path: &Path, measured: &BTreeMap<String, f64>) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let baseline = parse_baseline(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    let found = deltas(&baseline.bpb(), measured);
    let mut out = String::new();
    for case in &found {
        writeln!(
            out,
            "{}: {:.6} -> {:.6} bits/byte ({:+.6})",
            case.name,
            case.baseline_bpb,
            case.measured_bpb,
            case.delta()
        )
        .expect("writing to a String never fails");
    }
    writeln!(
        out,
        "train mean {:+.6} bits/byte over {} cases",
        mean_delta(&found),
        found.len()
    )
    .expect("writing to a String never fails");
    Ok(out)
}

/// The `check` mode: gates `measured` against the committed baseline, the
/// decode speed and the finals reports, naming each failure and its fix.
fn check(measured: &Snapshot) -> ExitCode {
    let text = match std::fs::read_to_string(baseline_path()) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("failed to read {}: {err}", baseline_path().display());
            return ExitCode::FAILURE;
        }
    };
    let baseline = match parse_baseline(&text) {
        Ok(baseline) => baseline,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let regs = regressions(&baseline.bpb(), &measured.bpb());
    let changed = output_changes(&baseline, measured);
    let slow = speed::slowdown(
        speed::BASELINE_DECODE_COST,
        measure_decode_cost(),
        speed::MAX_SLOWDOWN,
    );
    let stale = stale_finals_reports(&baseline.bpb());

    if !regs.is_empty() {
        eprintln!("bench baseline gate: {} case(s) regressed:", regs.len());
        for reg in &regs {
            eprintln!(
                "  {}: {:.6} -> {:.6} bits/byte (+{:.6})",
                reg.name,
                reg.baseline_bpb,
                reg.measured_bpb,
                reg.delta()
            );
        }
        eprintln!(
            "if this regression is an accepted trade, raise the case's value in \
             bench/baseline.json by hand in the same PR and say why in the PR body \
             (`-- write` holds a grown case at its old value, issue #1230)."
        );
    }
    if !changed.is_empty() {
        eprintln!(
            "bench baseline gate: {} case(s) compress to different bytes than bench/baseline.json \
             commits: {}",
            changed.len(),
            changed.join(", ")
        );
        eprintln!(
            "an intended output change regenerates the baseline in this PR and says so in the PR body:\n\
             \x20 cargo run -p mothergod-bench --release --bin baseline_gate -- write"
        );
    }
    if let Some(slow) = &slow {
        report_slowdown(slow);
    }
    if !stale.is_empty() {
        eprintln!(
            "bench baseline gate: {} held-out-final report(s) stale (issue #327):",
            stale.len()
        );
        for reason in &stale {
            eprintln!("  {reason}");
        }
        eprintln!(
            "regenerate the stale report(s) in this PR, locally:\n\
             \x20 cargo run -p mothergod-bench --release --features corpus-fetch --bin finals_report\n\
             \x20 cargo run -p mothergod-bench --release --features corpus-fetch --bin silesia_report\n\
             or on a runner, silesia_report being about 35 minutes (docs/benchmarks/README.md):\n\
             \x20 GH_TOKEN=\"$GH_WORKFLOW_TOKEN\" gh workflow run bench-report.yml --ref <branch>\n\
             \x20 gh run download <run-id> -n bench-report"
        );
    }

    if regs.is_empty() && changed.is_empty() && slow.is_none() && stale.is_empty() {
        println!(
            "bench baseline gate: {} cases, no regression, output unchanged, decode speed within margin, finals reports fresh",
            measured.cases.len()
        );
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The `write` mode: records `measured` over the committed baseline, holding
/// every case that grew at its old value (issue #1230) and naming each with
/// both numbers. A deliberate loosening is a hand edit of the JSON, never
/// this mode.
fn write(measured: &Snapshot) -> ExitCode {
    let path = baseline_path();
    let baseline = match std::fs::read_to_string(&path) {
        Ok(text) => match parse_baseline(&text) {
            Ok(baseline) => baseline,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::FAILURE;
            }
        },
        Err(err) => {
            eprintln!("failed to read {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let (bpb, held) = hold_at_best(&baseline.bpb(), &measured.bpb());
    let written = measured.with_bpb(&bpb);
    if let Err(err) = std::fs::write(&path, format_baseline(&written)) {
        eprintln!("failed to write {}: {err}", path.display());
        return ExitCode::FAILURE;
    }
    for case in &held {
        println!(
            "held {}: kept {:.6}, measured {:.6} bits/byte ({:+.6})",
            case.name,
            case.baseline_bpb,
            case.measured_bpb,
            case.delta()
        );
    }
    println!(
        "wrote {} cases to {}, held {} at the old value",
        written.cases.len(),
        path.display(),
        held.len()
    );
    ExitCode::SUCCESS
}

/// `spread`'s `--help`: what it reads and the pipe that fetches it.
const SPREAD_USAGE: &str = "\
usage: baseline_gate spread [--rows] < logs

Reads CI log text on stdin, keeps the `bench speed gate: decode cost ...` lines
`baseline_gate speed` prints, and prints runs, min, median, max and range
((max - min) / min) of the cost per CPU model. `--rows` reads the
`bench speed gate: row kernel decode cost ...` lines instead, the candidate
kernel #1184 validates; only runs of a build that prints them carry any.

The `ratio` job's log of every `ci.yml` push run on main:

  gh run list --workflow ci.yml --branch main --event push --limit 200 \\
      --json databaseId --jq '.[].databaseId' \\
    | while read -r id; do gh run view \"$id\" --log; done \\
    | cargo run -p mothergod-bench --release --bin baseline_gate -- spread
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "check".to_string());
    if mode == "spread" {
        let kernel = match args.next().as_deref() {
            Some("--help" | "-h") => {
                print!("{SPREAD_USAGE}");
                return ExitCode::SUCCESS;
            }
            Some("--rows") => speed::Kernel::Rows,
            None => speed::Kernel::Walk,
            Some(other) => {
                eprintln!("unknown spread option {other:?}; see `spread --help`");
                return ExitCode::FAILURE;
            }
        };
        let mut log = String::new();
        if let Err(err) = std::io::stdin().read_to_string(&mut log) {
            eprintln!("failed to read stdin: {err}");
            return ExitCode::FAILURE;
        }
        let rows = speed::spread(kernel, &log);
        if rows.is_empty() {
            eprintln!("no cost line of that kernel on stdin; see `spread --help`");
            return ExitCode::FAILURE;
        }
        print!("{}", speed::format_spread(&rows));
        return ExitCode::SUCCESS;
    }
    if mode == "speed" {
        measure_decode_cost();
        return ExitCode::SUCCESS;
    }
    let baseline_file = args.next().map(PathBuf::from);
    if mode == "delta" && baseline_file.is_none() {
        eprintln!("usage: baseline_gate delta <baseline file>");
        return ExitCode::FAILURE;
    }
    let measured = measure_all();

    match mode.as_str() {
        "measure" => {
            print!("{}", format_baseline(&measured));
            ExitCode::SUCCESS
        }
        "delta" => {
            let path = baseline_file.expect("checked above");
            match delta_report(&path, &measured.bpb()) {
                Ok(report) => {
                    print!("{report}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            }
        }
        "write" => write(&measured),
        "check" => check(&measured),
        other => {
            eprintln!(
                "unknown mode {other:?}, expected \"check\", \"write\", \"speed\", \"spread\", \"measure\" or \"delta\""
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BTreeMap, PathBuf, Snapshot, delta_report, format_baseline};
    use mothergod_bench::baseline::Entry;

    fn scratch_baseline(name: &str, text: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("mothergod-delta-{}-{name}", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path
    }

    /// A baseline file of `(name, bpb)` rows in the shape `write` emits.
    fn baseline_text(rows: &[(&str, u32)]) -> String {
        let cases = rows
            .iter()
            .map(|&(name, bpb)| {
                (
                    name.to_string(),
                    Entry {
                        bpb: f64::from(bpb),
                        digest: 0,
                    },
                )
            })
            .collect();
        format_baseline(&Snapshot { cases })
    }

    #[test]
    fn delta_report_is_ok_on_a_regression_past_the_gate_tolerance() {
        let path = scratch_baseline("regression", &baseline_text(&[("a", 1), ("b", 2)]));
        let measured = BTreeMap::from([("a".to_string(), 5.0), ("b".to_string(), 1.0)]);
        let report =
            delta_report(&path, &measured).expect("a regression is a report, not an error");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            report,
            "a: 1.000000 -> 5.000000 bits/byte (+4.000000)\n\
             b: 2.000000 -> 1.000000 bits/byte (-1.000000)\n\
             train mean +1.500000 bits/byte over 2 cases\n"
        );
    }

    #[test]
    fn delta_report_errs_on_an_unreadable_or_malformed_file() {
        let missing = std::env::temp_dir().join("mothergod-delta-no-such-file");
        assert!(delta_report(&missing, &BTreeMap::new()).is_err());
        let path = scratch_baseline("malformed", "{\n  not valid\n}\n");
        let result = delta_report(&path, &BTreeMap::new());
        std::fs::remove_file(&path).unwrap();
        assert!(result.is_err());
    }
}
