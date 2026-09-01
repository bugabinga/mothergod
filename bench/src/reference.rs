//! Shells out to the pinned reference compressors
//! (`research/corpus/POLICY.md`: "Reference compressors: zstd -19 and
//! xz -9e at pinned versions") plus gzip, measuring compressed size on the
//! same bytes [`crate::finals`] measures `mothergod::compress` on. Gated
//! behind `corpus-fetch`: only meaningful alongside a held-out-final fetch
//! (`crate::corpus`), so it never enters the default-feature build
//! CLAUDE.md's required checks compile.

use crate::finals::FileMeasurement;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// Writes `data` to a fresh file under `std::env::temp_dir()`, unique per
/// call (an atomic counter, not a hash of `data`) so concurrent
/// measurements on identical bytes never collide.
fn write_temp_file(data: &[u8], label: &str) -> io::Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("mothergod-bench-reference-{label}-{n}"));
    std::fs::write(&path, data)?;
    Ok(path)
}

/// Runs `cmd args <input_path>`, returning what it wrote to stdout.
///
/// Reads the input from a file argument rather than piping it over stdin:
/// gzip/zstd/xz all accept a filename plus `-c` to keep the compressed
/// form on stdout, and a file argument sidesteps the stdin/stdout pipe
/// deadlock a multi-megabyte write risks under `Command::output()`, which
/// captures stdout only after the child exits and never hands back a
/// stdin handle to write into concurrently.
fn run_to_stdout(cmd: &str, args: &[&str], input_path: &Path) -> io::Result<Vec<u8>> {
    let output = Command::new(cmd).args(args).arg(input_path).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{cmd} {args:?} {} exited with {}: {}",
            input_path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

/// Compressed byte length of `data` under `cmd` invoked with `args` (e.g.
/// `("gzip", &["-9", "-c"])`), via a temp file so the same bytes never
/// need to round-trip through this process's own stdin pipe. The temp
/// file is removed before returning, on both the success and error path.
///
/// # Errors
///
/// Returns an error if the temp file can't be written, `cmd` isn't on
/// `PATH`, or `cmd` exits non-zero.
pub fn compressed_len(cmd: &str, args: &[&str], data: &[u8]) -> io::Result<usize> {
    let path = write_temp_file(data, cmd)?;
    let result = run_to_stdout(cmd, args, &path);
    let _ = std::fs::remove_file(&path);
    Ok(result?.len())
}

/// `date -u`'s current UTC timestamp, `YYYY-MM-DDTHH:MM:SSZ`, the same
/// shape `site-status/src/bin/generate.rs` stamps `site/status-data.json`
/// with. Shelling out rather than a `SystemTime`-to-calendar conversion:
/// this crate's `corpus-fetch` feature already carries five dependencies:
/// pulling in a datetime crate just for one timestamp isn't worth a sixth.
/// Shared by every held-out-final report binary (`finals_report`,
/// `silesia_report`) rather than each defining its own copy.
///
/// # Errors
///
/// Returns an error if `date` isn't on `PATH`, exits non-zero, or its
/// output isn't valid UTF-8.
pub fn generated_at() -> io::Result<String> {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "date -u exited with {}",
            output.status
        )));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|err| io::Error::other(format!("date produced non-UTF-8 output: {err}")))
}

/// Best-effort description of the machine [`measure_all`] ran on: CPU
/// model, logical core count, whether the process looks like it ran on a
/// CI runner, and the one-thread-per-file measurement shape. Named in the
/// report next to the throughput columns (issue #432): an MB/s number
/// without its hardware is the same defect CLAUDE.md rule 4 already
/// forbids for a bits/byte number without its corpus, so a reader compares
/// mothergod runs to each other, never to a number from an unnamed
/// machine.
///
/// `CI` is the convention every major CI system exports (GitHub Actions,
/// GitLab, Travis, CircleCI); its absence just means "not detected as CI",
/// not "definitely a developer machine".
#[must_use]
pub fn machine_info() -> String {
    let cpu = cpu_model().unwrap_or_else(|| "unknown CPU".to_string());
    let cores = std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get);
    let runner = if std::env::var_os("CI").is_some() {
        "CI runner"
    } else {
        "non-CI machine"
    };
    format!(
        "{cpu}, {cores} logical core(s), {runner}, one thread per file \
         (`reference::measure_all`)"
    )
}

