#![forbid(unsafe_code)]
//! `mothergod` — command-line compressor/decompressor (ROADMAP M6).
//!
//! Reads the whole input into memory, chosen by a `compress`/`decompress`
//! subcommand. With no file argument it uses stdin/stdout, mirroring the
//! `gzip -c`/`zstd -c` shape users already know. With a file argument it
//! follows the `.mgdc` suffix convention already used by `tests/golden/`,
//! and never deletes the input or overwrites an existing output file:
//!
//! ```text
//! mothergod compress   < input       > input.mgdc   # stdin/stdout
//! mothergod decompress < input.mgdc  > input
//!
//! mothergod compress   input                        # writes input.mgdc
//! mothergod decompress input.mgdc                    # writes input
//! ```
//!
//! `compress` builds its whole output before writing it: the optimal-parse
//! encoder needs the full input. `decompress` streams through
//! [`mothergod::decompress_to_writer`], which bounds resident memory on the
//! output side (ROADMAP M4) only for the frame kinds its doc lists; the rest
//! decode whole first. Input is read whole either way.
//!
//! A decode failure to stdout surfaces after whatever prefix was already
//! written: `| tar x` sees truncated bytes ahead of the nonzero exit code.
//! To a file argument, the partial file is removed instead.
//!
//! `compress` with no file argument refuses to write its frame to a
//! terminal, as gzip, bzip2, xz and zstd do. `decompress` does not: its
//! output is the user's own bytes, and `zcat` shows them.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Suffix a compressed file carries, matching `tests/golden/*.mgdc`.
const SUFFIX: &str = ".mgdc";

const USAGE: &str = "usage: mothergod <compress|decompress> [file]\n\n\
With no file, reads stdin and writes stdout.\n\
With a file, compress writes <file>.mgdc; decompress reads a .mgdc file and writes <file> \
(the name with the suffix stripped). Never overwrites an existing output file.";

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let command = args.next();
    let path = args.next();
    if args.next().is_some() {
        return usage_fail("too many arguments");
    }

    match command.as_deref().and_then(OsStr::to_str) {
        Some("compress") => run_compress(path.as_ref()),
        Some("decompress") => run_decompress(path.as_ref()),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!(
                "mothergod {} (format {})",
                env!("CARGO_PKG_VERSION"),
                mothergod::FORMAT_VERSION
            );
            ExitCode::SUCCESS
        }
        Some(other) => usage_fail(&format!("unknown command {other:?}")),
        None => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// [`fail`] with [`USAGE`] appended.
fn usage_fail(message: &str) -> ExitCode {
    fail(&format!("{message}\n\n{USAGE}"))
}

/// Reads input (stdin, or `path` if given), compresses it, and writes the
/// result (stdout, or `path` with [`SUFFIX`] appended if given).
fn run_compress(path: Option<&OsString>) -> ExitCode {
    if path.is_none() && io::stdout().is_terminal() {
        return fail("compressed data not written to a terminal");
    }
    let input = match read_input(path) {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };
    let output = mothergod::compress(&input);
    match path {
        None => write_stdout(&output),
        Some(path) => write_new_file(&add_suffix(Path::new(path)), &output),
    }
}

/// Reads input, then streams the decoded bytes to stdout, or to `path` with
/// [`SUFFIX`] stripped if given.
fn run_decompress(path: Option<&OsString>) -> ExitCode {
    let input = match read_input(path) {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };

    match path {
        None => decompress_into(&input, &mut io::stdout().lock(), "stdout"),
        Some(path) => {
            let out_path = match strip_suffix(Path::new(path)) {
                Ok(out_path) => out_path,
                Err(err) => return fail(&err),
            };
            write_new_file_with(&out_path, |file| {
                decompress_into(&input, file, &out_path.display().to_string())
            })
        }
    }
}

