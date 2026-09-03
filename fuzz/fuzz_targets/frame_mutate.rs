#![no_main]

use std::sync::LazyLock;

use libfuzzer_sys::fuzz_target;
use mothergod_fuzz_support::{FUZZ_MAX_DECODED_LEN, frame_gen};

// frame_gen::frames() re-runs the real encoder over every preimage
// (compress trials each shortlisted filter candidate through a full
// optimal-parse + entropy-coding pass), so it is computed once per
// process here rather than per fuzz iteration: libFuzzer calls this
// closure thousands of times a second, and redoing that work on every
// call would collapse throughput to frame_gen's own cost instead of the
// decoder's.
static FRAMES: LazyLock<Vec<(&'static str, Vec<u8>)>> = LazyLock::new(frame_gen::frames);

// Structured mutation (issue #451, mechanism items 1-2): `decode_arbitrary`
// mutates from raw bytes, so libFuzzer spends nearly all its budget on the
// header check (BadMagic/Truncated) and rarely reaches the decoder's deep
// state machine (rep offsets, filter undo, adaptive model state). This
// target instead selects one of frame_gen's known-valid frames — already
// covering every Method, every filters::select::Candidate kind, and the
// rep cache — and applies `data`'s remaining bytes as a handful of
// single-byte XOR flips, so mutation explores the *neighborhood* of real
// frames instead of starting from nothing. Same contract as
// decode_arbitrary: CLAUDE.md hard rule 2, a decode error is fine, a panic
// or unbounded allocation is not. `decompress_bounded`, not `decompress`:
// a flip can land on the declared-length field same as decode_arbitrary's
// raw bytes can, so it needs the same cap (`FUZZ_MAX_DECODED_LEN`'s docs).
fuzz_target!(|data: &[u8]| {
    let Some((&selector, flips)) = data.split_first() else {
        return;
    };
    let (_, base_frame) = &FRAMES[usize::from(selector) % FRAMES.len()];
    if base_frame.is_empty() {
        return;
    }
    let mut frame = base_frame.clone();
    for &[at, xor] in flips.as_chunks::<2>().0 {
        let offset = usize::from(at) % frame.len();
        frame[offset] ^= xor;
    }
    let _ = mothergod::decompress_bounded(&frame, FUZZ_MAX_DECODED_LEN);
});
