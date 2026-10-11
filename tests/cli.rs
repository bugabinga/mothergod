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

use common::{bin, run, run_output, run_with_early_closing_reader};

#[test]
fn compress_then_decompress_roundtrips_arbitrary_bytes() {
    let original: Vec<u8> = (0..2000u32).map(|i| (i * 37 % 251) as u8).collect();

    let (compressed, compress_code) = run(&mut bin(), &original);
    assert_eq!(compress_code, 0);

    let (decompressed, decompress_code) = run(bin().arg("-d"), &compressed);
    assert_eq!(decompress_code, 0);
    assert_eq!(decompressed, original);
}

#[test]
fn compress_then_decompress_roundtrips_empty_input() {
    let (compressed, compress_code) = run(&mut bin(), b"");
    assert_eq!(compress_code, 0);

    let (decompressed, decompress_code) = run(bin().arg("-d"), &compressed);
    assert_eq!(decompress_code, 0);
    assert_eq!(decompressed, [] as [u8; 0]);
}

#[test]
fn decompress_of_garbage_fails_cleanly_instead_of_panicking() {
    let (_stdout, code) = run(bin().arg("-d"), b"not a mothergod frame");
    assert_ne!(code, 0);
}

#[test]
fn a_retired_subcommand_word_is_a_file_operand_now() {
    // `mothergod compress` used to be a command. It is a file that does not
    // exist, so the run fails to read it and writes nothing.
    let output = run_output(bin().arg("compress"), b"");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("reading compress"));
}

#[test]
fn unknown_options_fail_with_usage_naming_the_option() {
    for option in ["--frobnicate", "-x", "-dx"] {
        let output = run_output(bin().arg(option), b"");
        assert!(!output.status.success(), "{option} must fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("unknown option"), "{option}: {stderr}");
        assert!(stderr.contains("usage:"), "{option}: {stderr}");
    }
}

/// A scratch directory that holds named files, removed on drop.
struct Dir(std::path::PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mothergod-cli-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).expect("write file");
        path
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn option_clusters_and_long_options_select_the_same_modes() {
    let original = b"cluster cluster cluster cluster";
    let (frame, _) = run(&mut bin(), original);
    for flags in [
        &["-d"][..],
        &["--decompress"],
        &["-dc"],
        &["-cd"],
        &["-d", "-c"],
    ] {
        let (out, code) = run(bin().args(flags), &frame);
        assert_eq!((out.as_slice(), code), (&original[..], 0), "{flags:?}");
    }
}

#[test]
fn dash_names_stdin_and_a_double_dash_ends_options() {
    let original = b"dash dash dash dash dash dash";
    let (frame, _) = run(bin().arg("-"), original);
    assert_eq!(run(bin().args(["-d", "-"]), &frame).0, original);

    // After `--`, `-d` is the name of a file, not an option.
    let dir = Dir::new("double-dash");
    dir.file("-d", original);
    let (_out, code) = run(bin().current_dir(&dir.0).args(["--", "-d"]), b"");
    assert_eq!(code, 0);
    assert!(dir.0.join("-d.mgdc").exists());
}

#[test]
fn stdout_flag_writes_the_result_to_stdout_and_creates_no_file() {
    let dir = Dir::new("stdout-flag");
    let original = b"to stdout to stdout to stdout";
    let source = dir.file("payload", original);
    let (frame, code) = run(bin().arg("-c").arg(&source), b"");
    assert_eq!(code, 0);
    assert!(frame.starts_with(b"MGDC"));
    assert!(!dir.0.join("payload.mgdc").exists());

    // Decoding to stdout needs no `.mgdc` suffix on the input.
    let framed = dir.file("framed.bin", &frame);
    let (out, code) = run(bin().arg("-dc").arg(&framed), b"");
    assert_eq!((out.as_slice(), code), (&original[..], 0));
    assert!(!dir.0.join("framed").exists());
}

#[test]
fn several_operands_each_get_their_own_output_in_order() {
    let dir = Dir::new("operands");
    let first = dir.file("first", b"first first first first");
    let second = dir.file("second", b"second second second second");
    let (_out, code) = run(bin().arg(&first).arg(&second), b"");
    assert_eq!(code, 0);
    let first_frame = dir.0.join("first.mgdc");
    let second_frame = dir.0.join("second.mgdc");
    assert!(first_frame.exists() && second_frame.exists());

    // With -c, the outputs follow one another on stdout: concatenated frames
    // decode to the first payload alone until #978, so compare the frames.
    let (stdout, code) = run(bin().arg("-c").arg(&first).arg(&second), b"");
    assert_eq!(code, 0);
    let mut both = std::fs::read(&first_frame).unwrap();
    both.extend(std::fs::read(&second_frame).unwrap());
    assert_eq!(stdout, both);
}

#[test]
fn a_failing_operand_does_not_stop_the_next_and_the_run_exits_one() {
    let dir = Dir::new("failing-operand");
    let bad = dir.file("bad.mgdc", b"not a mothergod frame");
    let (frame, _) = run(&mut bin(), b"good good good good");
    let good = dir.file("good.mgdc", &frame);
    let output = run_output(bin().arg("-d").arg(&bad).arg(&good), b"");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        std::fs::read(dir.0.join("good")).unwrap(),
        b"good good good good"
    );
    assert!(!dir.0.join("bad").exists());
    // The decode failure names its input, the only clue among several.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bad.mgdc"), "{stderr}");
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

    let (_stdout, compress_code) = run(bin().arg(&input_path), b"");
    assert_eq!(compress_code, 0);
    let compressed_path = dir.join("payload.mgdc");
    assert!(compressed_path.exists());

    // The original is untouched; a file-argument run never deletes its input.
    assert_eq!(std::fs::read(&input_path).unwrap(), original);

    let output_path = dir.join("output");
    let renamed_compressed = output_path.with_extension("mgdc");
    std::fs::rename(&compressed_path, &renamed_compressed).expect("rename for decompress input");
    let (_stdout, decompress_code) = run(bin().arg("-d").arg(&renamed_compressed), b"");
    assert_eq!(decompress_code, 0);
    assert_eq!(std::fs::read(&output_path).unwrap(), original);

    std::fs::remove_dir_all(&dir).expect("clean up temp dir");
}

