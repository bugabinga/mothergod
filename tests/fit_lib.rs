//! The library half of the FIT matrix (ADR-0064, `docs/TESTING.md` layer 12).
//!
//! `tests/fit/lib.txt` records, per idiom the Rust compression crates share,
//! whether this crate holds it, and this suite fails when the record and the
//! observation disagree in either direction (`tests/fit/matrix.rs`). A row
//! with a file in `tests/fit/lib/` is the idiom as a user writes it, and it
//! holds when `rustc` accepts that file against the built crate; the other
//! rows run the behavior.

// Not under Miri, not on Android: a snippet probe spawns `rustc`, which
// neither lane can do (`tests/cli.rs` has the reasons for the binary).
#![cfg(not(any(miri, target_os = "android")))]

// Shared with the `frame_probe` binary; this suite reads `probe` alone.
#[allow(dead_code)]
#[path = "fit/integrity.rs"]
mod integrity;
#[path = "fit/matrix.rs"]
mod matrix;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use matrix::{Probe, Seen, seen};

/// The matrix, compiled in so the packaged crate's own `cargo test` reads it.
const MATRIX: &str = include_str!("fit/lib.txt");

/// A row's idiom file, compiled in for the same reason.
macro_rules! snippet {
    ($id:literal) => {
        ($id, include_str!(concat!("fit/lib/", $id, ".rs")))
    };
}

const SNIPPETS: &[(&str, &str)] = &[
    snippet!("enc-writer"),
    snippet!("dec-reader"),
    snippet!("enc-reader"),
    snippet!("dec-writer"),
    snippet!("finish-returns-writer"),
    snippet!("copy-encode"),
    snippet!("one-shot-compress"),
    snippet!("one-shot-decompress"),
    snippet!("output-bound"),
    snippet!("decode-to-writer"),
    snippet!("level-argument"),
    snippet!("error-std"),
    snippet!("error-into-io"),
    snippet!("send-sync"),
];

/// A directory removed on drop, so a failed probe leaves no litter.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("mothergod-fit-lib-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("create scratch dir");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The newest `libmothergod-*.rlib` beside this test binary: the one the
/// `cargo test` that started it just built. `None` where Cargo lays the
/// build out some other way.
fn built_crate() -> Option<(PathBuf, PathBuf)> {
    let exe = std::env::current_exe().ok()?;
    let deps = exe.parent()?.to_path_buf();
    let rlib = fs::read_dir(&deps)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libmothergod-") && name.ends_with(".rlib")
        })
        .max_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok())?
        .path();
    Some((deps, rlib))
}

fn rustc() -> OsString {
    std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into())
}

/// The triple `cargo test --target <triple>` built for, or `None` on a host
/// build. A cross build lays out `<target_dir>/<triple>/<profile>/deps` where a
/// host build has `<target_dir>/<profile>/deps`, so the candidate is the name
/// two levels above `deps`; only a name `rustc` lists as a target counts, so a
/// target directory that happens to be called `musl` is never passed as one.
/// A probe against a cross rlib needs the same `--target`, or `rustc` looks for
/// a host-triple crate and refuses with E0461.
fn cross_target(deps: &Path) -> Option<&str> {
    static TARGETS: OnceLock<String> = OnceLock::new();
    let candidate = deps.parent()?.parent()?.file_name()?.to_str()?;
    let listed = TARGETS.get_or_init(|| {
        let output = Command::new(rustc())
            .args(["--print", "target-list"])
            .output()
            .expect("run rustc --print target-list");
        String::from_utf8_lossy(&output.stdout).into_owned()
    });
    listed
        .lines()
        .any(|line| line == candidate)
        .then_some(candidate)
}

/// `rustc`'s verdict on `source` as a library that links the built crate, or
/// the diagnostics when it refuses.
fn compile(name: &str, source: &str) -> Option<Result<(), String>> {
    let (deps, rlib) = built_crate()?;
    let dir = Scratch::new(name);
    let file = dir.0.join("probe.rs");
    fs::write(&file, source).expect("write probe source");
    let mut command = Command::new(rustc());
    if let Some(triple) = cross_target(&deps) {
        command.args(["--target", triple]);
    }
    let output = command
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
        ])
        .arg("--crate-name")
        .arg("fit_probe")
        .arg("--out-dir")
        .arg(&dir.0)
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("mothergod={}", rlib.display()))
        .arg(&file)
        .output()
        .ok()?;
    Some(if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    })
}

