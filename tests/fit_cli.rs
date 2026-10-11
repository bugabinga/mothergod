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

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use common::{bin, run, run_output, run_with_early_closing_reader};

/// Every compressed frame starts with this magic (`docs/format/SPEC.md`).
const MAGIC: &[u8] = b"MGDC";

/// The matrix, compiled in so the packaged crate's own `cargo test` reads it.
const MATRIX: &str = include_str!("fit/cli.txt");

#[derive(Debug, PartialEq, Eq)]
enum State {
    Holds,
    Gap,
    Declined,
}

struct Row {
    id: &'static str,
    state: State,
    note: &'static str,
}

fn rows() -> Vec<Row> {
    MATRIX
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.splitn(5, " | ");
            let id = fields.next().expect("row has an id");
            let state = match fields.next().expect("row has a state") {
                "holds" => State::Holds,
                "gap" => State::Gap,
                "declined" => State::Declined,
                other => panic!("row {id}: state {other:?} is not holds, gap or declined"),
            };
            let _behavior = fields.next().expect("row has a behavior");
            let citation = fields.next().expect("row has a citation");
            assert!(!citation.trim().is_empty(), "row {id}: no citation");
            Row {
                id,
                state,
                note: fields.next().unwrap_or("").trim(),
            }
        })
        .collect()
}

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

/// The frame today's `compress` subcommand makes of `payload`.
fn frame_of(payload: &[u8]) -> Vec<u8> {
    let (frame, code) = run(bin().arg("compress"), payload);
    assert_eq!(
        code, 0,
        "compress must work for the probes to mean anything"
    );
    frame
}

/// `true` when the file at `path` is a frame that decodes to `payload`.
fn decodes_to(path: &Path, payload: &[u8]) -> bool {
    fs::read(path).is_ok_and(|frame| run(bin().arg("decompress"), &frame) == (payload.to_vec(), 0))
}

/// What a probe saw.
enum Seen {
    Holds,
    Fails,
    /// The lane cannot run the probe; the recorded state stands.
    Unmeasured,
}

fn seen(held: bool) -> Seen {
    if held { Seen::Holds } else { Seen::Fails }
}

type Probe = fn() -> Seen;

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
];

fn mode_d() -> Seen {
    let (out, code) = run(bin().arg("-d"), &frame_of(&sample()));
    seen(code == 0 && out == sample())
}

