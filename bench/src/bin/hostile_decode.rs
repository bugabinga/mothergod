//! Times the worst-case decode of each coded method at its output ceiling,
//! one after the other in one process, and prints each wall time, the
//! `Cm` time as a share of the `Lz` time, and the CPU (issue #1915).
//! `cm_path::MAX_OUTPUT_LEN` is sized so a hostile `Cm` frame costs no more
//! than a hostile `Lz` one, so the exit code is non-zero when it costs more.
//! The payloads and why they are the worst case:
//! [`mothergod_bench::hostile`].
//!
//! Usage: `cargo run -p mothergod-bench --release --bin hostile_decode`.
//! Expect minutes per method: the run is the measurement, not a gate CI
//! can afford per PR.

use mothergod_bench::hostile::{Method, time};
use mothergod_bench::reference::cpu_model;
use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args().len() > 1 {
        eprintln!("usage: hostile_decode (takes no arguments)");
        return ExitCode::FAILURE;
    }
    if cfg!(debug_assertions) {
        eprintln!("hostile_decode times the release build; rerun with --release");
        return ExitCode::FAILURE;
    }
    let cpu = cpu_model().unwrap_or_else(|| "unknown CPU".to_string());
    println!("hostile decode at each ceiling on {cpu}");
    let [lz, cm] = Method::ALL.map(|method| {
        let reading = time(method, method.ceiling());
        println!("{}", reading.line());
        reading
    });
    let share = cm.seconds / lz.seconds;
    println!("Cm / Lz: {:.0}%", share * 100.0);
    if share > 1.0 {
        eprintln!(
            "a hostile Cm frame at cm_path::MAX_OUTPUT_LEN decodes slower than a hostile Lz \
             frame at codec::MAX_DECODED_LEN"
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
