//! Guard: `README.md`, `site/index.html` and `CHANGELOG.md` restate
//! `FORMAT_VERSION`, the
//! decode promise `docs/format/SPEC.md` makes about it, the published
//! aggregate bits/byte numbers, the published aggregate
//! encode/decode MB/s and `xz -9e`'s beside them, the benchmark CPU model, the per-file win count and the files named behind it, the
//! measurement date, the
//! reference compressor
//! versions, the pre-alpha/no-release state of the project, and the CLI
//! recipe a reader is told to type and the line its first run prints, instead of
//! deriving any of them, so a codec change, a report regeneration, a
//! release or a
//! renamed subcommand can leave any of them stale with nothing catching it
//! (issue #431: twice in seven days, PR #243 and again the day this test
//! was added; issue #469: the report regenerated under an unchanged ratio
//! left only the restated date stale, and nothing compared it; the CPU
//! model had the same gap until this guard: the CI runner rotated between
//! 2026-09-27 and 2026-10-03 and `site/index.html` kept quoting the old
//! one; the per-file win count had the same gap too, unguarded since the
//! sentence was written). Compares
//! every restated claim against its single source of truth:
//! `FORMAT_VERSION` against `src/lib.rs`'s own constant, the aggregate
//! figures, date and tool versions against the matching generated
//! `docs/benchmarks/*.md` report, the release state against
//! `CHANGELOG.md`'s own headings, the recipe against the binary's own
//! usage output, and fails naming the file, the claimed value, and the
//! true value.
//!
//! A figure copied verbatim from a generated report is a located figure: its
//! locator returns the byte span it occupies in the surface, and
//! `check_figures` compares what sits there with the report's value. With
//! `MOTHERGOD_BLESS_CLAIMS=1` it rewrites the span to the report's value
//! instead of failing (issue #1486), so a benchmark publish moves the digits
//! in one command and this file stays the one parser of those sentences. The
//! words around a figure (a verdict, a win count, "more than nine times") are
//! derived claims and fail as before: a floor that breaks is a sentence to
//! rewrite, not a number to stamp.

// Not under Miri: prose-vs-source string comparison, no codec code runs
// here for Miri to observe, so the lane spends its budget elsewhere
// (issue #456).
// Not on Android: every claim here is read from README.md, site/index.html,
// docs/benchmarks/*.md or the built `mothergod` binary, none of which
// `.github/scripts/android-runner` pushes to the device (it pushes only the
// test executable plus tests/adversarial/ and tests/golden/), so every read
// and the binary spawn are a guaranteed failure unrelated to whether the
// claims are true (issue #518).
#![cfg(not(any(miri, target_os = "android")))]

use std::ops::Range;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

/// Serializes `check_figures`: tests run on threads and two of them rewrite
/// `README.md` or `site/index.html`, so each read, locate, compare, splice
/// and write happens whole under this lock, on a fresh read of the file. A
/// failing check panics holding it, so a poisoned lock is still a lock.
static SURFACES: Mutex<()> = Mutex::new(());

/// Whether a stale figure is rewritten to its source's value instead of
/// failing the test.
fn blessing() -> bool {
    std::env::var_os("MOTHERGOD_BLESS_CLAIMS").is_some_and(|value| value == "1")
}

/// Compares each figure `locate` finds in `file` with `truth`, in order.
/// A stale figure fails naming the file, `name(index)`, the claimed text and
/// the `source` that says otherwise; under `MOTHERGOD_BLESS_CLAIMS=1` it is
/// rewritten in place instead. Truth is the exact text the surface carries,
/// so a figure the reader sees as "1.37" where the report says "1.370" is
/// stale too.
fn check_figures(
    file: &str,
    source: &str,
    locate: impl Fn(&str) -> Vec<Range<usize>>,
    truth: &[String],
    name: impl Fn(usize) -> String,
) {
    check_figures_within(file, source, |text| 0..text.len(), locate, truth, name);
}

/// `check_figures` that rewrites only figures inside `scope(text)`: a figure
/// outside it, such as one in a released `CHANGELOG.md` section, is history
/// and fails even under bless, because blessing it would rewrite the record.
fn check_figures_within(
    file: &str,
    source: &str,
    scope: impl Fn(&str) -> Range<usize>,
    locate: impl Fn(&str) -> Vec<Range<usize>>,
    truth: &[String],
    name: impl Fn(usize) -> String,
) {
    let _held = SURFACES.lock().unwrap_or_else(PoisonError::into_inner);
    let mut text = read(file);
    let spans = locate(&text);
    assert_eq!(
        spans.len(),
        truth.len(),
        "{file}: found {} figures where {source} has {}",
        spans.len(),
        truth.len()
    );

    let scope = scope(&text);
    let mut stale = Vec::new();
    let mut failures = Vec::new();
    for (index, (span, truth)) in spans.iter().zip(truth).enumerate() {
        let claim = &text[span.clone()];
        if claim == truth {
            continue;
        }
        if blessing() && scope.start <= span.start && span.end <= scope.end {
            stale.push((span.clone(), truth));
        } else {
            failures.push(format!(
                "{file}'s {} is {claim}, {source} says {truth}",
                name(index)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    if stale.is_empty() {
        return;
    }

    // Back to front, so a longer or shorter replacement cannot move a span
    // still to be spliced.
    stale.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    for (span, truth) in stale {
        text.replace_range(span, truth);
    }
    // Rename, not truncate-and-write: a test reading the file without the
    // lock must see the old file or the new one, never half of one.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    let staging = path.with_extension("blessing");
    std::fs::write(&staging, text)
        .and_then(|()| std::fs::rename(&staging, &path))
        .unwrap_or_else(|error| panic!("cannot bless {file}: {error}"));
}

/// The one value every `(source, value)` pair agrees on. A figure two
/// reports both carry (the CPU model, the date, a tool version) has no single
/// truth to stamp when they differ, so that fails even under bless.
fn agreed(what: &str, from: &[(String, String)]) -> String {
    let (first_source, first) = &from[0];
    for (source, value) in &from[1..] {
        assert_eq!(
            value, first,
            "{what} is {first} in {first_source} and {value} in {source}"
        );
    }
    first.clone()
}

/// The first run of ASCII digits after `marker`, skipping intervening
/// whitespace: a line wrap can sit between a closing tag and its number.
fn digits_after(text: &str, marker: &str) -> u8 {
    number_and_tail_after(text, marker).0
}

/// `digits_after`'s number, plus the prose that follows it with leading
/// whitespace trimmed. A claim of the shape "version 2 or later" states its
/// floor in the number and whether it is open-ended in the tail, and only
/// the pair says whether the claim still covers the current version.
fn number_and_tail_after<'a>(text: &'a str, marker: &str) -> (u8, &'a str) {
    let after = text.find(marker).map_or_else(
        || panic!("{marker:?} not found"),
        |index| text[index + marker.len()..].trim_start(),
    );
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    let number = digits
        .parse()
        .unwrap_or_else(|_| panic!("no number follows {marker:?}"));
    (number, after[digits.len()..].trim_start())
}

/// Byte ranges of the numbers found between a `>` and the following `<`
/// within `region` of `text`, in encounter order: picks HTML tag *content*
/// (`<td class="ours">1.374</td>` -> `1.374`) while skipping tag attributes
/// and non-numeric content (`Canterbury`).
fn html_number_spans(text: &str, region: Range<usize>) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut at = region.start;
    while let Some(tag_end) = text[at..region.end].find('>') {
        let start = at + tag_end + 1;
        let len = text[start..region.end]
            .find('<')
            .unwrap_or(region.end - start);
        let content = &text[start..start + len];
        let number = content.trim();
        if number.parse::<f64>().is_ok() {
            let from = start + content.len() - content.trim_start().len();
            spans.push(from..from + number.len());
        }
        at = start + len;
    }
    spans
}

/// Byte ranges of the numbers in the cells of the markdown table row at
/// `line` of `text`, `**bold**` or plain, in column order; a non-numeric cell
/// (a label, an empty edge cell) is skipped.
fn markdown_number_spans(text: &str, line: Range<usize>) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut cell_start = line.start;
    for cell in text[line].split('|') {
        let number = cell.trim().trim_matches('*').trim();
        if number.parse::<f64>().is_ok() {
            // The cell holds only whitespace and asterisks before its number.
            let from = cell_start + cell.find(number).expect("the number is in its cell");
            spans.push(from..from + number.len());
        }
        cell_start += cell.len() + 1;
    }
    spans
}

/// Numbers in a markdown table row's cells, read through the same locator
/// the surfaces' figures use.
fn markdown_numbers(row: &str) -> Vec<f64> {
    markdown_number_spans(row, 0..row.len())
        .into_iter()
        .map(|span| row[span].parse().expect("a located number parses"))
        .collect()
}

/// Byte ranges of the decimal numbers anywhere in `region` of free-flowing
/// prose, in encounter order: `"(1.374 bits/byte vs 1.470 and 1.403)"` ->
/// the three. Unlike `markdown_number_spans`, there is no cell delimiter to
/// split on, so this scans character by character instead.
fn prose_number_spans(text: &str, region: Range<usize>) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let mut index = region.start;
    while index < region.end {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < region.end && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
                index += 1;
            }
            if text[start..index].parse::<f64>().is_ok() {
                spans.push(start..index);
            }
        } else {
            index += 1;
        }
    }
    spans
}

