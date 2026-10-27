//! The CLI half of the FIT matrix (ADR-0064, `docs/TESTING.md` layer 12).
//!
//! `tests/fit/cli.txt` records, per behavior the compressor genre shares,
//! whether the `mothergod` binary holds it. This suite probes the built
//! binary once per row and fails when the record and the observation
//! disagree in either direction: a `holds` row that fails is a regression, a
//! `gap` row that passes is a record gone stale. Either way the fix is a
//! line in the matrix, so the published count (`.github/scripts/status-data.py`
//! reads the same file) cannot drift from the binary and cannot be raised
//! by adding rows that are not probed.
//!
//! A probe returns `Seen::Unmeasured` where its lane cannot run it (no `script(1)`, no
//! Unix permission bits): the row is then unmeasured there and the record
//! stands, so a row only flips on a lane that can see it.

// Not under Miri, not on Android: every probe spawns the compiled binary,
// which neither lane can do (`tests/cli.rs` has the reasons).
#![cfg(not(any(miri, target_os = "android")))]

mod common;
#[path = "fit/matrix.rs"]
mod matrix;

use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use common::{bin, run, run_output, run_with_early_closing_reader, stored_frame};
use matrix::{Probe, Seen, seen};

/// Every compressed frame starts with this magic (`docs/format/SPEC.md`).
const MAGIC: &[u8] = b"MGDC";

/// The matrix, compiled in so the packaged crate's own `cargo test` reads it.
const MATRIX: &str = include_str!("fit/cli.txt");

/// A directory removed on drop, so a failed probe leaves no litter.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mothergod-fit-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create scratch dir");
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Writes `bytes` to a file called `name` and returns its path.
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, bytes).expect("write scratch file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sample() -> Vec<u8> {
    (0..2000u32).map(|i| (i * 37 % 251) as u8).collect()
}

fn other_sample() -> Vec<u8> {
    (0..1500u32).map(|i| (i * 11 % 241) as u8).collect()
}

/// The frame the binary makes of `payload` on stdin.
fn frame_of(payload: &[u8]) -> Vec<u8> {
    let (frame, code) = run(&mut bin(), payload);
    assert_eq!(
        code, 0,
        "compressing must work for the probes to mean anything"
    );
    frame
}

/// `true` when the file at `path` is a frame that decodes to `payload`.
fn decodes_to(path: &Path, payload: &[u8]) -> bool {
    fs::read(path).is_ok_and(|frame| run(bin().arg("-d"), &frame) == (payload.to_vec(), 0))
}

/// One probe per row that is not declined. The binary's grammar is spelled
/// as the genre does where the row is about the genre's grammar (`-c file`),
/// and as the binary's own where the row is about a behavior it already has.
const PROBES: &[(&str, Probe)] = &[
    ("mode-d", mode_d),
    ("stdout-c", stdout_c),
    ("keep-k", keep_k),
    ("force-f", force_f),
    ("test-t", test_t),
    ("verbose-v", verbose_v),
    ("quiet-q", quiet_q),
    ("level-digit", level_digit),
    ("help", help),
    ("version", version),
    ("end-of-options", end_of_options),
    ("stdin-dash", stdin_dash),
    ("no-operand-stdio", no_operand_stdio),
    ("bare-compresses", bare_compresses),
    ("multiple-operands", multiple_operands),
    ("unknown-option", unknown_option),
    ("source-kept", source_kept),
    ("no-clobber", no_clobber),
    ("suffix-derive", suffix_derive),
    ("corrupt-nonzero", corrupt_nonzero),
    ("broken-pipe", broken_pipe),
    ("refuses-terminal", refuses_terminal),
    ("mtime-copied", mtime_copied),
    ("mode-copied", mode_copied),
    ("concatenation", concatenation),
    ("streaming-memory", streaming_memory),
];

fn mode_d() -> Seen {
    let decodes = |flag: &str| {
        let (out, code) = run(bin().arg(flag), &frame_of(&sample()));
        code == 0 && out == sample()
    };
    seen(decodes("-d") && decodes("--decompress"))
}

fn stdout_c() -> Seen {
    let dir = Scratch::new("stdout-c");
    let source = dir.file("payload", &sample());
    let writes_stdout = |flag: &str| {
        let (out, code) = run(bin().arg(flag).arg(&source), b"");
        code == 0 && out.starts_with(MAGIC) && !dir.path("payload.mgdc").exists()
    };
    seen(writes_stdout("-c") && writes_stdout("--stdout"))
}

