//! `reference::measure_all`, `frontier_section` and `apart_section` end to
//! end (issues #1340, #1926):
//! mothergod is timed in a child process, so these need a real binary that
//! serves child mode, `bench_child`. A unit test inside the lib is its own
//! test harness and has none.

use mothergod_bench::field::competitors;
use mothergod_bench::reference::{apart_section, frontier_section, machine_info, measure_all};
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
    let whole = frontier_section(&files, &measurements, Path::new(CHILD), true)
        .expect("every level must measure");
    let (main, apart) = whole
        .split_once("\n## Frontier of mothergod -4:")
        .expect("the whole report carries the apart section");
    // Each table: the header, every competitor level and its bold rows;
    // the divider starts `|-`.
    let table_rows = |text: &str| text.lines().filter(|l| l.starts_with("| ")).count();
    assert_eq!(table_rows(main), 1 + competitors().len() + 3, "{main}");
    assert_eq!(table_rows(apart), 1 + competitors().len() + 1, "{apart}");
    for (text, label) in [
        (main, "| gzip -1 |"),
        (main, "| zstd -22 |"),
        (main, "| xz -9e |"),
        (main, "| **mothergod -1** |"),
        (main, "| **mothergod -2** |"),
        (main, "| **mothergod** |"),
        (apart, "| xz -9e |"),
        (apart, "| **mothergod -4** |"),
        (apart, "on the machine named above"),
        (apart, "\nfastest level at least as good as mothergod -4: "),
    ] {
        assert!(text.contains(label), "missing {label}: {text}");
    }
    assert!(!apart.contains("share of"), "{apart}");
    let mothergod_row = main
        .lines()
        .find(|l| l.starts_with("| **mothergod** |"))
        .expect("checked above");
    assert!(
        !mothergod_row.contains("**-**"),
        "mothergod's peak MiB is measured: {mothergod_row}"
    );

    let except = frontier_section(&files, &measurements, Path::new(CHILD), false)
        .expect("every level must measure");
    assert!(!except.contains("mothergod -4"), "{except}");
    assert_eq!(table_rows(&except), table_rows(main), "{except}");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "needs /usr/bin/time -f and gzip/zstd/xz on PATH"
)]
fn apart_section_names_its_own_machine_and_carries_its_own_matrix() {
    let files = vec![("a.txt".to_string(), fixture())];
    let section = apart_section(&files, Path::new(CHILD)).expect("every level must measure");
    assert!(
        section.starts_with("\n## Frontier of mothergod -4:"),
        "{section}"
    );
    assert!(
        section.contains(&format!("on its own machine, {},", machine_info())),
        "{section}"
    );
    let table_rows = section.lines().filter(|l| l.starts_with("| ")).count();
    assert_eq!(table_rows, 1 + competitors().len() + 1, "{section}");
}

/// Pipes `input` through `program --child <args>`; stdout and success.
fn child(program: &str, args: &[&str], input: &[u8]) -> (Vec<u8>, bool) {
    let mut process = Command::new(program)
        .arg("--child")
        .args(args)
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
#[cfg_attr(
    target_os = "android",
    ignore = "android-runner ships only the test executable; CARGO_BIN_EXE_bench_child is a host path (#518)"
)]
fn child_mode_round_trips_and_rejects_bad_input() {
    let data = fixture();
    for digit in ["1", "2", "3", "4"] {
        let (packed, ok) = child(CHILD, &["compress", digit], &data);
        assert!(ok && packed.len() < data.len(), "-{digit}");
        let (unpacked, ok) = child(CHILD, &["decompress"], &packed);
        assert!(ok && unpacked == data, "-{digit}");
    }
    let (_, ok) = child(CHILD, &["decompress"], b"not a mothergod frame");
    assert!(!ok, "a decode error must exit non-zero");
    for bad in [
        &["sideways"][..],
        &["compress"],
        &["compress", "0"],
        &["compress", "5"],
        &["compress", "13"],
        &["decompress", "3"],
    ] {
        let (_, ok) = child(CHILD, bad, &data);
        assert!(!ok, "{bad:?} must exit non-zero");
    }
}