/// The first line of `text` containing `needle`, newline excluded.
fn line_range(text: &str, needle: &str) -> Range<usize> {
    let at = text
        .find(needle)
        .unwrap_or_else(|| panic!("no line contains {needle:?}"));
    let start = text[..at].rfind('\n').map_or(0, |newline| newline + 1);
    let end = text[at..]
        .find('\n')
        .map_or(text.len(), |newline| at + newline);
    start..end
}

fn line_containing<'a>(text: &'a str, needle: &str) -> &'a str {
    &text[line_range(text, needle)]
}

#[test]
fn format_version_is_current_everywhere_it_is_restated() {
    let true_version = mothergod::FORMAT_VERSION;

    let readme_claim = digits_after(&read("README.md"), "`FORMAT_VERSION`");
    assert_eq!(
        readme_claim, true_version,
        "README.md claims FORMAT_VERSION {readme_claim}, src/lib.rs's FORMAT_VERSION is {true_version}"
    );

    let site_claim = digits_after(&read("site/index.html"), "container format (version");
    assert_eq!(
        site_claim, true_version,
        "site/index.html claims container format version {site_claim}, src/lib.rs's FORMAT_VERSION is {true_version}"
    );

    // The release notes state the version twice: the format a release
    // writes, and the floor below which nothing decodes. The decoder reads
    // exactly one version, so both are the current one.
    let changelog = read("CHANGELOG.md");
    for marker in [
        "Bitstream format at `FORMAT_VERSION`",
        "Format versions below",
    ] {
        let changelog_claim = digits_after(&changelog, marker);
        assert_eq!(
            changelog_claim, true_version,
            "CHANGELOG.md's {marker:?} claims {changelog_claim}, src/lib.rs's FORMAT_VERSION is {true_version}"
        );
    }
}

#[test]
fn format_promise_claims_agree_with_the_changelog() {
    // docs/format/SPEC.md's status paragraph owns the format promise
    // (ADR-0050): until 1.0 a version can be retired, and from the first
    // version a 1.0 build writes none ever is. README.md and
    // site/index.html restate it in plain words, and the restatement has
    // one stale mode, the pre-1.0 framing outliving 1.0. Same shape as
    // `release_state_claims_agree_with_the_changelog`: CHANGELOG.md is the
    // source for whether 1.0 has shipped, and before it ships the marker
    // has to match live prose, so a rewording cannot leave this guarding a
    // phrase nobody writes.
    const SURFACES: [&str; 2] = ["README.md", "site/index.html"];
    const PRE_1_0: &str = "until 1.0";

    let released_1_0 = changelog_released_versions().into_iter().find(|version| {
        version
            .split('.')
            .next()
            .and_then(|major| major.parse::<u32>().ok())
            .is_some_and(|major| major >= 1)
    });
    for file in SURFACES {
        let quotes = quotes_containing(file, PRE_1_0);
        if let Some(version) = &released_1_0 {
            assert!(
                quotes.is_empty(),
                "CHANGELOG.md records release {version}, so {file} may no longer frame the format promise as pre-1.0: {quotes:?}"
            );
        } else {
            assert!(
                !quotes.is_empty(),
                "{file} no longer says {PRE_1_0:?}; before 1.0 it states the format promise in ADR-0050's pre-1.0 form, or this guard watches a phrase nobody writes"
            );
        }
    }
}

/// Every version `CHANGELOG.md` records as released: the bracket content of
/// each `## [...]` heading that is not `Unreleased`. Keep a Changelog's
/// release step renames that one heading, so this file carries the release
/// in the commit that cuts it, earlier than a tag reaches a shallow
/// checkout and earlier than a generated field can be regenerated.
fn changelog_released_versions() -> Vec<String> {
    read("CHANGELOG.md")
        .lines()
        .filter_map(|line| {
            let heading = line.strip_prefix("## [")?;
            let version = heading.split_once(']')?.0;
            (version != "Unreleased").then(|| version.to_owned())
        })
        .collect()
}

/// Every occurrence of `marker` in `file`, quoted with the words around it,
/// matched case-insensitively against whitespace-normalized text so a claim
/// wrapped across source lines still matches. A window rather than the
/// enclosing sentence, because these surfaces include HTML: markup carries
/// few sentence boundaries, so a sentence-scoped quote of `site/index.html`
/// runs to hundreds of characters of tags and reads as noise. Lowercasing is
/// ASCII-only, which leaves every byte offset into `text` valid.
fn quotes_containing(file: &str, marker: &str) -> Vec<String> {
    /// Characters of surrounding prose to quote on each side.
    const CONTEXT: usize = 40;

    let text = normalize_whitespace(&read(file));
    let haystack = text.to_ascii_lowercase();
    let mut quotes = Vec::new();
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(marker) {
        let at = from + offset;
        let start = text[..at]
            .char_indices()
            .rev()
            .take(CONTEXT)
            .last()
            .map_or(at, |(index, _)| index);
        let end = text[at..]
            .char_indices()
            .nth(marker.len() + CONTEXT)
            .map_or(text.len(), |(index, _)| at + index);
        quotes.push(text[start..end].to_owned());
        from = at + marker.len();
    }
    quotes
}

