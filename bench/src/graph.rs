//! Renders `bench/baseline.json` (`baseline::parse_baseline`) as a static
//! SVG bar chart and a markdown table: `research/JOURNAL.md` S2-D1's
//! remaining "progress-graph rendering" line, ROADMAP M2's "per-dataset
//! graphs rendered from `research/progress.jsonl` into `docs/benchmarks/`".
//!
//! `bench/baseline.json` is the only source of real, named-corpus bits/byte
//! this crate can measure today (`research/JOURNAL.md` S2-A35); it has 11
//! cases, past the dataviz convention's ~7-class chart-alone ceiling, so
//! [`render_table_markdown`] is the table half of "table + chart" and
//! [`render_svg`] is the chart half — see `docs/benchmarks/README.md` for
//! how the two are used together. No charting dependency: the whole chart
//! is ~11 static bars, comfortably hand-rolled as SVG text.
//!
//! The chart is a static asset committed to `docs/benchmarks/`, viewed as a
//! markdown image on GitHub — no JavaScript runs there, so there is no
//! hover layer to ship (unlike `site/status.html`'s live, scripted charts).
//! Colors are the dataviz skill's default light palette, embedded as an
//! explicit background rect so the chart reads the same under GitHub's
//! light and dark themes rather than assuming a page background.

use std::fmt::Write as _;

/// One bar: a `bench/baseline.json` case name and its measured bits/byte.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    /// Case name, e.g. `"entropy_ladder_h4"` (`baseline::Case::name`).
    pub name: String,
    /// Measured bits/byte for this case.
    pub bits_per_byte: f64,
}

/// Bars sorted ascending by [`Bar::bits_per_byte`] (best-compressing case
/// first). The entropy ladder's cases land in this order on their own,
/// since each targets a strictly higher order-0 entropy than the last, so
/// sorting by value does not scramble that sequence.
fn sorted(bars: &[Bar]) -> Vec<&Bar> {
    let mut out: Vec<&Bar> = bars.iter().collect();
    out.sort_by(|a, b| a.bits_per_byte.total_cmp(&b.bits_per_byte));
    out
}

