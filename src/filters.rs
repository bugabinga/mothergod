//! Reversible byte-stream transforms (`JOURNAL` S1-A2, M1 port).
//!
//! Filters trade the input for a differently-shaped byte stream that a
//! downstream model can predict more cheaply — never a smaller one on their
//! own. Trial selection (`JOURNAL` S1-A1), not a static rule, decides when
//! to apply a filter; each submodule here only provides the reversible
//! transform itself. The `pick_filters` trial-selection heuristic and
//! wiring behind a [`crate::Method`] variant are follow-up slices
//! (`JOURNAL` S2-D2). Remaining filter kind (reverse): same DEBT entry.

/// Fixed-stride delta filter.
///
/// Wins on data with fixed-width records where corresponding columns of
/// adjacent rows are numerically close (for example interleaved
/// multi-channel audio samples); `JOURNAL` S1-R1 shows the same transform
/// loses on text, where the numeric difference of letters is *more*
/// scattered than the letters themselves.
pub mod delta {
    use std::num::NonZeroUsize;

    /// Replaces each byte at index `i >= stride` with its wrapping
    /// difference from the byte at `i - stride`; the first `stride` bytes
    /// are left as-is.
    ///
    /// Reversible by [`decode`] with the same `stride`. `stride` is
    /// [`NonZeroUsize`] because a zero stride would subtract every byte
    /// from itself, zeroing the data instead of transforming it reversibly
    /// — the type rules out that misuse instead of a runtime check.
    #[must_use]
    pub fn encode(data: &[u8], stride: NonZeroUsize) -> Vec<u8> {
        let stride = stride.get();
        let mut out = data.to_vec();
        for i in stride..out.len() {
            out[i] = data[i].wrapping_sub(data[i - stride]);
        }
        out
    }

    /// Inverts [`encode`] with the same `stride`.
    #[must_use]
    pub fn decode(data: &[u8], stride: NonZeroUsize) -> Vec<u8> {
        let stride = stride.get();
        let mut out = data.to_vec();
        for i in stride..out.len() {
            out[i] = out[i].wrapping_add(out[i - stride]);
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn nz(v: usize) -> NonZeroUsize {
            NonZeroUsize::new(v).unwrap()
        }

        #[test]
        fn roundtrip_empty() {
            let s = nz(1);
            assert_eq!(decode(&encode(&[], s), s), Vec::<u8>::new());
        }

        #[test]
        fn roundtrip_single_byte() {
            let s = nz(1);
            assert_eq!(decode(&encode(&[42], s), s), vec![42]);
        }

        #[test]
        fn roundtrip_shorter_than_stride() {
            // stride longer than the data: every byte is left untouched,
            // both directions are the identity.
            let data = vec![1, 2, 3];
            let s = nz(8);
            assert_eq!(encode(&data, s), data);
            assert_eq!(decode(&data, s), data);
        }

        #[test]
        fn roundtrip_various_strides() {
            let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
            for stride in [1usize, 2, 3, 4, 8, 16, 96, 255, 1000, 1001] {
                let s = nz(stride);
                let encoded = encode(&data, s);
                assert_eq!(decode(&encoded, s), data, "stride {stride}");
            }
        }

        #[test]
        fn encode_is_identity_for_first_stride_bytes() {
            let data = vec![10, 20, 30, 40, 50];
            let s = nz(2);
            let encoded = encode(&data, s);
            assert_eq!(&encoded[..2], &data[..2]);
        }

        #[test]
        fn wrapping_arithmetic_survives_overflow() {
            // Constructed so intermediate subtraction underflows u8; must
            // still round-trip losslessly via wrapping ops.
            let data = vec![0u8, 255, 1, 254, 2];
            let s = nz(1);
            assert_eq!(decode(&encode(&data, s), s), data);
        }
    }
}

/// Row-major to column-major byte reordering.
///
/// Treats `data` as rows of `columns` bytes each (the last row possibly
/// short) and rewrites it column by column: all bytes at column 0, then all
/// bytes at column 1, and so on. Wins on data with a fixed record width
/// where a column carries its own regularity across rows (`JOURNAL` S1-A2's
/// x-ray dataset: −0.47 b/B). Distinct from [`delta`], which predicts a
/// column's *next* value from the previous row; this filter regroups
/// columns so a downstream model with only short-range context can see
/// that regularity at all.
pub mod transpose {
    use std::num::NonZeroUsize;

