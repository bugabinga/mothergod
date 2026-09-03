//! Support code shared across `fuzz/fuzz_targets/`, kept out of that
//! directory because cargo-fuzz treats every file there as its own
//! `#![no_main]` binary rather than an importable module.

pub mod frame_gen;

/// Decode ceiling for fuzz targets that hand the decoder attacker-shaped
/// bytes (a declared-length field libFuzzer can mutate independent of the
/// content that backs it), never [`mothergod::codec::MAX_DECODED_LEN`]
/// itself.
///
/// libFuzzer's whole model is many fast executions per second; a target
/// that occasionally decodes toward the real 256 MiB ceiling defeats that
/// at today's decode throughput (`research/JOURNAL.md` S1-P6, issue #447:
/// worst case ~0.4 MB/s). One seeded run hit this directly: after #475
/// seeded `decode_arbitrary`'s corpus with real encoded frames, libFuzzer
/// mutated a declared-length field toward the max and spent over 1100s of
/// its 450s budget decoding a single input, blowing `fuzz-check.yml`'s
/// 45-minute timeout and cancelling that night's corpus minimization
/// (run 33726798807). `decompress_bounded` rejects an over-bound declared
/// length before any allocation or decode work, so this cap turns that
/// case back into a sub-millisecond `Err`, not a multi-minute decode,
/// while still exercising every header/parse/filter/entropy code path for
/// any input that fits under it — 1 MiB is generous headroom over every
/// real seed these targets carry (frame_gen's largest preimage is a few
/// KB) without reintroducing the bomb-shaped worst case.
pub const FUZZ_MAX_DECODED_LEN: u32 = 1 << 20;
