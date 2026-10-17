//! Reversible byte-stream transforms.
//!
//! A filter reshapes the input so a downstream model predicts it more
//! cheaply; it does not shrink it. [`select`] shortlists candidates and
//! `crate::codec` trial-encodes each, keeping the smallest (`JOURNAL` S1-A1,
//! ADR-0028).
//!
//! `delta`, `transpose`, `bcj` and `base64_unwrap` invert in two forms. The
//! public `decode` aborts if the allocator cannot satisfy its output buffer.
//! The `pub(crate)` `try_decode`, and each streaming `Undo::try_new`, return
//! `Err` instead, because the decode path must not abort on untrusted input
//! (hard rule 2, `tests/torture.rs`, #453). A pair differs only in how the
//! buffer is built, never in the scan run over it. `reverse` needs no
//! second form: `crate::codec` reverses the buffer it already owns in place.

/// Fixed-stride delta filter.
///
/// Wins on fixed-width records whose corresponding columns in adjacent rows
/// are numerically close (interleaved audio channels); loses on text
/// (`JOURNAL` S1-R1).
pub mod delta {
    use std::num::NonZeroUsize;

    /// Replaces each byte at index `i >= stride` with its wrapping
    /// difference from the byte at `i - stride`; the first `stride` bytes
    /// are left as-is.
    ///
    /// Reversible by [`decode`] with the same `stride`. [`NonZeroUsize`]
    /// because a zero stride would subtract every byte from itself.
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
        let mut out = data.to_vec();
        undo_in_place(&mut out, stride.get());
        out
    }

    /// Fallible [`decode`] (module docs).
    pub(crate) fn try_decode(
        data: &[u8],
        stride: NonZeroUsize,
    ) -> Result<Vec<u8>, std::collections::TryReserveError> {
        let mut out = crate::try_vec_from_slice(data)?;
        undo_in_place(&mut out, stride.get());
        Ok(out)
    }

    /// Scan shared by [`decode`] and [`try_decode`] over `out`, a copy of
    /// the filtered bytes.
    fn undo_in_place(out: &mut [u8], stride: usize) {
        for i in stride..out.len() {
            out[i] = out[i].wrapping_add(out[i - stride]);
        }
    }

    /// Streaming counterpart to [`decode`]: undoes one filtered byte at a
    /// time, so the whole filtered stream is never resident (ROADMAP M4's
    /// bounded-memory decode guarantee).
    ///
    /// `history` holds the last `stride` bytes this instance has undone, the
    /// same values `decode` reads back from `out`. Bounded by `stride` (at
    /// most 255, an 8-bit header field), never by input length.
    pub(crate) struct Undo {
        stride: usize,
        history: Vec<u8>,
        count: usize,
    }

    impl Undo {
        /// An undo state ready for the first filtered byte (fallible: module
        /// docs).
        pub(crate) fn try_new(
            stride: NonZeroUsize,
        ) -> Result<Self, std::collections::TryReserveError> {
            let stride = stride.get();
            Ok(Self {
                stride,
                history: crate::try_filled_vec(stride, 0u8)?,
                count: 0,
            })
        }

        /// Undoes one more filtered byte, returning the raw byte [`decode`]
        /// produces at this stream position. Calls must be in stream order:
        /// each result feeds the call `stride` bytes later.
        pub(crate) fn apply(&mut self, filtered_byte: u8) -> u8 {
            let slot = self.count % self.stride;
            let raw = if self.count < self.stride {
                filtered_byte
            } else {
                filtered_byte.wrapping_add(self.history[slot])
            };
            self.history[slot] = raw;
            self.count += 1;
            raw
        }
    }

    #[cfg(test)]
    mod undo_tests;

    #[cfg(test)]
    mod tests;

    // Not under Miri: same rationale as `coder.rs`'s `mod proptests`
    // header comment (interpretation cost, issue #456).
    #[cfg(test)]
    #[cfg(not(miri))]
    mod proptests;
}

