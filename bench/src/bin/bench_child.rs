//! The child `reference::measure_all` times mothergod in (issue #1340): run
//! as `bench_child --child compress` or `--child decompress`, it filters
//! stdin to stdout through `mothergod::compress` or `mothergod::decompress`.
//! The report binaries serve the same mode themselves and re-run their own
//! path; this one exists so a test, which has no such `main`, has a program
//! to hand `measure_all`.

use std::process::ExitCode;

fn main() -> ExitCode {
    mothergod_bench::field::run_child_if_requested().unwrap_or_else(|| {
        eprintln!("usage: bench_child --child compress|decompress < input > output");
        ExitCode::FAILURE
    })
}