#[test]
fn release_state_claims_agree_with_the_changelog() {
    // Five sentences on `/` and two in README.md tell the reader that
    // mothergod is pre-alpha and that no release exists, including the
    // `description`/`og:description` copy that is all an unfurled link
    // shows. `status-data.py` derives the same fact as `phase`, but it
    // renders only on `/status.html`, which no measured pageload has ever
    // landed on (marketing/JOURNAL.md, 2026-08-31, 09-12, 09-19). So the
    // copies that matter are hand-typed, and cutting 0.1.0 touches none of
    // the files they live in (issue #703). `/` is JavaScript-free by
    // decision (issue #431), so the mechanism is a guard on the copies
    // rather than a fetch of the generated field.
    //
    // Residual gap, named rather than hidden: this watches a fixed
    // vocabulary of denials, so a sixth sentence that denies the release in
    // other words goes unguarded. That is why the pre-release branch below
    // asserts each marker still matches live prose. A marker matching
    // nothing is dead machinery guarding nobody, and it would pass forever.
    const SURFACES: [&str; 4] = [
        "README.md",
        "site/index.html",
        "site/status.html",
        "site/agents.html",
    ];
    // Both fall with the first release: a project with a published version
    // has a release, and does not call itself pre-alpha on its landing page.
    const DENIALS: [&str; 2] = ["no release", "pre-alpha"];

    let released = changelog_released_versions();
    for marker in DENIALS {
        let found: Vec<(&str, Vec<String>)> = SURFACES
            .iter()
            .map(|file| (*file, quotes_containing(file, marker)))
            .filter(|(_, quotes)| !quotes.is_empty())
            .collect();

        if let Some(version) = released.first() {
            assert!(
                found.is_empty(),
                "CHANGELOG.md records release {version}, so {marker:?} is false wherever it is still written: {found:?}"
            );
        } else {
            assert!(
                !found.is_empty(),
                "no surface says {marker:?} anymore, so this guard watches a phrase nobody writes; re-anchor it on the words the surfaces use now"
            );
        }
    }
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

/// The first four numbers of README's table row for `corpus`: mothergod,
/// gzip -9, zstd -19, xz -9e, in the report's column order.
fn aggregate_spans_in_readme(text: &str, corpus: &str) -> Vec<Range<usize>> {
    let row = line_range(text, &format!("| {corpus} |"));
    markdown_number_spans(text, row)
        .into_iter()
        .take(4)
        .collect()
}

/// The first four numbers of the site's Measured-table row for `corpus`, in
/// the same order.
fn aggregate_spans_in_site(text: &str, corpus: &str) -> Vec<Range<usize>> {
    let marker = format!("<th scope=\"row\">{corpus}</th>");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{marker:?} not found in site/index.html"));
    let end = text[start..]
        .find("</tr>")
        .map_or(text.len(), |at| start + at);
    html_number_spans(text, start..end)
        .into_iter()
        .take(4)
        .collect()
}

/// A report figure as the surfaces print it: three decimals (issue #431:
/// "1.374", not "1.373741"). The throughput columns already carry three.
fn three_decimals(value: f64) -> String {
    format!("{value:.3}")
}

#[test]
fn aggregate_ratios_match_their_generated_reports() {
    let corpora = [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")];
    let columns = ["mothergod", "gzip -9", "zstd -19", "xz -9e"];

    for (corpus, report_file) in corpora {
        let truth = aggregate_from_report(report_file).map(three_decimals);
        let source = format!("docs/benchmarks/{report_file}");
        let name = |index: usize| format!("{corpus} {} bits/byte", columns[index]);

        check_figures(
            "README.md",
            &source,
            |text| aggregate_spans_in_readme(text, corpus),
            &truth,
            name,
        );
        check_figures(
            "site/index.html",
            &source,
            |text| aggregate_spans_in_site(text, corpus),
            &truth,
            name,
        );
    }
}

/// The `[Unreleased]` section of `CHANGELOG.md`: its heading to the next
/// release heading. A figure in a released section is history.
fn changelog_unreleased(text: &str) -> Range<usize> {
    let start = text
        .find("## [Unreleased]")
        .expect("CHANGELOG.md has an [Unreleased] section");
    let end = text[start..]
        .find("\n## [")
        .map_or(text.len(), |at| start + at);
    start..end
}

/// `CHANGELOG.md`'s "Ratio" bullet names only three of the four columns the
/// other surfaces carry, in this order: `[mothergod, zstd -19, xz -9e]`. It
/// never names gzip, so it maps onto `aggregate_from_report`'s four-wide
/// array by skipping index 1.
fn aggregate_spans_in_changelog(text: &str, corpus: &str) -> Vec<Range<usize>> {
    let marker = format!("{corpus} (");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{marker:?} not found in CHANGELOG.md"))
        + marker.len();
    let end = text[start..]
        .find(')')
        .unwrap_or_else(|| panic!("unterminated {corpus} parenthetical in CHANGELOG.md"))
        + start;
    prose_number_spans(text, start..end)
        .into_iter()
        .take(3)
        .collect()
}

#[test]
fn changelog_ratio_claim_matches_its_generated_reports() {
    let corpora = [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")];
    let columns = ["mothergod", "zstd -19", "xz -9e"];
    let report_indices = [0, 2, 3];

    for (corpus, report_file) in corpora {
        let report = aggregate_from_report(report_file);
        let truth = report_indices.map(|index| three_decimals(report[index]));

        check_figures_within(
            "CHANGELOG.md",
            &format!("docs/benchmarks/{report_file}"),
            changelog_unreleased,
            |text| aggregate_spans_in_changelog(text, corpus),
            &truth,
            |index| format!("{corpus} {} bits/byte", columns[index]),
        );
    }
}

/// Plain (non-decimal) integers anywhere in free-flowing prose, in
/// encounter order: `"wins 6 of 11 on Canterbury"` -> `[6, 11]`. Separate
/// from `prose_number_spans` because a win count has no fractional part, and
/// parsing straight to `usize` avoids a float-to-int cast on a count.
fn prose_integers(text: &str) -> Vec<usize> {
    let mut values = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if let Ok(value) = text[start..index].parse::<usize>() {
                values.push(value);
            }
        } else {
            index += 1;
        }
    }
    values
}

/// Each data row's regret column (mothergod's bits/byte minus the stronger
/// of zstd/xz, `docs/benchmarks/{canterbury,silesia}.md`'s own header),
/// read off the single source of truth rather than recomputed from the
/// four compressor columns, so this guard and the report agree on what
/// "stronger" means by construction. Excludes the `**aggregate` row, which
/// a per-file win count never counts. Paired with the file's name, the
/// row's first backticked cell.
fn per_file_regrets(report_file: &str) -> Vec<(String, f64)> {
    let text = read(&format!("docs/benchmarks/{report_file}"));
    text.lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let name = line.split('`').nth(1).unwrap_or_default().to_owned();
            (name, markdown_numbers(line)[5])
        })
        .collect()
}

/// How many files mothergod wins (negative regret) out of how many total.
fn win_count_from_report(report_file: &str) -> (usize, usize) {
    let regrets = per_file_regrets(report_file);
    let wins = regrets.iter().filter(|(_, regret)| *regret < 0.0).count();
    (wins, regrets.len())
}

