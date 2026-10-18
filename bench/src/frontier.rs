//! The frontier table of issue #1340: every competitor level and mothergod
//! as one row each, aggregated over a corpus, with mothergod's two
//! neighbours on the ratio/encode-speed plane named.
//!
//! Pure over [`crate::field::FieldPoint`]s: no process, no file, so it
//! builds and tests under the default features, like [`crate::finals`].
//! The slice after this wires it to a measurement run and a
//! `docs/benchmarks/` page.
//!
//! Speed on the plane is encode MB/s: it is the axis mothergod is furthest
//! from the field on (ROADMAP M5), and decode MB/s and peak RSS ride in the
//! same row as columns, not as a second plane.

use crate::baseline::bits_per_byte;
use crate::field::FieldPoint;
use crate::finals::mb_per_sec;
use crate::graph::escape_markdown_cell;
use std::fmt::Write as _;

/// One compressor at one level, aggregated over every file of a corpus.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// `"zstd -19"`, or whatever names mothergod's level.
    pub label: String,
    /// Total original bytes.
    pub original_len: usize,
    /// Total compressed bytes.
    pub compressed_len: usize,
    /// Total encode seconds, one process at a time.
    pub encode_secs: f64,
    /// Total decode seconds, one process at a time.
    pub decode_secs: f64,
    /// Highest resident set of any encode or decode process, KiB; `None`
    /// where the row's source does not measure it.
    pub peak_rss_kib: Option<u64>,
}

impl Row {
    /// The row for `label` over `files`, each `(original bytes, its point)`.
    ///
    /// Sizes and times add, so a rate is total bytes over total seconds and
    /// a big file weighs what it costs (the rule of `finals::format_report`'s
    /// aggregate row). Peak resident set is a maximum, never a sum: the
    /// processes ran one after another.
    #[must_use]
    pub fn aggregate(label: &str, files: &[(usize, FieldPoint)]) -> Self {
        Row {
            label: label.to_string(),
            original_len: files.iter().map(|(len, _)| len).sum(),
            compressed_len: files.iter().map(|(_, p)| p.compressed_len).sum(),
            encode_secs: files.iter().map(|(_, p)| p.encode_secs).sum(),
            decode_secs: files.iter().map(|(_, p)| p.decode_secs).sum(),
            peak_rss_kib: files
                .iter()
                .map(|(_, p)| p.encode_peak_rss_kib.max(p.decode_peak_rss_kib))
                .max(),
        }
    }

    /// Compressed bits per original byte.
    #[must_use]
    pub fn bpb(&self) -> f64 {
        bits_per_byte(self.compressed_len, self.original_len)
    }

    /// Encode rate, decimal MB/s.
    #[must_use]
    pub fn encode_mbps(&self) -> f64 {
        mb_per_sec(self.original_len, self.encode_secs)
    }

    /// Decode rate, decimal MB/s.
    #[must_use]
    pub fn decode_mbps(&self) -> f64 {
        mb_per_sec(self.original_len, self.decode_secs)
    }
}

/// The competitor levels beside mothergod on the ratio/encode-speed plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbours<'a> {
    /// The fastest-encoding level that compresses at least as well, or
    /// `None` where no competitor does: mothergod leads on ratio.
    pub as_good: Option<&'a Row>,
    /// The best-compressing level that encodes at least as fast, or `None`
    /// where no competitor does: mothergod is the fastest.
    pub as_fast: Option<&'a Row>,
}

/// `kib` KiB as MiB.
#[allow(
    clippy::cast_precision_loss,
    reason = "a resident set stays far below 2^53 KiB"
)]
fn mib(kib: u64) -> f64 {
    kib as f64 / 1024.0
}

