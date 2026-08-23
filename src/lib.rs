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

pub mod codec;
pub mod coder;
pub mod filters;
pub mod literal;
pub mod lz;
pub mod model;

/// First bytes of every mothergod frame.
pub const MAGIC: [u8; 4] = *b"MGDC";

/// Container format version written into frames produced by this crate.
///
/// Bumped to 1 when [`Method::Lz`] was added (`docs/adr/0026-wire-the-lz-context-mixing-method.md`):
/// a new method byte is a bitstream format change (CLAUDE.md hard rule 5).
/// A version-0 frame only ever contains [`Method::Stored`], which decodes
/// identically under this build, so no separate version-0 decode path is
/// needed: [`decompress`] already accepts any `version <= FORMAT_VERSION`.
pub const FORMAT_VERSION: u8 = 1;

/// Payload encoding methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Method {
    /// Payload is stored verbatim, no compression.
    Stored = 0,
    /// Optimal-parse LZ tokens, entropy-coded by adaptive flag/length/
    /// offset/rep-slot models and a six-expert context-mixing literal
    /// model, over an adaptive range coder. See [`codec`] for the payload
    /// layout. Filters are not wired in yet.
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
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => write!(f, "input ended before the frame header was complete"),
            Self::BadMagic => write!(f, "input is not a mothergod frame (bad magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            Self::UnknownMethod(m) => write!(f, "unknown compression method {m}"),
            Self::Corrupt => write!(f, "compressed payload is corrupt"),
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
/// # Errors
///
/// Returns an [`Error`] when `input` is truncated, is not a mothergod
/// frame, uses a version or method this build does not understand, or (for
/// [`Method::Lz`]) is not internally consistent — see [`codec::decode`].
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
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
        Method::Stored => Ok(payload.to_vec()),
        Method::Lz => codec::decode(payload),
    }
}

/// Shared deterministic pseudo-random fixture generator for round-trip
/// tests in `coder`, `model`, and `literal`: those three modules' test
/// suites each need a long, deterministic-but-unstructured symbol stream
/// with no external RNG dependency, and had each hand-rolled the same
/// xorshift32 step to get one.
#[cfg(test)]
pub(crate) mod test_support {
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
}