/// `/proc/cpuinfo`'s `model name` field. `None` if unreadable or absent
/// (e.g. a non-x86 kernel that doesn't populate the field), in which case
/// [`machine_info`] falls back to a plain "unknown CPU" rather than
/// failing the whole report over a cosmetic field.
#[cfg(target_os = "linux")]
fn cpu_model() -> Option<String> {
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    text.lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim().to_string())
}

/// `sysctl -n machdep.cpu.brand_string`, macOS's equivalent of Linux's
/// `/proc/cpuinfo` model name.
#[cfg(target_os = "macos")]
fn cpu_model() -> Option<String> {
    let output = Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Every other target: `PROCESSOR_IDENTIFIER` (set by Windows) if present,
/// else `None`.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn cpu_model() -> Option<String> {
    std::env::var("PROCESSOR_IDENTIFIER").ok()
}

/// First line of `cmd --version`'s output (stdout, falling back to stderr
/// if stdout is empty), trimmed. Names which build produced a reference
/// number: `research/corpus/POLICY.md` pins the flags (`zstd -19`,
/// `xz -9e`), not the installed binary version, so a report quoting these
/// numbers carries this line as its honesty record (CLAUDE.md rule 4).
///
/// # Errors
///
/// Returns an error if `cmd` isn't on `PATH` or produced no output on
/// either stream.
pub fn tool_version(cmd: &str) -> io::Result<String> {
    let output = Command::new(cmd).arg("--version").output()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    text.lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .ok_or_else(|| io::Error::other(format!("{cmd} --version produced no output")))
}

/// Measures one held-out-final file against `mothergod::compress` and the
/// three pinned reference compressors, all on the exact same bytes.
///
/// # Errors
///
/// Returns an error naming `name` if any reference compressor fails on it.
fn measure_one(name: &str, data: &[u8]) -> Result<FileMeasurement, String> {
    println!("measuring {name} ({} bytes)...", data.len());
    let encode_start = std::time::Instant::now();
    let mothergod_compressed = mothergod::compress(data);
    let encode_secs = encode_start.elapsed().as_secs_f64();
    let mothergod_len = mothergod_compressed.len();

    let decode_start = std::time::Instant::now();
    let decoded = mothergod::decompress(&mothergod_compressed)
        .map_err(|err| format!("mothergod::decompress failed on {name}: {err}"))?;
    let decode_secs = decode_start.elapsed().as_secs_f64();
    if decoded != data {
        return Err(format!(
            "mothergod round-trip mismatch on {name}: decompress(compress(x)) != x"
        ));
    }

    let gzip_len = compressed_len("gzip", &["-9", "-c"], data)
        .map_err(|err| format!("gzip failed on {name}: {err}"))?;
    let zstd_len = compressed_len("zstd", &["-19", "-c"], data)
        .map_err(|err| format!("zstd failed on {name}: {err}"))?;
    let xz_len = compressed_len("xz", &["-9e", "-c"], data)
        .map_err(|err| format!("xz failed on {name}: {err}"))?;
    Ok(FileMeasurement {
        name: name.to_string(),
        original_len: data.len(),
        mothergod_len,
        gzip_len,
        zstd_len,
        xz_len,
        encode_secs,
        decode_secs,
    })
}

