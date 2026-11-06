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
//! rule `tests/golden.rs` sets for an encoder-only change. The held-out
//! finals reports are not checked here: their one writer is the publish PR
//! cut from the pass on main (ADR-0076, issue #1437).
//!
//! `check` also gates decode speed (issue #907): [`mothergod_bench::speed`]
//! measures the cost in calibration steps per decoded byte and fails past
//! `BASELINE_DECODE_COST`'s margin, with the CPU printed beside the number.
//!
//! `measure` and `delta <file>` are the research loop's arithmetic
//! (issue #1169): `measure` prints the gate cases' bits/byte in
//! `bench/baseline.json`'s own format, so a champion measured in the
//! session is a file; `delta <file>` prints every case's move against such
//! a file and the mean, and exits 0 whatever the sign, because it
//! reports and `check` gates. Neither touches `bench/baseline.json`, and
//! neither reads a reach case (issue #1434), which `check` and `write`
//! add and whose length would break the mean's denominator. Both read a
//! [`Probe`]: by default [`mothergod_bench::baseline::cases`] through
//! `compress`; `--level <1-4>` compresses at the CLI's level of that digit,
//! `--len <bytes>` generates every case at that length, so a reach past
//! `lz::WIDE_WINDOW` is measurable, and `--sealed` reads the two sealed
//! cases instead, the keeper's one read (`research/README.md`). A `delta`
//! is meaningful only against a file `measure` wrote with the same
//! options, so `measure` records them in the file and `delta` refuses a
//! file whose record differs (issue #1894), before it measures anything.
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
//! `... -- spread [--fp] < logs` (see [`SPREAD_USAGE`]). `measure` and
//! `delta <file>` take the [`parse_probe`] options after their own
//! arguments.

use mothergod_bench::baseline::{
    CaseSet, Probe, Snapshot, deltas, format_baseline, hold_at_best, mean_delta, measure_gate,
    output_changes, parse_baseline, regressions,
};
use mothergod_bench::field::level_of_digit;
use mothergod_bench::reference::cpu_model;
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
    print_cost(speed::Kernel::Fp);
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

/// Every case's move of `measure(probe)` against the baseline file at
/// `path`, one line per case, then the mean, named by the set measured.
/// `Err` when the file cannot be read or parsed, when the probe it records
/// is not `probe` (issue #1894: a file measured at another `--level` or
/// `--len` shares every case name, and each move would be the gap between
/// two unrelated reads), or when a measured case is absent from it: the
/// mean's denominator is every measured case or there is no mean (issue
/// #1892, where a sealed read of a train file printed a mean of
/// `+0.000000` over 0 cases and exited 0). A file with no probe record,
/// `bench/baseline.json` among them, reads as [`Probe::default`]. The
/// probe is checked before `measure` runs, so a refusal costs no
/// measurement. A case only the file holds is skipped, so
/// `bench/baseline.json`, which adds the reach cases, still reads. A
/// regression is a line in the report, never an `Err`, because `delta`
/// reports and `check` gates.
fn delta_report(
    path: &Path,
    probe: Probe,
    measure: impl FnOnce(Probe) -> BTreeMap<String, f64>,
) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let baseline = parse_baseline(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    let recorded = match baseline.probe {
        Some(recorded) => format!("was measured with `{recorded}`"),
        None => format!("records no probe, read as `{}`", Probe::default()),
    };
    if baseline.probe.unwrap_or_default() != probe {
        return Err(format!(
            "{}: {recorded}, this delta reads `{probe}`; \
             delta reads a file `measure` wrote with the same options",
            path.display()
        ));
    }
    let set = probe.set;
    let measured = &measure(probe);
    let baseline = baseline.bpb();
    let missing: Vec<&str> = measured
        .keys()
        .filter(|name| !baseline.contains_key(*name))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "{}: holds {} of the {} {} cases measured (missing: {}); \
             delta reads a file `measure` wrote with the same options",
            path.display(),
            measured.len() - missing.len(),
            measured.len(),
            set.label(),
            missing.join(", ")
        ));
    }
    let found = deltas(&baseline, measured);
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
        "{} mean {:+.6} bits/byte over {} cases",
        set.label(),
        mean_delta(&found),
        found.len()
    )
    .expect("writing to a String never fails");
    Ok(out)
}

