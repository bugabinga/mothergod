//! Formats a held-out-final report (`research/corpus/POLICY.md`'s
//! Silesia/Canterbury tier): per-file bits/byte for `mothergod::compress`
//! plus the pinned reference compressors (gzip, zstd -19, xz -9e), and the
//! same numbers aggregated across the whole corpus.
//!
//! Deliberately free of the corpus-fetch/process-spawning I/O
//! (`crate::corpus`, `crate::reference`) so it builds and tests under the
//! default feature set, unlike the two modules it composes; the
//! `finals_report` binary (behind the `corpus-fetch` feature,
//! `research/JOURNAL.md` remaining S2-D1 scope: "a gzip/zstd/xz reference
//! column and real Silesia/Canterbury numbers") is the only caller that
//! brings all three together.

use crate::baseline::bits_per_byte;
use crate::graph::escape_markdown_cell;
use crate::regret;
use std::fmt::Write as _;

/// One held-out-final file's original size plus every compressor's
/// compressed size, all measured on the exact same bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct FileMeasurement {
    /// File name inside its corpus, e.g. `"alice29.txt"`.
    pub name: String,
    /// Original (uncompressed) byte length.
    pub original_len: usize,
    /// `mothergod::compress(data).len()`.
    pub mothergod_len: usize,
    /// Compressed length under `gzip -9`.
    pub gzip_len: usize,
    /// Compressed length under `zstd -19`.
    pub zstd_len: usize,
    /// Compressed length under `xz -9e`.
    pub xz_len: usize,
    /// Wall-clock seconds `mothergod::compress(data)` took, single-thread
    /// (the measurement thread itself; `measure_all` parallelizes across
    /// files, not within one). SPEED scorecard input (ROADMAP.md).
    pub encode_secs: f64,
    /// Wall-clock seconds `mothergod::decompress` took on the bytes
    /// `mothergod::compress` produced. Below-floor decode (ROADMAP.md:
    /// `>=1 MB/s`) is a finding this report surfaces, not a check it
    /// enforces.
    pub decode_secs: f64,
}

/// Reference-compressor version strings, named in the report so a reader
/// knows exactly which build produced its numbers
/// (`research/corpus/POLICY.md`: "Reference compressors: zstd -19 and
/// xz -9e at pinned versions" — this crate doesn't pin the installed
/// binary itself, only the flags, so the report names the version that
/// actually ran instead).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versions {
    /// `gzip --version`'s first line.
    pub gzip: String,
    /// `zstd --version`'s first line.
    pub zstd: String,
    /// `xz --version`'s first line.
    pub xz: String,
}

/// The report-header facts that aren't the corpus or the measurements
/// themselves, grouped so [`format_report`] takes one argument for "what
/// conditions produced this report" instead of three loose strings
/// (`clippy::too_many_arguments`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance<'a> {
    /// Reference-compressor version strings.
    pub versions: &'a Versions,
    /// `crate::baseline::fingerprint`'s digest of the `bench/baseline.json`
    /// this report was generated against; embedded so `baseline_gate` can
    /// tell when a later baseline change left this report behind (issue
    /// #327's [`stale_reason`]).
    pub baseline_fingerprint: &'a str,
    /// `crate::reference::machine_info()`'s description of the hardware
    /// the encode/decode MB/s columns were measured on (issue #432): a
    /// throughput number is only comparable to another number from the
    /// same line, never to one from the internet, so the report says which
    /// machine it is.
    pub machine: &'a str,
}

/// One compressor's bits/byte across all of `measurements`, aggregated as
/// total compressed bytes over total original bytes — not an average of
/// per-file ratios, which would over-weight small files relative to their
/// share of the corpus (CLAUDE.md rule 4: an honest whole-corpus number
/// alongside the named per-file ones, not a second, differently-weighted
/// metric wearing the same label).
fn aggregate_bpb(
    measurements: &[FileMeasurement],
    compressed_len: impl Fn(&FileMeasurement) -> usize,
) -> f64 {
    let total_original: usize = measurements.iter().map(|m| m.original_len).sum();
    let total_compressed: usize = measurements.iter().map(compressed_len).sum();
    bits_per_byte(total_compressed, total_original)
}

