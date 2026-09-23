//! Guard: `README.md` and `site/index.html` restate `FORMAT_VERSION`, the
//! decode-forever promise CLAUDE.md rule 5 makes about it, the published
//! aggregate bits/byte numbers, the published aggregate
//! encode/decode MB/s, the measurement date, the reference compressor
//! versions, the pre-alpha/no-release state of the project, and the CLI
//! recipe a reader is told to type, instead of
//! deriving any of them, so a codec change, a report regeneration, a
//! release or a
//! renamed subcommand can leave any of them stale with nothing catching it
//! (issue #431: twice in seven days, PR #243 and again the day this test
//! was added; issue #469: the report regenerated under an unchanged ratio
//! left only the restated date stale, and nothing compared it). Compares
//! every restated claim against its single source of truth:
//! `FORMAT_VERSION` against `src/lib.rs`'s own constant, the aggregate
//! figures, date and tool versions against the matching generated
//! `docs/benchmarks/*.md` report, the release state against
//! `CHANGELOG.md`'s own headings, the recipe against the binary's own
//! usage output, and fails naming the file, the claimed value, and the
//! true value.

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

use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
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

    let site_claim = digits_after(&read("site/index.html"), "container format (version");
    assert_eq!(
        site_claim, true_version,
        "site/index.html claims container format version {site_claim}, src/lib.rs's FORMAT_VERSION is {true_version}"
    );
}

