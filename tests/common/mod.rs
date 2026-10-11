//! Helpers shared by the suites that drive the compiled `mothergod` binary
//! (`tests/cli.rs`, `tests/fit_cli.rs`), so the spawn and pipe wiring has one
//! definition. The including file carries the Miri and Android exclusions.

use std::io::{ErrorKind, Read as _, Write as _};
use std::process::{Command, Output, Stdio};

/// Path to the compiled `mothergod` binary, set by Cargo for integration
/// tests (<https://doc.rust-lang.org/cargo/reference/environment-variables.html>).
pub fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mothergod"))
}

/// Runs `command` with `stdin` piped in and everything captured.
///
/// A child that exits without reading its stdin (an unknown flag, `--help`)
/// closes the pipe under the write; that is the child's answer, not a
/// harness failure, so a broken pipe on the way in is ignored.
pub fn run_output(command: &mut Command, stdin: &[u8]) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("mothergod binary should spawn");
    let mut pipe = child.stdin.take().expect("piped stdin");
    if let Err(err) = pipe.write_all(stdin) {
        assert_eq!(err.kind(), ErrorKind::BrokenPipe, "write to child stdin");
    }
    drop(pipe);
    child.wait_with_output().expect("mothergod should exit")
}

/// Runs `command` with `stdin` piped in, returns `(stdout, exit code)`.
pub fn run(command: &mut Command, stdin: &[u8]) -> (Vec<u8>, i32) {
    let output = run_output(command, stdin);
    (output.stdout, output.status.code().expect("no signal"))
}

/// Spawns `mothergod <command>` on `stdin`, reads one byte of its stdout,
/// drops the pipe (a reader that quit early, `| head -c 1`), and returns
/// `(stderr, exit code)`. Reading before the drop makes the child's later
/// writes hit a closed pipe, not an unread one.
pub fn run_with_early_closing_reader(command: &str, stdin: &[u8]) -> (Vec<u8>, i32) {
    let mut child = bin()
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("mothergod binary should spawn");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin)
        .expect("write to child stdin");
    let mut stdout = child.stdout.take().expect("piped stdout");
    stdout
        .read_exact(&mut [0u8; 1])
        .expect("child should emit a byte before the pipe closes");
    drop(stdout);
    let output = child.wait_with_output().expect("mothergod should exit");
    (output.stderr, output.status.code().expect("no signal"))
}

/// Runs `shell` with stdout on a pseudo-terminal, which `script(1)`
/// allocates for its child; `None` where `script` cannot be spawned.
/// Linux only: BSD's `script` takes other arguments and Windows has none.
#[cfg(target_os = "linux")]
pub fn on_pty(shell: &str) -> Option<Output> {
    Command::new("script")
        .args(["-qec", shell, "/dev/null"])
        .output()
        .ok()
}

/// Whether [`on_pty`] works here, measured, not assumed: a control that must
/// show its marker through the pty, because a `script` that fails to open
/// one exits nonzero with no output, which is what a refusal looks like.
#[cfg(target_os = "linux")]
pub fn pty_works() -> bool {
    const MARKER: &[u8] = b"pty-works";
    on_pty("printf pty-works").is_some_and(|output| {
        output.status.success() && output.stdout.windows(MARKER.len()).any(|w| w == MARKER)
    })
}
