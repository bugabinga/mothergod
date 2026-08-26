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
//!   fixtures were generated on.
//! - **Re-encoding is pinned on this toolchain only.** `compress(plaintext)
//!   == golden` is a same-platform regression pin, not a cross-platform
//!   claim: `lz.rs`'s match pricing and `filters.rs`'s filter scoring keep
//!   `f64::log2` (ADR-0024 decision 3, encoder-only), which libm does not
//!   guarantee bit-identical across targets. A future multi-platform CI
//!   matrix (`docs/TESTING.md` layer 5's other half) needs a
//!   `.github/workflows/` change reserved for whoever holds
//!   `GH_ADMIN_TOKEN` (`agents/GOVERNANCE.md`, "Push identity"); this test
//!   only ever runs on the one runner that already executes `cargo test`.
//!
//! When `FORMAT_VERSION` bumps (CLAUDE.md hard rule 5), add a new pair
//! named for the new version; keep every old pair so `decompress` staying
//! able to read historical frames (`docs/TESTING.md` layer 5's second
//! bullet) is a running test, not a claim in a doc comment.

use std::fs;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

#[test]
fn fixtures_decode_and_reencode_to_the_pinned_frame() {
    let dir = golden_dir();
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("tests/golden must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("mgdc") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("golden fixture has a UTF-8 stem")
            .to_string();
        let golden = fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let plaintext_path = path.with_extension("plaintext");
        let plaintext =
            fs::read(&plaintext_path).unwrap_or_else(|e| panic!("read {plaintext_path:?}: {e}"));

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

        if declared_version == mothergod::FORMAT_VERSION {
            assert_eq!(
                mothergod::compress(&plaintext),
                golden,
                "fixture {name:?}: current build re-encoded the pinned plaintext to a \
                 different frame than the committed golden one. If this is an intentional \
                 bitstream change, it needs FORMAT_VERSION bumped and an ADR (CLAUDE.md hard \
                 rule 5), and a new golden fixture alongside this one, not an edit to it."
            );
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "golden corpus in tests/golden/ must not be empty"
    );
}
