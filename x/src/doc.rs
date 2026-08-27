use std::path::Path;
use std::process::Command;

/// Builds crate documentation with rustdoc warnings denied, exactly the
/// CLAUDE.md doc gate: `RUSTDOCFLAGS="--deny warnings" cargo doc --no-deps`.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    let status = Command::new("cargo")
        .args(["doc", "--no-deps"])
        .env("RUSTDOCFLAGS", "--deny warnings")
        .current_dir(root)
        .status()
        .map_err(|error| format!("cannot run `cargo doc --no-deps`: {error}"))?;

    if !status.success() {
        eprintln!("doc: documentation build failed");
        eprintln!("  next: RUSTDOCFLAGS=\"--deny warnings\" cargo doc --no-deps");
        return Ok(true);
    }

    println!("doc: built without warnings");
    Ok(false)
}
