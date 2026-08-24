#![no_main]

use libfuzzer_sys::fuzz_target;

// CLAUDE.md hard rule 2 as an executable: the decoder never panics or
// overallocates unbounded on any input, adversarial or not. `data` is
// arbitrary, unstructured bytes; a decode error is fine, a panic or an
// allocation past `mothergod::codec::MAX_DECODED_LEN` is not.
fuzz_target!(|data: &[u8]| {
    let _ = mothergod::decompress(data);
});
