#![doc(
    html_logo_url = "https://raw.githubusercontent.com/bugabinga/mothergod/main/assets/logo.svg"
)]
#![forbid(unsafe_code)]
//! mothergod — general purpose compression.
//!
//! The library speaks a tiny framed container format. Every frame starts
//! with a magic number, a format version, and a method byte identifying
//! how the payload is encoded: `Stored` (no compression) or `Lz`
//! (optimal-parse LZ over an adaptive range coder, `research/JOURNAL.md`
//! S2-D2). [`compress`] always picks whichever produces the smaller frame.
//!
//! ```
//! let original = b"the quick brown fox jumps over the lazy dog".repeat(100);
//! let frame = mothergod::compress(&original);
//! assert!(frame.len() < original.len());
//! assert_eq!(mothergod::decompress(&frame), Ok(original));
//! ```

// `bittree`/`codec`/`coder`/`column`/`fieldtype`/`literal`/`lz`/`model`/
// `ppm`/`sse`/`tans` are the compression engine's internals, not a surface
// downstream crates are meant to call directly: `#[doc(hidden)]` keeps them
// out of the published API a 0.1 consumer sees (ROADMAP M6). Of the eleven,
// only `lz` has a real external call site today (`mothergod::lz::WINDOW`,
// `bench/src/lib.rs`); the other ten are still `pub` rather than
// `pub(crate)` because several of their items (`Ppm`, `Sse::contexts`,
// `Model::ideal_cost_bits`, `tans::normalize_frequencies`,
// `fieldtype::field_bank`) are research surface for standing leads not yet
// wired into the live codec path (S1-P1, S1-P2, S1-P3, S1-P6) and have no
// in-crate caller either — `pub(crate)` would turn them into `dead_code`
// lint errors under this crate's `-D warnings` gate. Narrowing them stays
// future work, done together with wiring or removing that research code,
// not as a side effect of a docs-only pass. `filters` stays fully
// documented: S2-A2 already judged it "a defensible standalone library
// surface on their own merits" (`research/JOURNAL.md`).
#[doc(hidden)]
pub mod bittree;
#[doc(hidden)]
pub mod codec;
#[doc(hidden)]
pub mod coder;
#[doc(hidden)]
pub mod column;
#[doc(hidden)]
pub mod fieldtype;
pub mod filters;
#[doc(hidden)]
pub mod literal;
#[doc(hidden)]
pub mod logistic;
#[doc(hidden)]
pub mod lz;
#[doc(hidden)]
pub mod model;
#[doc(hidden)]
pub mod ppm;
#[doc(hidden)]
pub mod sse;
#[doc(hidden)]
pub mod tans;

/// First bytes of every mothergod frame.
pub const MAGIC: [u8; 4] = *b"MGDC";

