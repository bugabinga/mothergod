use std::path::Path;

use crate::files::{self, TaskKind};
use crate::{doc, format, lint, test};

/// Runs the whole quality gate: fmt --check, lint, test, doc, unscoped, in
/// order. Stops at the first failing stage and names the command to re-run
/// just that one.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    if format::run(&files::select(&[], TaskKind::Format)?, true)? {
        return Ok(fail("fmt", "cargo x fmt --check"));
    }
    if lint::run(&files::select(&[], TaskKind::Lint)?, false)? {
        return Ok(fail("lint", "cargo x lint"));
    }
    if test::run(root)? {
        return Ok(fail("test", "cargo x test"));
    }
    if doc::run(root)? {
        return Ok(fail("doc", "cargo x doc"));
    }
    println!("check: 4 stages passed");
    Ok(false)
}

fn fail(stage: &str, next: &str) -> bool {
    eprintln!("check: {stage} stage failed");
    eprintln!("  next: {next}");
    true
}