    /// Rewrites `data`, interpreted as rows of `columns` bytes, column by
    /// column.
    ///
    /// Reversible by [`decode`] with the same `columns`. `columns` is
    /// [`NonZeroUsize`] because a zero column count has no rows to
    /// transpose — the type rules out that empty case instead of a
    /// runtime check.
    #[must_use]
    pub fn encode(data: &[u8], columns: NonZeroUsize) -> Vec<u8> {
        let columns = columns.get();
        let n = data.len();
        let mut out = Vec::with_capacity(n);
        for start in 0..columns {
            let mut i = start;
            while i < n {
                out.push(data[i]);
                i += columns;
            }
        }
        out
    }

    /// Inverts [`encode`] with the same `columns`.
    #[must_use]
    pub fn decode(data: &[u8], columns: NonZeroUsize) -> Vec<u8> {
        let columns = columns.get();
        let n = data.len();
        let mut out = vec![0u8; n];
        let mut pos = 0usize;
        for start in 0..columns {
            let mut i = start;
            while i < n {
                out[i] = data[pos];
                pos += 1;
                i += columns;
            }
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn nz(v: usize) -> NonZeroUsize {
            NonZeroUsize::new(v).unwrap()
        }

        #[test]
        fn roundtrip_empty() {
            let c = nz(1);
            assert_eq!(decode(&encode(&[], c), c), Vec::<u8>::new());
        }

        #[test]
        fn roundtrip_single_byte() {
            let c = nz(1);
            assert_eq!(decode(&encode(&[42], c), c), vec![42]);
        }

        #[test]
        fn roundtrip_fewer_rows_than_columns() {
            // columns wider than the data: every row has one element, so
            // both directions are the identity.
            let data = vec![1, 2, 3];
            let c = nz(8);
            assert_eq!(encode(&data, c), data);
            assert_eq!(decode(&data, c), data);
        }

        #[test]
        fn roundtrip_various_column_counts() {
            let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
            for columns in [1usize, 2, 3, 4, 8, 16, 96, 255, 1000, 1001] {
                let c = nz(columns);
                let encoded = encode(&data, c);
                assert_eq!(decode(&encoded, c), data, "columns {columns}");
            }
        }

        #[test]
        fn encode_groups_by_column() {
            // 2 columns, 3 rows (last row short): [a0 a1 | b0 b1 | c0] ->
            // column 0 then column 1.
            let data = vec![b'a', b'A', b'b', b'B', b'c'];
            let c = nz(2);
            assert_eq!(encode(&data, c), vec![b'a', b'b', b'c', b'A', b'B']);
        }

        #[test]
        fn single_column_is_identity() {
            let data = vec![10, 20, 30, 40, 50];
            let c = nz(1);
            assert_eq!(encode(&data, c), data);
            assert_eq!(decode(&data, c), data);
        }
    }
}

/// x86 call/jmp (BCJ) relative-to-absolute address filter.
///
/// Rewrites the 4-byte little-endian operand following every `0xE8`/`0xE9`
/// opcode (`call rel32` / `jmp rel32`) between a position-relative offset
/// and an absolute one. Executable code's call targets cluster (many calls
/// target the same handful of functions); as relative offsets each
/// occurrence encodes a different byte pattern, but as absolute addresses
/// they collide, which is what a downstream model actually matches
/// against. `JOURNAL` S1-A2.
pub mod bcj {
    /// Byte length of an opcode plus its rel32 operand: the unit the scan
    /// advances by on a match, and the position `encode`/`decode` measure
    /// the call/jmp target's absolute address from.
    const INSTRUCTION_LEN: usize = 5;