/// Container format version written into frames produced by this crate.
///
/// Bumped to 1 when [`Method::Lz`] was added
/// (`docs/adr/0026-wire-the-lz-context-mixing-method.md`), to 2 when
/// filter selection was wired into its payload
/// (`docs/adr/0028-wire-filter-selection.md`; that version was later retired
/// outright, below), to 3 when the literal sub-stream switched to
/// SSE-calibrated binary-tree coding
/// (`docs/adr/0038-wire-sse-into-the-literal-mixer.md`, `research/JOURNAL.md`
/// S1-P1), to 4 when a `Candidate::Transpose` frame's literal sub-stream
/// gained a column-keyed seventh expert
/// (`docs/adr/0046-wire-the-column-expert-into-the-literal-mixer.md`,
/// `research/JOURNAL.md` S1-P5), to 5 when every other candidate's
/// literal sub-stream switched from SSE-calibrated coding to a logit-domain
/// mixer over the same six experts
/// (`docs/adr/0052-wire-the-logistic-mixer-into-the-literal-model.md`,
/// `research/JOURNAL.md` S1-P8), to 6 when that same set of candidates
/// switched to a second rate schedule over the identical logit-domain mix,
/// a learned baseline instead of an annealed step count
/// (`docs/adr/0054-wire-the-surprise-rate-schedule-into-the-literal-model.md`,
/// `research/JOURNAL.md` S2-A104/S2-A105), to 7 when that same set of
/// candidates switched its SSE calibration stage from a linear table to a
/// stretch-domain one
/// (`docs/adr/0055-wire-logit-domain-sse-bins-into-the-literal-model.md`,
/// `research/JOURNAL.md` S2-A106/S2-A108), and to 8 when a copy token's
/// length symbol switched from one shared model for every
/// [`lz::Token::Match`]/[`lz::Token::Rep`] to two independent ones,
/// selected by which kind produced it, regardless of candidate
/// (`docs/adr/0057-wire-the-match-rep-length-model-split.md`,
/// `research/JOURNAL.md` S2-A109/S2-A110), and to 9 when a match's
/// distance symbol switched from one shared model regardless of that
/// match's own length to four independent ones selected by a coarse bucket
/// of it
/// (`docs/adr/0058-wire-the-offset-length-state-split.md`,
/// `research/JOURNAL.md` S2-A111/S2-A112): all nine are bitstream format
/// changes (CLAUDE.md hard rule 5). A version-0 frame only ever contains
/// [`Method::Stored`], which decodes identically under this build, so no
/// separate version-0 decode path is needed. A version-1 or version-2 frame
/// is rejected as `UnsupportedVersion` before [`codec::decode`] is ever
/// called (`codec::LZ_MIN_VERSION`, 3): version 1 named a `Method::Lz`
/// payload in a layout this build no longer parses (see [`codec`]'s module
/// docs), and version 2 named the current outer layout but coded its
/// literal sub-stream through a direct 256-way mix instead of
/// [`literal::Literal::decode_sse`] — retired outright, no release having
/// ever written it
/// (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`). A
/// version-4 frame whose filter selector names `Candidate::Transpose` codes
/// its literals through [`literal::Literal::decode_column`] instead of
/// `decode_sse`, every other candidate unchanged; a version-5 frame codes
/// every candidate except that same `Candidate::Transpose` case through
/// [`literal::Literal::decode_logistic`] instead; a version-6 frame codes
/// that same set of candidates through [`literal::Literal::decode_logistic_surprise`]
/// instead; a version-7 frame codes that same set of candidates through
/// [`literal::Literal::decode_logit_sse`] instead. Separately from any of
/// that, regardless of candidate, a version-8 frame decodes a copy token's
/// length through [`lz::Token::Match`]/[`lz::Token::Rep`]'s own independent
/// model instead of one shared between them
/// (`codec::LENGTH_SPLIT_MIN_VERSION`), and a version-9 frame decodes a
/// `Token::Match`'s distance through one of four models keyed on that
/// match's own length (`codec::OFFSET_LEN_SPLIT_MIN_VERSION`) instead of
/// one shared regardless of length. [`codec::decode`] takes the
/// frame's declared version (and, for `Candidate::Transpose`, its
/// already-parsed candidate) and picks between them, so hard rule 5's
/// "decode support for every version the spec still covers" is satisfied
/// by dispatch, not by dropping an old path (`tests/golden/v3-*.mgdc`,
/// `tests/golden/v4-*.mgdc`, `tests/golden/v5-*.mgdc`,
/// `tests/golden/v6-*.mgdc`, `tests/golden/v7-*.mgdc`,
/// `tests/golden/v8-*.mgdc`, and `tests/golden/v9-*.mgdc` pin that
/// forever).
pub const FORMAT_VERSION: u8 = 9;

/// Payload encoding methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Method {
    /// Payload is stored verbatim, no compression.
    Stored = 0,
    /// Optimal-parse LZ tokens, entropy-coded by adaptive flag/length/
    /// offset/rep-slot models and a six-expert context-mixing literal
    /// model, over an adaptive range coder, behind whichever filter
    /// (delta, BCJ, transpose, or none) trial-selection found smallest.
    /// See [`codec`] for the payload layout.
    Lz = 1,
}

impl TryFrom<u8> for Method {
    type Error = Error;

    fn try_from(byte: u8) -> Result<Self, Error> {
        match byte {
            0 => Ok(Self::Stored),
            1 => Ok(Self::Lz),
            other => Err(Error::UnknownMethod(other)),
        }
    }
}