/// Decimal MB/s (`10^6` bytes, matching this crate's existing "5.3 MB"
/// file-size convention), `bytes` over `secs`. `0.0` for a non-positive
/// `secs` rather than an infinity a markdown table can't render sensibly;
/// real measurements never hit that path since every measured
/// compress/decompress call takes strictly positive wall-clock time.
#[allow(
    clippy::cast_precision_loss,
    reason = "byte counts here stay far below 2^53"
)]
fn mb_per_sec(bytes: usize, secs: f64) -> f64 {
    if secs <= 0.0 {
        0.0
    } else {
        bytes as f64 / 1_000_000.0 / secs
    }
}

/// Aggregate MB/s across `measurements`: total original bytes over total
/// wall-clock seconds, the same byte-weighted-not-averaged shape
/// [`aggregate_bpb`] uses and for the same reason (CLAUDE.md rule 4).
fn aggregate_mb_per_sec(
    measurements: &[FileMeasurement],
    secs: impl Fn(&FileMeasurement) -> f64,
) -> f64 {
    let total_original: usize = measurements.iter().map(|m| m.original_len).sum();
    let total_secs: f64 = measurements.iter().map(secs).sum();
    mb_per_sec(total_original, total_secs)
}

/// Renders `measurements` (any order; sorted internally by file name) as a
/// markdown report: an "as of"/provenance header naming the corpus and the
/// reference-compressor versions, a per-file table, and an aggregate row.
///
/// `corpus_name` and `corpus_provenance` are free text, not escaped for
/// markdown table syntax — they belong in prose, not a table cell, and
/// callers pass fixed strings, not attacker-controlled input. `generator_bin`
/// names the `bench` binary that produced this report (e.g.
/// `"finals_report"`), so the header's regeneration command matches
/// whichever caller actually ran — one `format_report` shared across every
/// held-out-final corpus, per corpus its own binary and command line.
/// `provenance` carries the report-conditions facts ([`Provenance`]).
#[must_use]
pub fn format_report(
    corpus_name: &str,
    corpus_provenance: &str,
    generated_at: &str,
    measurements: &[FileMeasurement],
    generator_bin: &str,
    provenance: &Provenance<'_>,
) -> String {
    let Provenance {
        versions,
        baseline_fingerprint,
        machine,
    } = provenance;
    let mut ordered: Vec<&FileMeasurement> = measurements.iter().collect();
    ordered.sort_by(|a, b| a.name.cmp(&b.name));

    let mut out = String::new();
    writeln!(
        out,
        "<!-- Generated by `cargo run -p mothergod-bench --release --features \
         corpus-fetch --bin {generator_bin}`. Do not hand-edit; re-run the \
         generator instead. -->"
    )
    .expect("writing to a String never fails");
    writeln!(
        out,
        "<!-- baseline-fingerprint: {baseline_fingerprint} -->\n"
    )
    .expect("writing to a String never fails");
    writeln!(out, "# {corpus_name} held-out-final snapshot\n")
        .expect("writing to a String never fails");
    writeln!(
        out,
        "As of {generated_at}. Corpus: {corpus_provenance} \
         (`research/corpus/POLICY.md`'s held-out finals, fetched and \
         pin-verified by `bench::corpus`, never committed). Reference \
         compressors, versions as actually run: `{}` (gzip -9), `{}` \
         (zstd -19), `{}` (xz -9e) — `research/corpus/POLICY.md` pins the \
         flags, not the installed binary, so this line is the version \
         record CLAUDE.md rule 4 asks for. `regret` is mothergod's \
         bits/byte minus the stronger (lower) of zstd/xz on the same \
         file, positive meaning mothergod does worse \
         (`research/corpus/POLICY.md`, \"Growing the corpus\").\n\n\
         The `mothergod encode MB/s`/`mothergod decode MB/s` columns are \
         indicative of this one machine only, not a cross-machine claim: \
         {machine}.\n",
        versions.gzip, versions.zstd, versions.xz
    )
    .expect("writing to a String never fails");

    writeln!(
        out,
        "| file | bytes | mothergod b/B | gzip -9 b/B | zstd -19 b/B | xz -9e b/B | regret | \
         mothergod encode MB/s | mothergod decode MB/s |"
    )
    .expect("writing to a String never fails");
    writeln!(out, "|---|---|---|---|---|---|---|---|---|")
        .expect("writing to a String never fails");
    for m in &ordered {
        let mg = bits_per_byte(m.mothergod_len, m.original_len);
        let gz = bits_per_byte(m.gzip_len, m.original_len);
        let zs = bits_per_byte(m.zstd_len, m.original_len);
        let xz = bits_per_byte(m.xz_len, m.original_len);
        let encode_mbps = mb_per_sec(m.original_len, m.encode_secs);
        let decode_mbps = mb_per_sec(m.original_len, m.decode_secs);
        writeln!(
            out,
            "| `{}` | {} | {mg:.6} | {gz:.6} | {zs:.6} | {xz:.6} | {:+.6} | {encode_mbps:.3} | \
             {decode_mbps:.3} |",
            escape_markdown_cell(&m.name),
            m.original_len,
            regret(mg, zs, xz),
        )
        .expect("writing to a String never fails");
    }

    let total_original: usize = ordered.iter().map(|m| m.original_len).sum();
    let aggregate_mothergod = aggregate_bpb(measurements, |m| m.mothergod_len);
    let aggregate_gzip = aggregate_bpb(measurements, |m| m.gzip_len);
    let aggregate_zstd = aggregate_bpb(measurements, |m| m.zstd_len);
    let aggregate_xz = aggregate_bpb(measurements, |m| m.xz_len);
    let aggregate_encode_mbps = aggregate_mb_per_sec(measurements, |m| m.encode_secs);
    let aggregate_decode_mbps = aggregate_mb_per_sec(measurements, |m| m.decode_secs);
    writeln!(
        out,
        "| **aggregate ({} files)** | {total_original} | **{aggregate_mothergod:.6}** | \
         **{aggregate_gzip:.6}** | **{aggregate_zstd:.6}** | **{aggregate_xz:.6}** | **{:+.6}** | \
         **{aggregate_encode_mbps:.3}** | **{aggregate_decode_mbps:.3}** |",
        ordered.len(),
        regret(aggregate_mothergod, aggregate_zstd, aggregate_xz),
    )
    .expect("writing to a String never fails");

    out
}

