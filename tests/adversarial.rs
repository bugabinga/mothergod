//! Layer 2 of `docs/TESTING.md`: the decoder's contract is never panic, on
//! any input. The seed corpus in `tests/adversarial/` holds tiny fixtures
//! deliberately built to be invalid (truncations at every header boundary,
//! bit-flipped magic, a future format version, an unknown method): every
//! fixture here must decode to a graceful `Err`, never a panic and never a
//! false `Ok`. Fuzz-found crashers (`docs/TESTING.md` layer 3) get promoted
//! into this directory as regression seeds.
//!
//! A bare `is_err()` passes a seed that an earlier check rejects, so a
//! seed named for the bound check can die to the version gate and nothing
//! shows it. Each seed is therefore pinned to the [`Error`] variant it
//! must reach, in [`PINS`], and a seed on disk without a row fails. Only
//! the variant is pinned, never its payload.

use std::fs;
use std::mem::discriminant;
use std::path::{Path, PathBuf};

use mothergod::Error;

/// Seed file name to the error variant `decompress` must return for it.
/// A variant's payload is a placeholder: [`discriminant`] ignores it.
const PINS: &[(&str, Error)] = &[
    ("all-0x00", Error::BadMagic),
    ("all-0xff", Error::BadMagic),
    ("bad-magic", Error::BadMagic),
    ("bad-magic-bitflip", Error::BadMagic),
    ("empty", Error::Truncated),
    ("future-version", Error::UnsupportedVersion(0)),
    ("lz-declared-length-mismatch", Error::Corrupt),
    (
        "lz-declared-size-amplification-bomb",
        Error::TooLarge { len: 0, max: 0 },
    ),
    ("lz-declared-size-bomb", Error::TooLarge { len: 0, max: 0 }),
    ("lz-truncated-header", Error::Truncated),
    ("lz-truncated-literal-stream", Error::Corrupt),
    ("lz-unknown-filter-selector", Error::Corrupt),
    ("random-garbage", Error::BadMagic),
    ("stored-declared-length-max", Error::Truncated),
    ("stored-trailing-bytes", Error::Corrupt),
    ("stored-truncated-body", Error::Truncated),
    ("stored-truncated-length", Error::Truncated),
    ("truncated-1-byte", Error::Truncated),
    ("truncated-after-magic", Error::Truncated),
    ("truncated-after-version", Error::Truncated),
    ("truncated-magic-partial", Error::Truncated),
    ("unknown-method", Error::UnknownMethod(0)),
    ("unknown-method-with-payload", Error::UnknownMethod(0)),
];

#[test]
fn seed_corpus_decodes_to_pinned_errors() {
    let dir = std::env::var_os("MOTHERGOD_ADVERSARIAL_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/adversarial"),
        PathBuf::from,
    );
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("tests/adversarial must exist") {
        let path = entry.expect("readable dir entry").path();
        if !path.is_file() {
            continue;
        }
        let data = fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let (_, want) = PINS
            .iter()
            .find(|(pinned, _)| *pinned == name)
            .unwrap_or_else(|| {
                panic!("seed {name:?} has no row in PINS: pin the error it reaches")
            });
        match mothergod::decompress(&data) {
            Ok(_) => panic!("seed {name:?} was expected to be rejected, but decoded successfully"),
            Err(got) => assert!(
                discriminant(&got) == discriminant(want),
                "seed {name:?} reached {got:?}, pinned to the {want:?} variant"
            ),
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "seed corpus in tests/adversarial/ must not be empty"
    );
}
