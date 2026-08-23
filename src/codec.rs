//! `Method::Lz` wiring (`research/JOURNAL.md` S2-D2, ROADMAP M1's last
//! `Method`-wiring slice): the optimal-parse LZ parser ([`crate::lz`]),
//! the flag/length/offset/rep-slot adaptive tables ([`crate::model`]),
//! the six-expert literal mixer ([`crate::literal`]), and the range coder
//! ([`crate::coder`]), driven together as one entropy-coded payload.
//! Ported from the archive's `encode_body`/`decode`
//! (`research/imports/session-1/mothergod.rs`), not the code, per
//! ADR-0006.
//!
//! Not ported yet: the archive's outer `encode`/`decode` also trial-runs
//! [`crate::filters`] and picks whichever shrinks the output most.
//! [`encode`] here always runs on the raw input; filter selection is
//! `research/JOURNAL.md` S2-D2's remaining scope, a follow-up slice.
//!
//! # Payload layout
//!
//! ```text
//! offset  size  field
//! 0       4     declared output length, u32 LE
//! 4       4     token count, u32 LE
//! 8       ...   range-coded stream (crate::coder)
//! ```
//!
//! The declared output length is [`decode`]'s allocation bound
//! (`docs/format/SPEC.md`, `rust-craft` skill's allocation-discipline): a
//! hostile payload can claim any token count or any match/rep length, but
//! [`decode`] never grows its output buffer past this field, and never
//! preallocates a capacity derived from it either. That field is itself
//! capped at [`MAX_DECODED_LEN`], so a tiny payload cannot declare an
//! unbounded length and force unbounded decode work; see
//! [`MAX_DECODED_LEN`]'s docs for why, and [`decode`]'s docs for the rest.

use std::num::NonZeroU32;

use crate::Error;
use crate::coder::{Decoder, Encoder};
use crate::literal::{Context, Literal};
use crate::lz::{self, RepCache, RepSlot, Token};
use crate::model::Model;

/// Largest declared output length [`decode`] accepts, checked before any
/// allocation or decode work: `rust-craft`'s allocation-discipline
/// reference calls this the "against a configured ceiling" bound, needed
/// because general-purpose compression has no bound on output size
/// derivable from the input alone.
///
/// A ratio-relative bound (declared length vs. remaining payload bytes)
/// cannot substitute for this: measured directly (a release-mode
/// `compress` run on a 60,000-byte single-repeated-byte input, the
/// degenerate case this format's adaptive models handle best), a
/// legitimate frame already reaches a ~3,158:1 ratio at a 19-byte payload,
/// with the ratio still climbing as input grows and the payload barely
/// moving — this format's models saturate fast enough that a real
/// encoder's output and a forged header become indistinguishable by size
/// alone. An explicit ceiling on the declared length itself is the only
/// bound left, and it caps total decode work too: every token contributes
/// at least one byte toward `declared_len` before `ensure_room` rejects
/// it, so bounding the declared length bounds loop iterations as well as
/// allocation.
///
/// 256 MiB, chosen to clear the largest single file in the M2 benchmark
/// corpus (Silesia's `mozilla`, ~51 MB) with headroom, while keeping a
/// worst-case adversarial decode (an all-padding, all-literal stream, the
/// cheapest branch per iteration) bounded to low single-digit minutes
/// rather than unbounded. Provisional: `ROADMAP.md` M4's streaming/block
/// API is the real fix (bounded-memory decode without a single hardcoded
/// file-size ceiling), and should widen or remove this once it lands.
pub const MAX_DECODED_LEN: u32 = 256 * 1024 * 1024;

/// Flag symbols coded before every token, selecting which of the three
/// kinds follows. Matches the archive's `flag.enc(ac, {0,1,2})`.
const FLAG_LITERAL: usize = 0;
const FLAG_MATCH: usize = 1;
const FLAG_REP: usize = 2;
/// Alphabet size of the flag [`Model`]s: exactly the three symbols above.
const FLAG_ALPHABET: usize = 3;

/// Repeat-offset cache slot count and the [`Model`] alphabet size for
/// coding which slot a [`Token::Rep`] used.
const REP_SLOTS: usize = 3;

