#![forbid(unsafe_code)]
//! `mothergod` — command-line compressor/decompressor (ROADMAP M6).
//!
//! Minimal first slice: reads the whole input from stdin and writes the
//! whole output to stdout, chosen by a `compress`/`decompress` subcommand,
//! mirroring the `gzip -c`/`zstd -c` shape users already know.
//!
//! ```text
//! mothergod compress   < input       > input.mgdc
//! mothergod decompress < input.mgdc  > input
//! ```
//!
//! File arguments and an output-suffix convention are follow-on scope, not
//! built here. Buffering the whole input in memory before transforming it
//! matches [`mothergod::compress`]/[`mothergod::decompress`]'s own
//! whole-buffer signatures; a streaming API is ROADMAP M4 scope, not this
//! binary's to add on its own.

use std::io::{self, Read, Write};
use std::process::ExitCode;

const USAGE: &str = "usage: mothergod <compress|decompress>\n\nReads stdin, writes stdout.";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let command = args.next();
    if args.next().is_some() {
        eprintln!("mothergod: too many arguments\n\n{USAGE}");
        return ExitCode::FAILURE;
    }

    match command.as_deref() {
        Some("compress") => run(mothergod::compress),
        Some("decompress") => run_decompress(),
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

/// Reads stdin, applies `transform`, writes the result to stdout. Shared by
/// `compress`'s arm; `decompress` needs its own wrapper below because
/// [`mothergod::decompress`] returns a `Result`, not a bare `Vec<u8>`.
fn run(transform: impl FnOnce(&[u8]) -> Vec<u8>) -> ExitCode {
    match read_stdin() {
        Ok(input) => write_stdout(&transform(&input)),
        Err(err) => io_error(&err),
    }
}

fn run_decompress() -> ExitCode {
    let input = match read_stdin() {
        Ok(input) => input,
        Err(err) => return io_error(&err),
    };
    match mothergod::decompress(&input) {
        Ok(output) => write_stdout(&output),
        Err(err) => {
            eprintln!("mothergod: {err}");
            ExitCode::FAILURE
        }
    }
}

fn read_stdin() -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    io::stdin().lock().read_to_end(&mut buf)?;
    Ok(buf)
}

fn write_stdout(bytes: &[u8]) -> ExitCode {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => io_error(&err),
    }
}

fn io_error(err: &io::Error) -> ExitCode {
    eprintln!("mothergod: {err}");
    ExitCode::FAILURE
}
