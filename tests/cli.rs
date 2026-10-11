//! Integration tests for the `mothergod` CLI binary (`src/bin/mothergod.rs`,
//! ROADMAP M6): drives the compiled binary as a real subprocess over piped
//! stdin/stdout, the only way to exercise argument parsing and I/O wiring
//! `mothergod`'s library tests never touch.

// Not under Miri: every test here drives the compiled binary as a
// subprocess, and Miri cannot spawn one (issue #456).
// Not on Android: `.github/scripts/android-runner` pushes only the test
// executable plus tests/adversarial/ and tests/golden/ to the device, not
// `CARGO_BIN_EXE_mothergod`, so spawning it there is a guaranteed ENOENT,
// not a finding about the codec (issue #518).
#![cfg(not(any(miri, target_os = "android")))]

mod common;

use common::{bin, run, run_with_early_closing_reader};

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
    assert_eq!(decompressed, [] as [u8; 0]);
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
fn too_many_arguments_fails_with_usage() {
    let (_stdout, code) = run(bin().arg("compress").arg("a").arg("b"), b"");
    assert_ne!(code, 0);
}

#[test]
fn help_flag_succeeds() {
    let (stdout, code) = run(bin().arg("--help"), b"");
    assert_eq!(code, 0);
    let text = String::from_utf8(stdout).expect("help is UTF-8");
    // The exact line: `"--help"` contains `"-h"`, so a per-flag substring check
    // passes even when `-h` is dropped from the usage.
    assert!(
        text.contains("mothergod <-h|--help|-V|--version>"),
        "help omits the flag line: {text}"
    );
}

/// The line `--version` and `-V` print: the crate version and the one frame
/// version this build reads, both from the constants so neither can drift.
fn version_line() -> String {
    format!(
        "mothergod {} (format {})\n",
        env!("CARGO_PKG_VERSION"),
        mothergod::FORMAT_VERSION
    )
}

#[test]
fn version_flag_prints_crate_and_format_version() {
    let (stdout, code) = run(bin().arg("--version"), b"");
    assert_eq!(code, 0);
    assert_eq!(stdout, version_line().as_bytes());
}

#[test]
fn short_version_flag_prints_crate_and_format_version() {
    let (stdout, code) = run(bin().arg("-V"), b"");
    assert_eq!(code, 0);
    assert_eq!(stdout, version_line().as_bytes());
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

#[test]
fn decompress_exits_quietly_when_the_stdout_reader_goes_away() {
    // A Stored frame (method byte 0): 300 000 data bytes exceed any pipe
    // buffer, so the write after the reader closes cannot be absorbed.
    let mut frame = b"MGDC\x0b\x00".to_vec();
    frame.extend(300_000u64.to_le_bytes());
    frame.extend((0..300_000u32).map(|i| (i * 37 % 251) as u8));

    let (stderr, code) = run_with_early_closing_reader("decompress", &frame);
    assert_eq!(String::from_utf8_lossy(&stderr), "");
    assert_eq!(code, 141);
}

#[test]
fn compress_exits_quietly_when_the_stdout_reader_goes_away() {
    // Incompressible input stays near its own size, past a 64 KiB pipe buffer.
    let mut state = 0x9e37_79b9_u32;
    let input: Vec<u8> = (0..100_000)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect();

    let (stderr, code) = run_with_early_closing_reader("compress", &input);
    assert_eq!(String::from_utf8_lossy(&stderr), "");
    assert_eq!(code, 141);
}

/// Linux only, and only where `script(1)` shows a pty: see
/// [`common::pty_works`]. The frame is binary noise on a terminal, so
/// `compress` refuses; `decompress` writes the user's own bytes, as `zcat`
/// does, so it does not.
#[cfg(target_os = "linux")]
#[test]
fn compress_refuses_a_terminal_and_decompress_does_not() {
    if !common::pty_works() {
        return;
    }
    let exe = env!("CARGO_BIN_EXE_mothergod");

    let refused = common::on_pty(&format!("'{exe}' compress < /dev/null")).expect("script spawns");
    assert!(!refused.status.success());
    assert!(!refused.stdout.windows(4).any(|w| w == b"MGDC"));
    assert!(String::from_utf8_lossy(&refused.stdout).contains("not written to a terminal"));

    let shown = common::on_pty(&format!(
        "printf hello | '{exe}' compress | '{exe}' decompress"
    ))
    .expect("script spawns");
    assert!(shown.status.success());
    assert!(String::from_utf8_lossy(&shown.stdout).contains("hello"));
}