/// The "wins `<N>` of `<M>` on Canterbury and `<N>` of `<M>` on Silesia"
/// sentence's four numbers, in that order.
fn win_claim(text: &str) -> [(usize, usize); 2] {
    let marker = "wins ";
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"))
        + marker.len();
    let rest = &text[start..];
    let end = rest.find('.').unwrap_or(rest.len());
    let numbers = prose_integers(&rest[..end]);
    assert_eq!(
        numbers.len(),
        4,
        "expected 4 numbers (wins, total, wins, total) in the win-count sentence, found {numbers:?}"
    );
    [(numbers[0], numbers[1]), (numbers[2], numbers[3])]
}

#[test]
fn win_count_claims_match_their_generated_reports() {
    let corpora = [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")];
    let surfaces = [
        ("README.md", read("README.md")),
        ("site/index.html", read("site/index.html")),
    ];

    for (surface_name, text) in &surfaces {
        let claims = win_claim(text);
        for (index, (corpus, report_file)) in corpora.iter().enumerate() {
            let truth = win_count_from_report(report_file);
            assert_eq!(
                claims[index], truth,
                "{surface_name}'s {corpus} per-file win claim is {} of {}, docs/benchmarks/{report_file} says {} of {}",
                claims[index].0, claims[index].1, truth.0, truth.1
            );
        }
    }
}

/// The code-quoted file names in the sentence of `text` that opens with
/// `marker`, sorted; `quote` is the opening and closing delimiter pair,
/// backticks in Markdown, `<code>` tags in HTML. Whitespace is collapsed
/// first so a sentence wrapped across source lines still matches; the
/// sentence ends at a period followed by a space, never at the period
/// inside `fields.c`. An empty list panics: the loss check sizes its
/// ranking by this list's length, so zero names would compare an empty
/// claim to an empty truth.
fn files_named_after(text: &str, marker: &str, (open, close): (&str, &str)) -> Vec<String> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let start = flat
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"));
    let rest = &flat[start..];
    let mut sentence = &rest[..rest.find(". ").unwrap_or(rest.len())];
    let mut names = Vec::new();
    while let Some(at) = sentence.find(open) {
        let quoted = &sentence[at + open.len()..];
        let end = quoted
            .find(close)
            .unwrap_or_else(|| panic!("unclosed {open:?} after {marker:?}"));
        names.push(quoted[..end].to_owned());
        sentence = &quoted[end + close.len()..];
    }
    assert!(
        !names.is_empty(),
        "the sentence opening {marker:?} names no code-quoted file, so the claim checks nothing"
    );
    names.sort();
    names
}

#[test]
fn named_wins_and_widest_losses_match_their_generated_reports() {
    // README and the site name the files behind the win count, and the
    // files mothergod loses on by the widest margin, so a reader can ask
    // "will it win on my data" (issues #1515, #1566). Every regret across
    // both reports is the truth: the named wins are exactly the files with
    // negative regret, and the named losses are exactly as many files as
    // are named, taken from the top of the regret ranking.
    let mut regrets = per_file_regrets("canterbury.md");
    regrets.extend(per_file_regrets("silesia.md"));
    let mut true_wins: Vec<String> = regrets
        .iter()
        .filter(|(_, regret)| *regret < 0.0)
        .map(|(name, _)| name.clone())
        .collect();
    true_wins.sort();
    regrets.sort_by(|a, b| b.1.total_cmp(&a.1));

    let surfaces = [
        ("README.md", ("`", "`")),
        ("site/index.html", ("<code>", "</code>")),
    ];
    for (surface, quote) in surfaces {
        let text = read(surface);
        let claimed_wins = files_named_after(&text, "The wins are ", quote);
        assert_eq!(
            claimed_wins, true_wins,
            "{surface} names {claimed_wins:?} as its per-file wins, docs/benchmarks/ says {true_wins:?}"
        );

        let claimed_losses = files_named_after(&text, "The widest losses are ", quote);
        let mut true_losses: Vec<String> = regrets
            .iter()
            .take(claimed_losses.len())
            .map(|(name, _)| name.clone())
            .collect();
        true_losses.sort();
        assert_eq!(
            claimed_losses, true_losses,
            "{surface} names {claimed_losses:?} as its widest losses, docs/benchmarks/ ranks {true_losses:?} widest"
        );
    }
}

/// The verdict strip's per-corpus judgment word ("Wins"/"Loses") and its
/// caption text, in strip order. Scoped between the `class="verdict"`
/// container and its closing `</div>` so a same-named span elsewhere on
/// the page cannot match.
fn verdict_items(page: &str) -> Vec<(String, String)> {
    let container_start = page
        .find("class=\"verdict\"")
        .unwrap_or_else(|| panic!("no .verdict container in site/index.html"));
    let container_end = page[container_start..]
        .find("<p class=\"verdict-link\"")
        .map_or(page.len(), |offset| container_start + offset);
    let container = &page[container_start..container_end];

    let mut items = Vec::new();
    let mut rest = container;
    while let Some(item_start) = rest.find("class=\"verdict-item\"") {
        let item = &rest[item_start..];
        let item_end = item
            .find("</div>")
            .unwrap_or_else(|| panic!("unterminated verdict-item div"));
        let item = &item[..item_end];

        let figure = {
            let marker = "verdict-figure";
            let marker_start = item
                .find(marker)
                .unwrap_or_else(|| panic!("verdict-item has no verdict-figure span"));
            let tag_end = item[marker_start..]
                .find('>')
                .unwrap_or_else(|| panic!("unterminated verdict-figure span"))
                + marker_start;
            let text_end = item[tag_end..]
                .find('<')
                .unwrap_or_else(|| panic!("unterminated verdict-figure span"))
                + tag_end;
            item[tag_end + 1..text_end].trim().to_string()
        };
        let caption = {
            let marker = "verdict-caption";
            let marker_start = item
                .find(marker)
                .unwrap_or_else(|| panic!("verdict-item has no verdict-caption span"));
            let tag_end = item[marker_start..]
                .find('>')
                .unwrap_or_else(|| panic!("unterminated verdict-caption span"))
                + marker_start;
            let text_end = item[tag_end..]
                .find("</span>")
                .unwrap_or_else(|| panic!("unterminated verdict-caption span"))
                + tag_end;
            item[tag_end + 1..text_end].to_string()
        };
        items.push((figure, caption));
        rest = &rest[item_start + item_end..];
    }
    items
}

/// Whether the aggregate in `report_file` beats, or loses to, both
/// reference compressors: the only two outcomes the verdict strip's
/// binary Wins/Loses word can represent honestly.
fn verdict_truth(report_file: &str) -> &'static str {
    let [mothergod, _gzip, zstd, xz] = aggregate_from_report(report_file);
    if mothergod < zstd && mothergod < xz {
        "Wins"
    } else if mothergod > zstd && mothergod > xz {
        "Loses"
    } else {
        panic!(
            "docs/benchmarks/{report_file}'s aggregate beats one reference and loses to the \
             other; the verdict strip's binary Wins/Loses word cannot represent that honestly"
        )
    }
}

