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
//! decodes (`LZ_MIN_VERSION` and up). Every one of those versions' literal
//! sub-stream codes each byte as 8 SSE-calibrated binary decisions
//! ([`crate::literal::Literal::encode_sse`]/`decode_sse`,
//! `docs/adr/0038-wire-sse-into-the-literal-mixer.md`, `research/JOURNAL.md`
//! S1-P1). A version-4 frame whose filter selector names
//! [`Candidate::Transpose`] goes one step
//! further still: each literal byte blends a column-keyed seventh expert
//! into the mix before the same SSE-calibrated coding
//! ([`crate::literal::Literal::encode_column`]/`decode_column`,
//! `docs/adr/0046-wire-the-column-expert-into-the-literal-mixer.md`,
//! `research/JOURNAL.md` S1-P5); every other candidate at version 4 codes
//! its literals exactly as version 3 does. [`decode`] takes the frame's
//! declared `version` and its already-parsed `candidate` and picks the
//! matching literal path; every other symbol (flag/length/offset/slot) is
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
use crate::literal::{ColumnExpertState, Context, Literal, LogisticMix, PpmExpertState};
use crate::lz::{self, RepCache, RepSlot, Token};
use crate::model::Model;

/// Lowest `FORMAT_VERSION` whose `Method::Lz` payload this build can
/// decode: see the module docs' "Payload layout" section for the layout
/// change that moved this from 1 to 2, and for version 2's own retirement
/// (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`) that moved
/// it from 2 to 3. Every version this constant admits codes its literal
/// sub-stream through [`crate::literal::Literal::encode_sse`]/`decode_sse`
/// (or, at version 4 on a `Candidate::Transpose` frame,
/// [`crate::literal::Literal::encode_column`]/`decode_column`); no version
/// this build decodes still needs a separate literal-coding floor.
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
/// here.
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

    /// Fallible counterpart to [`Self::new`]: the same five fresh tables,
    /// but returns `Err` instead of aborting if the allocator cannot
    /// satisfy one of them. [`decode`] and [`decode_undoable_streaming`]
    /// use this (hard rule 2, `rust-craft` skill's allocation-discipline,
    /// `tests/torture.rs`, #453); every encode path keeps [`Self::new`],
    /// where an allocator failure this small is already a fatal condition
    /// for the whole process.
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            literal: Literal::try_new()?,
            flag: [
                Model::try_new(FLAG_ALPHABET)?,
                Model::try_new(FLAG_ALPHABET)?,
            ],
            length: Model::try_new(lz::LENGTH_BUCKETS)?,
            offset: Model::try_new(lz::OFFSET_BUCKETS)?,
            slot: Model::try_new(lz::REP_SLOTS)?,
        })
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
///
/// # Errors
///
/// Returns [`Error::OutOfMemory`] if the allocator cannot satisfy the
/// undo buffer a non-`Identity` candidate needs (hard rule 2,
/// `rust-craft` skill's allocation-discipline, `tests/torture.rs`, #453).
fn undo_filter(candidate: Candidate, data: Vec<u8>) -> Result<Vec<u8>, Error> {
    match candidate {
        Candidate::Identity => Ok(data),
        Candidate::Delta(stride) => {
            filters::delta::try_decode(&data, stride).map_err(|_| Error::OutOfMemory)
        }
        Candidate::Bcj => filters::bcj::try_decode(&data).map_err(|_| Error::OutOfMemory),
        Candidate::Transpose(columns) => {
            filters::transpose::try_decode(&data, columns).map_err(|_| Error::OutOfMemory)
        }
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
                sink.flag(models, flag_table, FlagKind::Literal);
                sink.literal(models, context, byte);
                context = context.after_literal(byte);
                pos += 1;
            }
            Token::Match { len, distance } => {
                sink.flag(models, flag_table, FlagKind::Match);
                sink.length(models, len);
                sink.offset(models, distance.get());
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
            Token::Rep { len, slot } => {
                sink.flag(models, flag_table, FlagKind::Rep);
                sink.slot(models, slot.index());
                sink.length(models, len);
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
/// [`crate::literal::Literal::encode_sse`] for every literal in this trial
/// (`research/JOURNAL.md` S1-P5, `COLUMN_EXPERT_MIN_VERSION`).
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
        // (`FORMAT_VERSION`), so encoding always takes the SSE-calibrated
        // path (with or without the column expert); `decode` is the one
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
            None => models.literal.encode_sse(self.ac, context, byte),
        }
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
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind) {
        self.bits += models.flag[flag_table].ideal_cost_bits(kind.index());
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
/// ([`ideal_cost_bits_ppm_expert_experiment`],
/// [`ideal_cost_bits_logistic_mix_experiment_at`]): flag/length/offset/slot
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
        self.cost
            .add_same(models.flag[flag_table].ideal_cost_bits(kind.index()));
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        let (baseline, candidate) = (self.pair_literal)(models, context, byte);
        self.cost.add(baseline, candidate);
    }

    fn length(&mut self, models: &mut Models, value: u32) {
        self.cost
            .add_same(ideal_cost_bucketed(&mut models.length, value));
    }

    fn offset(&mut self, models: &mut Models, value: u32) {
        self.cost
            .add_same(ideal_cost_bucketed(&mut models.offset, value));
    }

    fn slot(&mut self, models: &mut Models, symbol: usize) {
        self.cost.add_same(models.slot.ideal_cost_bits(symbol));
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

/// [`ideal_cost_bits_logistic_mix_experiment_at`] at the registered
/// default [`crate::literal::LOGISTIC_RATE_DECAY`].
///
/// Returns `(baseline_bits, candidate_bits)`.
#[must_use]
pub fn ideal_cost_bits_logistic_mix_experiment(data: &[u8]) -> (f64, f64) {
    ideal_cost_bits_logistic_mix_experiment_at(data, crate::literal::LOGISTIC_RATE_DECAY)
}

/// `research/JOURNAL.md` S2-A101's before-wiring measurement: the whole
/// codec's ideal cost of `data` with the literal model priced both as
/// shipped and through [`LogisticMix`] under rate-schedule `decay`, over
/// one shared token stream and one shared set of banks. Not reachable
/// from [`encode`]/[`decode`]: no `Method`/`FORMAT_VERSION` wiring,
/// measurement only.
///
/// Returns `(baseline_bits, candidate_bits)`; `baseline_bits` equals
/// [`ideal_cost_bits`]`(data)` exactly.
#[must_use]
pub fn ideal_cost_bits_logistic_mix_experiment_at(data: &[u8], decay: f64) -> (f64, f64) {
    let tokens = lz::parse_optimal(data);
    let mut models = Models::new();
    let mut logistic = LogisticMix::new();
    let mut sink = PairedTokenSink {
        cost: PairedCost::default(),
        pair_literal: |models: &mut Models, context: Context, byte: u8| {
            models
                .literal
                .ideal_cost_bits_logistic_pair(context, byte, &mut logistic, decay)
        },
    };
    walk_tokens(&tokens, data, &mut models, &mut sink);
    (sink.cost.baseline, sink.cost.candidate)
}

/// Whether `body_len` beats `best_len` in `encode`'s shortest-wins
/// candidate search. Strict: a tie keeps the earlier (lower-indexed)
/// candidate, so filter order in [`filters::select::pick`] is a stable
/// tie-break, not an accident of iteration.
fn shorter_than(body_len: usize, best_len: usize) -> bool {
    body_len < best_len
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
        if best
            .as_ref()
            .is_none_or(|(_, existing)| shorter_than(body.len(), existing.len()))
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
                let len = decode_bucketed(&mut models.length, ac);
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
                let len = decode_bucketed(&mut models.length, ac);
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
/// frame's candidate is [`Candidate::Transpose`] and its declared version
/// selects the column-keyed literal expert (`COLUMN_EXPERT_MIN_VERSION`).
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
            None => models.literal.decode_sse(ac, context),
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
/// called): every version decodes the literal sub-stream through
/// [`crate::literal::Literal::decode_sse`], except — only for a
/// [`Candidate::Transpose`] frame, version `COLUMN_EXPERT_MIN_VERSION` (4)
/// and above — through [`crate::literal::Literal::decode_column`] instead,
/// blending a column-keyed seventh expert into the mix (see the module
/// docs' "Payload layout" section). Every other symbol decodes identically
/// regardless of `version` or candidate, since only the literal
/// sub-stream's internal shape changed.
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
    let mut models = Models::try_new().map_err(|_| Error::OutOfMemory)?;
    let mut reps = RepCache::initial();
    // Some exactly when this frame's candidate is Candidate::Transpose and
    // its declared version codes the column-expert path (COLUMN_EXPERT_MIN_VERSION):
    // mirrors encode_tokens's ColumnCoding, but `state` is owned here
    // (there is no per-candidate trial to share it across).
    let mut column_state: Option<(NonZeroUsize, ColumnExpertState)> = match candidate {
        Candidate::Transpose(columns) if version >= COLUMN_EXPERT_MIN_VERSION => Some((
            columns,
            ColumnExpertState::try_new(MAX_COLUMN_BANKS).map_err(|_| Error::OutOfMemory)?,
        )),
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
    output
        .try_reserve_exact(declared_len)
        .map_err(|_| Error::OutOfMemory)?;

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
    version: u8,
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
            let undo = filters::delta::Undo::try_new(stride).map_err(|_| Error::OutOfMemory)?;
            decode_undoable_streaming(
                filtered_payload,
                max_len,
                writer,
                &mut StreamUndo::Delta(undo),
            )
        }
        Candidate::Bcj => {
            let undo = filters::bcj::Undo::try_new().map_err(|_| Error::OutOfMemory)?;
            decode_undoable_streaming(
                filtered_payload,
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
        let byte = models.literal.decode_sse(ac, context);
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
    let mut models = Models::try_new().map_err(|_| Error::OutOfMemory)?;
    let mut reps = RepCache::initial();
    let mut window = lz::Window::try_new().map_err(|_| Error::OutOfMemory)?;

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
mod tests {
    use super::*;

    /// [`decode_to_writer`], collected into a `Vec<u8>` (which implements
    /// [`std::io::Write`]) instead of streamed to a real sink, so tests can
    /// compare its output byte for byte against [`decode`]'s.
    fn decode_streaming(
        payload: &[u8],
        version: u8,
        max_len: u32,
    ) -> Result<Vec<u8>, crate::WriteError> {
        let mut out = Vec::new();
        decode_to_writer(payload, version, max_len, &mut out)?;
        Ok(out)
    }

    /// Extracts the [`Error`] from a [`crate::WriteError`] produced by
    /// [`decode_to_writer`], for tests asserting exactly which decode error
    /// occurred rather than just that some error did.
    fn as_codec_error(err: &crate::WriteError) -> Option<&Error> {
        match err {
            crate::WriteError::Decode(inner) => Some(inner),
            crate::WriteError::Io(_) => None,
        }
    }

    fn roundtrip(data: &[u8]) {
        let encoded = encode(data);
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(data),
            "roundtrip mismatch"
        );
        assert_eq!(
            decode_streaming(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .as_deref()
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            data,
            "streaming roundtrip mismatch"
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
            decode(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(data.as_slice())
        );
        assert_eq!(
            decode_streaming(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            data,
            "streaming roundtrip mismatch"
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

    /// Four independent small-step random walks, one per column, laid out
    /// row-major with a 4-byte stride: consecutive same-column bytes drift
    /// by a small step (`filters::select::pick` ranks `Candidate::Delta(4)`
    /// top for exactly this shape — mirrors its own
    /// `pick_selects_delta_for_columnar_drift` test), but consecutive raw
    /// bytes belong to different, unrelated walks.
    fn columnar_drift_data() -> Vec<u8> {
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
        data
    }

    #[test]
    fn roundtrip_columnar_drift_data_selects_delta_and_streams_it() {
        // Proves encode() actually wires a trial-selected filter into the
        // frame, not just plumbs pick() through unused; pins the kind byte
        // to 1 (Delta) rather than just non-identity, since this fixture is
        // also decode_undoable_streaming's only regression coverage for
        // Candidate::Delta (`research/JOURNAL.md` S1-P7) — a future trial-
        // cost change that made encode() prefer a different candidate here
        // would silently drop that coverage without this pin.
        let data = columnar_drift_data();
        let encoded = encode(&data);
        assert_eq!(
            encoded[0], 1,
            "columnar drift data should select Delta, got kind byte {}",
            encoded[0]
        );
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(data.as_slice())
        );
        assert_eq!(
            decode_streaming(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            data,
            "streaming roundtrip mismatch: decode_undoable_streaming's Delta path"
        );
    }

    #[test]
    fn decode_undoable_streaming_delta_path_covers_copy_streamed_too() {
        // columnar_drift_data's random walk gives decode_undoable_streaming's
        // Delta path literal-only coverage: no fixed-distance repeat in that
        // fixture is long enough for lz::parse_optimal to price a match over
        // literals. Crafted directly against Candidate::Delta (bypassing
        // encode()'s trial selection) so copy_streamed's own undo call gets
        // exercised: a filtered stream built from a short repeating pattern
        // guarantees parse_optimal finds match/rep tokens for the repeats.
        let stride = crate::test_support::nz(4);
        let filtered: Vec<u8> = [1u8, 2, 3, 4].iter().copied().cycle().take(200).collect();
        let raw = filters::delta::decode(&filtered, stride);
        let mut frame = Candidate::Delta(stride).to_header_bytes().to_vec();
        frame.extend(encode_tokens(&filtered, None));

        assert_eq!(
            decode(&frame, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(raw.as_slice())
        );
        assert_eq!(
            decode_streaming(&frame, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            raw,
            "streaming roundtrip mismatch: Delta path with match/rep copies"
        );
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
    fn bcj_call_dense_data() -> Vec<u8> {
        let mut data = vec![0x90u8; 4000];
        for (idx, chunk) in data.chunks_mut(20).enumerate() {
            chunk[0] = 0xE8;
            let post_addr = u32::try_from(idx * 20 + filters::bcj::INSTRUCTION_LEN)
                .expect("idx * 20 + 5 fits u32 for this fixture's size (4000)");
            let rel = 0u32.wrapping_sub(post_addr);
            chunk[1..filters::bcj::INSTRUCTION_LEN].copy_from_slice(&rel.to_le_bytes());
        }
        data
    }

    #[test]
    fn roundtrip_bcj_call_dense_data_selects_bcj_and_streams_it() {
        // Same shape as roundtrip_columnar_drift_data_selects_delta_and_
        // streams_it: proves encode() actually selects Candidate::Bcj for a
        // real fixture (kind byte 2), not just that filters::bcj round-trips
        // in isolation, and that decode_to_writer's Bcj path
        // (research/JOURNAL.md S1-P7) reproduces decode()'s output exactly.
        let data = bcj_call_dense_data();
        let encoded = encode(&data);
        assert_eq!(
            encoded[0], 2,
            "call-dense data should select Bcj, got kind byte {}",
            encoded[0]
        );
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(data.as_slice())
        );
        assert_eq!(
            decode_streaming(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            data,
            "streaming roundtrip mismatch: decode_undoable_streaming's Bcj path"
        );
    }

    /// Fixed-width records whose columns each cycle through their own
    /// period (`research/JOURNAL.md` S1-P5's target shape, same
    /// construction as `tests/golden/v4-tabular-columns`), with a fraction
    /// of bytes jittered off the clean pattern so the literal model, not
    /// just `lz::parse_optimal`'s LZ matches, carries real weight — the
    /// data this ADR-0046 slice's own real-bitstream measurement used.
    fn tabular_columns_data(columns: usize, rows: usize, seed: u32) -> Vec<u8> {
        let periods: Vec<u32> = (0..columns)
            .map(|c| 3 + ((u32::try_from(c).unwrap() * 7 + seed) % 6))
            .collect();
        let bases: Vec<u8> = (0..columns)
            .map(|c| {
                u8::try_from(
                    (u32::try_from(c).unwrap().wrapping_mul(0x1e))
                        .wrapping_add(seed.wrapping_mul(3))
                        % 256,
                )
                .unwrap()
            })
            .collect();
        let mut rng =
            crate::test_support::Xorshift32::new(0x9e37_79b9 ^ seed.wrapping_mul(0x8551_4d97) | 1);
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

    #[test]
    fn roundtrip_tabular_columns_data_selects_transpose_and_streams_it() {
        // Same shape as roundtrip_columnar_drift_data_selects_delta_and_
        // streams_it and roundtrip_bcj_call_dense_data_selects_bcj_and_
        // streams_it: proves encode() actually selects Candidate::Transpose
        // for real data (kind byte 3), and that both decode() and
        // decode_streaming (which falls back to decode()'s whole-buffer
        // path for Transpose, JOURNAL S2-D4) reproduce the original bytes —
        // this is the real-wiring column-expert path (ADR-0046), not just
        // filters::transpose round-tripping in isolation.
        let data = tabular_columns_data(8, 2000, 1);
        let encoded = encode(&data);
        assert_eq!(
            encoded[0], 3,
            "tabular column data should select Transpose, got kind byte {}",
            encoded[0]
        );
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(data.as_slice())
        );
        assert_eq!(
            decode_streaming(&encoded, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            data,
            "streaming roundtrip mismatch: Transpose's whole-buffer fallback path"
        );
    }

    #[test]
    fn column_expert_path_is_gated_on_both_version_and_candidate() {
        // A Candidate::Transpose frame declared at a version below
        // COLUMN_EXPERT_MIN_VERSION must decode through the plain SSE path
        // (Literal::decode_sse), never decode_column: encoding a payload
        // the column-expert path actually produced and then decoding it as
        // version 3 must NOT reproduce the original data (the two paths
        // code different bits for the same bytes), proving the version
        // gate, not just the candidate check, controls dispatch.
        let data = tabular_columns_data(8, 2000, 1);
        let encoded = encode(&data);
        assert_eq!(encoded[0], 3, "fixture must select Transpose");
        assert_ne!(
            decode(&encoded, LZ_MIN_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(data.as_slice()),
            "decoding a COLUMN_EXPERT_MIN_VERSION frame as version 3 must not \
             silently reproduce the original data"
        );
    }

    #[test]
    fn decode_undoable_streaming_bcj_path_covers_copy_streamed_too() {
        // A run of identical 5-byte instructions in the *filtered* stream
        // gives lz::parse_optimal real match/rep tokens to find, exercising
        // copy_streamed's own undo call — the raw (pre-filter) bytes this
        // decodes to are not themselves repetitive, since each instance's
        // absolute-to-relative rewrite depends on its own position, proving
        // filters::bcj::Undo recomputes that per position rather than
        // replaying whatever the first instance resolved to.
        let unit = [0xE8u8, 0x00, 0x00, 0x00, 0x00];
        let filtered: Vec<u8> = unit.iter().copied().cycle().take(200).collect();
        let raw = filters::bcj::decode(&filtered);
        let mut frame = Candidate::Bcj.to_header_bytes().to_vec();
        frame.extend(encode_tokens(&filtered, None));

        assert_eq!(
            decode(&frame, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(raw.as_slice())
        );
        assert_eq!(
            decode_streaming(&frame, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            raw,
            "streaming roundtrip mismatch: Bcj path with match/rep copies"
        );
    }

    #[test]
    fn decode_undoable_streaming_bcj_path_flushes_a_trailing_opcode_on_finish() {
        // The filtered stream ends on an 0xE8 opcode byte followed by only
        // 3 more bytes: one short of INSTRUCTION_LEN (5), so
        // filters::bcj::Undo::apply never resolves it and StreamUndo::Bcj's
        // `pending` is still non-empty when the token loop ends. Those 4
        // bytes only reach `writer` through StreamUndo::finish's flush
        // (codec.rs's own call, not filters::bcj::Undo::finish, which
        // undo_tests::matches_decode_too_short_for_any_instruction already
        // covers one layer below this one): turning that call into a no-op
        // silently drops them, a losslessness violation invisible to every
        // other Bcj fixture here because they all end on an instruction
        // boundary.
        let unit = [0xE8u8, 0x00, 0x00, 0x00, 0x00];
        let mut filtered: Vec<u8> = unit.iter().copied().cycle().take(200).collect();
        filtered.extend_from_slice(&[0xE8, 0x01, 0x02, 0x03]);
        let raw = filters::bcj::decode(&filtered);
        let mut frame = Candidate::Bcj.to_header_bytes().to_vec();
        frame.extend(encode_tokens(&filtered, None));

        assert_eq!(
            decode(&frame, crate::FORMAT_VERSION, MAX_DECODED_LEN).as_deref(),
            Ok(raw.as_slice())
        );
        assert_eq!(
            decode_streaming(&frame, crate::FORMAT_VERSION, MAX_DECODED_LEN)
                .expect("decode_to_writer must succeed whenever decode does, same payload"),
            raw,
            "streaming roundtrip mismatch: StreamUndo::finish must flush a trailing buffered Bcj opcode"
        );
    }

    #[test]
    fn truncated_header_is_rejected() {
        assert_eq!(
            decode(&[0u8; 4], crate::FORMAT_VERSION, MAX_DECODED_LEN),
            Err(Error::Truncated)
        );
        assert_eq!(
            decode(&[], crate::FORMAT_VERSION, MAX_DECODED_LEN),
            Err(Error::Truncated)
        );
    }

    #[test]
    fn unknown_filter_selector_is_rejected_not_panicking() {
        // Kind byte 5 names no Candidate::from_header_bytes ever produces
        // (0=Identity, 1=Delta, 2=Bcj, 3=Transpose): an adversarial or
        // future-format payload, never a bug in this decoder.
        let mut payload = vec![5u8, 0u8];
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            decode(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN),
            Err(Error::Corrupt)
        );
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
            decode(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN),
            Err(Error::TooLarge {
                len: u32::MAX,
                max: MAX_DECODED_LEN
            })
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
            decode(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN),
            Err(Error::TooLarge {
                len: over,
                max: MAX_DECODED_LEN
            })
        );
    }

    #[test]
    fn decode_clamps_a_max_len_above_max_decoded_len() {
        // decode is `#[doc(hidden)]` but still `pub`, reachable directly by
        // anything depending on this crate, not just decompress_bounded
        // (which always pre-clamps before calling in). Without decode's own
        // clamp, a caller passing u32::MAX here would relax the ceiling
        // past MAX_DECODED_LEN, the only value this decoder's worst-case
        // decode time has been measured against (S2-A27).
        let over = MAX_DECODED_LEN + 1;
        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&over.to_le_bytes());
        payload.extend_from_slice(&over.to_le_bytes());
        assert_eq!(
            decode(&payload, crate::FORMAT_VERSION, u32::MAX),
            Err(Error::TooLarge {
                len: over,
                max: MAX_DECODED_LEN
            })
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
        models.flag[0].encode(&mut ac, FlagKind::Match.index());
        encode_bucketed(&mut models.length, &mut ac, 4);
        encode_bucketed(&mut models.offset, &mut ac, 1);
        let _ = context;
        let ac_bytes = ac.finish();

        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&4u32.to_le_bytes()); // declared_len
        payload.extend_from_slice(&1u32.to_le_bytes()); // token_count
        payload.extend(ac_bytes);

        assert_eq!(
            decode(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN),
            Err(Error::Corrupt)
        );
    }

    #[test]
    fn ensure_room_rejects_growth_past_declared_len() {
        // Direct, not routed through decode(): output only ever grows, so
        // by the time a full decode loop finishes, the final `output.len()
        // != declared_len` check alone would already reject any payload
        // whose mid-stream growth ensure_room should have caught earlier,
        // with the exact same Err(Corrupt) — a decode()-level test can
        // never tell the two checks apart. This is the only place that
        // pins ensure_room itself is the one doing the rejecting (hard
        // rule 2, its own doc comment: "checked before every write rather
        // than trusted from the header").
        assert_eq!(ensure_room(6, 5, 10), Err(Error::Corrupt));
        assert_eq!(ensure_room(0, 1, 0), Err(Error::Corrupt));
        assert_eq!(ensure_room(5, 5, 10), Ok(()));
        assert_eq!(ensure_room(0, 0, 0), Ok(()));
    }

    #[test]
    fn match_distance_beyond_window_is_rejected() {
        // OFFSET_BUCKETS (21) lets decode_bucketed represent distances up
        // to 2 * lz::WINDOW - 1, wider than any real encoder ever emits
        // (its match finder never searches past lz::WINDOW): a distance
        // one past the window must be rejected on its own, before
        // ensure_room or copy_checked's own bounds checks even run.
        let over_window = u32::try_from(lz::WINDOW).expect("WINDOW fits u32") + 1;
        let mut models = Models::new();
        let mut ac = Encoder::new();
        models.flag[0].encode(&mut ac, FlagKind::Match.index());
        encode_bucketed(&mut models.length, &mut ac, 4);
        encode_bucketed(&mut models.offset, &mut ac, over_window);
        let ac_bytes = ac.finish();

        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&4u32.to_le_bytes()); // declared_len
        payload.extend_from_slice(&1u32.to_le_bytes()); // token_count
        payload.extend(ac_bytes);

        assert_eq!(
            decode(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN),
            Err(Error::Corrupt)
        );
    }

    #[test]
    fn ensure_within_window_accepts_the_boundary_and_rejects_one_past() {
        // The decode-level test above only ever exercises a distance one
        // past the window against an empty output, where copy_checked's own
        // bounds check (distance > output.len()) independently produces the
        // identical Err(Corrupt): #390's mutation sweep found `>` -> `==`
        // and `>` -> `>=` both survive the full suite because of it. Unit
        // testing the extracted comparison directly, at exactly the
        // boundary in both directions, is the only way to tell `>` apart
        // from its neighbors without driving a WINDOW-sized decode.
        let window = u32::try_from(lz::WINDOW).expect("WINDOW fits u32");
        assert_eq!(
            ensure_within_window(NonZeroU32::new(window).expect("WINDOW is not zero")),
            Ok(()),
            "a distance exactly at the window edge is legal"
        );
        assert_eq!(
            ensure_within_window(NonZeroU32::new(window + 1).expect("WINDOW + 1 is not zero")),
            Err(Error::Corrupt),
            "one past the window edge must be rejected"
        );
    }

    #[test]
    fn shorter_than_keeps_the_strictly_smaller_candidate_only() {
        // #390's mutation sweep found `<` -> `==` and `<` -> `<=` both
        // survive the full suite: `encode`'s outer round-trip and
        // filter-selection tests only ever observe the *winning* body, and
        // a wrong tie-break or wrong-direction comparison still produces
        // some valid, round-trippable encoding, just not necessarily the
        // smallest one. Unit testing the extracted comparison directly is
        // the only way to pin "strictly smaller, not tied or larger"
        // without constructing filter candidates whose encoded sizes land
        // on an exact boundary.
        assert!(shorter_than(3, 5), "a strictly smaller body must win");
        assert!(
            !shorter_than(5, 5),
            "a tied body must not displace the earlier candidate"
        );
        assert!(!shorter_than(6, 5), "a larger body must not win");
    }

    #[test]
    fn caller_supplied_max_len_below_max_decoded_len_is_honored() {
        // A frame legal under MAX_DECODED_LEN can still be rejected by a
        // caller's tighter max_len, proving the parameter is a real
        // additional bound, not a synonym for the constant.
        let data = b"the quick brown fox jumps over a lazy dog";
        let encoded = encode(data);
        let declared_len = u32::try_from(data.len()).unwrap();
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION, declared_len - 1),
            Err(Error::TooLarge {
                len: declared_len,
                max: declared_len - 1
            })
        );
        assert_eq!(
            decode(&encoded, crate::FORMAT_VERSION, declared_len).as_deref(),
            Ok(data.as_slice())
        );
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

        let real = encode_tokens(data, None);
        #[allow(
            clippy::cast_precision_loss,
            reason = "encoded length is far below f64's exact integer range (2^53)"
        )]
        let real_bits = ((real.len() - TOKEN_HEADER_LEN) * 8) as f64;

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

    #[test]
    fn ideal_cost_bits_ppm_expert_experiment_is_zero_on_empty_input() {
        let (baseline, with_ppm) = ideal_cost_bits_ppm_expert_experiment(b"");
        assert!(baseline.abs() < 1e-9);
        assert!(with_ppm.abs() < 1e-9);
    }

    #[test]
    fn ideal_cost_bits_ppm_expert_experiment_stays_finite_and_positive() {
        // research/JOURNAL.md S1-P3: no accuracy claim here, just that the
        // paired walk runs to completion and both totals land somewhere
        // sane — the actual accept/reject verdict is a train/sealed
        // measurement recorded in the journal, not a unit test assertion.
        let data: &[u8] = include_bytes!("../research/imports/session-1/mothergod.rs");
        let (baseline, with_ppm) = ideal_cost_bits_ppm_expert_experiment(data);
        assert!(
            baseline.is_finite() && baseline > 0.0,
            "baseline={baseline}"
        );
        assert!(
            with_ppm.is_finite() && with_ppm > 0.0,
            "with_ppm={with_ppm}"
        );
    }

    /// A fresh [`Models`]/[`PpmExpertState`] pair, for asserting
    /// [`PairedTokenSink`]'s ppm-paired literal against a value computed
    /// independently from the same starting state.
    fn fresh_ppm_expert_sink() -> (Models, PpmExpertState) {
        (Models::new(), PpmExpertState::new())
    }

    #[test]
    fn paired_token_sink_non_literal_events_add_the_same_cost_to_both_totals() {
        let mut expected_models = Models::new();
        let expected = expected_models.flag[1].ideal_cost_bits(FlagKind::Match.index())
            + ideal_cost_bucketed(&mut expected_models.length, 17)
            + ideal_cost_bucketed(&mut expected_models.offset, 123)
            + expected_models.slot.ideal_cost_bits(2);

        let mut models = Models::new();
        let mut sink = PairedTokenSink {
            cost: PairedCost::default(),
            pair_literal: |_: &mut Models, _: Context, _: u8| {
                unreachable!("this test never calls TokenSink::literal")
            },
        };
        sink.flag(&mut models, 1, FlagKind::Match);
        sink.length(&mut models, 17);
        sink.offset(&mut models, 123);
        sink.slot(&mut models, 2);

        assert!((sink.cost.baseline - expected).abs() < 1e-9);
        assert!((sink.cost.candidate - expected).abs() < 1e-9);
    }

    #[test]
    fn paired_token_sink_ppm_expert_literal_adds_the_baseline_and_with_ppm_pair_separately() {
        // Enough repeats for the additive PPM bank's own history to pull
        // away from the six-expert mix's baseline, so a sink that swaps or
        // drops one half of the pair is distinguishable from a correct one.
        let bytes = b"aaaaaaaaaaaaaaaaaab";

        let (mut expected_models, mut expected_ppm_state) = fresh_ppm_expert_sink();
        let (mut models, mut ppm_state) = fresh_ppm_expert_sink();
        let mut sink = PairedTokenSink {
            cost: PairedCost::default(),
            pair_literal: |models: &mut Models, context: Context, byte: u8| {
                models
                    .literal
                    .ideal_cost_bits_ppm_expert_pair(context, byte, &mut ppm_state)
            },
        };

        let mut context = Context::default();
        let mut expected_baseline_total = 0.0;
        let mut expected_with_ppm_total = 0.0;
        for &byte in bytes {
            let (baseline, with_ppm) = expected_models.literal.ideal_cost_bits_ppm_expert_pair(
                context,
                byte,
                &mut expected_ppm_state,
            );
            expected_baseline_total += baseline;
            expected_with_ppm_total += with_ppm;
            sink.literal(&mut models, context, byte);
            context = context.after_literal(byte);
        }

        assert!(
            (expected_baseline_total - expected_with_ppm_total).abs() > 1e-6,
            "baseline_total={expected_baseline_total} with_ppm_total={expected_with_ppm_total}, \
             test cannot tell the pair's two halves apart"
        );
        assert!((sink.cost.baseline - expected_baseline_total).abs() < 1e-6);
        assert!((sink.cost.candidate - expected_with_ppm_total).abs() < 1e-6);
    }

    /// `research/JOURNAL.md` S2-A101's guard on the comparison: the pair's
    /// baseline half is [`ideal_cost_bits`]'s own whole-file number, bit for
    /// bit, at any `decay`, so the measured delta is the literal mixer's
    /// alone.
    #[test]
    fn logistic_mix_experiment_baseline_is_exactly_ideal_cost_bits() {
        let data: &[u8] = include_bytes!("../research/imports/session-1/mothergod.rs");
        let expected = ideal_cost_bits(data);
        for decay in [0.0, crate::literal::LOGISTIC_RATE_DECAY, 1.0] {
            let (baseline, candidate) = ideal_cost_bits_logistic_mix_experiment_at(data, decay);
            assert_eq!(baseline.to_bits(), expected.to_bits(), "decay={decay}");
            assert!(
                candidate.is_finite() && candidate > 0.0,
                "decay={decay}: candidate={candidate}"
            );
            assert!(
                (candidate - baseline).abs() > 1e-6,
                "decay={decay}: the candidate half must differ from the baseline"
            );
        }
    }

    #[test]
    fn logistic_mix_experiment_is_zero_on_empty_input() {
        let (baseline, candidate) = ideal_cost_bits_logistic_mix_experiment(b"");
        assert!(baseline.abs() < 1e-9);
        assert!(candidate.abs() < 1e-9);
    }

    #[test]
    fn logistic_mix_experiment_defaults_to_the_registered_decay() {
        let data = b"abcabcabd abcabcabe abcabcabf".repeat(8);
        let via_default = ideal_cost_bits_logistic_mix_experiment(&data);
        let via_at =
            ideal_cost_bits_logistic_mix_experiment_at(&data, crate::literal::LOGISTIC_RATE_DECAY);
        assert_eq!(via_default.0.to_bits(), via_at.0.to_bits());
        assert_eq!(via_default.1.to_bits(), via_at.1.to_bits());
        let via_other = ideal_cost_bits_logistic_mix_experiment_at(&data, 0.0);
        assert_ne!(via_default.1.to_bits(), via_other.1.to_bits());
    }

    #[test]
    fn paired_token_sink_logistic_mix_literal_adds_the_baseline_and_candidate_separately() {
        let bytes = b"aaaaaaaaaaaaaaaaaab";
        let mut expected_models = Models::new();
        let mut expected_logistic = LogisticMix::new();
        let mut models = Models::new();
        let mut logistic = LogisticMix::new();
        let mut sink = PairedTokenSink {
            cost: PairedCost::default(),
            pair_literal: |models: &mut Models, context: Context, byte: u8| {
                models
                    .literal
                    .ideal_cost_bits_logistic_pair(context, byte, &mut logistic, 4e-4)
            },
        };
        let mut context = Context::default();
        let mut expected_baseline_total = 0.0;
        let mut expected_candidate_total = 0.0;
        for &byte in bytes {
            let (baseline, candidate) = expected_models.literal.ideal_cost_bits_logistic_pair(
                context,
                byte,
                &mut expected_logistic,
                4e-4,
            );
            expected_baseline_total += baseline;
            expected_candidate_total += candidate;
            sink.literal(&mut models, context, byte);
            context = context.after_literal(byte);
        }
        assert!(
            (expected_baseline_total - expected_candidate_total).abs() > 1e-6,
            "test cannot tell the pair's two halves apart"
        );
        assert_eq!(
            sink.cost.baseline.to_bits(),
            expected_baseline_total.to_bits()
        );
        assert_eq!(
            sink.cost.candidate.to_bits(),
            expected_candidate_total.to_bits()
        );
    }

    #[test]
    fn ideal_cost_bits_with_window_drops_once_a_repeat_becomes_reachable() {
        // research/JOURNAL.md S1-P4: a repeat past the window is priced as
        // fresh literals; the same repeat within a larger window is priced
        // as a match, so the real adaptive models must report meaningfully
        // fewer bits once `window` grows to reach it. Same disjoint
        // template/filler byte ranges as lz::tests::
        // optimal_with_window_reaches_a_repeat_a_smaller_window_would_miss,
        // so the only structure in `data` is the planted repeat itself.
        let template: Vec<u8> = (0u8..80).collect();
        let filler: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED)
            .take(420)
            .map(|s| 128 + u8::try_from(s % 128).unwrap())
            .collect();
        let mut data = template.clone();
        data.extend_from_slice(&filler);
        data.extend_from_slice(&template);

        let small_window = 300;
        let large_window = 700;
        let small_bits = ideal_cost_bits_with_window(&data, small_window);
        let large_bits = ideal_cost_bits_with_window(&data, large_window);
        assert!(
            large_bits < small_bits - f64::from(u16::try_from(template.len()).unwrap()),
            "a window reaching the planted repeat ({large_bits} bits) should cost at least a \
             template's worth of bits less than one that cannot ({small_bits} bits)"
        );
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "both sides run the identical token stream through the identical models \
                  (lz::parse_optimal_adaptive_window's gate is unconditionally false for \
                  data.len() <= lz::WINDOW), so exact equality, not tolerance, is the claim"
    )]
    fn ideal_cost_bits_adaptive_window_matches_wired_window_within_window() {
        // research/JOURNAL.md S1-P4's next slice after S2-A92:
        // lz::parse_optimal_adaptive_window's own gate is unconditionally
        // false for data.len() <= lz::WINDOW (lz::tests::
        // adaptive_window_matches_wired_window_within_window proves the
        // token streams are identical at that scale), so the models-wired
        // cost through this codec-layer entry point must match the wired
        // one exactly too, not just approximately.
        let data: Vec<u8> = (0..5000u32)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();
        assert_eq!(
            ideal_cost_bits_adaptive_window(&data),
            ideal_cost_bits(&data)
        );
    }

    #[test]
    fn compressed_len_with_window_matches_wired_encode_tokens_at_wired_window() {
        // Real-Encoder counterpart to the fixed-window ideal-cost test
        // above: lz::parse_optimal(data) is defined as
        // parse_optimal_with_window(data, WINDOW), so
        // compressed_len_with_window at that same window must match
        // encode_tokens's own wired length exactly, not just
        // approximately.
        let data: Vec<u8> = (0..5000u32)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();
        assert_eq!(
            compressed_len_with_window(&data, lz::WINDOW),
            encode_tokens(&data, None).len()
        );
    }

    #[test]
    fn compressed_len_adaptive_window_matches_wired_encode_tokens_within_window() {
        // Real-Encoder counterpart to
        // ideal_cost_bits_adaptive_window_matches_wired_window_within_window,
        // same reason: lz::parse_optimal_adaptive_window's gate is
        // unconditionally false for data.len() <= lz::WINDOW, so this
        // capability's real compressed length must match encode_tokens's
        // own wired length exactly, not just approximately, below that
        // scale. Not yet a decode()-level round-trip test: the whole point
        // of this capability (research/JOURNAL.md S1-P4) is measuring a
        // window past lz::WINDOW, and encode_tokens is not wired to use
        // one, so ensure_within_window still rejects a payload that
        // exercises it — the same caveat ideal_cost_bits_adaptive_window
        // has always carried.
        let data: Vec<u8> = (0..5000u32)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();
        assert_eq!(
            compressed_len_adaptive_window(&data),
            encode_tokens(&data, None).len()
        );
    }

    #[test]
    fn compressed_len_with_seed_and_search_window_matches_matched_window_call() {
        // compressed_len_with_window(data, w) is
        // parse_optimal_with_seed_and_search_window(data, lz::WINDOW, w)
        // by lz::parse_optimal_with_window's own documented contract, so
        // this capability must match it exactly at seed_window = lz::WINDOW,
        // not just approximately.
        let data: Vec<u8> = (0..5000u32)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();
        assert_eq!(
            compressed_len_with_seed_and_search_window(&data, lz::WINDOW, lz::WINDOW),
            compressed_len_with_window(&data, lz::WINDOW)
        );
    }

    #[test]
    fn streaming_rejects_bad_match_distance_not_panicking() {
        // Same hand-crafted single-Match-token payload as
        // bad_match_distance_is_rejected_not_panicking, driven through
        // decode_to_writer's own loop instead of decode's: the two loops
        // are separate code, so this checks the new one's wiring directly
        // rather than trusting decode's coverage to also prove it.
        let mut models = Models::new();
        let mut ac = Encoder::new();
        models.flag[0].encode(&mut ac, FlagKind::Match.index());
        encode_bucketed(&mut models.length, &mut ac, 4);
        encode_bucketed(&mut models.offset, &mut ac, 1);
        let ac_bytes = ac.finish();

        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&4u32.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend(ac_bytes);

        let err = decode_streaming(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN)
            .expect_err("distance reaching before the start of output must be rejected");
        assert_eq!(as_codec_error(&err), Some(&Error::Corrupt));
    }

    #[test]
    fn streaming_rejects_match_distance_beyond_window() {
        let over_window = u32::try_from(lz::WINDOW).expect("WINDOW fits u32") + 1;
        let mut models = Models::new();
        let mut ac = Encoder::new();
        models.flag[0].encode(&mut ac, FlagKind::Match.index());
        encode_bucketed(&mut models.length, &mut ac, 4);
        encode_bucketed(&mut models.offset, &mut ac, over_window);
        let ac_bytes = ac.finish();

        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&4u32.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend(ac_bytes);

        let err = decode_streaming(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN)
            .expect_err("a distance past lz::WINDOW must be rejected");
        assert_eq!(as_codec_error(&err), Some(&Error::Corrupt));
    }

    #[test]
    fn streaming_rejects_declared_length_over_the_max_before_any_work() {
        let over = MAX_DECODED_LEN + 1;
        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&over.to_le_bytes());
        payload.extend_from_slice(&over.to_le_bytes());
        let err = decode_streaming(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN)
            .expect_err("declared length past MAX_DECODED_LEN must be rejected");
        assert_eq!(
            as_codec_error(&err),
            Some(&Error::TooLarge {
                len: over,
                max: MAX_DECODED_LEN
            })
        );
    }

    #[test]
    fn streaming_falls_back_to_decode_for_transpose_declared_length_over_the_max() {
        // Same amplification-hazard shape as the Identity case above, but
        // routed through decode_to_writer's fallback branch (the one
        // candidate, Transpose, that still needs decode's own whole-buffer
        // path), which must reject before calling it, not after. Identity,
        // Delta, and Bcj all stream instead (research/JOURNAL.md S1-P7).
        let over = MAX_DECODED_LEN + 1;
        let mut payload = Candidate::Transpose(crate::test_support::nz(2))
            .to_header_bytes()
            .to_vec();
        payload.extend_from_slice(&over.to_le_bytes());
        payload.extend_from_slice(&over.to_le_bytes());
        let err = decode_streaming(&payload, crate::FORMAT_VERSION, MAX_DECODED_LEN)
            .expect_err("declared length past MAX_DECODED_LEN must be rejected");
        assert_eq!(
            as_codec_error(&err),
            Some(&Error::TooLarge {
                len: over,
                max: MAX_DECODED_LEN
            })
        );
    }

    #[test]
    fn streaming_propagates_the_writer_error_unwrapped() {
        // Distinguishes the two error origins decode_to_writer's docs
        // promise: a decode error comes back as WriteError::Decode
        // (as_codec_error unwraps it above), but a failure from the writer
        // itself must come back as WriteError::Io, carrying exactly what
        // the writer produced, not re-wrapped.
        struct FailingWriter;
        impl std::io::Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let data = b"abcdefgh".repeat(50);
        let encoded = encode(&data);
        assert_eq!(
            encoded[0], 0,
            "expected Candidate::Identity for this fixture"
        );
        let mut writer = FailingWriter;
        let err = decode_to_writer(
            &encoded,
            crate::FORMAT_VERSION,
            MAX_DECODED_LEN,
            &mut writer,
        )
        .expect_err("a writer that always fails must surface its error");
        assert!(
            as_codec_error(&err).is_none(),
            "a writer failure is not a decode Error and must not downcast to one"
        );
        match err {
            crate::WriteError::Io(err) => assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe),
            crate::WriteError::Decode(err) => {
                panic!("expected WriteError::Io, got Decode({err:?})")
            }
        }
    }
}
