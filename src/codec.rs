//! `Method::Lz` wiring (`research/JOURNAL.md` S2-D2, ROADMAP M1's last
//! `Method`-wiring slice): [`crate::filters::select::pick`] shortlists
//! candidate filters, each is trial-encoded through the optimal-parse LZ
//! parser ([`crate::lz`]), the flag/length/offset/rep-slot adaptive
//! tables ([`crate::model`]), the six-expert literal mixer
//! ([`crate::literal`]), and the range coder ([`crate::coder`]), and
//! whichever candidate produces the smallest payload wins. Ported from
//! the archive's `encode`/`encode_body`/`decode`
//! (`research/imports/session-1/mothergod.rs`), not the code, per
//! ADR-0006.
//!
//! # Payload layout
//!
//! ```text
//! offset  size  field
//! 0       2     filter selector: [kind, param] (`filters::select::Candidate`)
//! 2       4     declared output length, u32 LE
//! 6       4     token count, u32 LE
//! 10      ...   range-coded stream (crate::coder), of the FILTERED bytes
//! ```
//!
//! `docs/adr/0028-wire-filter-selection.md` added the 2-byte filter
//! selector ahead of the layout ADR-0026 shipped; decoding a frame that
//! named `FORMAT_VERSION` 1 under this layout would misread those two
//! bytes as part of the declared length, so [`crate::decompress`] rejects
//! any `Method::Lz` frame naming a version below `LZ_MIN_VERSION`
//! before calling [`decode`] at all, rather than relying on this parser's
//! own adversarial-input defenses to fail safely by coincidence.
//!
//! The outer layout above is unchanged since `FORMAT_VERSION` 2, but the
//! range-coded stream's literal sub-stream is not: a version-3 frame codes
//! each literal byte as 8 SSE-calibrated binary decisions
//! ([`crate::literal::Literal::encode_sse`]/`decode_sse`,
//! `docs/adr/0038-wire-sse-into-the-literal-mixer.md`, `research/JOURNAL.md`
//! S1-P1) where a version-2 frame codes it as one direct 256-way range
//! division ([`crate::literal::Literal::encode`]/`decode`). [`decode`]
//! takes the frame's declared `version` and picks the matching literal
//! path; every other symbol (flag/length/offset/slot) is unaffected and
//! coded identically at every version `LZ_MIN_VERSION` or above.
//!
//! The declared output length is [`decode`]'s allocation bound
//! (`docs/format/SPEC.md`, `rust-craft` skill's allocation-discipline): a
//! hostile payload can claim any token count or any match/rep length, but
//! [`decode`] never grows its output buffer past this field, and never
//! preallocates a capacity derived from it either. That field is itself
//! capped at [`MAX_DECODED_LEN`], so a tiny payload cannot declare an
//! unbounded length and force unbounded decode work; see
//! [`MAX_DECODED_LEN`]'s docs for why, and [`decode`]'s docs for the rest.
//!
//! [`ideal_cost_bits`] completes ROADMAP M2's ideal-cost accounting mode
//! (`research/JOURNAL.md` S2-A30/S2-A31 built the per-model pieces this
//! sums): the whole-codec `-log2(p)` pass across the flag/length/offset/slot
//! streams and literal bytes together, without touching an [`Encoder`].

use std::num::NonZeroU32;

