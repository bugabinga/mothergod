//! The paths CI runs its jobs for, read from ci.yml.

use std::path::Path;

use regex::Regex;

/// Where ci.yml's `changes` job keeps the filter.
const CI: &str = ".github/workflows/ci.yml";

/// The `rust` filter of ci.yml's `changes` job: a PR runs `test` and `doc`
/// only when a changed path matches it. Read from the file, never copied,
/// so a scoped `cargo x check` skips exactly what CI skips (#916).
pub(crate) fn rust(root: &Path) -> Result<Regex, String> {
    read(root, "rust")
}

/// The `quality` filter of the same job, which gates `fmt` and `clippy`.
/// Only a test reads it, to hold `files::clippy_input` to it.
#[cfg(test)]
pub(crate) fn quality(root: &Path) -> Result<Regex, String> {
    read(root, "quality")
}

fn read(root: &Path, output: &str) -> Result<Regex, String> {
    let ci = std::fs::read_to_string(root.join(CI))
        .map_err(|error| format!("cannot read {CI}: {error}"))?;
    parse(&ci, output)
}

/// The pattern is the one `grep -qE` guard followed by `echo "<output>=true"`,
/// the line `.github/scripts/ci-filter.test.mjs` extracts the same way for
/// `rust` to hold it to what `tests/claims.rs` reads. grep tests one path per
/// line, so `^` and `$` anchor a whole path; the regex is matched against one
/// path at a time to keep that meaning.
fn parse(ci: &str, output: &str) -> Result<Regex, String> {
    const GUARD: &str = "grep -qE '";
    let sets = format!("echo \"{output}=true\"");
    let lines: Vec<&str> = ci.lines().collect();
    let pattern = lines
        .iter()
        .zip(lines.iter().skip(1))
        .find_map(|(line, next)| {
            let start = line.find(GUARD)? + GUARD.len();
            next.contains(&sets).then(|| &line[start..])
        })
        .ok_or_else(|| format!("{CI} has no `grep -qE` guard followed by `{sets}`"))?;
    let end = pattern
        .find('\'')
        .ok_or_else(|| format!("{CI}: the {output} guard's pattern is unterminated"))?;
    Regex::new(&pattern[..end]).map_err(|error| {
        format!("{CI}: the {output} filter is not an expression x can read: {error}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files;

    #[test]
    fn reads_the_guard_that_sets_rust_true() {
        let ci = "          if echo \"$files\" | grep -qE '\\.md$'; then\n            echo \"quality=true\" >> \"$GITHUB_OUTPUT\"\n          fi\n          if echo \"$files\" | grep -qE '\\.rs$|^LICENSE$'; then\n            echo \"rust=true\" >> \"$GITHUB_OUTPUT\"\n";
        let filter = parse(ci, "rust").unwrap();
        assert!(filter.is_match("src/lib.rs"));
        assert!(filter.is_match("LICENSE"));
        assert!(!filter.is_match("docs/LICENSE"));
        assert!(!filter.is_match("README.md"), "quality's guard, not rust's");
        assert!(parse(ci, "quality").unwrap().is_match("README.md"));
    }

    #[test]
    fn a_file_without_the_guard_is_an_error_naming_it() {
        let error = parse("jobs:\n  changes:\n", "rust").unwrap_err();
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

    /// Runs against the checkout. A path `lint` widens to Clippy must be
    /// one CI's `clippy` job runs for, or a scoped check would lint what
    /// CI would not; the other direction is the point of `clippy_input`'s
    /// doc, since `quality` also holds files no Clippy run reads.
    #[test]
    fn every_clippy_input_is_under_the_quality_filter() {
        let quality = quality(Path::new("..")).unwrap();
        for path in [
            "Cargo.toml",
            "x/Cargo.toml",
            "Cargo.lock",
            "x/Cargo.lock",
            "clippy.toml",
            "bench/clippy.toml",
            "rustfmt.toml",
            "rust-toolchain.toml",
            ".cargo/config.toml",
            ".github/workflows/ci.yml",
            ".github/actions/rust-ci/check.sh",
        ] {
            assert!(files::clippy_input(Path::new(path)), "{path}");
            assert!(quality.is_match(path), "{path} left the quality filter");
        }
        for path in ["fuzz/Cargo.lock", "fuzz/rust-toolchain.toml", "README.md"] {
            assert!(!files::clippy_input(Path::new(path)), "{path}");
        }
    }
}
