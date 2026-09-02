//! Writes `frame_gen`'s output as corpus seed files (issue #451,
//! mechanism item 2), run once per `fuzz-check` job before the fuzz
//! steps. A fresh corpus (first run, or a cache miss) starts seeded
//! instead of empty; an already-populated persistent corpus (#450) just
//! gains a handful of duplicate-content files, which `cargo fuzz cmin`
//! prunes back out. Run from the `fuzz/` directory (`cargo run --bin
//! seed_corpus`), matching every other cargo-fuzz command in
//! `fuzz-check.yml`; writes under `corpus/`, relative to that directory.

use std::fs;
use std::path::Path;

use mothergod_fuzz_support::frame_gen;

fn write_corpus(dir: &str, entries: impl IntoIterator<Item = (String, Vec<u8>)>) {
    let dir = Path::new("corpus").join(dir);
    fs::create_dir_all(&dir).expect("create corpus dir");
    for (name, bytes) in entries {
        fs::write(dir.join(name), bytes).expect("write corpus seed");
    }
}

fn main() {
    let frames = frame_gen::frames();

    // decode_arbitrary decodes its input directly: seed it with the
    // frames themselves, the whole point of frame_gen (issue #451).
    write_corpus(
        "decode_arbitrary",
        frames
            .iter()
            .map(|(name, bytes)| (name.to_string(), bytes.clone())),
    );

    // frame_mutate's input is (selector byte, flip-pair bytes); an empty
    // flip list is a valid seed selecting each frame unmodified, letting
    // libFuzzer's own mutation grow the flips from there.
    write_corpus(
        "frame_mutate",
        (0..frames.len()).map(|i| {
            let selector = u8::try_from(i).expect("frame_gen's catalog stays well under 256");
            (i.to_string(), vec![selector])
        }),
    );

    // roundtrip compresses its input itself: seed it with preimages, not
    // frames.
    write_corpus(
        "roundtrip",
        frame_gen::preimages()
            .into_iter()
            .map(|(name, bytes)| (name.to_string(), bytes)),
    );
}
