#![no_main]

use libfuzzer_sys::fuzz_target;
use mothergod_fuzz_support::{FUZZ_MAX_DECODED_LEN, frame_gen};

// CLAUDE.md hard rule 2 as an executable: the decoder never panics or
// overallocates unbounded on any input, adversarial or not. `data` is
// arbitrary, unstructured bytes; a decode error is fine, a panic or an
// allocation past `mothergod::codec::MAX_DECODED_LEN` is not.
//
// `decompress_bounded`, not `decompress`: `data`'s declared-length field
// is fuzzer-controlled independent of its content, so this is exactly the
// shape a decompression bomb takes. See `FUZZ_MAX_DECODED_LEN`'s docs for
// why an unbounded decode here already cost a night's corpus
// minimization.
//
// Every frame ends in a check value the decoder verifies before it reads
// the payload, which unstructured bytes match once in 2^32. The second
// call seals `data` as a frame body, so a header the fuzzer has found
// leads on into the parse and the models (the shape of an adversary who
// computes the check).
fuzz_target!(|data: &[u8]| {
    let _ = mothergod::decompress_bounded(data, FUZZ_MAX_DECODED_LEN);
    let _ = mothergod::decompress_bounded(&frame_gen::sealed(data), FUZZ_MAX_DECODED_LEN);
});