fn keep_k() -> Seen {
    let dir = Scratch::new("keep-k");
    let source = dir.file("payload", &sample());
    let (_out, code) = run(bin().arg("-k").arg(&source), b"");
    seen(code == 0 && source.exists() && dir.path("payload.mgdc").exists())
}

fn force_f() -> Seen {
    let dir = Scratch::new("force-f");
    let source = dir.file("payload", &sample());
    let existing = dir.file("payload.mgdc", b"already here");
    let (_out, code) = run(bin().arg("-f").arg(&source), b"");
    seen(code == 0 && decodes_to(&existing, &sample()))
}

fn test_t() -> Seen {
    let dir = Scratch::new("test-t");
    let good = dir.file("good.mgdc", &frame_of(&sample()));
    let bad = dir.file("bad.mgdc", b"not a mothergod frame");
    let (good_out, good_code) = run(bin().arg("-t").arg(&good), b"");
    let (_bad_out, bad_code) = run(bin().arg("-t").arg(&bad), b"");
    seen(
        good_code == 0
            && good_out.is_empty()
            && bad_code != 0
            && !dir.path("good").exists()
            && !dir.path("bad").exists(),
    )
}

fn verbose_v() -> Seen {
    let (out, code) = run(bin().arg("-v"), &sample());
    seen(code == 0 && out.starts_with(MAGIC))
}

fn quiet_q() -> Seen {
    let (out, code) = run(bin().arg("-q"), &sample());
    seen(code == 0 && out.starts_with(MAGIC))
}

fn level_digit() -> Seen {
    let encodes = |digit: &str| {
        let (frame, code) = run(bin().arg(digit), &sample());
        code == 0 && run(bin().arg("-d"), &frame) == (sample(), 0)
    };
    seen(["-1", "-2", "-3"].into_iter().all(encodes))
}

/// `true` when `flag` exits 0 having written something. The stream is not
/// probed: the genre splits on it for usage and bzip2 alone for the version.
fn prints_and_exits_zero(flag: &str) -> bool {
    let output = run_output(bin().arg(flag), b"");
    output.status.success() && !(output.stdout.is_empty() && output.stderr.is_empty())
}

fn help() -> Seen {
    seen(prints_and_exits_zero("-h") && prints_and_exits_zero("--help"))
}

fn version() -> Seen {
    seen(prints_and_exits_zero("-V") && prints_and_exits_zero("--version"))
}

fn end_of_options() -> Seen {
    let dir = Scratch::new("end-of-options");
    dir.file("-payload", &sample());
    let (_out, code) = run(bin().current_dir(&dir.0).args(["--", "-payload"]), b"");
    seen(code == 0 && dir.path("-payload.mgdc").exists())
}

fn stdin_dash() -> Seen {
    let (out, code) = run(bin().arg("-"), &sample());
    seen(code == 0 && out.starts_with(MAGIC))
}

fn no_operand_stdio() -> Seen {
    let (frame, code) = run(&mut bin(), &sample());
    seen(code == 0 && frame.starts_with(MAGIC) && run(bin().arg("-d"), &frame) == (sample(), 0))
}

fn bare_compresses() -> Seen {
    let (out, code) = run(&mut bin(), &sample());
    seen(code == 0 && out.starts_with(MAGIC))
}

fn multiple_operands() -> Seen {
    let dir = Scratch::new("multiple-operands");
    let first = dir.file("first", &sample());
    let second = dir.file("second", &other_sample());
    let (_out, code) = run(bin().arg(&first).arg(&second), b"");
    seen(
        code == 0
            && decodes_to(&dir.path("first.mgdc"), &sample())
            && decodes_to(&dir.path("second.mgdc"), &other_sample()),
    )
}

fn unknown_option() -> Seen {
    let output = run_output(bin().arg("--frobnicate"), b"");
    seen(!output.status.success() && !output.stderr.is_empty())
}

fn source_kept() -> Seen {
    let dir = Scratch::new("source-kept");
    let source = dir.file("payload", &sample());
    let (_out, code) = run(bin().arg(&source), b"");
    seen(code == 0 && fs::read(&source).is_ok_and(|bytes| bytes == sample()))
}