#[test]
fn verdict_strip_words_match_their_aggregate_comparisons() {
    // The header's verdict strip restates a qualitative claim the Measured
    // section's numbers already prove (issue #604 item 1), but as a second,
    // separate location: nothing but this test ties its "Wins"/"Loses"
    // words to the comparison they name, so a regenerated report that flips
    // one could leave the header stale (site/index.html itself calls
    // closing Silesia the head of the roadmap, i.e. the exact word this
    // strip hardcodes for Silesia today is a live target).
    let site = read("site/index.html");
    let items = verdict_items(&site);

    for (corpus, report_file) in [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")] {
        let truth = verdict_truth(report_file);
        let (word, _caption) = items
            .iter()
            .find(|(_, caption)| caption.starts_with(corpus))
            .unwrap_or_else(|| {
                panic!("no verdict-item caption in site/index.html mentions {corpus}")
            });
        assert_eq!(
            word, truth,
            "site/index.html's verdict strip says {corpus} {word:?}, but docs/benchmarks/{report_file} says mothergod {truth} it"
        );
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

/// Byte ranges of the first two numbers immediately followed by `MB/s`,
/// scanning from `marker` onward. README states a corpus name once and then
/// gives its two rates in encode-then-decode order in prose ("encoded
/// Canterbury at 0.133 MB/s and decoded it at 4.422 MB/s"), so the corpus
/// name is the only anchor this needs and the prose stays free to reword
/// around it.
fn throughput_spans_after(text: &str, marker: &str) -> Vec<Range<usize>> {
    let mut cursor = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"))
        + marker.len();
    (0..2)
        .map(|_| {
            let unit = cursor
                + text[cursor..]
                    .find("MB/s")
                    .unwrap_or_else(|| panic!("no MB/s rate after {marker:?}"));
            let end = text[..unit].trim_end().len();
            let digits = text[cursor..end]
                .bytes()
                .rev()
                .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
                .count();
            cursor = unit + "MB/s".len();
            end - digits..end
        })
        .collect()
}

/// The Speed table's encode and decode MB/s cells for `corpus`, scoped to
/// `id="speed"` so it cannot match the Measured table's own
/// `<th scope="row">{corpus}</th>` row (issue #604 item 2: the Speed section
/// states its rates in a table, not prose, so this reads cells the way
/// `aggregate_spans_in_site` does).
fn throughput_spans_in_site(text: &str, corpus: &str) -> Vec<Range<usize>> {
    let table = text
        .find("id=\"speed\"")
        .unwrap_or_else(|| panic!("no #speed table in site/index.html"));
    let marker = format!("<th scope=\"row\">{corpus}</th>");
    let start = table
        + text[table..]
            .find(&marker)
            .unwrap_or_else(|| panic!("{marker:?} not found in the #speed table"));
    let end = text[start..]
        .find("</tr>")
        .map_or(text.len(), |at| start + at);
    html_number_spans(text, start..end)
        .into_iter()
        .take(2)
        .collect()
}

#[test]
fn published_throughput_matches_its_generated_reports() {
    let corpora = [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")];

    for (corpus, report_file) in corpora {
        let truth = throughput_from_report(report_file).map(three_decimals);
        let source = format!("docs/benchmarks/{report_file}");
        let name = |index: usize| format!("{corpus} {} MB/s", ["encode", "decode"][index]);
        let marker = format!("encoded {corpus} at");

        check_figures(
            "README.md",
            &source,
            |text| throughput_spans_after(text, &marker),
            &truth,
            name,
        );
        check_figures(
            "site/index.html",
            &source,
            |text| throughput_spans_in_site(text, corpus),
            &truth,
            name,
        );
    }
}

/// The frontier row's encode and decode MB/s for `level`, from the
/// generated report's every-level table.
fn frontier_throughput(report_file: &str, level: &str) -> [f64; 2] {
    let text = read(&format!("docs/benchmarks/{report_file}"));
    let row = line_containing(&text, &format!("| {level} |"));
    // Columns after the level: b/B, encode MB/s, decode MB/s, peak MiB.
    let numbers = markdown_numbers(row);
    [numbers[1], numbers[2]]
}

/// README and site anchor "slow" against `xz -9e` on Silesia: they quote its
/// rates from the same run and call it "more than nine times faster to
/// encode and more than twenty times faster to decode". The rates are
/// compared like `published_throughput_matches_its_generated_reports`
/// compares mothergod's; each multiplier holds while the report's ratio
/// stays at or above it, so a faster encoder fails here before the sentence
/// turns false. The encode multiplier was ten until the 2026-10-21 run on a
/// different CI CPU measured 9.6: the ratio moves with the machine, so the
/// sentence states what the newest report supports.
#[test]
fn published_speed_anchor_matches_its_generated_report() {
    let xz = frontier_throughput("silesia.md", "xz -9e");
    let ours = throughput_from_report("silesia.md");
    let truth = xz.map(three_decimals);
    let name = |index: usize| format!("xz -9e Silesia {} MB/s", ["encode", "decode"][index]);

    check_figures(
        "README.md",
        "docs/benchmarks/silesia.md",
        |text| throughput_spans_after(text, "`xz -9e` encoded Silesia at"),
        &truth,
        name,
    );
    check_figures(
        "site/index.html",
        "docs/benchmarks/silesia.md",
        |text| throughput_spans_after(text, "<code>xz -9e</code> encoded Silesia at"),
        &truth,
        name,
    );

    for (index, (direction, floor)) in [("encode", 9.0), ("decode", 20.0)].iter().enumerate() {
        let ratio = xz[index] / ours[index];
        assert!(
            ratio >= *floor,
            "README.md and site/index.html say xz -9e is more than {floor} times faster to {direction} Silesia, docs/benchmarks/silesia.md says {ratio:.1} times"
        );
    }
}

/// Byte range of the CPU model token after "AMD EPYC ": the run of
/// characters up to the next whitespace or comma, which strips the report's
/// own "96-Core Processor" suffix (`"AMD EPYC 9V45 96-Core Processor,"` ->
/// `"9V45"`) and the site prose's own trailing comma (`"AMD EPYC 9V45, 4
/// logical"` -> `"9V45"`) the same way regardless of which form `text` uses.
fn epyc_model_span(text: &str) -> Range<usize> {
    let marker = "AMD EPYC ";
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"))
        + marker.len();
    let end = text[start..]
        .find(|c: char| c.is_whitespace() || c == ',')
        .map_or(text.len(), |at| start + at);
    start..end
}

/// Byte range of the date after `marker`: the run of digits and hyphens, so
/// a surface's "Measured 2026-10-24 against" and a report's "As of
/// 2026-10-24T17:20:21Z" both end at the date.
fn date_span_after(text: &str, marker: &str) -> Range<usize> {
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"))
        + marker.len();
    let end = text[start..]
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .map_or(text.len(), |at| start + at);
    start..end
}

/// What `figure` locates in the `docs/benchmarks/` reports, which must agree:
/// both name the CPU, the date and the tool versions of one pass.
fn from_both_reports(what: &str, figure: impl Fn(&str) -> Range<usize>) -> String {
    let readings: Vec<(String, String)> = ["canterbury.md", "silesia.md"]
        .iter()
        .map(|report_file| {
            let text = read(&format!("docs/benchmarks/{report_file}"));
            (
                format!("docs/benchmarks/{report_file}"),
                text[figure(&text)].to_owned(),
            )
        })
        .collect();
    agreed(what, &readings)
}

const BOTH_REPORTS: &str = "docs/benchmarks/{canterbury,silesia}.md";

#[test]
fn published_cpu_model_matches_its_generated_reports() {
    check_figures(
        "site/index.html",
        BOTH_REPORTS,
        |text| vec![epyc_model_span(text)],
        &[from_both_reports(
            "the benchmark CPU model",
            epyc_model_span,
        )],
        |_| "CPU model".to_owned(),
    );
}

#[test]
fn measurement_date_matches_its_report() {
    let truth = from_both_reports("the measurement date", |text| {
        date_span_after(text, "As of ")
    });
    for file in ["README.md", "site/index.html"] {
        check_figures(
            file,
            BOTH_REPORTS,
            |text| vec![date_span_after(text, "Measured ")],
            std::slice::from_ref(&truth),
            |_| "measurement date".to_owned(),
        );
    }
}

/// Byte range of the first dotted version number (a digit run containing
/// `.`) after the first occurrence of `name` in `text`. Scans past
/// intervening prose rather than taking the text immediately after `name`:
/// the report's own zstd line reads "Zstandard CLI (64-bit) v1.5.7", so a run
/// of digits starts at "64" (from "64-bit") before the actual version, and
/// only the dot distinguishes the real version from that false start.
fn version_span_after(text: &str, name: &str) -> Range<usize> {
    let mut at = text
        .find(name)
        .unwrap_or_else(|| panic!("{name:?} not found"))
        + name.len();
    loop {
        let start = at
            + text[at..]
                .find(|c: char| c.is_ascii_digit())
                .unwrap_or_else(|| panic!("no version digits after {name:?}"));
        let end = text[start..]
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .map_or(text.len(), |len| start + len);
        if text[start..end].contains('.') {
            return start..end;
        }
        at = end;
    }
}

#[test]
fn reference_compressor_versions_match_their_generated_reports() {
    for tool in ["gzip", "Zstandard", "XZ Utils"] {
        let truth = from_both_reports(&format!("the {tool} version"), |text| {
            version_span_after(text, tool)
        });
        for file in ["README.md", "site/index.html"] {
            check_figures(
                file,
                BOTH_REPORTS,
                |text| vec![version_span_after(text, tool)],
                std::slice::from_ref(&truth),
                |_| format!("{tool} version"),
            );
        }
    }
}

/// Every data row's decode MB/s across both reports: the last column of a
/// per-file row (`docs/benchmarks/{canterbury,silesia}.md`'s own header).
/// Excludes the `**aggregate` row, which is not a file.
fn per_file_decode_rates() -> Vec<f64> {
    ["canterbury.md", "silesia.md"]
        .iter()
        .flat_map(|report_file| {
            read(&format!("docs/benchmarks/{report_file}"))
                .lines()
                .filter(|line| line.starts_with("| `"))
                .map(|line| markdown_numbers(line)[7])
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn site_per_file_decode_range_matches_its_generated_reports() {
    // "Per-file decode rates in those reports run from 0.6 to 25.8 MB/s":
    // the slowest and the fastest file of either report, to one decimal.
    // Unpinned until issue #1486: a publish moved it and nothing said so.
    let rates = per_file_decode_rates();
    let slowest = rates.iter().copied().fold(f64::INFINITY, f64::min);
    let fastest = rates.iter().copied().fold(0.0, f64::max);

    check_figures(
        "site/index.html",
        BOTH_REPORTS,
        |text| {
            let marker = "<span class=\"rate\">";
            let start = text
                .find(marker)
                .unwrap_or_else(|| panic!("{marker:?} not found in site/index.html"))
                + marker.len();
            let end = text[start..]
                .find("</span>")
                .map_or(text.len(), |at| start + at);
            prose_number_spans(text, start..end)
        },
        &[format!("{slowest:.1}"), format!("{fastest:.1}")],
        |index| format!("per-file {} decode MB/s", ["lowest", "highest"][index]),
    );
}

/// What `mothergod --help` prints. The binary's own usage text is the source
/// of truth for its interface: `src/bin/mothergod.rs` is where a rename
/// happens, and running it is the only reading that cannot go stale.
fn help_text() -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mothergod"))
        .arg("--help")
        .output()
        .expect("mothergod binary should spawn");
    assert!(output.status.success(), "mothergod --help should exit 0");
    String::from_utf8(output.stdout).expect("usage text is UTF-8")
}

/// The options a surface tells the reader to type: the dash-led words after
/// each invocation of the built binary on the same line. Anchored on the path
/// segment `release/mothergod ` so it picks up the copy-pasteable command lines
/// (`./target/release/mothergod -d < FILE.mgdc > FILE.out`) and not the prose
/// mentions of `mothergod --help`. The count of invocations rides along: a
/// recipe is a pair, so fewer than two means the anchor stopped matching.
fn published_invocations(surface: &str) -> Vec<Vec<&str>> {
    surface
        .match_indices("release/mothergod")
        .map(|(index, marker)| {
            surface[index + marker.len()..]
                .lines()
                .next()
                .expect("a line follows the binary path")
                .split_whitespace()
                .filter(|word| word.starts_with('-'))
                .collect()
        })
        .collect()
}

#[test]
fn published_cli_recipe_names_options_the_binary_answers_to() {
    // `tests/cli.rs` proves the binary behaves; this proves the recipe on
    // the public surfaces still names what it answers to, which is the half
    // a stranger's terminal would otherwise discover for us.
    let help = help_text();
    // Whole tokens, not substrings: `-d` is a substring of `--decompress`, so
    // a `contains` would keep passing after the short option was renamed.
    let help_words: Vec<&str> = help
        .split(|c: char| c.is_whitespace() || "[](),;|<>".contains(c))
        .filter(|word| !word.is_empty())
        .collect();

    for surface_file in ["README.md", "site/index.html"] {
        let surface = read(surface_file);
        let invocations = published_invocations(&surface);
        assert!(
            invocations.len() >= 2,
            "{surface_file} publishes {} runnable mothergod commands, expected the compress and decompress pair",
            invocations.len()
        );
        for option in invocations.into_iter().flatten() {
            assert!(
                help_words.contains(&option),
                "{surface_file} tells the reader to run `mothergod {option}`; `mothergod --help` does not mention it"
            );
        }
        // The suffix is restated in the recipe on both surfaces and derived
        // from `SUFFIX` in the binary, which prints it in the same usage.
        assert!(
            surface.contains(".mgdc") && help.contains(".mgdc"),
            "{surface_file} and mothergod --help disagree about the compressed-file suffix"
        );
    }
}

#[test]
fn published_first_run_transcript_matches_what_the_binary_prints() {
    // Both "Try it" sections quote one `-v` run on the clone's `LICENSE`
    // (issue #1196). A codec change that moves its size would otherwise
    // leave a stranger's terminal disagreeing with the page they followed.
    // The quoted run is `-v -c LICENSE > /dev/null`: it writes no
    // `LICENSE.mgdc`, so a reader can repeat it (issue #1216), and stdout is
    // not a terminal, which `-c` would refuse. The test runs it the same way.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_mothergod"))
        .args(["-v", "-c", "LICENSE"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdout(std::process::Stdio::null())
        .output()
        .expect("mothergod binary should spawn");
    assert!(
        output.status.success(),
        "mothergod -v -c LICENSE should exit 0"
    );
    let printed = String::from_utf8(output.stderr).expect("verbose line is UTF-8");
    let truth = printed.trim_end();

    for surface_file in ["README.md", "site/index.html"] {
        let surface = read(surface_file).replace("&gt;", ">");
        assert!(
            surface.contains("$ ./target/release/mothergod -v -c LICENSE > /dev/null\n"),
            "{surface_file} no longer quotes the `mothergod -v -c LICENSE > /dev/null` run this test checks"
        );
        // The site's `<pre>` closes on the transcript's last line.
        let claimed = line_containing(&surface, "mothergod: LICENSE: ")
            .trim()
            .trim_end_matches("</pre>");
        assert_eq!(
            claimed, truth,
            "{surface_file} quotes `{claimed}` for `mothergod -v -c LICENSE`; the binary prints `{truth}`"
        );
    }
}

/// The text between `<title>` and `</title>`.
fn title_text(page: &str) -> &str {
    let start = page
        .find("<title>")
        .unwrap_or_else(|| panic!("no <title> tag"))
        + "<title>".len();
    let rest = &page[start..];
    let end = rest
        .find("</title>")
        .unwrap_or_else(|| panic!("unterminated <title>"));
    &rest[..end]
}

/// The full `<... >` tag whose attributes contain `selector` (e.g.
/// `name="description"` or `property="og:title"`), scoped to the `<head>` so
/// a same-named string in page body text cannot match, and bounded to the
/// enclosing `<` and `>` so attribute lookups on it cannot spill into the
/// next tag when this one omits the attribute being looked up.
fn tag_containing<'a>(page: &'a str, selector: &str) -> &'a str {
    let head_end = page.find("</head>").unwrap_or(page.len());
    let head = &page[..head_end];
    let selector_start = head
        .find(selector)
        .unwrap_or_else(|| panic!("no tag with {selector:?} found"));
    let open = head[..selector_start]
        .rfind('<')
        .unwrap_or_else(|| panic!("no tag start before {selector:?}"));
    let close = head[selector_start..]
        .find('>')
        .unwrap_or_else(|| panic!("unterminated tag after {selector:?}"));
    &head[open..selector_start + close]
}

/// The `attr="..."` value inside a single already-scoped tag.
fn attr_value<'a>(tag: &'a str, attr: &str) -> &'a str {
    let marker = format!("{attr}=\"");
    let start = tag
        .find(&marker)
        .unwrap_or_else(|| panic!("tag has no {attr} attribute: {tag:?}"))
        + marker.len();
    let after = &tag[start..];
    let end = after
        .find('"')
        .unwrap_or_else(|| panic!("unterminated {attr} attribute in {tag:?}"));
    &after[..end]
}

/// The `content="..."` value of the head tag matching `selector`.
fn meta_content<'a>(page: &'a str, selector: &str) -> &'a str {
    attr_value(tag_containing(page, selector), "content")
}

/// The `href="..."` value of `<link rel="canonical">`.
fn canonical_href(page: &str) -> &str {
    attr_value(tag_containing(page, "rel=\"canonical\""), "href")
}

/// The number immediately following `marker` in `text`, e.g. `13` in
/// "pins the 13 archives".
fn number_after(text: &str, marker: &str) -> u32 {
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

#[test]
fn fuzz_target_count_matches_fuzz_fuzz_targets() {
    let true_count =
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/fuzz_targets"))
            .expect("fuzz/fuzz_targets should be readable")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
            .count();

    let claim = number_after(&read("site/index.html"), "Backed by ");
    assert_eq!(
        claim as usize, true_count,
        "site/index.html claims {claim} fuzz targets, fuzz/fuzz_targets/ holds {true_count}"
    );
}

#[test]
fn pinned_corpus_archive_count_matches_bench_corpus_toml() {
    let toml = read("bench/corpus.toml");
    let true_count = toml.matches("[[file]]").count();

    let claim = number_after(&read("site/index.html"), "pins the ");
    assert_eq!(
        claim as usize, true_count,
        "site/index.html claims {claim} pinned archives, bench/corpus.toml has {true_count} [[file]] entries"
    );
}

#[test]
fn experiment_floor_holds_against_research_progress_jsonl() {
    // A floor (`>=`), not an exact count: every experiment appends a line
    // (CLAUDE.md rule 6), so an exact-equality guard here would fail on
    // every unrelated PR that lands a research or codec experiment,
    // dragging site/index.html into a realm it does not own. The floor
    // only trips if the log shrinks or loses rejections, which is the
    // failure worth catching.
    let jsonl = read("research/progress.jsonl");
    let entries: Vec<&str> = jsonl
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let true_total = entries.len();
    let true_rejected = entries
        .iter()
        .filter(|line| line.contains("\"verdict\": \"rejected\""))
        .count();

    // The claim reads "... holds at least 160 entries, at least 40 of them
    // rejected ...": the two "at least" phrases in encounter order give
    // the total, then the rejected floor.
    let site = read("site/index.html");
    let mut floors = site
        .match_indices("at least ")
        .map(|(index, marker)| number_after(&site[index..], marker) as usize);
    let claimed_total = floors.next().expect("a total-experiments floor");
    let claimed_rejected = floors.next().expect("a rejected-experiments floor");

    assert!(
        true_total >= claimed_total,
        "site/index.html claims at least {claimed_total} experiments, research/progress.jsonl holds {true_total}"
    );
    assert!(
        true_rejected >= claimed_rejected,
        "site/index.html claims at least {claimed_rejected} rejected experiments, research/progress.jsonl holds {true_rejected}"
    );
}

/// A numbered rule's full text, from its `"<n>. "`-prefixed line up to the
/// next such line: a hard rule wraps across multiple lines in CLAUDE.md, so
/// `line_containing` alone would miss text on the rule's continuation
/// lines.
fn numbered_rule<'a>(text: &'a str, first_line: &str) -> &'a str {
    let start = text
        .find(first_line)
        .unwrap_or_else(|| panic!("{first_line:?} not found"));
    let rest = &text[start..];
    let end = rest
        .lines()
        .skip(1)
        .position(|line| line.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map_or(rest.len(), |offset_line| {
            rest.lines()
                .take(offset_line + 1)
                .map(|l| l.len() + 1)
                .sum()
        });
    &rest[..end]
}

/// Whitespace-normalized (newlines and indentation collapsed to single
/// spaces) so a phrase that wraps across a source line still matches a
/// contiguous `contains` check.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn independent_verification_claim_matches_claude_md_rules_3_and_8() {
    let claude_md = read("CLAUDE.md");
    let rule_3 = normalize_whitespace(numbered_rule(&claude_md, "3. Never weaken a guard"));
    assert!(
        rule_3.contains("you do not grade your own claim"),
        "CLAUDE.md rule 3 no longer forbids grading your own claim; site/index.html cites it by number"
    );
    let rule_8 = numbered_rule(&claude_md, "8. Do not merge your own PR");
    assert!(
        rule_8.contains("Do not merge your own PR"),
        "CLAUDE.md rule 8 no longer forbids merging your own PR; site/index.html cites it by number"
    );
}

