//! Integration tests for the `mothergod` CLI binary (`src/bin/mothergod.rs`,
//! ROADMAP M6): drives the compiled binary as a real subprocess over piped
//! stdin/stdout, the only way to exercise argument parsing and I/O wiring
//! `mothergod`'s library tests never touch.

use std::io::Write as _;
use std::process::{Command, Stdio};

/// Path to the compiled `mothergod` binary, set by Cargo for integration
/// tests (<https://doc.rust-lang.org/cargo/reference/environment-variables.html>).
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mothergod"))
}

/// Runs `command` with `stdin` piped in, returns `(stdout, exit code)`.
fn run(command: &mut Command, stdin: &[u8]) -> (Vec<u8>, i32) {
    let mut child = command
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
    let output = child.wait_with_output().expect("mothergod should exit");
    (output.stdout, output.status.code().expect("no signal"))
}

#[test]
fn compress_then_decompress_roundtrips_arbitrary_bytes() {
    let original: Vec<u8> = (0..2000u32).map(|i| (i * 37 % 251) as u8).collect();

    let (compressed, compress_code) = run(bin().arg("compress"), &original);
    assert_eq!(compress_code, 0);

    let (decompressed, decompress_code) = run(bin().arg("decompress"), &compressed);
    assert_eq!(decompress_code, 0);
    assert_eq!(decompressed, original);
}

#[test]
fn compress_then_decompress_roundtrips_empty_input() {
    let (compressed, compress_code) = run(bin().arg("compress"), b"");
    assert_eq!(compress_code, 0);

    let (decompressed, decompress_code) = run(bin().arg("decompress"), &compressed);
    assert_eq!(decompress_code, 0);
    assert!(decompressed.is_empty());
}

#[test]
fn decompress_of_garbage_fails_cleanly_instead_of_panicking() {
    let (_stdout, code) = run(bin().arg("decompress"), b"not a mothergod frame");
    assert_ne!(code, 0);
}

#[test]
fn missing_command_fails_with_usage() {
    let (_stdout, code) = run(&mut bin(), b"");
    assert_ne!(code, 0);
}

#[test]
fn unknown_command_fails_with_usage() {
    let (_stdout, code) = run(bin().arg("frobnicate"), b"");
    assert_ne!(code, 0);
}

#[test]
fn help_flag_succeeds() {
    let (stdout, code) = run(bin().arg("--help"), b"");
    assert_eq!(code, 0);
    assert!(!stdout.is_empty());
}