/// Whether `compile` can be trusted here, measured, not assumed: a snippet
/// that must link, because a `rustc` that cannot read the crate refuses every
/// idiom, which is what a gap looks like.
fn compile_works() -> bool {
    static WORKS: OnceLock<bool> = OnceLock::new();
    *WORKS.get_or_init(|| {
        let control = "pub fn control() -> Vec<u8> { mothergod::compress(b\"\") }\n";
        match compile("control", control) {
            None => false,
            Some(Ok(())) => true,
            Some(Err(why)) => {
                panic!("rustc cannot link the built crate, so no row is measured:\n{why}")
            }
        }
    })
}

fn idiom_compiles(id: &str) -> Seen {
    if !compile_works() {
        return Seen::Unmeasured;
    }
    let (_, source) = SNIPPETS
        .iter()
        .find(|(snippet_id, _)| *snippet_id == id)
        .unwrap_or_else(|| panic!("row {id} has no file in tests/fit/lib/"));
    match compile(id, source) {
        Some(verdict) => seen(verdict.is_ok()),
        None => Seen::Unmeasured,
    }
}

macro_rules! idiom_probes {
    ($($id:literal),* $(,)?) => {
        [$(($id, (|| idiom_compiles($id)) as Probe)),*]
    };
}

fn sample() -> Vec<u8> {
    (0..2000u32).map(|i| (i * 37 % 251) as u8).collect()
}

fn other_sample() -> Vec<u8> {
    (0..1500u32).map(|i| (i * 11 % 241) as u8).collect()
}

fn empty_roundtrip() -> Seen {
    seen(mothergod::decompress(&mothergod::compress(b"")) == Ok(Vec::new()))
}

fn concatenation() -> Seen {
    let mut frames = mothergod::compress(&sample());
    frames.extend(mothergod::compress(&other_sample()));
    let mut both = sample();
    both.extend(other_sample());
    seen(mothergod::decompress(&frames) == Ok(both))
}

/// 64 bytes no compressor shrinks, so `compress` falls back to `Stored`.
fn incompressible() -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    (0..64)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[3]
        })
        .collect()
}

/// The corpus the row names: one `Lz` frame and one `Stored` frame, every
/// single-bit flip and every truncation of each. The row holds when none
/// decodes to other bytes without an error.
fn integrity_check() -> Seen {
    let frames = [
        (mothergod::Method::Lz, sample()),
        (mothergod::Method::Stored, incompressible()),
    ];
    let mut silent = 0usize;
    for (method, payload) in frames {
        assert_eq!(
            mothergod::Method::try_from(mothergod::compress(&payload)[5]),
            Ok(method),
            "the corpus must hold one frame of each method"
        );
        silent += integrity::probe(&payload, usize::MAX).silent;
    }
    seen(silent == 0)
}

/// One probe per row that is not declined.
fn probes() -> Vec<(&'static str, Probe)> {
    let mut probes: Vec<(&'static str, Probe)> = idiom_probes![
        "enc-writer",
        "dec-reader",
        "enc-reader",
        "dec-writer",
        "finish-returns-writer",
        "copy-encode",
        "one-shot-compress",
        "one-shot-decompress",
        "output-bound",
        "decode-to-writer",
        "level-argument",
        "error-std",
        "error-into-io",
        "send-sync",
    ]
    .into();
    probes.extend([
        ("empty-roundtrip", empty_roundtrip as Probe),
        ("concatenation", concatenation),
        ("integrity-check", integrity_check),
    ]);
    probes
}

#[test]
fn every_row_is_probed_exactly_once_and_declined_rows_are_not() {
    matrix::check_probes_cover_rows("tests/fit/lib.txt", MATRIX, &probes());
}

#[test]
fn every_idiom_file_belongs_to_a_row() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fit/lib");
    let mut on_disk: Vec<String> = fs::read_dir(dir)
        .expect("tests/fit/lib")
        .map(|entry| {
            let name = entry.expect("dir entry").file_name();
            name.to_string_lossy().trim_end_matches(".rs").to_owned()
        })
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = SNIPPETS.iter().map(|(id, _)| (*id).to_owned()).collect();
    listed.sort();
    assert_eq!(
        on_disk, listed,
        "SNIPPETS and tests/fit/lib/ must list the same files"
    );
}

#[test]
fn the_record_matches_the_crate() {
    matrix::check_record("tests/fit/lib.txt", MATRIX, &probes());
}
