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
//! `FORMAT_VERSION` 2 named this same outer layout but coded its literal
//! sub-stream through a direct 256-way range division instead of the
//! SSE-calibrated coding below; no release ever wrote it, so it was
//! retired outright rather than kept forever
//! (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`), and
//! `LZ_MIN_VERSION` moved from 2 to 3 with it.
//!
//! The outer layout above is unchanged across every version this build
//! decodes (`LZ_MIN_VERSION` and up). Versions 3 and 4 code each literal
//! byte as 8 SSE-calibrated binary decisions
//! ([`crate::literal::Literal::encode_sse`]/`decode_sse`,
//! `docs/adr/0038-wire-sse-into-the-literal-mixer.md`, `research/JOURNAL.md`
//! S1-P1), except a version-4 frame whose filter selector names
//! [`Candidate::Transpose`], which goes one step further still: each
//! literal byte blends a column-keyed seventh expert into the mix before
//! the same SSE-calibrated coding
//! ([`crate::literal::Literal::encode_column`]/`decode_column`,
//! `docs/adr/0046-wire-the-column-expert-into-the-literal-mixer.md`,
//! `research/JOURNAL.md` S1-P5). Version `LOGISTIC_MIN_VERSION` (5) and
//! above codes every other candidate's literals through a second mixer
//! instead, blending the same six experts in the logit domain under its
//! own annealed-rate weights and its own SSE table
//! ([`crate::literal::Literal::encode_logistic`]/`decode_logistic`,
//! `docs/adr/0052-wire-the-logistic-mixer-into-the-literal-model.md`,
//! `research/JOURNAL.md` S1-P8); a version-5 `Candidate::Transpose` frame
//! still codes through `encode_column`/`decode_column` exactly as version 4
//! does — the logit-domain mix does not yet reach the seventh expert.
//! Version `SURPRISE_MIN_VERSION` (6) and above codes every other
//! candidate's literals through the same logit-domain mix again, but under
//! a different per-key rate schedule: a learned baseline instead of an
//! annealed step count
//! ([`crate::literal::Literal::encode_logistic_surprise`]/`decode_logistic_surprise`,
//! `docs/adr/0054-wire-the-surprise-rate-schedule-into-the-literal-model.md`,
//! `research/JOURNAL.md` S2-A104/S2-A105); a version-6 `Candidate::Transpose`
//! frame is unaffected, same carve-out as version 5. Version
//! `LOGIT_SSE_MIN_VERSION` (7) and above codes every other candidate's
//! literals through the same logit-domain mix and rate schedule again, but
//! calibrated through a stretch-domain SSE table instead of a linear one
//! ([`crate::literal::Literal::encode_logit_sse`]/`decode_logit_sse`,
//! `docs/adr/0055-wire-logit-domain-sse-bins-into-the-literal-model.md`,
//! `research/JOURNAL.md` S2-A106/S2-A108); a version-7 `Candidate::Transpose`
//! frame is unaffected, same carve-out as versions 5 and 6. [`decode`]
//! takes the frame's declared `version` and its already-parsed `candidate`
//! and picks the matching literal path. The `length` symbol (a
//! [`Token::Match`] or [`Token::Rep`]'s copy length) is coded identically
//! regardless of candidate at every version, but is itself version-gated
//! starting at `LENGTH_SPLIT_MIN_VERSION` (8): below it, every length
//! shares one [`Model`] regardless of which token kind produced it; at or
//! above it, a match's length and a rep's length code through two
//! independent models instead
//! (`docs/adr/0057-wire-the-match-rep-length-model-split.md`,
//! `research/JOURNAL.md` S2-A109/S2-A110). `offset` and `slot` are
//! unaffected and coded identically at every version `LZ_MIN_VERSION` or
//! above, regardless of candidate.
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
    ColumnExpertState, Context, Literal, LogisticMix, PpmExpertState, SurpriseLogisticMix,
    SurpriseLogisticMixLogitSse,
};
use crate::lz::{self, RepCache, RepSlot, Token};
use crate::model::Model;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload this build can
/// decode: see the module docs' "Payload layout" section for the layout
/// change that moved this from 1 to 2, and for version 2's own retirement
/// (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`) that moved
/// it from 2 to 3. Every version this constant admits codes its literal
/// sub-stream through [`crate::literal::Literal::encode_sse`]/`decode_sse`
/// below `LOGISTIC_MIN_VERSION`, `encode_logistic`/`decode_logistic` at
/// `LOGISTIC_MIN_VERSION` and above but below `SURPRISE_MIN_VERSION`,
/// `encode_logistic_surprise`/`decode_logistic_surprise` at
/// `SURPRISE_MIN_VERSION` and above but below `LOGIT_SSE_MIN_VERSION`,
/// `encode_logit_sse`/`decode_logit_sse` at `LOGIT_SSE_MIN_VERSION` and
/// above (or, at version 4 on a `Candidate::Transpose` frame,
/// [`crate::literal::Literal::encode_column`]/`decode_column` regardless of
/// any of those three gates); no version this build decodes still needs a
/// separate literal-coding floor.
pub(crate) const LZ_MIN_VERSION: u8 = 3;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload codes a
/// [`Candidate::Transpose`] frame's literal sub-stream through
/// [`crate::literal::Literal::encode_column`]/`decode_column` (a seventh,
/// column-keyed expert blended into the mix, `research/JOURNAL.md` S1-P5,
/// `docs/adr/0046-wire-the-column-expert-into-the-literal-mixer.md`)
/// instead of [`crate::literal::Literal::encode_sse`]/`decode_sse`. Every
/// other candidate's literal sub-stream, and every candidate at a lower
/// version, is unaffected — see the module docs' "Payload layout" section.
const COLUMN_EXPERT_MIN_VERSION: u8 = 4;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload codes a literal
/// sub-stream through [`crate::literal::Literal::encode_logistic`]/
/// `decode_logistic` (a logit-domain mix over the six real experts,
/// `research/JOURNAL.md` S1-P8, S2-A101,
/// `docs/adr/0052-wire-the-logistic-mixer-into-the-literal-model.md`)
/// instead of [`crate::literal::Literal::encode_sse`]/`decode_sse`. Applies
/// to every candidate except [`Candidate::Transpose`] at
/// `COLUMN_EXPERT_MIN_VERSION` and above, which keeps coding through
/// [`crate::literal::Literal::encode_column`]/`decode_column` regardless of
/// this constant — the logit-domain mix does not yet reach the seventh,
/// column-keyed expert (a separate lead, not this one's scope). Every
/// candidate at a lower version is unaffected — see the module docs'
/// "Payload layout" section.
const LOGISTIC_MIN_VERSION: u8 = 5;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload codes a literal
/// sub-stream through [`crate::literal::Literal::encode_logistic_surprise`]/
/// `decode_logistic_surprise` (the same logit-domain mix as
/// `LOGISTIC_MIN_VERSION`, but [`crate::literal::SurpriseLogisticMix`]'s
/// learned-baseline rate schedule in place of [`crate::literal::LogisticMix`]'s
/// step-count-derived one, `research/JOURNAL.md` S2-A104/S2-A105,
/// `docs/adr/0054-wire-the-surprise-rate-schedule-into-the-literal-model.md`)
/// instead of [`crate::literal::Literal::encode_logistic`]/`decode_logistic`.
/// Applies to every candidate except [`Candidate::Transpose`] at
/// `COLUMN_EXPERT_MIN_VERSION` and above, which keeps coding through
/// [`crate::literal::Literal::encode_column`]/`decode_column` regardless of
/// this constant, same carve-out as `LOGISTIC_MIN_VERSION`'s own docs give.
/// Every candidate at a lower version is unaffected — see the module docs'
/// "Payload layout" section.
const SURPRISE_MIN_VERSION: u8 = 6;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload codes a literal
/// sub-stream through [`crate::literal::Literal::encode_logit_sse`]/
/// `decode_logit_sse` (the same logit-domain mix and learned-baseline rate
/// schedule as `SURPRISE_MIN_VERSION`, but
/// [`crate::literal::SurpriseLogisticMixLogitSse`]'s stretch-domain SSE
/// bin spacing in place of [`crate::literal::SurpriseLogisticMix`]'s
/// linear one, `research/JOURNAL.md` S2-A106/S2-A108,
/// `docs/adr/0055-wire-logit-domain-sse-bins-into-the-literal-model.md`)
/// instead of [`crate::literal::Literal::encode_logistic_surprise`]/
/// `decode_logistic_surprise`. Applies to every candidate except
/// [`Candidate::Transpose`] at `COLUMN_EXPERT_MIN_VERSION` and above, which
/// keeps coding through [`crate::literal::Literal::encode_column`]/
/// `decode_column` regardless of this constant, same carve-out as
/// `SURPRISE_MIN_VERSION`'s own docs give. Every candidate at a lower
/// version is unaffected — see the module docs' "Payload layout" section.
const LOGIT_SSE_MIN_VERSION: u8 = 7;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload codes a
/// [`Token::Match`]'s length and a [`Token::Rep`]'s length through two
/// independent [`Model`]s (`Models::length_match`/`length_rep`) instead of
/// one shared [`Model`] (`Models::length`) regardless of which kind
/// produced it (`research/JOURNAL.md` S2-A109/S2-A110,
/// `docs/adr/0057-wire-the-match-rep-length-model-split.md`). Unlike every
/// `*_MIN_VERSION` constant above, this gate is not candidate-dependent:
/// it applies to every [`Candidate`], `Candidate::Transpose` included,
/// since the length symbol sits outside the literal sub-stream those
/// constants gate. Every candidate at a lower version is unaffected.
const LENGTH_SPLIT_MIN_VERSION: u8 = 8;

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
/// here. ADR-0052's `decode_logistic` is now the worst-case literal path at
/// `LOGISTIC_MIN_VERSION` and above (a bounded constant more per node than
/// `decode_sse`'s own bound above: six `stretch` calls and a dot product
/// against `expert_prefix_sums` instead of one `mix` lookup, still linear
/// in `declared_len`, no new loop or allocation); not separately
/// remeasured, the same provisional-ceiling argument covers it. ADR-0054's
/// `decode_logistic_surprise` replaces `decode_logistic`'s per-node step
/// count with two EMA updates at `SURPRISE_MIN_VERSION` and above, still a
/// bounded constant more, no new loop or allocation; the same argument
/// covers it too. ADR-0055's `decode_logit_sse` replaces `decode_logistic_
/// surprise`'s linear-domain `Sse` lookup with `LogitSse`'s own (one extra
/// `stretch` call per node) at `LOGIT_SSE_MIN_VERSION` and above, still a
/// bounded constant more, no new loop or allocation; the same argument
/// covers it too. `LENGTH_SPLIT_MIN_VERSION`'s length-model split touches a
/// different symbol (the length coded with every `Token::Match`/`Token::Rep`,
/// not the per-byte literal loop this argument is about) and replaces one
/// `Model::decode` call on `models.length` with the same call on whichever
/// of `models.length_match`/`length_rep` a cheap `FlagKind` branch selects:
/// no new loop or allocation, and strictly cheaper per token than the
/// per-byte literal cost this bound is already measured against.
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
    /// The logit-domain mixer [`crate::literal::Literal::encode_logistic`]/
    /// `decode_logistic` code every non-`Candidate::Transpose` literal
    /// through at `LOGISTIC_MIN_VERSION` and above, below `SURPRISE_MIN_VERSION`:
    /// its own weights, step counts and `Sse` table, one per frame/trial,
    /// the same lifetime as `literal`'s six real banks.
    logistic: LogisticMix,
    /// [`crate::literal::Literal::encode_logistic_surprise`]/
    /// `decode_logistic_surprise` code every non-`Candidate::Transpose`
    /// literal through at `SURPRISE_MIN_VERSION` and above, below
    /// `LOGIT_SSE_MIN_VERSION`: its own weights, error EMAs and `Sse`
    /// table, independent of `logistic`'s own, same lifetime.
    surprise: SurpriseLogisticMix,
    /// [`crate::literal::Literal::encode_logit_sse`]/`decode_logit_sse`
    /// code every non-`Candidate::Transpose` literal through at
    /// `LOGIT_SSE_MIN_VERSION` and above: its own weights, error EMAs and
    /// `LogitSse` table, independent of `surprise`'s own, same lifetime.
    logit_sse: SurpriseLogisticMixLogitSse,
    /// One flag table per "was the previous token a copy" state (the
    /// archive's `flag[2]`): a literal run and a post-copy position have
    /// different flag distributions worth modeling separately. Indexed by
    /// [`Context::after_copy`].
    flag: [Model; 2],
    /// Shared copy-length [`Model`], regardless of [`Token::Match`]/
    /// [`Token::Rep`]: the real coding path at versions below
    /// `LENGTH_SPLIT_MIN_VERSION`, kept only for decoding those older
    /// frames (`length_match`/`length_rep` below replace it at
    /// `LENGTH_SPLIT_MIN_VERSION` and above, same role `literal` keeps for
    /// `decode_sse` below `LOGISTIC_MIN_VERSION`).
    length: Model,
    /// [`Token::Match`]'s own copy-length [`Model`], independent of
    /// `length_rep`: the real coding path at `LENGTH_SPLIT_MIN_VERSION`
    /// and above (`research/JOURNAL.md` S2-A109/S2-A110,
    /// `docs/adr/0057-wire-the-match-rep-length-model-split.md`).
    length_match: Model,
    /// [`Token::Rep`]'s own copy-length [`Model`], independent of
    /// `length_match`: the real coding path at `LENGTH_SPLIT_MIN_VERSION`
    /// and above, same lifetime and gate as `length_match`.
    length_rep: Model,
    offset: Model,
    slot: Model,
}