use crate::Error;
use crate::coder::{Decoder, Encoder};
use crate::filters::{self, select::Candidate};
use crate::literal::{Context, Literal};
use crate::lz::{self, RepCache, RepSlot, Token};
use crate::model::Model;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload this build can
/// decode: see the module docs' "Payload layout" section for the layout
/// change that moved this from 1 to 2.
pub(crate) const LZ_MIN_VERSION: u8 = 2;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload codes its literal
/// sub-stream through [`crate::literal::Literal::encode_sse`]/`decode_sse`
/// rather than the older direct 256-way [`crate::literal::Literal::encode`]/
/// `decode`: see the module docs' "Payload layout" section and [`decode`]'s
/// own docs for the version dispatch this feeds.
const LITERAL_SSE_MIN_VERSION: u8 = 3;

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
/// worst-case adversarial decode bounded rather than unbounded. That
/// worst case is an all-literal stream (`research/JOURNAL.md` S2-A27):
/// every literal byte pays [`crate::literal::Literal::decode_sse`]'s full
/// six-expert mix plus the 8-chained-binary-decision SSE-calibrated coding
/// path (`FORMAT_VERSION` 3, ADR-0038) over the 256-symbol alphabet, the
/// most expensive of the three token kinds per output byte (a match or
/// rep byte, by contrast, is a single unmodeled array copy in this
/// module's `copy_checked`) — the opposite of "cheapest branch" an
/// earlier version of this comment claimed.
/// A pre-ADR-0038 measurement (release build, this project's CI runner
/// class) found a declared length of 256 MiB decoding in ~314s at a
/// steady ~1170 ns/byte through the old direct-division
/// `Literal::decode`, linear in declared length with no polynomial or
/// worse blowup found from 1 MiB to 256 MiB. `decode_sse`'s own per-byte
/// cost is a bounded constant more (8 `Sse::refine`/`Decoder::decode_bit`
/// calls instead of one direct cumulative-table scan), not a new
/// asymptotic shape: a smaller-scale check after this change (8 MiB of
/// incompressible data, forced through `Method::Lz` so every byte hits
/// `decode_sse`) measured ~1780 ns/byte, consistent with that bound.
/// Provisional either way: `ROADMAP.md` M4's streaming/block API is the
/// real fix (bounded-memory decode without a single hardcoded file-size
/// ceiling), and should widen or remove this once it lands; the constant
/// itself is a `research/JOURNAL.md` S1-P6 speed-tier target
/// (`Literal::mix` rebuilds all 256 cumulative entries from scratch every
/// byte instead of an incremental structure), not something to chase down
/// here.
pub const MAX_DECODED_LEN: u32 = 256 * 1024 * 1024;

/// Flag symbols coded before every token, selecting which of the three
/// kinds follows. Matches the archive's `flag.enc(ac, {0,1,2})`.
const FLAG_LITERAL: usize = 0;
const FLAG_MATCH: usize = 1;
const FLAG_REP: usize = 2;
/// Alphabet size of the flag [`Model`]s: exactly the three symbols above.
const FLAG_ALPHABET: usize = 3;

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
            slot: Model::new(lz::REP_SLOTS),
        }
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

/// [`ideal_cost_bits`]'s counterpart to [`encode_bucketed`]: the bucket
/// symbol's modeled `-log2(p)` cost plus the residual low bits' cost, which
/// is exactly `bits` — [`crate::coder::Encoder::encode_bits`] emits them
/// raw and unmodeled, so their cost is their count, not a `Model` lookup.
fn ideal_cost_bucketed(model: &mut Model, value: u32) -> f64 {
    let b = lz::bucket(value);
    let cost = model.ideal_cost_bits(b);
    // b is a Model alphabet index, at most OFFSET_BUCKETS - 1 (20): always
    // fits u32, same bound encode_bucketed relies on.
    let bits = u32::try_from(b).expect("bucket index is small, always fits u32");
    cost + f64::from(bits)
}

/// Applies `candidate`'s filter to `data`, or returns a copy of it
/// unchanged for [`Candidate::Identity`]. Every filter here preserves
/// length, so the result is always `data.len()` bytes.
fn apply_filter(candidate: Candidate, data: &[u8]) -> Vec<u8> {
    match candidate {
        Candidate::Identity => data.to_vec(),
        Candidate::Delta(stride) => filters::delta::encode(data, stride),
        Candidate::Bcj => filters::bcj::encode(data),
        Candidate::Transpose(columns) => filters::transpose::encode(data, columns),
    }
}