/// The two competitor rows nearest `mothergod` in `field`.
///
/// Ties resolve to the row better on the other axis, so equal speed picks
/// the smaller output and equal ratio the faster encode; a remaining tie
/// keeps the first in `field`'s order. A row over no bytes has no ratio
/// and is never a neighbour.
#[must_use]
pub fn neighbours<'a>(mothergod: &Row, field: &'a [Row]) -> Neighbours<'a> {
    let (bpb, rate) = (mothergod.bpb(), mothergod.encode_mbps());
    let measured = || field.iter().filter(|row| row.original_len > 0);
    // `max_by` keeps the last of equal elements and `min_by` the first, so
    // `as_good` walks `field` backwards to keep the first as well.
    let as_good = measured()
        .rev()
        .filter(|row| row.bpb() <= bpb)
        .max_by(|a, b| {
            a.encode_mbps()
                .total_cmp(&b.encode_mbps())
                .then_with(|| b.bpb().total_cmp(&a.bpb()))
        });
    let as_fast = measured()
        .filter(|row| row.encode_mbps() >= rate)
        .min_by(|a, b| {
            a.bpb()
                .total_cmp(&b.bpb())
                .then_with(|| b.encode_mbps().total_cmp(&a.encode_mbps()))
        });
    Neighbours { as_good, as_fast }
}