fn no_clobber() -> Seen {
    let dir = Scratch::new("no-clobber");
    let source = dir.file("payload", &sample());
    let existing = dir.file("payload.mgdc", b"already here");
    let (_out, code) = run(bin().arg(&source), b"");
    seen(code != 0 && fs::read(&existing).is_ok_and(|bytes| bytes == b"already here"))
}

fn suffix_derive() -> Seen {
    let dir = Scratch::new("suffix-derive");
    let source = dir.file("payload", &sample());
    let (_out, compress_code) = run(bin().arg(&source), b"");
    let compressed = dir.path("payload.mgdc");
    if compress_code != 0 || !compressed.exists() {
        return seen(false);
    }
    fs::remove_file(&source).expect("remove source");
    let (_out, decompress_code) = run(bin().arg("-d").arg(&compressed), b"");
    seen(decompress_code == 0 && fs::read(&source).is_ok_and(|bytes| bytes == sample()))
}

fn corrupt_nonzero() -> Seen {
    let (_out, code) = run(bin().arg("-d"), b"text that is no frame");
    seen(code != 0)
}

fn broken_pipe() -> Seen {
    // 300 000 stored bytes outgrow any pipe buffer, so the write after the
    // reader closes cannot be absorbed (the same frame `tests/cli.rs` builds).
    let data: Vec<u8> = (0..300_000u32).map(|i| (i * 37 % 251) as u8).collect();
    let frame = stored_frame(&data);
    let (stderr, code) = run_with_early_closing_reader(&["-d"], &frame);
    seen(stderr.is_empty() && code == 141)
}

/// Linux only: see [`common::on_pty`]. A lane without a working pty leaves
/// the row unmeasured (`Seen::Unmeasured`) rather than guessing.
#[cfg(target_os = "linux")]
fn refuses_terminal() -> Seen {
    if !common::pty_works() {
        return Seen::Unmeasured;
    }
    let shell = format!("'{}' < /dev/null", env!("CARGO_BIN_EXE_mothergod"));
    match common::on_pty(&shell) {
        Some(output) => seen(
            !output.status.success() && !output.stdout.windows(MAGIC.len()).any(|w| w == MAGIC),
        ),
        None => Seen::Unmeasured,
    }
}

#[cfg(not(target_os = "linux"))]
fn refuses_terminal() -> Seen {
    Seen::Unmeasured
}

/// 2001-09-09T01:46:40Z: old enough that "now" cannot pass for it.
fn long_ago() -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_000_000_000)
}

fn mtime_copied() -> Seen {
    let dir = Scratch::new("mtime-copied");
    let source = dir.file("payload", &sample());
    File::options()
        .write(true)
        .open(&source)
        .and_then(|file| file.set_modified(long_ago()))
        .expect("set source mtime");
    let (_out, code) = run(bin().arg(&source), b"");
    let modified = fs::metadata(dir.path("payload.mgdc")).and_then(|meta| meta.modified());
    seen(code == 0 && modified.is_ok_and(|time| time == long_ago()))
}

#[cfg(unix)]
fn mode_copied() -> Seen {
    use std::os::unix::fs::PermissionsExt as _;

    // 0o640 is not what a default umask gives a new file, so a copy shows.
    let dir = Scratch::new("mode-copied");
    let source = dir.file("payload", &sample());
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).expect("set source mode");
    let (_out, code) = run(bin().arg(&source), b"");
    let mode = fs::metadata(dir.path("payload.mgdc")).map(|meta| meta.permissions().mode());
    seen(code == 0 && mode.is_ok_and(|bits| bits & 0o777 == 0o640))
}

#[cfg(not(unix))]
fn mode_copied() -> Seen {
    Seen::Unmeasured
}

fn concatenation() -> Seen {
    let mut frames = frame_of(&sample());
    frames.extend(frame_of(&other_sample()));
    let (out, code) = run(bin().arg("-d"), &frames);
    let mut both = sample();
    both.extend(other_sample());
    seen(code == 0 && out == both)
}

/// The block `copy_encode` cuts (its doc names 16 MiB).
const BLOCK: usize = 16 * 1024 * 1024;

/// The most blocks the encoder holds unencoded or encoding: two encode at
/// once on two cores (`write::MAX_CONCURRENT_ENCODES`) while the third fills.
/// One core holds one, so this is the ceiling over every machine.
const BLOCKS_HELD: usize = 3;

/// Blocks the writer offers: more than the child may hold, so a child that
/// slurps its input reads them all.
const BLOCKS_OFFERED: usize = BLOCKS_HELD + 2;