/// Errors produced when decoding a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Input ended before the frame header was complete.
    Truncated,
    /// Input does not start with [`MAGIC`].
    BadMagic,
    /// Frame was written by a newer, incompatible format version.
    UnsupportedVersion(u8),
    /// Method byte does not name a known [`Method`].
    UnknownMethod(u8),
    /// Payload does not decode to a value consistent with itself (a
    /// declared length its content does not match, a match/rep distance
    /// reaching before the start of decoded output, or similar):
    /// adversarial or corrupted input, never a bug in this decoder.
    Corrupt,
    /// Payload's declared ([`Method::Lz`]) or actual ([`Method::Stored`])
    /// output length exceeds the bound in effect: [`codec::MAX_DECODED_LEN`]
    /// under [`decompress`], or a caller's own tighter `max_len` under
    /// [`decompress_bounded`]. `max` names whichever bound was actually
    /// violated, since the two can differ. A declared length alone is not
    /// bounded by the bytes that encode it: this format's adaptive models
    /// can make a handful of real payload bytes and a few million
    /// padding-decoded bytes indistinguishable by size, so the only sound
    /// bound is an explicit ceiling, checked before any allocation or
    /// decode work happens (`rust-craft` skill, allocation-discipline).
    TooLarge {
        /// The length that exceeded the bound.
        len: u32,
        /// The bound it exceeded (not always [`codec::MAX_DECODED_LEN`];
        /// see the variant's docs).
        max: u32,
    },
    /// The allocator could not satisfy some fixed-size allocation partway
    /// through decode: the output buffer's reservation (`declared_len`
    /// bytes, already checked against the effective bound before this
    /// point), a model's adaptive frequency table, or a filter's undo
    /// buffer. A real allocator failure this far into decode is rare, but
    /// hard rule 2 (`CLAUDE.md`) does not carve out an exception for it:
    /// every one of those allocations goes through `try_reserve_exact`
    /// specifically so this returns an `Err` instead of the process
    /// aborting (`rust-craft` skill's allocation-discipline, torture-swept
    /// by `tests/torture.rs`, #453).
    OutOfMemory,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => write!(f, "input ended before the frame header was complete"),
            Self::BadMagic => write!(f, "input is not a mothergod frame (bad magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            Self::UnknownMethod(m) => write!(f, "unknown compression method {m}"),
            Self::Corrupt => write!(f, "compressed payload is corrupt"),
            Self::TooLarge { len, max } => {
                write!(
                    f,
                    "output length {len} exceeds the decoder's bound ({max} bytes)"
                )
            }
            Self::OutOfMemory => write!(f, "allocator could not satisfy a decode allocation"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::collections::TryReserveError> for Error {
    /// Every `try_reserve`/`try_new` allocation this crate's decode path
    /// makes fails the same way: [`Error::OutOfMemory`], never a panic
    /// (hard rule 2). One impl instead of a `.map_err(|_| Error::OutOfMemory)`
    /// at each of the decoder's fallible-allocation call sites.
    fn from(_: std::collections::TryReserveError) -> Self {
        Self::OutOfMemory
    }
}

/// Error from [`decompress_to_writer`]: either the frame failed to decode
/// ([`Error`], same variants and meaning as [`decompress`]'s) or `writer`
/// itself failed.
///
/// Two owned variants instead of the single [`std::io::Error`] this type
/// replaced (downcast via [`std::io::Error::get_ref`]): that shape needed
/// `std::io::Error::other` to wrap every decode error, which boxes its
/// payload through two allocations with no fallible sibling on stable Rust
/// (`Box::new` twice: once into `Box<dyn Error + Send + Sync>`, once into
/// `io::Error`'s own `Custom` struct). That meant *every* decode error —
/// even `Truncated` on a two-byte input — could abort the process under
/// real allocator pressure, violating hard rule 2 (issue #479). Plain enum
/// construction allocates nothing.
#[derive(Debug)]
pub enum WriteError {
    /// The frame did not decode; see [`Error`] for what each variant means.
    Decode(Error),
    /// `writer` itself failed.
    Io(std::io::Error),
}

impl core::fmt::Display for WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Decode(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(err) => Some(err),
            Self::Io(err) => Some(err),
        }
    }
}

impl From<Error> for WriteError {
    fn from(err: Error) -> Self {
        Self::Decode(err)
    }
}

impl From<std::io::Error> for WriteError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

const VERSION_OFFSET: usize = MAGIC.len();
const METHOD_OFFSET: usize = VERSION_OFFSET + 1;
const HEADER_LEN: usize = METHOD_OFFSET + 1;

