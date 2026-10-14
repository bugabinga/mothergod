#![forbid(unsafe_code)]
//! `mothergod` — command-line compressor/decompressor (ROADMAP M6), in the
//! grammar gzip, bzip2, xz, zstd, brotli and lz4 share (ADR-0066): it
//! compresses, `-d` decompresses, `-c` writes to stdout. With no file operand,
//! or the operand `-`, it reads stdin and writes stdout. A file operand
//! follows the `.mgdc` suffix convention already used by `tests/golden/`, and
//! the run never deletes the input, nor overwrites an existing output file
//! unless `-f` says to. `-k` is accepted and does nothing, because the input
//! is kept anyway. `-t` decodes without writing anything, so a silent exit 0
//! says the file is intact (ADR-0066). `-v` reports each input's sizes on
//! stderr and leaves stdout alone:
//!
//! ```text
//! mothergod    < input       > input.mgdc   # stdin/stdout
//! mothergod -d < input.mgdc  > input
//!
//! mothergod input input2                    # writes input.mgdc, input2.mgdc
//! mothergod -d input.mgdc                   # writes input
//! mothergod -dc input.mgdc | tar x          # writes stdout, keeps input.mgdc
//! mothergod -f input                        # replaces input.mgdc
//! mothergod -t input.mgdc                   # writes nothing, exits 0 if intact
//! mothergod -v input                        # also says: input: 2000 -> 640 bytes (32.0%)
//! ```
//!
//! Operands are processed in order. A failure on one is reported and the run
//! goes on to the next, then exits 1; a reader that went away ends the run at
//! once with status 141.
//!
//! Compression streams through [`mothergod::copy_encode`], one 16 MiB block
//! and one frame at a time, so its memory is bounded by the block and not by
//! the input. Decompression streams through
//! [`mothergod::decompress_to_writer`], which bounds resident memory on the
//! output side (ROADMAP M4) for every frame kind except a `Transpose`-filtered
//! one, which decodes whole first. Decompression reads its input whole.
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

const USAGE: &str = "usage: mothergod [-d] [-c] [-k] [-f] [-t] [-v] [--] [file ...]\n       \
mothergod <-h|--help|-V|--version>\n\n\
Compresses; -d (--decompress) decompresses. -c (--stdout) writes to stdout.\n\
-k (--keep) keeps the input, which is the default. -f (--force) overwrites an existing output file.\n\
-t (--test) checks that each input decompresses and writes nothing.\n\
-v (--verbose) reports each input's sizes on stderr.\n\
With no file, or the file -, reads stdin and writes stdout.\n\
With a file, compression writes <file>.mgdc; decompression reads a .mgdc file and writes <file> \
(the name with the suffix stripped). Never overwrites an existing output file without -f.";

/// Which way a conversion goes.
#[derive(Clone, Copy)]
enum Direction {
    Compress,
    Decompress,
}

/// What a run does with each input.
#[derive(Clone, Copy)]
enum Mode {
    Convert(Direction),
    /// Decodes and discards, so the exit status alone reports the input's health.
    Test,
}

/// What a file output does when its path already exists.
#[derive(Clone, Copy)]
enum Existing {
    Refuse,
    Replace,
}

/// What every operand of a run shares, apart from its mode.
#[derive(Clone, Copy)]
struct Options {
    to_stdout: bool,
    existing: Existing,
    /// Reports each input's sizes on stderr once it converted.
    verbose: bool,
}

/// What the command line asked for.
enum Request {
    Help,
    Version,
    Run {
        mode: Mode,
        options: Options,
        /// Empty means stdin to stdout; `-` is already folded to `None`.
        operands: Vec<Option<PathBuf>>,
    },
}

