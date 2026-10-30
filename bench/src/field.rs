//! The field around the pinned ratio rungs (issue #1340): every level of
//! `gzip`, `zstd` and `xz`, each measured as one process at a time for
//! compressed size, encode and decode wall-clock, and peak resident set.
//!
//! [`crate::reference`] keeps the three rungs `research/corpus/POLICY.md`
//! pins (`gzip -9`, `zstd -19`, `xz -9e`) as sizes only. This module is the
//! primitive under the full matrix: [`competitors`] lists the levels and
//! [`measure`] times one of them on one input. [`measure_mothergod`] times
//! mothergod the same way, by re-running the report binary itself in the
//! child mode [`run_child_if_requested`] serves, so the codec is measured by
//! the instrument the competitors are and its peak resident set exists at
//! all. Callers run it serially, the way `reference::measure_all` does
//! (#1321): a wall-clock taken beside another measurement is not a rate.
//!
//! Peak RSS comes from GNU `time` (`/usr/bin/time -f %M`, KiB), so
//! [`measure`] needs Linux and reports an error elsewhere rather than a
//! number from a different instrument.

use crate::reference::write_temp_file;
use mothergod::Level;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

/// GNU `time`'s absolute path. Not looked up on `PATH`: the shell keyword
/// `time` is a different instrument with no `-f`.
const GNU_TIME: &str = "/usr/bin/time";

/// A reference compressor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// `gzip`, levels 1 to 9.
    Gzip,
    /// `zstd`, levels 1 to 19 and `--ultra` 20 to 22.
    Zstd,
    /// `xz`, levels 0 to 9 and `9e`.
    Xz,
}

impl Tool {
    /// The executable name looked up on `PATH`.
    #[must_use]
    pub fn command(self) -> &'static str {
        match self {
            Tool::Gzip => "gzip",
            Tool::Zstd => "zstd",
            Tool::Xz => "xz",
        }
    }
}

/// One tool at one level: a row of the frontier table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Competitor {
    /// The family.
    pub tool: Tool,
    /// The level as a user types it after the dash: `"1"`, `"22"`, `"9e"`.
    pub level: String,
    /// Arguments selecting the level, before `-c`.
    args: Vec<String>,
}

impl Competitor {
    fn new(tool: Tool, level: &str, ultra: bool) -> Self {
        let mut args = Vec::new();
        if ultra {
            args.push("--ultra".to_string());
        }
        args.push(format!("-{level}"));
        Competitor {
            tool,
            level: level.to_string(),
            args,
        }
    }

    /// The tool and level as one label, `"zstd -19"`.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{} -{}", self.tool.command(), self.level)
    }
}

/// Every level each tool offers, in tool then ascending level order:
/// gzip 1 to 9, zstd 1 to 22 (`--ultra` from 20), xz 0 to 9 then 9e.
#[must_use]
pub fn competitors() -> Vec<Competitor> {
    let gzip = (1..=9).map(|n| Competitor::new(Tool::Gzip, &n.to_string(), false));
    let zstd = (1..=22).map(|n| Competitor::new(Tool::Zstd, &n.to_string(), n >= 20));
    let xz = (0..=9)
        .map(|n| n.to_string())
        .chain(["9e".to_string()])
        .map(|level| Competitor::new(Tool::Xz, &level, false));
    gzip.chain(zstd).chain(xz).collect()
}

/// What [`measure`] reads off one competitor on one input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldPoint {
    /// Compressed bytes.
    pub compressed_len: usize,
    /// Wall-clock seconds of the compress process, spawn to stdout drained.
    pub encode_secs: f64,
    /// Wall-clock seconds of the decompress process.
    pub decode_secs: f64,
    /// Peak resident set of the compress process, KiB.
    pub encode_peak_rss_kib: u64,
    /// Peak resident set of the decompress process, KiB.
    pub decode_peak_rss_kib: u64,
}

/// A temp file removed on drop, so an error path leaves nothing behind.
struct TempFile(PathBuf);

