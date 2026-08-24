//! Fetch-and-cache for the held-out-final corpora (Silesia, Canterbury).
//!
//! `research/corpus/POLICY.md`: "Borrowed corpora are never committed
//! (size and third-party copyright). `bench/corpus.toml` pins each file
//! by URL + SHA-256; the harness fetches and caches, and refuses to run
//! on a checksum mismatch."
//!
//! [`parse_manifest`] reads `bench/corpus.toml`'s `[[file]]` entries;
//! [`fetch_and_cache`] resolves one entry by name, serving it from a
//! local on-disk cache keyed by its pinned hash when present and
//! refusing to return bytes that don't match the pin. Decompression
//! (bzip2 for Silesia, tar+gzip for Canterbury) is out of scope for this
//! slice — see the module doc on `bench/corpus.toml` for the remaining
//! `research/JOURNAL.md` S1-D2 scope.

use sha2::{Digest, Sha256};
use std::fmt;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// One `[[file]]` entry from `bench/corpus.toml`: a pinned download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    /// Short handle used to look the entry up, e.g. `"dickens"`.
    pub name: String,
    /// Which held-out-final corpus this file belongs to, e.g. `"silesia"`.
    pub corpus: String,
    /// Where to download the (compressed) bytes from.
    pub url: String,
    /// SHA-256, lowercase hex, of the bytes as downloaded from `url`.
    pub sha256: String,
}

/// Parses `bench/corpus.toml`'s `[[file]]` entries.
///
/// Hand-rolled rather than pulling in a TOML crate: the manifest's shape
/// is fixed (flat string key/value pairs inside repeated `[[file]]`
/// tables, no nesting, no inline tables) and small enough that a
/// dependency buys nothing a short line-based parser doesn't already
/// have. Unrecognized keys and blank/comment lines are ignored; a
/// well-formed manifest round-trips exactly (see `corpus_manifest_parses`
/// below).
#[must_use]
pub fn parse_manifest(toml: &str) -> Vec<ManifestEntry> {
    let mut entries = Vec::new();
    let mut current: Option<ManifestEntry> = None;
    for line in toml.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[file]]" {
            entries.extend(current.take());
            current = Some(ManifestEntry {
                name: String::new(),
                corpus: String::new(),
                url: String::new(),
                sha256: String::new(),
            });
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        if let Some(entry) = current.as_mut() {
            match key.trim() {
                "name" => value.clone_into(&mut entry.name),
                "corpus" => value.clone_into(&mut entry.corpus),
                "url" => value.clone_into(&mut entry.url),
                "sha256" => value.clone_into(&mut entry.sha256),
                _ => {}
            }
        }
    }
    entries.extend(current);
    entries
}

