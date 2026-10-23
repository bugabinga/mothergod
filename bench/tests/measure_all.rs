//! `reference::measure_all` and `frontier_section` end to end (issue #1340):
//! mothergod is timed in a child process, so these need a real binary that
//! serves child mode, `bench_child`. A unit test inside the lib is its own
//! test harness and has none.

use mothergod_bench::field::competitors;
use mothergod_bench::reference::{frontier_section, measure_all};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const CHILD: &str = env!("CARGO_BIN_EXE_bench_child");

/// Repeated enough (200x) that every reference compressor's fixed
/// container overhead is dwarfed by the savings.
fn fixture() -> Vec<u8> {
    "hello, mothergod reference-compressor fixture. "
        .repeat(200)
        .into_bytes()
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "needs /usr/bin/time -f and gzip/zstd/xz on PATH"
)]
fn measure_all_measures_every_file_and_preserves_input_order() {
    let files = vec![
        ("b.txt".to_string(), b"bbbbbbbbbbbbbbbbbbbb".to_vec()),
        ("a.txt".to_string(), b"aaaaaaaaaaaaaaaaaaaa".to_vec()),
    ];
    let measurements = measure_all(&files, Path::new(CHILD))
        .expect("GNU time and the reference compressors must be installed");
    let names: Vec<&str> = measurements.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["b.txt", "a.txt"],
        "measure_all must preserve input order, not race threads into an arbitrary one"
    );
    for m in &measurements {
        assert_eq!(m.original_len, 20);
        assert!(m.mothergod_len > 0);
        assert!(m.gzip_len > 0);
        assert!(m.zstd_len > 0);
        assert!(m.xz_len > 0);
        assert!(m.encode_secs > 0.0);
        assert!(m.decode_secs > 0.0);
        assert!(m.peak_rss_kib > 0, "mothergod's peak resident set is read");
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "needs /usr/bin/time -f and gzip/zstd/xz on PATH"
)]
fn frontier_section_has_a_row_per_level_and_mothergod() {
    let files = vec![("a.txt".to_string(), fixture())];
    let measurements = measure_all(&files, Path::new(CHILD))
        .expect("GNU time and the reference compressors must be installed");
    let section = frontier_section(&files, &measurements).expect("every level must measure");
    // The header, every competitor level and mothergod; the divider
    // starts `|-`.
    let table_rows = section.lines().filter(|l| l.starts_with("| ")).count();
    assert_eq!(table_rows, 1 + competitors().len() + 1, "{section}");
    for label in [
        "| gzip -1 |",
        "| zstd -22 |",
        "| xz -9e |",
        "| **mothergod** |",
    ] {
        assert!(section.contains(label), "missing {label}: {section}");
    }
    let mothergod_row = section
        .lines()
        .find(|l| l.starts_with("| **mothergod** |"))
        .expect("checked above");
    assert!(
        !mothergod_row.contains("**-**"),
        "mothergod's peak MiB is measured: {mothergod_row}"
    );
}

/// Pipes `input` through `program --child <mode>`; stdout and success.
fn child(program: &str, mode: &str, input: &[u8]) -> (Vec<u8>, bool) {
    let mut process = Command::new(program)
        .args(["--child", mode])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("child spawns");
    let mut stdin = process.stdin.take().expect("piped");
    let input = input.to_vec();
    let writer = std::thread::spawn(move || {
        // A child that exits early closes the pipe; that is the failure
        // under test, not this thread's.
        let _ = stdin.write_all(&input);
    });
    let output = process.wait_with_output().expect("child exits");
    writer.join().expect("writer thread");
    (output.stdout, output.status.success())
}

#[test]
fn child_mode_round_trips_and_rejects_bad_input() {
    let data = fixture();
    let (packed, ok) = child(CHILD, "compress", &data);
    assert!(ok && packed.len() < data.len());
    let (unpacked, ok) = child(CHILD, "decompress", &packed);
    assert!(ok && unpacked == data);
    let (_, ok) = child(CHILD, "decompress", b"not a mothergod frame");
    assert!(!ok, "a decode error must exit non-zero");
    let (_, ok) = child(CHILD, "sideways", &data);
    assert!(!ok, "an unknown mode must exit non-zero");
}