fn main() -> ExitCode {
    match parse(std::env::args_os().skip(1)) {
        Ok(Request::Help) => print_line(USAGE),
        Ok(Request::Version) => print_line(&format!(
            "mothergod {} (format {})",
            env!("CARGO_PKG_VERSION"),
            mothergod::FORMAT_VERSION
        )),
        Ok(Request::Run {
            mode,
            options,
            operands,
        }) => run(mode, options, &operands),
        Err(message) => fail(&format!("{message}\n\n{USAGE}")),
    }
}

/// Reads the genre's grammar: option clusters (`-dc`), long options, `--` to
/// end options, and `-` as an operand naming stdin. `-h` and `-V` answer at
/// once, wherever they stand. `-k` is accepted and changes nothing: the
/// input is kept without it.
fn parse(args: impl Iterator<Item = OsString>) -> Result<Request, String> {
    let mut direction = Direction::Compress;
    let mut test_only = false;
    let mut to_stdout = false;
    let mut existing = Existing::Refuse;
    let mut verbose = false;
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
                "decompress" => direction = Direction::Decompress,
                "test" => test_only = true,
                "stdout" => to_stdout = true,
                "keep" => {}
                "force" => existing = Existing::Replace,
                "verbose" => verbose = true,
                "help" => return Ok(Request::Help),
                "version" => return Ok(Request::Version),
                _ => return Err(format!("unknown option \"--{long}\"")),
            }
            continue;
        }
        for flag in text.chars() {
            match flag {
                'd' => direction = Direction::Decompress,
                't' => test_only = true,
                'c' => to_stdout = true,
                'k' => {}
                'f' => existing = Existing::Replace,
                'v' => verbose = true,
                'h' => return Ok(Request::Help),
                'V' => return Ok(Request::Version),
                _ => return Err(format!("unknown option \"-{flag}\"")),
            }
        }
    }
    Ok(Request::Run {
        // `-t` wins over `-d` wherever each stands, as in gzip.
        mode: if test_only {
            Mode::Test
        } else {
            Mode::Convert(direction)
        },
        options: Options {
            to_stdout,
            existing,
            verbose,
        },
        operands,
    })
}

/// Processes each operand in order, or stdin when there is none. A failed
/// operand does not stop the rest; the run exits 1 if any failed.
fn run(mode: Mode, options: Options, operands: &[Option<PathBuf>]) -> ExitCode {
    let stdin_only = [None];
    let operands = if operands.is_empty() {
        &stdin_only
    } else {
        operands
    };
    let writes_stdout = options.to_stdout || operands.iter().any(Option::is_none);
    if matches!(mode, Mode::Convert(Direction::Compress))
        && writes_stdout
        && io::stdout().is_terminal()
    {
        return fail("compressed data not written to a terminal");
    }
    let mut status = ExitCode::SUCCESS;
    for operand in operands {
        let operand = operand.as_deref();
        let code = match mode {
            Mode::Convert(direction) => convert(direction, operand, options),
            Mode::Test => test(operand, options.verbose),
        };
        if code == ExitCode::from(SIGPIPE_STATUS) {
            return code;
        }
        if code != ExitCode::SUCCESS {
            status = ExitCode::FAILURE;
        }
    }
    status
}

/// Decodes one input to nowhere: it names no output file, so the input needs
/// no suffix, and nothing is written whether it decodes or not.
fn test(operand: Option<&Path>, verbose: bool) -> ExitCode {
    let input = match read_input(operand) {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };
    decompress_into(&input, io::sink(), "nowhere", &input_name(operand), verbose)
}

/// What a message calls an input.
fn input_name(operand: Option<&Path>) -> String {
    operand.map_or_else(|| "stdin".to_owned(), |path| path.display().to_string())
}

