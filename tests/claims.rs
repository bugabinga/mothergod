//! Guard: `README.md` and `site/index.html` restate `FORMAT_VERSION`, the
//! published aggregate bits/byte numbers, and the published aggregate
//! encode/decode MB/s instead of computing them, so a codec change or a
//! report regeneration can leave any of them stale with nothing catching it
//! (issue #431: twice in seven days, PR #243 and again the day this test
//! was added). Compares every restated claim against its single source of
//! truth — `FORMAT_VERSION` against `src/lib.rs`'s own constant, the
//! aggregate figures against the matching generated `docs/benchmarks/*.md`
//! report — and fails naming the file, the claimed value, and the true
//! value.

use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

/// The first run of ASCII digits after `marker`, skipping intervening
/// whitespace: a line wrap can sit between a closing tag and its number.
fn digits_after(text: &str, marker: &str) -> u8 {
    let after = text.find(marker).map_or_else(
        || panic!("{marker:?} not found"),
        |index| &text[index + marker.len()..],
    );
    let digits: String = after
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("no number follows {marker:?}"))
}

/// Numbers found between a `>` and the following `<`, in encounter order:
/// picks HTML tag *content* (`<td class="ours">1.374</td>` -> `1.374`)
/// while skipping tag attributes and non-numeric content (`Canterbury`).
fn html_numbers(fragment: &str) -> Vec<f64> {
    let mut values = Vec::new();
    let mut rest = fragment;
    while let Some(tag_end) = rest.find('>') {
        rest = &rest[tag_end + 1..];
        let text_end = rest.find('<').unwrap_or(rest.len());
        if let Ok(value) = rest[..text_end].trim().parse::<f64>() {
            values.push(value);
        }
        rest = &rest[text_end..];
    }
    values
}

/// Numbers in a markdown table row's cells, `**bold**` or plain, in column
/// order; a non-numeric cell (a label, an empty edge cell) is skipped.
fn markdown_numbers(row: &str) -> Vec<f64> {
    row.split('|')
        .filter_map(|cell| cell.trim().trim_matches('*').trim().parse::<f64>().ok())
        .collect()
}

fn line_containing<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle:?}"))
}

#[test]
fn format_version_is_current_everywhere_it_is_restated() {
    let true_version = mothergod::FORMAT_VERSION;

    let readme_claim = digits_after(&read("README.md"), "`FORMAT_VERSION`");
    assert_eq!(
        readme_claim, true_version,
        "README.md claims FORMAT_VERSION {readme_claim}, src/lib.rs's FORMAT_VERSION is {true_version}"
    );

    let site_claim = digits_after(&read("site/index.html"), "<code>FORMAT_VERSION</code>");
    assert_eq!(
        site_claim, true_version,
        "site/index.html claims FORMAT_VERSION {site_claim}, src/lib.rs's FORMAT_VERSION is {true_version}"
    );
}

/// The aggregate row's mothergod/gzip -9/zstd -19/xz -9e bits/byte, read
/// from the generated report that is the single source of truth for them.
fn aggregate_from_report(report_file: &str) -> [f64; 4] {
    let text = read(&format!("docs/benchmarks/{report_file}"));
    let row = line_containing(&text, "**aggregate");
    let numbers = markdown_numbers(row);
    // Column order: bytes, mothergod, gzip -9, zstd -19, xz -9e, regret,
    // encode MB/s, decode MB/s (docs/benchmarks/{canterbury,silesia}.md's
    // own header row) -- skip the byte count, keep the four ratios.
    [numbers[1], numbers[2], numbers[3], numbers[4]]
}

fn aggregate_from_readme(corpus: &str) -> [f64; 4] {
    let readme = read("README.md");
    let row = line_containing(&readme, &format!("| {corpus} |"));
    let numbers = markdown_numbers(row);
    [numbers[0], numbers[1], numbers[2], numbers[3]]
}