impl Models {
    fn new() -> Self {
        Self {
            literal: Literal::new(),
            logistic: LogisticMix::new(),
            surprise: SurpriseLogisticMix::new(),
            logit_sse: SurpriseLogisticMixLogitSse::new(),
            flag: [Model::new(FLAG_ALPHABET), Model::new(FLAG_ALPHABET)],
            length: Model::new(lz::LENGTH_BUCKETS),
            length_match: Model::new(lz::LENGTH_BUCKETS),
            length_rep: Model::new(lz::LENGTH_BUCKETS),
            offset: Model::new(lz::OFFSET_BUCKETS),
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
            logistic: LogisticMix::try_new()?,
            surprise: SurpriseLogisticMix::try_new()?,
            logit_sse: SurpriseLogisticMixLogitSse::try_new()?,
            flag: [
                Model::try_new(FLAG_ALPHABET)?,
                Model::try_new(FLAG_ALPHABET)?,
            ],
            length: Model::try_new(lz::LENGTH_BUCKETS)?,
            length_match: Model::try_new(lz::LENGTH_BUCKETS)?,
            length_rep: Model::try_new(lz::LENGTH_BUCKETS)?,
            offset: Model::try_new(lz::OFFSET_BUCKETS)?,
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

/// Picks [`Models::length_match`]/`length_rep` by `kind`, the
/// `LENGTH_SPLIT_MIN_VERSION` split every real-path [`TokenSink`] (and the
/// `ideal_cost_bits` pricer that must price what they code) selects a copy
/// token's length model through.
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

/// `-log2(p)` cost of a flag symbol under the shared `models.flag` tables:
/// every pricing-only [`TokenSink`] ([`CostSink`], [`PairedTokenSink`],
/// [`OffsetLenSplitSink`]) must price `flag` this same way, since none of
/// them vary its coding.
fn price_flag(models: &mut Models, flag_table: usize, kind: FlagKind) -> f64 {
    models.flag[flag_table].ideal_cost_bits(kind.index())
}

/// `-log2(p)` cost of a literal byte under the shared, no-column-expert
/// literal path: [`CostSink`]'s and [`OffsetLenSplitSink`]'s shared
/// `literal` pricing (neither trials [`Candidate::Transpose`]; see each
/// sink's own docs).
fn price_literal(models: &mut Models, context: Context, byte: u8) -> f64 {
    models
        .literal
        .ideal_cost_bits_logistic_surprise_logit_sse(context, byte, &mut models.logit_sse)
}

/// `-log2(p)` cost of a copy token's length symbol, through whichever of
/// [`Models::length_match`]/`length_rep` [`split_length_model`] selects:
/// [`CostSink`]'s, [`PairedTokenSink`]'s, and [`OffsetLenSplitSink`]'s
/// shared `length` pricing.
fn price_length(models: &mut Models, kind: FlagKind, value: u32) -> f64 {
    ideal_cost_bucketed(split_length_model(models, kind), value)
}

/// `-log2(p)` cost of a match's distance under the shared, unconditioned
/// `models.offset`: [`CostSink`]'s and [`PairedTokenSink`]'s shared
/// `offset` pricing, and [`OffsetLenSplitSink`]'s baseline half.
fn price_offset(models: &mut Models, value: u32) -> f64 {
    ideal_cost_bucketed(&mut models.offset, value)
}

/// `-log2(p)` cost of a rep-slot symbol under the shared `models.slot`:
/// every pricing-only [`TokenSink`] ([`CostSink`], [`PairedTokenSink`],
/// [`OffsetLenSplitSink`]) must price `slot` this same way, since none of
/// them vary its coding.
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
                sink.flag(models, flag_table, FlagKind::Literal);
                sink.literal(models, context, byte);
                context = context.after_literal(byte);
                pos += 1;
            }
            Token::Match { len, distance } => {
                sink.flag(models, flag_table, FlagKind::Match);
                sink.length(models, FlagKind::Match, len);
                sink.offset(models, distance.get());
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
/// trial (`research/JOURNAL.md` S1-P5, `COLUMN_EXPERT_MIN_VERSION`).
struct EncodeSink<'a> {
    ac: &'a mut Encoder,
    column: Option<ColumnCoding<'a>>,
}

impl TokenSink for EncodeSink<'_> {
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind) {
        models.flag[flag_table].encode(self.ac, kind.index());
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        // Compression always targets the newest format version
        // (`FORMAT_VERSION`), so encoding always takes the logit-domain
        // mixer (with or without the column expert); `decode` is the one
        // that must still read older frames.
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
        // Compression always targets the newest format version, so
        // encoding always takes the split models (LENGTH_SPLIT_MIN_VERSION);
        // `decode` is the one that must still read older frames through
        // the shared `models.length`.
        encode_bucketed(split_length_model(models, kind), self.ac, value);
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
        // Matches EncodeSink::length (the split models): ideal_cost_bits
        // stays a true estimate of what encode_tokens's real Encoder pays.
        self.bits += price_length(models, kind, value);
    }

