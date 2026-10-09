//! Reversible byte-stream transforms (`JOURNAL` S1-A2, M1 port).
//!
//! Filters trade the input for a differently-shaped byte stream that a
//! downstream model can predict more cheaply — never a smaller one on their
//! own. Trial selection (`JOURNAL` S1-A1), not a static rule, decides when
//! to apply a filter: each submodule here provides the reversible
//! transform itself, [`select`] shortlists which candidates are worth a
//! full trial encode, and `crate::codec` (`JOURNAL` S2-D2, ADR-0028) trials
//! them against real [`crate::Method::Lz`] output and keeps the smallest.
//!
//! Every inverse comes in two forms. The public `decode` aborts if the
//! allocator cannot satisfy its output buffer and is what the tests drive.
//! The `pub(crate)` `try_decode`, and each streaming `Undo::try_new`,
//! return `Err` instead: `crate::codec`'s real decode paths use them,
//! because hard rule 2 forbids an abort on untrusted input (`rust-craft`
//! skill's allocation-discipline, `tests/torture.rs`, #453). A pair differs
//! only in how the buffer is built, never in the scan run over it.

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

    /// Streaming counterpart to [`decode`]: undoes the transform one
    /// filtered byte at a time instead of over a complete buffer, so a
    /// caller never needs the whole filtered stream resident to recover the
    /// original bytes (`research/JOURNAL.md` S1-P7, ROADMAP M4's
    /// bounded-memory decode guarantee).
    ///
    /// `history` holds the last `stride` bytes this instance has itself
    /// undone, not `decode`'s raw output directly — the two never diverge,
    /// because each undone byte is exactly what a future [`Self::apply`]
    /// call `stride` positions later needs, matching `decode`'s
    /// `out[i] = out[i-stride] + data[i]` reading `out`, not `data`, on its
    /// right-hand side. Bounded by `stride` (at most 255, since it comes off
    /// an 8-bit header field), never by input length.
    pub(crate) struct Undo {
        stride: usize,
        history: Vec<u8>,
        count: usize,
    }

    impl Undo {
        /// An undo state ready to accept the first filtered byte, holding
        /// `stride` bytes of history (module docs on why it is fallible).
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
        /// would produce at this same stream position. Calls must be in
        /// stream order: each call's result feeds the one `stride` calls
        /// later, the same dependency [`decode`]'s loop has on `out`.
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
/// and an absolute one. Executable code's call targets cluster (many calls
/// target the same handful of functions); as relative offsets each
/// occurrence encodes a different byte pattern, but as absolute addresses
/// they collide, which is what a downstream model actually matches
/// against. `JOURNAL` S1-A2.
pub mod bcj {
    /// Byte length of an opcode plus its rel32 operand: the unit the scan
    /// advances by on a match, and the position `encode`/`decode` measure
    /// the call/jmp target's absolute address from. `pub(crate)` rather
    /// than private: `codec`'s own test fixtures build hand-crafted
    /// instructions and need the same constant, not a second copy of `5`.
    pub(crate) const INSTRUCTION_LEN: usize = 5;

    /// Whether `byte` opens a BCJ-eligible instruction (`call rel32` /
    /// `jmp rel32`). The one source of truth [`rewrite_in_place`],
    /// [`Undo::apply`] and `select::pick`'s density heuristic all gate on,
    /// so widening the targeted opcode set is one edit, not three kept in
    /// sync by hand.
    pub(crate) const fn is_opcode(byte: u8) -> bool {
        byte == 0xE8 || byte == 0xE9
    }

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
        rewrite_in_place(&mut out, rewrite_operand);
        out
    }

    /// Rewrites the rel32 operand of the [`INSTRUCTION_LEN`]-byte
    /// `instruction` that starts at stream index `position`, through
    /// `rewrite_operand(operand, post_addr)`. The one place the operand's
    /// layout and its post-instruction address are computed, shared by the
    /// batch scan and [`Undo::apply`] so the two cannot disagree on either.
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
        // x86 rel32 addressing itself wraps at 2^32; truncating the
        // position to u32 before adding matches that hardware semantic
        // rather than losing information, so a stream past 4 GiB still
        // round-trips the same address a batch decode would compute.
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

    /// Fallible [`decode`] (module docs).
    pub(crate) fn try_decode(data: &[u8]) -> Result<Vec<u8>, std::collections::TryReserveError> {
        let mut out = crate::try_vec_from_slice(data)?;
        rewrite_in_place(&mut out, u32::wrapping_sub);
        Ok(out)
    }

    /// Bytes [`Undo::apply`]/[`Undo::finish`] resolved from stream data so
    /// far, in order. Fixed [`INSTRUCTION_LEN`] capacity avoids a heap
    /// allocation per filtered byte in the streaming decode's hot loop
    /// (`rust-craft` skill, mechanical sympathy).
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

        /// `bytes.len()` must be at most [`INSTRUCTION_LEN`]: the only
        /// caller, [`Undo`], never accumulates more than that many pending
        /// bytes before resolving or flushing them.
        fn from_slice(bytes: &[u8]) -> Self {
            let mut buf = [0; INSTRUCTION_LEN];
            buf[..bytes.len()].copy_from_slice(bytes);
            // Undo's pending buffer never exceeds INSTRUCTION_LEN (5).
            #[allow(clippy::cast_possible_truncation)]
            let len = bytes.len() as u8;
            Self { buf, len }
        }

        pub(crate) fn as_slice(&self) -> &[u8] {
            &self.buf[..self.len as usize]
        }
    }

    /// Streaming counterpart to [`decode`]: undoes the transform as filtered
    /// bytes arrive instead of over a complete buffer (`research/JOURNAL.md`
    /// S1-P7, ROADMAP M4's bounded-memory decode guarantee), mirroring
    /// [`delta::Undo`]'s role for this filter.
    ///
    /// Unlike delta's fixed lookback, bcj's [`rewrite`] scan needs
    /// *lookahead*: whether the byte at position `i` starts an instruction
    /// is decidable the instant it arrives (opcode byte or not), but
    /// transforming that instruction's operand needs the
    /// `INSTRUCTION_LEN - 1` bytes that follow it, not yet available when
    /// the opcode byte itself reaches [`Self::apply`]. `pending` holds those
    /// not-yet-resolved bytes; `position` is the same absolute stream index
    /// [`rewrite`]'s own `i` tracks, advancing only once a byte is resolved
    /// one way or the other, never while `pending` is still filling.
    pub(crate) struct Undo {
        pending: Vec<u8>,
        position: usize,
    }

    impl Undo {
        /// A fresh undo state, ready to accept the first filtered byte, with
        /// [`INSTRUCTION_LEN`] bytes of pending-buffer capacity (module docs
        /// on why it is fallible).
        pub(crate) fn try_new() -> Result<Self, std::collections::TryReserveError> {
            let mut pending = Vec::new();
            pending.try_reserve_exact(INSTRUCTION_LEN)?;
            Ok(Self {
                pending,
                position: 0,
            })
        }

        /// Feeds one more filtered byte, in stream order, returning any
        /// bytes this call resolved: empty while still buffering a
        /// candidate instruction's operand, one immediately for a
        /// non-opcode byte (no lookahead needed to know it is not
        /// `0xE8`/`0xE9`), or all [`INSTRUCTION_LEN`] the instant a full
        /// instruction's operand has arrived.
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

        /// Flushes any bytes still buffered at end of stream: a candidate
        /// instruction seen too close to the end to resolve (fewer than
        /// [`INSTRUCTION_LEN`] bytes followed its opcode byte), passed
        /// through unchanged — the same "too short for any instruction"
        /// case [`rewrite`]'s own scan bound (`i + INSTRUCTION_LEN <= n`)
        /// leaves untouched.
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

    /// [`ALPHABET`]'s inverse, indexed by byte value: `DECODE_TABLE[b]` is
    /// `Some(v)` iff `ALPHABET[v] == b`. Built from `ALPHABET` itself so the
    /// two directions cannot drift apart.
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
        let unwrapped = looks_like_base64
            .then(|| try_decode(data))
            .flatten()
            .filter(|decoded| b64_encode(decoded) == data);
        let (flag, body) = match &unwrapped {
            Some(decoded) => (1, decoded.as_slice()),
            None => (0, data),
        };
        let mut out = Vec::with_capacity(1 + body.len());
        out.push(flag);
        out.extend_from_slice(body);
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
    mod tests;
}