/// Splits `input` into its declared version, [`Method`], and the payload
/// past the header, checked against [`MAGIC`] and [`FORMAT_VERSION`] but
/// nothing past that: shared by every function that dispatches on a
/// frame's method before deciding how much of the payload it actually
/// needs, so the two never drift on what counts as a well-formed header.
fn parse_header(input: &[u8]) -> Result<(u8, Method, &[u8]), Error> {
    let (header, payload) = input.split_at_checked(HEADER_LEN).ok_or(Error::Truncated)?;
    if header[..MAGIC.len()] != MAGIC {
        return Err(Error::BadMagic);
    }
    let version = header[VERSION_OFFSET];
    if version > FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let method = Method::try_from(header[METHOD_OFFSET])?;
    Ok((version, method, payload))
}

/// A parsed frame header plus the two bounds [`decompress_bounded`] and
/// [`decompress_to_writer`] both enforce before dispatching on `method`,
/// built by [`bounded_header`] so the two can't drift on either rule.
struct BoundedHeader<'a> {
    /// `Some` only when the caller's `max_len` is strictly tighter than
    /// [`codec::MAX_DECODED_LEN`]; see [`bounded_header`] for why a looser
    /// one never rejects a [`Method::Stored`] frame on size alone.
    stored_bound: Option<u32>,
    max_len: u32,
    version: u8,
    method: Method,
    payload: &'a [u8],
}

/// Shared by [`decompress_bounded`] and [`decompress_to_writer`]: parses the
/// header, then reduces `max_len` and the caller-visible `stored_bound` the
/// same way both do. Both callers need `Error`, never `WriteError`;
/// `decompress_to_writer` converts on its own `?` through [`WriteError`]'s
/// `From<Error>` impl.
fn bounded_header(input: &[u8], max_len: u32) -> Result<BoundedHeader<'_>, Error> {
    // Method::Stored's payload length is read directly from `input`, never
    // spoofable past what was already loaded into memory, so unlike
    // Method::Lz's declared-length field (docs/format/SPEC.md lines 91-94)
    // MAX_DECODED_LEN itself buys it no safety margin. Only a caller-chosen
    // bound strictly tighter than MAX_DECODED_LEN is worth enforcing here;
    // at or above it this arm stays exactly as unbounded as `decompress`
    // (equivalent to calling this with max_len == MAX_DECODED_LEN) always
    // was, so incompressible input at or past 256 MiB keeps round-tripping.
    let stored_bound = (max_len < codec::MAX_DECODED_LEN).then_some(max_len);
    let max_len = max_len.min(codec::MAX_DECODED_LEN);
    let (version, method, payload) = parse_header(input)?;
    Ok(BoundedHeader {
        stored_bound,
        max_len,
        version,
        method,
        payload,
    })
}

/// Rejects a [`Method::Stored`] frame whose payload exceeds `stored_bound`
/// (see [`bounded_header`] for when that bound is `None`).
fn check_stored_bound(payload: &[u8], stored_bound: Option<u32>) -> Result<(), Error> {
    if let Some(bound) = stored_bound
        && payload.len() > bound as usize
    {
        return Err(Error::TooLarge {
            len: u32::try_from(payload.len()).unwrap_or(u32::MAX),
            max: bound,
        });
    }
    Ok(())
}

/// Frequency increment and rescale ceiling shared by every adaptive
/// frequency table in the crate that runs at the archive's original rate:
/// [`model::Model`], [`ppm::Ppm`], and every [`literal::Literal`] bank
/// except its fast-rate bank 0 (which tunes its own, faster-forgetting
/// `FAST_INCREMENT`/`FAST_LIMIT`). Ported unchanged from the archive's
/// `INC`/`LIM` (`research/imports/session-1/mothergod.rs`): before this
/// pair was pulled out from under them, each of the three had
/// independently declared the same two values, the same duplication
/// [`rescale_bank`] itself already closed for the arithmetic that consumes
/// them.
pub(crate) const DEFAULT_RESCALE_INCREMENT: u32 = 12;
/// See [`DEFAULT_RESCALE_INCREMENT`].
pub(crate) const DEFAULT_RESCALE_LIMIT: u32 = 65536;