#[test]
fn social_preview_tags_match_each_pages_own_title_and_description() {
    // A link unfurler reads `og:*`/`twitter:*`, not `<title>` or
    // `<meta name="description">`, so a page can update the reader-facing
    // pair and silently leave the unfurled preview stale (issue #523). This
    // is the description's single source of truth enforced across the
    // duplicate the OG spec requires: the og:title/og:description content
    // attributes, and the canonical/og:url pair naming the same address.
    let pages = [
        ("site/index.html", "https://mothergod.dev/"),
        ("site/status.html", "https://mothergod.dev/status"),
        ("site/agents.html", "https://mothergod.dev/agents"),
        ("site/decisions.html", "https://mothergod.dev/decisions"),
    ];

    for (file, canonical) in pages {
        let page = read(file);
        let title = title_text(&page);
        let description = meta_content(&page, "name=\"description\"");
        let og_title = meta_content(&page, "property=\"og:title\"");
        let og_description = meta_content(&page, "property=\"og:description\"");
        let og_url = meta_content(&page, "property=\"og:url\"");

        assert_eq!(
            og_title, title,
            "{file}'s og:title is {og_title:?}, its own <title> is {title:?}"
        );
        assert_eq!(
            og_description, description,
            "{file}'s og:description is {og_description:?}, its own meta description is {description:?}"
        );
        assert_eq!(
            canonical_href(&page),
            canonical,
            "{file}'s canonical link does not point at {canonical}"
        );
        assert_eq!(
            og_url, canonical,
            "{file}'s og:url is {og_url:?}, expected {canonical} to match its canonical link"
        );
    }
}

