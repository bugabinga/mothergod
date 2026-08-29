//! Layer 5 of `docs/TESTING.md`: golden-frame regression tests.
//!
//! `tests/golden/` holds one `<name>.mgdc`/`<name>.plaintext` pair per
//! fixture, named `v<FORMAT_VERSION>-...`. Two claims, of different
//! strength:
//!
//! - **Decode is pinned across platforms.** `decompress(golden) ==
//!   plaintext` is a real cross-platform guarantee: the decode path is
//!   integer-only (`JOURNAL` S1-A5, `docs/adr/0024-no-libm-on-the-decode-path.md`
//!   decision 1), so this fixture must decode identically on every target
//!   in `docs/TESTING.md`'s runtime matrix, not just the runner these
//!   fixtures were generated on. Every fixture ever committed, current or
//!   superseded, carries this claim forever.
//! - **Re-encoding is pinned per libm as a regression check, not
//!   guaranteed across platforms.** `compress(plaintext) == golden` holds
//!   only as far as `f64::log2` agrees between libms: `lz.rs`'s match
//!   pricing and `filters.rs`'s filter scoring keep it (ADR-0024
//!   decision 3, encoder-only), and libm does not promise bit-identical
//!   results across targets. The weekly monster matrix
//!   (`.github/workflows/monster.yml`) runs this test on every runtime
//!   target anyway, and every libm tried so far (glibc, musl, MSVC CRT,
//!   mingw, Darwin) agrees on these fixtures. A re-encode failure on one
//!   platform only is that platform's libm disagreeing: a finding to
//!   record against ADR-0024's boundary, not a decode regression.
//!
//! Off the build host (the Android emulator lane), `MOTHERGOD_GOLDEN_DIR`
//! overrides the fixture directory; `.github/scripts/android-runner` pushes
//! `tests/golden/` to the device and sets it.
//!
//! `FORMAT_VERSION` versions the decode contract, nothing else (issue #290's
//! ruling). Two different kinds of change trip the re-encode assertion below,
//! and they take different fixes:
//!
//! - **Decode-visible** (frame layout, method byte, model semantics —
//!   CLAUDE.md hard rule 5): bump `FORMAT_VERSION`, write an ADR, add a new
//!   `v<new-version>-...` pair here. The old pair stays in `tests/golden/`
//!   unchanged forever, since its `declared_version` no longer equals
//!   `FORMAT_VERSION` and it drops out of the re-encode check on its own.
//! - **Encoder-only** (a parse or pricing heuristic changes which valid
//!   token sequence `compress()` picks, with `decode` byte-for-byte
//!   unchanged): move the current pair into `tests/golden/superseded/`
//!   unchanged, so it keeps proving that frame still decodes, then
//!   regenerate the pair of the same name in `tests/golden/` from the new
//!   `compress()` output. Declare the regeneration in the PR body with the
//!   measured justification, the `bench/baseline.json` pattern exactly: this
//!   is the one legitimate way to edit a committed golden fixture, and an
//!   undeclared regeneration is a review-time FAIL, not a pass.
//!
//! `tests/golden/superseded/` is decode-only, unconditionally: fixtures
//! there are never re-encode-checked regardless of their declared version,
//! because the whole point of moving one there is that the current encoder
//! no longer produces it.

use std::fs;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    std::env::var_os("MOTHERGOD_GOLDEN_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden"),
        PathBuf::from,
    )
}

fn superseded_dir() -> PathBuf {
    golden_dir().join("superseded")
}

/// Checks one `.mgdc`/`.plaintext` pair's decode pin, and its re-encode pin
/// when `check_reencode` is set and the fixture's declared version is the
/// current `FORMAT_VERSION`. Returns the fixture's declared version.
fn check_fixture(path: &Path, check_reencode: bool) -> u8 {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("golden fixture has a UTF-8 stem")
        .to_string();
    let golden = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let plaintext_path = path.with_extension("plaintext");
    let plaintext = fs::read(&plaintext_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", plaintext_path.display()));

    let declared_version: u8 = name
        .strip_prefix('v')
        .and_then(|rest| rest.split('-').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("fixture {name:?} must be named v<FORMAT_VERSION>-..."));
    assert_eq!(
        golden[mothergod::MAGIC.len()],
        declared_version,
        "fixture {name:?}'s file name and its frame's version byte disagree"
    );

    assert_eq!(
        mothergod::decompress(&golden),
        Ok(plaintext.clone()),
        "fixture {name:?}: golden frame must decode to the pinned plaintext"
    );

    if check_reencode && declared_version == mothergod::FORMAT_VERSION {
        assert_eq!(
            mothergod::compress(&plaintext),
            golden,
            "fixture {name:?}: current build re-encoded the pinned plaintext to a \
             different frame than the committed golden one.\n\
             - Decode-visible change: bump FORMAT_VERSION, write an ADR (CLAUDE.md \
             hard rule 5), add a new v<new-version> pair; leave this one alone.\n\
             - Encoder-only change: move this pair into tests/golden/superseded/ \
             unchanged, regenerate {name} here from the new compress() output, and \
             declare the regeneration in the PR body with the measured justification \
             (the bench/baseline.json pattern)."
        );
    }
    declared_version
}

#[test]
fn fixtures_decode_and_reencode_to_the_pinned_frame() {
    let mut checked = 0;
    for entry in fs::read_dir(golden_dir()).expect("tests/golden must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("mgdc") {
            continue;
        }
        check_fixture(&path, true);
        checked += 1;
    }
    assert!(
        checked > 0,
        "golden corpus in tests/golden/ must not be empty"
    );
}

#[test]
fn superseded_fixtures_still_decode() {
    let dir = superseded_dir();
    if !dir.exists() {
        // No fixture has been superseded yet; nothing to check.
        return;
    }
    for entry in fs::read_dir(&dir).expect("readable tests/golden/superseded") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("mgdc") {
            continue;
        }
        check_fixture(&path, false);
    }
}