/// Increments `freq[symbol]`/`*total` by `increment`, then halves every
/// entry of `freq` (`(f+1) >> 1`, so a bank with any real evidence never
/// rescales down to an impossible-to-code symbol) once `*total` exceeds
/// `limit`, recomputing `*total` from the halved counts.
///
/// Shared by every adaptive frequency table in the crate —
/// [`model::Model`], [`ppm::Ppm`], and [`literal::Literal`]'s six banks
/// plus its `ColumnExpertState` experiment bank — so none of them can
/// drift on what "one observation" does to a bank; each had independently
/// ported the archive's identical `INC`/`LIM` update rule before this was
/// pulled out from under them.
pub(crate) fn rescale_bank(
    freq: &mut [u32],
    total: &mut u32,
    symbol: usize,
    increment: u32,
    limit: u32,
) {
    freq[symbol] += increment;
    *total += increment;
    if *total > limit {
        let mut new_total = 0u32;
        for f in freq.iter_mut() {
            *f = (*f + 1) >> 1;
            new_total += *f;
        }
        *total = new_total;
    }
}

/// Codes `symbol` through `encoder` at the sub-range `freq[..symbol].sum()..
/// +freq[symbol]` out of `denom`, the cumulative-frequency range every
/// symbol coder in the crate hands [`coder::Encoder::encode`].
///
/// Shared by [`model::Model::encode`] and [`ppm::Ppm::encode`], which pass
/// their own table and `denom` (`total`, or `total + distinct` under PPM
/// Method C's escape band) around this one scan, so the two can't drift on
/// how a symbol's range is carved out of a frequency table.
pub(crate) fn encode_symbol(encoder: &mut coder::Encoder, freq: &[u32], symbol: usize, denom: u64) {
    let low: u32 = freq[..symbol].iter().sum();
    let high = low + freq[symbol];
    encoder.encode(u64::from(low), u64::from(high), denom);
}

/// Finds the symbol whose cumulative-frequency range in `freq` contains
/// `target` (as returned by [`coder::Decoder::target`]), returning the
/// symbol and the `[low, high)` range [`coder::Decoder::decode`] must
/// consume to stay in lockstep with [`encode_symbol`].
///
/// Never runs past the end of `freq`: callers bound `target` to
/// `freq.iter().sum()` (plus, for [`ppm::Ppm`], a reserved escape share
/// handled before calling this), so the scan always finds a symbol
/// regardless of what bytes produced `target`.
///
/// Shared by [`model::Model::decode`] and [`ppm::Ppm::decode`], the decode
/// side of [`encode_symbol`]'s split.
pub(crate) fn scan_for_target(freq: &[u32], target: u64) -> (usize, u64, u64) {
    let mut symbol = 0;
    let mut low = 0u64;
    while low + u64::from(freq[symbol]) <= target {
        low += u64::from(freq[symbol]);
        symbol += 1;
    }
    let high = low + u64::from(freq[symbol]);
    (symbol, low, high)
}

/// Builds a `Vec<T>` of `n` clones of `value`, failing gracefully instead
/// of aborting if the allocator cannot satisfy `n` (hard rule 2,
/// `rust-craft` skill's allocation-discipline): `try_reserve_exact` first,
/// then `resize`, which never asks the allocator for more than that
/// already-reserved capacity, mirroring [`codec::decode`]'s own
/// `output.try_reserve_exact` shape (#453).
///
/// Shared by every fixed-size adaptive table on the real decode path
/// ([`model::Model::try_new`], [`sse::Sse::try_new`],
/// [`literal::Literal::try_new`]) so none of them can drift on how a
/// fallible fill is built.
pub(crate) fn try_filled_vec<T: Clone>(
    n: usize,
    value: T,
) -> Result<Vec<T>, std::collections::TryReserveError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)?;
    v.resize(n, value);
    Ok(v)
}

/// Fallible counterpart to `data.to_vec()`: same bytes, but returns `Err`
/// instead of aborting if the allocator cannot satisfy `data.len()`
/// bytes. Shared by the filter undo buffers on the real decode path
/// ([`filters::delta::try_decode`], [`filters::bcj::try_decode`]) that
/// start from a copy of their input (#453).
pub(crate) fn try_vec_from_slice(
    data: &[u8],
) -> Result<Vec<u8>, std::collections::TryReserveError> {
    let mut v = Vec::new();
    v.try_reserve_exact(data.len())?;
    v.extend_from_slice(data);
    Ok(v)
}

