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
//! selector ahead of the layout ADR-0026 shipped.
//!
//! [`decode`] reads exactly one wire version, `FORMAT_VERSION`: every
//! earlier version was retired outright, no release ever having written
//! one (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`,
//! `docs/adr/0063-retire-format-versions-1-through-9.md`), so
//! [`crate::decompress`] rejects any `Method::Lz` frame naming another
//! version before calling [`decode`] at all. Each literal byte codes
//! through [`crate::literal::Literal::encode_logit_sse`]/`decode_logit_sse`
//! (`docs/adr/0055-wire-logit-domain-sse-bins-into-the-literal-model.md`),
//! except in a frame whose filter selector names [`Candidate::Transpose`],
//! which goes one step further: each literal byte blends a column-keyed
//! seventh expert into the mix before the same SSE-calibrated coding
//! ([`crate::literal::Literal::encode_column`]/`decode_column`,
//! `docs/adr/0046-wire-the-column-expert-into-the-literal-mixer.md`,
//! `research/JOURNAL.md` S1-P5). A [`Token::Match`]'s and a [`Token::Rep`]'s
//! length code through two independent [`Model`]s
//! (`docs/adr/0057-wire-the-match-rep-length-model-split.md`), the bits
//! below a length's `lz::bucket` through an adaptive tree per model and
//! bucket (`docs/adr/0061-wire-the-length-residual-trees.md`), and a
//! [`Token::Match`]'s `offset` (its distance) through one of four
//! independent models selected by a coarse bucket of that match's length
//! (`docs/adr/0058-wire-the-offset-length-state-split.md`); a distance's
//! residual bits stay raw. `slot` codes through one model.
//!
//! The declared output length is [`decode`]'s allocation bound
//! (`docs/format/SPEC.md`, `rust-craft` skill's allocation-discipline): a
//! hostile payload can claim any token count or any match/rep length, but
//! [`decode`] never grows its output buffer past this field, and never
//! preallocates a capacity derived from it either. That field is itself
//! capped at [`decode`]'s caller-supplied `max_len`, in turn capped at
//! [`MAX_DECODED_LEN`] by [`crate::decompress_bounded`], so a tiny payload
//! cannot declare an unbounded length and force unbounded decode work; see
//! [`MAX_DECODED_LEN`]'s docs for why, and [`decode`]'s docs for the rest.
//!
//! [`ideal_cost_bits`] completes ROADMAP M2's ideal-cost accounting mode
//! (`research/JOURNAL.md` S2-A30/S2-A31 built the per-model pieces this
//! sums): the whole-codec `-log2(p)` pass across the flag/length/offset/slot
//! streams and literal bytes together, without touching an [`Encoder`].

use std::num::{NonZeroU32, NonZeroUsize};

use crate::Error;
use crate::coder::{Decoder, Encoder};
use crate::column;
use crate::filters::{self, select::Candidate};
use crate::literal::{
    ColumnExpertState, Context, Literal, PpmExpertState, SurpriseLogisticMixLogitSse,
};
use crate::lz::{self, RepCache, RepSlot, Token};
use crate::model::Model;

/// Fixed bank count [`crate::literal::ColumnExpertState`] sizes its storage
/// from on the real coding path (`encode_tokens`'s [`EncodeSink`], `decode`):
/// a decoder reads a frame's `columns` param from untrusted input, so bank
/// storage must size from a constant, never from that field directly
/// (`crate::column`'s own docs, CLAUDE.md hard rule 2). 256 covers every
/// column count `filters::select::pick` can ever choose (its widest
/// candidate is 96) with no aliasing at all, and matches the alphabet-sized
/// scale every other `literal.rs` bank-count constant in this range already
/// uses (`ALPHABET`).
const MAX_COLUMN_BANKS: NonZeroUsize = NonZeroUsize::new(256).unwrap();

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
/// every literal byte pays [`crate::literal::Literal::decode_logit_sse`]'s
/// full six-expert logit-domain mix plus 8 chained binary decisions over
/// the 256-symbol alphabet, the most expensive of the three token kinds
/// per output byte (a match or rep byte, by contrast, is a single
/// unmodeled array copy in this module's `copy_checked`).
/// Measured on the retired direct-division literal path (release build,
/// this project's CI runner class): a declared length of 256 MiB decoded
/// in ~314s at a steady ~1170 ns/byte, linear in declared length with no
/// polynomial or worse blowup found from 1 MiB to 256 MiB; the retired
/// SSE-calibrated path measured ~1780 ns/byte on 8 MiB of incompressible
/// data. `decode_logit_sse`'s own per-node cost is a bounded constant
/// more (six `stretch` calls, a dot product, two EMA updates and one
/// stretch-domain `LogitSse` lookup per node), with no new loop or
/// allocation, so the bound stays linear in `declared_len`; not
/// separately remeasured. The length and distance model selection costs
/// a cheap `FlagKind` branch or array index per copy token, strictly
/// cheaper than the per-byte literal cost this bound is measured against.
/// Provisional either way: `ROADMAP.md` M4's streaming/block API is the
/// real fix (bounded-memory decode without a single hardcoded file-size
/// ceiling), and should widen or remove this once it lands; the constant
/// itself is a `research/JOURNAL.md` S1-P6 speed-tier target, not
/// something to chase down here.
pub const MAX_DECODED_LEN: u32 = 256 * 1024 * 1024;

/// Which of the three kinds a token codes as: the flag symbol coded
/// before every token. Matches the archive's `flag.enc(ac, {0,1,2})`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FlagKind {
    Literal,
    Match,
    Rep,
}

impl FlagKind {
    /// The [`Model`] symbol this kind codes as, `flag.enc`'s `{0,1,2}`.
    const fn index(self) -> usize {
        match self {
            Self::Literal => 0,
            Self::Match => 1,
            Self::Rep => 2,
        }
    }

    /// Inverse of [`Self::index`]. `models.flag`'s alphabet is
    /// [`FLAG_ALPHABET`] (3), so [`Model::decode`] never returns anything
    /// past `Rep`.
    const fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Literal,
            1 => Self::Match,
            _ => Self::Rep,
        }
    }
}

/// Alphabet size of the flag [`Model`]s: exactly [`FlagKind`]'s three symbols.
const FLAG_ALPHABET: usize = 3;

/// The adaptive tables `Method::Lz` drives, bundled so encode and decode
/// construct and thread them identically.
struct Models {
    literal: Literal,
    /// [`crate::literal::Literal::encode_logit_sse`]/`decode_logit_sse`
    /// code every non-`Candidate::Transpose` literal through this mixer:
    /// its own weights, error EMAs and `LogitSse` table, one per
    /// frame/trial, the same lifetime as `literal`'s six real banks.
    logit_sse: SurpriseLogisticMixLogitSse,
    /// One flag table per "was the previous token a copy" state (the
    /// archive's `flag[2]`): a literal run and a post-copy position have
    /// different flag distributions worth modeling separately. Indexed by
    /// [`Context::after_copy`].
    flag: [Model; 2],
    /// [`Token::Match`]'s own copy-length [`Model`], independent of
    /// `length_rep` (`research/JOURNAL.md` S2-A109/S2-A110,
    /// `docs/adr/0057-wire-the-match-rep-length-model-split.md`).
    length_match: Model,
    /// [`Token::Rep`]'s own copy-length [`Model`], independent of
    /// `length_match`, same lifetime.
    length_rep: Model,
    /// [`ResidualTree`] beside `length_match`: codes the bits below a
    /// match length's bucket.
    length_match_residual: ResidualTree,
    /// [`ResidualTree`] beside `length_rep`, same lifetime.
    length_rep_residual: ResidualTree,
    /// [`OFFSET_LEN_STATES`] independent match-distance [`Model`]s, indexed
    /// by [`offset_len_state`] of the triggering [`Token::Match`]'s own
    /// length (`research/JOURNAL.md` S2-A111/S2-A112,
    /// `docs/adr/0058-wire-the-offset-length-state-split.md`). Never
    /// consulted for a [`Token::Rep`], which has no `offset` symbol at all.
    offset_len: [Model; OFFSET_LEN_STATES],
    slot: Model,
}