/// Extracts the `baseline-fingerprint` marker [`format_report`] embeds, or
/// `None` when `report_text` predates that marker or was hand-edited past
/// recognition — [`stale_reason`] treats either as stale rather than
/// panicking on an old committed report.
fn embedded_fingerprint(report_text: &str) -> Option<&str> {
    let after = report_text.split_once("<!-- baseline-fingerprint: ")?.1;
    after.split_once(" -->").map(|(digest, _)| digest)
}

/// `None` when `report_text`'s embedded fingerprint matches
/// `current_baseline_fingerprint`; otherwise a human-readable reason
/// naming what a reader should do (issue #327). Pure text comparison, no
/// I/O: `baseline_gate` supplies both strings from disk.
#[must_use]
pub fn stale_reason(report_text: &str, current_baseline_fingerprint: &str) -> Option<String> {
    match embedded_fingerprint(report_text) {
        Some(embedded) if embedded == current_baseline_fingerprint => None,
        Some(embedded) => Some(format!(
            "embeds baseline-fingerprint {embedded}, but bench/baseline.json now \
             fingerprints as {current_baseline_fingerprint}"
        )),
        None => Some(
            "has no baseline-fingerprint marker (predates issue #327 or was \
                       hand-edited)"
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{FileMeasurement, Provenance, Versions, format_report, stale_reason};

    fn sample_measurements() -> Vec<FileMeasurement> {
        vec![
            FileMeasurement {
                name: "zeta.txt".to_string(),
                original_len: 1000,
                mothergod_len: 400,
                gzip_len: 500,
                zstd_len: 450,
                xz_len: 420,
                encode_secs: 0.001,
                decode_secs: 0.0005,
            },
            FileMeasurement {
                name: "alpha.txt".to_string(),
                original_len: 2000,
                mothergod_len: 900,
                gzip_len: 1000,
                zstd_len: 950,
                xz_len: 920,
                encode_secs: 0.002,
                decode_secs: 0.001,
            },
        ]
    }

    fn sample_versions() -> &'static Versions {
        static VERSIONS: std::sync::OnceLock<Versions> = std::sync::OnceLock::new();
        VERSIONS.get_or_init(|| Versions {
            gzip: "gzip 1.12".to_string(),
            zstd: "*** Zstandard CLI (64-bit) v1.5.7 ***".to_string(),
            xz: "xz (XZ Utils) 5.4.5".to_string(),
        })
    }

    fn sample_provenance() -> Provenance<'static> {
        Provenance {
            versions: sample_versions(),
            baseline_fingerprint: "deadbeefcafef00d",
            machine: "test machine, 1 logical core(s)",
        }
    }

    #[test]
    fn format_report_sorts_files_by_name() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        assert!(
            report.find("alpha.txt").unwrap() < report.find("zeta.txt").unwrap(),
            "expected alpha.txt before zeta.txt: {report}"
        );
    }

    #[test]
    fn format_report_names_the_corpus_and_versions() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        assert!(report.contains("Canterbury"));
        assert!(report.contains("corpus.canterbury.ac.nz"));
        assert!(report.contains("gzip 1.12"));
        assert!(report.contains("v1.5.7"));
        assert!(report.contains("5.4.5"));
    }

    #[test]
    fn format_report_names_the_machine_the_throughput_columns_ran_on() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        assert!(
            report.contains("test machine, 1 logical core(s)"),
            "expected the machine description next to the MB/s columns: {report}"
        );
    }

    #[test]
    fn format_report_names_its_generator_binary() {
        let report = format_report(
            "Silesia",
            "sun.aei.polsl.pl",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "silesia_report",
            &sample_provenance(),
        );
        assert!(
            report.contains("--bin silesia_report`"),
            "expected the regeneration command to name silesia_report: {report}"
        );
    }

    #[test]
    fn format_report_aggregate_is_total_bytes_not_average_of_ratios() {
        // alpha: 900/2000 = 3.6 b/B; zeta: 400/1000 = 3.2 b/B. A naive
        // average of the two ratios would read 3.4; the correct aggregate
        // (1300 compressed / 3000 original * 8) is 3.466667.
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        assert!(
            report.contains("3.466667"),
            "expected the byte-weighted aggregate, got: {report}"
        );
        assert!(
            !report.contains("**3.400000**"),
            "aggregate must not be a naive average of per-file ratios: {report}"
        );
    }

    #[test]
    fn format_report_aggregate_row_names_the_file_count() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        assert!(report.contains("aggregate (2 files)"));
    }

    #[test]
    fn format_report_handles_no_measurements() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &[],
            "finals_report",
            &sample_provenance(),
        );
        assert!(report.contains("aggregate (0 files)"));
        // 0/0 bytes: `bits_per_byte` returns 0.0 for a zero original length.
        assert!(report.contains("**0.000000**"));
    }

    #[test]
    fn format_report_escapes_a_pipe_in_a_file_name() {
        let measurements = vec![FileMeasurement {
            name: "weird|name.txt".to_string(),
            original_len: 10,
            mothergod_len: 5,
            gzip_len: 6,
            zstd_len: 6,
            xz_len: 6,
            encode_secs: 0.001,
            decode_secs: 0.0005,
        }];
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &measurements,
            "finals_report",
            &sample_provenance(),
        );
        assert!(report.contains(r"weird\|name.txt"));
    }

    #[test]
    fn format_report_embeds_the_baseline_fingerprint() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        assert!(report.contains("<!-- baseline-fingerprint: deadbeefcafef00d -->"));
    }

    #[test]
    fn stale_reason_is_none_when_the_fingerprint_matches() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        assert_eq!(stale_reason(&report, "deadbeefcafef00d"), None);
    }

    #[test]
    fn stale_reason_names_the_mismatch_when_baseline_moved() {
        let report = format_report(
            "Canterbury",
            "corpus.canterbury.ac.nz",
            "2026-08-25T00:00:00Z",
            &sample_measurements(),
            "finals_report",
            &sample_provenance(),
        );
        let reason = stale_reason(&report, "0000000000000000")
            .expect("a moved baseline must be reported stale");
        assert!(reason.contains("deadbeefcafef00d"));
        assert!(reason.contains("0000000000000000"));
    }

    #[test]
    fn stale_reason_flags_a_report_with_no_marker_at_all() {
        let reason = stale_reason("# an old report with no marker\n", "deadbeefcafef00d")
            .expect("a markerless report must be reported stale");
        assert!(reason.contains("no baseline-fingerprint marker"));
    }
}