/// Assembles a complete frame from `method` and its `payload`.
fn build_frame(method: Method, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.push(FORMAT_VERSION);
    frame.push(method as u8);
    frame.extend_from_slice(payload);
    frame
}

/// Whether a `candidate_len`-byte encoding beats an `incumbent_len`-byte
/// incumbent in a shortest-wins search. Strict: a tie keeps the
/// incumbent, so search order is a stable tie-break, not an accident of
/// iteration. Shared by [`compress`]'s [`Method::Lz`]-vs-[`Method::Stored`]
/// choice and [`codec::encode`]'s filter-candidate search (where it keeps
/// [`filters::select::pick`]'s candidate order a stable tie-break) —
/// both are this same convention, not something `docs/format/SPEC.md`
/// requires (the spec only bounds the frame from above, which either
/// choice satisfies on a tie).
///
/// #390's mutation sweep found `<` survive as both `<=` and `==` at this
/// function's two call sites, before they were merged here: neither
/// `compress`'s nor `encode`'s own round-trip and selection tests ever
/// observe which candidate was chosen on a tie, since every candidate
/// they compare is already a valid, lossless encoding. The unit test
/// below is the only place that pins "strictly smaller, not tied or
/// larger" directly.
pub(crate) fn candidate_beats_incumbent(candidate_len: usize, incumbent_len: usize) -> bool {
    candidate_len < incumbent_len
}

/// Compresses `input` into a self-describing frame.
///
/// Tries [`Method::Lz`] and falls back to [`Method::Stored`] whenever that
/// does not produce a smaller frame (`docs/format/SPEC.md`'s Stored-floor
/// invariant): tiny, incompressible, or already-compressed input, and any
/// input longer than `u32::MAX` bytes, which [`codec::encode`] does not
/// support yet.
#[must_use]
pub fn compress(input: &[u8]) -> Vec<u8> {
    if u32::try_from(input.len()).is_ok() {
        let body = codec::encode(input);
        if candidate_beats_incumbent(body.len(), input.len()) {
            return build_frame(Method::Lz, &body);
        }
    }
    build_frame(Method::Stored, input)
}

/// Decodes a frame produced by [`compress`] back into the original bytes.
///
/// Equivalent to [`decompress_bounded`] with [`codec::MAX_DECODED_LEN`] as
/// the bound, the largest output this decoder's worst-case decode time has
/// been measured against.
///
/// # Errors
///
/// Returns an [`Error`] when `input` is truncated, is not a mothergod
/// frame, uses a version or method this build does not understand, or (for
/// [`Method::Lz`]) is not internally consistent — see [`codec::decode`].
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_bounded(input, codec::MAX_DECODED_LEN)
}

/// Like [`decompress`], but rejects any frame whose output would exceed
/// `max_len` bytes, checked before any allocation or decode work
/// (`rust-craft` skill's allocation-discipline). `max_len` is clamped to
/// [`codec::MAX_DECODED_LEN`] regardless of what is passed in: that
/// constant is the only ceiling this decoder's worst-case decode time has
/// been measured against (see its docs), so a caller can tighten the bound
/// for its own memory budget but never loosen it past what has been
/// proven safe.
///
/// A caller embedding mothergod under a known memory budget (well below
/// [`codec::MAX_DECODED_LEN`]'s 256 MiB) should call this instead of
/// [`decompress`] directly: ROADMAP M4's bounded-memory decode guarantee,
/// ahead of and independent from a future streaming/block API.
///
/// # Errors
///
/// Same as [`decompress`], plus [`Error::TooLarge`] whenever [`Method::Lz`]'s
/// declared output length exceeds the effective bound (`max_len` clamped to
/// [`codec::MAX_DECODED_LEN`]), or a [`Method::Stored`] frame's payload
/// exceeds a `max_len` strictly below [`codec::MAX_DECODED_LEN`] (an
/// explicit opt-in to a smaller memory budget). `max_len` at or above
/// [`codec::MAX_DECODED_LEN`] never rejects a [`Method::Stored`] frame on
/// size alone: its payload length is read directly from `input`, never
/// spoofable past what was already loaded, so unlike [`Method::Lz`]'s
/// declared-length field, [`codec::MAX_DECODED_LEN`] buys it no safety
/// margin, only a compatibility break for large incompressible input.
pub fn decompress_bounded(input: &[u8], max_len: u32) -> Result<Vec<u8>, Error> {
    let BoundedHeader {
        stored_bound,
        max_len,
        version,
        method,
        payload,
    } = bounded_header(input, max_len)?;
    match method {
        Method::Stored => {
            check_stored_bound(payload, stored_bound)?;
            Ok(payload.to_vec())
        }
        Method::Lz if version < codec::LZ_MIN_VERSION => Err(Error::UnsupportedVersion(version)),
        Method::Lz => codec::decode(payload, version, max_len),
    }
}

