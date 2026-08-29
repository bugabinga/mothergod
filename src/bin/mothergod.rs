#![forbid(unsafe_code)]
//! `mothergod` — command-line compressor/decompressor (ROADMAP M6).
//!
//! Reads the whole input and writes the whole output, chosen by a
//! `compress`/`decompress` subcommand. With no file argument it uses
//! stdin/stdout, mirroring the `gzip -c`/`zstd -c` shape users already
//! know. With a file argument it follows the `.mgdc` suffix convention
//! already used by `tests/golden/`, and never deletes the input or
//! overwrites an existing output file:
//!
//! ```text
//! mothergod compress   < input       > input.mgdc   # stdin/stdout
//! mothergod decompress < input.mgdc  > input
//!
//! mothergod compress   input                        # writes input.mgdc
//! mothergod decompress input.mgdc                    # writes input
//! ```
//!
//! Streaming I/O is follow-on scope, not built here. Buffering the whole
//! input in memory before transforming it matches
//! [`mothergod::compress`]/[`mothergod::decompress`]'s own whole-buffer
//! signatures; a streaming API is ROADMAP M4 scope, not this binary's to
//! add on its own.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
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
        eprintln!("mothergod: too many arguments\n\n{USAGE}");
        return ExitCode::FAILURE;
    }

    match command.as_deref().and_then(OsStr::to_str) {
        Some("compress") => run(path.as_ref(), checked_compress, add_suffix),
        Some("decompress") => run(path.as_ref(), checked_decompress, strip_suffix),
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("mothergod: unknown command {other:?}\n\n{USAGE}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// Reads input (stdin, or `path` if given), applies `transform`, writes the
/// result (stdout, or `path` renamed by `derive_output` if given). Shared by
/// both subcommands: `transform` carries the compress/decompress difference,
/// `derive_output` carries the `.mgdc` suffix direction.
fn run(
    path: Option<&OsString>,
    transform: impl FnOnce(&[u8]) -> Result<Vec<u8>, String>,
    derive_output: impl FnOnce(&Path) -> Result<PathBuf, String>,
) -> ExitCode {
    let input = match path {
        None => read_stdin(),
        Some(path) => {
            fs::read(path).map_err(|err| format!("reading {}: {err}", Path::new(path).display()))
        }
    };
    let input = match input {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };

    let output = match transform(&input) {
        Ok(output) => output,
        Err(err) => return fail(&err),
    };

    match path {
        None => write_stdout(&output),
        Some(path) => {
            let out_path = match derive_output(Path::new(path)) {
                Ok(out_path) => out_path,
                Err(err) => return fail(&err),
            };
            write_new_file(&out_path, &output)
        }
    }
}

// Shares `run`'s `Result`-returning `transform` signature with
// `checked_decompress`, which can fail; `compress` itself cannot.
#[allow(clippy::unnecessary_wraps)]
fn checked_compress(input: &[u8]) -> Result<Vec<u8>, String> {
    Ok(mothergod::compress(input))
}

fn checked_decompress(input: &[u8]) -> Result<Vec<u8>, String> {
    mothergod::decompress(input).map_err(|err| err.to_string())
}

// Shares `run`'s `Result`-returning `derive_output` signature with
// `strip_suffix`, which can fail; appending a suffix cannot.
#[allow(clippy::unnecessary_wraps)]
fn add_suffix(path: &Path) -> Result<PathBuf, String> {
    let mut out = path.as_os_str().to_owned();
    out.push(SUFFIX);
    Ok(out.into())
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

fn read_stdin() -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    io::stdin()
        .lock()
        .read_to_end(&mut buf)
        .map_err(|err| format!("reading stdin: {err}"))?;
    Ok(buf)
}

fn write_stdout(bytes: &[u8]) -> ExitCode {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => fail(&format!("writing stdout: {err}")),
    }
}

/// Writes `bytes` to `path`, refusing to clobber an existing file (`compress`
/// re-run over an already-compressed file, or a `decompress` target that
/// already exists, must not silently destroy it).
fn write_new_file(path: &Path, bytes: &[u8]) -> ExitCode {
    let result = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => fail(&format!("writing {}: {err}", path.display())),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("mothergod: {message}");
    ExitCode::FAILURE
}
