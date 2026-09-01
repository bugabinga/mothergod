//! Runs the held-out-final report for Canterbury (`research/corpus/
//! POLICY.md`'s held-out finals; `research/JOURNAL.md` S2-D1's remaining
//! "a gzip/zstd/xz reference column and real Silesia/Canterbury numbers"
//! line): fetches the pinned `cantrbry.tar.gz` (`bench/corpus.toml`),
//! compresses each file with `mothergod::compress` and the three pinned
//! reference compressors, and writes `docs/benchmarks/canterbury.md`.
//!
//! Canterbury only; `silesia_report` is the Silesia counterpart, its own
//! binary because Canterbury's single tarball (`extract_canterbury`) and
//! Silesia's 12 individually pinned files (`decompress_silesia`) fetch
//! differently. Measured throughput on this codec's optimal-parse LZ:
//! `xml`, the smallest Silesia file (5.3 MB), took 39s (~0.14 MB/s)
//! single-threaded. Canterbury (~2.7 MB total across 11 files) finishes
//! in under a minute either way; Silesia's ~200 MB would take on the
//! order of half an hour run file-after-file, which is why
//! `mothergod_bench::reference::measure_all` (shared by both binaries)
//! spreads the per-file measurements across a thread each instead.
//!
//! Usage: `cargo run -p mothergod-bench --release --features corpus-fetch
//! --bin finals_report`. Markdown is linted, not formatted (`cargo x lint
//! -- docs/benchmarks/canterbury.md` to check).

use mothergod_bench::baseline::load_and_fingerprint;
use mothergod_bench::corpus::{extract_canterbury, fetch_and_cache, parse_manifest};
use mothergod_bench::finals::{Provenance, Versions, format_report};
use mothergod_bench::reference::{generated_at, machine_info, measure_all, tool_version};
use mothergod_bench::repo_root;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = repo_root();

    let baseline_fingerprint = match load_and_fingerprint(&root) {
        Ok(fingerprint) => fingerprint,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let manifest_text = match std::fs::read_to_string(root.join("bench/corpus.toml")) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("failed to read bench/corpus.toml: {err}");
            return ExitCode::FAILURE;
        }
    };
    let manifest = parse_manifest(&manifest_text);
    let cache_dir = root.join("target/bench-corpus-cache");

    let compressed = match fetch_and_cache(&manifest, "cantrbry", &cache_dir) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("failed to fetch the Canterbury corpus: {err}");
            return ExitCode::FAILURE;
        }
    };
    let files = match extract_canterbury(&compressed) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("failed to extract the Canterbury tarball: {err}");
            return ExitCode::FAILURE;
        }
    };

    let (gzip, zstd, xz) = (
        tool_version("gzip"),
        tool_version("zstd"),
        tool_version("xz"),
    );
    let versions = match (gzip, zstd, xz) {
        (Ok(gzip), Ok(zstd), Ok(xz)) => Versions { gzip, zstd, xz },
        (gzip, zstd, xz) => {
            eprintln!(
                "failed to read a reference compressor's version: gzip={gzip:?} zstd={zstd:?} xz={xz:?}"
            );
            return ExitCode::FAILURE;
        }
    };

    let measurements = match measure_all(&files) {
        Ok(measurements) => measurements,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let generated_at = match generated_at() {
        Ok(stamp) => stamp,
        Err(err) => {
            eprintln!("failed to get the current timestamp: {err}");
            return ExitCode::FAILURE;
        }
    };

    let machine = machine_info();
    let report = format_report(
        "Canterbury",
        "corpus.canterbury.ac.nz `cantrbry.tar.gz`, pinned by URL + SHA-256 in `bench/corpus.toml`",
        &generated_at,
        &measurements,
        "finals_report",
        &Provenance {
            versions: &versions,
            baseline_fingerprint: &baseline_fingerprint,
            machine: &machine,
        },
    );

    let out_path = root.join("docs/benchmarks/canterbury.md");
    if let Err(err) = std::fs::write(&out_path, &report) {
        eprintln!("failed to write {}: {err}", out_path.display());
        return ExitCode::FAILURE;
    }
    println!(
        "wrote {} file measurements to {}",
        measurements.len(),
        out_path.display()
    );
    ExitCode::SUCCESS
}