/// Row-major to column-major byte reordering.
///
/// Treats `data` as rows of `columns` bytes each (the last row possibly
/// short) and rewrites it column by column: all bytes at column 0, then all
/// bytes at column 1, and so on. Wins on fixed-width records whose columns
/// are regular across rows (`JOURNAL` S1-A2's x-ray dataset: −0.47 b/B).
/// Where [`delta`] predicts a column's next value from the previous row,
/// this regroups columns so a short-range model can see that regularity.
pub mod transpose {
    use std::num::NonZeroUsize;

    /// Rewrites `data`, interpreted as rows of `columns` bytes, column by
    /// column.
    ///
    /// Reversible by [`decode`] with the same `columns`. [`NonZeroUsize`]
    /// because a zero column count has no rows to transpose.
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
        let mut out = vec![0u8; data.len()];
        decode_into(&mut out, data, columns.get());
        out
    }

    /// Fallible [`decode`] (module docs).
    pub(crate) fn try_decode(
        data: &[u8],
        columns: NonZeroUsize,
    ) -> Result<Vec<u8>, std::collections::TryReserveError> {
        let mut out = crate::try_filled_vec(data.len(), 0u8)?;
        decode_into(&mut out, data, columns.get());
        Ok(out)
    }

    /// Scan shared by [`decode`] and [`try_decode`] over a zero-filled `out`.
    fn decode_into(out: &mut [u8], data: &[u8], columns: usize) {
        let n = out.len();
        let mut pos = 0usize;
        for start in 0..columns {
            let mut i = start;
            while i < n {
                out[i] = data[pos];
                pos += 1;
                i += columns;
            }
        }
    }

    #[cfg(test)]
    mod tests;

    // Not under Miri: same rationale as `coder.rs`'s `mod proptests`
    // header comment (interpretation cost, issue #456).
    #[cfg(test)]
    #[cfg(not(miri))]
    mod proptests;
}

/// x86 call/jmp (BCJ) relative-to-absolute address filter.
///
/// Rewrites the 4-byte little-endian operand following every `0xE8`/`0xE9`
/// opcode (`call rel32` / `jmp rel32`) between a position-relative offset
/// and an absolute one. Calls cluster on a few targets: as relative offsets
/// each occurrence is a different byte pattern, as absolute addresses they
/// repeat, which is what a downstream model matches. `JOURNAL` S1-A2.
pub mod bcj {
    /// Byte length of an opcode plus its rel32 operand: the unit the scan
    /// advances by on a match, and the offset past the opcode that the
    /// absolute address is measured from. `pub(crate)` for `codec`'s test
    /// fixtures.
    pub(crate) const INSTRUCTION_LEN: usize = 5;

    /// Whether `byte` opens a BCJ-eligible instruction (`call rel32` /
    /// `jmp rel32`). The one gate for [`rewrite_in_place`], [`Undo::apply`]
    /// and `select::pick`'s density heuristic.
    pub(crate) const fn is_opcode(byte: u8) -> bool {
        byte == 0xE8 || byte == 0xE9
    }

    /// Copies `data` and rewrites each instruction's operand through
    /// `rewrite_operand`. [`encode`] adds the post-instruction address and
    /// [`decode`] subtracts it; the scan must be identical in both
    /// directions or `decode` finds different positions than `encode` wrote.
    fn rewrite(data: &[u8], rewrite_operand: impl Fn(u32, u32) -> u32) -> Vec<u8> {
        let mut out = data.to_vec();
        rewrite_in_place(&mut out, rewrite_operand);
        out
    }

    /// Rewrites the rel32 operand of the [`INSTRUCTION_LEN`]-byte
    /// `instruction` that starts at stream index `position`, through
    /// `rewrite_operand(operand, post_addr)`. Shared by the batch scan and
    /// [`Undo::apply`] so the two cannot disagree on layout or address.
    fn rewrite_instruction(
        instruction: &mut [u8],
        position: usize,
        rewrite_operand: impl Fn(u32, u32) -> u32,
    ) {
        let operand = u32::from_le_bytes([
            instruction[1],
            instruction[2],
            instruction[3],
            instruction[4],
        ]);
        // x86 rel32 addressing wraps at 2^32, so truncating the position
        // to u32 keeps a stream past 4 GiB round-tripping.
        #[allow(clippy::cast_possible_truncation)]
        let post_addr = (position as u32).wrapping_add(INSTRUCTION_LEN as u32);
        instruction[1..INSTRUCTION_LEN]
            .copy_from_slice(&rewrite_operand(operand, post_addr).to_le_bytes());
    }