/// Inverse of [`apply_filter`]: reconstructs the original bytes from
/// `data` (the filtered bytes [`decode`] just reassembled) and the
/// `candidate` its payload named.
fn undo_filter(candidate: Candidate, data: Vec<u8>) -> Vec<u8> {
    match candidate {
        Candidate::Identity => data,
        Candidate::Delta(stride) => filters::delta::decode(&data, stride),
        Candidate::Bcj => filters::bcj::decode(&data),
        Candidate::Transpose(columns) => filters::transpose::decode(&data, columns),
    }
}

/// Where a token's flag/length/offset/slot symbols and literal bytes go:
/// real arithmetic coding for [`encode_tokens`]'s [`EncodeSink`], summed
/// `-log2(p)` pricing for [`ideal_cost_bits`]'s [`CostSink`]. The two walks
/// must code the same fields, in the same order, off the same token stream,
/// or [`ideal_cost_bits`] silently stops pricing what [`encode_tokens`]
/// actually emits; routing both through [`walk_tokens`] makes that a single
/// piece of code instead of two loops kept in sync by hand.
trait TokenSink {
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: usize);
    fn literal(&mut self, models: &mut Models, context: Context, byte: u8);
    fn length(&mut self, models: &mut Models, value: u32);
    fn offset(&mut self, models: &mut Models, value: u32);
    fn slot(&mut self, models: &mut Models, symbol: usize);
}

/// The shared skeleton behind [`encode_tokens`] and [`ideal_cost_bits`]:
/// walks `tokens` (already parsed from `data`) in coding order, routing
/// every symbol through `sink`, and advancing the literal-model context
/// exactly as [`Context::after_literal`]/[`Context::after_copy`] require.
fn walk_tokens(tokens: &[Token], data: &[u8], models: &mut Models, sink: &mut impl TokenSink) {
    let mut context = Context::default();
    let mut pos = 0usize;

    for token in tokens {
        let flag_table = usize::from(context.after_copy);
        match *token {
            Token::Literal(byte) => {
                sink.flag(models, flag_table, FLAG_LITERAL);
                sink.literal(models, context, byte);
                context = context.after_literal(byte);
                pos += 1;
            }
            Token::Match { len, distance } => {
                sink.flag(models, flag_table, FLAG_MATCH);
                sink.length(models, len);
                sink.offset(models, distance.get());
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
            Token::Rep { len, slot } => {
                sink.flag(models, flag_table, FLAG_REP);
                sink.slot(models, slot.index());
                sink.length(models, len);
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
        }
    }
}

/// [`TokenSink`] that drives a real [`Encoder`], [`walk_tokens`]'s use in
/// [`encode_tokens`].
struct EncodeSink<'a> {
    ac: &'a mut Encoder,
}

impl TokenSink for EncodeSink<'_> {
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: usize) {
        models.flag[flag_table].encode(self.ac, kind);
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        // Compression always targets the newest format version
        // (`FORMAT_VERSION`), so encoding always takes the SSE-calibrated
        // path; `decode` is the one that must still read older frames.
        models.literal.encode_sse(self.ac, context, byte);
    }

    fn length(&mut self, models: &mut Models, value: u32) {
        encode_bucketed(&mut models.length, self.ac, value);
    }

    fn offset(&mut self, models: &mut Models, value: u32) {
        encode_bucketed(&mut models.offset, self.ac, value);
    }

    fn slot(&mut self, models: &mut Models, symbol: usize) {
        models.slot.encode(self.ac, symbol);
    }
}

/// [`TokenSink`] that sums `-log2(p)` instead of coding, [`walk_tokens`]'s
/// use in [`ideal_cost_bits`].
#[derive(Default)]
struct CostSink {
    bits: f64,
}