/// Byte-order reversal.
///
/// Wins when structure is right-anchored (a fixed suffix, a length-prefixed
/// tail, records better predicted from their end than their start):
/// reversing turns that right anchor into a left one, where the downstream
/// LZ/model's recency bias and forward context actually reach it (`JOURNAL`
/// S1-A2). Its own inverse: reversing twice reproduces the input, so
/// [`decode`](reverse::decode) is [`encode`](reverse::encode) under a
/// different name, kept as two functions to match this module's
/// established encode/decode-pair shape.
pub mod reverse {
    /// Reverses `data`.
    ///
    /// Self-inverse: applying this function to its own output reproduces
    /// the original `data`, so [`decode`] is this same operation.
    #[must_use]
    pub fn encode(data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        out.reverse();
        out
    }

    /// Inverts [`encode`]. Identical to [`encode`] because byte-order
    /// reversal is its own inverse.
    #[must_use]
    pub fn decode(data: &[u8]) -> Vec<u8> {
        encode(data)
    }

    #[cfg(test)]
    mod tests;
}

/// Trial-selection shortlist: which filters are worth a full trial encode.
///
/// `JOURNAL` S1-A1: the filter bank is never applied by a static rule, only
/// kept when a trial encode measurably wins. Trialing every filter this
/// crate knows against every input would be correct but wasteful;
/// [`select::pick`] narrows the menu to a cheap shortlist using an order-1
/// entropy proxy on a bounded probe, so the expensive trial encode in the
/// caller only runs on candidates worth the cost. Ported from the archive's
/// `pick_filters`
/// (`research/imports/session-1/mothergod.rs`), not the code (ADR-0006):
/// only [`delta`] and [`bcj`] and [`transpose`] are shortlisted here,
/// because those are the only filters `pick_filters` covers in that file —
/// [`base64_unwrap`] and [`reverse`] are selected by a different path in the
/// archive (`JOURNAL` S2-A5, S2-A6) that this slice does not port.
pub mod select {
    use super::delta;
    use std::collections::HashMap;
    use std::num::NonZeroUsize;