/// Measures every `(name, data)` pair, one OS thread per file, joining
/// before returning.
///
/// `mothergod::compress`'s optimal-parse LZ is the dominant, strictly
/// single-threaded cost per file (measured on Silesia's smallest file,
/// `xml` at 5.3 MB: ~39s, ~0.14 MB/s); run one file after another and the
/// full ~200 MB Silesia corpus takes on the order of half an hour, too
/// slow for a single CI or by-hand turn. Every file is independent (no
/// shared mutable state, no cross-file model), so spawning a thread per
/// file lets the OS scheduler spread the work across every available
/// core instead — the same total CPU time, in wall-clock time divided by
/// core count. `finals_report` and `silesia_report` both call this so
/// the parallel measurement loop exists in exactly one place.
///
/// # Errors
///
/// Returns the first per-file error encountered, by input order, if any
/// reference compressor fails on any file.
pub fn measure_all(files: &[(String, Vec<u8>)]) -> Result<Vec<FileMeasurement>, String> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = files
            .iter()
            .map(|(name, data)| scope.spawn(move || measure_one(name, data)))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|_| Err("measurement thread panicked".to_string()))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{compressed_len, generated_at, machine_info, measure_all, tool_version};

    /// Repeated enough (200x) that every reference compressor's fixed
    /// container overhead (gzip's ~18-byte header/trailer, xz's larger
    /// container) is dwarfed by the savings, unlike a single short
    /// sentence, which a real compressor can legitimately grow.
    fn fixture() -> Vec<u8> {
        "hello, mothergod reference-compressor fixture. "
            .repeat(200)
            .into_bytes()
    }

    #[test]
    fn compressed_len_shrinks_a_repetitive_fixture_under_gzip() {
        let fixture = fixture();
        let len = compressed_len("gzip", &["-9", "-c"], &fixture).expect("gzip must be on PATH");
        assert!(
            len > 0 && len < fixture.len(),
            "expected 0 < len < {}, got {len}",
            fixture.len()
        );
    }

    #[test]
    fn compressed_len_shrinks_a_repetitive_fixture_under_zstd() {
        let fixture = fixture();
        let len = compressed_len("zstd", &["-19", "-c"], &fixture).expect("zstd must be on PATH");
        assert!(
            len > 0 && len < fixture.len(),
            "expected 0 < len < {}, got {len}",
            fixture.len()
        );
    }

    #[test]
    fn compressed_len_shrinks_a_repetitive_fixture_under_xz() {
        let fixture = fixture();
        let len = compressed_len("xz", &["-9e", "-c"], &fixture).expect("xz must be on PATH");
        assert!(
            len > 0 && len < fixture.len(),
            "expected 0 < len < {}, got {len}",
            fixture.len()
        );
    }

    #[test]
    fn compressed_len_rejects_an_unknown_command() {
        assert!(compressed_len("mothergod-does-not-exist-as-a-binary", &[], &fixture()).is_err());
    }

    #[test]
    fn compressed_len_handles_empty_input() {
        // gzip/zstd/xz all still emit a (nonzero-length) container/header
        // for zero bytes of input; the point of this test is that an empty
        // temp file doesn't error out, not any particular byte count.
        let len = compressed_len("gzip", &["-9", "-c"], b"").expect("gzip must be on PATH");
        assert!(len > 0, "expected gzip to emit at least a header, got 0");
    }

    #[test]
    fn tool_version_reads_a_nonempty_line_for_every_reference_compressor() {
        for cmd in ["gzip", "zstd", "xz"] {
            let version = tool_version(cmd).unwrap_or_else(|err| panic!("{cmd}: {err}"));
            assert!(
                !version.is_empty(),
                "{cmd} --version returned an empty line"
            );
        }
    }

    #[test]
    fn tool_version_rejects_an_unknown_command() {
        assert!(tool_version("mothergod-does-not-exist-as-a-binary").is_err());
    }

    #[test]
    fn generated_at_produces_an_iso8601_utc_timestamp() {
        let stamp = generated_at().expect("date must be on PATH");
        assert_eq!(
            stamp.len(),
            20,
            "expected YYYY-MM-DDTHH:MM:SSZ, got {stamp:?}"
        );
        assert!(
            stamp.ends_with('Z'),
            "expected a Z-suffixed UTC stamp, got {stamp:?}"
        );
        assert_eq!(
            stamp.matches('-').count(),
            2,
            "expected two date separators, got {stamp:?}"
        );
    }

    #[test]
    fn machine_info_names_a_core_count_and_the_measurement_shape() {
        let info = machine_info();
        assert!(
            info.contains("logical core"),
            "expected a logical core count, got {info:?}"
        );
        assert!(
            info.contains("measure_all"),
            "expected the one-thread-per-file shape named, got {info:?}"
        );
    }

    #[test]
    fn measure_all_measures_every_file_and_preserves_input_order() {
        let files = vec![
            ("b.txt".to_string(), b"bbbbbbbbbbbbbbbbbbbb".to_vec()),
            ("a.txt".to_string(), b"aaaaaaaaaaaaaaaaaaaa".to_vec()),
        ];
        let measurements = measure_all(&files).expect("reference compressors must be on PATH");
        let names: Vec<&str> = measurements.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["b.txt", "a.txt"],
            "measure_all must preserve input order, not race threads into an arbitrary one"
        );
        for m in &measurements {
            assert_eq!(m.original_len, 20);
            assert!(m.mothergod_len > 0);
            assert!(m.gzip_len > 0);
            assert!(m.zstd_len > 0);
            assert!(m.xz_len > 0);
            assert!(m.encode_secs >= 0.0);
            assert!(m.decode_secs >= 0.0);
        }
    }
}