fn stdout_c() -> Seen {
    let dir = Scratch::new("stdout-c");
    let source = dir.file("payload", &sample());
    let (out, code) = run(bin().arg("-c").arg(&source), b"");
    seen(code == 0 && out.starts_with(MAGIC) && !dir.path("payload.mgdc").exists())
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

fn level_digit() -> Seen {
    let (out, code) = run(bin().arg("-1"), &sample());
    seen(code == 0 && out.starts_with(MAGIC))
}

fn prints_to_stdout(flag: &str) -> bool {
    let (out, code) = run(bin().arg(flag), b"");
    code == 0 && !out.is_empty()
}

fn help() -> Seen {
    seen(prints_to_stdout("-h") && prints_to_stdout("--help"))
}

fn version() -> Seen {
    seen(prints_to_stdout("-V") && prints_to_stdout("--version"))
}

fn end_of_options() -> Seen {
    let dir = Scratch::new("end-of-options");
    dir.file("-payload", &sample());
    let (_out, code) = run(
        bin()
            .current_dir(&dir.0)
            .args(["compress", "--", "-payload"]),
        b"",
    );
    seen(code == 0 && dir.path("-payload.mgdc").exists())
}

fn stdin_dash() -> Seen {
    let (out, code) = run(bin().args(["compress", "-"]), &sample());
    seen(code == 0 && out.starts_with(MAGIC))
}

fn no_operand_stdio() -> Seen {
    let (frame, code) = run(bin().arg("compress"), &sample());
    seen(
        code == 0
            && frame.starts_with(MAGIC)
            && run(bin().arg("decompress"), &frame) == (sample(), 0),
    )
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
    let (_out, code) = run(bin().arg("compress").arg(&source), b"");
    seen(code == 0 && fs::read(&source).is_ok_and(|bytes| bytes == sample()))
}

fn no_clobber() -> Seen {
    let dir = Scratch::new("no-clobber");
    let source = dir.file("payload", &sample());
    let existing = dir.file("payload.mgdc", b"already here");
    let (_out, code) = run(bin().arg("compress").arg(&source), b"");
    seen(code != 0 && fs::read(&existing).is_ok_and(|bytes| bytes == b"already here"))
}

fn suffix_derive() -> Seen {
    let dir = Scratch::new("suffix-derive");
    let source = dir.file("payload", &sample());
    let (_out, compress_code) = run(bin().arg("compress").arg(&source), b"");
    let compressed = dir.path("payload.mgdc");
    if compress_code != 0 || !compressed.exists() {
        return seen(false);
    }
    fs::remove_file(&source).expect("remove source");
    let (_out, decompress_code) = run(bin().arg("decompress").arg(&compressed), b"");
    seen(decompress_code == 0 && fs::read(&source).is_ok_and(|bytes| bytes == sample()))
}

fn corrupt_nonzero() -> Seen {
    let (_out, code) = run(bin().arg("decompress"), b"text that is no frame");
    seen(code != 0)
}

fn broken_pipe() -> Seen {
    // 300 000 stored bytes outgrow any pipe buffer, so the write after the
    // reader closes cannot be absorbed (the same frame `tests/cli.rs` builds).
    let mut frame = b"MGDC\x0b\x00".to_vec();
    frame.extend(300_000u64.to_le_bytes());
    frame.extend((0..300_000u32).map(|i| (i * 37 % 251) as u8));
    let (stderr, code) = run_with_early_closing_reader("decompress", &frame);
    seen(stderr.is_empty() && code == 141)
}

/// Runs `compress` with stdout on a pseudo-terminal, which `script(1)`
/// allocates for its child. Linux only: BSD's `script` takes other
/// arguments and Windows has none, and a lane without it leaves the row
/// unmeasured (`Seen::Unmeasured`) rather than guessing.
#[cfg(target_os = "linux")]
fn refuses_terminal() -> Seen {
    use std::process::Command;

    let shell = format!("'{}' compress < /dev/null", env!("CARGO_BIN_EXE_mothergod"));
    let Ok(output) = Command::new("script")
        .args(["-qec", &shell, "/dev/null"])
        .output()
    else {
        return Seen::Unmeasured;
    };
    seen(!output.status.success() && !output.stdout.windows(MAGIC.len()).any(|w| w == MAGIC))
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
    let (_out, code) = run(bin().arg("compress").arg(&source), b"");
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
    let (_out, code) = run(bin().arg("compress").arg(&source), b"");
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
    let (out, code) = run(bin().arg("decompress"), &frames);
    let mut both = sample();
    both.extend(other_sample());
    seen(code == 0 && out == both)
}

#[test]
fn every_row_is_probed_exactly_once_and_declined_rows_are_not() {
    let rows = rows();
    let ids: BTreeSet<_> = rows.iter().map(|row| row.id).collect();
    assert_eq!(
        ids.len(),
        rows.len(),
        "duplicate row id in tests/fit/cli.txt"
    );

    let probed: BTreeSet<_> = PROBES.iter().map(|(id, _)| *id).collect();
    assert_eq!(probed.len(), PROBES.len(), "a row is probed twice");

    let live: BTreeSet<_> = rows
        .iter()
        .filter(|row| row.state != State::Declined)
        .map(|row| row.id)
        .collect();
    assert_eq!(
        probed, live,
        "probes and non-declined rows must be the same set"
    );

    for row in rows.iter().filter(|row| row.state == State::Declined) {
        assert!(
            !row.note.is_empty(),
            "declined row {} has no reason",
            row.id
        );
    }
}

#[test]
fn the_record_matches_the_binary() {
    let recorded: Vec<Row> = rows();
    let mut stale = Vec::new();
    for (id, probe) in PROBES {
        let row = recorded
            .iter()
            .find(|row| row.id == *id)
            .unwrap_or_else(|| panic!("probe {id} has no row"));
        match (&row.state, probe()) {
            (State::Holds, Seen::Fails) => stale.push(format!(
                "{id}: recorded holds, binary fails it (a regression)"
            )),
            (State::Gap, Seen::Holds) => stale.push(format!(
                "{id}: recorded gap, binary holds it; flip the row to holds in tests/fit/cli.txt"
            )),
            _ => {}
        }
    }
    assert!(stale.is_empty(), "\n{}", stale.join("\n"));
}