    fn offset(&mut self, models: &mut Models, value: u32) {
        self.bits += price_offset(models, value);
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
        // baseline half must equal ideal_cost_bits exactly, the guard
        // `ppm_expert_experiment_baseline_is_exactly_ideal_cost_bits` checks.
        self.cost.add_same(price_length(models, kind, value));
    }

    fn offset(&mut self, models: &mut Models, value: u32) {
        self.cost.add_same(price_offset(models, value));
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

/// `research/JOURNAL.md` S2-A109/S2-A110 split the shared length model by
/// copy kind; this candidate targets a different split axis LZMA also
/// conditions on. LZMA (`lzma-specification.txt`) prices a match's
/// distance slot through one of four independent probability trees
/// selected by `len_to_pos_state`, a coarse bucket of the match's own
/// length, not by copy kind: `Models::offset` only ever prices a
/// [`Token::Match`]'s distance in the first place ([`Token::Rep`] reuses a
/// cached distance and prices through `models.slot` instead). Hypothesis:
/// a short match's distance and a long match's distance come from
/// measurably different distributions (a short copy is more likely an
/// incidental near-range coincidence, a long one more likely a genuine
/// structural recurrence reaching further back), so splitting
/// `Models::offset` by the triggering match's own length state improves
/// bpb without regressing the sealed set. Priced through
/// `OffsetLenSplitSink`, the same before-wiring shape `research/JOURNAL.md`
/// S2-A109's own (since-wired-and-deleted) `ideal_cost_bits_length_split_experiment`
/// established.
///
/// Not reachable from [`encode`]/[`decode`]: no `Method`/`FORMAT_VERSION`
/// wiring, measurement only.
///
/// Returns `(baseline_bits, with_split_bits)`.
#[must_use]
pub fn ideal_cost_bits_offset_length_split_experiment(data: &[u8]) -> (f64, f64) {
    let tokens = lz::parse_optimal(data);
    let mut models = Models::new();
    let mut state = OffsetLenSplitState::new();
    let mut sink = OffsetLenSplitSink {
        cost: PairedCost::default(),
        state: &mut state,
        last_match_len: 0,
    };
    walk_tokens(&tokens, data, &mut models, &mut sink);
    (sink.cost.baseline, sink.cost.candidate)
}

/// How many independent [`Model`]s [`OffsetLenSplitState`] holds, and the
/// bucket count [`offset_len_state`] ever returns: LZMA's own
/// `len_to_pos_state` uses four states (`lzma-specification.txt`).
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

/// [`ideal_cost_bits_offset_length_split_experiment`]'s own state: four
/// offset [`Model`]s instead of [`Models::offset`]'s one, selected by
/// [`offset_len_state`] of the triggering match's own length.
struct OffsetLenSplitState {
    offset: [Model; OFFSET_LEN_STATES],
}

impl OffsetLenSplitState {
    fn new() -> Self {
        Self {
            offset: std::array::from_fn(|_| Model::new(lz::OFFSET_BUCKETS)),
        }
    }
}

/// [`TokenSink`] for [`ideal_cost_bits_offset_length_split_experiment`]:
/// every field prices identically to [`CostSink`] except `offset`, which
/// the baseline side still prices through the shared `models.offset` and
/// the candidate side prices through whichever of
/// [`OffsetLenSplitState`]'s four models [`offset_len_state`] selects from
/// the match length [`Self::length`] most recently recorded.
struct OffsetLenSplitSink<'a> {
    cost: PairedCost,
    state: &'a mut OffsetLenSplitState,
    /// The length `walk_tokens` most recently passed to [`Self::length`]
    /// for a [`Token::Match`]: always current by the time [`Self::offset`]
    /// reads it, since `walk_tokens` only ever calls `offset` immediately
    /// after `length` for a `Token::Match` (never for a `Token::Rep`,
    /// which has no `offset` call at all — see this sink's own `offset`
    /// doc).
    last_match_len: u32,
}

impl TokenSink for OffsetLenSplitSink<'_> {
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind) {
        self.cost.add_same(price_flag(models, flag_table, kind));
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        self.cost.add_same(price_literal(models, context, byte));
    }

    fn length(&mut self, models: &mut Models, kind: FlagKind, value: u32) {
        if kind == FlagKind::Match {
            self.last_match_len = value;
        }
        self.cost.add_same(price_length(models, kind, value));
    }

    fn offset(&mut self, models: &mut Models, value: u32) {
        let baseline = price_offset(models, value);
        let candidate_model = &mut self.state.offset[offset_len_state(self.last_match_len)];
        let candidate = ideal_cost_bucketed(candidate_model, value);
        self.cost.add(baseline, candidate);
    }

    fn slot(&mut self, models: &mut Models, symbol: usize) {
        self.cost.add_same(price_slot(models, symbol));
    }
}

