//! `frame_probe` and the probe it shares with the `integrity-check` FIT row
//! (issue #1210): the sample is what the cap promises, the probe counts a
//! decoder that is wrong, and the binary's exit code follows `silent`.

#[allow(dead_code)]
#[path = "../../tests/fit/integrity.rs"]
mod integrity;

use integrity::{Report, positions, probe, probe_with};
use std::process::Command;

#[test]
fn positions_are_exhaustive_at_or_under_the_cap() {
    assert_eq!(
        positions(7, 7).collect::<Vec<_>>(),
        (0..7).collect::<Vec<_>>()
    );
    assert_eq!(
        positions(7, 100).collect::<Vec<_>>(),
        (0..7).collect::<Vec<_>>()
    );
    assert_eq!(positions(0, 5).count(), 0);
    assert_eq!(positions(5, 0).count(), 0);
}

#[test]
fn positions_over_the_cap_are_cap_many_and_one_per_cell() {
    let (total, cap) = (1000, 37);
    let sample: Vec<usize> = positions(total, cap).collect();
    assert_eq!(sample.len(), cap);
    for (cell, &position) in sample.iter().enumerate() {
        assert!(
            (cell * total / cap..(cell + 1) * total / cap).contains(&position),
            "cell {cell} holds {position}"
        );
    }
    assert_eq!(sample, positions(total, cap).collect::<Vec<_>>(), "seeded");
}

#[test]
fn the_codec_reports_no_silent_mutation_and_the_counts_follow_the_cap() {
    let payload = b"frame probe over a short, repetitive input. ".repeat(20);
    let frame_len = mothergod::compress(&payload).len();
    let all = probe(&payload, usize::MAX);
    assert_eq!(
        all,
        Report {
            bytes: frame_len,
            flips: frame_len * 8,
            truncations: frame_len,
            silent: 0
        }
    );
    let capped = probe(&payload, 50);
    assert_eq!(
        (capped.flips, capped.truncations, capped.silent),
        (50, 50, 0)
    );
}

/// A decoder that skips the checksum, the failure the probe exists to see.
#[test]
fn a_decoder_that_accepts_a_flipped_stream_is_counted_silent() {
    let payload = [0xAA_u8; 32];
    let frame = payload.to_vec();
    // Reads every byte as data: a flip changes the output, a truncation
    // shortens it, and neither is an error.
    let report = probe_with(
        &payload,
        &frame,
        |bytes| Ok::<_, ()>(bytes.to_vec()),
        usize::MAX,
    );
    assert_eq!(report.flips, 256);
    assert_eq!(report.truncations, 32);
    assert_eq!(report.silent, 256 + 32);
}

/// A decoder that errs on every mutation is not silent however wrong it is.
#[test]
fn an_erroring_decoder_is_never_silent() {
    let payload = [1_u8, 2, 3];
    let report = probe_with(&payload, &payload, |_| Err::<Vec<u8>, _>(()), usize::MAX);
    assert_eq!(
        (report.flips, report.truncations, report.silent),
        (24, 3, 0)
    );
}

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_frame_probe"))
        .args(args)
        .output()
        .expect("frame_probe spawns");
    (
        output.status.code().expect("exits, not killed"),
        String::from_utf8(output.stdout).expect("utf8"),
        String::from_utf8(output.stderr).expect("utf8"),
    )
}

#[test]
fn the_binary_prints_one_line_per_file_and_exits_zero_when_none_is_silent() {
    let dir = std::env::temp_dir().join(format!("frame_probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("input.txt");
    std::fs::write(&file, b"one cited line per review. ".repeat(10)).expect("scratch file");
    let path = file.to_str().expect("utf8 path");
    let (code, stdout, _) = run(&["--max-decodes", "40", path]);
    std::fs::remove_dir_all(&dir).expect("scratch cleanup");
    assert_eq!(code, 0);
    let line = stdout
        .strip_prefix(&format!("{path}: bytes "))
        .expect("path first");
    assert!(
        line.ends_with(", flips 40, truncations 40, silent 0\n"),
        "{line}"
    );
}

#[test]
fn the_binary_fails_on_a_missing_file_and_on_no_arguments() {
    let (code, stdout, stderr) = run(&["/nonexistent/frame_probe_input"]);
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert!(
        stderr.contains("/nonexistent/frame_probe_input"),
        "{stderr}"
    );
    assert_eq!(run(&[]).0, 2);
    assert_eq!(run(&["--max-decodes", "x", "a"]).0, 2);
    assert_eq!(run(&["--max-decodes", "0", "a"]).0, 2);
}
