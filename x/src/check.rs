use std::path::{Path, PathBuf};

use crate::files::{self, Selection, TaskKind};
use crate::{doc, filter, format, lint, test};

/// Runs the quality gate: fmt --check, lint, test, doc, in order; the whole
/// of it without paths, with paths the stages that own them. Stops at the
/// first failing stage and names the command to re-run just that one.
pub(crate) fn run(paths: &[PathBuf]) -> Result<bool, String> {
    let plan = Plan::new(paths)?;
    let mut passed = Vec::new();
    let mut skipped = Vec::new();

    let fmt = plan.selection(TaskKind::Format)?;
    if fmt.files.is_empty() {
        skipped.push("fmt (nothing to format)");
    } else {
        if format::run(&fmt, true)? {
            return Ok(fail("fmt", &format!("cargo x fmt --check{}", plan.scope())));
        }
        passed.push("fmt");
    }
    let lint = plan.selection(TaskKind::Lint)?;
    if lint.files.is_empty() {
        skipped.push("lint (nothing to lint)");
    } else {
        if lint::run(&lint, false)? {
            return Ok(fail("lint", &format!("cargo x lint{}", plan.scope())));
        }
        passed.push("lint");
    }
    if plan.crate_stages() {
        if test::run(plan.root())? {
            return Ok(fail("test", "cargo x test"));
        }
        passed.push("test");
        if doc::run(plan.root())? {
            return Ok(fail("doc", "cargo x doc"));
        }
        passed.push("doc");
    } else {
        skipped.push("test and doc (no path matches ci.yml's rust filter)");
    }

    if skipped.is_empty() {
        println!("check: {} stages passed", passed.len());
    } else {
        println!(
            "check: {} passed; skipped {}",
            passed.join(", "),
            skipped.join(", ")
        );
    }
    Ok(false)
}

/// What one check runs.
enum Plan {
    /// Every stage, each selecting at its turn as it would alone, so a
    /// stage's finding comes before the next stage's selection error.
    Whole { root: PathBuf },
    /// The stages the named paths reach, from one resolution of the paths.
    Scoped {
        root: PathBuf,
        fmt: Vec<PathBuf>,
        lint: Vec<PathBuf>,
        /// `test` and `doc` are not file-scoped: they run when a path is
        /// one a PR must touch for CI to run them, ci.yml's `rust` filter
        /// (#916).
        crate_stages: bool,
        /// ` -- <paths>` as given, for a file-scoped stage's re-run line.
        scope: String,
    },
}

/// Why a named file runs no stage of a scoped check.
const UNOWNED: &str =
    "not formatted, not linted and outside ci.yml's rust filter, so no stage of check runs for it";

impl Plan {
    fn new(paths: &[PathBuf]) -> Result<Self, String> {
        if paths.is_empty() {
            let cwd = std::env::current_dir()
                .map_err(|error| format!("cannot read the current directory: {error}"))?;
            return Ok(Self::Whole {
                root: files::repository_root(&cwd)?,
            });
        }

        let resolved = files::resolve(paths)?;
        let rust = filter::rust(&resolved.root)?;
        let matches = |path: &Path| rust.is_match(&path.to_string_lossy());
        let (fmt, _) = resolved.owned(TaskKind::Format);
        let (lint, _) = resolved.owned(TaskKind::Lint);
        let crate_stages = resolved.files().any(matches);
        let unowned: Vec<&PathBuf> = resolved
            .named()
            .iter()
            .filter(|path| !fmt.contains(path) && !lint.contains(path) && !matches(path))
            .collect();

        if fmt.is_empty() && lint.is_empty() && !crate_stages {
            // As `select`: the first named file's advice, else the scope's.
            return Err(match unowned.first() {
                Some(path) => format!("{}: {UNOWNED}", path.display()),
                None => "no files `cargo x check` can check were found in selected paths".into(),
            });
        }
        for path in unowned {
            eprintln!("skipped {}: {UNOWNED}", path.display());
        }

        let scope = paths.iter().fold(String::from(" --"), |scope, path| {
            format!("{scope} {}", path.display())
        });
        Ok(Self::Scoped {
            root: resolved.root,
            fmt,
            lint,
            crate_stages,
            scope,
        })
    }

    fn root(&self) -> &Path {
        match self {
            Self::Whole { root } | Self::Scoped { root, .. } => root,
        }
    }

    fn selection(&self, task: TaskKind) -> Result<Selection, String> {
        match self {
            Self::Whole { .. } => files::select(&[], task),
            Self::Scoped {
                root, fmt, lint, ..
            } => Ok(Selection {
                root: root.clone(),
                files: match task {
                    TaskKind::Format => fmt.clone(),
                    TaskKind::Lint => lint.clone(),
                },
            }),
        }
    }

    fn crate_stages(&self) -> bool {
        match self {
            Self::Whole { .. } => true,
            Self::Scoped { crate_stages, .. } => *crate_stages,
        }
    }

    fn scope(&self) -> &str {
        match self {
            Self::Whole { .. } => "",
            Self::Scoped { scope, .. } => scope,
        }
    }
}

fn fail(stage: &str, next: &str) -> bool {
    eprintln!("check: {stage} stage failed");
    eprintln!("  next: {next}");
    true
}