/// 2001-09-09T01:46:40Z: old enough that "now" cannot pass for it.
#[cfg(unix)]
fn long_ago() -> std::time::SystemTime {
    std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000)
}

/// The output of either direction takes its input file's mtime and mode
/// (#1040); `decompress` has no frame field to read them from, so it reads
/// the `.mgdc` file's own metadata.
#[cfg(unix)]
#[test]
fn file_outputs_take_the_input_files_mtime_and_mode_in_both_directions() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("mothergod-cli-meta-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let stamp = |path: &std::path::Path, mode: u32| {
        std::fs::File::options()
            .write(true)
            .open(path)
            .and_then(|file| file.set_modified(long_ago()))
            .expect("set mtime");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set mode");
    };
    let seen = |path: &std::path::Path| {
        let meta = std::fs::metadata(path).expect("stat output");
        (
            meta.modified().expect("mtime"),
            meta.permissions().mode() & 0o7777,
        )
    };

    let source = dir.join("payload");
    std::fs::write(&source, b"payload payload payload").expect("write input file");
    stamp(&source, 0o640);
    let (_stdout, code) = run(bin().arg(&source), b"");
    assert_eq!(code, 0);
    let compressed = dir.join("payload.mgdc");
    assert_eq!(seen(&compressed), (long_ago(), 0o640));

    // A read-only input still yields an output that was writable while written.
    let restored = dir.join("restored.mgdc");
    std::fs::rename(&compressed, &restored).expect("rename for decompress input");
    stamp(&restored, 0o444);
    let (_stdout, code) = run(bin().arg("-d").arg(&restored), b"");
    assert_eq!(code, 0);
    assert_eq!(seen(&dir.join("restored")), (long_ago(), 0o444));

    // setuid, setgid and sticky stay behind: the output has another owner.
    let setuid = dir.join("setuid");
    std::fs::write(&setuid, b"payload payload payload").expect("write input file");
    stamp(&setuid, 0o4755);
    let (_stdout, code) = run(bin().arg(&setuid), b"");
    assert_eq!(code, 0);
    assert_eq!(seen(&dir.join("setuid.mgdc")).1, 0o755);
    let special = dir.join("special.mgdc");
    std::fs::rename(dir.join("setuid.mgdc"), &special).expect("rename for decompress input");
    stamp(&special, 0o7755);
    let (_stdout, code) = run(bin().arg("-d").arg(&special), b"");
    assert_eq!(code, 0);
    assert_eq!(seen(&dir.join("special")).1, 0o755);

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

    let (_stdout, code) = run(bin().arg(&input_path), b"");
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
    let (compressed, compress_code) = run(&mut bin(), original);
    assert_eq!(compress_code, 0);
    let input_path = dir.join("payload.mgdc");
    std::fs::write(&input_path, &compressed).expect("write compressed input file");
    let output_path = dir.join("payload");
    std::fs::write(&output_path, b"already here").expect("write pre-existing output");

    let (_stdout, code) = run(bin().arg("-d").arg(&input_path), b"");
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

    let (_stdout, code) = run(bin().arg("-d").arg(&input_path), b"");
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

    let (_stdout, code) = run(bin().arg("-d").arg(&input_path), b"");
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

    let (stderr, code) = run_with_early_closing_reader(&["-d"], &frame);
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

    let (stderr, code) = run_with_early_closing_reader(&[], &input);
    assert_eq!(String::from_utf8_lossy(&stderr), "");
    assert_eq!(code, 141);
}

/// Linux only, and only where `script(1)` shows a pty: see
/// [`common::pty_works`]. The frame is binary noise on a terminal, so
/// compressing refuses; `-d` writes the user's own bytes, as `zcat` does, so
/// it does not.
#[cfg(target_os = "linux")]
#[test]
fn compress_refuses_a_terminal_and_decompress_does_not() {
    if !common::pty_works() {
        return;
    }
    let exe = env!("CARGO_BIN_EXE_mothergod");

    let refused = common::on_pty(&format!("'{exe}' < /dev/null")).expect("script spawns");
    assert!(!refused.status.success());
    assert!(!refused.stdout.windows(4).any(|w| w == b"MGDC"));
    assert!(String::from_utf8_lossy(&refused.stdout).contains("not written to a terminal"));

    let shown =
        common::on_pty(&format!("printf hello | '{exe}' | '{exe}' -d")).expect("script spawns");
    assert!(shown.status.success());
    assert!(String::from_utf8_lossy(&shown.stdout).contains("hello"));
}