/// Minimal write-buffering wrapper, standing in for [`std::io::BufWriter`]
/// on [`decompress_to_writer`]'s real decode path: `BufWriter::new` itself
/// allocates its buffer through an infallible path, which would abort the
/// process under a real allocator failure instead of returning an `Err`
/// (hard rule 2, `rust-craft` skill's allocation-discipline,
/// `tests/torture.rs`, #453). [`codec::decode_to_writer`]'s streaming path
/// writes one filtered byte at a time
/// (`codec::StreamUndo::apply`), so wrapping `writer` unbuffered would turn
/// each into its own `write_all` call; this preserves that batching while
/// building its buffer through `try_reserve_exact`.
struct TryBufWriter<'w, W: std::io::Write> {
    inner: &'w mut W,
    buf: Vec<u8>,
}

impl<'w, W: std::io::Write> TryBufWriter<'w, W> {
    /// Matches [`std::io::BufWriter`]'s own default buffer size.
    const CAPACITY: usize = 8 * 1024;

    fn try_new(inner: &'w mut W) -> Result<Self, std::collections::TryReserveError> {
        let mut buf = Vec::new();
        buf.try_reserve_exact(Self::CAPACITY)?;
        Ok(Self { inner, buf })
    }

    /// Writes out and clears any bytes still buffered, without flushing
    /// `inner` itself (see [`std::io::Write::flush`] for that).
    fn flush_buf(&mut self) -> std::io::Result<()> {
        if !self.buf.is_empty() {
            self.inner.write_all(&self.buf)?;
            self.buf.clear();
        }
        Ok(())
    }
}

impl<W: std::io::Write> std::io::Write for TryBufWriter<'_, W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if data.len() > Self::CAPACITY - self.buf.len() {
            self.flush_buf()?;
        }
        if data.len() >= Self::CAPACITY {
            return self.inner.write(data);
        }
        // The flush above guarantees at least `CAPACITY - self.buf.len()`
        // room, and this arm is only reached when `data.len()` is at most
        // that: never grows `buf` past the capacity reserved in `try_new`.
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_buf()?;
        self.inner.flush()
    }
}

impl<W: std::io::Write> Drop for TryBufWriter<'_, W> {
    /// Best-effort flush on drop, matching [`std::io::BufWriter`]'s own
    /// `Drop` impl: a caller that returns through `?` before reaching an
    /// explicit `flush()` call still gets whatever was buffered so far.
    /// Errors are unobservable this late (no `Result` to return them
    /// through) and so are ignored here exactly as `BufWriter` ignores
    /// them.
    fn drop(&mut self) {
        let _ = self.flush_buf();
    }
}

