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

// `bittree`/`codec`/`coder`/`column`/`literal`/`lz`/`model`/`ppm`/`sse` are
// the compression engine's internals, not a surface downstream crates are
// meant to call directly: `#[doc(hidden)]` keeps them out of the published
// API a 0.1 consumer sees (ROADMAP M6). Of the nine, only `lz` has a real
// external call site today (`mothergod::lz::WINDOW`, `bench/src/lib.rs`);
// the other eight are still `pub` rather than `pub(crate)` because several
// of their items (`Ppm`, `Sse::contexts`, `Model::ideal_cost_bits`) are
// research surface for standing leads not yet wired into the live codec
// path (S1-P1, S1-P3) and have no in-crate caller either — `pub(crate)`
// would turn them into `dead_code` lint errors under this crate's `-D
// warnings` gate. Narrowing them stays future work, done together with
// wiring or removing that research code, not as a side effect of a
// docs-only pass. `filters` stays fully documented: S2-A2 already judged it
// "a defensible standalone library surface on their own merits"
// (`research/JOURNAL.md`).
#[doc(hidden)]
pub mod bittree;
#[doc(hidden)]
pub mod codec;
#[doc(hidden)]
pub mod coder;
#[doc(hidden)]
pub mod column;
pub mod filters;
#[doc(hidden)]
pub mod literal;
#[doc(hidden)]
pub mod lz;
#[doc(hidden)]
pub mod model;
#[doc(hidden)]
pub mod ppm;
#[doc(hidden)]
pub mod sse;

/// First bytes of every mothergod frame.
pub const MAGIC: [u8; 4] = *b"MGDC";

/// Container format version written into frames produced by this crate.
///
/// Bumped to 1 when [`Method::Lz`] was added
/// (`docs/adr/0026-wire-the-lz-context-mixing-method.md`), to 2 when
/// filter selection was wired into its payload
/// (`docs/adr/0028-wire-filter-selection.md`), and to 3 when the literal
/// sub-stream switched to SSE-calibrated binary-tree coding
/// (`docs/adr/0038-wire-sse-into-the-literal-mixer.md`, `research/JOURNAL.md`
/// S1-P1): all three are bitstream format changes (CLAUDE.md hard rule 5).
/// A version-0 frame only ever contains [`Method::Stored`], which decodes
/// identically under this build, so no separate version-0 decode path is
/// needed. A version-1 frame can contain a `Method::Lz` payload in a
/// layout this build no longer parses (see [`codec`]'s module docs);
/// [`decompress`] rejects that combination explicitly
/// (`codec::LZ_MIN_VERSION`) rather than silently misreading it. A
/// version-2 frame's `Method::Lz` payload has the same outer layout as
/// version 3 but a different literal sub-stream shape (the old direct
/// 256-way mix, [`literal::Literal::decode`], instead of
/// [`literal::Literal::decode_sse`]); [`codec::decode`] takes the frame's
/// declared version and picks between them, so hard rule 5's "decode
/// support for all previous versions, unless an ADR drops one" is
/// satisfied by dispatch, not by dropping the old path
/// (`tests/golden/v2-lz-repeated-text.mgdc` pins that forever).
pub const FORMAT_VERSION: u8 = 3;

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
    /// The allocator could not satisfy the output buffer's reservation
    /// (`declared_len` bytes, already checked against the effective bound
    /// before this point). A real allocator failure this far into decode
    /// is rare, but hard rule 2 (`CLAUDE.md`) does not carve out an
    /// exception for it: [`codec::decode`] reserves `output`'s capacity
    /// through `try_reserve_exact` specifically so this returns an `Err`
    /// instead of the process aborting (`rust-craft` skill's
    /// allocation-discipline, torture-swept by `tests/torture.rs`, #453).
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
            Self::OutOfMemory => write!(f, "allocator could not satisfy the output buffer"),
        }
    }
}

impl std::error::Error for Error {}

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

/// Assembles a complete frame from `method` and its `payload`.
fn build_frame(method: Method, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.push(FORMAT_VERSION);
    frame.push(method as u8);
    frame.extend_from_slice(payload);
    frame
}