impl Models {
    fn new() -> Self {
        Self {
            literal: Literal::new(),
            logit_sse: SurpriseLogisticMixLogitSse::new(),
            flag: [Model::new(FLAG_ALPHABET), Model::new(FLAG_ALPHABET)],
            length_match: Model::new(lz::LENGTH_BUCKETS),
            length_rep: Model::new(lz::LENGTH_BUCKETS),
            length_match_residual: ResidualTree::new(lz::LENGTH_BUCKETS),
            length_rep_residual: ResidualTree::new(lz::LENGTH_BUCKETS),
            offset_len: std::array::from_fn(|_| Model::new(lz::OFFSET_BUCKETS)),
            slot: Model::new(lz::REP_SLOTS),
        }
    }

    /// Fallible counterpart to [`Self::new`]: the same fresh tables, but
    /// returns `Err` instead of aborting if the allocator cannot satisfy
    /// one of them. [`decode`] and [`decode_undoable_streaming`] use this
    /// (hard rule 2, `rust-craft` skill's allocation-discipline,
    /// `tests/torture.rs`, #453); every encode path keeps [`Self::new`],
    /// where an allocator failure this small is already a fatal condition
    /// for the whole process.
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            literal: Literal::try_new()?,
            logit_sse: SurpriseLogisticMixLogitSse::try_new()?,
            flag: [
                Model::try_new(FLAG_ALPHABET)?,
                Model::try_new(FLAG_ALPHABET)?,
            ],
            length_match: Model::try_new(lz::LENGTH_BUCKETS)?,
            length_rep: Model::try_new(lz::LENGTH_BUCKETS)?,
            length_match_residual: ResidualTree::try_new(lz::LENGTH_BUCKETS)?,
            length_rep_residual: ResidualTree::try_new(lz::LENGTH_BUCKETS)?,
            // One `Model::try_new` call per `OFFSET_LEN_STATES` entry
            // (`std::array::try_from_fn` is not yet stable): the array
            // literal's length is checked against `[Model;
            // OFFSET_LEN_STATES]` by the compiler, so a future change to
            // that constant without updating this list fails to build
            // rather than silently constructing the wrong count.
            offset_len: [
                Model::try_new(lz::OFFSET_BUCKETS)?,
                Model::try_new(lz::OFFSET_BUCKETS)?,
                Model::try_new(lz::OFFSET_BUCKETS)?,
                Model::try_new(lz::OFFSET_BUCKETS)?,
            ],
            slot: Model::try_new(lz::REP_SLOTS)?,
        })
    }
}

/// Converts a [`lz::bucket`] alphabet index to the residual bit count
/// [`Encoder::encode_bits`]/[`Decoder::decode_bits`] take: `b` is at most
/// `OFFSET_BUCKETS - 1` (20), the largest bucket alphabet in use, so it
/// always fits `u32`.
fn bucket_bits(b: usize) -> u32 {
    u32::try_from(b).expect("bucket index is small, always fits u32")
}

/// Codes `value` (a match/rep length, or a match distance) as a
/// [`lz::bucket`] symbol through `model`, then the residual low bits of
/// `value` within that bucket as raw, unmodeled bits. Matches the
/// archive's `lenm.enc(ac,lb); ac.bits(l,lb)` (and the identical shape for
/// offsets).
fn encode_bucketed(model: &mut Model, ac: &mut Encoder, value: u32) {
    let b = lz::bucket(value);
    model.encode(ac, b);
    ac.encode_bits(value, bucket_bits(b));
}

/// Inverse of [`encode_bucketed`]: decodes a bucket symbol, then the
/// residual bits, and reconstructs `value` as `(1 << bucket) |
/// residual_bits`. Never panics on adversarial `ac` state: `model.decode`
/// and `ac.decode_bits` are both panic-free on any input (see their own
/// docs), and the shift below is bounded by the same small-alphabet
/// argument as [`encode_bucketed`].
fn decode_bucketed(model: &mut Model, ac: &mut Decoder) -> u32 {
    let b = model.decode(ac);
    let bits = bucket_bits(b);
    (1u32 << bits) | ac.decode_bits(bits)
}

/// [`ideal_cost_bits`]'s counterpart to [`encode_bucketed`]: the bucket
/// symbol's modeled `-log2(p)` cost plus the residual low bits' cost, which
/// is exactly `bits` — [`crate::coder::Encoder::encode_bits`] emits them
/// raw and unmodeled, so their cost is their count, not a `Model` lookup.
fn ideal_cost_bucketed(model: &mut Model, value: u32) -> f64 {
    let b = lz::bucket(value);
    let cost = model.ideal_cost_bits(b);
    cost + f64::from(bucket_bits(b))
}

/// Picks [`Models::length_match`]/`length_rep` by `kind`, the split every
/// real-path [`TokenSink`] and [`DecodeSink`] (and the `ideal_cost_bits`
/// pricer that must price what they code) selects a copy token's length
/// model through.
///
/// # Panics
///
/// Panics if `kind` is [`FlagKind::Literal`]: [`walk_tokens`] never calls
/// [`TokenSink::length`] for a [`Token::Literal`], so every real caller
/// here already has a [`FlagKind::Match`] or [`FlagKind::Rep`] in hand.
fn split_length_model(models: &mut Models, kind: FlagKind) -> &mut Model {
    match kind {
        FlagKind::Match => &mut models.length_match,
        FlagKind::Rep => &mut models.length_rep,
        FlagKind::Literal => {
            unreachable!("walk_tokens never calls length for a Token::Literal")
        }
    }
}

/// Picks [`Models::length_match_residual`]/`length_rep_residual` by `kind`,
/// the [`ResidualTree`] beside [`split_length_model`]'s bucket model.
///
/// # Panics
///
/// Panics if `kind` is [`FlagKind::Literal`], same as
/// [`split_length_model`].
fn split_length_residual(models: &mut Models, kind: FlagKind) -> &mut ResidualTree {
    match kind {
        FlagKind::Match => &mut models.length_match_residual,
        FlagKind::Rep => &mut models.length_rep_residual,
        FlagKind::Literal => {
            unreachable!("walk_tokens never calls length for a Token::Literal")
        }
    }
}

/// `-log2(p)` cost of a flag symbol under the shared `models.flag` tables:
/// every pricing-only [`TokenSink`] ([`CostSink`], [`PairedTokenSink`])
/// must price `flag` this same way, since neither varies its coding.
fn price_flag(models: &mut Models, flag_table: usize, kind: FlagKind) -> f64 {
    models.flag[flag_table].ideal_cost_bits(kind.index())
}

/// `-log2(p)` cost of a literal byte under the shared, no-column-expert
/// literal path: [`CostSink`]'s own `literal` pricing (it never trials
/// [`Candidate::Transpose`]; see its own docs).
fn price_literal(models: &mut Models, context: Context, byte: u8) -> f64 {
    models
        .literal
        .ideal_cost_bits_logistic_surprise_logit_sse(context, byte, &mut models.logit_sse)
}

/// `-log2(p)` cost of a copy token's length, bucket symbol through
/// whichever of [`Models::length_match`]/`length_rep`
/// [`split_length_model`] selects, residual bits through the
/// [`ResidualTree`] beside it: [`CostSink`]'s and [`PairedTokenSink`]'s
/// shared `length` pricing, matching [`EncodeSink::length`].
fn price_length(models: &mut Models, kind: FlagKind, value: u32) -> f64 {
    let symbol = split_length_model(models, kind).ideal_cost_bits(lz::bucket(value));
    symbol + split_length_residual(models, kind).ideal_cost_bits(value)
}

