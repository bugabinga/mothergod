use std::path::Path;
use std::process::Command;

struct Suite {
    name: &'static str,
    args: &'static [&'static str],
}

const SUITES: &[Suite] = &[
    Suite {
        name: "core",
        args: &["test", "--all-targets"],
    },
    Suite {
        name: "x",
        args: &["test", "--manifest-path", "x/Cargo.toml"],
    },
    Suite {
        name: "doc",
        args: &["test", "--doc"],
    },
];

/// Runs the fixed test plan: core, x, then doc. Stops at the first failing
/// suite and reports the command to re-run just that one.
pub(crate) fn run(root: &Path) -> Result<bool, String> {
    for suite in SUITES {
        let status = Command::new("cargo")
            .args(suite.args)
            .current_dir(root)
            .status()
            .map_err(|error| format!("cannot run `cargo {}`: {error}", suite.args.join(" ")))?;

        if !status.success() {
            eprintln!("test: {} suite failed", suite.name);
            eprintln!("  next: cargo {}", suite.args.join(" "));
            return Ok(true);
        }
    }

    println!("test: {} suites passed", SUITES.len());
    Ok(false)
}
