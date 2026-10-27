//! The paths CI runs `test` and `doc` for, read from ci.yml.

use std::path::Path;

use regex::Regex;

/// Where ci.yml's `changes` job keeps the filter.
const CI: &str = ".github/workflows/ci.yml";

/// The `rust` filter of ci.yml's `changes` job: a PR runs `test` and `doc`
/// only when a changed path matches it. Read from the file, never copied,
/// so a scoped `cargo x check` skips exactly what CI skips (#916). The
/// pattern is the one `grep -qE` guard followed by `echo "rust=true"`,
/// the line `.github/scripts/ci-filter.test.mjs` extracts the same way
/// to hold it to what `tests/claims.rs` reads.
pub(crate) fn rust(root: &Path) -> Result<Regex, String> {
    let ci = std::fs::read_to_string(root.join(CI))
        .map_err(|error| format!("cannot read {CI}: {error}"))?;
    parse(&ci)
}

/// grep tests one path per line, so `^` and `$` anchor a whole path; the
/// regex is matched against one path at a time to keep that meaning.
fn parse(ci: &str) -> Result<Regex, String> {
    const GUARD: &str = "grep -qE '";
    let lines: Vec<&str> = ci.lines().collect();
    let pattern = lines
        .iter()
        .zip(lines.iter().skip(1))
        .find_map(|(line, next)| {
            let start = line.find(GUARD)? + GUARD.len();
            next.contains("echo \"rust=true\"").then(|| &line[start..])
        })
        .ok_or_else(|| format!("{CI} has no `grep -qE` guard followed by `echo \"rust=true\"`"))?;
    let end = pattern
        .find('\'')
        .ok_or_else(|| format!("{CI}: the rust guard's pattern is unterminated"))?;
    Regex::new(&pattern[..end])
        .map_err(|error| format!("{CI}: the rust filter is not an expression x can read: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_guard_that_sets_rust_true() {
        let ci = "          if echo \"$files\" | grep -qE '\\.md$'; then\n            echo \"quality=true\" >> \"$GITHUB_OUTPUT\"\n          fi\n          if echo \"$files\" | grep -qE '\\.rs$|^LICENSE$'; then\n            echo \"rust=true\" >> \"$GITHUB_OUTPUT\"\n";
        let filter = parse(ci).unwrap();
        assert!(filter.is_match("src/lib.rs"));
        assert!(filter.is_match("LICENSE"));
        assert!(!filter.is_match("docs/LICENSE"));
        assert!(!filter.is_match("README.md"), "quality's guard, not rust's");
    }

    #[test]
    fn a_file_without_the_guard_is_an_error_naming_it() {
        let error = parse("jobs:\n  changes:\n").unwrap_err();
        assert!(error.starts_with(CI), "{error}");
    }

    /// Runs against the checkout: cargo sets a test's cwd to `x/`. The two
    /// anchors are the ones `.github/scripts/ci-filter.test.mjs` pins, so
    /// both readers of ci.yml agree on which guard is the rust one.
    #[test]
    fn the_repository_filter_is_the_rust_one() {
        let filter = rust(Path::new("..")).unwrap();
        assert!(filter.is_match("src/lib.rs"));
        assert!(filter.is_match("tests/golden/a.mgdc"));
        assert!(!filter.is_match("agents/README.md"));
        assert!(!filter.is_match(".github/scripts/x.test.mjs"));
    }
}
