use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileKind {
    Rust,
    Markdown,
    Json,
    JsonLines,
    Toml,
    Yaml,
    Script,
    Markup,
}

#[derive(Clone, Copy)]
pub(crate) enum TaskKind {
    Format,
    Lint,
}

pub(crate) struct Selection {
    pub(crate) root: PathBuf,
    pub(crate) files: Vec<PathBuf>,
}

/// The repository files a path list names, resolved once so one task, or
/// `check` routing to several, can ask what each owns: a named file is
/// kept whether or not a task owns it, so the caller can say why it was
/// skipped; a directory contributes its tracked and non-ignored untracked
/// regular files; no paths means the whole repository.
pub(crate) struct Resolved {
    pub(crate) root: PathBuf,
    named: Vec<PathBuf>,
    found: Vec<PathBuf>,
}

pub(crate) fn resolve(paths: &[PathBuf]) -> Result<Resolved, String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("cannot read the current directory: {error}"))?;
    let root = repository_root(&cwd)?;
    let mut named = Vec::new();
    let mut found = Vec::new();

    if paths.is_empty() {
        found.extend(
            git_files(&root, None)?
                .into_iter()
                .filter(|candidate| regular_file(&root, candidate) && !excluded(candidate)),
        );
    }

    for path in paths {
        let absolute = if path.is_absolute() {
            path.clone()
        } else {
            cwd.join(path)
        };
        let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
            format!(
                "{}: path does not exist or cannot be read: {error}",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{}: symbolic links are not rewritten by x",
                path.display()
            ));
        }
        let absolute = absolute.canonicalize().map_err(|error| {
            format!(
                "{}: path does not exist or cannot be read: {error}",
                path.display()
            )
        })?;
        let relative = absolute.strip_prefix(&root).map_err(|_| {
            format!(
                "{}: path is outside repository {}",
                path.display(),
                root.display()
            )
        })?;

        if excluded(relative) {
            return Err(format!(
                "{}: archived imports are immutable and excluded from x",
                relative.display()
            ));
        }

        if absolute.is_file() {
            named.push(relative.to_path_buf());
        } else if absolute.is_dir() {
            found.extend(
                git_files(&root, Some(relative))?
                    .into_iter()
                    .filter(|candidate| regular_file(&root, candidate) && !excluded(candidate)),
            );
        } else {
            return Err(format!("{}: expected a file or directory", path.display()));
        }
    }

    Ok(Resolved { root, named, found })
}

impl Resolved {
    /// The files `task` owns, sorted without duplicates, and the named
    /// files it does not, each with the advice naming the command that can.
    pub(crate) fn owned(&self, task: TaskKind) -> (Vec<PathBuf>, Vec<String>) {
        let mut files = BTreeSet::new();
        let mut skipped = Vec::new();
        for path in &self.named {
            match unsupported(path, task) {
                Some(advice) => skipped.push(format!("{}: {advice}", path.display())),
                None => {
                    files.insert(path.clone());
                }
            }
        }
        files.extend(
            self.found
                .iter()
                .filter(|path| supports(path, task))
                .cloned(),
        );
        (files.into_iter().collect(), skipped)
    }

    /// The files named on the command line, repository-relative.
    pub(crate) fn named(&self) -> &[PathBuf] {
        &self.named
    }

    /// Every resolved file, named and found.
    pub(crate) fn files(&self) -> impl Iterator<Item = &Path> {
        self.named.iter().chain(&self.found).map(PathBuf::as_path)
    }
}

pub(crate) fn select(paths: &[PathBuf], task: TaskKind) -> Result<Selection, String> {
    let resolved = resolve(paths)?;
    let (files, skipped) = resolved.owned(task);

    if files.is_empty() {
        // A list where nothing was taken is an error carrying the first
        // path's advice; one where something was only notes the rest, so a
        // mixed list of changed files never dies at the first script it
        // does not own (#916).
        if let Some(reason) = skipped.first() {
            return Err(reason.clone());
        }
        let scope = if paths.is_empty() {
            "repository"
        } else {
            "selected paths"
        };
        return Err(format!(
            "no files supported by `cargo x {}` were found in {scope}",
            task.name()
        ));
    }

    for reason in skipped {
        eprintln!("skipped {reason}");
    }

    Ok(Selection {
        root: resolved.root,
        files,
    })
}

pub(crate) fn kind(path: &Path) -> Option<FileKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "rs" => Some(FileKind::Rust),
        "md" | "markdown" => Some(FileKind::Markdown),
        "json" => Some(FileKind::Json),
        "jsonl" => Some(FileKind::JsonLines),
        "toml" => Some(FileKind::Toml),
        "yaml" | "yml" => Some(FileKind::Yaml),
        "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" => Some(FileKind::Script),
        "html" | "svg" => Some(FileKind::Markup),
        _ => None,
    }
}

pub(crate) fn repository_root(cwd: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err("not inside a Git repository; run x from the mothergod checkout".into());
    }
    let root = String::from_utf8(output.stdout)
        .map_err(|_| "git returned a non-UTF-8 repository path".to_string())?;
    Ok(PathBuf::from(root.trim()))
}

fn git_files(root: &Path, path: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    let mut command = Command::new("git");
    command
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root);
    if let Some(path) = path {
        command.arg("--").arg(if path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            path
        });
    }

    let output = command
        .output()
        .map_err(|error| format!("cannot enumerate repository files: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git could not enumerate repository files: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map(PathBuf::from)
                .map_err(|_| "x requires UTF-8 repository paths".to_string())
        })
        .collect()
}