/// Whether an [`Method::Lz`] body of `body_len` bytes beats a
/// [`Method::Stored`] frame of `input_len` bytes. Strict: a tie keeps
/// `Stored`. This is `compress`'s own convention, not something
/// `docs/format/SPEC.md` requires — the spec only bounds the frame from
/// above (`header + len(x)`), which either method satisfies on a tie.
fn lz_beats_stored(body_len: usize, input_len: usize) -> bool {
    body_len < input_len
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
        if lz_beats_stored(body.len(), input.len()) {
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
    match method {
        Method::Stored => {
            if let Some(bound) = stored_bound
                && payload.len() > bound as usize
            {
                return Err(Error::TooLarge {
                    len: u32::try_from(payload.len()).unwrap_or(u32::MAX),
                    max: bound,
                });
            }
            Ok(payload.to_vec())
        }
        Method::Lz if version < codec::LZ_MIN_VERSION => Err(Error::UnsupportedVersion(version)),
        Method::Lz => codec::decode(payload, version, max_len),
    }
}

/// Like [`decompress_bounded`], but writes the decoded bytes to `writer`
/// incrementally instead of collecting them into one returned `Vec<u8>`.
/// Same frame-level checks, in the same order, so keep the two in sync if
/// either changes; header parsing and the `max_len`/`stored_bound` rules
/// are duplicated rather than shared because they return through two
/// different error types ([`Error`] here needs wrapping into
/// [`std::io::Error`], [`decompress_bounded`] does not).
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
/// An [`std::io::Error`] wrapping an [`Error`] (retrievable via
/// [`std::io::Error::get_ref`] and a downcast) for anything
/// [`decompress_bounded`] would itself return as an `Err`, or an
/// unwrapped [`std::io::Error`] if `writer` itself fails.
pub fn decompress_to_writer<W: std::io::Write>(
    input: &[u8],
    max_len: u32,
    writer: &mut W,
) -> std::io::Result<()> {
    use std::io::Write as _;

    let stored_bound = (max_len < codec::MAX_DECODED_LEN).then_some(max_len);
    let max_len = max_len.min(codec::MAX_DECODED_LEN);
    let (version, method, payload) = parse_header(input).map_err(std::io::Error::other)?;
    let mut writer = std::io::BufWriter::new(writer);
    match method {
        Method::Stored => {
            if let Some(bound) = stored_bound
                && payload.len() > bound as usize
            {
                return Err(std::io::Error::other(Error::TooLarge {
                    len: u32::try_from(payload.len()).unwrap_or(u32::MAX),
                    max: bound,
                }));
            }
            writer.write_all(payload)?;
        }
        Method::Lz if version < codec::LZ_MIN_VERSION => {
            return Err(std::io::Error::other(Error::UnsupportedVersion(version)));
        }
        Method::Lz => codec::decode_to_writer(payload, version, max_len, &mut writer)?,
    }
    writer.flush()
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
pub(crate) mod test_support {
    /// Deterministic pseudo-random symbol stream for round-trip tests in
    /// `coder`, `model`, and `literal`: those three modules' test suites
    /// each need a long, deterministic-but-unstructured stream with no
    /// external RNG dependency, and had each hand-rolled the same
    /// xorshift32 step to get one.
    ///
    /// xorshift32 generator: `next()` advances the state and returns it,
    /// so the seed itself is never yielded, only states derived from it.
    pub(crate) struct Xorshift32(u32);

    impl Xorshift32 {
        pub(crate) fn new(seed: u32) -> Self {
            Self(seed)
        }
    }

    impl Iterator for Xorshift32 {
        type Item = u32;

        fn next(&mut self) -> Option<u32> {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            Some(self.0)
        }
    }

    /// `v` as a [`std::num::NonZeroUsize`], for tests that need one as a
    /// filter parameter (a delta stride, a transpose column count) and
    /// know `v` is nonzero by construction: `filters::delta` and
    /// `filters::transpose`'s test suites each need this same conversion
    /// and had each hand-rolled the identical helper to get it.
    ///
    /// # Panics
    ///
    /// Panics if `v` is zero: every call site passes a literal already
    /// known to be nonzero, so this is a test-fixture bug, never
    /// something a non-test caller could trigger (this function only
    /// exists under `#[cfg(test)]`).
    pub(crate) fn nz(v: usize) -> std::num::NonZeroUsize {
        std::num::NonZeroUsize::new(v).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `MAGIC` + `FORMAT_VERSION` + `Method::Lz` as a fresh `Vec<u8>`, for
    /// tests that hand-craft a payload past it rather than going through
    /// [`compress`]: several need a specific declared length or filter
    /// selector `compress` itself would never choose.
    fn lz_frame_header() -> Vec<u8> {
        vec![
            MAGIC[0],
            MAGIC[1],
            MAGIC[2],
            MAGIC[3],
            FORMAT_VERSION,
            Method::Lz as u8,
        ]
    }

    #[test]
    fn roundtrip_empty() {
        assert_eq!(decompress(&compress(b"")), Ok(Vec::new()));
    }

    #[test]
    fn roundtrip_data() {
        let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
        assert_eq!(decompress(&compress(&input)), Ok(input));
    }

    #[test]
    fn compressible_input_picks_method_lz() {
        let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
        let frame = compress(&input);
        assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
        assert!(
            frame.len() < input.len(),
            "a 100x repeat should compress smaller than the input: {} -> {}",
            input.len(),
            frame.len()
        );
        assert_eq!(decompress(&frame), Ok(input));
    }

    #[test]
    fn lz_beats_stored_keeps_the_strictly_smaller_body_only() {
        // #390's mutation sweep found `<` -> `<=` surviving the full suite:
        // `compress`'s own round-trip tests never observe which method was
        // chosen on a tie, since both frame a losslessly. Unit testing the
        // extracted comparison directly is the only way to pin the
        // Stored-floor invariant ("Lz wins outright, never on a tie")
        // without constructing an input whose encoded Lz body happens to
        // land exactly at input.len().
        assert!(lz_beats_stored(3, 5), "a strictly smaller body must win");
        assert!(
            !lz_beats_stored(5, 5),
            "a tied body must fall back to Stored"
        );
        assert!(
            !lz_beats_stored(6, 5),
            "a larger body must fall back to Stored"
        );
    }

    #[test]
    fn old_version_lz_frame_is_rejected_not_misparsed() {
        // A frame naming FORMAT_VERSION 1 with Method::Lz predates the
        // 2-byte filter selector codec.rs's payload now starts with
        // (docs/adr/0028-wire-filter-selection.md). Decoding its payload
        // under the new layout would misread those bytes as part of the
        // declared length rather than a filter selector; decompress must
        // reject the version/method combination outright instead
        // (codec::LZ_MIN_VERSION).
        let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
        let mut frame = compress(&input);
        assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
        frame[MAGIC.len()] = 1;
        assert_eq!(decompress(&frame), Err(Error::UnsupportedVersion(1)));
    }

    #[test]
    fn tiny_input_falls_back_to_stored() {
        // A handful of bytes: Method::Lz's 8-byte header alone already
        // exceeds this, so compress must pick Stored (the "Stored floor"
        // invariant, docs/format/SPEC.md).
        let input = b"hi";
        let frame = compress(input);
        assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
        assert_eq!(decompress(&frame), Ok(input.to_vec()));
    }

    #[test]
    fn incompressible_input_falls_back_to_stored_and_roundtrips() {
        let input: Vec<u8> = test_support::Xorshift32::new(0x9E37_79B9)
            .take(2000)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();
        let frame = compress(&input);
        assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
        assert_eq!(decompress(&frame), Ok(input));
    }

    #[test]
    fn truncated_input_is_rejected() {
        assert_eq!(decompress(b"MGDC"), Err(Error::Truncated));
    }

    #[test]
    fn bad_magic_is_rejected() {
        assert_eq!(decompress(b"NOPE\0\0data"), Err(Error::BadMagic));
    }

    #[test]
    fn future_version_is_rejected() {
        let mut frame = compress(b"x");
        frame[MAGIC.len()] = FORMAT_VERSION + 1;
        assert_eq!(
            decompress(&frame),
            Err(Error::UnsupportedVersion(FORMAT_VERSION + 1))
        );
    }

    #[test]
    fn unknown_method_is_rejected() {
        let mut frame = compress(b"x");
        frame[MAGIC.len() + 1] = 0xFF;
        assert_eq!(decompress(&frame), Err(Error::UnknownMethod(0xFF)));
    }

    #[test]
    fn lz_declared_length_over_the_max_is_rejected() {
        // A tiny frame declaring an output far past codec::MAX_DECODED_LEN
        // (and a matching token count, so the loop-iterations argument in
        // codec::decode's docs doesn't save it either): must reject before
        // doing any decode work, not just eventually. Public-API-level
        // regression for the amplification hazard codec.rs's unit tests
        // cover directly.
        let over = codec::MAX_DECODED_LEN + 1;
        let mut frame = lz_frame_header();
        frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        assert_eq!(
            decompress(&frame),
            Err(Error::TooLarge {
                len: over,
                max: codec::MAX_DECODED_LEN
            })
        );
    }

    #[test]
    fn decompress_matches_decompress_bounded_at_the_max() {
        let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
        let frame = compress(&input);
        assert_eq!(
            decompress(&frame),
            decompress_bounded(&frame, codec::MAX_DECODED_LEN)
        );
    }

    #[test]
    fn decompress_bounded_rejects_an_lz_frame_over_its_own_tighter_bound() {
        // Legal under codec::MAX_DECODED_LEN, but a caller with a smaller
        // memory budget must still be able to reject it before any decode
        // work (ROADMAP M4's bounded-memory decode guarantee).
        let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
        let frame = compress(&input);
        assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
        let declared_len = u32::try_from(input.len()).unwrap();
        assert_eq!(
            decompress_bounded(&frame, declared_len - 1),
            Err(Error::TooLarge {
                len: declared_len,
                max: declared_len - 1
            })
        );
        assert_eq!(decompress_bounded(&frame, declared_len), Ok(input));
    }

    #[test]
    fn decompress_bounded_rejects_a_stored_frame_over_its_own_tighter_bound() {
        // Method::Stored has no declared-length field to check against;
        // decompress_bounded must still bound it by the payload's own
        // length rather than only ever bounding Method::Lz.
        let input = b"hi";
        let frame = compress(input);
        assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
        assert_eq!(
            decompress_bounded(&frame, 1),
            Err(Error::TooLarge {
                len: u32::try_from(input.len()).unwrap(),
                max: 1
            })
        );
        assert_eq!(
            decompress_bounded(&frame, u32::try_from(input.len()).unwrap()),
            Ok(input.to_vec())
        );
    }

    #[test]
    fn decompress_bounded_clamps_a_max_len_above_max_decoded_len() {
        // A caller passing u32::MAX must not bypass MAX_DECODED_LEN: the
        // clamp, not the caller's value, is the real ceiling.
        let over = codec::MAX_DECODED_LEN + 1;
        let mut frame = lz_frame_header();
        frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        assert_eq!(
            decompress_bounded(&frame, u32::MAX),
            Err(Error::TooLarge {
                len: over,
                max: codec::MAX_DECODED_LEN
            })
        );
    }

    #[test]
    fn decompress_roundtrips_a_stored_payload_past_max_decoded_len() {
        // The regression this guards: decompress must stay exactly as
        // unbounded for Method::Stored as it was before decompress_bounded
        // existed, since the payload length is read from `input` itself,
        // never spoofable past it (see decompress_bounded's docs). Builds
        // the frame directly rather than via compress(), which would run
        // the full LZ encoder over 256+ MiB just to hit the Stored
        // fallback; this test's target is decompress, not compress.
        let payload = vec![0xA5u8; (codec::MAX_DECODED_LEN + 1) as usize];
        let frame = build_frame(Method::Stored, &payload);
        assert_eq!(decompress(&frame), Ok(payload));
    }

    #[test]
    fn stored_frame_decodes_incrementally() {
        assert_eq!(decodes_incrementally(&compress(b"hi")), Ok(true));
    }

    #[test]
    fn non_transpose_candidates_decode_incrementally() {
        for candidate in [
            filters::select::Candidate::Identity,
            filters::select::Candidate::Delta(test_support::nz(4)),
            filters::select::Candidate::Bcj,
        ] {
            let mut frame = lz_frame_header();
            frame.extend_from_slice(&candidate.to_header_bytes());
            assert_eq!(
                decodes_incrementally(&frame),
                Ok(true),
                "{candidate:?} should decode incrementally"
            );
        }
    }

    #[test]
    fn transpose_candidate_does_not_decode_incrementally() {
        let mut frame = lz_frame_header();
        frame.extend_from_slice(
            &filters::select::Candidate::Transpose(test_support::nz(4)).to_header_bytes(),
        );
        assert_eq!(decodes_incrementally(&frame), Ok(false));
    }

    #[test]
    fn decodes_incrementally_pins_the_lz_min_version_boundary() {
        // #388: three mutants survived on this guard (`< false`, `<` -> `==`,
        // `<` -> `<=`) because no test distinguished it from a version-blind
        // one. Both sides of the boundary, on the same frame shape so only
        // the version byte differs.
        let mut frame = lz_frame_header();
        frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
        frame[MAGIC.len()] = codec::LZ_MIN_VERSION - 1;
        assert_eq!(
            decodes_incrementally(&frame),
            Err(Error::UnsupportedVersion(codec::LZ_MIN_VERSION - 1))
        );
        frame[MAGIC.len()] = codec::LZ_MIN_VERSION;
        assert_eq!(decodes_incrementally(&frame), Ok(true));
    }

    #[test]
    fn malformed_filter_selector_is_rejected_as_corrupt() {
        // [0, 1]: kind 0 (Identity) never carries a nonzero param, so
        // Candidate::from_header_bytes names no real candidate for it
        // (filters.rs's own unit tests cover the same byte pair).
        let mut frame = lz_frame_header();
        frame.extend_from_slice(&[0, 1]);
        assert_eq!(decodes_incrementally(&frame), Err(Error::Corrupt));
    }

    #[test]
    fn truncated_filter_selector_is_rejected() {
        let mut frame = lz_frame_header();
        frame.push(0);
        assert_eq!(decodes_incrementally(&frame), Err(Error::Truncated));
    }

    #[test]
    fn decodes_incrementally_shares_header_errors_with_decompress() {
        let mut frame = compress(b"hi");
        frame[0] = frame[0].wrapping_add(1);
        assert_eq!(decodes_incrementally(&frame), Err(Error::BadMagic));
    }

    /// Downcasts an [`std::io::Error`] produced by [`decompress_to_writer`]
    /// back to the [`Error`] it wrapped via [`std::io::Error::other`], for
    /// tests asserting exactly which decode error occurred.
    fn as_codec_error(err: &std::io::Error) -> Option<&Error> {
        err.get_ref()
            .and_then(|inner| inner.downcast_ref::<Error>())
    }

    #[test]
    fn decompress_to_writer_matches_decompress_for_stored_and_lz_frames() {
        for input in [
            b"hi".to_vec(),
            b"the quick brown fox jumps over the lazy dog".repeat(100),
        ] {
            let frame = compress(&input);
            let via_decompress = decompress(&frame).unwrap();
            let mut out = Vec::new();
            decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out).unwrap();
            assert_eq!(out, via_decompress);
        }
    }

    #[test]
    fn decompress_to_writer_rejects_an_lz_frame_over_the_max() {
        let over = codec::MAX_DECODED_LEN + 1;
        let mut frame = lz_frame_header();
        frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        let mut out = Vec::new();
        let err = decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out)
            .expect_err("declared length past MAX_DECODED_LEN must be rejected");
        assert_eq!(
            as_codec_error(&err),
            Some(&Error::TooLarge {
                len: over,
                max: codec::MAX_DECODED_LEN
            })
        );
    }

    #[test]
    fn decompress_to_writer_rejects_a_stored_frame_over_its_own_tighter_bound() {
        let input = b"hi";
        let frame = compress(input);
        assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
        let mut out = Vec::new();
        let err = decompress_to_writer(&frame, 1, &mut out)
            .expect_err("a Stored payload over a caller's tighter bound must be rejected");
        assert_eq!(
            as_codec_error(&err),
            Some(&Error::TooLarge {
                len: u32::try_from(input.len()).unwrap(),
                max: 1
            })
        );
        let mut out = Vec::new();
        decompress_to_writer(&frame, u32::try_from(input.len()).unwrap(), &mut out).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn decompress_to_writer_rejects_unsupported_version() {
        let mut frame = compress(b"hi");
        frame[VERSION_OFFSET] = FORMAT_VERSION + 1;
        let mut out = Vec::new();
        let err = decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out)
            .expect_err("a newer format version must be rejected");
        assert_eq!(
            as_codec_error(&err),
            Some(&Error::UnsupportedVersion(FORMAT_VERSION + 1))
        );
    }

    #[test]
    fn decompress_to_writer_propagates_writer_errors_unwrapped() {
        struct FailingWriter;
        impl std::io::Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
        }

        let frame = compress(b"hi");
        let mut writer = FailingWriter;
        let err = decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut writer)
            .expect_err("a writer that always fails must surface its error");
        assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
        assert!(
            as_codec_error(&err).is_none(),
            "a writer failure is not a decode Error and must not downcast to one"
        );
    }
}
