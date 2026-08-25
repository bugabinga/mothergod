//! Regenerates `site/status-data.json` (`mothergod_site_status`, issue
//! #95) from this checkout: ROADMAP.md's milestone checkboxes,
//! `research/progress.jsonl`'s experiment ledger, `git log` for the last
//! 7 days' merged-PR-shaped commits, and `src/**/*.rs` line/test counts.
//! Not yet wired into a scheduled workflow (`research/JOURNAL.md`'s S2-D1
//! "progress-graph rendering" line, [`mothergod_site_status`]'s module
//! docs); run by hand until it is: `cargo run -p mothergod-site-status
//! --release --bin generate`.

use mothergod_site_status::{StatusData, experiments, render, roadmap};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The workspace root, located relative to this crate's manifest so the
/// result is correct regardless of the caller's working directory.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("site-status/Cargo.toml has a parent directory (the workspace root)")
        .to_path_buf()
}

/// Runs `cmd args` in `cwd` and returns its trimmed stdout.
///
/// # Panics
///
/// Panics if the command cannot be spawned, exits non-zero, or writes
/// non-UTF-8 stdout. This binary is generation tooling run by hand in a
/// known checkout, not the codec's decoder (CLAUDE.md hard rule 2 does
/// not apply here): a loud failure beats a silently wrong snapshot.
fn run(cmd: &str, args: &[&str], cwd: &Path) -> String {
    let output = Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {cmd} {args:?}: {err}"));
    assert!(
        output.status.success(),
        "{cmd} {args:?} exited {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap_or_else(|err| panic!("{cmd} {args:?} produced non-UTF-8 stdout: {err}"))
        .trim()
        .to_string()
}

/// Collects every `.rs` file under `dir`, recursively. Silently skips a
/// directory this process cannot read rather than panicking: the caller
/// only ever passes `src/`, which always exists in this checkout.
fn rs_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files_recursive(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// True if `subject` (one `git log --pretty=%s` line) ends in this repo's
/// squash-merge convention, `... (#<digits>)` (visible in `git log
/// --oneline`, e.g. `"docs: ... (#243)"`) — the merged-PR proxy
/// [`main`] counts over the last 7 days.
fn is_merge_commit_subject(subject: &str) -> bool {
    let subject = subject.trim();
    let Some(paren) = subject.rfind(" (#") else {
        return false;
    };
    let Some(rest) = subject[paren + 3..].strip_suffix(')') else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

/// Counts entries in `dates` (`"YYYY-MM-DD"` strings) at or after
/// `cutoff` — a lexical comparison, valid because ISO 8601 dates sort
/// lexically in calendar order.
fn count_since<'a>(dates: impl Iterator<Item = &'a str>, cutoff: &str) -> usize {
    dates.filter(|date| *date >= cutoff).count()
}

/// Lines in `text`, the SLOC proxy for one file.
fn count_lines(text: &str) -> usize {
    text.lines().count()
}

/// `#[test]` attribute lines in `text`.
fn count_test_attributes(text: &str) -> usize {
    text.lines().filter(|line| line.trim() == "#[test]").count()
}

fn main() -> ExitCode {
    let root = repo_root();

    let roadmap_text = std::fs::read_to_string(root.join("ROADMAP.md"))
        .unwrap_or_else(|err| panic!("failed to read ROADMAP.md: {err}"));
    let milestones = roadmap::parse_milestones(&roadmap_text);

    let progress_text = std::fs::read_to_string(root.join("research/progress.jsonl"))
        .unwrap_or_else(|err| panic!("failed to read research/progress.jsonl: {err}"));
    let parsed_experiments = experiments::parse_progress(&progress_text);
    let experiment_stats = experiments::stats(&parsed_experiments);

    let generated_at = run("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"], &root);
    let cutoff_date = run("date", &["-u", "-d", "7 days ago", "+%Y-%m-%d"], &root);

    let commit_subjects = run("git", &["log", "--since=7 days ago", "--pretty=%s"], &root);
    let merged_prs_7d = commit_subjects
        .lines()
        .filter(|subject| is_merge_commit_subject(subject))
        .count();

    let recorded_experiments_7d = count_since(
        parsed_experiments.iter().map(|e| e.date.as_str()),
        &cutoff_date,
    );

    let mut rs_files = Vec::new();
    rs_files_recursive(&root.join("src"), &mut rs_files);
    let mut sloc_src = 0usize;
    let mut test_functions_src = 0usize;
    for path in &rs_files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        sloc_src += count_lines(&text);
        test_functions_src += count_test_attributes(&text);
    }

    let data = StatusData {
        generated_at,
        phase: "pre-alpha".to_string(),
        format_version: mothergod::FORMAT_VERSION,
        works_today: "Stored (verbatim) and Lz (optimal-parse LZ over an adaptive, \
            context-mixing range coder) frame methods, both losslessly round-tripped \
            in the adversarial-input suite."
            .to_string(),
        milestones,
        benchmarks_note: "Not yet measurable: no Silesia/Canterbury bits/byte vs \
            gzip/zstd/xz exists yet for this Rust build (ROADMAP M2's bench harness is \
            still landing, research/JOURNAL.md S2-D1). The founding research prototype \
            beat zstd -19 in a single derivation session on different code; see \
            research/JOURNAL.md S2-A17 for a dev-time spot-check, not the aggregate \
            claim the scorecard wants."
            .to_string(),
        experiments: experiment_stats,
        merged_prs_7d,
        recorded_experiments_7d,
        sloc_src,
        test_functions_src,
    };

    let text = render(&data);
    let out_path = root.join("site/status-data.json");
    if let Err(err) = std::fs::write(&out_path, &text) {
        eprintln!("failed to write {}: {err}", out_path.display());
        return ExitCode::FAILURE;
    }
    println!("wrote {}", out_path.display());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_commit_subject_matches_the_squash_convention() {
        assert!(is_merge_commit_subject(
            "docs: correct stale claim in README/site (#243)"
        ));
        assert!(is_merge_commit_subject("short (#1)"));
    }

    #[test]
    fn merge_commit_subject_rejects_non_matches() {
        assert!(!is_merge_commit_subject("no pr number here"));
        assert!(!is_merge_commit_subject("trailing text (#12) more"));
        assert!(!is_merge_commit_subject("empty number (#)"));
        assert!(!is_merge_commit_subject("not digits (#abc)"));
    }

    #[test]
    fn count_since_is_a_lexical_date_cutoff() {
        let dates = ["2026-08-18", "2026-08-20", "2026-08-25"];
        assert_eq!(count_since(dates.into_iter(), "2026-08-20"), 2);
        assert_eq!(count_since(dates.into_iter(), "2026-08-26"), 0);
    }

    #[test]
    fn count_lines_counts_newline_separated_lines() {
        assert_eq!(count_lines("a\nb\nc\n"), 3);
        assert_eq!(count_lines(""), 0);
    }

    #[test]
    fn count_test_attributes_matches_exact_attribute_lines() {
        let text = "#[test]\nfn a() {}\n    #[test]\nfn b() {}\n// #[test] in a comment\n";
        assert_eq!(count_test_attributes(text), 2);
    }
}
