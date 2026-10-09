#![no_main]

use libfuzzer_sys::fuzz_target;

// CLAUDE.md hard rule 1 as an executable: `decompress(compress(x)) == x`
// for arbitrary `x`, always. `compress` never fails, so an `expect` here
// on `decompress` is a genuine bug report, not a false positive. The
// writer leg lives here, not in its own target, because a leg adds no
// target and so takes no share of the daily fuzz budget, which
// `fuzz-check.yml` divides by the target count; and this target feeds
// raw arbitrary bytes where `frame_recipe` feeds structured recipes.
fuzz_target!(|data: &[u8]| {
    let compressed = mothergod::compress(data);
    let decompressed = mothergod::decompress(&compressed).expect("compress output must decode");
    assert_eq!(decompressed, data);

    let mut written = Vec::new();
    mothergod::decompress_to_writer(&compressed, mothergod::codec::MAX_DECODED_LEN, &mut written)
        .expect("compress output must decode through the writer");
    assert_eq!(written, data);
});
