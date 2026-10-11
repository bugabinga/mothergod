#![forbid(unsafe_code)]
//! `mothergod` — command-line compressor/decompressor (ROADMAP M6), in the
//! grammar gzip, bzip2, xz, zstd, brotli and lz4 share (ADR-0066): it
//! compresses, `-d` decompresses, `-c` writes to stdout. With no file operand,
//! or the operand `-`, it reads stdin and writes stdout. A file operand
//! follows the `.mgdc` suffix convention already used by `tests/golden/`, and
//! the run never deletes the input or overwrites an existing output file:
//!
//! ```text
//! mothergod    < input       > input.mgdc   # stdin/stdout
//! mothergod -d < input.mgdc  > input
//!
//! mothergod input input2                    # writes input.mgdc, input2.mgdc
//! mothergod -d input.mgdc                   # writes input
//! mothergod -dc input.mgdc | tar x          # writes stdout, keeps input.mgdc
//! ```
//!
//! Operands are processed in order. A failure on one is reported and the run
//! goes on to the next, then exits 1; a reader that went away ends the run at
//! once with status 141.
//!
//! Compression builds its whole output before writing it: the optimal-parse
//! encoder needs the full input. Decompression streams through
//! [`mothergod::decompress_to_writer`], which bounds resident memory on the
//! output side (ROADMAP M4) for every frame kind except a `Transpose`-filtered
//! one, which decodes whole first. Input is read whole either way.
//!
//! A decode failure to stdout surfaces after whatever prefix was already
//! written: `| tar x` sees truncated bytes ahead of the nonzero exit code.
//! To a file, the partial file is removed instead.
//!
//! A file output takes the input file's modification time and, on Unix,
//! its permission bits, as gzip does; decompression reads them from the
//! `.mgdc` file, the frame carries no timestamp.
//!
//! Compression to stdout refuses a terminal, as gzip, bzip2, xz and zstd
//! do. Decompression does not: its output is the user's own bytes, and
//! `zcat` shows them.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Suffix a compressed file carries, matching `tests/golden/*.mgdc`.
const SUFFIX: &str = ".mgdc";

const USAGE: &str = "usage: mothergod [-d] [-c] [--] [file ...]\n       \
mothergod <-h|--help|-V|--version>\n\n\
Compresses; -d (--decompress) decompresses. -c (--stdout) writes to stdout.\n\
With no file, or the file -, reads stdin and writes stdout.\n\
With a file, compression writes <file>.mgdc; decompression reads a .mgdc file and writes <file> \
(the name with the suffix stripped). Never overwrites an existing output file.";

/// Which way a run converts.
#[derive(Clone, Copy)]
enum Mode {
    Compress,
    Decompress,
}

/// What the command line asked for.
enum Request {
    Help,
    Version,
    Run {
        mode: Mode,
        to_stdout: bool,
        /// Empty means stdin to stdout; `-` is already folded to `None`.
        operands: Vec<Option<PathBuf>>,
    },
}

fn main() -> ExitCode {
    match parse(std::env::args_os().skip(1)) {
        Ok(Request::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Request::Version) => {
            println!(
                "mothergod {} (format {})",
                env!("CARGO_PKG_VERSION"),
                mothergod::FORMAT_VERSION
            );
            ExitCode::SUCCESS
        }
        Ok(Request::Run {
            mode,
            to_stdout,
            operands,
        }) => run(mode, to_stdout, &operands),
        Err(message) => fail(&format!("{message}\n\n{USAGE}")),
    }
}

/// Reads the genre's grammar: option clusters (`-dc`), long options, `--` to
/// end options, and `-` as an operand naming stdin. `-h` and `-V` answer at
/// once, wherever they stand.
fn parse(args: impl Iterator<Item = OsString>) -> Result<Request, String> {
    let mut mode = Mode::Compress;
    let mut to_stdout = false;
    let mut operands = Vec::new();
    let mut options_ended = false;
    for arg in args {
        let option = arg.to_str().filter(|text| !options_ended && text.len() > 1);
        let Some(text) = option.and_then(|text| text.strip_prefix('-')) else {
            operands.push(if arg == "-" && !options_ended {
                None
            } else {
                Some(PathBuf::from(arg))
            });
            continue;
        };
        if let Some(long) = text.strip_prefix('-') {
            match long {
                "" => options_ended = true,
                "decompress" => mode = Mode::Decompress,
                "stdout" => to_stdout = true,
                "help" => return Ok(Request::Help),
                "version" => return Ok(Request::Version),
                _ => return Err(format!("unknown option \"--{long}\"")),
            }
            continue;
        }
        for flag in text.chars() {
            match flag {
                'd' => mode = Mode::Decompress,
                'c' => to_stdout = true,
                'h' => return Ok(Request::Help),
                'V' => return Ok(Request::Version),
                _ => return Err(format!("unknown option \"-{flag}\"")),
            }
        }
    }
    Ok(Request::Run {
        mode,
        to_stdout,
        operands,
    })
}

/// Processes each operand in order, or stdin when there is none. A failed
/// operand does not stop the rest; the run exits 1 if any failed.
fn run(mode: Mode, to_stdout: bool, operands: &[Option<PathBuf>]) -> ExitCode {
    let stdin_only = [None];
    let operands = if operands.is_empty() {
        &stdin_only
    } else {
        operands
    };
    let writes_stdout = to_stdout || operands.iter().any(Option::is_none);
    if matches!(mode, Mode::Compress) && writes_stdout && io::stdout().is_terminal() {
        return fail("compressed data not written to a terminal");
    }
    let mut status = ExitCode::SUCCESS;
    for operand in operands {
        let code = process(mode, operand.as_deref(), to_stdout);
        if code == ExitCode::from(SIGPIPE_STATUS) {
            return code;
        }
        if code != ExitCode::SUCCESS {
            status = ExitCode::FAILURE;
        }
    }
    status
}

