//! Repository quality command: format, lint, test, doc, and the check gate.

mod check;
mod doc;
mod files;
mod format;
mod lint;
mod test;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use crate::files::TaskKind;

#[derive(Parser)]
#[command(
    name = "cargo x",
    bin_name = "cargo x",
    version,
    about = "The repository quality interface",
    long_about = "The repository quality interface.\n\n`cargo x check` runs the whole gate. Run `cargo x help <COMMAND>` for task-specific scope, fixes, and examples.",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Run the whole quality gate: fmt --check, lint, test, doc
    #[command(
        after_help = "Runs `cargo x fmt --check`, `cargo x lint`, `cargo x test`, then `cargo x doc`, unscoped, in order.\n\nStops at the first failing stage and names the command to re-run just that one. This is the pre-push gate; CI enforces the same commands.\n\nExamples:\n  cargo x check"
    )]
    Check,
    /// Format supported files, or verify formatting with --check
    Fmt(FormatArgs),
    /// Run Rust and Markdown linters, optionally applying safe Markdown fixes
    Lint(LintArgs),
    /// Run the fixed test plan: core, x, then doc
    #[command(
        after_help = "Runs `cargo test --all-targets`, `cargo test --manifest-path x/Cargo.toml`, then `cargo test --doc`, in order.\n\nNot file-scoped: the plan is fixed. Stops at the first failing suite and names the command to re-run just that one.\n\nExamples:\n  cargo x test"
    )]
    Test,
    /// Build documentation with rustdoc warnings denied
    #[command(
        after_help = "Runs `RUSTDOCFLAGS=\"--deny warnings\" cargo doc --no-deps`.\n\nNot file-scoped: the whole crate documents or the task fails, naming the command to re-run.\n\nExamples:\n  cargo x doc"
    )]
    Doc,
}

#[derive(Args)]
#[command(
    after_help = "Formats Rust, JSON, JSONL, TOML, YAML, JavaScript, HTML, and SVG files.\n\nWithout PATH arguments, every supported tracked file is selected. Files select exactly themselves; directories recurse through tracked and non-ignored untracked files.\n\nExamples:\n  cargo x fmt\n  cargo x fmt --check\n  cargo x fmt --check -- src x/src/main.rs\n  cargo x fmt -- site/index.html"
)]
struct FormatArgs {
    /// Report files requiring formatting without changing them
    #[arg(long)]
    check: bool,

    /// Files or directories to format
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Args)]
#[command(
    after_help = "Runs Clippy for selected Rust package scopes and rumdl for selected Markdown files.\n\nWithout PATH arguments, every supported tracked file is selected. Markdown files are checked exactly; Rust files select their containing Cargo package because Clippy works at package scope.\n\nThe repository Markdown profile is documented in x/README.md. --fix applies only fixes rumdl marks safe, then checks the result again.\n\nExamples:\n  cargo x lint\n  cargo x lint -- src\n  cargo x lint -- README.md\n  cargo x lint --fix -- README.md"
)]
struct LintArgs {
    /// Apply safe Markdown fixes before reporting remaining findings
    #[arg(long)]
    fix: bool,

    /// Files or directories to lint
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let task = Cli::parse().task;
    let result = match task {
        Task::Fmt(args) => execute(TaskKind::Format, &args.paths, |selection| {
            format::run(selection, args.check)
        }),
        Task::Lint(args) => execute(TaskKind::Lint, &args.paths, |selection| {
            lint::run(selection, args.fix)
        }),
        Task::Check => run_at_root(check::run),
        Task::Test => run_at_root(test::run),
        Task::Doc => run_at_root(doc::run),
    };

    match result {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::from(1),
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!("help: run `cargo x help` or `cargo x help <COMMAND>`");
            ExitCode::from(2)
        }
    }
}

fn execute(
    task: TaskKind,
    paths: &[PathBuf],
    run: impl FnOnce(&files::Selection) -> Result<bool, String>,
) -> Result<bool, String> {
    let selection = files::select(paths, task)?;
    run(&selection)
}

fn run_at_root(run: impl FnOnce(&Path) -> Result<bool, String>) -> Result<bool, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("cannot read the current directory: {error}"))?;
    let root = files::repository_root(&cwd)?;
    run(&root)
}