/// Converts one input, stdin when `operand` is `None`, to stdout when there
/// is no operand or `options.to_stdout` is set, else to the file named from
/// `operand` by [`add_suffix`] or [`strip_suffix`].
fn convert(direction: Direction, operand: Option<&Path>, options: Options) -> ExitCode {
    let Options {
        to_stdout,
        existing,
        verbose,
    } = options;
    let target = match operand.filter(|_| !to_stdout) {
        Some(source) => match output_name(direction, source) {
            Ok(out_path) => Some((source, out_path)),
            Err(err) => return fail(&err),
        },
        None => None,
    };
    let input_name = input_name(operand);
    if matches!(direction, Direction::Compress) {
        let reader = match open_input(operand) {
            Ok(reader) => reader,
            Err(err) => return fail(&err),
        };
        return write_to(target, existing, |sink, dest_name| {
            compress_into(reader, sink, dest_name, &input_name, verbose)
        });
    }
    let input = match read_input(operand) {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };
    write_to(target, existing, |sink, dest_name| {
        decompress_into(&input, sink, dest_name, &input_name, verbose)
    })
}

/// Runs `write` against stdout when `target` is `None`, else against the new
/// file [`write_new_file_with`] makes for it, with the destination's name for
/// messages.
fn write_to(
    target: Option<(&Path, PathBuf)>,
    existing: Existing,
    write: impl FnOnce(&mut dyn Write, &str) -> ExitCode,
) -> ExitCode {
    match target {
        None => write(&mut io::stdout().lock(), "stdout"),
        Some((source, out_path)) => write_new_file_with(&out_path, source, existing, |file| {
            write(file, &out_path.display().to_string())
        }),
    }
}

/// The file `direction` writes for `source`.
fn output_name(direction: Direction, source: &Path) -> Result<PathBuf, String> {
    match direction {
        Direction::Compress => Ok(add_suffix(source)),
        Direction::Decompress => strip_suffix(source),
    }
}

/// A reader that keeps the error its inner reader raised, so a failure of
/// [`mothergod::copy_encode`] can be told to come from reading and not from
/// writing: the library returns one `io::Error` for both. An `Interrupted`
/// read is not kept: the library retries it and it never surfaces, so
/// keeping it would blame a later write failure on the read. It also counts
/// the bytes it delivered, for `-v`.
struct Tracked<R> {
    inner: R,
    failure: Option<io::Error>,
    bytes: u64,
}

impl<R: Read> Read for Tracked<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf).map_err(|err| -> io::Error {
            let kind = err.kind();
            if kind != io::ErrorKind::Interrupted {
                self.failure = Some(err);
            }
            kind.into()
        })?;
        self.bytes += read as u64;
        Ok(read)
    }
}