    /// How much of `data` [`pick`] examines when scoring delta strides and
    /// transpose column counts. Bounds the cost of trial-selection itself:
    /// a multi-gigabyte input still only pays for an entropy scan of this
    /// many bytes.
    const PROBE_LEN: usize = 16384;

    /// Largest fixed stride [`pick`] scores for [`delta`]. Matches the
    /// archive's scan range (`sdelta` tried for `k` in `1..=96`).
    const MAX_DELTA_STRIDE: u8 = 96;

    /// How much of `data` [`pick`] scans for x86 call/jmp opcode density.
    /// Unlike the delta/transpose probes this is measured from the full
    /// input, not [`PROBE_LEN`], matching the archive.
    const BCJ_SCAN_LEN: usize = 65536;

    /// [`Candidate::Bcj`] is shortlisted when opcode hits exceed one in
    /// this many scanned bytes.
    const BCJ_DENSITY_DIVISOR: usize = 400;

    /// Below this input length, [`pick`] never scores [`transpose`]: too
    /// few rows for a column count to mean anything.
    const MIN_TRANSPOSE_LEN: usize = 4096;

    /// Column counts [`pick`] scores for [`transpose`]. Common fixed-width
    /// record sizes (small integer types, alignment-padded structs), not
    /// an exhaustive scan. `NonZeroUsize` so [`pick`] never needs a
    /// fallible conversion back from a plain `usize` at call time; the
    /// `unwrap()`s below run over literals at compile time, never at
    /// runtime.
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
    }

    impl Candidate {
        /// Serializes this candidate into the 2-byte filter-selector
        /// prefix `Method::Lz`'s payload carries ahead of its declared-
        /// length header (`docs/format/SPEC.md`): `[kind, param]`.
        /// `param` is the delta stride or transpose column count, zero
        /// for the two filters that take none ([`Self::Identity`],
        /// [`Self::Bcj`]). [`pick`] never returns a stride past
        /// `MAX_DELTA_STRIDE` (96) or a column count past this
        /// module's widest `TRANSPOSE_COLUMNS` entry (96), both well
        /// under 256, so `param` never truncates a value this module
        /// actually produces.
        ///
        /// # Panics
        ///
        /// Does not panic on any `Candidate` this module's [`pick`] can
        /// build: the two internal `.expect()`s guard exactly the
        /// stride/column bound argued above, never adversarial input —
        /// this side of the format only ever runs on the encoder's own
        /// choices, never a decoded payload.
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
                _ => None,
            }
        }
    }

    /// One `(from, to)` pair's contribution to an order-1 entropy sum:
    /// `-count * log2(count / total)`, `total` being how often `from`
    /// occurred as the first byte of a pair. Shared by [`order1_entropy`]
    /// and [`column_entropy`], which both score a byte stream's
    /// predictability by this same formula and differ only in how they
    /// count `(from, to)` pairs — a plain array over the full probe in one,
    /// a `HashMap` over one interleaved column in the other, sized that way
    /// because a column is far smaller than the 256x256 pairs the array
    /// bounds — never in what a pair's count is worth once counted.
    #[allow(
        clippy::disallowed_methods,
        reason = "encoder-only: filter-selection heuristic (ADR-0024 decision 3), no bitstream depends on it"
    )]
    fn surprisal_bits(count: u32, total: u32) -> f64 {
        let p = f64::from(count) / f64::from(total);
        -f64::from(count) * p.log2()
    }

    /// Order-1 entropy in bits per byte: `data`, conditioned on each byte's
    /// immediate predecessor, estimated from `data`'s own pair frequencies.
    /// The proxy [`pick`] ranks delta candidates by — never a
    /// compressibility measurement itself, only a cheap stand-in for one
    /// (`JOURNAL` S1-L3: histogram entropy is not compressibility, but a
    /// *conditional* entropy proxy still separates structured candidates
    /// from noise well enough to shortlist).
    fn order1_entropy(data: &[u8]) -> f64 {
        let mut pair_counts = vec![0u32; 256 * 256];
        let mut byte_counts = [0u32; 256];
        for window in data.windows(2) {
            let (a, b) = (usize::from(window[0]), usize::from(window[1]));
            pair_counts[(a << 8) | b] += 1;
            byte_counts[a] += 1;
        }
        let mut bits = 0f64;
        for a in 0..256 {
            let total = byte_counts[a];
            if total == 0 {
                continue;
            }
            for b in 0..256 {
                let count = pair_counts[(a << 8) | b];
                if count > 0 {
                    bits += surprisal_bits(count, total);
                }
            }
        }
        // data.len() is bounded by PROBE_LEN (16384): exact in f64.
        #[allow(clippy::cast_precision_loss)]
        let transitions = (data.len().max(2) - 1) as f64;
        bits / transitions
    }

    /// Order-1 entropy of `data` reinterpreted as `columns` interleaved
    /// streams (column `j` is `data[j], data[j + columns], ...`), weighted
    /// by each column's own transition count. The scoring proxy for
    /// [`Candidate::Transpose`]: low when a fixed-width record's columns
    /// are each internally predictable, even though the raw byte stream
    /// (whose immediate predecessor is usually a *different* column) looks
    /// unpredictable.
    fn column_entropy(data: &[u8], columns: usize) -> f64 {
        let mut bits = 0f64;
        let mut transitions = 0usize;
        for start in 0..columns {
            let column: Vec<u8> = data[start..].iter().copied().step_by(columns).collect();
            if column.len() < 2 {
                continue;
            }
            let mut pair_counts: HashMap<(u8, u8), u32> = HashMap::new();
            let mut byte_counts: HashMap<u8, u32> = HashMap::new();
            for window in column.windows(2) {
                *pair_counts.entry((window[0], window[1])).or_insert(0) += 1;
                *byte_counts.entry(window[0]).or_insert(0) += 1;
            }
            for (&(from, _), &count) in &pair_counts {
                let total = byte_counts[&from];
                bits += surprisal_bits(count, total);
            }
            transitions += column.len() - 1;
        }
        // transitions <= PROBE_LEN (16384): exact in f64.
        #[allow(clippy::cast_precision_loss)]
        let transitions = transitions.max(1) as f64;
        bits / transitions
    }

    /// Shortlists filters worth a full trial encode against `data`.
    ///
    /// [`Candidate::Identity`] is always included, either as the top-scored
    /// candidate or alongside it, so a caller trialing every returned
    /// candidate never trials filters alone without a baseline to beat.
    #[must_use]
    pub fn pick(data: &[u8]) -> Vec<Candidate> {
        let probe = &data[..data.len().min(PROBE_LEN)];

        let mut scored: Vec<(f64, Candidate)> =
            Vec::with_capacity(usize::from(MAX_DELTA_STRIDE) + 1);
        scored.push((order1_entropy(probe), Candidate::Identity));
        for stride in 1..=MAX_DELTA_STRIDE {
            let stride = NonZeroUsize::new(usize::from(stride)).unwrap_or(NonZeroUsize::MIN);
            scored.push((
                order1_entropy(&delta::encode(probe, stride)),
                Candidate::Delta(stride),
            ));
        }
        scored.sort_by(|a, b| a.0.total_cmp(&b.0));

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

        if data.len() >= MIN_TRANSPOSE_LEN {
            let baseline = column_entropy(probe, 1);
            let best = TRANSPOSE_COLUMNS
                .iter()
                .map(|&columns| (columns, column_entropy(probe, columns.get())))
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