/// Runs [`mothergod::decompress_to_writer`] into `writer`: a write failure
/// is reported with `dest_name`, a decode failure plainly.
fn decompress_into<W: Write>(input: &[u8], writer: &mut W, dest_name: &str) -> ExitCode {
    match mothergod::decompress_to_writer(input, u32::MAX, writer) {
        Ok(()) => ExitCode::SUCCESS,
        Err(mothergod::WriteError::Decode(decode_err)) => fail(&decode_err.to_string()),
        Err(mothergod::WriteError::Io(err)) => write_fail(dest_name, &err),
    }
}

fn add_suffix(path: &Path) -> PathBuf {
    let mut out = path.as_os_str().to_owned();
    out.push(SUFFIX);
    out.into()
}

fn strip_suffix(path: &Path) -> Result<PathBuf, String> {
    let name = path.to_str().ok_or_else(|| {
        format!(
            "{}: not valid UTF-8, cannot derive an output name",
            path.display()
        )
    })?;
    name.strip_suffix(SUFFIX)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{}: expected a {SUFFIX} file", path.display()))
}

/// Reads stdin, or `path` if given.
fn read_input(path: Option<&OsString>) -> Result<Vec<u8>, String> {
    match path {
        None => {
            let mut buf = Vec::new();
            io::stdin()
                .lock()
                .read_to_end(&mut buf)
                .map_err(|err| format!("reading stdin: {err}"))?;
            Ok(buf)
        }
        Some(path) => {
            fs::read(path).map_err(|err| format!("reading {}: {err}", Path::new(path).display()))
        }
    }
}

fn write_stdout(bytes: &[u8]) -> ExitCode {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => write_fail("stdout", &err),
    }
}

/// Writes `bytes` to `path`, refusing to clobber an existing file (`compress`
/// re-run over an already-compressed file, or a `decompress` target that
/// already exists, must not silently destroy it).
fn write_new_file(path: &Path, bytes: &[u8]) -> ExitCode {
    write_new_file_with(path, |file| match file.write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => fail(&format!("writing {}: {err}", path.display())),
    })
}

/// Opens `path` as a new file, refusing to clobber an existing one, then
/// runs `write` against it. Shared by [`write_new_file`] and
/// [`run_decompress`]'s file branch, which otherwise each open the file and
/// clean up after a failed write identically.
///
/// A write failure after `create_new` succeeded (disk full, interrupted, a
/// corrupt decode) removes the partial file: otherwise it survives as a
/// corrupt file that `create_new` refuses to retry over, permanently
/// blocking a re-run.
fn write_new_file_with(path: &Path, write: impl FnOnce(&mut File) -> ExitCode) -> ExitCode {
    let mut file = match File::options().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(err) => return fail(&format!("writing {}: {err}", path.display())),
    };
    let code = write(&mut file);
    if code == ExitCode::FAILURE {
        let _ = fs::remove_file(path);
    }
    code
}

/// Exit status of a process killed by SIGPIPE: 128 + 13. Rust starts with
/// SIGPIPE ignored, so the CLI reproduces gzip's status by hand rather than
/// restoring the signal's default disposition (that takes `libc` or unsafe,
/// both barred from the crate).
const SIGPIPE_STATUS: u8 = 141;

/// Reports a failed write to `dest_name`. A [`io::ErrorKind::BrokenPipe`] is
/// the reader going away (`| head`, `| less` quit early), not a fault of this
/// run: it exits silently with [`SIGPIPE_STATUS`], as gzip does. The kind
/// also covers Windows' `ERROR_BROKEN_PIPE` and `ERROR_NO_DATA`. Only a
/// pipe raises it, so a file destination never takes this arm and
/// [`write_new_file_with`]'s cleanup is not skipped by it.
fn write_fail(dest_name: &str, err: &io::Error) -> ExitCode {
    if err.kind() == io::ErrorKind::BrokenPipe {
        return ExitCode::from(SIGPIPE_STATUS);
    }
    fail(&format!("writing {dest_name}: {err}"))
}

fn fail(message: &str) -> ExitCode {
    eprintln!("mothergod: {message}");
    ExitCode::FAILURE
}
