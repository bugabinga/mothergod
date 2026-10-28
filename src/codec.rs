//! `Method::Lz` wiring (`research/JOURNAL.md` S2-D2):
//! [`crate::filters::select::pick`] shortlists candidate filters, each is
//! trial-encoded through the optimal-parse LZ parser ([`crate::lz`]), the
//! flag/length/offset/rep-slot adaptive tables (`crate::model`), the
//! six-expert literal mixer ([`crate::literal`]), and the range coder
//! (`crate::coder`), and whichever candidate produces the smallest payload
//! wins.
//!
//! # Payload layout
//!
//! ```text
//! offset  size  field
//! 0       2     selector: [kind | reach << 7, param] (`filters::select::Candidate`, `lz::Reach`)
//! 2       4     declared length of the filtered bytes, u32 LE
//! 6       4     token count, u32 LE
//! 10      8     range-coded stream length in bytes, u64 LE
//! 18      ...   range-coded stream (crate::coder), of the FILTERED bytes
//! ```
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
//! length code through two independent `Model`s
//! (`docs/adr/0057-wire-the-match-rep-length-model-split.md`), the bits
//! below a length's `lz::bucket` through an adaptive tree per model and
//! bucket (`docs/adr/0061-wire-the-length-residual-trees.md`), and a
//! [`Token::Match`]'s `offset` (its distance) through one of four
//! independent models selected by a coarse bucket of that match's length
//! (`docs/adr/0058-wire-the-offset-length-state-split.md`); a distance's
//! residual bits stay raw. `slot` codes through one model.
//!
//! The declared length is [`decode`]'s allocation bound
//! (`docs/format/SPEC.md`, `rust-craft` skill's allocation-discipline): a
//! hostile payload can claim any token count or any match/rep length, but
//! [`decode`] rejects the token that would grow its output buffer past this
//! field. What the frame decodes to, [`Candidate::output_len`] of that
//! field, is itself capped at [`decode`]'s caller-supplied
//! `max_len`, in turn capped at [`MAX_DECODED_LEN`] by
//! [`crate::decompress_bounded`], so a tiny payload cannot declare an
//! unbounded length and force unbounded decode work; see [`MAX_DECODED_LEN`]'s
//! docs for why, and [`decode`]'s docs for the rest.

use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use crate::coder::{Decoder, Encoder};
use crate::column;
use crate::filters::{
    self,
    select::{Candidate, Streaming},
};
use crate::literal::{ColumnExpertState, Context, Literal, SurpriseLogisticMix};
use crate::lz::{self, Reach, RepCache, RepSlot, Token};
use crate::model::Model;
use crate::{Error, Level, rejoin};

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

/// Largest output length [`decode`] accepts, checked before any
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
/// moving: this format's models saturate fast enough that a real
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
/// a cheap `CopyKind` branch or array index per copy token, strictly
/// cheaper than the per-byte literal cost this bound is measured against.
/// It bounds one `Lz` frame's output and the total `Lz` output of a stream
/// [`crate::decompress`] buffers; it is no file-size ceiling, because
/// [`crate::compress`] cuts its input into 16 MiB frames up to this length
/// (past it, one `Stored` frame) and [`crate::decompress_to_writer`]
/// carries a stream of any length at a `max_len` of this or more
/// (ADR-0067). The constant itself is a `research/JOURNAL.md` S1-P6
/// speed-tier target, not something to chase down here.
pub const MAX_DECODED_LEN: u32 = 256 * 1024 * 1024;

/// Which of the three kinds a token codes as: the flag symbol coded
/// before every token.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FlagKind {
    Literal,
    Match,
    Rep,
}

impl FlagKind {
    /// The [`Model`] symbol this kind codes as.
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

/// The two token kinds that code a copy length, each through models of its
/// own ([`length_models`]).
#[derive(Clone, Copy)]
enum CopyKind {
    Match,
    Rep,
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
    logit_sse: SurpriseLogisticMix,
    /// One flag table per "was the previous token a copy" state: a literal
    /// run and a post-copy position have different flag distributions worth
    /// modeling separately. Indexed by [`Context::after_copy`].
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
        crate::abort_on_oom(Self::try_new())
    }