#[cfg(test)]
mod offset_length_split_tests {
    use super::ideal_cost_bits_offset_length_split_experiment;

    /// Sanity check that both totals are finite and non-negative over a
    /// buffer containing a real mix of literals and fresh matches at two
    /// different distances (`"ab"` repeated, then a long run of one byte,
    /// then `"ab"` repeated again), before any corpus-scale measurement is
    /// trusted.
    #[test]
    fn offset_length_split_prices_a_mixed_buffer_finitely() {
        let mut data = Vec::new();
        data.extend_from_slice(b"ab");
        for _ in 0..40 {
            data.extend_from_slice(b"ab");
        }
        data.extend_from_slice(&[0x5Au8; 200]);
        data.extend_from_slice(b"ab");
        for _ in 0..40 {
            data.extend_from_slice(b"ab");
        }
        let (baseline, candidate) = ideal_cost_bits_offset_length_split_experiment(&data);
        assert!(baseline.is_finite() && baseline >= 0.0);
        assert!(candidate.is_finite() && candidate >= 0.0);
    }

    /// [`offset_len_state`] saturates instead of panicking on a length at
    /// or above `lz::MIN_MATCH_LEN + OFFSET_LEN_STATES`: real callers only
    /// ever pass a [`Token::Match`] length, which can run into the
    /// thousands on highly repetitive input.
    #[test]
    fn offset_len_state_saturates_on_a_long_match() {
        use super::offset_len_state;
        assert_eq!(
            offset_len_state(u32::try_from(crate::lz::MIN_MATCH_LEN).unwrap() + 1_000),
            super::OFFSET_LEN_STATES - 1
        );
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

/// Decodes a copy token's length symbol: through [`Models::length_match`]/
/// `length_rep` (selected by `kind`) when `length_split` is set
/// (`LENGTH_SPLIT_MIN_VERSION` and above), or through the shared
/// [`Models::length`] otherwise. `EncodeSink::length` always takes the
/// split branch unconditionally (compression always targets the newest
/// version); this function is the one that must still read older frames.
///
/// # Panics
///
/// Panics if `kind` is [`FlagKind::Literal`]: both of [`decode_tokens`]'s
/// call sites already have a [`FlagKind::Match`] or [`FlagKind::Rep`] in
/// hand.
fn decode_length(models: &mut Models, ac: &mut Decoder, length_split: bool, kind: FlagKind) -> u32 {
    if length_split {
        decode_bucketed(split_length_model(models, kind), ac)
    } else {
        decode_bucketed(&mut models.length, ac)
    }
}

/// The shared skeleton behind [`decode`] and [`decode_undoable_streaming`]:
/// decodes `token_count` tokens off `ac` in coding order, routing every
/// literal byte and copy through `sink`, and advancing the literal-model
/// context and `reps` exactly as [`decode`] and [`decode_undoable_streaming`]
/// both require. See [`DecodeSink`]'s docs for why this exists. `length_split`
/// is the frame's declared version checked against
/// `LENGTH_SPLIT_MIN_VERSION` once, up front, by both callers (mirroring
/// [`LiteralPath::for_version`]'s own once-per-frame version read).
fn decode_tokens<S: DecodeSink>(
    token_count: u32,
    declared_len: usize,
    length_split: bool,
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
                let len = decode_length(models, ac, length_split, FlagKind::Match);
                let distance = decode_bucketed(&mut models.offset, ac);
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
                let len = decode_length(models, ac, length_split, FlagKind::Rep);
                let distance = reps.get(slot);
                ensure_room(sink.len(), len as usize, declared_len)?;
                context = sink.copy(len, distance, context)?;
                reps.promote(slot);
            }
        }
    }
    Ok(())
}