/// `-log2(p)` cost of a match's distance, through whichever of
/// [`Models::offset_len`]'s [`OFFSET_LEN_STATES`] models [`offset_len_state`]
/// selects from the triggering match's own `len`: [`CostSink`]'s and
/// [`PairedTokenSink`]'s shared `offset` pricing.
fn price_offset(models: &mut Models, len: u32, value: u32) -> f64 {
    ideal_cost_bucketed(&mut models.offset_len[offset_len_state(len)], value)
}

/// `-log2(p)` cost of a rep-slot symbol under the shared `models.slot`:
/// every pricing-only [`TokenSink`] ([`CostSink`], [`PairedTokenSink`])
/// must price `slot` this same way, since neither varies its coding.
fn price_slot(models: &mut Models, symbol: usize) -> f64 {
    models.slot.ideal_cost_bits(symbol)
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
///
/// # Errors
///
/// Returns [`Error::OutOfMemory`] if the allocator cannot satisfy the
/// undo buffer a non-`Identity` candidate needs (hard rule 2,
/// `rust-craft` skill's allocation-discipline, `tests/torture.rs`, #453).
fn undo_filter(candidate: Candidate, data: Vec<u8>) -> Result<Vec<u8>, Error> {
    match candidate {
        Candidate::Identity => Ok(data),
        Candidate::Delta(stride) => Ok(filters::delta::try_decode(&data, stride)?),
        Candidate::Bcj => Ok(filters::bcj::try_decode(&data)?),
        Candidate::Transpose(columns) => Ok(filters::transpose::try_decode(&data, columns)?),
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
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind);
    fn literal(&mut self, models: &mut Models, context: Context, byte: u8);
    /// `kind` is always [`FlagKind::Match`] or [`FlagKind::Rep`]:
    /// [`walk_tokens`] never calls this for a [`Token::Literal`].
    fn length(&mut self, models: &mut Models, kind: FlagKind, value: u32);
    /// `len` is the triggering [`Token::Match`]'s own already-emitted
    /// length: [`walk_tokens`] never calls this for a [`Token::Rep`] (which
    /// has no `offset` symbol at all) or a [`Token::Literal`].
    fn offset(&mut self, models: &mut Models, len: u32, value: u32);
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
                sink.flag(models, flag_table, FlagKind::Literal);
                sink.literal(models, context, byte);
                context = context.after_literal(byte);
                pos += 1;
            }
            Token::Match { len, distance } => {
                sink.flag(models, flag_table, FlagKind::Match);
                sink.length(models, FlagKind::Match, len);
                sink.offset(models, len, distance.get());
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
            Token::Rep { len, slot } => {
                sink.flag(models, flag_table, FlagKind::Rep);
                sink.slot(models, slot.index());
                sink.length(models, FlagKind::Rep, len);
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
        }
    }
}

/// [`EncodeSink`]'s column-coding state for a [`Candidate::Transpose`]
/// trial: `columns` is the candidate's own param, `data_len` is the
/// filtered data's length (`crate::column::column_of`'s `len`, always the
/// pre-filter length too since every filter here preserves length), and
/// `state` is the seventh expert's own fresh bank/weight/SSE state for this
/// one trial encode.
struct ColumnCoding<'a> {
    columns: NonZeroUsize,
    data_len: usize,
    state: &'a mut ColumnExpertState,
}

/// [`TokenSink`] that drives a real [`Encoder`], [`walk_tokens`]'s use in
/// [`encode_tokens`]. `column` is `Some` exactly when the candidate under
/// trial is [`Candidate::Transpose`] (`encode`'s caller), selecting
/// [`crate::literal::Literal::encode_column`] over
/// [`crate::literal::Literal::encode_logit_sse`] for every literal in this
/// trial (`research/JOURNAL.md` S1-P5).
struct EncodeSink<'a> {
    ac: &'a mut Encoder,
    column: Option<ColumnCoding<'a>>,
}

impl TokenSink for EncodeSink<'_> {
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind) {
        models.flag[flag_table].encode(self.ac, kind.index());
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        match &mut self.column {
            Some(col) => {
                let bank = column::bank_of(
                    context.position,
                    col.columns,
                    col.data_len,
                    MAX_COLUMN_BANKS,
                );
                models
                    .literal
                    .encode_column(self.ac, context, byte, bank, col.state);
            }
            None => {
                models
                    .literal
                    .encode_logit_sse(self.ac, context, byte, &mut models.logit_sse);
            }
        }
    }

    fn length(&mut self, models: &mut Models, kind: FlagKind, value: u32) {
        split_length_model(models, kind).encode(self.ac, lz::bucket(value));
        split_length_residual(models, kind).encode(self.ac, value);
    }

    fn offset(&mut self, models: &mut Models, len: u32, value: u32) {
        encode_bucketed(
            &mut models.offset_len[offset_len_state(len)],
            self.ac,
            value,
        );
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
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind) {
        self.bits += price_flag(models, flag_table, kind);
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        // Matches EncodeSink::literal's None branch (encode_logit_sse;
        // every CostSink caller here parses `data` with no filter selection,
        // so Candidate::Transpose's encode_column never arises) — this
        // trait's own docs: CostSink and EncodeSink must price and code the
        // same thing, so ideal_cost_bits stays a true estimate of what
        // encode_tokens's real Encoder pays.
        self.bits += price_literal(models, context, byte);
    }

    fn length(&mut self, models: &mut Models, kind: FlagKind, value: u32) {
        // Matches EncodeSink::length (split models, residual trees): ideal_cost_bits
        // stays a true estimate of what encode_tokens's real Encoder pays.
        self.bits += price_length(models, kind, value);
    }

    fn offset(&mut self, models: &mut Models, len: u32, value: u32) {
        // Matches EncodeSink::offset (the length-keyed models):
        // ideal_cost_bits stays a true estimate of what encode_tokens's
        // real Encoder pays.
        self.bits += price_offset(models, len, value);
    }

    fn slot(&mut self, models: &mut Models, symbol: usize) {
        self.bits += price_slot(models, symbol);
    }
}

/// Encodes already-filtered `data` through the LZ + context-mixing
/// pipeline: [`encode`]'s per-candidate trial body, and the whole of what
/// this function used to be before filter trial-selection wrapped it.
/// `columns` is `Some` exactly when this trial's candidate is
/// [`Candidate::Transpose`] with that column count, selecting the
/// column-expert literal path for the whole trial (see [`EncodeSink`]).
/// Panics under the same condition as [`encode_tokens_with`], which this
/// delegates to.
fn encode_tokens(data: &[u8], columns: Option<NonZeroUsize>) -> Vec<u8> {
    encode_tokens_with(data, columns, &lz::parse_optimal(data))
}

/// `encode_tokens`'s real-bitstream length, but parsed with
/// [`lz::parse_optimal_with_window`] under `window` instead of the wired
/// [`lz::WINDOW`] (`research/JOURNAL.md` S1-P4): the real-`Encoder`
/// counterpart to [`ideal_cost_bits_with_window`], which prices the same
/// candidate through modeled cost only. Lets a candidate window's actual
/// compressed size be measured without wiring it into `encode_tokens` or
/// `compress`/`encode`, and without bumping `FORMAT_VERSION`: the returned
/// length is a real payload byte count, but not itself guaranteed
/// decodable by this crate's current [`decode`] if `window` exceeds
/// [`lz::WINDOW`] (`ensure_within_window` still rejects a distance past
/// it, same as [`ideal_cost_bits_with_window`]'s own ideal-cost-only
/// caveat). See [`lz::parse_optimal_with_window`]'s docs for the bucket
/// ceiling `window` must stay under.
#[must_use]
pub fn compressed_len_with_window(data: &[u8], window: usize) -> usize {
    encode_tokens_with(data, None, &lz::parse_optimal_with_window(data, window)).len()
}