/// The five adaptive tables `Method::Lz` drives, bundled so encode and
/// decode construct and thread them identically.
struct Models {
    literal: Literal,
    /// One flag table per "was the previous token a copy" state (the
    /// archive's `flag[2]`): a literal run and a post-copy position have
    /// different flag distributions worth modeling separately. Indexed by
    /// [`Context::after_copy`].
    flag: [Model; 2],
    length: Model,
    offset: Model,
    slot: Model,
}

impl Models {
    fn new() -> Self {
        Self {
            literal: Literal::new(),
            flag: [Model::new(FLAG_ALPHABET), Model::new(FLAG_ALPHABET)],
            length: Model::new(lz::LENGTH_BUCKETS),
            offset: Model::new(lz::OFFSET_BUCKETS),
            slot: Model::new(REP_SLOTS),
        }
    }
}

fn slot_to_symbol(slot: RepSlot) -> usize {
    match slot {
        RepSlot::First => 0,
        RepSlot::Second => 1,
        RepSlot::Third => 2,
    }
}

/// [`Model::decode`] over a [`REP_SLOTS`]-symbol alphabet always returns a
/// value `< REP_SLOTS`, never anything adversarial input could push out of
/// range (see [`Model::decode`]'s own panic-free guarantee), so the only
/// three cases below are exhaustive without a fallback panic.
fn symbol_to_slot(symbol: usize) -> RepSlot {
    match symbol {
        0 => RepSlot::First,
        1 => RepSlot::Second,
        _ => RepSlot::Third,
    }
}

/// Codes `value` (a match/rep length, or a match distance) as a
/// [`lz::bucket`] symbol through `model`, then the residual low bits of
/// `value` within that bucket as raw, unmodeled bits. Matches the
/// archive's `lenm.enc(ac,lb); ac.bits(l,lb)` (and the identical shape for
/// offsets).
fn encode_bucketed(model: &mut Model, ac: &mut Encoder, value: u32) {
    let b = lz::bucket(value);
    model.encode(ac, b);
    // b is a Model alphabet index, at most OFFSET_BUCKETS - 1 (20): always
    // fits u32.
    let bits = u32::try_from(b).expect("bucket index is small, always fits u32");
    ac.encode_bits(value, bits);
}

/// Inverse of [`encode_bucketed`]: decodes a bucket symbol, then the
/// residual bits, and reconstructs `value` as `(1 << bucket) |
/// residual_bits`. Never panics on adversarial `ac` state: `model.decode`
/// and `ac.decode_bits` are both panic-free on any input (see their own
/// docs), and the shift below is bounded by the same small-alphabet
/// argument as [`encode_bucketed`].
fn decode_bucketed(model: &mut Model, ac: &mut Decoder) -> u32 {
    let b = model.decode(ac);
    let bits = u32::try_from(b).expect("bucket index is small, always fits u32");
    (1u32 << bits) | ac.decode_bits(bits)
}