/// Escapes `text` for use inside SVG element content or a double-quoted
/// attribute value (e.g. `aria-label="{escape_svg(text)}"`): also escapes
/// `"` and `'`, since an unescaped `"` in an attribute value closes the
/// attribute early and turns the remainder into raw markup.
fn escape_svg(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Escapes `text` for use inside a markdown table cell: a pipe would end
/// the cell early. Case names are `[a-z0-9_]+` in practice, so this only
/// guards against a future name that isn't. `pub(crate)`: [`crate::finals`]
/// reuses it for held-out-final file names rather than growing a second
/// copy of the same escape.
pub(crate) fn escape_markdown_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// Chart surface, ink, gridline, and mark colors: the dataviz skill's
/// validated default light palette (`references/palette.md`), sequential
/// slot 450 for the single-series bars.
mod palette {
    pub const SURFACE: &str = "#fcfcfb";
    pub const PRIMARY_INK: &str = "#0b0b0b";
    pub const SECONDARY_INK: &str = "#52514e";
    pub const MUTED_INK: &str = "#898781";
    pub const GRIDLINE: &str = "#e1e0d9";
    pub const BAR: &str = "#2a78d6";
}

const BAR_HEIGHT: f64 = 20.0;
const BAR_GAP: f64 = 6.0;
const BAR_RADIUS: f64 = 4.0;
const LABEL_COLUMN_WIDTH: f64 = 170.0;
const CHART_WIDTH: f64 = 560.0;
const VALUE_COLUMN_WIDTH: f64 = 70.0;
const TOP_MARGIN: f64 = 64.0;
const BOTTOM_MARGIN: f64 = 36.0;
const SIDE_MARGIN: f64 = 16.0;

/// Smallest whole number of bits/byte at or above `value`, as an integer
/// tick count. Avoids a float-to-int cast (`clippy::cast_possible_truncation`)
/// for what is, in every real input, a single-digit count.
fn ceil_ticks(value: f64) -> u32 {
    let mut ticks = 0u32;
    while f64::from(ticks) < value {
        ticks += 1;
    }
    ticks
}

/// Renders `bars` (any order; sorted internally by value ascending) as a
/// static SVG horizontal bar chart: one bar per case, `title` and
/// `subtitle` (typically the "as of" stamp and source) above the plot.
///
/// # Panics
///
/// Panics if `bars` has more entries than fit in a `u32` row index — never
/// true for any real `bench/baseline.json`, which has a fixed, tiny case
/// count. `bars` may be empty, in which case the chart renders as an empty
/// plot area with only the title and subtitle.
#[must_use]
pub fn render_svg(title: &str, subtitle: &str, bars: &[Bar]) -> String {
    let ordered = sorted(bars);
    let row_count = u32::try_from(ordered.len()).expect("bar count fits in u32");
    let plot_width = CHART_WIDTH - LABEL_COLUMN_WIDTH - VALUE_COLUMN_WIDTH - SIDE_MARGIN;
    let plot_left = LABEL_COLUMN_WIDTH;
    let data_max = ordered
        .iter()
        .map(|bar| bar.bits_per_byte)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    // The axis ceiling is rounded up to a whole number so every gridline,
    // including the last, is a "clean" tick; bar widths scale against this
    // same ceiling, not the raw data max, so the last gridline lands
    // exactly at the plot's right edge instead of overshooting past it.
    let axis_max = f64::from(ceil_ticks(data_max));
    let row_height = BAR_HEIGHT + BAR_GAP;
    let plot_height = row_height * f64::from(row_count);
    let height = TOP_MARGIN + plot_height + BOTTOM_MARGIN;
    let plot_bottom = TOP_MARGIN + plot_height;

    let mut svg = String::new();
    let alt_text = escape_svg(&format!("{title}. {subtitle}"));
    let title_esc = escape_svg(title);
    let subtitle_esc = escape_svg(subtitle);
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {CHART_WIDTH} {height}" role="img" aria-label="{alt_text}">"#
    )
    .expect("writing to a String never fails");
    writeln!(svg, "<title>{title_esc}</title>").expect("writing to a String never fails");
    writeln!(
        svg,
        r#"<rect x="0" y="0" width="{CHART_WIDTH}" height="{height}" fill="{}"/>"#,
        palette::SURFACE
    )
    .expect("writing to a String never fails");
    writeln!(
        svg,
        r#"<text x="{SIDE_MARGIN}" y="24" font-family="system-ui, sans-serif" font-size="15" font-weight="600" fill="{}">{title_esc}</text>"#,
        palette::PRIMARY_INK
    )
    .expect("writing to a String never fails");
    writeln!(
        svg,
        r#"<text x="{SIDE_MARGIN}" y="42" font-family="system-ui, sans-serif" font-size="11" fill="{}">{subtitle_esc}</text>"#,
        palette::SECONDARY_INK
    )
    .expect("writing to a String never fails");

    // Gridlines at clean bits/byte ticks (0 and every whole number up to
    // the ceiling of the largest bar), one-step-off-surface gray hairlines
    // behind the bars, per the mark spec.
    for tick in 0..=ceil_ticks(axis_max) {
        let x = plot_left + (f64::from(tick) / axis_max) * plot_width;
        writeln!(
            svg,
            r#"<line x1="{x:.1}" y1="{TOP_MARGIN:.1}" x2="{x:.1}" y2="{plot_bottom:.1}" stroke="{}" stroke-width="1"/>"#,
            palette::GRIDLINE
        )
        .expect("writing to a String never fails");
    }

    for (row, bar) in ordered.iter().enumerate() {
        let row_index = u32::try_from(row).expect("bar count fits in u32");
        let y = TOP_MARGIN + row_height * f64::from(row_index);
        let y_bottom = y + BAR_HEIGHT;
        let bar_width = (bar.bits_per_byte / axis_max) * plot_width;
        let radius = BAR_RADIUS.min(bar_width / 2.0).max(0.0);
        let x0 = plot_left;
        let x1 = plot_left + bar_width;
        let lead = x1 - radius;
        let path = if radius > 0.0 {
            format!(
                "M{x0:.1},{y:.1} L{lead:.1},{y:.1} A{radius:.1},{radius:.1} 0 0 1 {x1:.1},{top:.1} \
L{x1:.1},{bottom:.1} A{radius:.1},{radius:.1} 0 0 1 {lead:.1},{y_bottom:.1} L{x0:.1},{y_bottom:.1} Z",
                top = y + radius,
                bottom = y_bottom - radius,
            )
        } else {
            format!(
                "M{x0:.1},{y:.1} L{x1:.1},{y:.1} L{x1:.1},{y_bottom:.1} L{x0:.1},{y_bottom:.1} Z"
            )
        };
        writeln!(svg, r#"<path d="{path}" fill="{}"/>"#, palette::BAR)
            .expect("writing to a String never fails");

        let label_y = y + BAR_HEIGHT / 2.0 + 4.0;
        let name_esc = escape_svg(&bar.name);
        writeln!(
            svg,
            r#"<text x="{:.1}" y="{label_y:.1}" text-anchor="end" font-family="ui-monospace, monospace" font-size="11" fill="{}">{name_esc}</text>"#,
            plot_left - 8.0,
            palette::MUTED_INK,
        )
        .expect("writing to a String never fails");
        writeln!(
            svg,
            r#"<text x="{:.1}" y="{label_y:.1}" font-family="ui-monospace, monospace" font-size="11" fill="{}">{:.3}</text>"#,
            x1 + 8.0,
            palette::SECONDARY_INK,
            bar.bits_per_byte,
        )
        .expect("writing to a String never fails");
    }

    svg.push_str("</svg>\n");
    svg
}

/// Renders `bars` (any order; sorted internally by value ascending) as a
/// markdown table: case name and bits/byte, 6 decimal places matching
/// `baseline::format_baseline`'s own precision.
#[must_use]
pub fn render_table_markdown(bars: &[Bar]) -> String {
    let ordered = sorted(bars);
    let mut out = String::from("| case | bits/byte |\n|---|---|\n");
    for bar in ordered {
        writeln!(
            out,
            "| `{}` | {:.6} |",
            escape_markdown_cell(&bar.name),
            bar.bits_per_byte
        )
        .expect("writing to a String never fails");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Bar, render_svg, render_table_markdown};

    fn sample_bars() -> Vec<Bar> {
        vec![
            Bar {
                name: "entropy_ladder_h1".to_string(),
                bits_per_byte: 1.298_08,
            },
            Bar {
                name: "markov_h8_2_trap".to_string(),
                bits_per_byte: 2.447_04,
            },
            Bar {
                name: "entropy_ladder_h8".to_string(),
                bits_per_byte: 8.000_96,
            },
        ]
    }

    #[test]
    fn svg_is_well_formed_and_contains_one_path_per_bar() {
        let svg = render_svg("title", "subtitle", &sample_bars());
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert_eq!(svg.matches("<path ").count(), 3);
    }

    #[test]
    fn svg_orders_bars_ascending_by_value() {
        let svg = render_svg("t", "s", &sample_bars());
        let h1_pos = svg.find("entropy_ladder_h1").unwrap();
        let trap_pos = svg.find("markov_h8_2_trap").unwrap();
        let h8_pos = svg.find("entropy_ladder_h8").unwrap();
        assert!(
            h1_pos < trap_pos,
            "h1 (1.3) should render before the trap (2.4)"
        );
        assert!(
            trap_pos < h8_pos,
            "the trap (2.4) should render before h8 (8.0)"
        );
    }

    #[test]
    fn svg_escapes_title_and_subtitle() {
        let svg = render_svg("A & B", "<script>", &[]);
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&amp;"));
        assert!(svg.contains("&lt;script&gt;"));
    }

    #[test]
    fn svg_escapes_quotes_in_the_attribute_context() {
        // aria-label="{alt_text}" is a double-quoted attribute; an
        // unescaped `"` in alt_text would close the attribute early and
        // turn the rest of its value into raw markup.
        let svg = render_svg("A \"quote\" and 'apostrophe'", "s", &[]);
        assert!(!svg.contains(r#"aria-label="A "quote""#));
        assert!(svg.contains("&quot;quote&quot;"));
        assert!(svg.contains("&apos;apostrophe&apos;"));
    }

    #[test]
    fn svg_renders_with_no_bars() {
        let svg = render_svg("empty", "no cases", &[]);
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert_eq!(svg.matches("<path ").count(), 0);
    }

    #[test]
    fn svg_clamps_radius_for_a_bar_narrower_than_the_corner_radius() {
        // A tiny value relative to a much larger one produces a bar a few
        // pixels wide; the radius must not exceed half that width, or the
        // rounded path's control points would cross and self-intersect.
        let bars = vec![
            Bar {
                name: "tiny".to_string(),
                bits_per_byte: 0.001,
            },
            Bar {
                name: "huge".to_string(),
                bits_per_byte: 8.0,
            },
        ];
        // Must not panic building the path, and must still emit two bars.
        let svg = render_svg("t", "s", &bars);
        assert_eq!(svg.matches("<path ").count(), 2);
    }

    #[test]
    fn max_bar_reaches_exactly_the_last_gridline_not_past_it() {
        // Regression: bar width and gridline position must scale against
        // the same axis ceiling. Scaling bars by the raw data max while
        // scaling gridlines by the rounded ceiling put the top gridline
        // past the plot's right edge, into the value-label column.
        let bars = vec![Bar {
            name: "exact".to_string(),
            bits_per_byte: 4.0,
        }];
        let svg = render_svg("t", "s", &bars);
        let plot_right = super::LABEL_COLUMN_WIDTH
            + (super::CHART_WIDTH
                - super::LABEL_COLUMN_WIDTH
                - super::VALUE_COLUMN_WIDTH
                - super::SIDE_MARGIN);
        // The bar's tip (path's rounded-corner arc target) and the last
        // gridline both land on the plot's right edge for a bar whose
        // value exactly matches the (whole-number) axis ceiling.
        let last_gridline = svg
            .lines()
            .rfind(|line| line.starts_with("<line"))
            .expect("at least one gridline");
        assert!(
            last_gridline.contains(&format!(r#"x1="{plot_right:.1}""#)),
            "last gridline should sit exactly at the plot's right edge {plot_right:.1}, got: {last_gridline}"
        );
        assert!(
            svg.contains(&format!("0 0 1 {plot_right:.1},")),
            "the bar's own tip should also land on {plot_right:.1}, not past it"
        );
    }

    #[test]
    fn table_has_a_header_and_one_row_per_bar_sorted_ascending() {
        let table = render_table_markdown(&sample_bars());
        let mut lines = table.lines();
        assert_eq!(lines.next(), Some("| case | bits/byte |"));
        assert_eq!(lines.next(), Some("|---|---|"));
        assert_eq!(lines.next(), Some("| `entropy_ladder_h1` | 1.298080 |"));
        assert_eq!(lines.next(), Some("| `markov_h8_2_trap` | 2.447040 |"));
        assert_eq!(lines.next(), Some("| `entropy_ladder_h8` | 8.000960 |"));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn table_escapes_a_pipe_in_a_case_name() {
        let bars = vec![Bar {
            name: "weird|name".to_string(),
            bits_per_byte: 1.0,
        }];
        let table = render_table_markdown(&bars);
        assert!(table.contains(r"weird\|name"));
    }
}