/// Same as [`compressed_len_with_window`], but parsed with
/// [`lz::parse_optimal_adaptive_window`] instead of a fixed candidate
/// `window`: the real-`Encoder` counterpart to
/// [`ideal_cost_bits_adaptive_window`], which prices the same parse
/// through modeled cost only. Not wired to `encode_tokens` itself
/// (`research/JOURNAL.md` S1-P4's own remaining-scope note on that lead):
/// a real-bitstream measurement first, the same shape every earlier S1-P4
/// slice took before any wiring decision.
#[must_use]
pub fn compressed_len_adaptive_window(data: &[u8]) -> usize {
    encode_tokens_with(data, None, &lz::parse_optimal_adaptive_window(data)).len()
}

/// Same as [`compressed_len_with_window`], but exposes both of
/// `lz::parse_optimal_with_seed_and_search_window`'s windows as
/// independent parameters instead of fixing the seed pass to [`lz::WINDOW`]
/// (`research/JOURNAL.md` S1-P4). [`compressed_len_adaptive_window`] always
/// passes the same value for both (the matched-window contract its own
/// gate relies on); this entry point lets a mismatch be re-measured on a
/// new candidate data class without touching that wired caller. S2-R9
/// found a mismatch (seed capped at [`lz::WINDOW`] while every `dp_round`
/// searched past it) that regressed `access_log`/`json_records`; S2-A93
/// fixed it by matching the windows. Whether the same mismatch helps or
/// hurts `Base64Wrapped`, whose far matches are S1-P4's current open
/// failure (S2-R11/S2-A95), is exactly what this measures.
#[must_use]
pub fn compressed_len_with_seed_and_search_window(
    data: &[u8],
    seed_window: usize,
    search_window: usize,
) -> usize {
    encode_tokens_with(
        data,
        None,
        &lz::parse_optimal_with_seed_and_search_window(data, seed_window, search_window),
    )
    .len()
}

/// Shared body of [`encode_tokens`], [`compressed_len_with_window`], and
/// [`compressed_len_adaptive_window`]: encodes already-parsed `tokens`
/// through the context-mixing pipeline and returns the real payload bytes.
///
/// # Panics
///
/// Panics if `data.len()` exceeds `u32::MAX`: the declared-output-length
/// header field is a `u32`, the same bound [`lz::parse_greedy`] already
/// enforces. [`crate::compress`] checks this before calling in, so
/// nothing reachable from the public API hits it today.
fn encode_tokens_with(data: &[u8], columns: Option<NonZeroUsize>, tokens: &[Token]) -> Vec<u8> {
    let declared_len = u32::try_from(data.len())
        .expect("codec::encode: input longer than u32::MAX is not supported yet");
    let token_count = u32::try_from(tokens.len())
        .expect("token count bounded by input length, already checked to fit u32 above");

    let mut models = Models::new();
    let mut ac = Encoder::new();
    let mut column_state = columns.map(|_| ColumnExpertState::new(MAX_COLUMN_BANKS));
    let column = columns
        .zip(column_state.as_mut())
        .map(|(columns, state)| ColumnCoding {
            columns,
            data_len: data.len(),
            state,
        });
    walk_tokens(
        tokens,
        data,
        &mut models,
        &mut EncodeSink {
            ac: &mut ac,
            column,
        },
    );

    let mut out = Vec::with_capacity(TOKEN_HEADER_LEN + data.len() / 2);
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
    ideal_cost_bits_with_window(data, lz::WINDOW)
}

/// Same as [`ideal_cost_bits`], but parses `data` with [`lz::parse_optimal_with_window`]
/// under `window` instead of the wired [`lz::WINDOW`] (`research/JOURNAL.md`
/// S1-P4): measures a candidate window's real coding-cost effect through
/// this crate's actual adaptive models, without wiring it into
/// [`encode`] or bumping `FORMAT_VERSION`. See
/// [`lz::parse_optimal_with_window`]'s docs for the bucket ceiling
/// `window` must stay under.
#[must_use]
pub fn ideal_cost_bits_with_window(data: &[u8], window: usize) -> f64 {
    ideal_cost_for_tokens(data, &lz::parse_optimal_with_window(data, window))
}

/// Same as [`ideal_cost_bits`], but parses `data` with
/// [`lz::parse_optimal_adaptive_window`] instead of the wired
/// [`lz::WINDOW`] (`research/JOURNAL.md` S1-P4): measures the gated
/// window's real coding-cost effect through this crate's actual adaptive
/// models, without wiring it into [`encode`] or bumping `FORMAT_VERSION`.
#[must_use]
pub fn ideal_cost_bits_adaptive_window(data: &[u8]) -> f64 {
    ideal_cost_for_tokens(data, &lz::parse_optimal_adaptive_window(data))
}

/// Shared body of [`ideal_cost_bits_with_window`] and
/// [`ideal_cost_bits_adaptive_window`]: prices already-parsed `tokens`
/// through [`CostSink`] and returns the total. Mirrors
/// [`encode_tokens_with`], the same already-parsed-tokens-in shape on the
/// real-`Encoder` side of this same pair of measurements.
fn ideal_cost_for_tokens(data: &[u8], tokens: &[Token]) -> f64 {
    let mut models = Models::new();
    let mut sink = CostSink::default();
    walk_tokens(tokens, data, &mut models, &mut sink);
    sink.bits
}

/// Accumulates the two totals a paired-experiment [`TokenSink`]
/// ([`PairedTokenSink`]) prices side by side: `baseline` for the shipped
/// model, `candidate` for the same event under the experimental change. One
/// shared [`add`](Self::add) means a symbol can't be added to just one half
/// of the pair by accident.
#[derive(Default)]
struct PairedCost {
    baseline: f64,
    candidate: f64,
}

impl PairedCost {
    /// Adds `bits` to both totals, for a [`TokenSink`] event whose cost is
    /// identical under the baseline and the candidate.
    fn add_same(&mut self, bits: f64) {
        self.add(bits, bits);
    }

    /// Adds `baseline`/`candidate` to their own totals, for a
    /// [`TokenSink`] event (`literal`) where the two differ.
    fn add(&mut self, baseline: f64, candidate: f64) {
        self.baseline += baseline;
        self.candidate += candidate;
    }
}

/// `TokenSink` shared by every paired baseline/candidate experiment
/// ([`ideal_cost_bits_ppm_expert_experiment`]): flag/length/offset/slot
/// price identically to [`CostSink`] and add the same bits to both
/// [`PairedCost`] halves, since only the literal model differs between an
/// experiment's baseline and candidate. `pair_literal` supplies that one
/// difference: the experiment-specific `(baseline_bits, candidate_bits)`
/// pairing function.
struct PairedTokenSink<F> {
    cost: PairedCost,
    pair_literal: F,
}

impl<F> TokenSink for PairedTokenSink<F>
where
    F: FnMut(&mut Models, Context, u8) -> (f64, f64),
{
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind) {
        self.cost.add_same(price_flag(models, flag_table, kind));
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        let (baseline, candidate) = (self.pair_literal)(models, context, byte);
        self.cost.add(baseline, candidate);
    }

    fn length(&mut self, models: &mut Models, kind: FlagKind, value: u32) {
        // Matches CostSink::length (the split models): this sink's
        // baseline half must equal ideal_cost_bits's own length pricing
        // exactly (not its literal pricing, which differs on purpose — see
        // `tests::LiteralOnlyCostSink`'s docs), the guard
        // `ppm_expert_experiment_baseline_non_literal_subtotal_matches_ideal_cost_bits`
        // checks.
        self.cost.add_same(price_length(models, kind, value));
    }

    fn offset(&mut self, models: &mut Models, len: u32, value: u32) {
        // Matches CostSink::offset (the length-keyed models): this sink's
        // baseline half must equal ideal_cost_bits's own offset pricing
        // exactly, the same guard `length`'s own comment above names.
        self.cost.add_same(price_offset(models, len, value));
    }

    fn slot(&mut self, models: &mut Models, symbol: usize) {
        self.cost.add_same(price_slot(models, symbol));
    }
}