impl TokenSink for CostSink {
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: usize) {
        self.bits += models.flag[flag_table].ideal_cost_bits(kind);
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        // Matches EncodeSink::literal's encode_sse path (this trait's own
        // docs: CostSink and EncodeSink must price and code the same
        // thing), so ideal_cost_bits stays a true estimate of what
        // encode_tokens's real Encoder pays.
        self.bits += models.literal.ideal_cost_bits_sse(context, byte);
    }

    fn length(&mut self, models: &mut Models, value: u32) {
        self.bits += ideal_cost_bucketed(&mut models.length, value);
    }

    fn offset(&mut self, models: &mut Models, value: u32) {
        self.bits += ideal_cost_bucketed(&mut models.offset, value);
    }

    fn slot(&mut self, models: &mut Models, symbol: usize) {
        self.bits += models.slot.ideal_cost_bits(symbol);
    }
}

/// Encodes already-filtered `data` through the LZ + context-mixing
/// pipeline: [`encode`]'s per-candidate trial body, and the whole of what
/// this function used to be before filter trial-selection wrapped it.
///
/// # Panics
///
/// Panics if `data.len()` exceeds `u32::MAX`: the declared-output-length
/// header field is a `u32`, the same bound [`lz::parse_greedy`] already
/// enforces. [`crate::compress`] checks this before calling in, so
/// nothing reachable from the public API hits it today.
fn encode_tokens(data: &[u8]) -> Vec<u8> {
    let declared_len = u32::try_from(data.len())
        .expect("codec::encode: input longer than u32::MAX is not supported yet");

    let tokens = lz::parse_optimal(data);
    let token_count = u32::try_from(tokens.len())
        .expect("token count bounded by input length, already checked to fit u32 above");

    let mut models = Models::new();
    let mut ac = Encoder::new();
    walk_tokens(&tokens, data, &mut models, &mut EncodeSink { ac: &mut ac });

    let mut out = Vec::with_capacity(8 + data.len() / 2);
    out.extend_from_slice(&declared_len.to_le_bytes());
    out.extend_from_slice(&token_count.to_le_bytes());
    out.extend(ac.finish());
    out
}

/// Sums the whole-codec ideal coding cost of already-filtered `data`, in
/// bits: ROADMAP M2's ideal-cost accounting mode (ADR-0006), the slice
/// `research/JOURNAL.md` S2-A30 and S2-A31 each flagged as remaining scope
/// after building `Model::ideal_cost_bits` and `Literal::ideal_cost_bits`
/// respectively. Walks the same `lz::parse_optimal` token stream this
/// module's private `encode_tokens` would encode and prices every
/// flag/length/offset/slot symbol and every literal byte through those two
/// methods instead of an [`Encoder`], so an experiment loop can price a
/// whole file's coding cost under this crate's real adaptive models without
/// paying for real arithmetic coding or trialing candidate filters (this
/// operates on one already-chosen filter's output, the same layer
/// `encode_tokens` does, not on [`encode`]'s filter-selection loop above
/// it).
#[must_use]
pub fn ideal_cost_bits(data: &[u8]) -> f64 {
    let tokens = lz::parse_optimal(data);
    let mut models = Models::new();
    let mut sink = CostSink::default();
    walk_tokens(&tokens, data, &mut models, &mut sink);
    sink.bits
}