/// Like [`decompress_bounded`], but writes the decoded bytes to `writer`
/// incrementally instead of collecting them into one returned `Vec<u8>`.
/// Same frame-level checks, in the same order (shared with it through the
/// same private header-parsing and bound-checking helpers), so the two
/// can't drift.
///
/// Only bounds resident memory better than [`decompress_bounded`] for a
/// [`Method::Lz`] frame whose encoder picked
/// [`filters::select::Candidate::Identity`], `Delta`, or `Bcj`: filters
/// whose undo step runs sequentially with small fixed lookback or lookahead
/// (a no-op, a stride, or a 5-byte call/jmp instruction), so the decoded LZ
/// token stream needs no whole-buffer pass afterward, and
/// `codec::decode_to_writer` bounds memory to [`lz::WINDOW`] (1 MiB) for any
/// of them regardless of the frame's declared length (`research/JOURNAL.md`
/// S1-P7/S2-D5/S2-A74, ROADMAP M4's bounded-memory decode guarantee).
/// [`Method::Stored`] and [`filters::select::Candidate::Transpose`] fall
/// back to a whole-buffer decode followed by one bulk write: no worse than
/// [`decompress_bounded`], just never streamed. [`decodes_incrementally`]
/// tells a caller which case a frame is without decoding it, though this
/// function does not require calling it first.
///
/// # Errors
///
/// [`WriteError::Decode`] for anything [`decompress_bounded`] would itself
/// return as an `Err`, plus [`Error::OutOfMemory`] if the allocator cannot
/// satisfy the write buffer's own allocation; [`WriteError::Io`] if
/// `writer` itself fails. Neither variant boxes its payload through
/// `std::io::Error::other` (issue #479): both are plain enum
/// construction, so a decode error can never abort the process under
/// allocator pressure the way the wrapped form could.
pub fn decompress_to_writer<W: std::io::Write>(
    input: &[u8],
    max_len: u32,
    writer: &mut W,
) -> Result<(), WriteError> {
    use std::io::Write as _;

    let BoundedHeader {
        stored_bound,
        max_len,
        version,
        method,
        payload,
    } = bounded_header(input, max_len)?;
    let mut writer = TryBufWriter::try_new(writer).map_err(Error::from)?;
    match method {
        Method::Stored => {
            check_stored_bound(payload, stored_bound)?;
            writer.write_all(payload)?;
        }
        Method::Lz if version < codec::LZ_MIN_VERSION => {
            return Err(Error::UnsupportedVersion(version).into());
        }
        Method::Lz => codec::decode_to_writer(payload, version, max_len, &mut writer)?,
    }
    writer.flush()?;
    Ok(())
}

/// Reports whether `input`'s frame can decode with the output produced in
/// address order and bounded lookback, without doing any of that decode
/// work itself: a precondition a future streaming/block API needs, checked
/// here first because it does not have one uniform answer
/// (`research/JOURNAL.md` S2-D4, ROADMAP M4).
///
/// [`Method::Stored`] always answers `true`: there is no filter step, so
/// nothing about a bound on resident memory depends on its content.
/// [`Method::Lz`]'s answer depends on which filter its encoder picked —
/// [`filters::select::Candidate::Identity`], `Delta`, and `Bcj` all undo
/// sequentially with small fixed lookback or lookahead (a no-op, a stride,
/// or a 5-byte call/jmp instruction), and [`decompress_to_writer`] streams
/// all three, but `Candidate::Transpose`'s decode writes scattered across
/// the *entire* buffer in column-major order, so it needs the whole buffer
/// resident regardless of how a streaming decoder is otherwise built. This
/// predicate is checked independently of [`decompress_to_writer`]'s own
/// dispatch rather than the two sharing one classification, so a caller can
/// ask the question before committing to either API, and the two staying in
/// sync is a property tests can verify rather than an invariant the code
/// silently assumes.
///
/// # Errors
///
/// Same as [`decompress`]'s header-parsing errors
/// ([`Error::Truncated`], [`Error::BadMagic`], [`Error::UnsupportedVersion`],
/// [`Error::UnknownMethod`]), plus [`Error::Corrupt`] when a [`Method::Lz`]
/// frame's filter selector does not name a real [`filters::select::Candidate`]
/// — everything short of actually decoding the payload.
pub fn decodes_incrementally(input: &[u8]) -> Result<bool, Error> {
    let (version, method, payload) = parse_header(input)?;
    match method {
        Method::Stored => Ok(true),
        Method::Lz if version < codec::LZ_MIN_VERSION => Err(Error::UnsupportedVersion(version)),
        Method::Lz => {
            let filter_bytes = payload.get(0..2).ok_or(Error::Truncated)?;
            let candidate =
                filters::select::Candidate::from_header_bytes([filter_bytes[0], filter_bytes[1]])
                    .ok_or(Error::Corrupt)?;
            Ok(!matches!(
                candidate,
                filters::select::Candidate::Transpose(_)
            ))
        }
    }
}

/// Shared test-only fixtures multiple modules' test suites had each
/// hand-rolled a copy of.
#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;

// Not under Miri: same rationale as `coder.rs`'s `mod proptests` header
// comment (interpretation cost, issue #456).
#[cfg(test)]
#[cfg(not(miri))]
mod proptests;