pub(crate) fn supports(path: &Path, task: TaskKind) -> bool {
    match task {
        TaskKind::Format => kind(path).is_some_and(|kind| kind != FileKind::Markdown),
        TaskKind::Lint => clippy_input(path) || lints_by_kind(path),
    }
}

fn lints_by_kind(path: &Path) -> bool {
    kind(path).is_some_and(|kind| {
        matches!(kind, FileKind::Rust | FileKind::Markdown)
            && !(kind == FileKind::Rust && path.starts_with("fuzz"))
            || kind == FileKind::Yaml && path.starts_with(".github")
    })
}

/// A file that is not Rust source but changes what Clippy sees: manifests,
/// the lockfile, lint and toolchain configuration, and the CI definition of
/// how Clippy runs. Clippy's input is the whole package set, so `lint`
/// widens any of these to every package. These are the non-extension
/// alternatives of ci.yml's `quality` filter that gate the `clippy` job
/// (held to it by `filter::tests::every_clippy_input_is_under_the_quality_filter`),
/// plus `clippy.toml` at any depth because `bench/clippy.toml` configures
/// the bench package. The nightly `fuzz/` workspace is outside `lint`.
pub(crate) fn clippy_input(path: &Path) -> bool {
    if path.starts_with("fuzz") {
        return false;
    }
    let name = path.file_name().and_then(|name| name.to_str());
    matches!(
        name,
        Some("Cargo.toml" | "Cargo.lock" | "clippy.toml" | "rustfmt.toml")
    ) || name.is_some_and(|name| name.starts_with("rust-toolchain"))
        || path.starts_with(".cargo")
        || path == Path::new(".github/workflows/ci.yml")
        || path.starts_with(".github/actions/rust-ci")
}

/// Why `task` cannot take `path`, naming the command that can, or `None`
/// when it can.
fn unsupported(path: &Path, task: TaskKind) -> Option<&'static str> {
    if supports(path, task) {
        return None;
    }

    Some(match (task, kind(path)) {
        (TaskKind::Format, Some(FileKind::Markdown)) => {
            "Markdown is linted, not formatted; use `cargo x lint -- PATH`"
        }
        (TaskKind::Lint, Some(FileKind::Rust)) if path.starts_with("fuzz") => {
            "fuzz targets require their pinned nightly toolchain and are outside x lint"
        }
        (TaskKind::Lint, Some(_)) => {
            "this file type has no embedded linter; use `cargo x fmt --check -- PATH` for syntax and format validation"
        }
        _ => "this file type is not supported by x",
    })
}

fn regular_file(root: &Path, path: &Path) -> bool {
    fs::symlink_metadata(root.join(path)).is_ok_and(|metadata| metadata.file_type().is_file())
}

fn excluded(path: &Path) -> bool {
    path.starts_with("research/imports")
}

impl TaskKind {
    fn name(self) -> &'static str {
        match self {
            Self::Format => "fmt",
            Self::Lint => "lint",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_files() {
        assert_eq!(kind(Path::new("src/lib.rs")), Some(FileKind::Rust));
        assert_eq!(kind(Path::new("Cargo.lock")), None);
        assert_eq!(kind(Path::new("a.jsonl")), Some(FileKind::JsonLines));
        assert_eq!(kind(Path::new("logo.svg")), Some(FileKind::Markup));
        assert_eq!(kind(Path::new("script.py")), None);
    }

    #[test]
    fn keeps_task_boundaries_explicit() {
        assert!(supports(Path::new("README.md"), TaskKind::Lint));
        assert!(!supports(Path::new("README.md"), TaskKind::Format));
        assert!(supports(Path::new("src/lib.rs"), TaskKind::Format));
        assert!(!supports(
            Path::new("fuzz/fuzz_targets/a.rs"),
            TaskKind::Lint
        ));
        assert!(!supports(Path::new("site/index.html"), TaskKind::Lint));
        assert!(supports(
            Path::new(".github/workflows/ci.yml"),
            TaskKind::Lint
        ));
        assert!(supports(
            Path::new(".github/actions/rust-ci/action.yml"),
            TaskKind::Lint
        ));
        assert!(!supports(Path::new("infra/wrangler.yml"), TaskKind::Lint));
    }

    #[test]
    fn explains_why_a_task_cannot_take_a_path() {
        assert_eq!(unsupported(Path::new("src/lib.rs"), TaskKind::Format), None);
        assert!(
            unsupported(Path::new("README.md"), TaskKind::Format)
                .is_some_and(|advice| advice.contains("cargo x lint"))
        );
    }

    /// Runs against the checkout: cargo sets a test's cwd to `x/`, and
    /// `README.md` and `Cargo.toml` are the root's.
    #[test]
    fn an_explicit_list_skips_unowned_files_unless_nothing_remains() {
        let mixed = select(
            &[
                PathBuf::from("../README.md"),
                PathBuf::from("../Cargo.toml"),
            ],
            TaskKind::Format,
        )
        .map(|selection| selection.files)
        .unwrap();
        assert_eq!(mixed, vec![PathBuf::from("Cargo.toml")]);

        let only = select(&[PathBuf::from("../README.md")], TaskKind::Format)
            .err()
            .expect("a list where nothing was taken is an error");
        assert!(only.starts_with("README.md: Markdown is linted"), "{only}");
    }
}
