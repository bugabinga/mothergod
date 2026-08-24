//! End-to-end checks for the agent-facing command contract.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REPOSITORY: AtomicU64 = AtomicU64::new(0);

struct Repository(PathBuf);

impl Repository {
    fn new() -> Self {
        let id = NEXT_REPOSITORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("mothergod-x-{}-{id}", std::process::id()));
        fs::create_dir(&path).unwrap();
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&path)
            .status()
            .unwrap();
        assert!(status.success());
        Self(path)
    }

    fn write(&self, path: &str, contents: &str) {
        fs::write(self.0.join(path), contents).unwrap();
    }

    fn track(&self) {
        let status = Command::new("git")
            .args(["add", "."])
            .current_dir(&self.0)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn x(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_cargo-x"))
            .args(arguments)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn help_teaches_task_discovery_and_scope() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-x"))
        .args(["help", "fmt"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("cargo x fmt [OPTIONS] [PATH]..."));
    assert!(stdout.contains("Without PATH arguments"));
    assert!(stdout.contains("cargo x fmt --check -- src"));
}

#[test]
fn format_scope_reports_and_repairs_only_the_selected_file() {
    let repository = Repository::new();
    repository.write("a.json", "{\"a\":1}");
    repository.write("b.json", "{\"b\":2}");
    repository.track();

    let check = repository.x(&["fmt", "--check", "--", "a.json"]);
    assert_eq!(check.status.code(), Some(1));
    let stderr = String::from_utf8(check.stderr).unwrap();
    assert!(stderr.contains("a.json: needs formatting"));
    assert!(!stderr.contains("b.json"));

    let fix = repository.x(&["fmt", "--", "a.json"]);
    assert!(
        fix.status.success(),
        "{}",
        String::from_utf8_lossy(&fix.stderr)
    );
    assert_eq!(
        fs::read_to_string(repository.0.join("a.json")).unwrap(),
        "{\n  \"a\": 1\n}\n"
    );
    assert_eq!(
        fs::read_to_string(repository.0.join("b.json")).unwrap(),
        "{\"b\":2}"
    );
}

#[test]
fn lint_reports_locations_and_applies_safe_fixes() {
    let repository = Repository::new();
    repository.write("bad.md", "# Title \n");
    repository.track();

    let check = repository.x(&["lint", "--", "bad.md"]);
    assert_eq!(check.status.code(), Some(1));
    let stderr = String::from_utf8(check.stderr).unwrap();
    assert!(stderr.contains("bad.md:1:8: MD009"));
    assert!(stderr.contains("cargo x lint --fix -- bad.md"));

    let fix = repository.x(&["lint", "--fix", "--", "bad.md"]);
    assert!(
        fix.status.success(),
        "{}",
        String::from_utf8_lossy(&fix.stderr)
    );
    assert_eq!(
        fs::read_to_string(repository.0.join("bad.md")).unwrap(),
        "# Title\n"
    );
}

#[test]
fn unsupported_explicit_paths_fail_with_the_next_valid_command() {
    let repository = Repository::new();
    repository.write("script.py", "print('x')\n");

    let output = repository.x(&["lint", "--", "script.py"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("script.py: this file type is not supported by x"));
    assert!(stderr.contains("cargo x help"));
}

#[test]
fn missing_paths_fail_without_falling_back_to_the_whole_repository() {
    let repository = Repository::new();
    repository.write("a.json", "{}\n");
    repository.track();

    let output = repository.x(&["fmt", "--check", "--", "missing.json"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing.json: path does not exist"));
}

#[test]
fn unscoped_discovery_ignores_deleted_tracked_files() {
    let repository = Repository::new();
    repository.write("kept.json", "{}\n");
    repository.write("deleted.json", "{\"bad\":1}");
    repository.track();
    fs::remove_file(repository.0.join("deleted.json")).unwrap();

    let output = repository.x(&["fmt", "--check"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn explicit_symbolic_links_are_never_rewritten() {
    use std::os::unix::fs::symlink;

    let repository = Repository::new();
    repository.write("target.json", "{}\n");
    symlink("target.json", repository.0.join("link.json")).unwrap();

    let output = repository.x(&["fmt", "--", "link.json"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("link.json: symbolic links are not rewritten by x"));
}