/// The `check` mode: gates `measured` against the committed baseline and the
/// decode speed, naming each failure and its fix.
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

    if regs.is_empty() && changed.is_empty() && slow.is_none() {
        println!(
            "bench baseline gate: {} cases, no regression, output unchanged, decode speed within margin",
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

/// The [`Probe`] `measure`'s and `delta`'s options select, from the
/// arguments after the mode's own: `--level <1-4>`, `--len <bytes>` (at
/// least one) and `--sealed`, in any order, each at most once in effect
/// (a repeat overrides). `Err` names the first argument it cannot read.
fn parse_probe(args: impl IntoIterator<Item = String>) -> Result<Probe, String> {
    let mut probe = Probe::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sealed" => probe.set = CaseSet::Sealed,
            "--level" => {
                let value = args.next().unwrap_or_default();
                probe.level = level_of_digit(&value)
                    .ok_or_else(|| format!("--level {value:?}: the levels are 1 to 4"))?;
            }
            "--len" => {
                let value = args.next().unwrap_or_default();
                probe.len = value
                    .parse()
                    .ok()
                    .filter(|&len| len > 0)
                    .ok_or_else(|| format!("--len {value:?}: a byte count of at least 1"))?;
            }
            other => {
                return Err(format!(
                    "unknown option {other:?}, expected --level, --len or --sealed"
                ));
            }
        }
    }
    Ok(probe)
}

/// `spread`'s `--help`: what it reads and the pipe that fetches it.
const SPREAD_USAGE: &str = "\
usage: baseline_gate spread [--fp] < logs