    /// Walks `data`, rewriting each `0xE8`/`0xE9` opcode's operand through
    /// `rewrite_operand`.
    ///
    /// Shared by [`encode`] and [`decode`], which differ only in whether
    /// the operand is added to or subtracted from the post-instruction
    /// address: the scan itself (which positions count as an instruction,
    /// how far it advances) must stay identical between the two directions,
    /// or `decode` would rediscover different positions than `encode`
    /// wrote them at.
    fn rewrite(data: &[u8], rewrite_operand: impl Fn(u32, u32) -> u32) -> Vec<u8> {
        let mut out = data.to_vec();
        let n = out.len();
        let mut i = 0usize;
        while i + INSTRUCTION_LEN <= n {
            if out[i] == 0xE8 || out[i] == 0xE9 {
                let operand = u32::from_le_bytes([out[i + 1], out[i + 2], out[i + 3], out[i + 4]]);
                // x86 rel32 addressing itself wraps at 2^32; truncating
                // the position to u32 before adding matches that hardware
                // semantic rather than losing information.
                #[allow(clippy::cast_possible_truncation)]
                let post_addr = (i as u32).wrapping_add(INSTRUCTION_LEN as u32);
                let new_operand = rewrite_operand(operand, post_addr);
                out[i + 1..i + INSTRUCTION_LEN].copy_from_slice(&new_operand.to_le_bytes());
                i += INSTRUCTION_LEN;
            } else {
                i += 1;
            }
        }
        out
    }

    /// Rewrites each `0xE8`/`0xE9` opcode's operand from relative to
    /// absolute.
    ///
    /// Reversible by [`decode`]. Only the opcode byte gates which
    /// positions are rewritten; the 4-byte operand that follows is never
    /// itself mistaken for a new opcode, because the scan jumps past the
    /// whole 5-byte instruction on a match — so `decode` rediscovers
    /// exactly the same positions from the same untouched opcode bytes.
    #[must_use]
    pub fn encode(data: &[u8]) -> Vec<u8> {
        rewrite(data, u32::wrapping_add)
    }

    /// Inverts [`encode`].
    #[must_use]
    pub fn decode(data: &[u8]) -> Vec<u8> {
        rewrite(data, u32::wrapping_sub)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn roundtrip_empty() {
            assert_eq!(decode(&encode(&[])), Vec::<u8>::new());
        }

        #[test]
        fn roundtrip_too_short_for_any_instruction() {
            // An 0xE8 opcode with fewer than 4 trailing bytes has no full
            // operand to rewrite; both directions are the identity.
            let data = vec![0xE8, 0x01, 0x02, 0x03];
            assert_eq!(encode(&data), data);
            assert_eq!(decode(&data), data);
        }

        #[test]
        fn encode_rewrites_e8_call_operand() {
            // call rel32 at offset 0, relative operand 0x10 -> absolute
            // target 0x10 + (0 + 5) = 0x15.
            let data = vec![0xE8, 0x10, 0x00, 0x00, 0x00];
            assert_eq!(encode(&data), vec![0xE8, 0x15, 0x00, 0x00, 0x00]);
        }

        #[test]
        fn encode_rewrites_e9_jmp_operand() {
            let data = vec![0xE9, 0x10, 0x00, 0x00, 0x00];
            assert_eq!(encode(&data), vec![0xE9, 0x15, 0x00, 0x00, 0x00]);
        }

        #[test]
        fn encode_is_identity_when_no_opcode_present() {
            let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
            assert_eq!(encode(&data), data);
        }

        #[test]
        fn roundtrip_various_data() {
            // 0xE8 == 232 and 0xE9 == 233 fall inside this cycle, so the
            // scan is actually exercised, not just a no-op pass-through.
            let data: Vec<u8> = (0..=255u8).cycle().take(2000).collect();
            assert_eq!(decode(&encode(&data)), data);
        }

        #[test]
        fn roundtrip_adjacent_instructions() {
            let mut data = vec![];
            for _ in 0..20 {
                data.push(0xE8);
                data.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
            }
            assert_eq!(decode(&encode(&data)), data);
        }

        #[test]
        fn roundtrip_wrapping_overflow() {
            // Operand large enough that adding the post-instruction
            // address wraps u32; must still round-trip losslessly.
            let data = vec![0xE8, 0xFF, 0xFF, 0xFF, 0xFF];
            assert_eq!(decode(&encode(&data)), data);
        }
    }
}

/// Standard-base64 unwrap.
///
/// Base64-encoded data (email attachments, embedded certs, JSON blobs with
/// inline binary) inflates every 3 source bytes to 4 printable ones; a
/// downstream model sees only the 4-symbol blow-up, never the binary
/// structure underneath. Unwrapping it back to raw bytes before modeling
/// was the single biggest ratio drop of the founding session (`JOURNAL`
/// S1-A2). Unlike [`delta`]/[`transpose`]/[`bcj`], this filter's decision
/// is data-dependent rather than a caller-supplied parameter, so it can't
/// be a pure `data -> data` pair: whether `data` was in fact unwrapped has
/// to survive into [`decode`](base64_unwrap::decode).
/// [`encode`](base64_unwrap::encode) therefore always prepends one flag
/// byte (`1` = unwrapped, `0` = passed through) ahead of its output, and
/// [`decode`](base64_unwrap::decode) reads that byte back instead of
/// taking a filter parameter.
pub mod base64_unwrap {
    /// Standard base64 alphabet (RFC 4648 with `+`/`/` and `=` padding),
    /// indexed by 6-bit value.
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /// Shortest input worth trying to unwrap. Below this, the one-byte
    /// flag overhead can't be recouped even in the best case.
    const MIN_LEN: usize = 8;