/// Past `BLOCKS_HELD` blocks, a read-ahead beyond what the pipe itself buffers.
const READ_AHEAD: usize = 1024 * 1024;

/// How long the probe lets a child that has read one block keep reading.
const SETTLE: Duration = Duration::from_secs(1);

/// How long the writer gets to push one block into the child.
const FILL: Duration = Duration::from_secs(60);

/// The encoder stops reading once its running blocks and the one filling
/// are held, so a writer feeding five blocks stalls within three: input is
/// not buffered ahead of the work. Measuring the peak itself needs an encode
/// of at least one block, minutes in a debug build, so the weekly
/// `memory-check` lane owns that number and the probe is killed with the
/// first block still encoding. A child that
/// slurps its input fails, and so does one that exits before a full block
/// arrived: a crash is what the row exists to see. A child still alive when
/// `FILL` ends has a writer that filled under one block, which measured the
/// runner, not the child, so that is `Seen::Unmeasured`. A slow runner cannot
/// fail a child that streams.
fn streaming_memory() -> Seen {
    stream_probe(bin(), FILL)
}

fn stream_probe(mut command: Command, fill: Duration) -> Seen {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("command should spawn");
    let mut stdin = child.stdin.take().expect("piped stdin");
    let written = Arc::new(AtomicUsize::new(0));
    let progress = Arc::clone(&written);
    let writer = thread::spawn(move || {
        let chunk = vec![0u8; 64 * 1024];
        for _ in 0..BLOCKS_OFFERED * BLOCK / chunk.len() {
            if stdin.write_all(&chunk).is_err() {
                return;
            }
            progress.fetch_add(chunk.len(), Ordering::SeqCst);
        }
    });
    let deadline = Instant::now() + fill;
    let mut exited = false;
    while written.load(Ordering::SeqCst) < BLOCK && Instant::now() < deadline && !exited {
        thread::sleep(Duration::from_millis(10));
        exited = child.try_wait().expect("poll the child").is_some();
    }
    let filled = written.load(Ordering::SeqCst) >= BLOCK;
    if filled {
        thread::sleep(SETTLE);
    }
    let read = written.load(Ordering::SeqCst);
    child.kill().expect("kill the child still encoding");
    writer.join().expect("writer thread");
    child.wait().expect("reap the child");
    if exited && !filled {
        return Seen::Fails;
    }
    if !filled {
        return Seen::Unmeasured;
    }
    seen(read < BLOCKS_HELD * BLOCK + READ_AHEAD)
}

#[test]
fn every_row_is_probed_exactly_once_and_declined_rows_are_not() {
    matrix::check_probes_cover_rows("tests/fit/cli.txt", MATRIX, PROBES);
}

#[test]
fn the_record_matches_the_binary() {
    matrix::check_record("tests/fit/cli.txt", MATRIX, PROBES);
}

/// The ways a child leaves `stream_probe` short of one block or past the
/// held blocks, on a short `fill` so the stalled case costs milliseconds, not the real 60 s.
#[cfg(unix)]
mod stream_probe_cases {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn a_child_that_exits_before_one_block_fails() {
        let seen = stream_probe(Command::new("false"), Duration::from_secs(30));
        assert!(matches!(seen, Seen::Fails));
    }

    #[test]
    fn a_child_alive_but_stalled_is_unmeasured() {
        let mut command = Command::new("sleep");
        command.arg("30");
        let seen = stream_probe(command, Duration::from_millis(200));
        assert!(matches!(seen, Seen::Unmeasured));
    }

    #[test]
    fn a_child_that_slurps_its_input_fails() {
        let seen = stream_probe(sh("cat >/dev/null"), Duration::from_secs(30));
        assert!(matches!(seen, Seen::Fails));
    }

    #[test]
    fn a_child_that_reads_a_block_past_the_held_blocks_fails() {
        let seen = stream_probe(
            sh("head -c 67108864 >/dev/null; exec sleep 30"),
            Duration::from_secs(30),
        );
        assert!(matches!(seen, Seen::Fails));
    }

    #[test]
    fn a_child_that_stops_after_one_block_holds() {
        let seen = stream_probe(
            sh("head -c 16777216 >/dev/null; exec sleep 30"),
            Duration::from_secs(30),
        );
        assert!(matches!(seen, Seen::Holds));
    }
}