/// Which mixer a decoded frame's non-`Candidate::Transpose` literal
/// sub-stream codes through, selected once from the frame's declared
/// `version`: four mutually exclusive states derived from three
/// thresholds (`LOGISTIC_MIN_VERSION`, `SURPRISE_MIN_VERSION`,
/// `LOGIT_SSE_MIN_VERSION`) rather than carried as independent bools, so a
/// version cannot read as selecting more than one mixer at once (CLAUDE.md's
/// precision value, illegal states unrepresentable). `Candidate::Transpose`
/// never consults this: its own `COLUMN_EXPERT_MIN_VERSION` gate picks
/// `encode_column`/`decode_column` regardless.
#[derive(Clone, Copy)]
enum LiteralPath {
    Sse,
    Logistic,
    LogisticSurprise,
    LogitSse,
}

impl LiteralPath {
    /// Picks the path a frame declaring `version` codes its literals
    /// through, mirroring [`decode`]'s own version gates
    /// (`LOGISTIC_MIN_VERSION`, `SURPRISE_MIN_VERSION`,
    /// `LOGIT_SSE_MIN_VERSION`).
    const fn for_version(version: u8) -> Self {
        if version >= LOGIT_SSE_MIN_VERSION {
            Self::LogitSse
        } else if version >= SURPRISE_MIN_VERSION {
            Self::LogisticSurprise
        } else if version >= LOGISTIC_MIN_VERSION {
            Self::Logistic
        } else {
            Self::Sse
        }
    }
}

