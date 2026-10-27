//! Where the fixture directories live: `tests/golden` and `tests/adversarial`
//! under the build host's manifest dir, or the directory an environment
//! variable names (the Android lane pushes the fixtures to the device and
//! points the test executable at them).
//! `tests/golden.rs`, `tests/parse_digest.rs`, `tests/torture.rs` and
//! `tests/adversarial.rs` include this file with `#[path]`; a second lookup
//! would be a second place for the override rule to drift (issue #1608).
//! It is not `tests/common/mod.rs`: that module spawns the binary and its
//! includers carry the Miri and Android exclusions, which these suites do
//! not.

use std::path::{Path, PathBuf};

/// `variable`'s value, else `default` under the manifest dir.
fn fixture_dir(variable: &str, default: &str) -> PathBuf {
    std::env::var_os(variable).map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join(default),
        PathBuf::from,
    )
}

/// The `.mgdc`/`.plaintext` pairs of `tests/golden`, or `MOTHERGOD_GOLDEN_DIR`.
pub fn golden_dir() -> PathBuf {
    fixture_dir("MOTHERGOD_GOLDEN_DIR", "tests/golden")
}

/// The seeds of `tests/adversarial`, or `MOTHERGOD_ADVERSARIAL_DIR`.
pub fn adversarial_dir() -> PathBuf {
    fixture_dir("MOTHERGOD_ADVERSARIAL_DIR", "tests/adversarial")
}
