#![no_main]

use libfuzzer_sys::fuzz_target;

// CLAUDE.md hard rule 1 as an executable: `decompress(compress(x)) == x`
// for arbitrary `x`, always. `compress` never fails, so an `expect` here
// on `decompress` is a genuine bug report, not a false positive.
fuzz_target!(|data: &[u8]| {
    let compressed = mothergod::compress(data);
    let decompressed = mothergod::decompress(&compressed).expect("compress output must decode");
    assert_eq!(decompressed, data);
});