/// Converts one input, stdin when `operand` is `None`, to stdout when there
/// is no operand or `to_stdout` is set, else to the file named from `operand`
/// by [`add_suffix`] or [`strip_suffix`].
fn process(mode: Mode, operand: Option<&Path>, to_stdout: bool) -> ExitCode {
    let target = match operand.filter(|_| !to_stdout) {
        Some(source) => match output_name(mode, source) {
            Ok(out_path) => Some((source, out_path)),
            Err(err) => return fail(&err),
        },
        None => None,
    };
    let input = match read_input(operand) {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };
    let input_name = operand.map_or_else(|| "stdin".to_owned(), |path| path.display().to_string());
    match (mode, target) {
        (Mode::Compress, None) => write_stdout(&mothergod::compress(&input)),
        (Mode::Compress, Some((source, out_path))) => {
            write_new_file(&out_path, &mothergod::compress(&input), source)
        }
        (Mode::Decompress, None) => {
            decompress_into(&input, &mut io::stdout().lock(), "stdout", &input_name)
        }
        (Mode::Decompress, Some((source, out_path))) => {
            write_new_file_with(&out_path, source, |file| {
                decompress_into(&input, file, &out_path.display().to_string(), &input_name)
            })
        }
    }
}

/// The file `mode` writes for `source`.
fn output_name(mode: Mode, source: &Path) -> Result<PathBuf, String> {
    match mode {
        Mode::Compress => Ok(add_suffix(source)),
        Mode::Decompress => strip_suffix(source),
    }
}

/// Runs [`mothergod::decompress_to_writer`] into `writer`: a write failure
/// is reported with `dest_name`, a decode failure with `input_name`, which
/// several operands make the only clue to which input failed.
fn decompress_into<W: Write>(
    input: &[u8],
    writer: &mut W,
    dest_name: &str,
    input_name: &str,
) -> ExitCode {
    match mothergod::decompress_to_writer(input, u32::MAX, writer) {
        Ok(()) => ExitCode::SUCCESS,
        Err(mothergod::WriteError::Decode(decode_err)) => {
            fail(&format!("{input_name}: {decode_err}"))
        }
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
fn read_input(path: Option<&Path>) -> Result<Vec<u8>, String> {
    match path {
        None => {
            let mut buf = Vec::new();
            io::stdin()
                .lock()
                .read_to_end(&mut buf)
                .map_err(|err| format!("reading stdin: {err}"))?;
            Ok(buf)
        }
        Some(path) => fs::read(path).map_err(|err| format!("reading {}: {err}", path.display())),
    }
}

fn write_stdout(bytes: &[u8]) -> ExitCode {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => write_fail("stdout", &err),
    }
}

/// Writes `bytes` to `path`, refusing to clobber an existing file (a compression
/// re-run over an already-compressed file, or a decompression target that
/// already exists, must not silently destroy it).
fn write_new_file(path: &Path, bytes: &[u8], source: &Path) -> ExitCode {
    write_new_file_with(path, source, |file| match file.write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => fail(&format!("writing {}: {err}", path.display())),
    })
}

/// Opens `path` as a new file, refusing to clobber an existing one, then
/// runs `write` against it. Shared by [`write_new_file`] and
/// [`process`]'s decompression branch, which otherwise each open the file and
/// clean up after a failed write identically.
///
/// A write failure after `create_new` succeeded (disk full, interrupted, a
/// corrupt decode) removes the partial file: otherwise it survives as a
/// corrupt file that `create_new` refuses to retry over, permanently
/// blocking a re-run.
///
/// After a successful write the file takes `source`'s modification time and
/// permission bits ([`copy_metadata`]), as gzip, bzip2 and xz do.
fn write_new_file_with(
    path: &Path,
    source: &Path,
    write: impl FnOnce(&mut File) -> ExitCode,
) -> ExitCode {
    let mut file = match File::options().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(err) => return fail(&format!("writing {}: {err}", path.display())),
    };
    let code = write(&mut file);
    if code == ExitCode::FAILURE {
        let _ = fs::remove_file(path);
    } else if let Err(err) = copy_metadata(source, &file) {
        // The bytes are complete and correct; a stamp that did not take is
        // not worth discarding them, and `create_new` would refuse a retry.
        eprintln!(
            "mothergod: warning: {}: copying metadata from {}: {err}",
            path.display(),
            source.display()
        );
    }
    code
}

/// Gives `dest` the modification time of `source` and, on Unix, its
/// permission bits `rwxrwxrwx`. Called after the last write, so the stamp is
/// not overwritten by it. The mode half is Unix only: Windows has a read-only
/// flag where Unix has bits, and copying that flag is not the genre's mode.
///
/// The setuid, setgid and sticky bits are not copied. The output is owned by
/// the invoking user, not the source's owner, so carrying them would let
/// decompressing a hostile `.mgdc` mint a setuid file owned by that user.
fn copy_metadata(source: &Path, dest: &File) -> io::Result<()> {
    let meta = fs::metadata(source)?;
    dest.set_modified(meta.modified()?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let rwx = meta.permissions().mode() & 0o777;
        dest.set_permissions(fs::Permissions::from_mode(rwx))?;
    }
    Ok(())
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
