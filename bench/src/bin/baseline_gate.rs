//! CI entry point for [`mothergod_bench::baseline`]: measures mothergod's
//! bits/byte on the fixed regression-gate cases and either checks them
//! against the committed `bench/baseline.json` (exit non-zero on a
//! regression past `baseline::TOLERANCE_BITS`) or overwrites that file with
//! today's measurements. Wired into `.github/workflows/ci.yml` as the
//! required `ratio` check.
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
//! Usage: `cargo run -p mothergod-bench --release --bin baseline_gate --
//! check` (the default) or `... -- write` (after an accepted ratio change,
//! to commit the new numbers alongside it).

use mothergod_bench::baseline::{
    fingerprint, format_baseline, measure_all, parse_baseline, regressions,
};
use mothergod_bench::finals::stale_reason;
use mothergod_bench::repo_root;
use std::path::PathBuf;
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

fn main() -> ExitCode {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "check".to_string());
    let measured = measure_all();

    match mode.as_str() {
        "write" => {
            let text = format_baseline(&measured);
            if let Err(err) = std::fs::write(baseline_path(), &text) {
                eprintln!("failed to write {}: {err}", baseline_path().display());
                return ExitCode::FAILURE;
            }
            println!(
                "wrote {} cases to {}",
                measured.len(),
                baseline_path().display()
            );
            ExitCode::SUCCESS
        }
        "check" => {
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
            let regs = regressions(&baseline, &measured);
            let stale = stale_finals_reports(&baseline);

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
                    "if this regression is an accepted trade, update bench/baseline.json \
                     (`cargo run -p mothergod-bench --release --bin baseline_gate -- write`, \
                     then `cargo x fmt -- bench/baseline.json`) in the same PR and say why in \
                     the PR body."
                );
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
                    "regenerate the stale report(s) in this PR:\n\
                     \x20 cargo run -p mothergod-bench --release --features corpus-fetch --bin finals_report\n\
                     \x20 cargo run -p mothergod-bench --release --features corpus-fetch --bin silesia_report"
                );
            }

            if regs.is_empty() && stale.is_empty() {
                println!(
                    "bench baseline gate: {} cases, no regression, finals reports fresh",
                    measured.len()
                );
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        other => {
            eprintln!("unknown mode {other:?}, expected \"check\" or \"write\"");
            ExitCode::FAILURE
        }
    }
}
