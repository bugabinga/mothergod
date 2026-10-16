/// Deterministic pseudo-random symbol stream for round-trip tests in
/// `coder`, `model`, and `literal`: those three modules' test suites
/// each need a long, deterministic-but-unstructured stream with no
/// external RNG dependency, and had each hand-rolled the same
/// xorshift32 step to get one.
///
/// xorshift32 generator: `next()` advances the state and returns it,
/// so the seed itself is never yielded, only states derived from it.
pub(crate) struct Xorshift32(u32);

impl Xorshift32 {
    pub(crate) fn new(seed: u32) -> Self {
        Self(seed)
    }
}

impl Iterator for Xorshift32 {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        Some(self.0)
    }
}

/// `v` as a [`std::num::NonZeroUsize`], for tests that need one as a
/// filter parameter (a delta stride, a transpose column count) and
/// know `v` is nonzero by construction: `filters::delta` and
/// `filters::transpose`'s test suites each need this same conversion
/// and had each hand-rolled the identical helper to get it.
///
/// # Panics
///
/// Panics if `v` is zero: every call site passes a literal already
/// known to be nonzero, so this is a test-fixture bug, never
/// something a non-test caller could trigger (this function only
/// exists under `#[cfg(test)]`).
pub(crate) fn nz(v: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(v).unwrap()
}

/// Extracts the [`Error`] from a [`WriteError`] produced by a
/// `decode_to_writer` call, for tests asserting exactly which decode
/// error occurred; `None` for a [`WriteError::Io`] (a `writer` failure,
/// never a decode error): `codec` and `lib`'s test suites each need
/// this same match and had each hand-rolled the identical helper to
/// get it.
pub(crate) fn as_codec_error(err: &super::WriteError) -> Option<&super::Error> {
    match err {
        super::WriteError::Decode(inner) => Some(inner),
        super::WriteError::Io(_) => None,
    }
}

/// The least declared output length for which an `Lz` payload with an empty
/// range-coded stream stays within the Stored floor (`docs/format/SPEC.md`,
/// "Invariants"): a test that hand-builds a header-only frame declares at
/// least this, or the extent parse rejects it as `Corrupt`.
pub(crate) fn floor_declared_len() -> u32 {
    u32::try_from(crate::codec::EXTENT_PREFIX_LEN - crate::STORED_LEN_BYTES)
        .expect("the extent prefix is a few bytes")
}

/// Rewrites the declared output length of the `Lz` `frame` to `declared_len`
/// and its coded length to the most the Stored floor lets that declare, so
/// the extent parse accepts a frame whose stream is not there. The check
/// value is left stale: the extent is read before it.
pub(crate) fn forge_floor_extent(frame: &mut [u8], declared_len: u32) {
    let coded_field = crate::HEADER_LEN + crate::codec::EXTENT_PREFIX_LEN - size_of::<u64>();
    let declared_field = coded_field - 2 * size_of::<u32>();
    let coded_len = u64::from(declared_len) + (crate::STORED_LEN_BYTES as u64)
        - crate::codec::EXTENT_PREFIX_LEN as u64;
    frame[declared_field..declared_field + size_of::<u32>()]
        .copy_from_slice(&declared_len.to_le_bytes());
    frame[coded_field..coded_field + size_of::<u64>()].copy_from_slice(&coded_len.to_le_bytes());
}

/// A `Method::Lz` payload from its parts: the filter `selector`, the
/// declared output length and token count, then `stream`, with the coded
/// length computed from it. Tests that hand-build a payload each needed
/// this layout and had each re-spelled its fields.
pub(crate) fn lz_payload(
    selector: [u8; 2],
    declared_len: u32,
    token_count: u32,
    stream: &[u8],
) -> Vec<u8> {
    let mut payload = selector.to_vec();
    payload.extend_from_slice(&declared_len.to_le_bytes());
    payload.extend_from_slice(&token_count.to_le_bytes());
    payload.extend_from_slice(&(stream.len() as u64).to_le_bytes());
    payload.extend_from_slice(stream);
    payload
}

/// Four independent small-step random walks, one per column, laid out
/// row-major with a 4-byte stride: consecutive same-column bytes drift
/// by a small step (`filters::select::pick` ranks `Candidate::Delta(4)`
/// top for exactly this shape — mirrors its own
/// `pick_selects_delta_for_columnar_drift` test), but consecutive raw
/// bytes belong to different, unrelated walks.
pub(crate) fn columnar_drift_data() -> Vec<u8> {
    const STEPS: [u8; 5] = [0u8.wrapping_sub(2), 0u8.wrapping_sub(1), 0, 1, 2];
    let mut rng = Xorshift32::new(0x1234_5678);
    let mut walk = [64u8, 96, 160, 200];
    let rows = 2000usize;
    let mut data = Vec::with_capacity(rows * walk.len());
    for _ in 0..rows {
        for col in &mut walk {
            let state = rng.next().expect("Xorshift32 never terminates");
            let step = STEPS[usize::try_from(state % 5).unwrap_or(0)];
            *col = col.wrapping_add(step);
            data.push(*col);
        }
    }
    data
}

