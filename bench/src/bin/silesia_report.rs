//! Runs the held-out-final report for Silesia (`research/corpus/
//! POLICY.md`'s held-out finals; `research/JOURNAL.md` S2-D1's remaining
//! "real Silesia finals numbers" line, S2-A45): fetches each of the 12
//! individually pinned Silesia files (`bench/corpus.toml`), decompresses
//! their bzip2 downloads, compresses each with `mothergod::compress` and
//! the three pinned reference compressors, and writes
//! `docs/benchmarks/silesia.md`.
//!
//! Silesia's counterpart to `finals_report` (Canterbury): a separate
//! binary because Canterbury ships as one tarball (`extract_canterbury`)
//! while Silesia's `bench/corpus.toml` entries are 12 independently
//! fetched bzip2 streams (`decompress_silesia`), one per file. Filters
//! the parsed manifest by `corpus == "silesia"` rather than a
//! second hardcoded file-name list, so `bench/corpus.toml` stays the one
//! place that names which files belong to Silesia.
//!
//! `finals_report`'s module doc measured `xml`, Silesia's smallest file
//! (5.3 MB), at 39s (~0.14 MB/s) single-threaded under this codec's
//! optimal-parse LZ; run one file after another, the full ~200 MB corpus
//! is on the order of half an hour, too slow for one PR's by-hand run.
//! `mothergod_bench::reference::measure_all` spreads the 12 independent
//! files across a thread per file instead, so the wall-clock cost is that
//! half hour divided by however many cores the runner has, not paid
//! serially.
//!
//! Usage: `cargo run -p mothergod-bench --release --features corpus-fetch
//! --bin silesia_report`. Markdown is linted, not formatted (`cargo x lint
//! -- docs/benchmarks/silesia.md` to check).

use mothergod_bench::baseline::{fingerprint, parse_baseline};
use mothergod_bench::corpus::{ManifestEntry, decompress_silesia, fetch_and_cache, parse_manifest};
use mothergod_bench::finals::{Versions, format_report};
use mothergod_bench::reference::{generated_at, measure_all, tool_version};
use mothergod_bench::repo_root;
use std::path::Path;
use std::process::ExitCode;

/// Fetches and decompresses every `bench/corpus.toml` entry whose
/// `corpus` field is `"silesia"`, sorted by name for a stable report
/// order. Filtering the parsed manifest rather than a second hardcoded
/// name list keeps `bench/corpus.toml` the one place naming Silesia's
/// files.
fn fetch_silesia_files(
    manifest: &[ManifestEntry],
    cache_dir: &Path,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut names: Vec<&str> = manifest
        .iter()
        .filter(|entry| entry.corpus == "silesia")
        .map(|entry| entry.name.as_str())
        .collect();
    names.sort_unstable();

    let mut files = Vec::with_capacity(names.len());
    for name in names {
        println!("fetching {name}...");
        let compressed = fetch_and_cache(manifest, name, cache_dir)
            .map_err(|err| format!("failed to fetch {name}: {err}"))?;
        let data = decompress_silesia(&compressed)
            .map_err(|err| format!("failed to decompress {name}: {err}"))?;
        files.push((name.to_string(), data));
    }
    Ok(files)
}

fn main() -> ExitCode {
    let root = repo_root();

    let baseline_text = match std::fs::read_to_string(root.join("bench/baseline.json")) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("failed to read bench/baseline.json: {err}");
            return ExitCode::FAILURE;
        }
    };
    let baseline = match parse_baseline(&baseline_text) {
        Ok(baseline) => baseline,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let baseline_fingerprint = fingerprint(&baseline);

    let manifest_text = match std::fs::read_to_string(root.join("bench/corpus.toml")) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("failed to read bench/corpus.toml: {err}");
            return ExitCode::FAILURE;
        }
    };
    let manifest = parse_manifest(&manifest_text);
    let cache_dir = root.join("target/bench-corpus-cache");

    let files = match fetch_silesia_files(&manifest, &cache_dir) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{err}");
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

    let report = format_report(
        "Silesia",
        "sun.aei.polsl.pl/~sdeor/corpus, 12 files pinned by URL + SHA-256 in `bench/corpus.toml`",
        &generated_at,
        &versions,
        &measurements,
        "silesia_report",
        &baseline_fingerprint,
    );

    let out_path = root.join("docs/benchmarks/silesia.md");
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