/// Encodes `data` into a `Method::Lz` payload: trials every candidate
/// filter [`filters::select::pick`] shortlists, keeps whichever produces
/// the smallest `encode_tokens` body, and prefixes that body with the
/// winning candidate's 2-byte selector (see the module docs' "Payload
/// layout"). Filters are trialed against the raw input directly (never
/// stacked), matching the archive's `encode`.
///
/// # Panics
///
/// Panics if `data.len()` exceeds `u32::MAX`; see `encode_tokens`'s
/// docs, which this delegates to per candidate.
#[must_use]
pub fn encode(data: &[u8]) -> Vec<u8> {
    let mut best: Option<(Candidate, Vec<u8>)> = None;
    for candidate in filters::select::pick(data) {
        let filtered = apply_filter(candidate, data);
        let body = encode_tokens(&filtered);
        if best
            .as_ref()
            .is_none_or(|(_, existing)| body.len() < existing.len())
        {
            best = Some((candidate, body));
        }
    }
    let (candidate, body) =
        best.expect("filters::select::pick always returns at least Candidate::Identity");

    let mut out = Vec::with_capacity(2 + body.len());
    out.extend_from_slice(&candidate.to_header_bytes());
    out.extend(body);
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
/// Returns [`Error::Truncated`] if `payload` is shorter than the 2-byte
/// filter selector plus the 8-byte declared-length/token-count header.
/// Returns [`Error::Corrupt`] if the filter selector is not one
/// [`Candidate::from_header_bytes`] recognizes. Returns
/// [`Error::TooLarge`] if the declared length exceeds
/// [`MAX_DECODED_LEN`], checked before any allocation or decode work.
/// Returns [`Error::Corrupt`] if a match or rep token's distance reaches
/// before the start of decoded output, if a token would grow decoded
/// output past the declared length, or if the final decoded length does
/// not equal it: all adversarial or malformed input, never a bug in this
/// decoder (`rust-craft` skill, panic-discipline).
///
/// `version` is the frame's declared `FORMAT_VERSION` byte
/// (`crate::decompress` already has it in scope at its one call site):
/// versions below 3 decode the literal sub-stream through
/// [`crate::literal::Literal::decode`] (the old direct 256-way division),
/// version 3 and above through [`crate::literal::Literal::decode_sse`] (see
/// the module docs' "Payload layout" section). Every other symbol decodes
/// identically regardless of `version`, since only the literal sub-stream's
/// internal shape changed.
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
pub fn decode(payload: &[u8], version: u8) -> Result<Vec<u8>, Error> {
    let (filter_bytes, payload) = payload.split_at_checked(2).ok_or(Error::Truncated)?;
    let candidate =
        Candidate::from_header_bytes([filter_bytes[0], filter_bytes[1]]).ok_or(Error::Corrupt)?;
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
                let byte = if version >= LITERAL_SSE_MIN_VERSION {
                    models.literal.decode_sse(&mut ac, context)
                } else {
                    models.literal.decode(&mut ac, context)
                };
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
                // models.flag's alphabet is FLAG_ALPHABET (3), so this arm
                // is FLAG_REP (2), never a fourth flag value.
                // RepSlot::from_index documents why models.slot's decode
                // is safe to feed it directly.
                let slot = RepSlot::from_index(models.slot.decode(&mut ac));
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
    Ok(undo_filter(candidate, output))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(data: &[u8]) {
        let encoded = encode(data);
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION).as_deref(),
            Ok(data),
            "roundtrip mismatch"
        );
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
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION).as_deref(),
            Ok(data.as_slice())
        );
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
    fn roundtrip_columnar_drift_data_uses_a_non_identity_filter() {
        // Four independent small-step random walks, one per column, laid
        // out row-major with a 4-byte stride: consecutive same-column
        // bytes drift by a small step (filters::select::pick ranks
        // Candidate::Delta(4) top for exactly this shape — mirrors its
        // own pick_selects_delta_for_columnar_drift test), but
        // consecutive raw bytes belong to different, unrelated walks.
        // Proves encode() actually wires a trial-selected filter into the
        // frame, not just plumbs pick() through unused, and that decode()
        // correctly reverses it.
        const STEPS: [u8; 5] = [0u8.wrapping_sub(2), 0u8.wrapping_sub(1), 0, 1, 2];
        let mut rng = crate::test_support::Xorshift32::new(0x1234_5678);
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

        let encoded = encode(&data);
        assert_ne!(
            encoded[0], 0,
            "columnar drift data should select a non-identity filter, got kind byte {}",
            encoded[0]
        );
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION).as_deref(),
            Ok(data.as_slice())
        );
    }

    #[test]
    fn truncated_header_is_rejected() {
        assert_eq!(
            decode(&[0u8; 4], crate::FORMAT_VERSION),
            Err(Error::Truncated)
        );
        assert_eq!(decode(&[], crate::FORMAT_VERSION), Err(Error::Truncated));
    }

    #[test]
    fn unknown_filter_selector_is_rejected_not_panicking() {
        // Kind byte 5 names no Candidate::from_header_bytes ever produces
        // (0=Identity, 1=Delta, 2=Bcj, 3=Transpose): an adversarial or
        // future-format payload, never a bug in this decoder.
        let mut payload = vec![5u8, 0u8];
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode(&payload, crate::FORMAT_VERSION), Err(Error::Corrupt));
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
        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&u32::MAX.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            decode(&payload, crate::FORMAT_VERSION),
            Err(Error::TooLarge(u32::MAX))
        );
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
        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&over.to_le_bytes());
        payload.extend_from_slice(&over.to_le_bytes());
        assert_eq!(
            decode(&payload, crate::FORMAT_VERSION),
            Err(Error::TooLarge(over))
        );
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

        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&4u32.to_le_bytes()); // declared_len
        payload.extend_from_slice(&1u32.to_le_bytes()); // token_count
        payload.extend(ac_bytes);

        assert_eq!(decode(&payload, crate::FORMAT_VERSION), Err(Error::Corrupt));
    }

    #[test]
    fn ideal_cost_bits_is_zero_on_empty_input() {
        assert!(ideal_cost_bits(b"").abs() < 1e-9);
    }

    #[test]
    fn ideal_cost_bits_tracks_real_encoded_length_within_one_percent() {
        // Named corpus (CLAUDE.md hard rule 4): the founding session's
        // archived codec, real structured Rust source, 25,524 bytes — the
        // same fixture roundtrip_founding_archive_source and literal.rs's
        // vendored-exp accuracy test use. encode_tokens's real Encoder
        // output is compared past its 8-byte declared-length/token-count
        // header (no ideal_cost_bits call ever prices that header): summed
        // ideal cost is an estimate, not the real coder's bit-exact output
        // (integer cumulative-frequency division rounds; the coder also
        // pays a handful of flush bits at the end), so this checks
        // closeness, not equality — the same tolerance shape as
        // model.rs's and literal.rs's own ideal-cost accuracy tests.
        let data: &[u8] = include_bytes!("../research/imports/session-1/mothergod.rs");

        let ideal_bits = ideal_cost_bits(data);

        let real = encode_tokens(data);
        #[allow(
            clippy::cast_precision_loss,
            reason = "encoded length is far below f64's exact integer range (2^53)"
        )]
        let real_bits = ((real.len() - 8) * 8) as f64;

        let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
        assert!(
            relative_diff <= 0.01,
            "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
             {relative_diff:.4} relative difference exceeds the 1% budget"
        );
    }

    #[test]
    fn ideal_cost_bits_is_lower_for_repetitive_than_random_data() {
        // A sanity check the accuracy test above can't give directly: the
        // ideal-cost pass must actually reflect the LZ/model pipeline's own
        // sense of compressibility, not just track real encoded length on
        // one fixture. A 50x repeat of an 8-byte pattern (long enough to
        // clear lz::OPTIMAL_MIN_LEN) must cost far fewer bits per byte than
        // pseudo-random bytes of the same length.
        let repetitive = b"abcdefgh".repeat(50);
        let random: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
            .take(repetitive.len())
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();

        #[allow(
            clippy::cast_precision_loss,
            reason = "byte lengths here are tiny, far below f64's exact integer range (2^53)"
        )]
        let (repetitive_len, random_len) = (repetitive.len() as f64, random.len() as f64);
        let repetitive_bpb = ideal_cost_bits(&repetitive) / repetitive_len;
        let random_bpb = ideal_cost_bits(&random) / random_len;
        assert!(
            repetitive_bpb < random_bpb / 2.0,
            "repetitive data's ideal cost ({repetitive_bpb} bits/byte) should be far below \
             random data's ({random_bpb} bits/byte)"
        );
    }
}