impl TempFile {
    fn new(data: &[u8], label: &str) -> io::Result<Self> {
        write_temp_file(data, label).map(TempFile)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// One process run: its stdout, wall-clock and peak resident set.
struct Run {
    stdout: Vec<u8>,
    secs: f64,
    peak_rss_kib: u64,
}

/// The peak resident set `time -f %M` wrote: its last non-empty line, since
/// a failing child makes `time` write a status line first.
fn parse_peak_rss_kib(report: &str) -> Option<u64> {
    report
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .parse()
        .ok()
}

/// Runs `program args` with `input` as stdin under GNU `time`, stdout
/// captured.
///
/// Stdin is the file itself, not a pipe: no write deadlock against the
/// captured stdout, and the tool sees no file name, so gzip embeds none
/// into its header (#1326).
fn run_timed(program: &Path, args: &[String], input: &Path) -> io::Result<Run> {
    let cmd = program.display();
    let report = TempFile::new(b"", "time")?;
    let start = Instant::now();
    let output = Command::new(GNU_TIME)
        .args(["-f", "%M", "-o"])
        .arg(report.path())
        .arg(program)
        .args(args)
        .stdin(File::open(input)?)
        .output()?;
    let secs = start.elapsed().as_secs_f64();
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{cmd} {args:?} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let peak_rss_kib = parse_peak_rss_kib(&std::fs::read_to_string(report.path())?)
        .ok_or_else(|| io::Error::other(format!("{GNU_TIME} wrote no peak RSS for {cmd}")))?;
    Ok(Run {
        stdout: output.stdout,
        secs,
        peak_rss_kib,
    })
}

/// Compresses `data` with `program encode_args`, decompresses it with
/// `program decode_args`, checks the round trip, and returns both
/// processes' size, time and peak resident set.
fn measure_processes(
    label: &str,
    program: &Path,
    encode_args: &[String],
    decode_args: &[String],
    data: &[u8],
) -> io::Result<FieldPoint> {
    let input = TempFile::new(data, "input")?;
    let encode = run_timed(program, encode_args, input.path())?;
    let compressed = TempFile::new(&encode.stdout, "compressed")?;
    let decode = run_timed(program, decode_args, compressed.path())?;
    if decode.stdout != data {
        return Err(io::Error::other(format!(
            "{label} round-trip mismatch: decompress(compress(x)) != x"
        )));
    }
    Ok(FieldPoint {
        compressed_len: encode.stdout.len(),
        encode_secs: encode.secs,
        decode_secs: decode.secs,
        encode_peak_rss_kib: encode.peak_rss_kib,
        decode_peak_rss_kib: decode.peak_rss_kib,
    })
}

/// Compresses `data` under `competitor`, decompresses it, checks the round
/// trip, and returns both processes' size, time and peak resident set.
///
/// # Errors
///
/// Returns an error if GNU `time` or the tool is missing, either process
/// exits non-zero, or the decoded bytes differ from `data`.
pub fn measure(competitor: &Competitor, data: &[u8]) -> io::Result<FieldPoint> {
    let mut encode_args = competitor.args.clone();
    encode_args.push("-c".to_string());
    let decode_args = ["-d", "-c"].map(String::from);
    measure_processes(
        &competitor.label(),
        Path::new(competitor.tool.command()),
        &encode_args,
        &decode_args,
        data,
    )
}

/// The first argument that puts a report binary in child mode.
const CHILD_FLAG: &str = "--child";

/// Every encoder level the reports measure, quickest first. Not
/// [`Level::Extreme`] yet: its trials and decode would add most of an hour
/// to a report run (issue #1824).
pub const LEVELS: [Level; 3] = [Level::Fastest, Level::Fast, Level::Best];

/// The digit the CLI selects `level` by.
fn level_digit(level: Level) -> char {
    match level {
        Level::Fastest => '1',
        Level::Fast => '2',
        Level::Best => '3',
        Level::Extreme => '4',
    }
}

/// How a report names `level`: `mothergod -3`, as a user types it.
#[must_use]
pub fn level_label(level: Level) -> String {
    format!("mothergod -{}", level_digit(level))
}

/// What a mothergod child process does to its stdin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    /// `mothergod::compress_level` at the level.
    Compress(Level),
    /// `mothergod::decompress`.
    Decompress,
}

impl Direction {
    /// The arguments after [`CHILD_FLAG`] that select it: `compress 3`, or
    /// `decompress`.
    fn child_args(self) -> Vec<String> {
        let mut args = vec![CHILD_FLAG.to_string()];
        match self {
            Direction::Compress(level) => {
                args.extend(["compress".to_string(), level_digit(level).to_string()]);
            }
            Direction::Decompress => args.push("decompress".to_string()),
        }
        args
    }

    /// The direction `args`, the child's whole command line after the
    /// program, select.
    fn parse(args: &[String]) -> Result<Direction, String> {
        match args {
            [flag, mode, digit] if flag == CHILD_FLAG && mode == "compress" => LEVELS
                .into_iter()
                .find(|&level| *digit == level_digit(level).to_string())
                .map(Direction::Compress)
                .ok_or_else(|| format!("unknown child level {digit:?}")),
            [flag, mode] if flag == CHILD_FLAG && mode == "decompress" => Ok(Direction::Decompress),
            _ => Err(format!("unknown child command line {args:?}")),
        }
    }

