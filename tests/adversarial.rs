//! Layer 2 of `docs/TESTING.md`: the decoder's contract is never panic, on
//! any input. The seed corpus in `tests/adversarial/` holds tiny fixtures
//! deliberately built to be invalid (truncations at every header boundary,
//! bit-flipped magic, a future format version, an unknown method): every
//! fixture here must decode to a graceful `Err`, never a panic and never a
//! false `Ok`. Fuzz-found crashers (`docs/TESTING.md` layer 3) get promoted
//! into this directory as regression seeds.

use std::fs;
use std::path::Path;

#[test]
fn seed_corpus_decodes_to_graceful_errors() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/adversarial");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("tests/adversarial must exist") {
        let path = entry.expect("readable dir entry").path();
        if !path.is_file() {
            continue;
        }
        let data = fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        assert!(
            mothergod::decompress(&data).is_err(),
            "fixture {name:?} was expected to be rejected, but decoded successfully"
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "seed corpus in tests/adversarial/ must not be empty"
    );
}
