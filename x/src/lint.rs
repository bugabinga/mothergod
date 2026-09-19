use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rumdl_lib::config::{Config, MarkdownFlavor};
use rumdl_lib::rule::{LintWarning, Rule};
use rumdl_lib::rules::{all_rules, opt_in_rules};
use rumdl_lib::utils::fix_utils::apply_warning_fixes;

use crate::files::{FileKind, Selection, kind};
use crate::format::replace;

const DISABLED_MARKDOWN_RULES: &[&str] = &[
    "MD004", "MD013", "MD018", "MD024", "MD032", "MD033", "MD034", "MD040", "MD041", "MD057",
    "MD076",
];

// GitHub rejects a release body over 125,000 characters. Warn at 80% of that
// so `## [Unreleased]` gets flagged before a cut release would actually fail
// (issue #607: it once reached 87,159, 70% of the limit, unnoticed).
const CHANGELOG_UNRELEASED_WARN_CHARS: usize = 100_000;

pub(crate) fn run(selection: &Selection, fix: bool) -> Result<bool, String> {
    let mut findings = 0;
    let mut fixed_files = 0;
    let rust_files = selection
        .files
        .iter()
        .filter(|path| kind(path) == Some(FileKind::Rust))
        .cloned()
        .collect::<Vec<_>>();
    let markdown_files = selection
        .files
        .iter()
        .filter(|path| kind(path) == Some(FileKind::Markdown))
        .collect::<Vec<_>>();
    let rules = if markdown_files.is_empty() {
        Vec::new()
    } else {
        markdown_rules()
    };
    let config = Config::default();

    for relative in markdown_files {
        let path = selection.root.join(relative);
        let mut source = fs::read_to_string(&path)
            .map_err(|error| format!("{}: cannot read UTF-8 text: {error}", relative.display()))?;
        let mut warnings = lint_markdown(&source, relative, &rules, &config)?;

        if fix && warnings.iter().any(|warning| warning.fix.is_some()) {
            let fixed = apply_warning_fixes(&source, &warnings).map_err(|error| {
                format!(
                    "{}: cannot apply Markdown fixes: {error}",
                    relative.display()
                )
            })?;
            if fixed != source {
                replace(&path, fixed.as_bytes())?;
                source = fixed;
                fixed_files += 1;
                warnings = lint_markdown(&source, relative, &rules, &config)?;
            }
        }

        findings += warnings.len();
        report_markdown(relative, &warnings, fix);

        if relative.file_name().and_then(|name| name.to_str()) == Some("CHANGELOG.md") {
            let check = check_changelog(relative, &source);
            findings += check.findings.len();
            for finding in &check.findings {
                eprintln!("{finding}");
            }
            if let Some(warning) = &check.unreleased_size_warning {
                eprintln!("{warning}");
            }
        }
    }

    if !rust_files.is_empty() && !run_clippy(&selection.root, &rust_files)? {
        findings += 1;
        eprintln!("Rust linting failed");
        eprintln!(
            "  next: address the diagnostics, then rerun `cargo x lint -- {}`",
            display_paths(&rust_files)
        );
    }

    if findings == 0 {
        if fixed_files == 0 {
            println!("lint: {} files checked", selection.files.len());
        } else {
            println!("lint: {fixed_files} files fixed; all checks pass");
        }
    } else {
        eprintln!("lint: {findings} finding(s)");
    }

    Ok(findings > 0)
}

fn markdown_rules() -> Vec<Box<dyn Rule>> {
    let config = Config::default();
    let opt_in = opt_in_rules();
    all_rules(&config)
        .into_iter()
        .filter(|rule| {
            !opt_in.contains(rule.name()) && !DISABLED_MARKDOWN_RULES.contains(&rule.name())
        })
        .collect()
}

fn lint_markdown(
    source: &str,
    path: &Path,
    rules: &[Box<dyn Rule>],
    config: &Config,
) -> Result<Vec<LintWarning>, String> {
    rumdl_lib::lint(
        source,
        rules,
        false,
        MarkdownFlavor::Standard,
        Some(path.to_path_buf()),
        Some(config),
    )
    .map_err(|error| format!("{}: Markdown lint failed: {error}", path.display()))
}

fn report_markdown(path: &Path, warnings: &[LintWarning], fixing: bool) {
    for warning in warnings {
        let rule = warning.rule_name.as_deref().unwrap_or("Markdown");
        eprintln!(
            "{}:{}:{}: {rule}: {}",
            path.display(),
            warning.line,
            warning.column,
            warning.message
        );
    }
    if !fixing && warnings.iter().any(|warning| warning.fix.is_some()) {
        eprintln!("  fix available: cargo x lint --fix -- {}", path.display());
    }
}

struct ChangelogCheck {
    findings: Vec<String>,
    unreleased_size_warning: Option<String>,
}