/// `research/JOURNAL.md` S1-P3's before-wiring measurement: does blending
/// [`crate::ppm::Ppm`] in as a genuinely additive expert help, priced
/// through [`Literal::ideal_cost_bits_ppm_expert_pair`], the same
/// "measure before wiring" shape S1-P5's column expert (`JOURNAL` S2-A69)
/// and S1-P2's fieldtype expert (`JOURNAL` S2-R15) each used for their own
/// candidate expert. Not reachable from [`encode`]/[`decode`]: no
/// `Method`/`FORMAT_VERSION` wiring, measurement only.
///
/// Returns `(baseline_bits, with_ppm_bits)`.
#[must_use]
pub fn ideal_cost_bits_ppm_expert_experiment(data: &[u8]) -> (f64, f64) {
    let tokens = lz::parse_optimal(data);
    let mut models = Models::new();
    let mut ppm_state = PpmExpertState::new();
    let mut sink = PairedTokenSink {
        cost: PairedCost::default(),
        pair_literal: |models: &mut Models, context: Context, byte: u8| {
            models
                .literal
                .ideal_cost_bits_ppm_expert_pair(context, byte, &mut ppm_state)
        },
    };
    walk_tokens(&tokens, data, &mut models, &mut sink);
    (sink.cost.baseline, sink.cost.candidate)
}

/// How many independent [`Model`]s [`Models::offset_len`] holds, and the
/// bucket count [`offset_len_state`] ever returns: LZMA's own
/// `len_to_pos_state` uses four states (`lzma-specification.txt`,
/// `research/JOURNAL.md` S2-A111/S2-A112).
const OFFSET_LEN_STATES: usize = 4;

/// LZMA's `len_to_pos_state` formula, adapted to this crate's own
/// [`lz::MIN_MATCH_LEN`] floor (LZMA's own floor is 2, not 4): the first
/// [`OFFSET_LEN_STATES`] match lengths each get their own state, every
/// longer length saturates into the last one. `len` is always a real
/// [`Token::Match`] length (`>= lz::MIN_MATCH_LEN` by that variant's own
/// construction), so the saturating subtraction only ever guards against
/// that invariant being violated, never a real underflow.
fn offset_len_state(len: u32) -> usize {
    let min = u32::try_from(lz::MIN_MATCH_LEN).expect("MIN_MATCH_LEN (4) always fits u32");
    (len.saturating_sub(min) as usize).min(OFFSET_LEN_STATES - 1)
}

/// How many of a bucketed length's residual bits, counted from the most
/// significant, [`ResidualTree`] models instead of sending raw: registered
/// before measuring (`research/JOURNAL.md` S2-A114), never tuned.
const RESIDUAL_MODELED_BITS: u32 = 4;

/// One adaptive binary tree per [`lz::bucket`] over the top
/// [`RESIDUAL_MODELED_BITS`] residual bits of a bucketed value, the rest
/// raw, LZMA-style (`lzma-specification.txt` models every length bit).
/// Invariant: bucket `b`'s nodes are `nodes[b << RESIDUAL_MODELED_BITS..][1..1
/// << RESIDUAL_MODELED_BITS]`; a walk of `k <= RESIDUAL_MODELED_BITS` bits
/// starts at node 1 and visits nodes below `1 << k`, so it never leaves
/// its own bucket's slice, and a `b` below the `buckets` the tree was
/// built for stays inside `nodes`.
struct ResidualTree {
    nodes: Vec<Model>,
}

impl ResidualTree {
    fn new(buckets: usize) -> Self {
        Self {
            nodes: vec![Model::new(2); buckets << RESIDUAL_MODELED_BITS],
        }
    }

    /// Fallible counterpart to [`Self::new`], for [`Models::try_new`]'s
    /// decode path (hard rule 2).
    fn try_new(buckets: usize) -> Result<Self, std::collections::TryReserveError> {
        let count = buckets << RESIDUAL_MODELED_BITS;
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(count)?;
        for _ in 0..count {
            nodes.push(Model::try_new(2)?);
        }
        Ok(Self { nodes })
    }

    /// Residual bit count of bucket `b`, split into `(modeled, raw)`: the
    /// top part through the tree, the rest sent raw.
    fn split(b: usize) -> (u32, u32) {
        let residual = bucket_bits(b);
        let modeled = residual.min(RESIDUAL_MODELED_BITS);
        (modeled, residual - modeled)
    }

    /// Codes `value`'s residual bits below its [`lz::bucket`]: the top
    /// `min(b, RESIDUAL_MODELED_BITS)` through this tree, most significant
    /// first, then the rest raw.
    fn encode(&mut self, ac: &mut Encoder, value: u32) {
        let b = lz::bucket(value);
        let (modeled, raw) = Self::split(b);
        let base = b << RESIDUAL_MODELED_BITS;
        let mut node = 1usize;
        for shift in (raw..raw + modeled).rev() {
            let bit = ((value >> shift) & 1) as usize;
            self.nodes[base + node].encode(ac, bit);
            node = node * 2 + bit;
        }
        ac.encode_bits(value, raw);
    }

    /// Inverse of [`Self::encode`] for the decoded bucket `b`: returns the
    /// whole value, `(1 << b) | residual`. Never panics on adversarial `ac`
    /// state: `b` is a decoded bucket symbol, always below the `buckets`
    /// this tree was built for, and [`Model::decode`] and
    /// [`Decoder::decode_bits`] are panic-free on any input.
    fn decode(&mut self, ac: &mut Decoder, b: usize) -> u32 {
        let (modeled, raw) = Self::split(b);
        let base = b << RESIDUAL_MODELED_BITS;
        let mut node = 1usize;
        for _ in 0..modeled {
            node = node * 2 + self.nodes[base + node].decode(ac);
        }
        // `node` is a leading 1 over the `modeled` decoded bits, so
        // shifting it left by `raw` already places the bucket's own bit.
        let top = u32::try_from(node).expect("node stays below 1 << (RESIDUAL_MODELED_BITS + 1)");
        (top << raw) | ac.decode_bits(raw)
    }

