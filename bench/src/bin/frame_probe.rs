//! Integrity probe of the reviewer's input (issue #1210): compresses each
//! file named on the command line, then decodes every single-bit flip and
//! every truncation of its frame, exhaustive below `--max-decodes` each and
//! a seeded sample across the whole frame above it. Prints one line per
//! file, `<path>: bytes <n>, flips <n>, truncations <n>, silent <n>`, and
//! exits non-zero when any mutation decodes to bytes other than the file's,
//! or a file cannot be probed. The probe is `tests/fit/integrity.rs`, the
//! same function the `integrity-check` FIT row runs, so what counts as
//! silent has one definition.
//!
//! A file longer than one block (16 MiB) is refused: `compress` cuts it
//! into several frames, and a truncation at a frame boundary is a valid
//! shorter stream, not a silent one.
//!
//! Usage: `cargo run -p mothergod-bench --release --bin frame_probe --
//! [--max-decodes <n>] <file>...`. A run is capped by decode count, and every
//! decode costs time linear in the frame, so the default of 20000 bounds a
//! file at seconds only for a small one. Measured on a release build at the
//! default: `README.md` (3492 B frame) 1.0 s, a 700 KB slice of the release
//! binary (253 KB frame) 16.8 s, 1 MB of `/dev/urandom` (1000018 B frame)
//! 64 s, so about a minute per MB. Pass `--max-decodes 18446744073709551615`
//! for every flip.

// The two items a bench binary reads are `probe` and `Report`; the
// rest is the shared module's own surface.
#[allow(dead_code)]
#[path = "../../../tests/fit/integrity.rs"]
mod integrity;

use std::path::Path;
use std::process::ExitCode;

/// Largest file probed: `compress`'s block, one frame.
const MAX_FILE_LEN: usize = 16 * 1024 * 1024;

/// Decodes per mutation kind when `--max-decodes` is absent.
const DEFAULT_MAX_DECODES: usize = 20_000;

const USAGE: &str = "usage: frame_probe [--max-decodes <n>] <file>...";

fn probe_file(path: &Path, cap: usize) -> Result<integrity::Report, String> {
    let payload = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    if payload.len() > MAX_FILE_LEN {
        return Err(format!(
            "{}: {} bytes is over one block ({MAX_FILE_LEN}), which is several frames",
            path.display(),
            payload.len()
        ));
    }
    Ok(integrity::probe(&payload, cap))
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).peekable();
    let mut cap = DEFAULT_MAX_DECODES;
    if args.next_if(|arg| arg == "--max-decodes").is_some() {
        let Some(Ok(n)) = args.next().map(|n| n.parse()).filter(|n| *n != Ok(0)) else {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        };
        cap = n;
    }
    if args.peek().is_none() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for path in args {
        match probe_file(Path::new(&path), cap) {
            Ok(report) => {
                println!(
                    "{path}: bytes {}, flips {}, truncations {}, silent {}",
                    report.bytes, report.flips, report.truncations, report.silent
                );
                failed |= report.silent != 0;
            }
            Err(err) => {
                eprintln!("{err}");
                failed = true;
            }
        }
    }
    ExitCode::from(u8::from(failed))
}