    /// The one statement of the fresh tables, returning `Err` if the
    /// allocator cannot satisfy one of them. [`decode`] and
    /// [`decode_to_writer`] use this (hard rule 2, `rust-craft` skill's
    /// allocation-discipline, `tests/torture.rs`, #453); every encode path
    /// keeps [`Self::new`], this with an abort, where an allocator failure
    /// this small is already a fatal condition for the whole process.
    fn try_new() -> Result<Self, std::collections::TryReserveError> {
        Ok(Self {
            literal: Literal::try_new()?,
            logit_sse: SurpriseLogisticMix::try_new()?,
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

/// A copy token's length models, bucket [`Model`] and the [`ResidualTree`]
/// beside it: [`Models::length_match`] and its residual for
/// [`CopyKind::Match`], `length_rep` and its residual for [`CopyKind::Rep`].
/// [`EncodeSink`] and every [`DecodeSink`] select through here.
fn length_models(models: &mut Models, kind: CopyKind) -> (&mut Model, &mut ResidualTree) {
    match kind {
        CopyKind::Match => (&mut models.length_match, &mut models.length_match_residual),
        CopyKind::Rep => (&mut models.length_rep, &mut models.length_rep_residual),
    }
}

/// Applies `candidate`'s filter to `data`, or returns a copy of it
/// unchanged for [`Candidate::Identity`]. Every filter but
/// [`Candidate::Base64`] preserves length, so the result is `data.len()`
/// bytes; `Base64`'s is the decoded binary, which
/// [`Candidate::output_len`] inverts.
///
/// # Panics
///
/// If `candidate` is [`Candidate::Base64`] and `data` is not canonical
/// base64: [`filters::select::pick`] shortlists it only for data
/// [`filters::base64_unwrap::encode`] accepted.
fn apply_filter(candidate: Candidate, data: &[u8]) -> Vec<u8> {
    match candidate {
        Candidate::Identity => data.to_vec(),
        Candidate::Delta(stride) => filters::delta::encode(data, stride),
        Candidate::Bcj => filters::bcj::encode(data),
        Candidate::Transpose(columns) => filters::transpose::encode(data, columns),
        Candidate::Base64 => filters::base64_unwrap::encode(data)
            .expect("pick() shortlists Base64 only for data base64_unwrap::encode accepted"),
        Candidate::Reverse => filters::reverse::encode(data),
        Candidate::ReverseDelta(stride) => {
            filters::delta::encode(&filters::reverse::encode(data), stride)
        }
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
fn undo_filter(candidate: Candidate, mut data: Vec<u8>) -> Result<Vec<u8>, Error> {
    match candidate {
        Candidate::Identity => Ok(data),
        Candidate::Delta(stride) => Ok(filters::delta::try_decode(&data, stride)?),
        Candidate::Bcj => Ok(filters::bcj::try_decode(&data)?),
        Candidate::Transpose(columns) => Ok(filters::transpose::try_decode(&data, columns)?),
        Candidate::Base64 => Ok(filters::base64_unwrap::try_decode(&data)?),
        Candidate::Reverse => {
            // Self-inverse and length-preserving, so it reverses the owned
            // buffer in place and allocates nothing.
            data.reverse();
            Ok(data)
        }
        Candidate::ReverseDelta(stride) => {
            let mut undone = filters::delta::try_decode(&data, stride)?;
            undone.reverse();
            Ok(undone)
        }
    }
}

/// [`encode_tokens`]'s walk: codes `tokens` (already parsed from `data`) in
/// order through `sink`, advancing the literal-model context exactly as
/// [`Context::after_literal`]/[`Context::after_copy`] require.
fn walk_tokens(tokens: &[Token], data: &[u8], models: &mut Models, sink: &mut EncodeSink<'_>) {
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
                sink.length(models, CopyKind::Match, len);
                sink.offset(models, len, distance.get());
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
            Token::Rep { len, slot } => {
                sink.flag(models, flag_table, FlagKind::Rep);
                sink.slot(models, slot.index());
                sink.length(models, CopyKind::Rep, len);
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
        }
    }
}

/// The column-coding state of a [`Candidate::Transpose`] frame, in
/// [`EncodeSink`] and [`VecSink`] alike: `columns` is the candidate's own
/// param, `data_len` is the filtered data's length (`crate::column::column_of`'s
/// `len`, always the pre-filter length too since a `Transpose` frame's filter
/// preserves length), and `state` is the seventh expert's own fresh
/// bank/weight/SSE state for this one trial encode or frame decode.
struct ColumnCoding {
    columns: NonZeroUsize,
    data_len: usize,
    state: ColumnExpertState,
}

impl ColumnCoding {
    /// The [`ColumnExpertState`] bank the literal at `position` of the
    /// filtered stream codes under.
    fn bank(&self, position: usize) -> usize {
        column::bank_of(position, self.columns, self.data_len, MAX_COLUMN_BANKS)
    }
}

/// Codes a token's flag/length/offset/slot symbols and literal bytes through
/// a real [`Encoder`], for [`walk_tokens`]. `column` is `Some` exactly when
/// the candidate under trial is [`Candidate::Transpose`] (`encode`'s caller),
/// selecting [`crate::literal::Literal::encode_column`] over
/// [`crate::literal::Literal::encode_logit_sse`] for every literal in this
/// trial (`research/JOURNAL.md` S1-P5).
struct EncodeSink<'a> {
    ac: &'a mut Encoder,
    column: Option<ColumnCoding>,
}

impl EncodeSink<'_> {
    fn flag(&mut self, models: &mut Models, flag_table: usize, kind: FlagKind) {
        models.flag[flag_table].encode(self.ac, kind.index());
    }

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        match &mut self.column {
            Some(col) => {
                let bank = col.bank(context.position);
                models
                    .literal
                    .encode_column(self.ac, context, byte, bank, &mut col.state);
            }
            None => {
                models
                    .literal
                    .encode_logit_sse(self.ac, context, byte, &mut models.logit_sse);
            }
        }
    }

    fn length(&mut self, models: &mut Models, kind: CopyKind, value: u32) {
        let (bucket, residual) = length_models(models, kind);
        bucket.encode(self.ac, lz::bucket(value));
        residual.encode(self.ac, value);
    }

    /// The [`lz::bucket`] symbol through the model [`offset_len_state`]
    /// selects, then `value`'s residual low bits within that bucket as raw,
    /// unmodeled bits. `len` is the triggering [`Token::Match`]'s own
    /// already-emitted length: [`walk_tokens`] never calls this for a
    /// [`Token::Rep`] (which has no `offset` symbol at all) or a
    /// [`Token::Literal`].
    fn offset(&mut self, models: &mut Models, len: u32, value: u32) {
        let b = lz::bucket(value);
        models.offset_len[offset_len_state(len)].encode(self.ac, b);
        self.ac.encode_bits(value, bucket_bits(b));
    }

    fn slot(&mut self, models: &mut Models, symbol: usize) {
        models.slot.encode(self.ac, symbol);
    }
}

/// Encodes already-filtered `data` through the LZ + context-mixing
/// pipeline: [`encode`]'s per-candidate trial body. `columns` is `Some`
/// exactly when this trial's candidate is [`Candidate::Transpose`] with that
/// column count, selecting the column-expert literal path for the whole
/// trial (see [`EncodeSink`]).
/// `reach` bounds the parse's match distances and is the one the payload's
/// selector must declare ([`selector`]).
/// Panics under the same condition as [`encode_tokens_with`], which this
/// delegates to.
fn encode_tokens(data: &[u8], columns: Option<NonZeroUsize>, reach: Reach) -> Vec<u8> {
    encode_tokens_with(data, columns, &lz::parse_optimal_with_window(data, reach))
}

/// Shared body of [`encode_tokens`] and the codec tests: encodes already-parsed
/// `tokens` through the context-mixing pipeline and returns the real payload
/// bytes.
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
    let column = columns.map(|columns| ColumnCoding {
        columns,
        data_len: data.len(),
        state: ColumnExpertState::new(MAX_COLUMN_BANKS),
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

    let stream = ac.finish();
    let coded_len = u64::try_from(stream.len()).expect("usize is at most 64 bits");
    let mut out = Vec::with_capacity(TOKEN_HEADER_LEN + stream.len());
    out.extend_from_slice(&declared_len.to_le_bytes());
    out.extend_from_slice(&token_count.to_le_bytes());
    out.extend_from_slice(&coded_len.to_le_bytes());
    out.extend(stream);
    out
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
    /// A fresh tree, or `Err` if the allocator refuses (hard rule 2).
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

    /// Residual bits of `value` sent raw, the ones [`Self::walk`] leaves to
    /// its caller.
    fn raw_bits(value: u32) -> u32 {
        Self::split(lz::bucket(value)).1
    }

    /// Hands `code` each node on the path of `value`'s modeled residual
    /// bits, most significant first, with the bit taken there: the one walk
    /// that [`Self::encode`] codes.
    fn walk(&mut self, value: u32, mut code: impl FnMut(&mut Model, usize)) {
        let b = lz::bucket(value);
        let (modeled, raw) = Self::split(b);
        let base = b << RESIDUAL_MODELED_BITS;
        let mut node = 1usize;
        for shift in (raw..raw + modeled).rev() {
            let bit = ((value >> shift) & 1) as usize;
            code(&mut self.nodes[base + node], bit);
            node = node * 2 + bit;
        }
    }

    /// Codes `value`'s residual bits below its [`lz::bucket`]: the top
    /// `min(b, RESIDUAL_MODELED_BITS)` through this tree, most significant
    /// first, then the rest raw.
    fn encode(&mut self, ac: &mut Encoder, value: u32) {
        self.walk(value, |model, bit| model.encode(ac, bit));
        ac.encode_bits(value, Self::raw_bits(value));
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
}

/// Encodes `data` into a `Method::Lz` payload: trials every candidate
/// filter [`filters::select::pick`] shortlists at [`Reach::Standard`], keeps
/// whichever produces the smallest `encode_tokens` body, and prefixes that
/// body with the winning candidate's 2-byte selector (see the module docs'
/// "Payload layout"). Each candidate is applied to the raw input: one
/// filter, or [`Candidate::ReverseDelta`], the one fixed composition.
///
/// Input longer than [`lz::WINDOW`] gets one more trial, the winning
/// candidate at [`Reach::Wide`], kept only when strictly smaller: the
/// repeats between 1 and 2 MiB back that real files have shrink them, and
/// where the wide parse loses (`research/JOURNAL.md` S2-R35's
/// `interleaved_audio16`, `access_log`, `gradient_image`) the payload is the
/// one a narrow encode writes. A shorter input has nothing to reach, so it
/// pays no trial.
///
/// Up to `workers` trials run at once, the calling thread being one of them.
/// The trials are independent encodes, so the payload is the one a serial
/// search writes, a tie going to the earlier candidate; each running trial
/// holds a working set of its own, which is what bounds the caller's choice
/// of `workers` (`research/JOURNAL.md` S2-A126).
///
/// # Panics
///
/// Panics if `data.len()` exceeds `u32::MAX`; see `encode_tokens`'s
/// docs, which this delegates to per candidate.
#[must_use]
pub fn encode(data: &[u8], workers: NonZeroUsize) -> Vec<u8> {
    encode_level(data, workers, Level::Best)
}

/// [`encode`] at `level`. [`Level::Best`] is the search [`encode`]
/// documents; the other two skip it and trial the shortlist's first
/// candidate alone (`quick_trial`), so `workers` has nothing to spread and
/// goes unused. Every level writes a payload [`decode`] reads the same way:
/// the selector declares the filter and the reach, and nothing in the
/// bitstream says how the parse was found.
///
/// # Panics
///
/// Same as [`encode`].
#[must_use]
pub fn encode_level(data: &[u8], workers: NonZeroUsize, level: Level) -> Vec<u8> {
    let candidates = filters::select::pick(data);
    let first = candidates.first().copied();
    let (candidate, reach, body) = match level {
        Level::Best => search(data, &candidates, workers),
        Level::Fast => first.map(|candidate| {
            let reach = if data.len() > lz::WINDOW {
                Reach::Wide
            } else {
                Reach::Standard
            };
            quick_trial(data, candidate, reach, 1)
        }),
        Level::Fastest => first.map(|candidate| quick_trial(data, candidate, Reach::Standard, 0)),
    }
    .expect("filters::select::pick always returns at least Candidate::Identity");
    let mut out = Vec::with_capacity(SELECTOR_LEN + body.len());
    out.extend_from_slice(&selector(candidate, reach));
    out.extend(body);
    out
}

/// A finished trial: the candidate's position in the shortlist, the
/// candidate, and its body at [`Reach::Standard`].
type Winner = (usize, Candidate, Vec<u8>);

/// One trial: `data` through `candidate`'s filter, then [`encode_tokens`] at
/// `reach`. The one statement of what a trial is, for [`search`]'s narrow
/// trials and the wide trial [`encode`] documents alike.
fn trial_body(data: &[u8], candidate: Candidate, reach: Reach) -> Vec<u8> {
    encode_tokens(&apply_filter(candidate, data), candidate.columns(), reach)
}

/// The one trial [`Level::Fast`] and [`Level::Fastest`] run, as `(candidate,
/// reach, body)`: no search over filters, and `rounds` DP rounds where
/// [`encode`]'s parse runs [`lz::DP_ROUNDS`]. [`Level::Fast`] passes one and
/// reaches wide on input longer than [`lz::WINDOW`], where [`encode`] pays a
/// second trial to find out whether wide wins; [`Level::Fastest`] passes
/// zero, [`lz::parse_greedy`], which never sees past the standard window and
/// so declares [`Reach::Standard`].
fn quick_trial(
    data: &[u8],
    candidate: Candidate,
    reach: Reach,
    rounds: usize,
) -> (Candidate, Reach, Vec<u8>) {
    let filtered = apply_filter(candidate, data);
    let tokens = lz::parse_rounds(&filtered, reach, rounds);
    let body = encode_tokens_with(&filtered, candidate.columns(), &tokens);
    (candidate, reach, body)
}

/// The winner among `candidates` as `(candidate, reach, body)`: the first
/// candidate in the list to reach the smallest body at [`Reach::Standard`],
/// what a serial loop that replaces its incumbent only on a strictly smaller
/// body keeps, then the wide trial [`encode`] documents. `None` for no
/// candidates.
///
/// Up to `workers` threads take the next untried candidate until none is
/// left; each keeps its own winner, which is its first smallest because it
/// takes candidates in list order, and the merge by list position then picks
/// the global first smallest.
///
/// A thread that finds none left while another trial still runs would idle
/// until the winner is known, so it starts the wide trial of the smallest
/// body finished so far. That guess is the winner when the running trial
/// loses, and then the wide body is ready when the narrow search ends; when
/// it is not, the real wide trial starts at once beside the wasted one, so
/// the working sets stay at `workers` and the payload is the serial one
/// either way (`research/JOURNAL.md` S2-A129).
fn search(
    data: &[u8],
    candidates: &[Candidate],
    workers: NonZeroUsize,
) -> Option<(Candidate, Reach, Vec<u8>)> {
    let sizes: Vec<AtomicUsize> = candidates
        .iter()
        .map(|_| AtomicUsize::new(PENDING))
        .collect();
    let next = AtomicUsize::new(0);
    let speculating = AtomicBool::new(false);
    let has_wide_trial = data.len() > lz::WINDOW;

    thread::scope(|scope| {
        let work = || -> (Option<Winner>, Option<(usize, WideTrial<'_>)>) {
            let mut best: Option<Winner> = None;
            loop {
                // Relaxed: the counter hands out each index once and orders
                // nothing else.
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(&candidate) = candidates.get(index) else {
                    break;
                };
                let body = trial_body(data, candidate, Reach::Standard);
                sizes[index].store(body.len(), Ordering::Relaxed);
                if best.as_ref().is_none_or(|(_, _, existing)| {
                    crate::candidate_beats_incumbent(body.len(), existing.len())
                }) {
                    best = Some((index, candidate, body));
                }
            }
            let guess = if has_wide_trial && sizes.iter().any(|size| finished(size).is_none()) {
                leader(&sizes).filter(|_| !speculating.swap(true, Ordering::Relaxed))
            } else {
                None
            };
            // A refused spawn (the process is out of threads) is no guess.
            let trial = guess.and_then(|index| {
                let candidate = candidates[index];
                thread::Builder::new()
                    .spawn_scoped(scope, move || trial_body(data, candidate, Reach::Wide))
                    .ok()
                    .map(|handle| (index, handle))
            });
            (best, trial)
        };

        // A refused spawn leaves the work to the threads that did start,
        // this one included.
        let helpers: Vec<_> = (1..workers.get().min(candidates.len()))
            .filter_map(|_| thread::Builder::new().spawn_scoped(scope, work).ok())
            .collect();
        let (own, own_trial) = work();
        let mut winners: Vec<Winner> = own.into_iter().collect();
        let mut trial = own_trial;
        for helper in helpers {
            let (winner, helper_trial) = rejoin(helper.join());
            winners.extend(winner);
            trial = trial.or(helper_trial);
        }
        // Smallest body, a tie to the earlier candidate: the serial rule.
        let winner = winners
            .into_iter()
            .min_by_key(|(index, _, body)| (body.len(), *index))?;
        Some(if has_wide_trial {
            widen(data, winner, trial)
        } else {
            let (_, candidate, body) = winner;
            (candidate, Reach::Standard, body)
        })
    })
}

/// [`search`]'s last step: the narrow `winner` against its wide trial, the
/// wide body kept only when strictly smaller. `trial` is the early guess
/// and the index it was made for; it is joined when that index is the
/// winner's, and any other guess is left running and joined when the scope
/// ends, the real wide trial running here instead. Either way the result is
/// what a serial search writes.
fn widen(
    data: &[u8],
    (index, candidate, body): Winner,
    trial: Option<(usize, WideTrial<'_>)>,
) -> (Candidate, Reach, Vec<u8>) {
    let wide = match trial {
        Some((guessed, handle)) if guessed == index => rejoin(handle.join()),
        _ => trial_body(data, candidate, Reach::Wide),
    };
    if crate::candidate_beats_incumbent(wide.len(), body.len()) {
        (candidate, Reach::Wide, wide)
    } else {
        (candidate, Reach::Standard, body)
    }
}

/// A wide trial running on a thread of [`search`]'s scope.
type WideTrial<'scope> = thread::ScopedJoinHandle<'scope, Vec<u8>>;

/// What a trial's size slot holds while it runs or waits; a body never has
/// that length.
const PENDING: usize = usize::MAX;

/// The body size a trial's slot holds, `None` while it is [`PENDING`].
/// Relaxed: the guess it feeds never changes the payload, only what runs
/// early.
fn finished(size: &AtomicUsize) -> Option<usize> {
    Some(size.load(Ordering::Relaxed)).filter(|&len| len != PENDING)
}

/// The first candidate among the finished trials to reach the smallest
/// body, the serial loop's rule; `None` when no trial has finished.
fn leader(sizes: &[AtomicUsize]) -> Option<usize> {
    sizes
        .iter()
        .enumerate()
        .filter_map(|(index, size)| Some((index, finished(size)?)))
        .min_by_key(|&(_, size)| size)
        .map(|(index, _)| index)
}

/// Byte length of [`encode_tokens_with`]'s header: declared output length
/// and token count, each a little-endian `u32`, then the range-coded
/// stream's byte length as a little-endian `u64` (this module's own
/// "Payload layout" doc names the fields), right before the stream.
/// [`read_header`] inverts it exactly, so a caller sizing a buffer around
/// that header (encode's own capacity hint, a test computing
/// coded-bits-past-header) reads this constant instead of re-deriving it.
/// The stream length is a `u64` for the reason a `Stored` payload's is
/// (ADR-0065): no bound short of `u64` is proved for what the coder emits.
const TOKEN_HEADER_LEN: usize = 2 * size_of::<u32>() + size_of::<u64>();

/// Byte length of the selector that opens a `Method::Lz` payload.
const SELECTOR_LEN: usize = 2;

/// The bit of a selector's `kind` byte that declares [`Reach::Wide`];
/// [`Candidate::to_header_bytes`] leaves it clear, its kinds being under
/// 128.
const WIDE_REACH_BIT: u8 = 0x80;

/// The 2-byte selector that opens a `Method::Lz` payload: `candidate`'s
/// bytes, `kind` carrying `reach` in its top bit.
fn selector(candidate: Candidate, reach: Reach) -> [u8; SELECTOR_LEN] {
    let [kind, param] = candidate.to_header_bytes();
    let wide = match reach {
        Reach::Standard => 0,
        Reach::Wide => WIDE_REACH_BIT,
    };
    [kind | wide, param]
}

/// Splits a `Method::Lz` payload into the filter [`Candidate`] and the
/// [`Reach`] its 2-byte selector names, and the rest, which [`read_header`]
/// expects.
///
/// # Errors
///
/// Returns [`Error::Truncated`] if `payload` is shorter than the selector,
/// [`Error::Corrupt`] if the selector names no [`Candidate`].
pub(crate) fn split_selector(payload: &[u8]) -> Result<(Candidate, Reach, &[u8]), Error> {
    let (&[kind, param], rest) = payload.split_first_chunk().ok_or(Error::Truncated)?;
    let reach = if kind & WIDE_REACH_BIT == 0 {
        Reach::Standard
    } else {
        Reach::Wide
    };
    let candidate =
        Candidate::from_header_bytes([kind & !WIDE_REACH_BIT, param]).ok_or(Error::Corrupt)?;
    Ok((candidate, reach, rest))
}

/// The three fields of an `Lz` payload's header, before the stream.
struct TokenHeader {
    declared_len: u32,
    token_count: u32,
    coded_len: u64,
}

/// Reads the [`TOKEN_HEADER_LEN`]-byte header off the front of `rest` (a
/// payload past its filter selector), returning it and the bytes after it.
fn split_token_header(rest: &[u8]) -> Result<(TokenHeader, &[u8]), Error> {
    let (&declared_len, rest) = rest.split_first_chunk().ok_or(Error::Truncated)?;
    let (&token_count, rest) = rest.split_first_chunk().ok_or(Error::Truncated)?;
    let (&coded_len, rest) = rest.split_first_chunk().ok_or(Error::Truncated)?;
    let header = TokenHeader {
        declared_len: u32::from_le_bytes(declared_len),
        token_count: u32::from_le_bytes(token_count),
        coded_len: u64::from_le_bytes(coded_len),
    };
    Ok((header, rest))
}

/// What a frame's framing needs of a `Method::Lz` payload without decoding
/// it: how many bytes it spans and how many it decodes to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Extent {
    /// Bytes the payload spans: selector, header and range-coded stream.
    pub(crate) payload_len: usize,
    /// What the payload decodes to: the declared length of the filtered
    /// bytes, put through the selector's [`Candidate::output_len`].
    pub(crate) output_len: u64,
}

/// Bytes at the front of a `Method::Lz` payload that [`declared_extent`]
/// reads: the selector and the header, before the range-coded stream.
pub(crate) const EXTENT_PREFIX_LEN: usize = SELECTOR_LEN + TOKEN_HEADER_LEN;

/// The [`Extent`] the first [`EXTENT_PREFIX_LEN`] bytes of a `Method::Lz`
/// payload declare, whatever follows them: reads the selector and header
/// and trusts nothing else, so it costs no decode work and no allocation,
/// and a reader that has not yet seen the stream can learn how long it is.
///
/// # Errors
///
/// [`Error::Truncated`] if `bytes` is shorter than the header or the
/// declared stream length overflows `usize`; [`Error::Corrupt`] if the
/// selector names no [`Candidate`].
pub(crate) fn declared_extent(bytes: &[u8]) -> Result<Extent, Error> {
    let (candidate, _, rest) = split_selector(bytes)?;
    let (header, _) = split_token_header(rest)?;
    let coded_len = usize::try_from(header.coded_len).map_err(|_| Error::Truncated)?;
    Ok(Extent {
        payload_len: EXTENT_PREFIX_LEN
            .checked_add(coded_len)
            .ok_or(Error::Truncated)?,
        output_len: candidate.output_len(header.declared_len),
    })
}

/// Splits `payload` into its declared length, token count, and the
/// range-coded bytes, rejecting a frame whose output ([`Candidate::output_len`]
/// of the declared length) is past `max_len` (clamped to
/// [`MAX_DECODED_LEN`]) before any allocation or decode work. The
/// stream is exactly the bytes its declared length names: `payload` is one
/// frame's, so a payload with bytes past the stream is corrupt.
///
/// # Errors
///
/// Returns [`Error::Truncated`] if `payload` is shorter than the
/// [`TOKEN_HEADER_LEN`]-byte header or than the stream it declares,
/// [`Error::TooLarge`] if the output exceeds the clamped `max_len`,
/// [`Error::Corrupt`] if bytes follow the stream.
fn read_header(
    payload: &[u8],
    max_len: u32,
    candidate: Candidate,
) -> Result<(usize, u32, &[u8]), Error> {
    let max_len = max_len.min(MAX_DECODED_LEN);
    let (header, stream) = split_token_header(payload)?;
    let output_len = candidate.output_len(header.declared_len);
    Error::ensure_within(output_len, max_len)?;
    match u64::try_from(stream.len()).map(|len| len.cmp(&header.coded_len)) {
        Ok(std::cmp::Ordering::Less) => Err(Error::Truncated),
        Ok(std::cmp::Ordering::Equal) => {
            Ok((header.declared_len as usize, header.token_count, stream))
        }
        _ => Err(Error::Corrupt),
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

/// Rejects a match distance past the frame's declared [`Reach`].
/// [`decode_offset`] can return distances up to `2 * WINDOW - 1`
/// (`OFFSET_BUCKETS`'s docs, bucket 20's residual bits), which is
/// [`lz::WIDE_WINDOW`]; a [`Reach::Standard`] frame's encoder never
/// searches past `WINDOW`, so a wider distance there is adversarial, and
/// rejecting it here, before it ever reaches `RepCache`, keeps every cached
/// rep distance within the reach too, so this is the only place that needs
/// the check.
fn ensure_within_window(distance: NonZeroU32, reach: Reach) -> Result<(), Error> {
    if distance.get() as usize > reach.window() {
        Err(Error::Corrupt)
    } else {
        Ok(())
    }
}

/// Copies `len` bytes to the end of `output` from `distance` bytes before
/// its current end, one byte at a time so a distance shorter than `len` (a
/// run, not a disjoint repeat) still reproduces the source correctly, each
/// copied byte visible to later reads in the same call.
///
/// Returns [`Error::Corrupt`], never panics, when `distance` reaches before
/// the start of `output`: this runs on a token stream decoded from an
/// adversarial bitstream (`rust-craft` skill, panic-discipline: decode-path
/// input is never trusted). [`lz::replay`], whose tokens [`lz::parse_optimal`]
/// just produced, calls it too and treats that error as a bug.
pub(crate) fn copy_checked(
    output: &mut Vec<u8>,
    len: u32,
    distance: NonZeroU32,
) -> Result<(), Error> {
    let distance = distance.get() as usize;
    let start = output.len().checked_sub(distance).ok_or(Error::Corrupt)?;
    for k in 0..len as usize {
        // start + k < output.len() at every iteration: start < output.len()
        // going in (checked_sub succeeded against a distance >= 1), and
        // output grows by exactly one element per iteration from there, so
        // the index this iteration reads is always already written,
        // either from before this call or by an earlier iteration of it
        // (the overlapping-run case).
        let byte = output[start + k];
        output.push(byte);
    }
    Ok(())
}

/// Where a decoded token's literal byte and match/rep copy land: real
/// output for [`decode`]'s [`VecSink`], undone-and-written-immediately
/// output for [`decode_to_writer`]'s `StreamingSink`. The two
/// walks must decode the same flag/length/offset/slot symbols, in the same
/// order, off the same token stream, or one silently drifts from what the
/// other produces; routing both through [`decode_tokens`] makes that a
/// single piece of code instead of two loops kept in sync by hand, mirroring
/// [`EncodeSink`]/[`walk_tokens`] on the encode side.
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
/// the model [`length_models`] selects by `kind`, the residual bits below it
/// through the [`ResidualTree`] beside that model. Mirrors
/// [`EncodeSink::length`].
fn decode_length(models: &mut Models, ac: &mut Decoder, kind: CopyKind) -> u32 {
    let (bucket, residual) = length_models(models, kind);
    let b = bucket.decode(ac);
    residual.decode(ac, b)
}

/// Decodes a match's distance: the bucket symbol through whichever of
/// [`Models::offset_len`]'s [`OFFSET_LEN_STATES`] models [`offset_len_state`]
/// of the already-decoded `len` selects, then the residual bits, as
/// `(1 << bucket) | residual_bits`, so never zero. Mirrors
/// [`EncodeSink::offset`]. Never panics on adversarial `ac` state:
/// `Model::decode` and `Decoder::decode_bits` are both panic-free on any
/// input (see their own docs), and the shift is bounded by the same
/// small-alphabet argument as [`bucket_bits`].
fn decode_offset(models: &mut Models, ac: &mut Decoder, len: u32) -> u32 {
    let bits = bucket_bits(models.offset_len[offset_len_state(len)].decode(ac));
    (1u32 << bits) | ac.decode_bits(bits)
}

/// The shared skeleton behind [`decode`] and [`decode_to_writer`]:
/// decodes `token_count` tokens off `ac_bytes` in coding order with fresh
/// models and rep cache, routing every literal byte and copy through `sink`,
/// and advancing the literal-model context and the rep cache exactly as
/// [`decode`] and [`decode_to_writer`] both require. See [`DecodeSink`]'s
/// docs for why this exists.
fn decode_tokens<S: DecodeSink>(
    token_count: u32,
    declared_len: usize,
    reach: Reach,
    ac_bytes: &[u8],
    sink: &mut S,
) -> Result<(), S::Err> {
    let mut ac = Decoder::new(ac_bytes);
    let mut models = Models::try_new().map_err(Error::from)?;
    let mut reps = RepCache::initial();
    let mut context = Context::default();
    for _ in 0..token_count {
        let flag_table = usize::from(context.after_copy);
        match FlagKind::from_index(models.flag[flag_table].decode(&mut ac)) {
            FlagKind::Literal => {
                ensure_room(sink.len(), 1, declared_len)?;
                let byte = sink.literal(&mut models, &mut ac, context)?;
                context = context.after_literal(byte);
            }
            FlagKind::Match => {
                let len = decode_length(&mut models, &mut ac, CopyKind::Match);
                let distance = decode_offset(&mut models, &mut ac, len);
                let distance =
                    NonZeroU32::new(distance).expect("decode_offset's result is always >= 1");
                ensure_within_window(distance, reach)?;
                ensure_room(sink.len(), len as usize, declared_len)?;
                context = sink.copy(len, distance, context)?;
                reps.push_front(distance);
            }
            FlagKind::Rep => {
                // RepSlot::from_index documents why models.slot's decode
                // is safe to feed it directly.
                let slot = RepSlot::from_index(models.slot.decode(&mut ac));
                let len = decode_length(&mut models, &mut ac, CopyKind::Rep);
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
/// `output` already holds. `column` is `Some` exactly when this frame's
/// candidate is [`Candidate::Transpose`], selecting the column-keyed literal
/// expert over [`crate::literal::Literal::decode_logit_sse`].
struct VecSink<'a> {
    output: &'a mut Vec<u8>,
    column: Option<ColumnCoding>,
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
            Some(col) => {
                let bank = col.bank(context.position);
                models
                    .literal
                    .decode_column(ac, context, bank, &mut col.state)
            }
            None => models
                .literal
                .decode_logit_sse(ac, context, &mut models.logit_sse),
        };
        self.output.push(byte);
        Ok(byte)
    }

    fn copy(&mut self, len: u32, distance: NonZeroU32, context: Context) -> Result<Context, Error> {
        copy_checked(self.output, len, distance)?;
        Ok(context.after_copy(&self.output[self.output.len() - len as usize..]))
    }
}

/// Decodes a payload produced by [`encode`] back into the original bytes.
///
/// Bounds decode work and allocation to the frame's declared length
/// (`docs/format/SPEC.md`, `rust-craft` skill's allocation-discipline):
/// `output` is reserved fallibly and exactly once, up front, to
/// `declared_len` (`try_reserve_exact`, so a real allocator failure is
/// [`Error::OutOfMemory`], not an abort), and every token is rejected the
/// moment it would grow `output` past that length. A payload lying about the
/// declared length or the token count therefore cannot cost more work or
/// memory than the length it declared allows. `max_len` bounds what it may
/// declare, clamped to [`MAX_DECODED_LEN`] whatever the caller passes:
/// [`crate::decompress_bounded`] clamps too, so the clamp here covers other
/// callers of this `#[doc(hidden)]` `pub` function. See [`MAX_DECODED_LEN`]'s
/// docs for why a fixed ceiling, not a ratio against the payload's own size,
/// is the sound bound.
///
/// A [`Candidate::Transpose`] frame decodes its literals through
/// [`crate::literal::Literal::decode_column`], every other candidate through
/// [`crate::literal::Literal::decode_logit_sse`] (see the module docs'
/// "Payload layout"). `payload` is a `Method::Lz` payload of the current
/// `FORMAT_VERSION`: [`crate::decompress`] rejects any other version before
/// calling this.
///
/// # Errors
///
/// - [`Error::Truncated`] if `payload` is shorter than the filter selector,
///   the header, or the stream the header declares.
/// - [`Error::TooLarge`] if the frame's output ([`Candidate::output_len`] of
///   the declared length) exceeds `max_len`, checked before any allocation
///   or decode work.
/// - [`Error::OutOfMemory`] if the allocator cannot satisfy `output`'s
///   reservation, one of the model's fixed-size tables, or (for
///   [`Candidate::Delta`], [`Candidate::Bcj`], [`Candidate::Transpose`],
///   [`Candidate::Base64`], [`Candidate::Reverse`] and
///   [`Candidate::ReverseDelta`]) the filter's undo buffer.
/// - [`Error::Corrupt`] if the filter selector names no [`Candidate`]; bytes
///   follow the stream; a match distance exceeds the frame's [`Reach`] (the
///   encoder's match finder never searches past it, so a wider one is never
///   legitimate, and left unrejected would force the decoder to retain
///   output far past what any real bitstream needs, `research/JOURNAL.md`
///   M4); a match or rep distance reaches before the start of decoded
///   output; a token would grow decoded output past the declared length; or
///   the final decoded length differs from it.
///
/// # Panics
///
/// Never, on any `payload`.
pub fn decode(payload: &[u8], max_len: u32) -> Result<Vec<u8>, Error> {
    let (candidate, reach, payload) = split_selector(payload)?;
    let (declared_len, token_count, ac_bytes) = read_header(payload, max_len, candidate)?;

    let column = candidate
        .columns()
        .map(|columns| {
            ColumnExpertState::try_new(MAX_COLUMN_BANKS).map(|state| ColumnCoding {
                columns,
                data_len: declared_len,
                state,
            })
        })
        .transpose()?;
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
        column,
    };
    decode_tokens(token_count, declared_len, reach, ac_bytes, &mut sink)?;

    if output.len() != declared_len {
        return Err(Error::Corrupt);
    }
    undo_filter(candidate, output)
}

/// Streaming counterpart to [`decode`], reached only through
/// [`crate::decompress_to_writer`].
///
/// A candidate [`StreamUndo`] has a variant for (`Identity`, `Delta`, `Bcj`)
/// undoes the LZ token stream one filtered byte at a time as it is produced,
/// so it bounds resident memory to the frame's [`Reach::window`] via [`lz::Window`]
/// regardless of `declared_len` (`research/JOURNAL.md`
/// S1-P7/S2-D5/S2-A74, ROADMAP M4's bounded-memory decode guarantee). The
/// rest need their filter undone over the complete buffer, so they take
/// [`decode`]'s whole-buffer path and one bulk write: `Transpose` scatters
/// across the whole buffer (`research/JOURNAL.md` S2-D4), `Base64` has no
/// per-byte undo, `Reverse` and `ReverseDelta` emit the last byte first.
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
    let (candidate, reach, filtered_payload) = split_selector(payload)?;
    let Some(streaming) = candidate.streaming() else {
        writer.write_all(&decode(payload, max_len)?)?;
        return Ok(());
    };
    let mut undo = match streaming {
        Streaming::Identity => StreamUndo::Identity,
        Streaming::Delta(stride) => {
            StreamUndo::Delta(filters::delta::Undo::try_new(stride).map_err(Error::from)?)
        }
        Streaming::Bcj => StreamUndo::Bcj(filters::bcj::Undo::try_new().map_err(Error::from)?),
    };
    let (declared_len, token_count, ac_bytes) = read_header(filtered_payload, max_len, candidate)?;

    let mut window = lz::Window::try_new(reach).map_err(Error::from)?;

    let mut sink = StreamingSink {
        window: &mut window,
        undo: &mut undo,
        writer: &mut *writer,
    };
    decode_tokens(token_count, declared_len, reach, ac_bytes, &mut sink)?;

    undo.finish(writer)?;

    if window.written_len() != declared_len {
        return Err(Error::Corrupt.into());
    }
    Ok(())
}

/// Per-byte filter undo for [`decode_to_writer`], one variant per
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

/// [`DecodeSink`] for [`decode_to_writer`]'s path: a literal byte (and a
/// copy's replayed bytes) land in `window`, then are undone through `undo`
/// and written to `writer` immediately. `window` holds the *filtered* byte
/// stream throughout: matches and reps reference positions in what the
/// encoder's LZ pass actually saw, which is filtered data (`apply_filter`
/// runs before `encode_tokens`), so only the byte handed to `writer` differs
/// per candidate, never what goes into `window` or `context`. Never carries
/// column-expert state: [`decode_to_writer`] builds this sink only for candidates
/// [`StreamUndo`] covers, and [`Candidate::Transpose`] is not one.
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

    /// `context` folds over the *filtered* bytes `window` holds, never
    /// `undo`'s output. One [`Context::after_copy`] per byte equals
    /// [`VecSink::copy`]'s one call over the whole run, which is not
    /// available here: `window`'s ring buffer does not hold the run
    /// contiguously when it wraps. A zero-length copy still validates
    /// `distance` against what has been written ([`copy_checked`]'s
    /// rejection) and still marks the context as after a copy.
    fn copy(
        &mut self,
        len: u32,
        distance: NonZeroU32,
        mut context: Context,
    ) -> Result<Context, crate::WriteError> {
        let distance = distance.get() as usize;
        self.window
            .written_len()
            .checked_sub(distance)
            .ok_or(Error::Corrupt)?;
        if len == 0 {
            return Ok(context.after_copy(&[]));
        }
        for _ in 0..len {
            let byte = self.window.get(distance);
            self.window.push(byte);
            self.undo.apply(byte, self.writer)?;
            context = context.after_copy(std::slice::from_ref(&byte));
        }
        Ok(context)
    }
}

#[cfg(test)]
mod tests;
