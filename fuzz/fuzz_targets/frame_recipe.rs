#![no_main]

use libfuzzer_sys::fuzz_target;
use mothergod_fuzz_support::frame_gen::PreimageRecipe;

// Structured mutation in frame-space (issue #451, mechanism item 3): the
// other two frame_gen-based targets mutate bytes, either raw
// (decode_arbitrary) or near an already-encoded frame (frame_mutate).
// This one derives `Arbitrary` on `PreimageRecipe` instead, so libFuzzer
// mutates the small parameter space that shape comes from (a byte, a
// length, a seed) and every mutation stays a recognizable member of the
// same shape family, rather than drifting toward whatever the header
// check rejects. Every recipe compresses through the real encoder and
// must round-trip: CLAUDE.md hard rule 1, exercised here the same way
// `frame_gen::frames()` exercises it for the fixed dozen.
fuzz_target!(|recipe: PreimageRecipe| {
    let data = recipe.to_bytes();
    let frame = mothergod::compress(&data);
    assert_eq!(
        mothergod::decompress(&frame).as_deref(),
        Ok(data.as_slice()),
        "frame_recipe {recipe:?} failed to roundtrip"
    );
});