/// Why [`fetch_and_cache`] failed to return trustworthy bytes.
#[derive(Debug)]
pub enum FetchError {
    /// No `[[file]]` entry in the manifest has this `name`.
    UnknownName(String),
    /// The download failed or the local cache couldn't be read/written.
    Io(std::io::Error),
    /// The fetched bytes' SHA-256 doesn't match the manifest's pin.
    ChecksumMismatch {
        /// The entry that failed verification.
        name: String,
        /// The manifest's pinned hash.
        expected: String,
        /// The hash actually computed over the downloaded bytes.
        actual: String,
    },
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownName(name) => write!(f, "no corpus.toml entry named {name:?}"),
            Self::Io(err) => write!(f, "fetch-and-cache I/O error: {err}"),
            Self::ChecksumMismatch {
                name,
                expected,
                actual,
            } => write!(
                f,
                "checksum mismatch for {name:?}: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for FetchError {}

impl From<std::io::Error> for FetchError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// SHA-256 of `data`, lowercase hex.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // `write!` never fails on a `String` target.
        use fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Fetches the entry named `name` from `manifest`, using `cache_dir` as a
/// local on-disk cache keyed by the entry's pinned SHA-256. Verifies
/// every byte against that pin before returning it or writing it to the
/// cache; a checksum mismatch is [`FetchError::ChecksumMismatch`], never
/// a silently wrong result (`research/corpus/POLICY.md`, "refuses to run
/// on a checksum mismatch").
///
/// # Errors
///
/// Returns [`FetchError::UnknownName`] if `manifest` has no entry called
/// `name`, [`FetchError::Io`] if the download or the local cache
/// read/write fails, and [`FetchError::ChecksumMismatch`] if the fetched
/// bytes don't hash to the entry's pin.
pub fn fetch_and_cache(
    manifest: &[ManifestEntry],
    name: &str,
    cache_dir: &Path,
) -> Result<Vec<u8>, FetchError> {
    fetch_and_cache_with(manifest, name, cache_dir, http_get)
}

fn fetch_and_cache_with(
    manifest: &[ManifestEntry],
    name: &str,
    cache_dir: &Path,
    fetch: impl FnOnce(&str) -> Result<Vec<u8>, FetchError>,
) -> Result<Vec<u8>, FetchError> {
    let entry = manifest
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| FetchError::UnknownName(name.to_string()))?;

    // A hash-keyed cache file whose content doesn't match its own key
    // means the file was tampered with or truncated on disk; don't trust
    // it, fall through to refetch instead.
    let cache_path = cache_path_for(cache_dir, entry);
    if let Ok(cached) = std::fs::read(&cache_path)
        && sha256_hex(&cached) == entry.sha256
    {
        return Ok(cached);
    }

    let bytes = fetch(&entry.url)?;
    let actual = sha256_hex(&bytes);
    if actual != entry.sha256 {
        return Err(FetchError::ChecksumMismatch {
            name: name.to_string(),
            expected: entry.sha256.clone(),
            actual,
        });
    }

    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(&cache_path, &bytes)?;
    Ok(bytes)
}

fn cache_path_for(cache_dir: &Path, entry: &ManifestEntry) -> PathBuf {
    cache_dir.join(&entry.sha256)
}

fn http_get(url: &str) -> Result<Vec<u8>, FetchError> {
    let response = ureq::get(url)
        .call()
        .map_err(|err| FetchError::Io(std::io::Error::other(err.to_string())))?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{FetchError, ManifestEntry, fetch_and_cache, fetch_and_cache_with, parse_manifest};
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn corpus_manifest_parses() {
        let toml = r#"
            # a comment, and a blank line above

            [[file]]
            name = "dickens"
            corpus = "silesia"
            url = "https://example.invalid/dickens.bz2"
            sha256 = "abc123"

            [[file]]
            name = "cantrbry"
            corpus = "canterbury"
            url = "https://example.invalid/cantrbry.tar.gz"
            sha256 = "def456"
        "#;
        let entries = parse_manifest(toml);
        assert_eq!(
            entries,
            vec![
                ManifestEntry {
                    name: "dickens".to_string(),
                    corpus: "silesia".to_string(),
                    url: "https://example.invalid/dickens.bz2".to_string(),
                    sha256: "abc123".to_string(),
                },
                ManifestEntry {
                    name: "cantrbry".to_string(),
                    corpus: "canterbury".to_string(),
                    url: "https://example.invalid/cantrbry.tar.gz".to_string(),
                    sha256: "def456".to_string(),
                },
            ]
        );
    }

    #[test]
    fn real_manifest_parses_to_thirteen_entries_with_valid_pins() {
        let entries = parse_manifest(include_str!("../corpus.toml"));
        assert_eq!(entries.len(), 13);
        for entry in &entries {
            assert!(!entry.name.is_empty());
            assert!(
                entry.url.starts_with("https://"),
                "{} url isn't https",
                entry.name
            );
            assert_eq!(entry.sha256.len(), 64, "{} has a malformed pin", entry.name);
            assert!(
                entry.sha256.chars().all(|c| c.is_ascii_hexdigit()),
                "{} pin isn't hex",
                entry.name
            );
        }
        assert_eq!(entries.iter().filter(|e| e.corpus == "silesia").count(), 12);
        assert_eq!(
            entries.iter().filter(|e| e.corpus == "canterbury").count(),
            1
        );
    }

    fn temp_cache_dir(label: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mothergod-bench-corpus-test-{label}-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn manifest_with_one_entry(sha256: &str) -> Vec<ManifestEntry> {
        vec![ManifestEntry {
            name: "fixture".to_string(),
            corpus: "test".to_string(),
            url: "https://example.invalid/fixture".to_string(),
            sha256: sha256.to_string(),
        }]
    }

    #[test]
    fn fetch_and_cache_verifies_and_writes_the_cache() {
        let payload = b"held-out final bytes".to_vec();
        let expected = super::sha256_hex(&payload);
        let manifest = manifest_with_one_entry(&expected);
        let cache_dir = temp_cache_dir("hit");

        let fetched = payload.clone();
        let result =
            fetch_and_cache_with(&manifest, "fixture", &cache_dir, move |_url| Ok(fetched));
        assert_eq!(result.unwrap(), payload);
        assert!(cache_dir.join(&expected).exists());
    }

    #[test]
    fn fetch_and_cache_serves_from_cache_without_fetching_again() {
        let payload = b"cached bytes".to_vec();
        let expected = super::sha256_hex(&payload);
        let manifest = manifest_with_one_entry(&expected);
        let cache_dir = temp_cache_dir("cached");

        fetch_and_cache_with(&manifest, "fixture", &cache_dir, {
            let payload = payload.clone();
            move |_url| Ok(payload)
        })
        .unwrap();

        // A fetch closure that panics if called at all proves the second
        // call served the cache instead of re-fetching.
        let result = fetch_and_cache_with(&manifest, "fixture", &cache_dir, |_url| {
            panic!("should not re-fetch a cache hit")
        });
        assert_eq!(result.unwrap(), payload);
    }

    #[test]
    fn fetch_and_cache_rejects_a_checksum_mismatch_and_does_not_cache_it() {
        let manifest = manifest_with_one_entry(
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        let cache_dir = temp_cache_dir("mismatch");

        let result = fetch_and_cache_with(&manifest, "fixture", &cache_dir, |_url| {
            Ok(b"not what was pinned".to_vec())
        });
        assert!(matches!(result, Err(FetchError::ChecksumMismatch { .. })));
        assert!(
            std::fs::read_dir(&cache_dir).is_err(),
            "a rejected download must not be cached"
        );
    }

    #[test]
    fn fetch_and_cache_rejects_an_unknown_name() {
        let manifest = manifest_with_one_entry("abc123");
        let cache_dir = temp_cache_dir("unknown");
        let result = fetch_and_cache(&manifest, "does-not-exist", &cache_dir);
        assert!(matches!(result, Err(FetchError::UnknownName(name)) if name == "does-not-exist"));
    }

    #[test]
    #[ignore = "hits the real network; run explicitly with `cargo test -- --ignored` to smoke-test corpus.toml's pins"]
    fn fetch_and_cache_smoke_tests_the_real_pins() {
        let manifest = parse_manifest(include_str!("../corpus.toml"));
        let cache_dir = temp_cache_dir("smoke");
        for name in ["xml", "cantrbry"] {
            fetch_and_cache(&manifest, name, &cache_dir)
                .unwrap_or_else(|err| panic!("fetching {name} failed: {err}"));
        }
    }
}