    /// Scan shared by [`rewrite`] and [`try_decode`] over `out`, a copy of
    /// the input.
    fn rewrite_in_place(out: &mut [u8], rewrite_operand: impl Fn(u32, u32) -> u32) {
        let n = out.len();
        let mut i = 0usize;
        while i + INSTRUCTION_LEN <= n {
            if is_opcode(out[i]) {
                rewrite_instruction(&mut out[i..i + INSTRUCTION_LEN], i, &rewrite_operand);
                i += INSTRUCTION_LEN;
            } else {
                i += 1;
            }
        }
    }

    /// Rewrites each `0xE8`/`0xE9` opcode's operand from relative to
    /// absolute.
    ///
    /// Reversible by [`decode`]: the scan jumps past each whole
    /// instruction, so an operand is never mistaken for an opcode and
    /// `decode` finds the same positions from the same untouched opcode
    /// bytes.
    #[must_use]
    pub fn encode(data: &[u8]) -> Vec<u8> {
        rewrite(data, u32::wrapping_add)
    }

    /// Inverts [`encode`].
    #[must_use]
    pub fn decode(data: &[u8]) -> Vec<u8> {
        rewrite(data, u32::wrapping_sub)
    }

    /// Fallible [`decode`] (module docs).
    pub(crate) fn try_decode(data: &[u8]) -> Result<Vec<u8>, std::collections::TryReserveError> {
        let mut out = crate::try_vec_from_slice(data)?;
        rewrite_in_place(&mut out, u32::wrapping_sub);
        Ok(out)
    }

    /// Bytes [`Undo::apply`]/[`Undo::finish`] resolved, in order. Fixed
    /// capacity: no heap allocation per filtered byte in the streaming
    /// decode's hot loop.
    pub(crate) struct Resolved {
        buf: [u8; INSTRUCTION_LEN],
        len: u8,
    }

    impl Resolved {
        const NONE: Self = Self {
            buf: [0; INSTRUCTION_LEN],
            len: 0,
        };

        fn one(byte: u8) -> Self {
            let mut buf = [0; INSTRUCTION_LEN];
            buf[0] = byte;
            Self { buf, len: 1 }
        }

        /// `bytes.len()` must be at most [`INSTRUCTION_LEN`]: [`Undo`] never
        /// holds more pending bytes.
        fn from_slice(bytes: &[u8]) -> Self {
            let mut buf = [0; INSTRUCTION_LEN];
            buf[..bytes.len()].copy_from_slice(bytes);
            #[allow(clippy::cast_possible_truncation)]
            let len = bytes.len() as u8;
            Self { buf, len }
        }

        pub(crate) fn as_slice(&self) -> &[u8] {
            &self.buf[..self.len as usize]
        }
    }

    /// Streaming counterpart to [`decode`], as [`delta::Undo`] is for delta
    /// (ROADMAP M4's bounded-memory decode guarantee).
    ///
    /// Unlike delta's lookback, the scan needs *lookahead*: an opcode byte
    /// is recognised on arrival, but its operand needs the
    /// `INSTRUCTION_LEN - 1` bytes after it. `pending` holds the
    /// unresolved bytes; `position` is [`rewrite`]'s `i`, advancing only as
    /// bytes resolve, never while `pending` fills.
    pub(crate) struct Undo {
        pending: Vec<u8>,
        position: usize,
    }

    impl Undo {
        /// A fresh undo state with [`INSTRUCTION_LEN`] bytes of pending
        /// capacity (fallible: module docs).
        pub(crate) fn try_new() -> Result<Self, std::collections::TryReserveError> {
            let mut pending = Vec::new();
            pending.try_reserve_exact(INSTRUCTION_LEN)?;
            Ok(Self {
                pending,
                position: 0,
            })
        }