fn aggregate_from_site(corpus: &str) -> [f64; 4] {
    let site = read("site/index.html");
    let marker = format!("<th scope=\"row\">{corpus}</th>");
    let start = site
        .find(&marker)
        .unwrap_or_else(|| panic!("{marker:?} not found in site/index.html"));
    let fragment = &site[start..];
    let end = fragment.find("</tr>").unwrap_or(fragment.len());
    let numbers = html_numbers(&fragment[..end]);
    [numbers[0], numbers[1], numbers[2], numbers[3]]
}

#[test]
fn aggregate_ratios_match_their_generated_reports() {
    let corpora = [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")];
    let columns = ["mothergod", "gzip -9", "zstd -19", "xz -9e"];

    for (corpus, report_file) in corpora {
        let truth = aggregate_from_report(report_file);
        let readme = aggregate_from_readme(corpus);
        let site = aggregate_from_site(corpus);

        for (index, column) in columns.iter().enumerate() {
            // The published surfaces round to 3 decimals (issue #431: e.g.
            // "1.374", not "1.373741"), so compare rounded strings rather
            // than exact floats or an unrounded prefix.
            let rounded = format!("{:.3}", truth[index]);
            let readme_claim = format!("{:.3}", readme[index]);
            let site_claim = format!("{:.3}", site[index]);

            assert_eq!(
                readme_claim, rounded,
                "README.md's {corpus} {column} bits/byte is {readme_claim}, docs/benchmarks/{report_file} says {rounded}"
            );
            assert_eq!(
                site_claim, rounded,
                "site/index.html's {corpus} {column} bits/byte is {site_claim}, docs/benchmarks/{report_file} says {rounded}"
            );
        }
    }
}

/// The aggregate row's encode and decode MB/s, from the generated report.
fn throughput_from_report(report_file: &str) -> [f64; 2] {
    let text = read(&format!("docs/benchmarks/{report_file}"));
    let row = line_containing(&text, "**aggregate");
    let numbers = markdown_numbers(row);
    // Same column order as `aggregate_from_report`: bytes, mothergod,
    // gzip -9, zstd -19, xz -9e, regret, encode MB/s, decode MB/s.
    [numbers[6], numbers[7]]
}

/// The first two numbers immediately followed by `MB/s`, scanning from
/// `marker` onward. Both surfaces name a corpus once and then give its two
/// rates in encode-then-decode order ("encoded Canterbury at 0.133 MB/s and
/// decoded it at 4.422 MB/s"), so the corpus name is the only anchor this
/// needs and the prose stays free to reword around it.
fn throughput_after(text: &str, marker: &str) -> [f64; 2] {
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"));
    let mut rates = text[start..].split("MB/s").take(2).map(|before| {
        let reversed: String = before
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let number: String = reversed.chars().rev().collect();
        number
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("no MB/s rate after {marker:?}, found {before:?}"))
    });
    let encode = rates.next().expect("encode rate after the corpus name");
    let decode = rates.next().expect("decode rate after the corpus name");
    [encode, decode]
}

#[test]
fn published_throughput_matches_its_generated_reports() {
    let corpora = [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")];

    for (corpus, report_file) in corpora {
        let truth = throughput_from_report(report_file);
        let marker = format!("encoded {corpus} at");
        let readme = throughput_after(&read("README.md"), &marker);
        let site = throughput_after(&read("site/index.html"), &marker);

        for (index, direction) in ["encode", "decode"].iter().enumerate() {
            // The reports already print three decimals, so the surfaces
            // quote them verbatim; compare as strings for the same reason
            // the ratio test does.
            let rounded = format!("{:.3}", truth[index]);
            let readme_claim = format!("{:.3}", readme[index]);
            let site_claim = format!("{:.3}", site[index]);

            assert_eq!(
                readme_claim, rounded,
                "README.md's {corpus} {direction} MB/s is {readme_claim}, docs/benchmarks/{report_file} says {rounded}"
            );
            assert_eq!(
                site_claim, rounded,
                "site/index.html's {corpus} {direction} MB/s is {site_claim}, docs/benchmarks/{report_file} says {rounded}"
            );
        }
    }
}