/// Rejects a `### X` heading repeated under the same `## [...]` release
/// (the drift #602 fixed by hand, PR #606) and warns, without failing, when
/// `## [Unreleased]`'s body approaches GitHub's release-body size limit.
fn check_changelog(relative: &Path, source: &str) -> ChangelogCheck {
    let mut findings = Vec::new();
    let mut release: Option<&str> = None;
    let mut seen_subheadings: BTreeSet<&str> = BTreeSet::new();
    let mut in_unreleased = false;
    let mut unreleased_chars = 0usize;

    for (index, line) in source.lines().enumerate() {
        if let Some(heading) = line.strip_prefix("## ") {
            let heading = heading.trim();
            release = Some(heading);
            seen_subheadings.clear();
            in_unreleased = heading.eq_ignore_ascii_case("[Unreleased]");
            continue;
        }
        if let Some(heading) = line.strip_prefix("### ") {
            let heading = heading.trim();
            if !seen_subheadings.insert(heading) {
                findings.push(format!(
                    "{}:{}: duplicate `### {heading}` heading under `## {}`",
                    relative.display(),
                    index + 1,
                    release.unwrap_or("(no release heading)"),
                ));
            }
            continue;
        }
        if in_unreleased {
            unreleased_chars += line.len() + 1;
        }
    }

    let unreleased_size_warning = (unreleased_chars > CHANGELOG_UNRELEASED_WARN_CHARS).then(|| {
        format!(
            "{}: `## [Unreleased]` body is {unreleased_chars} characters, over the \
             {CHANGELOG_UNRELEASED_WARN_CHARS}-character warning ceiling (GitHub's release-body \
             limit is 125,000); curate it before the next release cut",
            relative.display(),
        )
    });

    ChangelogCheck {
        findings,
        unreleased_size_warning,
    }
}

fn run_clippy(root: &Path, files: &[PathBuf]) -> Result<bool, String> {
    let mut core = false;
    let mut bench = false;
    let mut x = false;
    for path in files {
        if path.starts_with("x") {
            x = true;
        } else if path.starts_with("bench") {
            bench = true;
        } else {
            core = true;
        }
    }

    let mut success = true;
    if core || bench {
        let mut command = Command::new("cargo");
        command.args(["clippy", "--quiet"]).current_dir(root);
        match (core, bench) {
            (true, true) => {}
            (true, false) => {
                command.args(["--package", "mothergod"]);
            }
            (false, true) => {
                command.args(["--package", "mothergod-bench"]);
            }
            (false, false) => unreachable!(),
        }
        success &= clippy_status(command)?;
    }
    if x {
        let mut command = Command::new("cargo");
        command
            .args(["clippy", "--quiet", "--manifest-path", "x/Cargo.toml"])
            .current_dir(root);
        success &= clippy_status(command)?;
    }
    Ok(success)
}

fn clippy_status(mut command: Command) -> Result<bool, String> {
    command.args(["--all-targets", "--", "--deny", "warnings"]);
    command
        .status()
        .map(|status| status.success())
        .map_err(|error| {
            format!("cannot run Clippy: {error}; install it with `rustup component add clippy`")
        })
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_profile_disables_known_false_positives_but_keeps_actionable_rules() {
        let rules = markdown_rules();
        let config = Config::default();
        let accepted = format!("<p>lead</p>\n\n#197\n\n{}\n", "x".repeat(200));
        assert!(
            lint_markdown(&accepted, Path::new("a.md"), &rules, &config)
                .unwrap()
                .is_empty()
        );

        let warnings = lint_markdown("# Title \n", Path::new("a.md"), &rules, &config).unwrap();
        assert!(
            warnings
                .iter()
                .any(|warning| warning.rule_name.as_deref() == Some("MD009"))
        );
        assert!(warnings.iter().any(|warning| warning.fix.is_some()));
    }

    #[test]
    fn changelog_rejects_a_duplicate_subheading_within_one_release() {
        let source =
            "## [Unreleased]\n\n### Added\n\n- a\n\n### Fixed\n\n- b\n\n### Added\n\n- c\n";
        let check = check_changelog(Path::new("CHANGELOG.md"), source);
        assert_eq!(check.findings.len(), 1);
        assert!(
            check.findings[0].contains("duplicate `### Added` heading under `## [Unreleased]`")
        );
        assert!(check.unreleased_size_warning.is_none());
    }

    #[test]
    fn changelog_allows_the_same_subheading_reused_across_releases() {
        let source =
            "## [Unreleased]\n\n### Added\n\n- a\n\n## [0.1.0] - 2026-01-01\n\n### Added\n\n- b\n";
        let check = check_changelog(Path::new("CHANGELOG.md"), source);
        assert!(check.findings.is_empty());
    }

    #[test]
    fn changelog_warns_but_does_not_fail_past_the_unreleased_size_ceiling() {
        let body = "x".repeat(CHANGELOG_UNRELEASED_WARN_CHARS + 1);
        let source = format!("## [Unreleased]\n\n### Added\n\n{body}\n");
        let check = check_changelog(Path::new("CHANGELOG.md"), &source);
        assert!(check.findings.is_empty());
        assert!(
            check
                .unreleased_size_warning
                .as_deref()
                .is_some_and(
                    |warning| warning.contains("over the") && warning.contains("warning ceiling")
                )
        );
    }

    #[test]
    fn changelog_stays_quiet_under_the_unreleased_size_ceiling() {
        let source = "## [Unreleased]\n\n### Added\n\n- a\n";
        let check = check_changelog(Path::new("CHANGELOG.md"), source);
        assert!(check.unreleased_size_warning.is_none());
    }
}