        /// Feeds one more filtered byte, in stream order, returning the
        /// bytes this call resolved: none while an operand is buffering, one
        /// for a non-opcode byte, all [`INSTRUCTION_LEN`] once an
        /// instruction's operand is complete.
        pub(crate) fn apply(&mut self, filtered_byte: u8) -> Resolved {
            if self.pending.is_empty() {
                if is_opcode(filtered_byte) {
                    self.pending.push(filtered_byte);
                    return Resolved::NONE;
                }
                self.position += 1;
                return Resolved::one(filtered_byte);
            }
            self.pending.push(filtered_byte);
            if self.pending.len() < INSTRUCTION_LEN {
                return Resolved::NONE;
            }
            rewrite_instruction(&mut self.pending, self.position, u32::wrapping_sub);
            self.position += INSTRUCTION_LEN;
            let resolved = Resolved::from_slice(&self.pending);
            self.pending.clear();
            resolved
        }

        /// Flushes bytes still buffered at end of stream: an opcode with
        /// fewer than [`INSTRUCTION_LEN`] bytes after it, passed through
        /// unchanged as [`rewrite`]'s scan bound (`i + INSTRUCTION_LEN <= n`)
        /// leaves it.
        pub(crate) fn finish(&mut self) -> Resolved {
            let resolved = Resolved::from_slice(&self.pending);
            self.pending.clear();
            resolved
        }
    }

    #[cfg(test)]
    mod undo_tests;

    #[cfg(test)]
    mod tests;

    // Not under Miri: same rationale as `coder.rs`'s `mod proptests`
    // header comment (interpretation cost, issue #456).
    #[cfg(test)]
    #[cfg(not(miri))]
    mod proptests;
}

/// Standard-base64 unwrap.
///
/// Base64 inflates every 3 source bytes to 4 printable ones and hides the
/// binary structure underneath; unwrapping it was the biggest ratio drop of
/// the founding session (`JOURNAL` S1-A2). Unlike [`delta`], [`transpose`]
/// and [`bcj`], whether it applies depends on the data:
/// [`encode`](base64_unwrap::encode) answers `None` for input that is not
/// canonical standard base64, and the frame's filter selector, not a flag
/// byte, records that it ran ([`select::Candidate::Base64`]). It is the one
/// filter that changes length, so a frame's output length is derived from
/// its filtered length ([`select::Candidate::output_len`]).
pub mod base64_unwrap {
    /// Standard base64 alphabet (RFC 4648 with `+`/`/` and `=` padding),
    /// indexed by 6-bit value.
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /// Prefix the alphabet scan checks before a full decode is attempted,
    /// so a large non-base64 input costs a bounded scan.
    const SCAN_LIMIT: usize = 4096;

