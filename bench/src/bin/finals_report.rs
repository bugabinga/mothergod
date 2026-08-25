//! Runs the held-out-final report for Canterbury (`research/corpus/
//! POLICY.md`'s held-out finals; `research/JOURNAL.md` S2-D1's remaining
//! "a gzip/zstd/xz reference column and real Silesia/Canterbury numbers"
//! line): fetches the pinned `cantrbry.tar.gz` (`bench/corpus.toml`),
//! compresses each file with `mothergod::compress` and the three pinned
//! reference compressors, and writes `docs/benchmarks/canterbury.md`.
//!
//! Canterbury only, not Silesia yet. Measured throughput on this codec's
//! optimal-parse LZ: `xml`, the smallest Silesia file (5.3 MB), took 39s
//! (~0.14 MB/s) end to end. The full ~200 MB Silesia corpus would run
//! `mothergod::compress` for on the order of half an hour, too slow for
//! one PR's by-hand run; Canterbury (~2.7 MB total across 11 files)
//! finishes in under a minute. Silesia numbers stay remaining S2-D1
//! scope, most naturally landing behind the scheduled `corpus-fetch`
//! workflow (issue #231) once `GH_ADMIN_TOKEN` wiring exists, rather than
//! forced into a slow by-hand run here.
//!
//! Usage: `cargo run -p mothergod-bench --release --features corpus-fetch
//! --bin finals_report`. Markdown is linted, not formatted (`cargo x lint
//! -- docs/benchmarks/canterbury.md` to check).

use mothergod_bench::corpus::{extract_canterbury, fetch_and_cache, parse_manifest};
use mothergod_bench::finals::{FileMeasurement, Versions, format_report};
use mothergod_bench::reference::{compressed_len, tool_version};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

/// The workspace root, located relative to this crate's manifest so the
/// result is correct regardless of the caller's working directory.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("bench/Cargo.toml has a parent directory (the workspace root)")
        .to_path_buf()
}

/// `date -u`'s current UTC timestamp, `YYYY-MM-DDTHH:MM:SSZ`, the same
/// shape `site-status/src/bin/generate.rs` stamps `site/status-data.json`
/// with. Shelling out rather than a `SystemTime`-to-calendar conversion:
/// this crate stays zero-dependency outside `corpus-fetch`'s own five
/// crates, and pulling in a datetime crate just for one timestamp isn't
/// worth a sixth.
fn generated_at() -> Result<String, String> {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .map_err(|err| format!("failed to run date: {err}"))?;
    if !output.status.success() {
        return Err(format!("date exited with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|err| format!("date produced non-UTF-8 output: {err}"))
}

fn main() -> ExitCode {
    let root = repo_root();

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

    let mut measurements = Vec::with_capacity(files.len());
    for (name, data) in &files {
        println!("measuring {name} ({} bytes)...", data.len());
        let mothergod_len = mothergod::compress(data).len();
        let gzip_len = match compressed_len("gzip", &["-9", "-c"], data) {
            Ok(len) => len,
            Err(err) => {
                eprintln!("gzip failed on {name}: {err}");
                return ExitCode::FAILURE;
            }
        };
        let zstd_len = match compressed_len("zstd", &["-19", "-c"], data) {
            Ok(len) => len,
            Err(err) => {
                eprintln!("zstd failed on {name}: {err}");
                return ExitCode::FAILURE;
            }
        };
        let xz_len = match compressed_len("xz", &["-9e", "-c"], data) {
            Ok(len) => len,
            Err(err) => {
                eprintln!("xz failed on {name}: {err}");
                return ExitCode::FAILURE;
            }
        };
        measurements.push(FileMeasurement {
            name: name.clone(),
            original_len: data.len(),
            mothergod_len,
            gzip_len,
            zstd_len,
            xz_len,
        });
    }

    let generated_at = match generated_at() {
        Ok(stamp) => stamp,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let report = format_report(
        "Canterbury",
        "corpus.canterbury.ac.nz `cantrbry.tar.gz`, pinned by URL + SHA-256 in `bench/corpus.toml`",
        &generated_at,
        &versions,
        &measurements,
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