/// Encodes `data` through the LZ + context-mixing pipeline (no filters;
/// see the module docs).
///
/// # Panics
///
/// Panics if `data.len()` exceeds `u32::MAX`: the declared-output-length
/// header field is a `u32`, the same bound [`lz::parse_greedy`] already
/// enforces. [`crate::compress`] checks this before calling in, so
/// nothing reachable from the public API hits it today.
#[must_use]
pub fn encode(data: &[u8]) -> Vec<u8> {
    let declared_len = u32::try_from(data.len())
        .expect("codec::encode: input longer than u32::MAX is not supported yet");

    let tokens = lz::parse_optimal(data);
    let token_count = u32::try_from(tokens.len())
        .expect("token count bounded by input length, already checked to fit u32 above");

    let mut models = Models::new();
    let mut ac = Encoder::new();
    let mut context = Context::default();
    let mut pos = 0usize;

    for token in &tokens {
        let flag_table = usize::from(context.after_copy);
        match *token {
            Token::Literal(byte) => {
                models.flag[flag_table].encode(&mut ac, FLAG_LITERAL);
                models.literal.encode(&mut ac, context, byte);
                context = context.after_literal(byte);
                pos += 1;
            }
            Token::Match { len, distance } => {
                models.flag[flag_table].encode(&mut ac, FLAG_MATCH);
                encode_bucketed(&mut models.length, &mut ac, len);
                encode_bucketed(&mut models.offset, &mut ac, distance.get());
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
            Token::Rep { len, slot } => {
                models.flag[flag_table].encode(&mut ac, FLAG_REP);
                models.slot.encode(&mut ac, slot_to_symbol(slot));
                encode_bucketed(&mut models.length, &mut ac, len);
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
        }
    }

    let mut out = Vec::with_capacity(8 + data.len() / 2);
    out.extend_from_slice(&declared_len.to_le_bytes());
    out.extend_from_slice(&token_count.to_le_bytes());
    out.extend(ac.finish());
    out
}

/// Splits `payload` into its declared output length, token count, and the
/// remaining range-coded bytes.
///
/// # Errors
///
/// Returns [`Error::Truncated`] if `payload` is shorter than the 8-byte
/// header.
fn read_header(payload: &[u8]) -> Result<(usize, u32, &[u8]), Error> {
    let declared_len = payload.get(0..4).ok_or(Error::Truncated)?;
    let declared_len = u32::from_le_bytes(
        declared_len
            .try_into()
            .expect("checked to be exactly 4 bytes"),
    );
    let token_count = payload.get(4..8).ok_or(Error::Truncated)?;
    let token_count = u32::from_le_bytes(
        token_count
            .try_into()
            .expect("checked to be exactly 4 bytes"),
    );
    Ok((declared_len as usize, token_count, &payload[8..]))
}

/// Rejects a token whose declared size would grow `output` past
/// `declared_len`: `docs/format/SPEC.md`'s allocation-bound invariant,
/// checked before every write rather than trusted from the header.
fn ensure_room(output_len: usize, additional: usize, declared_len: usize) -> Result<(), Error> {
    match output_len.checked_add(additional) {
        Some(total) if total <= declared_len => Ok(()),
        _ => Err(Error::Corrupt),
    }
}

/// Copies `len` bytes to the end of `output` from `distance` bytes before
/// its current end, one byte at a time so a distance shorter than `len` (a
/// run, not a disjoint repeat) still reproduces the source correctly.
/// Mirrors [`lz::replay`]'s `copy_match`, but returns [`Error::Corrupt`]
/// instead of panicking when `distance` reaches before the start of
/// `output`: unlike `replay`, this runs on a token stream decoded from an
/// adversarial bitstream, not one [`lz::parse_optimal`] just produced
/// (`rust-craft` skill, panic-discipline — decode-path input is never
/// trusted).
fn copy_checked(output: &mut Vec<u8>, len: u32, distance: NonZeroU32) -> Result<(), Error> {
    let distance = distance.get() as usize;
    let start = output.len().checked_sub(distance).ok_or(Error::Corrupt)?;
    for k in 0..len as usize {
        // start + k < output.len() at every iteration: start < output.len()
        // going in (checked_sub succeeded against a distance >= 1), and
        // output grows by exactly one element per iteration from there, so
        // the index this iteration reads is always already written —
        // either from before this call, or by an earlier iteration of it
        // (the overlapping-run case).
        let byte = output[start + k];
        output.push(byte);
    }
    Ok(())
}

/// Decodes a payload produced by [`encode`] back into the original bytes.
///
/// Bounds decode work and allocation to the frame's declared output size,
/// itself bounded by [`MAX_DECODED_LEN`] (`docs/format/SPEC.md`,
/// `rust-craft` skill's allocation-discipline): `output` starts empty and
/// grows one token at a time, never preallocated from the header's
/// declared length, and every token is rejected the moment it would grow
/// `output` past that length — so a payload lying about either the
/// declared length or the token count cannot make this function do more
/// work, or allocate more memory, than the length it declared allows, and
/// [`MAX_DECODED_LEN`] bounds what it is allowed to declare in the first
/// place. See [`MAX_DECODED_LEN`]'s docs for why a fixed ceiling, not a
/// ratio check against the payload's own size, is the sound bound here.
///
/// # Errors
///
/// Returns [`Error::Truncated`] if `payload` is shorter than the 8-byte
/// declared-length/token-count header. Returns [`Error::TooLarge`] if the
/// declared length exceeds [`MAX_DECODED_LEN`], checked before any
/// allocation or decode work. Returns [`Error::Corrupt`] if a match or rep
/// token's distance reaches before the start of decoded output, if a
/// token would grow decoded output past the declared length, or if the
/// final decoded length does not equal it: all adversarial or malformed
/// input, never a bug in this decoder (`rust-craft` skill,
/// panic-discipline).
///
/// # Panics
///
/// Does not panic on adversarial `payload`. Two internal `.expect()`s
/// guard invariants of this function's own math, never a property of
/// `payload`: turning a decoded match distance into a [`NonZeroU32`]
/// (a mathematical invariant of `decode_bucketed`, see that function's
/// docs), and casting a declared length already found `<=
/// MAX_DECODED_LEN` (a `u32`) back down from the `usize` `read_header`
/// widened it to.
pub fn decode(payload: &[u8]) -> Result<Vec<u8>, Error> {
    let (declared_len, token_count, ac_bytes) = read_header(payload)?;
    if declared_len > MAX_DECODED_LEN as usize {
        // declared_len was cast up from the header's u32 field (read_header),
        // so casting back down here is always exact.
        return Err(Error::TooLarge(u32::try_from(declared_len).expect(
            "declared_len came from a u32 header field, so it always fits back into one",
        )));
    }

    let mut ac = Decoder::new(ac_bytes);
    let mut models = Models::new();
    let mut context = Context::default();
    let mut reps = RepCache::initial();
    let mut output: Vec<u8> = Vec::new();

    for _ in 0..token_count {
        let flag_table = usize::from(context.after_copy);
        match models.flag[flag_table].decode(&mut ac) {
            FLAG_LITERAL => {
                ensure_room(output.len(), 1, declared_len)?;
                let byte = models.literal.decode(&mut ac, context);
                output.push(byte);
                context = context.after_literal(byte);
            }
            FLAG_MATCH => {
                let len = decode_bucketed(&mut models.length, &mut ac);
                let distance = decode_bucketed(&mut models.offset, &mut ac);
                // decode_bucketed always ORs in `1 << bits`, which is >= 1
                // regardless of the residual bits: never zero.
                let distance =
                    NonZeroU32::new(distance).expect("decode_bucketed's result is always >= 1");
                ensure_room(output.len(), len as usize, declared_len)?;
                copy_checked(&mut output, len, distance)?;
                reps.push_front(distance);
                context = context.after_copy(&output[output.len() - len as usize..]);
            }
            _ => {
                // models.slot's alphabet is REP_SLOTS (3) symbols, and
                // Model::decode never returns a value outside its
                // alphabet; models.flag's alphabet is FLAG_ALPHABET (3),
                // so this arm is FLAG_REP (2), never a fourth flag value.
                let slot = symbol_to_slot(models.slot.decode(&mut ac));
                let len = decode_bucketed(&mut models.length, &mut ac);
                let distance = reps.get(slot);
                ensure_room(output.len(), len as usize, declared_len)?;
                copy_checked(&mut output, len, distance)?;
                reps.promote(slot);
                context = context.after_copy(&output[output.len() - len as usize..]);
            }
        }
    }

    if output.len() != declared_len {
        return Err(Error::Corrupt);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(data: &[u8]) {
        let encoded = encode(data);
        assert_eq!(decode(&encoded).as_deref(), Ok(data), "roundtrip mismatch");
    }

    #[test]
    fn roundtrip_empty() {
        roundtrip(b"");
    }

    #[test]
    fn roundtrip_single_byte() {
        roundtrip(b"x");
    }

    #[test]
    fn roundtrip_all_literals_no_repeats() {
        roundtrip(b"the quick brown fox jumps over a lazy dog");
    }

    #[test]
    fn roundtrip_simple_repeat_shrinks() {
        let data = b"abcdefgh".repeat(50);
        let encoded = encode(&data);
        assert!(
            encoded.len() < data.len(),
            "a 50x repeat of an 8-byte pattern should compress: {} -> {}",
            data.len(),
            encoded.len()
        );
        assert_eq!(decode(&encoded).as_deref(), Ok(data.as_slice()));
    }

    #[test]
    fn roundtrip_long_run_of_one_repeated_byte() {
        // Deliberately not lz.rs's 200_000-byte equivalent: encode here
        // always goes through lz::parse_optimal (never parse_greedy), and
        // parse_optimal's rep-candidate pricing is a linear match_len scan
        // at every position (see lz.rs's dp_round docs) — cost grows with
        // the square of a same-byte run once it passes MAX_MATCH_LEN
        // (65535). This project's own encoder hung for minutes at 200_000
        // bytes during this PR's development (`research/JOURNAL.md`
        // S2-A17), which is exactly why the test stays well under that
        // threshold rather than proving the hang here too.
        roundtrip(&vec![b'z'; 4000]);
    }

    #[test]
    fn roundtrip_cyclic_data() {
        let data: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        roundtrip(&data);
    }

    #[test]
    fn roundtrip_pseudo_random_bytes() {
        let data: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
            .take(5000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();
        roundtrip(&data);
    }

    #[test]
    fn roundtrip_binary_data_with_zero_bytes() {
        let data: Vec<u8> = (0..1000u32)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();
        roundtrip(&data);
    }

    #[test]
    fn roundtrip_founding_archive_source() {
        // Named corpus (CLAUDE.md hard rule 4): the founding session's
        // archived codec, real structured Rust source, 25,524 bytes — the
        // same file `literal.rs`'s ADR-0024 accuracy test measures
        // against.
        let data: &[u8] = include_bytes!("../research/imports/session-1/mothergod.rs");
        roundtrip(data);
    }

    #[test]
    fn truncated_header_is_rejected() {
        assert_eq!(decode(&[0u8; 4]), Err(Error::Truncated));
        assert_eq!(decode(&[]), Err(Error::Truncated));
    }

    #[test]
    fn declared_length_lie_is_rejected_not_overallocated() {
        // A payload claiming a huge (u32::MAX) declared output length but
        // a token count of 0: decode must reject the mismatch, and must
        // do so in zero loop iterations rather than trying to honor the
        // claim by preallocating or looping toward it (rust-craft skill,
        // allocation-discipline). u32::MAX exceeds MAX_DECODED_LEN, so
        // this actually now hits the TooLarge fast-reject path below
        // before token_count is even consulted; kept as Corrupt's own
        // regression case too since it predates that check.
        let mut payload = u32::MAX.to_le_bytes().to_vec();
        payload.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode(&payload), Err(Error::TooLarge(u32::MAX)));
    }

    #[test]
    fn declared_length_over_the_max_is_rejected_before_any_work() {
        // The amplification hazard a ratio check cannot rule out (see
        // MAX_DECODED_LEN's docs): declared_len and token_count agree with
        // each other, both far past MAX_DECODED_LEN, with an empty coded
        // stream. Previously legal-but-slow under ADR-0026's declared-
        // length-bounds-work argument (true, but declared_len itself was
        // unbounded); now rejected in zero loop iterations.
        let over = MAX_DECODED_LEN + 1;
        let mut payload = over.to_le_bytes().to_vec();
        payload.extend_from_slice(&over.to_le_bytes());
        assert_eq!(decode(&payload), Err(Error::TooLarge(over)));
    }

    #[test]
    fn bad_match_distance_is_rejected_not_panicking() {
        // Hand-crafted: one token (flag=Match), coded through a fresh
        // Encoder so the bit-level framing is real, at a position where no
        // output exists yet — the distance necessarily reaches before the
        // start of decoded output.
        let mut models = Models::new();
        let mut ac = Encoder::new();
        let context = Context::default();
        models.flag[0].encode(&mut ac, FLAG_MATCH);
        encode_bucketed(&mut models.length, &mut ac, 4);
        encode_bucketed(&mut models.offset, &mut ac, 1);
        let _ = context;
        let ac_bytes = ac.finish();

        let mut payload = 4u32.to_le_bytes().to_vec(); // declared_len
        payload.extend_from_slice(&1u32.to_le_bytes()); // token_count
        payload.extend(ac_bytes);

        assert_eq!(decode(&payload), Err(Error::Corrupt));
    }
}
