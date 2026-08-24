//! CI entry point for [`mothergod_bench::baseline`]: measures mothergod's
//! bits/byte on the fixed regression-gate cases and either checks them
//! against the committed `bench/baseline.json` (exit non-zero on a
//! regression past `baseline::TOLERANCE_BITS`) or overwrites that file with
//! today's measurements. Not yet wired into `.github/workflows/ci.yml`
//! (`research/JOURNAL.md` S2-A35: that wiring needs a workflow-file push,
//! which needs `GH_ADMIN_TOKEN`); run by hand until it is.
//!
//! Usage: `cargo run -p mothergod-bench --release --bin baseline_gate --
//! check` (the default) or `... -- write` (after an accepted ratio change,
//! to commit the new numbers alongside it).

use mothergod_bench::baseline::{format_baseline, measure_all, parse_baseline, regressions};
use std::path::PathBuf;
use std::process::ExitCode;

/// `bench/baseline.json`, located relative to this crate's manifest so the
/// path is correct regardless of the caller's working directory.
fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("baseline.json")
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
            if regs.is_empty() {
                println!(
                    "bench baseline gate: {} cases, no regression",
                    measured.len()
                );
                ExitCode::SUCCESS
            } else {
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
                ExitCode::FAILURE
            }
        }
        other => {
            eprintln!("unknown mode {other:?}, expected \"check\" or \"write\"");
            ExitCode::FAILURE
        }
    }
}