    fn apply(self, input: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            Direction::Compress(level) => Ok(mothergod::compress_level(input, level)),
            Direction::Decompress => mothergod::decompress(input).map_err(|err| err.to_string()),
        }
    }
}

/// Serves the child mode [`measure_mothergod`] spawns: when the process was
/// started as `<exe> --child compress <digit>` or `<exe> --child
/// decompress`, runs mothergod over stdin to stdout and returns the exit
/// code the binary must end with. Returns `None` for any other command line
/// that does not start with the flag.
///
/// Every binary that passes its own path to [`measure_mothergod`] calls this
/// first in `main`: a binary that does not would run its whole report again
/// in the child.
#[must_use]
pub fn run_child_if_requested() -> Option<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some(CHILD_FLAG) {
        return None;
    }
    let result = Direction::parse(&args).and_then(|direction| {
        let mut input = Vec::new();
        io::stdin()
            .lock()
            .read_to_end(&mut input)
            .map_err(|err| err.to_string())?;
        let output = direction.apply(&input)?;
        io::stdout()
            .lock()
            .write_all(&output)
            .map_err(|err| err.to_string())
    });
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    })
}

/// Compresses `data` at `level` in a child of `program`, decompresses it in
/// another, and returns what [`measure`] returns for a
/// competitor, so mothergod's row and the field's share one instrument.
///
/// `program` is a binary that calls [`run_child_if_requested`] first in
/// `main`: a report binary's own path in production, `bench_child` in tests.
///
/// # Errors
///
/// Returns an error if GNU `time` or `program` is missing, either process
/// exits non-zero, or the decoded bytes differ from `data`.
pub fn measure_mothergod(program: &Path, level: Level, data: &[u8]) -> io::Result<FieldPoint> {
    measure_processes(
        &level_label(level),
        program,
        &Direction::Compress(level).child_args(),
        &Direction::Decompress.child_args(),
        data,
    )
}

#[cfg(test)]
mod tests {
    use super::{Tool, competitors, measure, parse_peak_rss_kib};

    #[test]
    fn competitors_lists_every_level_each_tool_offers() {
        let labels: Vec<String> = competitors().iter().map(super::Competitor::label).collect();
        assert_eq!(labels.len(), 9 + 22 + 11);
        for expected in [
            "gzip -1", "gzip -9", "zstd -1", "zstd -22", "xz -0", "xz -9e",
        ] {
            assert!(labels.iter().any(|l| l == expected), "missing {expected}");
        }
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len(), "a level is listed twice");
    }

    #[test]
    fn only_zstd_levels_from_20_pass_ultra() {
        for c in competitors() {
            let ultra = c.args.iter().any(|a| a == "--ultra");
            let expected = c.tool == Tool::Zstd && c.level.parse::<u32>().is_ok_and(|n| n >= 20);
            assert_eq!(ultra, expected, "{}", c.label());
        }
    }

    #[test]
    fn peak_rss_is_the_last_line_of_the_time_report() {
        assert_eq!(parse_peak_rss_kib("1234\n"), Some(1234));
        assert_eq!(
            parse_peak_rss_kib("Command exited with non-zero status 2\n987\n"),
            Some(987)
        );
        assert_eq!(parse_peak_rss_kib(""), None);
        assert_eq!(parse_peak_rss_kib("not a number\n"), None);
    }

    // GNU time and the reference compressors are a Linux CI image's, not
    // Android's toolbox (#618) or the macOS and Windows lanes'.
    #[test]
    #[cfg_attr(
        not(target_os = "linux"),
        ignore = "needs /usr/bin/time -f and gzip/zstd/xz on PATH"
    )]
    fn measure_round_trips_a_level_of_every_tool() {
        let data = "hello, mothergod field fixture. ".repeat(500).into_bytes();
        let all = competitors();
        for label in ["gzip -1", "zstd -3", "zstd -20", "xz -0"] {
            let competitor = all.iter().find(|c| c.label() == label).expect(label);
            let point = measure(competitor, &data).unwrap_or_else(|err| panic!("{label}: {err}"));
            assert!(
                point.compressed_len > 0 && point.compressed_len < data.len(),
                "{label}: {} bytes",
                point.compressed_len
            );
            assert!(
                point.encode_secs > 0.0 && point.decode_secs > 0.0,
                "{label}"
            );
            assert!(
                point.encode_peak_rss_kib > 0 && point.decode_peak_rss_kib > 0,
                "{label}"
            );
        }
    }
}