/// `mothergod` and `field` as one markdown table, best-compressing first,
/// mothergod's row in bold, followed by one line per neighbour.
#[must_use]
pub fn format_table(mothergod: &Row, field: &[Row]) -> String {
    let mut rows: Vec<(&Row, bool)> = field.iter().map(|r| (r, false)).collect();
    rows.push((mothergod, true));
    // A stable sort: a tie keeps `field`'s order and puts mothergod last.
    rows.sort_by(|(a, _), (b, _)| a.bpb().total_cmp(&b.bpb()));

    let mut out = String::from(
        "| level | b/B | encode MB/s | decode MB/s | peak MB |\n|---|---|---|---|---|\n",
    );
    for (row, is_mothergod) in rows {
        let peak = row
            .peak_rss_kib
            .map_or_else(|| "-".to_string(), |kib| format!("{:.1}", mib(kib)));
        let cells = [
            escape_markdown_cell(&row.label),
            format!("{:.6}", row.bpb()),
            format!("{:.3}", row.encode_mbps()),
            format!("{:.3}", row.decode_mbps()),
            peak,
        ]
        .map(|cell| {
            if is_mothergod {
                format!("**{cell}**")
            } else {
                cell
            }
        });
        writeln!(out, "| {} |", cells.join(" | ")).expect("writing to a String never fails");
    }

    let Neighbours { as_good, as_fast } = neighbours(mothergod, field);
    for (what, row) in [
        ("fastest level at least as good", as_good),
        ("best level at least as fast", as_fast),
    ] {
        let named = row.map_or("none", |r| r.label.as_str());
        writeln!(out, "\n{what}: {named}").expect("writing to a String never fails");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Neighbours, Row, format_table, neighbours};
    use crate::field::FieldPoint;

    fn point(
        compressed_len: usize,
        encode_secs: f64,
        decode_secs: f64,
        rss: (u64, u64),
    ) -> FieldPoint {
        FieldPoint {
            compressed_len,
            encode_secs,
            decode_secs,
            encode_peak_rss_kib: rss.0,
            decode_peak_rss_kib: rss.1,
        }
    }

    /// `label` over one 1,000,000-byte file at `compressed` bytes and
    /// `encode_secs`: bits per byte is `compressed * 8 / 1e6`, MB/s is
    /// `1 / encode_secs`.
    fn row(label: &str, compressed: usize, encode_secs: f64) -> Row {
        Row::aggregate(
            label,
            &[(1_000_000, point(compressed, encode_secs, 0.5, (1024, 512)))],
        )
    }

    #[test]
    fn aggregate_weights_by_bytes_and_takes_the_peak_not_the_sum() {
        let files = [
            (1_000_000, point(400_000, 1.0, 0.5, (2048, 1024))),
            (3_000_000, point(900_000, 3.0, 1.5, (4096, 8192))),
        ];
        let r = Row::aggregate("zstd -3", &files);
        assert_eq!((r.original_len, r.compressed_len), (4_000_000, 1_300_000));
        // 1.3e6 * 8 / 4e6, not the mean of 3.2 and 2.4.
        assert!((r.bpb() - 2.6).abs() < 1e-12, "{}", r.bpb());
        assert!((r.encode_mbps() - 1.0).abs() < 1e-12);
        assert!((r.decode_mbps() - 2.0).abs() < 1e-12);
        assert_eq!(r.peak_rss_kib, Some(8192));
    }

    #[test]
    fn aggregate_of_no_files_is_an_empty_row() {
        let r = Row::aggregate("none", &[]);
        assert_eq!(
            (r.original_len, r.compressed_len, r.peak_rss_kib),
            (0, 0, None)
        );
    }

    #[test]
    fn neighbours_are_the_fastest_as_good_and_the_best_as_fast() {
        // mothergod: 2.0 b/B at 0.04 MB/s.
        let mg = row("mothergod", 250_000, 25.0);
        let field = [
            row("xz -9e", 240_000, 30.0),   // better, slower
            row("zstd -19", 250_000, 5.0),  // equal ratio, 0.2 MB/s
            row("zstd -22", 245_000, 20.0), // better, 0.05 MB/s: faster, the best of those
            row("gzip -9", 330_000, 0.5),   // worse, 2 MB/s
            row("xz -0", 300_000, 25.0),    // worse, equal speed
        ];
        let n = neighbours(&mg, &field);
        assert_eq!(n.as_good.map(|r| r.label.as_str()), Some("zstd -19"));
        assert_eq!(n.as_fast.map(|r| r.label.as_str()), Some("zstd -22"));
    }

    #[test]
    fn a_leader_on_an_axis_has_no_neighbour_on_it() {
        let slow_best = row("mothergod", 200_000, 25.0);
        let field = [row("xz -9e", 240_000, 10.0), row("gzip -9", 330_000, 0.5)];
        assert_eq!(
            neighbours(&slow_best, &field),
            Neighbours {
                as_good: None,
                as_fast: Some(&field[0])
            }
        );
        let fast_worst = row("mothergod", 400_000, 0.1);
        let n = neighbours(&fast_worst, &field);
        assert_eq!(
            (n.as_good.map(|r| r.label.as_str()), n.as_fast),
            (Some("gzip -9"), None)
        );
    }

    #[test]
    fn ties_resolve_to_the_row_better_on_the_other_axis() {
        let mg = row("mothergod", 250_000, 25.0);
        // Both as good and equally fast: the smaller output wins as_fast,
        // the faster encode wins as_good.
        let field = [
            row("a", 250_000, 25.0),
            row("b", 240_000, 25.0),
            row("c", 250_000, 20.0),
        ];
        let n = neighbours(&mg, &field);
        assert_eq!(n.as_good.map(|r| r.label.as_str()), Some("c"));
        assert_eq!(n.as_fast.map(|r| r.label.as_str()), Some("b"));
    }

    #[test]
    fn a_full_tie_keeps_the_first_row_on_both_neighbours() {
        let mg = row("mothergod", 250_000, 25.0);
        let field = [row("first", 250_000, 25.0), row("second", 250_000, 25.0)];
        let n = neighbours(&mg, &field);
        assert_eq!(n.as_good.map(|r| r.label.as_str()), Some("first"));
        assert_eq!(n.as_fast.map(|r| r.label.as_str()), Some("first"));
    }

    #[test]
    fn a_row_over_no_bytes_is_never_a_neighbour() {
        let mg = row("mothergod", 250_000, 25.0);
        let field = [Row::aggregate("empty", &[])];
        assert_eq!(
            neighbours(&mg, &field),
            Neighbours {
                as_good: None,
                as_fast: None
            }
        );
    }

    #[test]
    fn table_lists_best_first_bolds_mothergod_and_names_the_neighbours() {
        let mg = Row::aggregate(
            "mothergod",
            &[(1_000_000, point(250_000, 25.0, 1.0, (0, 0)))],
        );
        let mg = Row {
            peak_rss_kib: None,
            ..mg
        };
        let field = [row("gzip -9", 330_000, 0.5), row("xz -9e", 240_000, 10.0)];
        assert_eq!(
            format_table(&mg, &field),
            "\
| level | b/B | encode MB/s | decode MB/s | peak MB |
|---|---|---|---|---|
| xz -9e | 1.920000 | 0.100 | 2.000 | 1.0 |
| **mothergod** | **2.000000** | **0.040** | **1.000** | **-** |
| gzip -9 | 2.640000 | 2.000 | 2.000 | 1.0 |

fastest level at least as good: xz -9e

best level at least as fast: xz -9e
"
        );
    }
}