Reads CI log text on stdin, keeps the `bench speed gate: decode cost ...` lines
`baseline_gate speed` prints, and prints runs, min, median, max and range
((max - min) / min) of the cost per CPU model. `--fp` reads the
`bench speed gate: fp kernel decode cost ...` lines instead, the candidate
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
            Some("--fp") => speed::Kernel::Fp,
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
    let baseline_file = if mode == "delta" {
        let Some(file) = args.next() else {
            eprintln!(
                "usage: baseline_gate delta <baseline file> [--level <1-4>] [--len <bytes>] [--sealed]"
            );
            return ExitCode::FAILURE;
        };
        Some(PathBuf::from(file))
    } else {
        None
    };
    let probe = if matches!(mode.as_str(), "measure" | "delta") {
        match parse_probe(args) {
            Ok(probe) => probe,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        Probe::default()
    };
    match mode.as_str() {
        "measure" => {
            print!("{}", format_baseline(&probe.measure()));
            ExitCode::SUCCESS
        }
        "delta" => {
            let path = baseline_file.expect("delta always reads its file argument above");
            match delta_report(&path, probe, |probe| probe.measure().bpb()) {
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
        "write" => write(&measure_gate()),
        "check" => check(&measure_gate()),
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
    use super::{
        BTreeMap, CaseSet, PathBuf, Probe, Snapshot, delta_report, format_baseline, parse_probe,
    };
    use mothergod::Level;
    use mothergod_bench::baseline::Entry;

    fn scratch_baseline(name: &str, text: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("mothergod-delta-{}-{name}", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path
    }

    /// A baseline file of `(name, bpb)` rows recording `probe`, in the
    /// shape `measure` emits, or `write` when `probe` is `None`.
    fn baseline_text(probe: Option<Probe>, rows: &[(&str, u32)]) -> String {
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
        format_baseline(&Snapshot { probe, cases })
    }

    /// The default probe with the sealed set.
    fn sealed() -> Probe {
        Probe {
            set: CaseSet::Sealed,
            ..Probe::default()
        }
    }

    /// `delta_report` against `rows` recording `recorded`, read by `probe`,
    /// with `measured` as the measurement.
    fn delta(
        name: &str,
        recorded: Option<Probe>,
        rows: &[(&str, u32)],
        probe: Probe,
        measured: &[(&str, f64)],
    ) -> Result<String, String> {
        let path = scratch_baseline(name, &baseline_text(recorded, rows));
        let measured = measured
            .iter()
            .map(|&(name, bpb)| (name.to_string(), bpb))
            .collect::<BTreeMap<_, _>>();
        let report = delta_report(&path, probe, |_| measured);
        std::fs::remove_file(&path).unwrap();
        report
    }

    #[test]
    fn delta_report_is_ok_on_a_regression_past_the_gate_tolerance() {
        let report = delta(
            "regression",
            Some(Probe::default()),
            &[("a", 1), ("b", 2)],
            Probe::default(),
            &[("a", 5.0), ("b", 1.0)],
        )
        .expect("a regression is a report, not an error");
        assert_eq!(
            report,
            "a: 1.000000 -> 5.000000 bits/byte (+4.000000)\n\
             b: 2.000000 -> 1.000000 bits/byte (-1.000000)\n\
             train mean +1.500000 bits/byte over 2 cases\n"
        );
    }

    #[test]
    fn delta_report_names_the_sealed_mean_on_a_sealed_read() {
        let report = delta(
            "sealed",
            Some(sealed()),
            &[("a", 2)],
            sealed(),
            &[("a", 1.0)],
        )
        .unwrap();
        assert!(
            report.ends_with("sealed mean -1.000000 bits/byte over 1 cases\n"),
            "{report}"
        );
    }

    #[test]
    fn delta_report_reads_a_file_without_a_probe_record_as_the_default_probe() {
        // `bench/baseline.json`'s shape: no record, a reach case no probe reads.
        let rows = [("a", 2), ("reach", 3)];
        let report = delta("gate", None, &rows, Probe::default(), &[("a", 1.0)]).unwrap();
        assert_eq!(
            report,
            "a: 2.000000 -> 1.000000 bits/byte (-1.000000)\n\
             train mean -1.000000 bits/byte over 1 cases\n"
        );
        let err = delta("gate-sealed", None, &rows, sealed(), &[]).unwrap_err();
        assert!(
            err.contains(
                "records no probe, read as `--level 3 --len 50000`, \
                 this delta reads `--level 3 --len 50000 --sealed`"
            ),
            "{err}"
        );
    }

    #[test]
    fn delta_report_refuses_a_file_measured_at_another_level_or_length_before_measuring() {
        // #1894: the case names agree, so only the record tells the reads apart.
        let recorded = Probe {
            len: 20_000,
            ..Probe::default()
        };
        for (probe, named) in [
            (
                Probe {
                    len: 50_000,
                    ..recorded
                },
                "this delta reads `--level 3 --len 50000`",
            ),
            (
                Probe {
                    level: Level::Fast,
                    ..recorded
                },
                "this delta reads `--level 2 --len 20000`",
            ),
            (
                sealed(),
                "this delta reads `--level 3 --len 50000 --sealed`",
            ),
        ] {
            let path = scratch_baseline("mismatch", &baseline_text(Some(recorded), &[("a", 2)]));
            let result = delta_report(&path, probe, |_| panic!("measured past a refusal"));
            std::fs::remove_file(&path).unwrap();
            let err = result.expect_err(named);
            assert!(
                err.contains("was measured with `--level 3 --len 20000`"),
                "{err}"
            );
            assert!(err.contains(named), "{err}");
        }
    }

    #[test]
    fn delta_report_errs_naming_a_measured_case_the_file_lacks() {
        // #1892: a sealed read of a file holding no sealed case has no mean.
        let disjoint = delta(
            "disjoint",
            Some(sealed()),
            &[("train", 2)],
            sealed(),
            &[("s1", 1.0), ("s2", 1.0)],
        );
        let err = disjoint.expect_err("no shared case is no mean");
        assert!(
            err.contains("holds 0 of the 2 sealed cases measured (missing: s1, s2)"),
            "{err}"
        );
        // One shared case of two still leaves the mean without a denominator.
        let partial = delta(
            "partial",
            None,
            &[("train", 2)],
            Probe::default(),
            &[("s1", 1.0), ("train", 1.0)],
        );
        let err = partial.expect_err("a missing case is no mean");
        assert!(
            err.contains("holds 1 of the 2 train cases measured (missing: s1)"),
            "{err}"
        );
    }

    #[test]
    fn delta_report_errs_on_an_unreadable_or_malformed_file() {
        let missing = std::env::temp_dir().join("mothergod-delta-no-such-file");
        assert!(delta_report(&missing, Probe::default(), |_| BTreeMap::new()).is_err());
        let path = scratch_baseline("malformed", "{\n  not valid\n}\n");
        let result = delta_report(&path, Probe::default(), |_| BTreeMap::new());
        std::fs::remove_file(&path).unwrap();
        assert!(result.is_err());
    }

    fn probe(args: &[&str]) -> Result<Probe, String> {
        parse_probe(args.iter().map(ToString::to_string))
    }

    #[test]
    fn no_option_is_the_default_probe() {
        assert_eq!(probe(&[]), Ok(Probe::default()));
    }

    #[test]
    fn every_option_moves_its_own_field() {
        let read = probe(&["--len", "4000000", "--sealed", "--level", "2"]).unwrap();
        assert_eq!(
            read,
            Probe {
                set: CaseSet::Sealed,
                level: Level::Fast,
                len: 4_000_000,
            }
        );
    }

    #[test]
    fn an_unreadable_option_is_an_error_naming_it() {
        for (args, named) in [
            (&["--level", "5"][..], "\"5\""),
            (&["--level", "22"], "\"22\""),
            (&["--level"], "\"\""),
            (&["--len", "0"], "\"0\""),
            (&["--len", "-1"], "\"-1\""),
            (&["--len"], "\"\""),
            (&["--train"], "\"--train\""),
        ] {
            let err = probe(args).expect_err(&format!("{args:?}"));
            assert!(err.contains(named), "{args:?}: {err}");
        }
    }
}