    /// Ideal cost of `value`'s residual bits below its bucket, priced
    /// exactly as [`Self::encode`] codes them.
    fn ideal_cost_bits(&mut self, value: u32) -> f64 {
        let b = lz::bucket(value);
        let (modeled, raw) = Self::split(b);
        let base = b << RESIDUAL_MODELED_BITS;
        let mut node = 1usize;
        let mut cost = f64::from(raw);
        for shift in (raw..raw + modeled).rev() {
            let bit = ((value >> shift) & 1) as usize;
            cost += self.nodes[base + node].ideal_cost_bits(bit);
            node = node * 2 + bit;
        }
        cost
    }
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
        let columns = match candidate {
            Candidate::Transpose(columns) => Some(columns),
            Candidate::Identity | Candidate::Delta(_) | Candidate::Bcj => None,
        };
        let body = encode_tokens(&filtered, columns);
        if best.as_ref().is_none_or(|(_, existing)| {
            crate::candidate_beats_incumbent(body.len(), existing.len())
        }) {
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

/// Byte width of one little-endian `u32` header field: [`read_u32_le`]'s
/// own unit, and the stride from one [`read_header`] field to the next.
const U32_LEN: usize = 4;

/// Byte length of [`encode_tokens_with`]'s header (declared output length
/// then token count, each a [`U32_LEN`]-byte `u32`): this module's own
/// "Payload layout" doc names it as the two 4-byte fields at relative
/// offsets 0 and 4, right before the range-coded stream. [`read_header`]
/// inverts it exactly, so a caller sizing a buffer around that header
/// (encode's own capacity hint, a test computing coded-bits-past-header)
/// reads this constant instead of re-deriving `2 * U32_LEN`.
const TOKEN_HEADER_LEN: usize = 2 * U32_LEN;

/// Reads the [`U32_LEN`]-byte little-endian `u32` at `payload[start..start +
/// U32_LEN]`.
///
/// # Errors
///
/// Returns [`Error::Truncated`] if `payload` is shorter than `start +
/// U32_LEN`.
fn read_u32_le(payload: &[u8], start: usize) -> Result<u32, Error> {
    let field = payload
        .get(start..start + U32_LEN)
        .ok_or(Error::Truncated)?;
    Ok(u32::from_le_bytes(
        field
            .try_into()
            .expect("checked to be exactly U32_LEN bytes"),
    ))
}

/// Splits `payload` into its declared output length, token count, and the
/// remaining range-coded bytes.
///
/// # Errors
///
/// Returns [`Error::Truncated`] if `payload` is shorter than the
/// [`TOKEN_HEADER_LEN`]-byte header.
fn read_header(payload: &[u8]) -> Result<(usize, u32, &[u8]), Error> {
    let declared_len = read_u32_le(payload, 0)?;
    let token_count = read_u32_le(payload, U32_LEN)?;
    Ok((
        declared_len as usize,
        token_count,
        &payload[TOKEN_HEADER_LEN..],
    ))
}

/// Rejects a declared output length past `max_len`, before any allocation
/// or decode work: shared by [`decode`] and [`decode_undoable_streaming`],
/// both of which read it straight out of [`read_header`].
fn ensure_within_max_len(declared_len: usize, max_len: u32) -> Result<(), Error> {
    if declared_len > max_len as usize {
        // declared_len was cast up from the header's u32 field (read_header),
        // so casting back down here is always exact.
        Err(Error::TooLarge {
            len: u32::try_from(declared_len).expect(
                "declared_len came from a u32 header field, so it always fits back into one",
            ),
            max: max_len,
        })
    } else {
        Ok(())
    }
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

/// Rejects a match distance past [`lz::WINDOW`]. `decode_bucketed` can
/// return distances up to `2 * WINDOW - 1` (`OFFSET_BUCKETS`'s docs, bucket
/// 20's residual bits), but the encoder's match finder never searches past
/// `WINDOW`: a wider distance is adversarial, and rejecting it here, before
/// it ever reaches `RepCache`, keeps every cached rep distance within
/// `WINDOW` too, so this is the only place that needs the check.
fn ensure_within_window(distance: NonZeroU32) -> Result<(), Error> {
    if distance.get() as usize > lz::WINDOW {
        Err(Error::Corrupt)
    } else {
        Ok(())
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

/// Where a decoded token's literal byte and match/rep copy land: real
/// output for [`decode`]'s [`VecSink`], undone-and-written-immediately
/// output for [`decode_undoable_streaming`]'s `StreamingSink`. The two
/// walks must decode the same flag/length/offset/slot symbols, in the same
/// order, off the same token stream, or one silently drifts from what the
/// other produces; routing both through [`decode_tokens`] makes that a
/// single piece of code instead of two loops kept in sync by hand, mirroring
/// [`TokenSink`]/[`walk_tokens`] on the encode side.
trait DecodeSink {
    /// Widens with every accepted output byte, checked against
    /// `declared_len` before decoding one more.
    type Err: From<Error>;

    /// Bytes produced so far, for [`ensure_room`]'s bound.
    fn len(&self) -> usize;

    /// Decodes and applies one literal byte at `context`.
    fn literal(
        &mut self,
        models: &mut Models,
        ac: &mut Decoder,
        context: Context,
    ) -> Result<u8, Self::Err>;

    /// Applies an already-decoded `len`-byte copy from `distance` bytes
    /// back, returning the context after it.
    fn copy(
        &mut self,
        len: u32,
        distance: NonZeroU32,
        context: Context,
    ) -> Result<Context, Self::Err>;
}

/// Decodes a copy token's length: the bucket symbol through
/// [`Models::length_match`]/`length_rep` (selected by `kind`), the residual
/// bits below it through the [`ResidualTree`] beside that model. Mirrors
/// [`EncodeSink::length`].
///
/// # Panics
///
/// Panics if `kind` is [`FlagKind::Literal`]: both of [`decode_tokens`]'s
/// call sites already have a [`FlagKind::Match`] or [`FlagKind::Rep`] in
/// hand.
fn decode_length(models: &mut Models, ac: &mut Decoder, kind: FlagKind) -> u32 {
    let b = split_length_model(models, kind).decode(ac);
    split_length_residual(models, kind).decode(ac, b)
}

/// Decodes a match's distance symbol through whichever of
/// [`Models::offset_len`]'s [`OFFSET_LEN_STATES`] models [`offset_len_state`]
/// of the already-decoded `len` selects. Mirrors [`EncodeSink::offset`].
fn decode_offset(models: &mut Models, ac: &mut Decoder, len: u32) -> u32 {
    decode_bucketed(&mut models.offset_len[offset_len_state(len)], ac)
}

/// The shared skeleton behind [`decode`] and [`decode_undoable_streaming`]:
/// decodes `token_count` tokens off `ac` in coding order, routing every
/// literal byte and copy through `sink`, and advancing the literal-model
/// context and `reps` exactly as [`decode`] and [`decode_undoable_streaming`]
/// both require. See [`DecodeSink`]'s docs for why this exists.
fn decode_tokens<S: DecodeSink>(
    token_count: u32,
    declared_len: usize,
    models: &mut Models,
    ac: &mut Decoder,
    reps: &mut RepCache,
    sink: &mut S,
) -> Result<(), S::Err> {
    let mut context = Context::default();
    for _ in 0..token_count {
        let flag_table = usize::from(context.after_copy);
        match FlagKind::from_index(models.flag[flag_table].decode(ac)) {
            FlagKind::Literal => {
                ensure_room(sink.len(), 1, declared_len)?;
                let byte = sink.literal(models, ac, context)?;
                context = context.after_literal(byte);
            }
            FlagKind::Match => {
                let len = decode_length(models, ac, FlagKind::Match);
                let distance = decode_offset(models, ac, len);
                // decode_bucketed always ORs in `1 << bits`, which is >= 1
                // regardless of the residual bits: never zero.
                let distance =
                    NonZeroU32::new(distance).expect("decode_bucketed's result is always >= 1");
                ensure_within_window(distance)?;
                ensure_room(sink.len(), len as usize, declared_len)?;
                context = sink.copy(len, distance, context)?;
                reps.push_front(distance);
            }
            FlagKind::Rep => {
                // RepSlot::from_index documents why models.slot's decode
                // is safe to feed it directly.
                let slot = RepSlot::from_index(models.slot.decode(ac));
                let len = decode_length(models, ac, FlagKind::Rep);
                let distance = reps.get(slot);
                ensure_room(sink.len(), len as usize, declared_len)?;
                context = sink.copy(len, distance, context)?;
                reps.promote(slot);
            }
        }
    }
    Ok(())
}

/// [`DecodeSink`] for [`decode`]'s whole-buffer path: a literal byte is
/// pushed onto `output`, and a copy replays [`copy_checked`] over what
/// `output` already holds. `column` mirrors [`encode_tokens`]'s
/// `ColumnCoding`, but owned here rather than borrowed (there is no
/// per-candidate trial to share it across): `Some` exactly when this
/// frame's candidate is [`Candidate::Transpose`], selecting the
/// column-keyed literal expert over [`crate::literal::Literal::decode_logit_sse`].
struct VecSink<'a> {
    output: &'a mut Vec<u8>,
    declared_len: usize,
    column: Option<(NonZeroUsize, &'a mut ColumnExpertState)>,
}

impl DecodeSink for VecSink<'_> {
    type Err = Error;

    fn len(&self) -> usize {
        self.output.len()
    }

    fn literal(
        &mut self,
        models: &mut Models,
        ac: &mut Decoder,
        context: Context,
    ) -> Result<u8, Error> {
        let byte = match &mut self.column {
            Some((columns, state)) => {
                let bank = column::bank_of(
                    context.position,
                    *columns,
                    self.declared_len,
                    MAX_COLUMN_BANKS,
                );
                models.literal.decode_column(ac, context, bank, state)
            }
            None => models
                .literal
                .decode_logit_sse(ac, context, &mut models.logit_sse),
        };
        self.output.push(byte);
        Ok(byte)
    }

    fn copy(
        &mut self,
        len: u32,
        distance: NonZeroU32,
        mut context: Context,
    ) -> Result<Context, Error> {
        copy_checked(self.output, len, distance)?;
        context = context.after_copy(&self.output[self.output.len() - len as usize..]);
        Ok(context)
    }
}

/// Decodes a payload produced by [`encode`] back into the original bytes.
///
/// Bounds decode work and allocation to the frame's declared output size,
/// itself bounded by `max_len` (`docs/format/SPEC.md`,
/// `rust-craft` skill's allocation-discipline): `output` is reserved
/// fallibly and exactly once, up front, to `declared_len` (`try_reserve_exact`,
/// returning [`Error::OutOfMemory`] on a real allocator failure instead of
/// aborting), and every token is rejected the moment it would grow `output`
/// past that length — so a payload lying about either the declared length or
/// the token count cannot make this function do more work, or allocate more
/// memory, than the length it declared allows, and `max_len` bounds what it
/// is allowed to declare in the first place, itself
/// clamped to [`MAX_DECODED_LEN`] regardless of what is passed in: that
/// constant is the only ceiling this decoder's worst-case decode time has
/// been measured against (see its docs). [`crate::decompress_bounded`]
/// clamps before calling this function too, so the clamp here is a second,
/// redundant guarantee for any other caller reaching this `#[doc(hidden)]`
/// but still `pub` function directly. See [`MAX_DECODED_LEN`]'s docs for why
/// a fixed ceiling, not a ratio check against the payload's own size, is the
/// sound bound here.
///
/// # Errors
///
/// Returns [`Error::Truncated`] if `payload` is shorter than the 2-byte
/// filter selector plus the 8-byte declared-length/token-count header.
/// Returns [`Error::Corrupt`] if the filter selector is not one
/// [`Candidate::from_header_bytes`] recognizes. Returns
/// [`Error::TooLarge`] if the declared length exceeds `max_len`, checked
/// before any allocation or decode work.
/// Returns [`Error::OutOfMemory`] if the allocator cannot satisfy `output`'s
/// capacity reservation, one of the model's fixed-size tables, or (for
/// [`Candidate::Delta`]/[`Candidate::Bcj`]/[`Candidate::Transpose`]) the
/// filter's undo buffer.
/// Returns [`Error::Corrupt`] if a match token's distance exceeds
/// [`lz::WINDOW`] (the real encoder's match finder never searches past it,
/// `research/JOURNAL.md` M4's bounded-memory decode guarantee: a distance
/// beyond `WINDOW` is never legitimate and, left unrejected, would force
/// this decoder to retain output far past what any real bitstream needs),
/// if a match or rep token's distance reaches before the start of decoded
/// output, if a token would grow decoded output past the declared length,
/// or if the final decoded length does not equal it: all adversarial or
/// malformed input, never a bug in this decoder (`rust-craft` skill,
/// panic-discipline).
///
/// `payload` is a `Method::Lz` payload of the current `FORMAT_VERSION`:
/// [`crate::decompress`] rejects any other version before calling this.
/// A [`Candidate::Transpose`] frame decodes its literals through
/// [`crate::literal::Literal::decode_column`], blending a column-keyed
/// seventh expert into the mix; every other candidate through
/// [`crate::literal::Literal::decode_logit_sse`] (see the module docs'
/// "Payload layout" section).
///
/// # Panics
///
/// Does not panic on adversarial `payload`. Two internal `.expect()`s
/// guard invariants of this function's own math, never a property of
/// `payload`: turning a decoded match distance into a [`NonZeroU32`]
/// (a mathematical invariant of `decode_bucketed`, see that function's
/// docs), and casting a declared length already found `<= max_len` (a
/// `u32`) back down from the `usize` `read_header` widened it to.
pub fn decode(payload: &[u8], max_len: u32) -> Result<Vec<u8>, Error> {
    let max_len = max_len.min(MAX_DECODED_LEN);
    let (filter_bytes, payload) = payload.split_at_checked(2).ok_or(Error::Truncated)?;
    let candidate =
        Candidate::from_header_bytes([filter_bytes[0], filter_bytes[1]]).ok_or(Error::Corrupt)?;
    let (declared_len, token_count, ac_bytes) = read_header(payload)?;
    ensure_within_max_len(declared_len, max_len)?;

    let mut ac = Decoder::new(ac_bytes);
    let mut models = Models::try_new()?;
    let mut reps = RepCache::initial();
    // Some exactly when this frame's candidate is Candidate::Transpose:
    // mirrors encode_tokens's ColumnCoding, but `state` is owned here
    // (there is no per-candidate trial to share it across).
    let mut column_state: Option<(NonZeroUsize, ColumnExpertState)> = match candidate {
        Candidate::Transpose(columns) => {
            Some((columns, ColumnExpertState::try_new(MAX_COLUMN_BANKS)?))
        }
        _ => None,
    };
    // Reserved fallibly and exactly up front, not left to grow through
    // `push`/`extend`'s doubling: `declared_len` is already bounded above,
    // so nothing past this point ever asks `output` to grow past what it
    // holds here, and `try_reserve_exact` turns a real allocator failure
    // into `Error::OutOfMemory` instead of the process aborting through
    // `push`'s infallible growth path (hard rule 2, torture-swept by
    // `tests/torture.rs`, #453).
    let mut output: Vec<u8> = Vec::new();
    output.try_reserve_exact(declared_len)?;

    let mut sink = VecSink {
        output: &mut output,
        declared_len,
        column: column_state
            .as_mut()
            .map(|(columns, state)| (*columns, state)),
    };
    decode_tokens(
        token_count,
        declared_len,
        &mut models,
        &mut ac,
        &mut reps,
        &mut sink,
    )?;

    if output.len() != declared_len {
        return Err(Error::Corrupt);
    }
    undo_filter(candidate, output)
}

/// Streaming counterpart to [`decode`], reached only through
/// [`crate::decompress_to_writer`].
///
/// [`filters::select::Candidate::Identity`]'s undo step (`undo_filter`) is
/// the identity transform, [`filters::select::Candidate::Delta`]'s is a
/// small fixed-lookback accumulate ([`filters::delta::Undo`]), and
/// [`filters::select::Candidate::Bcj`]'s is a small fixed-lookahead rewrite
/// ([`filters::bcj::Undo`]): all three undo the LZ token stream one filtered
/// byte at a time as it is produced, so [`decode_undoable_streaming`] takes
/// that path for any of them, bounding resident memory to [`lz::WINDOW`] via
/// [`lz::Window`] regardless of `declared_len` (`research/JOURNAL.md`
/// S1-P7/S2-D5/S2-A74, ROADMAP M4's bounded-memory decode guarantee).
/// `Transpose` needs its filter undone over the complete buffer regardless
/// of how the LZ loop itself is decoded (`research/JOURNAL.md` S2-D4), so
/// this function falls back to [`decode`]'s whole-buffer path for it,
/// unchanged from what a caller who ignored streaming entirely would get.
///
/// # Errors
///
/// [`crate::WriteError::Decode`] for the same errors [`decode`] itself
/// would return; [`crate::WriteError::Io`] for whatever `writer`'s own
/// `write_all` returns. Neither boxes its payload through
/// `std::io::Error::other` (issue #479): both are plain enum
/// construction.
pub(crate) fn decode_to_writer<W: std::io::Write>(
    payload: &[u8],
    max_len: u32,
    writer: &mut W,
) -> Result<(), crate::WriteError> {
    let (filter_bytes, filtered_payload) = payload.split_at_checked(2).ok_or(Error::Truncated)?;
    let candidate =
        Candidate::from_header_bytes([filter_bytes[0], filter_bytes[1]]).ok_or(Error::Corrupt)?;
    match candidate {
        Candidate::Identity => {
            decode_undoable_streaming(filtered_payload, max_len, writer, &mut StreamUndo::Identity)
        }
        Candidate::Delta(stride) => {
            let undo = filters::delta::Undo::try_new(stride).map_err(Error::from)?;
            decode_undoable_streaming(
                filtered_payload,
                max_len,
                writer,
                &mut StreamUndo::Delta(undo),
            )
        }
        Candidate::Bcj => {
            let undo = filters::bcj::Undo::try_new().map_err(Error::from)?;
            decode_undoable_streaming(
                filtered_payload,
                max_len,
                writer,
                &mut StreamUndo::Bcj(undo),
            )
        }
        Candidate::Transpose(_) => {
            let decoded = decode(payload, max_len)?;
            writer.write_all(&decoded)?;
            Ok(())
        }
    }
}

/// Per-byte filter undo for [`decode_undoable_streaming`], one variant per
/// [`filters::select::Candidate`] this function's caller streams.
enum StreamUndo {
    /// `undo_filter` is the identity transform: pass the byte through.
    Identity,
    /// `undo_filter` is [`filters::delta::decode`]; undone one byte at a
    /// time by [`filters::delta::Undo`].
    Delta(filters::delta::Undo),
    /// `undo_filter` is [`filters::bcj::decode`]; undone one filtered byte
    /// at a time by [`filters::bcj::Undo`], which may buffer a few bytes
    /// before a call resolves any output (see its own docs).
    Bcj(filters::bcj::Undo),
}

impl StreamUndo {
    /// Undoes one more filtered byte, in stream order, writing whatever
    /// bytes that resolves to `writer` immediately (zero for
    /// [`Self::Bcj`] while it is still buffering a candidate instruction,
    /// exactly one otherwise).
    fn apply<W: std::io::Write>(
        &mut self,
        filtered_byte: u8,
        writer: &mut W,
    ) -> std::io::Result<()> {
        match self {
            Self::Identity => writer.write_all(std::slice::from_ref(&filtered_byte)),
            Self::Delta(undo) => {
                let raw = undo.apply(filtered_byte);
                writer.write_all(std::slice::from_ref(&raw))
            }
            Self::Bcj(undo) => writer.write_all(undo.apply(filtered_byte).as_slice()),
        }
    }

    /// Flushes any bytes an undo step is still buffering at end of stream.
    /// A no-op for [`Self::Identity`]/[`Self::Delta`], which never buffer;
    /// [`Self::Bcj`] may hold up to `INSTRUCTION_LEN - 1` unresolved bytes
    /// ([`filters::bcj::Undo::finish`]).
    fn finish<W: std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        if let Self::Bcj(undo) = self {
            writer.write_all(undo.finish().as_slice())?;
        }
        Ok(())
    }
}

/// [`DecodeSink`] for [`decode_undoable_streaming`]'s path: a literal byte
/// (and a copy's replayed bytes) land in `window`, then are undone through
/// `undo` and written to `writer` immediately. Never carries column-expert
/// state: [`decode_to_writer`] never builds this sink for
/// [`Candidate::Transpose`], which falls back to [`decode`]'s whole-buffer
/// path instead (see that function's docs).
struct StreamingSink<'a, W: std::io::Write> {
    window: &'a mut lz::Window,
    undo: &'a mut StreamUndo,
    writer: &'a mut W,
}

impl<W: std::io::Write> DecodeSink for StreamingSink<'_, W> {
    type Err = crate::WriteError;

    fn len(&self) -> usize {
        self.window.written_len()
    }

    fn literal(
        &mut self,
        models: &mut Models,
        ac: &mut Decoder,
        context: Context,
    ) -> Result<u8, crate::WriteError> {
        let byte = models
            .literal
            .decode_logit_sse(ac, context, &mut models.logit_sse);
        self.window.push(byte);
        self.undo.apply(byte, self.writer)?;
        Ok(byte)
    }

    fn copy(
        &mut self,
        len: u32,
        distance: NonZeroU32,
        context: Context,
    ) -> Result<Context, crate::WriteError> {
        copy_streamed(self.window, self.undo, self.writer, len, distance, context)
    }
}

/// [`decode_to_writer`]'s streaming path for candidates whose `undo_filter`
/// step can run one byte at a time ([`StreamUndo`]): the same token loop as
/// [`decode`], replaying matches and reps through a fixed-capacity
/// [`lz::Window`] instead of a fully-resident `Vec<u8>`, undoing each byte
/// through `undo` and writing the result to `writer` the instant it is
/// produced. `window` holds the *filtered* byte stream throughout — matches
/// and reps reference positions in what the encoder's LZ pass actually saw,
/// which is filtered data (`apply_filter` runs before `encode_tokens`) — so
/// only the byte handed to `writer` differs per candidate, never what goes
/// into `window` or `context`. `payload` here has already had its 2-byte
/// filter selector stripped by [`decode_to_writer`], matching
/// [`read_header`]'s expected input.
fn decode_undoable_streaming<W: std::io::Write>(
    payload: &[u8],
    max_len: u32,
    writer: &mut W,
    undo: &mut StreamUndo,
) -> Result<(), crate::WriteError> {
    let max_len = max_len.min(MAX_DECODED_LEN);
    let (declared_len, token_count, ac_bytes) = read_header(payload)?;
    ensure_within_max_len(declared_len, max_len)?;

    let mut ac = Decoder::new(ac_bytes);
    let mut models = Models::try_new().map_err(Error::from)?;
    let mut reps = RepCache::initial();
    let mut window = lz::Window::try_new().map_err(Error::from)?;

    let mut sink = StreamingSink {
        window: &mut window,
        undo: &mut *undo,
        writer: &mut *writer,
    };
    decode_tokens(
        token_count,
        declared_len,
        &mut models,
        &mut ac,
        &mut reps,
        &mut sink,
    )?;

    undo.finish(writer)?;

    if window.written_len() != declared_len {
        return Err(Error::Corrupt.into());
    }
    Ok(())
}

/// Replays a match/rep copy of `len` bytes from `distance` bytes back
/// through `window` (the filtered stream), undoing each byte through `undo`
/// and writing the result to `writer` as it is produced, returning the
/// context updated the same way [`decode`]'s batch
/// `context.after_copy(&output[start..])` does — `context` folds over the
/// *filtered* bytes `window` holds, never `undo`'s output, matching
/// [`decode_undoable_streaming`]'s own split. Splitting the run into
/// per-byte [`Context::after_copy`] calls instead of one batch call is
/// exactly equivalent: each call folds `word_hash` over its slice and sets
/// `prev1`/`prev2` from its last (up to) two bytes, so chaining single-byte
/// calls reproduces the same final state a single multi-byte call would —
/// `window`'s ring buffer never holds the whole run contiguously when it
/// wraps, so a batch call is not an option here the way it is in [`decode`].
///
/// Mirrors [`copy_checked`]'s distance-before-the-start rejection: even a
/// zero-length copy validates `distance` against what has actually been
/// written so far.
fn copy_streamed<W: std::io::Write>(
    window: &mut lz::Window,
    undo: &mut StreamUndo,
    writer: &mut W,
    len: u32,
    distance: NonZeroU32,
    mut context: Context,
) -> Result<Context, crate::WriteError> {
    let distance = distance.get() as usize;
    window
        .written_len()
        .checked_sub(distance)
        .ok_or(Error::Corrupt)?;
    if len == 0 {
        return Ok(context.after_copy(&[]));
    }
    for _ in 0..len {
        let byte = window.get(distance);
        window.push(byte);
        undo.apply(byte, writer)?;
        context = context.after_copy(std::slice::from_ref(&byte));
    }
    Ok(context)
}

#[cfg(test)]
mod tests;
