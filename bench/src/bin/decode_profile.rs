//! Prints where the default level's decode time goes, per stage, over the
//! files named on the command line (issue #1930, journal S2-R43): the table
//! charter #1922 prices each candidate against, from one command instead of
//! a recipe typed per session.
//!
//! The binary builds the `mothergod` CLI in release with line tables only
//! (`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`, codegen unchanged) into
//! `target/profile`, compresses each file with it at the default level,
//! then decodes each frame under `perf record -g -F 999`, pinned to the
//! highest-numbered CPU with `taskset`, and fails when a decode differs
//! from its file. [`mothergod_bench::decode_profile`] attributes each
//! sample to a stage through its inline chain and prints the table, CPU
//! seconds by the `task-clock` periods. Work files are deleted file by
//! file, so the disk holds one file's frame, output and samples at a time.
//!
//! Needs `perf` and `taskset` on `PATH` and `perf_event_paranoid` at 1 or
//! below; a GitHub-hosted runner ships 4, so run `sudo sysctl
//! kernel.perf_event_paranoid=1` first. No hardware counter is read: the
//! hosted pool exposes none, and `task-clock` is a software clock.
//!
//! Usage: `cargo run -p mothergod-bench --release --bin decode_profile --
//! <file>...`. Encode dominates the wall time: about 27 minutes for all
//! 12 Silesia files at `-3` on an AMD EPYC 9V45.

use mothergod_bench::decode_profile::{Tally, format_table};
use mothergod_bench::reference::{cpu_model, tool_version};
use mothergod_bench::repo_root;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "usage: decode_profile <file>...";

/// Samples per second of `task-clock`, S2-R43's rate.
const FREQUENCY: &str = "999";

/// Runs `command` to completion, its stdout into `stdout` when given.
fn run(command: &mut Command, stdout: Option<&Path>) -> Result<(), String> {
    if let Some(path) = stdout {
        let file = File::create(path).map_err(|err| format!("{}: {err}", path.display()))?;
        command.stdout(file);
    }
    let status = command
        .status()
        .map_err(|err| format!("{command:?}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?} exited with {status}"))
    }
}

/// Refuses a kernel that would not let `perf` sample this user's process,
/// naming the command that fixes it.
fn check_paranoid() -> Result<(), String> {
    let path = "/proc/sys/kernel/perf_event_paranoid";
    let text = std::fs::read_to_string(path).map_err(|err| format!("{path}: {err}"))?;
    let level: i32 = text
        .trim()
        .parse()
        .map_err(|_| format!("{path}: {text:?} is not a level"))?;
    if level > 1 {
        return Err(format!(
            "{path} is {level}; perf needs 1 or below: sudo sysctl kernel.perf_event_paranoid=1"
        ));
    }
    Ok(())
}

/// Builds the CLI with line tables into `target/profile` and returns its
/// path; a separate target directory, so the ordinary release build keeps
/// its cache.
fn build(root: &Path) -> Result<PathBuf, String> {
    let target = root.join("target/profile");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    run(
        Command::new(cargo)
            .args(["build", "--quiet", "--release", "--bin", "mothergod"])
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .env("CARGO_PROFILE_RELEASE_DEBUG", "line-tables-only")
            .env("CARGO_TARGET_DIR", &target),
        None,
    )?;
    Ok(target.join("release/mothergod"))
}

/// Compresses `input`, decodes it under `perf` on `cpu`, checks the
/// decode, and adds its samples to `tally`; returns the file's length.
fn profile(
    mothergod: &Path,
    input: &Path,
    work: &Path,
    cpu: &str,
    tally: &mut Tally,
) -> Result<usize, String> {
    let frame = work.join("frame.mgdc");
    let decoded = work.join("decoded");
    let samples = work.join("perf.data");
    run(
        Command::new(mothergod).arg("-3").arg("-c").arg(input),
        Some(&frame),
    )?;
    run(
        Command::new("perf")
            .args(["record", "--quiet", "-g", "-F", FREQUENCY, "-o"])
            .arg(&samples)
            .args(["--", "taskset", "-c", cpu])
            .arg(mothergod)
            .arg("-dc")
            .arg(&frame),
        Some(&decoded),
    )?;
    let original = std::fs::read(input).map_err(|err| format!("{}: {err}", input.display()))?;
    let output = std::fs::read(&decoded).map_err(|err| format!("{}: {err}", decoded.display()))?;
    if output != original {
        return Err(format!(
            "{}: the decode differs from the file, a lossless failure",
            input.display()
        ));
    }
    let script = Command::new("perf")
        .args(["script", "-F", "ip,sym,dso,period", "--inline", "-i"])
        .arg(&samples)
        .stderr(Stdio::null())
        .output()
        .map_err(|err| format!("perf script: {err}"))?;
    if !script.status.success() {
        return Err(format!("perf script exited with {}", script.status));
    }
    let text = String::from_utf8(script.stdout).map_err(|err| format!("perf script: {err}"))?;
    tally.add(&text)?;
    for path in [&frame, &decoded, &samples] {
        std::fs::remove_file(path).map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(original.len())
}

fn main() -> ExitCode {
    let inputs: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if inputs.is_empty() || inputs.iter().any(|path| path.starts_with("-")) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match report(&inputs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("decode_profile: {err}");
            ExitCode::FAILURE
        }
    }
}

fn report(inputs: &[PathBuf]) -> Result<(), String> {
    check_paranoid()?;
    let perf = tool_version("perf").map_err(|err| format!("perf: {err}"))?;
    let root = repo_root();
    let mothergod = build(&root)?;
    let work = root.join("target/profile/decode_profile");
    std::fs::create_dir_all(&work).map_err(|err| format!("{}: {err}", work.display()))?;
    // The highest-numbered CPU: the first is where interrupts tend to land.
    let cpu = std::thread::available_parallelism()
        .map_or(0, |n| n.get() - 1)
        .to_string();

    let mut tally = Tally::default();
    let mut bytes = 0;
    for input in inputs {
        eprintln!("profiling {}...", input.display());
        bytes += profile(&mothergod, input, &work, &cpu, &mut tally)?;
    }
    let machine = cpu_model().unwrap_or_else(|| "unknown CPU".to_string());
    println!(
        "mothergod -3 decode of {} file(s), {bytes} bytes, on CPU {cpu} of {machine}, \
         {perf}, {} samples at {FREQUENCY} Hz\n",
        inputs.len(),
        tally.samples
    );
    print!("{}", format_table(&tally));
    Ok(())
}