    /// How much of the input the alphabet scan checks before giving up.
    /// Matches the founding session's proxy: full-buffer decode is tried
    /// only after this cheap prefix check passes, so a large non-base64
    /// input costs a bounded scan, not a wasted full decode attempt.
    const SCAN_LIMIT: usize = 4096;

    fn decode_char(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    fn is_base64_byte(b: u8) -> bool {
        decode_char(b).is_some() || b == b'='
    }

    /// Strict standard-base64 decode: `=` padding is accepted only as the
    /// last group's trailing 1 or 2 characters, never elsewhere. Returns
    /// `None` on any invalid character or invalid padding placement;
    /// non-canonical padding bits (set but insignificant) are caught by
    /// the round-trip check in [`encode`], not here.
    fn try_decode(data: &[u8]) -> Option<Vec<u8>> {
        if data.is_empty() || !data.len().is_multiple_of(4) {
            return None;
        }
        let (groups, _) = data.as_chunks::<4>();
        let group_count = groups.len();
        let mut out = Vec::with_capacity(group_count * 3);
        for (idx, group) in groups.iter().enumerate() {
            let pad = group.iter().rev().take_while(|&&b| b == b'=').count();
            let is_last = idx + 1 == group_count;
            if pad > 2 || (pad > 0 && !is_last) {
                return None;
            }
            let digits = &group[..4 - pad];
            if digits.contains(&b'=') {
                return None;
            }
            let mut vals = [0u8; 4];
            for (v, &b) in vals.iter_mut().zip(digits) {
                *v = decode_char(b)?;
            }
            out.push((vals[0] << 2) | (vals[1] >> 4));
            if pad < 2 {
                out.push((vals[1] << 4) | (vals[2] >> 2));
            }
            if pad < 1 {
                out.push((vals[2] << 6) | vals[3]);
            }
        }
        Some(out)
    }

    fn b64_encode(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0];
            let b1 = chunk.get(1).copied();
            let b2 = chunk.get(2).copied();
            out.push(ALPHABET[(b0 >> 2) as usize]);
            out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1.unwrap_or(0) >> 4)) as usize]);
            out.push(match b1 {
                Some(b1) => ALPHABET[(((b1 & 0x0F) << 2) | (b2.unwrap_or(0) >> 6)) as usize],
                None => b'=',
            });
            out.push(match b2 {
                Some(b2) => ALPHABET[(b2 & 0x3F) as usize],
                None => b'=',
            });
        }
        out
    }

    /// Unwraps `data` to its decoded form when `data` is valid, canonical
    /// standard base64 (checked by decoding it and re-encoding the result:
    /// equal to `data` iff no information — non-canonical padding bits,
    /// alternate alphabets — was thrown away). Otherwise passes `data`
    /// through unchanged. Either way the result carries a one-byte prefix
    /// (`1` unwrapped, `0` passed through) so [`decode`] knows which
    /// happened; empty `data` is too short to be worth trying and is
    /// always passed through.
    #[must_use]
    pub fn encode(data: &[u8]) -> Vec<u8> {
        let looks_like_base64 = data.len() >= MIN_LEN
            && data.len().is_multiple_of(4)
            && data[..data.len().min(SCAN_LIMIT)]
                .iter()
                .all(|&b| is_base64_byte(b));
        if looks_like_base64
            && let Some(decoded) = try_decode(data)
            && b64_encode(&decoded) == data
        {
            let mut out = Vec::with_capacity(1 + decoded.len());
            out.push(1);
            out.extend_from_slice(&decoded);
            return out;
        }
        let mut out = Vec::with_capacity(1 + data.len());
        out.push(0);
        out.extend_from_slice(data);
        out
    }

    /// Inverts [`encode`]. Reads the flag byte [`encode`] always writes
    /// first; any value other than `1` is treated as "passed through" (the
    /// same as `0`), and empty `data` — no flag byte at all — decodes to
    /// empty, so this never panics regardless of what `data` holds.
    #[must_use]
    pub fn decode(data: &[u8]) -> Vec<u8> {
        match data.split_first() {
            Some((1, rest)) => b64_encode(rest),
            Some((_, rest)) => rest.to_vec(),
            None => Vec::new(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn roundtrip_empty() {
            assert_eq!(decode(&encode(&[])), Vec::<u8>::new());
        }

        #[test]
        fn roundtrip_too_short_to_try() {
            // Below MIN_LEN: always passed through, even though "YWJj" (7
            // bytes short by one) would otherwise decode cleanly.
            let data = b"YWJj".as_slice();
            assert_eq!(&decode(&encode(data)), data);
            assert_eq!(encode(data)[0], 0);
        }

        #[test]
        fn encode_unwraps_valid_base64() {
            // "aGVsbG8gd29ybGQ=" == base64("hello world")
            let data = b"aGVsbG8gd29ybGQ=".as_slice();
            let encoded = encode(data);
            assert_eq!(encoded[0], 1);
            assert_eq!(&encoded[1..], b"hello world");
            assert_eq!(decode(&encoded), data);
        }

        #[test]
        fn encode_passes_through_non_base64() {
            let data = b"not base64 data!".as_slice();
            let encoded = encode(data);
            assert_eq!(encoded[0], 0);
            assert_eq!(&encoded[1..], data);
            assert_eq!(decode(&encoded), data);
        }

        #[test]
        fn encode_passes_through_invalid_padding_placement() {
            // '=' before the final group: alphabet-scan-eligible (same
            // length class) but not valid base64.
            let data = b"AB==CDEF".as_slice();
            let encoded = encode(data);
            assert_eq!(encoded[0], 0);
            assert_eq!(decode(&encoded), data);
        }

        #[test]
        fn encode_passes_through_non_canonical_padding_bits() {
            // "aGVsbG9=" decodes to 5 bytes but its trailing group carries
            // non-zero padding bits, so re-encoding would not reproduce
            // this exact string: must pass through, not silently accept.
            let data = b"aGVsbG9=".as_slice();
            let decoded_roundtrips = try_decode(data).is_some_and(|d| b64_encode(&d) == data);
            assert!(!decoded_roundtrips, "fixture must exercise the guard");
            let encoded = encode(data);
            assert_eq!(encoded[0], 0);
            assert_eq!(decode(&encoded), data);
        }

        #[test]
        fn roundtrip_all_padding_lengths() {
            // Shorter messages wrap to fewer than MIN_LEN base64 bytes and
            // are covered by `roundtrip_too_short_to_try` instead.
            for msg in [
                "abcd",
                "abcde",
                "abcdef",
                "abcdefg",
                "abcdefgh",
                "abcdefghi",
            ] {
                let wrapped = b64_encode(msg.as_bytes());
                let encoded = encode(&wrapped);
                assert_eq!(encoded[0], 1, "message {msg:?} should unwrap");
                assert_eq!(&encoded[1..], msg.as_bytes());
                assert_eq!(decode(&encoded), wrapped, "message {msg:?}");
            }
        }

        #[test]
        fn decode_of_empty_is_empty() {
            assert_eq!(decode(&[]), Vec::<u8>::new());
        }

        #[test]
        fn roundtrip_binary_payload() {
            let raw: Vec<u8> = (0..=255u8).cycle().take(300).collect();
            let wrapped = b64_encode(&raw);
            let encoded = encode(&wrapped);
            assert_eq!(encoded[0], 1);
            assert_eq!(decode(&encoded), wrapped);
        }
    }
}
