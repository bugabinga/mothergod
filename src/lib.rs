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
    /// Payload declares an output length larger than this decoder accepts
    /// (`codec::MAX_DECODED_LEN`). A declared length alone is not bounded
    /// by the bytes that encode it: this format's adaptive models can make
    /// a handful of real payload bytes and a few million padding-decoded
    /// bytes indistinguishable by size, so the only sound bound is an
    /// explicit ceiling, checked before any allocation or decode work
    /// happens (`rust-craft` skill, allocation-discipline).
    TooLarge(u32),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => write!(f, "input ended before the frame header was complete"),
            Self::BadMagic => write!(f, "input is not a mothergod frame (bad magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            Self::UnknownMethod(m) => write!(f, "unknown compression method {m}"),
            Self::Corrupt => write!(f, "compressed payload is corrupt"),
            Self::TooLarge(len) => write!(
                f,
                "declared output length {len} exceeds this decoder's maximum ({} bytes)",
                codec::MAX_DECODED_LEN
            ),
        }
    }
}

impl std::error::Error for Error {}

const VERSION_OFFSET: usize = MAGIC.len();
const METHOD_OFFSET: usize = VERSION_OFFSET + 1;
const HEADER_LEN: usize = METHOD_OFFSET + 1;

/// Assembles a complete frame from `method` and its `payload`.
fn build_frame(method: Method, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&MAGIC);
    frame.push(FORMAT_VERSION);
    frame.push(method as u8);
    frame.extend_from_slice(payload);
    frame
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
        if body.len() < input.len() {
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
/// Same as [`decompress`], plus [`Error::TooLarge`] whenever the frame's
/// declared or actual output length exceeds the effective bound (`max_len`
/// clamped to [`codec::MAX_DECODED_LEN`]) — including a [`Method::Stored`]
/// frame, which [`decompress`] never bounds beyond `input`'s own length
/// since a stored payload cannot amplify past what was already read.
pub fn decompress_bounded(input: &[u8], max_len: u32) -> Result<Vec<u8>, Error> {
    let max_len = max_len.min(codec::MAX_DECODED_LEN);
    let (header, payload) = input.split_at_checked(HEADER_LEN).ok_or(Error::Truncated)?;
    if header[..MAGIC.len()] != MAGIC {
        return Err(Error::BadMagic);
    }
    let version = header[VERSION_OFFSET];
    if version > FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let method = Method::try_from(header[METHOD_OFFSET])?;
    match method {
        Method::Stored => {
            if payload.len() > max_len as usize {
                return Err(Error::TooLarge(
                    u32::try_from(payload.len()).unwrap_or(u32::MAX),
                ));
            }
            Ok(payload.to_vec())
        }
        Method::Lz if version < codec::LZ_MIN_VERSION => Err(Error::UnsupportedVersion(version)),
        Method::Lz => codec::decode(payload, version, max_len),
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
        let mut frame = vec![
            MAGIC[0],
            MAGIC[1],
            MAGIC[2],
            MAGIC[3],
            FORMAT_VERSION,
            Method::Lz as u8,
        ];
        frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        assert_eq!(decompress(&frame), Err(Error::TooLarge(over)));
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
            Err(Error::TooLarge(declared_len))
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
            Err(Error::TooLarge(u32::try_from(input.len()).unwrap()))
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
        let mut frame = vec![
            MAGIC[0],
            MAGIC[1],
            MAGIC[2],
            MAGIC[3],
            FORMAT_VERSION,
            Method::Lz as u8,
        ];
        frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        frame.extend_from_slice(&over.to_le_bytes());
        assert_eq!(
            decompress_bounded(&frame, u32::MAX),
            Err(Error::TooLarge(over))
        );
    }
}