/// Decodes one non-column literal through `path`'s mixer: [`VecSink`] and
/// [`StreamingSink`] both need exactly this dispatch, named once so the two
/// sinks' version gates cannot drift apart from each other.
fn decode_literal_path(
    path: LiteralPath,
    models: &mut Models,
    ac: &mut Decoder,
    context: Context,
) -> u8 {
    match path {
        LiteralPath::LogitSse => {
            models
                .literal
                .decode_logit_sse(ac, context, &mut models.logit_sse)
        }
        LiteralPath::LogisticSurprise => {
            models
                .literal
                .decode_logistic_surprise(ac, context, &mut models.surprise)
        }
        LiteralPath::Logistic => models
            .literal
            .decode_logistic(ac, context, &mut models.logistic),
        LiteralPath::Sse => models.literal.decode_sse(ac, context),
    }
}

/// [`DecodeSink`] for [`decode`]'s whole-buffer path: a literal byte is
/// pushed onto `output`, and a copy replays [`copy_checked`] over what
/// `output` already holds. `column` mirrors [`encode_tokens`]'s
/// `ColumnCoding`, but owned here rather than borrowed (there is no
/// per-candidate trial to share it across): `Some` exactly when this
/// frame's candidate is [`Candidate::Transpose`] and its declared version
/// selects the column-keyed literal expert (`COLUMN_EXPERT_MIN_VERSION`).
/// `literal_path` selects the mixer for every other candidate, consulted
/// only when `column` is `None`.
struct VecSink<'a> {
    output: &'a mut Vec<u8>,
    declared_len: usize,
    column: Option<(NonZeroUsize, &'a mut ColumnExpertState)>,
    literal_path: LiteralPath,
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
            None => decode_literal_path(self.literal_path, models, ac, context),
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
/// `version` is the frame's declared `FORMAT_VERSION` byte
/// (`crate::decompress` already has it in scope at its one call site,
/// guaranteed at least `LZ_MIN_VERSION` (3) before this function is ever
/// called): versions 3 and below `LOGISTIC_MIN_VERSION` decode the literal
/// sub-stream through [`crate::literal::Literal::decode_sse`]; versions
/// `LOGISTIC_MIN_VERSION` (5) and above but below `SURPRISE_MIN_VERSION`
/// decode it through [`crate::literal::Literal::decode_logistic`] instead;
/// versions `SURPRISE_MIN_VERSION` (6) and above but below
/// `LOGIT_SSE_MIN_VERSION` decode it through
/// [`crate::literal::Literal::decode_logistic_surprise`] instead; versions
/// `LOGIT_SSE_MIN_VERSION` (7) and above decode it through
/// [`crate::literal::Literal::decode_logit_sse`] instead — except, at every
/// one of those versions, for a [`Candidate::Transpose`] frame, version
/// `COLUMN_EXPERT_MIN_VERSION` (4) and above, which decodes through
/// [`crate::literal::Literal::decode_column`] regardless, blending a
/// column-keyed seventh expert into the mix (see the module docs' "Payload
/// layout" section). `offset` and `slot` decode identically regardless of
/// `version` or candidate; `length` decodes through the shared
/// `Models::length` below `LENGTH_SPLIT_MIN_VERSION` (8) and through
/// `Models::length_match`/`length_rep` at or above it, regardless of
/// candidate (see the module docs' "Payload layout" section).
///
/// # Panics
///
/// Does not panic on adversarial `payload`. Two internal `.expect()`s
/// guard invariants of this function's own math, never a property of
/// `payload`: turning a decoded match distance into a [`NonZeroU32`]
/// (a mathematical invariant of `decode_bucketed`, see that function's
/// docs), and casting a declared length already found `<= max_len` (a
/// `u32`) back down from the `usize` `read_header` widened it to.
pub fn decode(payload: &[u8], version: u8, max_len: u32) -> Result<Vec<u8>, Error> {
    let max_len = max_len.min(MAX_DECODED_LEN);
    let (filter_bytes, payload) = payload.split_at_checked(2).ok_or(Error::Truncated)?;
    let candidate =
        Candidate::from_header_bytes([filter_bytes[0], filter_bytes[1]]).ok_or(Error::Corrupt)?;
    let (declared_len, token_count, ac_bytes) = read_header(payload)?;
    ensure_within_max_len(declared_len, max_len)?;

    let mut ac = Decoder::new(ac_bytes);
    let mut models = Models::try_new()?;
    let mut reps = RepCache::initial();
    // Some exactly when this frame's candidate is Candidate::Transpose and
    // its declared version codes the column-expert path (COLUMN_EXPERT_MIN_VERSION):
    // mirrors encode_tokens's ColumnCoding, but `state` is owned here
    // (there is no per-candidate trial to share it across).
    let mut column_state: Option<(NonZeroUsize, ColumnExpertState)> = match candidate {
        Candidate::Transpose(columns) if version >= COLUMN_EXPERT_MIN_VERSION => {
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
        literal_path: LiteralPath::for_version(version),
    };
    decode_tokens(
        token_count,
        declared_len,
        version >= LENGTH_SPLIT_MIN_VERSION,
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
    version: u8,
    max_len: u32,
    writer: &mut W,
) -> Result<(), crate::WriteError> {
    let (filter_bytes, filtered_payload) = payload.split_at_checked(2).ok_or(Error::Truncated)?;
    let candidate =
        Candidate::from_header_bytes([filter_bytes[0], filter_bytes[1]]).ok_or(Error::Corrupt)?;
    match candidate {
        Candidate::Identity => decode_undoable_streaming(
            filtered_payload,
            version,
            max_len,
            writer,
            &mut StreamUndo::Identity,
        ),
        Candidate::Delta(stride) => {
            let undo = filters::delta::Undo::try_new(stride).map_err(Error::from)?;
            decode_undoable_streaming(
                filtered_payload,
                version,
                max_len,
                writer,
                &mut StreamUndo::Delta(undo),
            )
        }
        Candidate::Bcj => {
            let undo = filters::bcj::Undo::try_new().map_err(Error::from)?;
            decode_undoable_streaming(
                filtered_payload,
                version,
                max_len,
                writer,
                &mut StreamUndo::Bcj(undo),
            )
        }
        Candidate::Transpose(_) => {
            let decoded = decode(payload, version, max_len)?;
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
/// path instead (see that function's docs). `literal_path` mirrors
/// [`VecSink`]'s own version gate.
struct StreamingSink<'a, W: std::io::Write> {
    window: &'a mut lz::Window,
    undo: &'a mut StreamUndo,
    writer: &'a mut W,
    literal_path: LiteralPath,
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
        let byte = decode_literal_path(self.literal_path, models, ac, context);
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
/// [`read_header`]'s expected input. `version` is the frame's declared
/// `FORMAT_VERSION` byte, [`decode_to_writer`]'s own parameter passed
/// through unchanged, gating the literal sub-stream exactly as [`decode`]'s
/// own `version` does (`LOGISTIC_MIN_VERSION`, `SURPRISE_MIN_VERSION`,
/// `LOGIT_SSE_MIN_VERSION`).
fn decode_undoable_streaming<W: std::io::Write>(
    payload: &[u8],
    version: u8,
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
        literal_path: LiteralPath::for_version(version),
    };
    decode_tokens(
        token_count,
        declared_len,
        version >= LENGTH_SPLIT_MIN_VERSION,
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