#[test]
fn frozen_format_promise_is_open_ended_on_every_surface_that_makes_it() {
    // CLAUDE.md rule 5 owns this promise: from one floor version upward,
    // every format version decodes forever. README.md and site/index.html
    // restate it, and both spelled it as a closed list, "a version 2 or 3
    // frame". That list stopped being true the moment FORMAT_VERSION reached
    // 4: a reader compressing a file today was told the guarantee covers two
    // versions, neither of them theirs, by a sentence that then concluded
    // "so a frame written today stays readable". A closed list has to be
    // re-edited on every bump by whoever remembers these two files, so the
    // guard is on the open form rather than on the list's contents.
    let claude_md = normalize_whitespace(&read("CLAUDE.md"));
    let (floor, _) = number_and_tail_after(&claude_md, "that carve-out is retired for version ");
    let true_version = mothergod::FORMAT_VERSION;
    assert!(
        floor <= true_version,
        "CLAUDE.md rule 5 floors the decode-forever promise at version {floor}, above this build's FORMAT_VERSION {true_version}"
    );

    for file in ["README.md", "site/index.html"] {
        let surface = normalize_whitespace(&read(file));
        let (claimed_floor, tail) =
            number_and_tail_after(&surface, "decode support for a version ");
        assert_eq!(
            claimed_floor, floor,
            "{file} promises decode support from version {claimed_floor} up, CLAUDE.md rule 5 promises it from version {floor} up"
        );
        let opener: String = tail.chars().take(20).collect();
        assert!(
            tail.starts_with("or later"),
            "{file} states the decode-forever promise as \"version {claimed_floor} {opener}...\"; it has to read \"or later\", because a closed list of versions goes stale on the next FORMAT_VERSION bump"
        );
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
    // "Closing Silesia" the project's current milestone, i.e. the exact
    // word this strip hardcodes for Silesia today is a live target).
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

/// The first two numbers immediately followed by `MB/s`, scanning from
/// `marker` onward. README states a corpus name once and then gives its two
/// rates in encode-then-decode order in prose ("encoded Canterbury at 0.133
/// MB/s and decoded it at 4.422 MB/s"), so the corpus name is the only
/// anchor this needs and the prose stays free to reword around it.
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

/// The Speed table's encode/decode MB/s for `corpus`, scoped to `id="speed"`
/// so it cannot match the Measured table's own `<th scope="row">{corpus}</th>`
/// row (issue #604 item 2: the Speed section states its rates in a table,
/// not prose, so this reads cells the way `aggregate_from_site` does).
fn throughput_from_site(corpus: &str) -> [f64; 2] {
    let site = read("site/index.html");
    let table_start = site
        .find("id=\"speed\"")
        .unwrap_or_else(|| panic!("no #speed table in site/index.html"));
    let table = &site[table_start..];
    let marker = format!("<th scope=\"row\">{corpus}</th>");
    let start = table
        .find(&marker)
        .unwrap_or_else(|| panic!("{marker:?} not found in the #speed table"));
    let fragment = &table[start..];
    let end = fragment.find("</tr>").unwrap_or(fragment.len());
    let numbers = html_numbers(&fragment[..end]);
    [numbers[0], numbers[1]]
}

#[test]
fn published_throughput_matches_its_generated_reports() {
    let corpora = [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")];

    for (corpus, report_file) in corpora {
        let truth = throughput_from_report(report_file);
        let marker = format!("encoded {corpus} at");
        let readme = throughput_after(&read("README.md"), &marker);
        let site = throughput_from_site(corpus);

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

/// The date restated in a surface's "Measured `<date>`" phrase.
fn measured_date(text: &str) -> String {
    let marker = "Measured ";
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"))
        + marker.len();
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

/// A report's own measurement date, the date part of its `As of <timestamp>`
/// header.
fn report_date(report_file: &str) -> String {
    let text = read(&format!("docs/benchmarks/{report_file}"));
    let marker = "As of ";
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found in {report_file}"))
        + marker.len();
    let rest = &text[start..];
    let end = rest
        .find('T')
        .unwrap_or_else(|| panic!("no time separator after {marker:?} in {report_file}"));
    rest[..end].to_string()
}

#[test]
fn measurement_date_matches_its_report() {
    let readme_claim = measured_date(&read("README.md"));
    let site_claim = measured_date(&read("site/index.html"));

    for (corpus, report_file) in [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")] {
        let truth = report_date(report_file);
        assert_eq!(
            readme_claim, truth,
            "README.md's measurement date is {readme_claim}, docs/benchmarks/{report_file} ({corpus}) says {truth}"
        );
        assert_eq!(
            site_claim, truth,
            "site/index.html's measurement date is {site_claim}, docs/benchmarks/{report_file} ({corpus}) says {truth}"
        );
    }
}

/// The first dotted version number (a digit run containing `.`) after the
/// first occurrence of `name` in `text`. Scans past intervening prose
/// rather than taking the text immediately after `name`: the report's own
/// zstd line reads "Zstandard CLI (64-bit) v1.5.7", so a run of digits
/// starts at "64" (from "64-bit") before the actual version, and only the
/// dot distinguishes the real version from that false start.
fn version_after(text: &str, name: &str) -> String {
    let start = text
        .find(name)
        .unwrap_or_else(|| panic!("{name:?} not found"))
        + name.len();
    let mut rest = &text[start..];
    loop {
        let digit_start = rest
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or_else(|| panic!("no version digits after {name:?}"));
        let run = &rest[digit_start..];
        let run_end = run
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(run.len());
        let candidate = &run[..run_end];
        if candidate.contains('.') {
            return candidate.to_string();
        }
        rest = &run[run_end..];
    }
}

#[test]
fn reference_compressor_versions_match_their_generated_reports() {
    let readme = read("README.md");
    let site = read("site/index.html");

    for (corpus, report_file) in [("Canterbury", "canterbury.md"), ("Silesia", "silesia.md")] {
        let report = read(&format!("docs/benchmarks/{report_file}"));
        for tool in ["gzip", "Zstandard", "XZ Utils"] {
            let truth = version_after(&report, tool);
            let readme_claim = version_after(&readme, tool);
            let site_claim = version_after(&site, tool);
            assert_eq!(
                readme_claim, truth,
                "README.md's {tool} version is {readme_claim}, docs/benchmarks/{report_file} ({corpus}) says {truth}"
            );
            assert_eq!(
                site_claim, truth,
                "site/index.html's {tool} version is {site_claim}, docs/benchmarks/{report_file} ({corpus}) says {truth}"
            );
        }
    }
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

/// The subcommands the binary offers, from the `<compress|decompress>`
/// alternation in the first line of its usage. Parsed rather than substring
/// matched because `decompress` contains `compress`, so a plain `contains`
/// would keep passing after `compress` alone was renamed.
fn binary_subcommands(help: &str) -> Vec<&str> {
    let open = help.find('<').expect("usage line brackets its subcommands");
    let close = help[open..].find('>').expect("unterminated <...>") + open;
    help[open + 1..close].split('|').collect()
}

/// The subcommands a surface tells the reader to type, taken as the word
/// after each invocation of the built binary. Anchored on the path segment
/// `release/mothergod ` so it picks up the copy-pasteable command lines
/// (`./target/release/mothergod compress < FILE > FILE.mgdc`) and not the
/// prose mentions of `mothergod --help`.
fn published_subcommands(surface: &str) -> Vec<&str> {
    surface
        .match_indices("release/mothergod ")
        .map(|(index, marker)| {
            surface[index + marker.len()..]
                .split_whitespace()
                .next()
                .expect("a subcommand follows the binary path")
        })
        .collect()
}

#[test]
fn published_cli_recipe_names_commands_the_binary_answers_to() {
    // `tests/cli.rs` proves the binary behaves; this proves the recipe on
    // the public surfaces still names what it answers to, which is the half
    // a stranger's terminal would otherwise discover for us.
    let help = help_text();
    let offered = binary_subcommands(&help);

    for surface_file in ["README.md", "site/index.html"] {
        let surface = read(surface_file);
        let published = published_subcommands(&surface);
        assert!(
            published.len() >= 2,
            "{surface_file} publishes {} runnable mothergod commands, expected the compress and decompress pair",
            published.len()
        );
        for command in published {
            assert!(
                offered.contains(&command),
                "{surface_file} tells the reader to run `mothergod {command}`; the binary offers {offered:?}"
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

    // The claim reads "... holds at least 90 entries, at least 10 of them
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
        ("site/status.html", "https://mothergod.dev/status.html"),
        ("site/agents.html", "https://mothergod.dev/agents.html"),
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
    // The frame (issue #604 item 4) is three inline copies of one nav,
    // because the site has no build step to share a stylesheet or a partial.
    // A copy is a synchronization debt: a page gains a destination and the
    // other two silently keep the old map, which on a three-page site means
    // a reader who can reach a page from one place and not from another.
    // This is that debt, paid by a test: same destinations in the same order
    // everywhere, and exactly one of them marked as where the reader is.
    let pages = [
        ("site/index.html", "/"),
        ("site/status.html", "/status.html"),
        ("site/agents.html", "/agents.html"),
    ];

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
        assert_eq!(
            current.len(),
            1,
            "{file}'s frame nav marks {} links aria-current=\"page\"; exactly one is where the reader is",
            current.len()
        );
        assert_eq!(
            current[0].0, own_path,
            "{file}'s frame nav marks {:?} as the current page, but the page is served at {own_path}",
            current[0].0
        );

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