/// The `<nav class="frame">` element of a page, opening tag to closing tag.
fn frame_nav(page: &str) -> &str {
    let start = page
        .find("<nav class=\"frame\"")
        .unwrap_or_else(|| panic!("no <nav class=\"frame\"> on the page"));
    let rest = &page[start..];
    let end = rest
        .find("</nav>")
        .unwrap_or_else(|| panic!("unterminated <nav class=\"frame\">"))
        + "</nav>".len();
    &rest[..end]
}

/// Every link in a frame nav as `(href, visible text, marks the current
/// page)`, in document order. Parsed rather than string-compared so the HTML
/// formatter is free to rewrap the markup: what has to match across the three
/// pages is the destination list, not the indentation.
fn frame_links(nav: &str) -> Vec<(String, String, bool)> {
    nav.match_indices("<a ")
        .map(|(index, _)| {
            let rest = &nav[index..];
            let open_end = rest.find('>').expect("unterminated <a> in the frame nav");
            let open = &rest[..open_end];
            let text_end = rest.find("</a>").expect("unclosed <a> in the frame nav");
            (
                attr_value(open, "href").to_owned(),
                normalize_whitespace(&rest[open_end + 1..text_end]),
                open.contains("aria-current=\"page\""),
            )
        })
        .collect()
}