/// A writer that counts the bytes its inner writer accepted, for `-v`.
struct Counted<W> {
    inner: W,
    bytes: u64,
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.bytes += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// What `-v` says of a compression: the compressed size as a share of the
/// input's, in tenths of a percent, left out for an empty input.
fn share(read: u64, written: u64) -> String {
    if read == 0 {
        return String::new();
    }
    let tenths = u128::from(written) * 1000 / u128::from(read);
    format!(" ({}.{}%)", tenths / 10, tenths % 10)
}

/// Runs [`mothergod::copy_encode`] from `reader` into `writer`: a read
/// failure is reported with `input_name`, a write failure with `dest_name`.
/// With `verbose`, a success says the sizes on stderr.
fn compress_into<R: Read, W: Write>(
    reader: R,
    writer: W,
    dest_name: &str,
    input_name: &str,
    verbose: bool,
) -> ExitCode {
    let mut reader = Tracked {
        inner: reader,
        failure: None,
        bytes: 0,
    };
    let mut writer = Counted {
        inner: writer,
        bytes: 0,
    };
    match mothergod::copy_encode(&mut reader, &mut writer) {
        Ok(()) => {
            if verbose {
                let (read, written) = (reader.bytes, writer.bytes);
                say(&format!(
                    "{input_name}: {read} -> {written} bytes{}",
                    share(read, written)
                ));
            }
            ExitCode::SUCCESS
        }
        Err(err) => match reader.failure {
            Some(read_err) => fail(&format!("reading {input_name}: {read_err}")),
            None => write_fail(dest_name, &err),
        },
    }
}

/// Runs [`mothergod::decompress_to_writer`] into `writer`: a write failure
/// is reported with `dest_name`, a decode failure with `input_name`, which
/// several operands make the only clue to which input failed. With
/// `verbose`, a success says the sizes on stderr.
fn decompress_into<W: Write>(
    input: &[u8],
    writer: W,
    dest_name: &str,
    input_name: &str,
    verbose: bool,
) -> ExitCode {
    let mut writer = Counted {
        inner: writer,
        bytes: 0,
    };
    match mothergod::decompress_to_writer(input, u32::MAX, &mut writer) {
        Ok(()) => {
            if verbose {
                say(&format!(
                    "{input_name}: {} -> {} bytes",
                    input.len(),
                    writer.bytes
                ));
            }
            ExitCode::SUCCESS
        }
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

/// Reads stdin, or `path` if given, whole.
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

/// Opens stdin, or `path` if given, to be read as it is consumed.
fn open_input(path: Option<&Path>) -> Result<Box<dyn Read>, String> {
    match path {
        None => Ok(Box::new(io::stdin().lock())),
        Some(path) => File::open(path)
            .map(|file| Box::new(file) as Box<dyn Read>)
            .map_err(|err| format!("reading {}: {err}", path.display())),
    }
}

/// Opens `path` as a new file, refusing to clobber an existing one unless
/// `existing` is [`Existing::Replace`] (a compression re-run over an
/// already-compressed file, or a decompression target that already exists,
/// must not silently destroy it), then runs `write` against it. Replacing
/// unlinks the old file first and creates a new one, so a hard link to the
/// input, whose bytes are still being read or were read whole, cannot be
/// truncated under it; a failed write then leaves no file, as gzip does.
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
    existing: Existing,
    write: impl FnOnce(&mut File) -> ExitCode,
) -> ExitCode {
    if matches!(existing, Existing::Replace)
        && let Err(err) = fs::remove_file(path)
        && err.kind() != io::ErrorKind::NotFound
    {
        return fail(&format!("writing {}: {err}", path.display()));
    }
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
        say(&format!(
            "warning: {}: copying metadata from {}: {err}",
            path.display(),
            source.display()
        ));
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

/// Writes `text` and a newline to stdout, where the text is the whole output
/// of the run (`--help`, `--version`). `println!` would panic and exit 101 on
/// a full or closed stdout; a failed write takes [`write_fail`]'s status.
fn print_line(text: &str) -> ExitCode {
    match writeln!(io::stdout(), "{text}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => write_fail("stdout", &err),
    }
}

/// Writes one `mothergod: `-prefixed line to stderr. A failed write is
/// dropped: `eprintln!` would panic, exit 101, and turn a complete output
/// into a failed run because the diagnostic channel is full or closed
/// (`2>/dev/full`). Every stderr line goes through here.
fn say(message: &str) {
    let _ = writeln!(io::stderr(), "mothergod: {message}");
}

fn fail(message: &str) -> ExitCode {
    say(message);
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fails once with `kind`, then reads as an empty stream.
    struct FailOnce(Option<io::ErrorKind>);

    impl Read for FailOnce {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            self.0.take().map_or(Ok(0), |kind| Err(kind.into()))
        }
    }

    fn tracked_after_one_read(kind: io::ErrorKind) -> Option<io::ErrorKind> {
        let mut reader = Tracked {
            inner: FailOnce(Some(kind)),
            failure: None,
            bytes: 0,
        };
        let _ = reader.read(&mut [0; 1]);
        reader.failure.map(|err| err.kind())
    }

    #[test]
    fn tracked_keeps_a_read_failure() {
        assert_eq!(
            tracked_after_one_read(io::ErrorKind::PermissionDenied),
            Some(io::ErrorKind::PermissionDenied)
        );
    }

    #[test]
    fn tracked_drops_an_interrupted_read_the_library_retries() {
        assert_eq!(tracked_after_one_read(io::ErrorKind::Interrupted), None);
    }
}