    /// [`ALPHABET`]'s inverse: `DECODE_TABLE[b]` is `Some(v)` iff
    /// `ALPHABET[v] == b`.
    const DECODE_TABLE: [Option<u8>; 256] = {
        let mut table = [None; 256];
        let mut v = 0;
        while v < ALPHABET.len() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "v < ALPHABET.len() == 64, always fits u8"
            )]
            {
                table[ALPHABET[v] as usize] = Some(v as u8);
            }
            v += 1;
        }
        table
    };

    fn decode_char(b: u8) -> Option<u8> {
        DECODE_TABLE[b as usize]
    }

    fn is_base64_byte(b: u8) -> bool {
        decode_char(b).is_some() || b == b'='
    }

    /// Strict standard-base64 decode: `=` padding is accepted only as the
    /// last group's trailing 1 or 2 characters, never elsewhere. Returns
    /// `None` on any invalid character or invalid padding placement;
    /// non-canonical padding bits (set but insignificant) are caught by
    /// the round-trip check in [`encode`], not here.
    fn strict_decode(data: &[u8]) -> Option<Vec<u8>> {
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
            let mut vals = [0u8; 4];
            for (v, &b) in vals.iter_mut().zip(&group[..4 - pad]) {
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

    /// Appends the padded base64 text of `data` to `out`: the scan shared
    /// by [`b64_encode`] and [`try_decode`], which differ only in how
    /// `out` is allocated.
    fn encode_into(data: &[u8], out: &mut Vec<u8>) {
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
    }

    fn b64_encode(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
        encode_into(data, &mut out);
        out
    }

    /// Unwraps `data` to its decoded form when `data` is valid, canonical
    /// standard base64 (checked by decoding it and re-encoding the result:
    /// equal to `data` iff no information, such as non-canonical padding
    /// bits or an alternate alphabet, was thrown away), and answers `None`
    /// otherwise. Empty `data` is not base64, so it answers `None` too.
    #[must_use]
    pub fn encode(data: &[u8]) -> Option<Vec<u8>> {
        if !data.len().is_multiple_of(4)
            || !data[..data.len().min(SCAN_LIMIT)]
                .iter()
                .all(|&b| is_base64_byte(b))
        {
            return None;
        }
        strict_decode(data).filter(|decoded| b64_encode(decoded) == data)
    }

    /// Inverts [`encode`]: the padded base64 text of `data`, `4 *
    /// data.len().div_ceil(3)` bytes. Total, so no byte string is an
    /// invalid unwrapped form.
    #[must_use]
    pub fn decode(data: &[u8]) -> Vec<u8> {
        b64_encode(data)
    }

    /// Fallible [`decode`] (module docs of [`crate::filters`]).
    pub(crate) fn try_decode(data: &[u8]) -> Result<Vec<u8>, std::collections::TryReserveError> {
        let mut out = Vec::new();
        out.try_reserve_exact(data.len().div_ceil(3) * 4)?;
        encode_into(data, &mut out);
        Ok(out)
    }

    #[cfg(test)]
    mod tests;
}

/// Byte-order reversal.
///
/// Wins when a byte is better predicted by the bytes after it than by the
/// bytes before it, which no count over n-grams can see: every such entropy
/// reads the same backwards, so only the trial encode decides
/// (`JOURNAL` S1-A2, S2-A119). Its own inverse, so
/// [`decode`](reverse::decode) is [`encode`](reverse::encode) under the
/// module's encode/decode-pair shape.
pub mod reverse {
    /// Reverses `data`. Self-inverse.
    #[must_use]
    pub fn encode(data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        out.reverse();
        out
    }

    /// Inverts [`encode`], which is its own inverse.
    #[must_use]
    pub fn decode(data: &[u8]) -> Vec<u8> {
        encode(data)
    }

    #[cfg(test)]
    mod tests;
}

/// Trial-selection shortlist: which filters are worth a full trial encode.
///
/// [`select::pick`] narrows the menu with an order-1 entropy proxy on a
/// bounded probe, so the expensive trial encode runs only on candidates
/// worth it (`JOURNAL` S1-A1). It shortlists [`delta`], [`bcj`],
/// [`transpose`], [`base64_unwrap`] and, for every input, [`reverse`]; and,
/// when the reversed input's best delta stride beats none, [`reverse`]
/// followed by that [`delta`].
pub mod select {
    use super::delta;
    use std::num::NonZeroUsize;

    /// How much of `data` [`pick`] examines when scoring delta strides and
    /// transpose column counts. Bounds the cost of trial-selection itself:
    /// a multi-gigabyte input still only pays for an entropy scan of this
    /// many bytes.
    const PROBE_LEN: usize = 16384;

    /// Largest fixed stride [`pick`] scores for [`delta`].
    const MAX_DELTA_STRIDE: u8 = 96;

    /// How much of `data` [`pick`] scans for x86 call/jmp opcode density,
    /// counted from the full input, not [`PROBE_LEN`].
    const BCJ_SCAN_LEN: usize = 65536;

    /// [`Candidate::Bcj`] is shortlisted when opcode hits exceed one in
    /// this many scanned bytes.
    const BCJ_DENSITY_DIVISOR: usize = 400;

    /// Below this input length, [`pick`] never scores [`transpose`]: too
    /// few rows for a column count to mean anything.
    const MIN_TRANSPOSE_LEN: usize = 4096;

    /// Column counts [`pick`] scores for [`transpose`]: common fixed-width
    /// record sizes (small integer types, alignment-padded structs). The
    /// `unwrap()`s run over literals at compile time.
    const TRANSPOSE_COLUMNS: [NonZeroUsize; 14] = [
        NonZeroUsize::new(2).unwrap(),
        NonZeroUsize::new(3).unwrap(),
        NonZeroUsize::new(4).unwrap(),
        NonZeroUsize::new(7).unwrap(),
        NonZeroUsize::new(8).unwrap(),
        NonZeroUsize::new(12).unwrap(),
        NonZeroUsize::new(14).unwrap(),
        NonZeroUsize::new(16).unwrap(),
        NonZeroUsize::new(24).unwrap(),
        NonZeroUsize::new(28).unwrap(),
        NonZeroUsize::new(32).unwrap(),
        NonZeroUsize::new(56).unwrap(),
        NonZeroUsize::new(64).unwrap(),
        NonZeroUsize::new(96).unwrap(),
    ];

    /// [`Candidate::Transpose`] is shortlisted only when its column entropy
    /// beats the untransposed baseline by at least this many bits per byte
    /// — small entropy deltas are noise on a [`PROBE_LEN`]-sized sample.
    const TRANSPOSE_ENTROPY_MARGIN: f64 = 0.35;

    /// A filter worth a full trial encode, as shortlisted by [`pick`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Candidate {
        /// No filter: try `data` unmodified.
        Identity,
        /// [`delta::encode`] with this stride.
        Delta(NonZeroUsize),
        /// [`super::bcj::encode`].
        Bcj,
        /// [`super::transpose::encode`] with this many columns.
        Transpose(NonZeroUsize),
        /// [`super::base64_unwrap::encode`], which succeeded: the filtered
        /// bytes are the decoded binary, three for every four of the
        /// original.
        Base64,
        /// [`super::reverse::encode`].
        Reverse,
        /// [`super::reverse::encode`], then [`delta::encode`] with this
        /// stride over the reversed bytes: the composition a single
        /// [`Self::Delta`] or [`Self::Reverse`] cannot write.
        ReverseDelta(NonZeroUsize),
    }

    impl Candidate {
        /// Serializes this candidate into the 2-byte filter selector
        /// `[kind, param]` that opens `Method::Lz`'s payload
        /// (`docs/format/SPEC.md`). `param` is the delta stride or transpose
        /// column count, zero for [`Self::Identity`], [`Self::Bcj`],
        /// [`Self::Base64`] and [`Self::Reverse`]; for
        /// [`Self::ReverseDelta`] it is the delta stride.
        ///
        /// # Panics
        ///
        /// If a stride or column count exceeds 255. [`pick`] bounds both at
        /// 96, and this runs only on the encoder's own choices, never a
        /// decoded payload.
        #[must_use]
        pub fn to_header_bytes(self) -> [u8; 2] {
            match self {
                Self::Identity => [0, 0],
                Self::Delta(stride) => [
                    1,
                    u8::try_from(stride.get())
                        .expect("pick() bounds delta strides to MAX_DELTA_STRIDE (96), fits u8"),
                ],
                Self::Bcj => [2, 0],
                Self::Transpose(columns) => [
                    3,
                    u8::try_from(columns.get()).expect(
                        "pick() only selects TRANSPOSE_COLUMNS entries, all under 100, fits u8",
                    ),
                ],
                Self::Base64 => [4, 0],
                Self::Reverse => [5, 0],
                Self::ReverseDelta(stride) => [
                    6,
                    u8::try_from(stride.get())
                        .expect("pick() bounds delta strides to MAX_DELTA_STRIDE (96), fits u8"),
                ],
            }
        }

        /// Inverse of [`to_header_bytes`](Self::to_header_bytes). Returns
        /// `None` for any byte pair that method never produces: an
        /// unknown `kind`, a nonzero `param` on a kind that takes none,
        /// or a zero `param` on [`Self::Delta`]/[`Self::Transpose`]
        /// (both require a [`NonZeroUsize`]). This prefix comes off an
        /// untrusted payload on the decode path, so `codec::decode`
        /// turns `None` into `Error::Corrupt` rather than guessing.
        #[must_use]
        pub fn from_header_bytes(bytes: [u8; 2]) -> Option<Self> {
            match bytes {
                [0, 0] => Some(Self::Identity),
                [1, param] => NonZeroUsize::new(usize::from(param)).map(Self::Delta),
                [2, 0] => Some(Self::Bcj),
                [3, param] => NonZeroUsize::new(usize::from(param)).map(Self::Transpose),
                [4, 0] => Some(Self::Base64),
                [5, 0] => Some(Self::Reverse),
                [6, param] => NonZeroUsize::new(usize::from(param)).map(Self::ReverseDelta),
                _ => None,
            }
        }

        /// The column count of [`Self::Transpose`], the one candidate whose
        /// literals are coded by a column-keyed expert.
        pub(crate) fn columns(self) -> Option<NonZeroUsize> {
            match self {
                Self::Transpose(columns) => Some(columns),
                Self::Identity
                | Self::Delta(_)
                | Self::Bcj
                | Self::Base64
                | Self::Reverse
                | Self::ReverseDelta(_) => None,
            }
        }

        /// The length of the frame data this candidate reconstructs from
        /// `filtered_len` filtered bytes: the same length for every filter
        /// but [`Self::Base64`], which re-wraps three bytes into four
        /// (`4 * filtered_len.div_ceil(3)`, the padded text). `u64`
        /// because that can pass `u32::MAX`. A frame's header declares the
        /// filtered length, so this is what a decoder budgets the output
        /// against before it allocates a byte.
        #[must_use]
        pub fn output_len(self, filtered_len: u32) -> u64 {
            match self {
                Self::Base64 => u64::from(filtered_len).div_ceil(3) * 4,
                Self::Identity
                | Self::Delta(_)
                | Self::Bcj
                | Self::Transpose(_)
                | Self::Reverse
                | Self::ReverseDelta(_) => u64::from(filtered_len),
            }
        }
    }

    /// One `(from, to)` pair's contribution to an order-1 entropy sum:
    /// `-count * log2(count / total)`, `total` being how often `from`
    /// occurred as the first byte of a pair. Used by [`Order1::bits`].
    #[allow(
        clippy::disallowed_methods,
        reason = "encoder-only: filter-selection heuristic (ADR-0024 decision 3), no bitstream depends on it"
    )]
    fn surprisal_bits(count: u32, total: u32) -> f64 {
        let p = f64::from(count) / f64::from(total);
        -f64::from(count) * p.log2()
    }

    /// Pair-count scratch for the order-1 entropy proxies, allocated once
    /// per [`pick`] and left all-zero between calls, so scoring a stream
    /// costs its length, not the table's 256 KiB.
    struct Order1 {
        /// `pairs[(a << 8) | b]`: how often byte `b` followed byte `a`.
        pairs: Vec<u32>,
        /// `firsts[a]`: how often byte `a` was the first byte of a pair.
        firsts: [u32; 256],
    }

    impl Order1 {
        fn new() -> Self {
            Self {
                pairs: vec![0; 256 * 256],
                firsts: [0; 256],
            }
        }

        /// Order-1 entropy in bits per byte of `data` reinterpreted as
        /// `columns` interleaved streams (column `j` is `data[j],
        /// data[j + columns], ...`), each byte conditioned on its
        /// predecessor in its column, estimated from the column's own pair
        /// frequencies and weighted by each column's transition count. One
        /// column is plain order-1 entropy, the proxy [`pick`] ranks delta
        /// candidates by; several score [`Candidate::Transpose`], low when
        /// each column of a fixed-width record is internally predictable.
        /// Not a compressibility measurement, but it separates structure
        /// from noise well enough to shortlist (`JOURNAL` S1-L3).
        fn entropy(&mut self, data: &[u8], columns: NonZeroUsize) -> f64 {
            let (bits, transitions) = self.bits(data, columns.get());
            // transitions <= PROBE_LEN (16384): exact in f64.
            #[allow(clippy::cast_precision_loss)]
            let transitions = transitions.max(1) as f64;
            bits / transitions
        }

        /// Total pair surprisal in bits and transition count over the
        /// `columns` interleaved streams of `data`. Each pair is summed at
        /// its first appearance in its column, so the float sum has one
        /// order for one input; a hash map's iteration order is seeded per
        /// process and would let two runs pick different filters on a
        /// near-tie. The same walk zeroes the scratch again.
        fn bits(&mut self, data: &[u8], columns: usize) -> (f64, usize) {
            let mut bits = 0f64;
            let mut transitions = 0usize;
            for start in 0..columns.min(data.len()) {
                let column = || data[start..].iter().step_by(columns);
                for (&a, &b) in column().zip(column().skip(1)) {
                    self.pairs[(usize::from(a) << 8) | usize::from(b)] += 1;
                    self.firsts[usize::from(a)] += 1;
                    transitions += 1;
                }
                for (&a, &b) in column().zip(column().skip(1)) {
                    let slot = &mut self.pairs[(usize::from(a) << 8) | usize::from(b)];
                    if *slot > 0 {
                        bits += surprisal_bits(*slot, self.firsts[usize::from(a)]);
                        *slot = 0;
                    }
                }
                self.firsts.fill(0);
            }
            (bits, transitions)
        }
    }

    /// [`Candidate::Identity`] and every [`Candidate::Delta`] stride up to
    /// [`MAX_DELTA_STRIDE`], scored by [`Order1::entropy`] of `probe` after
    /// the filter, lowest first.
    fn rank_by_delta(probe: &[u8], order1: &mut Order1) -> Vec<(f64, Candidate)> {
        let mut scored: Vec<(f64, Candidate)> =
            Vec::with_capacity(usize::from(MAX_DELTA_STRIDE) + 1);
        scored.push((
            order1.entropy(probe, NonZeroUsize::MIN),
            Candidate::Identity,
        ));
        for stride in 1..=MAX_DELTA_STRIDE {
            let stride = NonZeroUsize::new(usize::from(stride)).unwrap_or(NonZeroUsize::MIN);
            scored.push((
                order1.entropy(&delta::encode(probe, stride), NonZeroUsize::MIN),
                Candidate::Delta(stride),
            ));
        }
        scored.sort_by(|a, b| a.0.total_cmp(&b.0));
        scored
    }

    /// Shortlists filters worth a full trial encode against `data`.
    ///
    /// [`Candidate::Identity`] is always included, as the top-scored
    /// candidate or alongside it, so every trial has a baseline.
    #[must_use]
    pub fn pick(data: &[u8]) -> Vec<Candidate> {
        let probe = &data[..data.len().min(PROBE_LEN)];

        let mut order1 = Order1::new();
        let scored = rank_by_delta(probe, &mut order1);

        let mut candidates = vec![scored[0].1];
        candidates.push(if scored[0].1 == Candidate::Identity {
            scored[1].1
        } else {
            Candidate::Identity
        });

        let bcj_window = &data[..data.len().min(BCJ_SCAN_LEN)];
        let bcj_hits = bcj_window
            .iter()
            .filter(|&&b| super::bcj::is_opcode(b))
            .count();
        if bcj_hits * BCJ_DENSITY_DIVISOR > bcj_window.len() {
            candidates.push(Candidate::Bcj);
        }

        if super::base64_unwrap::encode(data).is_some() {
            candidates.push(Candidate::Base64);
        }

        candidates.push(Candidate::Reverse);

        // The reversed input's first PROBE_LEN bytes are the original's last
        // PROBE_LEN bytes, reversed, so that window is what a trial of
        // ReverseDelta codes first. Ranked on its own: a window at the other
        // end of the block can rank another stride first.
        let tail = &data[data.len().saturating_sub(PROBE_LEN)..];
        let reversed_tail: Vec<u8> = tail.iter().rev().copied().collect();
        if let (_, Candidate::Delta(stride)) = rank_by_delta(&reversed_tail, &mut order1)[0] {
            candidates.push(Candidate::ReverseDelta(stride));
        }

        if data.len() >= MIN_TRANSPOSE_LEN {
            // rank_by_delta's Identity score is the one-column entropy of
            // `probe`, the baseline a transpose has to beat.
            let baseline = scored
                .iter()
                .find_map(|&(entropy, c)| (c == Candidate::Identity).then_some(entropy))
                .unwrap_or(f64::INFINITY);
            let best = TRANSPOSE_COLUMNS
                .iter()
                .map(|&columns| (columns, order1.entropy(probe, columns)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((columns, entropy)) = best
                && entropy < baseline - TRANSPOSE_ENTROPY_MARGIN
            {
                candidates.push(Candidate::Transpose(columns));
            }
        }

        candidates
    }

    #[cfg(test)]
    mod tests;
}
