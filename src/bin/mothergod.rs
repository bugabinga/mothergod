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
//! `compress` still builds the whole output [`Vec<u8>`] before writing it
//! ([`mothergod::compress`]'s own whole-buffer signature; the optimal-parse
//! encoder needs the full input regardless). `decompress` writes its output
//! incrementally via [`mothergod::decompress_to_writer`] instead, bounding
//! resident memory on the output side (ROADMAP M4's bounded-memory decode
//! guarantee) even for a small frame that decodes to a much larger buffer.
//! Input is still read whole into memory either way: the library has a
//! streaming *writer*, not yet a streaming *reader*.
//!
//! Streaming trades away the old all-or-nothing output guarantee for a
//! corrupt frame. To stdout, a decode failure now surfaces after whatever
//! prefix `decompress_to_writer` already emitted, so a pipeline consuming
//! incrementally (`| tar x`) sees truncated bytes ahead of the nonzero exit
//! code, where a whole-buffer decode used to guarantee zero bytes reached
//! stdout before the error. To a file argument, the same failure instead
//! removes the partial file `create_new` had to open before decoding could
//! start, so the on-disk case still sees nothing survive a failed run.
//! Stdout has no equivalent mitigation: bytes already written cannot be
//! unwritten.

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
        Some("compress") => run_compress(path.as_ref()),
        Some("decompress") => run_decompress(path.as_ref()),
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

/// Reads input (stdin, or `path` if given), compresses it, and writes the
/// result (stdout, or `path` with [`SUFFIX`] appended if given).
fn run_compress(path: Option<&OsString>) -> ExitCode {
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

/// Like [`run_compress`], but streams the decoded bytes straight to the
/// destination writer via [`mothergod::decompress_to_writer`] instead of
/// collecting them into a `Vec<u8>` first, bounding resident memory on the
/// output side even when a small frame decodes to a much larger buffer.
/// Not built on [`run_compress`]'s shape: that function collects a whole
/// `Vec<u8>` for its caller to write, which is exactly the buffering this
/// function exists to avoid.
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

/// Runs [`mothergod::decompress_to_writer`] into `writer`, telling a write
/// failure (reported with `dest_name`) apart from a decode failure (a
/// corrupt or oversized frame, reported plainly): the two arrived through
/// separate calls before streaming merged them into one `Result`, and
/// callers still want the old distinction in the error message.
fn decompress_into<W: Write>(input: &[u8], writer: &mut W, dest_name: &str) -> ExitCode {
    match mothergod::decompress_to_writer(input, u32::MAX, writer) {
        Ok(()) => ExitCode::SUCCESS,
        Err(mothergod::WriteError::Decode(decode_err)) => fail(&decode_err.to_string()),
        Err(mothergod::WriteError::Io(err)) => fail(&format!("writing {dest_name}: {err}")),
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

/// Reads stdin, or `path` if given. Shared by both subcommands' entry
/// points, which otherwise differ only in how they use the result.
fn read_input(path: Option<&OsString>) -> Result<Vec<u8>, String> {
    match path {
        None => read_stdin(),
        Some(path) => {
            fs::read(path).map_err(|err| format!("reading {}: {err}", Path::new(path).display()))
        }
    }
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

fn fail(message: &str) -> ExitCode {
    eprintln!("mothergod: {message}");
    ExitCode::FAILURE
}