/// Many `call rel32` instructions, 20 bytes apart (room for a full
/// 5-byte instruction with no overlap), all targeting the same absolute
/// address (0): as raw relative offsets each instance's operand differs
/// by position, but `Candidate::Bcj` rewrites every one of them to the
/// same absolute bytes, which is what gives `encode()`'s trial a real
/// win to find here (mirrors `filters::select::pick`'s own
/// `pick_shortlists_bcj_for_opcode_dense_data` density fixture, but
/// with real operands instead of zeros so the rewrite actually creates
/// the repetition rather than leaving it accidentally already there).
pub(crate) fn bcj_call_dense_data() -> Vec<u8> {
    let mut data = vec![0x90u8; 4000];
    for (idx, chunk) in data.chunks_mut(20).enumerate() {
        chunk[0] = 0xE8;
        let post_addr = u32::try_from(idx * 20 + crate::filters::bcj::INSTRUCTION_LEN)
            .expect("idx * 20 + 5 fits u32 for this fixture's size (4000)");
        let rel = 0u32.wrapping_sub(post_addr);
        chunk[1..crate::filters::bcj::INSTRUCTION_LEN].copy_from_slice(&rel.to_le_bytes());
    }
    data
}

/// Fixed-width records whose columns each cycle through their own
/// period (`research/JOURNAL.md` S1-P5's target shape, same
/// construction as `tests/golden/v11-tabular-columns`), with a fraction
/// of bytes jittered off the clean pattern so the literal model, not
/// just `lz::parse_optimal`'s LZ matches, carries real weight — the
/// data this ADR-0046 slice's own real-bitstream measurement used.
pub(crate) fn tabular_columns_data(columns: usize, rows: usize, seed: u32) -> Vec<u8> {
    let periods: Vec<u32> = (0..columns)
        .map(|c| 3 + ((u32::try_from(c).unwrap() * 7 + seed) % 6))
        .collect();
    let bases: Vec<u8> = (0..columns)
        .map(|c| {
            u8::try_from(
                (u32::try_from(c).unwrap().wrapping_mul(0x1e)).wrapping_add(seed.wrapping_mul(3))
                    % 256,
            )
            .unwrap()
        })
        .collect();
    let mut rng = Xorshift32::new(0x9e37_79b9 ^ seed.wrapping_mul(0x8551_4d97) | 1);
    let mut data = vec![0u8; columns * rows];
    for row in 0..rows {
        for (col, &period) in periods.iter().enumerate() {
            let phase = u32::try_from(row).unwrap() % period;
            let mut v = bases[col].wrapping_add(u8::try_from(phase).unwrap());
            let r = rng.next().expect("Xorshift32 never terminates");
            if r % 100 < 20 {
                let jitter = u8::try_from((r >> 8) % 40).unwrap();
                v = v.wrapping_add(jitter).wrapping_sub(20);
            }
            data[row * columns + col] = v;
        }
    }
    data
}

/// One 45-byte sentence repeated 50 times: input on which the trial encode
/// prefers `Candidate::Delta(45)` to every other filter, the one class here
/// `Candidate::ReverseDelta` does not take (`JOURNAL` S2-A120).
pub(crate) fn repeated_sentence() -> Vec<u8> {
    b"the quick brown fox jumps over the lazy dog. ".repeat(50)
}

/// A stream of a `Stored` frame, an `Lz` frame and a `Stored` frame, with
/// the bytes it decodes to.
pub(crate) fn mixed_stream() -> (Vec<u8>, Vec<u8>) {
    let parts: [Vec<u8>; 3] = [b"hi".to_vec(), repeated_sentence(), Vec::new()];
    let methods = [
        crate::Method::Stored,
        crate::Method::Lz,
        crate::Method::Stored,
    ];
    let mut stream = Vec::new();
    for (part, method) in parts.iter().zip(methods) {
        let frame = crate::compress(part);
        assert_eq!(frame[crate::METHOD_OFFSET], method as u8);
        stream.extend(frame);
    }
    (stream, parts.concat())
}

/// JSON-ish text, `lines` rows long: what `Candidate::Base64` unwraps once
/// `filters::base64_unwrap::decode` has wrapped it.
pub(crate) fn record_text(lines: usize) -> Vec<u8> {
    (0..lines)
        .flat_map(|i| format!("{{\"id\":{i},\"score\":{}}}\n", i * 37 % 1000).into_bytes())
        .collect()
}

/// `rows` fixed-width rows of a little-endian `i64` timestamp, a 4-byte
/// category and a little-endian `f64` measurement, 20 bytes each: input on
/// which the trial encode prefers `Candidate::Reverse` to every other
/// filter (`JOURNAL` S2-A119).
pub(crate) fn le_rows(rows: usize) -> Vec<u8> {
    const CATEGORIES: [&[u8; 4]; 3] = [b"temp", b"hum\0", b"pres"];
    let mut noise = Xorshift32::new(0x9E37_79B9);
    let mut out = Vec::with_capacity(rows * 20);
    let mut timestamp = 1_700_000_000_i64;
    for _ in 0..rows {
        let pick = noise.next().expect("infinite") as usize % CATEGORIES.len();
        let value = 20.0 + f64::from(noise.next().expect("infinite") % 6000) / 1000.0;
        out.extend_from_slice(&timestamp.to_le_bytes());
        timestamp += 60;
        out.extend_from_slice(CATEGORIES[pick]);
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}
