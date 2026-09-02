//! Integration tests for the `mothergod` CLI binary (`src/bin/mothergod.rs`,
//! ROADMAP M6): drives the compiled binary as a real subprocess over piped
//! stdin/stdout, the only way to exercise argument parsing and I/O wiring
//! `mothergod`'s library tests never touch.

// Not under Miri: every test here drives the compiled binary as a
// subprocess, and Miri cannot spawn one (issue #456).
#![cfg(not(miri))]

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

#[test]
fn compress_then_decompress_roundtrips_a_file_argument() {
    let dir = std::env::temp_dir().join(format!("mothergod-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let input_path = dir.join("payload");
    let original: Vec<u8> = (0..2000u32).map(|i| (i * 37 % 251) as u8).collect();
    std::fs::write(&input_path, &original).expect("write input file");

    let (_stdout, compress_code) = run(bin().arg("compress").arg(&input_path), b"");
    assert_eq!(compress_code, 0);
    let compressed_path = dir.join("payload.mgdc");
    assert!(compressed_path.exists());

    // The original is untouched; a file-argument run never deletes its input.
    assert_eq!(std::fs::read(&input_path).unwrap(), original);

    let output_path = dir.join("output");
    let renamed_compressed = output_path.with_extension("mgdc");
    std::fs::rename(&compressed_path, &renamed_compressed).expect("rename for decompress input");
    let (_stdout, decompress_code) = run(bin().arg("decompress").arg(&renamed_compressed), b"");
    assert_eq!(decompress_code, 0);
    assert_eq!(std::fs::read(&output_path).unwrap(), original);

    std::fs::remove_dir_all(&dir).expect("clean up temp dir");
}

#[test]
fn compress_refuses_to_overwrite_an_existing_output_file() {
    let dir =
        std::env::temp_dir().join(format!("mothergod-cli-test-clobber-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let input_path = dir.join("payload");
    std::fs::write(&input_path, b"hello").expect("write input file");
    let output_path = dir.join("payload.mgdc");
    std::fs::write(&output_path, b"already here").expect("write pre-existing output");

    let (_stdout, code) = run(bin().arg("compress").arg(&input_path), b"");
    assert_ne!(code, 0);
    assert_eq!(std::fs::read(&output_path).unwrap(), b"already here");

    std::fs::remove_dir_all(&dir).expect("clean up temp dir");
}

#[test]
fn decompress_refuses_to_overwrite_an_existing_output_file() {
    let dir = std::env::temp_dir().join(format!(
        "mothergod-cli-test-decompress-clobber-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let original = b"hello";
    let (compressed, compress_code) = run(bin().arg("compress"), original);
    assert_eq!(compress_code, 0);
    let input_path = dir.join("payload.mgdc");
    std::fs::write(&input_path, &compressed).expect("write compressed input file");
    let output_path = dir.join("payload");
    std::fs::write(&output_path, b"already here").expect("write pre-existing output");

    let (_stdout, code) = run(bin().arg("decompress").arg(&input_path), b"");
    assert_ne!(code, 0);
    assert_eq!(std::fs::read(&output_path).unwrap(), b"already here");

    std::fs::remove_dir_all(&dir).expect("clean up temp dir");
}

#[test]
fn decompress_of_a_corrupt_file_argument_leaves_no_output_file() {
    let dir = std::env::temp_dir().join(format!(
        "mothergod-cli-test-decompress-corrupt-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let input_path = dir.join("payload.mgdc");
    std::fs::write(&input_path, b"not a mothergod frame").expect("write corrupt input file");
    let output_path = dir.join("payload");

    let (_stdout, code) = run(bin().arg("decompress").arg(&input_path), b"");
    assert_ne!(code, 0);
    assert!(
        !output_path.exists(),
        "decode failure must not leave a partial output file behind"
    );

    std::fs::remove_dir_all(&dir).expect("clean up temp dir");
}

#[test]
fn decompress_of_a_file_without_the_mgdc_suffix_fails_cleanly() {
    let dir =
        std::env::temp_dir().join(format!("mothergod-cli-test-suffix-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let input_path = dir.join("payload.bin");
    std::fs::write(&input_path, b"not compressed").expect("write input file");

    let (_stdout, code) = run(bin().arg("decompress").arg(&input_path), b"");
    assert_ne!(code, 0);

    std::fs::remove_dir_all(&dir).expect("clean up temp dir");
}