#[test]
fn shared_frame_offers_the_same_links_on_every_page() {
    // The frame (issue #604 item 4) is one nav copied inline into every page,
    // because the site has no build step to share a stylesheet or a partial.
    // A copy is a synchronization debt: a page gains a destination and the
    // others silently keep the old map, which means a reader who can reach a
    // page from one place and not from another.
    // This is that debt, paid by a test: same destinations in the same order
    // everywhere, and exactly one of them marked as where the reader is.
    // 404.html (issue #753 item 3) carries the same frame but marks none of
    // them current, because a not-found page is none of the site's real
    // destinations.
    let pages = [
        ("site/index.html", Some("/")),
        ("site/status.html", Some("/status")),
        ("site/agents.html", Some("/agents")),
        ("site/decisions.html", Some("/decisions")),
        ("site/404.html", None),
    ];

    // site/404.html says every page the site has is linked from its frame
    // (issue #1283), so the list above must be the directory, not a copy
    // of it: a page added to site/ and left out of the list ships unchecked
    // (issue #1284), and a listed page that is gone is a stale entry.
    let mut on_disk: Vec<String> =
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("site"))
            .expect("site/ should be readable")
            .map(|entry| entry.expect("site/ entry should be readable").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "html"))
            .map(|path| format!("site/{}", path.file_name().unwrap().to_string_lossy()))
            .collect();
    on_disk.sort();
    let mut listed: Vec<&str> = pages.iter().map(|(file, _)| *file).collect();
    listed.sort_unstable();
    assert_eq!(
        on_disk, listed,
        "site/*.html differs from the pages this test checks; add the new page to `pages`"
    );

    let mut expected: Option<(&str, Vec<(String, String)>)> = None;
    for (file, own_path) in pages {
        let page = read(file);
        let links = frame_links(frame_nav(&page));
        assert!(
            links.len() >= 3,
            "{file}'s frame nav has {} links; it should carry at least the three pages",
            links.len()
        );

        let current: Vec<&(String, String, bool)> =
            links.iter().filter(|(_, _, current)| *current).collect();
        let expected_current_count = usize::from(own_path.is_some());
        assert_eq!(
            current.len(),
            expected_current_count,
            "{file}'s frame nav marks {} links aria-current=\"page\"; expected {expected_current_count}",
            current.len()
        );
        if let Some(own_path) = own_path {
            assert_eq!(
                current[0].0, own_path,
                "{file}'s frame nav marks {:?} as the current page, but the page is served at {own_path}",
                current[0].0
            );
        }

        let destinations: Vec<(String, String)> = links
            .into_iter()
            .map(|(href, text, _)| (href, text))
            .collect();
        match &expected {
            None => expected = Some((file, destinations)),
            Some((first_file, first)) => assert_eq!(
                &destinations, first,
                "{file}'s frame nav offers {destinations:?}, {first_file}'s offers {first:?}"
            ),
        }
    }
}

#[test]
fn sitemap_lists_exactly_the_pages_with_a_canonical_tag() {
    // #753: a sitemap that names a page the site does not serve, or omits
    // one it does, teaches a crawler the wrong map. Read each page's own
    // canonical tag rather than a hardcoded literal, so this stays tied to
    // the pages' actual source instead of a copy of it.
    let pages = [
        "site/index.html",
        "site/status.html",
        "site/agents.html",
        "site/decisions.html",
    ];
    let canonical_urls: Vec<String> = pages
        .iter()
        .map(|file| canonical_href(&read(file)).to_owned())
        .collect();

    let sitemap = read("site/sitemap.xml");
    let listed: Vec<&str> = sitemap
        .match_indices("<loc>")
        .map(|(index, marker)| {
            let after = &sitemap[index + marker.len()..];
            let end = after
                .find("</loc>")
                .unwrap_or_else(|| panic!("unterminated <loc> in site/sitemap.xml"));
            &after[..end]
        })
        .collect();

    assert_eq!(
        listed,
        canonical_urls
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        "site/sitemap.xml lists {listed:?}, the site's canonical URLs are {canonical_urls:?}"
    );
}

#[test]
fn robots_txt_points_at_the_committed_sitemap() {
    // A robots.txt with a Sitemap: line naming a URL that 404s is worse
    // than no line at all, per the same reasoning #753 filed against the
    // canonical-tag defect: a crawler-facing pointer should name a URL
    // the site actually serves. Exact match, not substring: a `.bak`
    // suffix or any other corruption of the URL must fail this. Also
    // reads the sitemap file itself, so a renamed or deleted
    // site/sitemap.xml fails here too.
    let robots = read("site/robots.txt");
    let line = robots
        .lines()
        .find(|line| line.starts_with("Sitemap:"))
        .unwrap_or_else(|| panic!("site/robots.txt has no Sitemap: line: {robots:?}"));
    let url = line.trim_start_matches("Sitemap:").trim();
    assert_eq!(
        url, "https://mothergod.dev/sitemap.xml",
        "site/robots.txt's Sitemap: line names {url:?}, not the committed sitemap.xml"
    );
    read("site/sitemap.xml");
}
