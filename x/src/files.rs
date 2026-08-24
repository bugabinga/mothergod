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

pub(crate) fn select(paths: &[PathBuf], task: TaskKind) -> Result<Selection, String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("cannot read the current directory: {error}"))?;
    let root = repository_root(&cwd)?;
    let explicit = !paths.is_empty();
    let mut files = BTreeSet::new();

    if explicit {
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
                ensure_supported(relative, task)?;
                files.insert(relative.to_path_buf());
            } else if absolute.is_dir() {
                for candidate in git_files(&root, Some(relative))? {
                    if regular_file(&root, &candidate)
                        && !excluded(&candidate)
                        && supports(&candidate, task)
                    {
                        files.insert(candidate);
                    }
                }
            } else {
                return Err(format!("{}: expected a file or directory", path.display()));
            }
        }
    } else {
        for candidate in git_files(&root, None)? {
            if regular_file(&root, &candidate)
                && !excluded(&candidate)
                && supports(&candidate, task)
            {
                files.insert(candidate);
            }
        }
    }

    if files.is_empty() {
        let scope = if explicit {
            "selected paths"
        } else {
            "repository"
        };
        return Err(format!(
            "no files supported by `cargo x {}` were found in {scope}",
            task.name()
        ));
    }

    Ok(Selection {
        root,
        files: files.into_iter().collect(),
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

fn repository_root(cwd: &Path) -> Result<PathBuf, String> {
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

fn supports(path: &Path, task: TaskKind) -> bool {
    kind(path).is_some_and(|kind| match task {
        TaskKind::Format => kind != FileKind::Markdown,
        TaskKind::Lint => {
            matches!(kind, FileKind::Rust | FileKind::Markdown)
                && !(kind == FileKind::Rust && path.starts_with("fuzz"))
        }
    })
}

fn ensure_supported(path: &Path, task: TaskKind) -> Result<(), String> {
    if supports(path, task) {
        return Ok(());
    }

    let advice = match (task, kind(path)) {
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
    };
    Err(format!("{}: {advice}", path.display()))
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
    }
}
