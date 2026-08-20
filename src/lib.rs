//! mothergod — general purpose compression.
//!
//! The library speaks a tiny framed container format. Every frame starts
//! with a magic number, a format version, and a method byte identifying
//! how the payload is encoded. Only the `Stored` method (no compression)
//! exists so far; real codecs plug in as new method bytes without
//! breaking old frames.

/// First bytes of every mothergod frame.
pub const MAGIC: [u8; 4] = *b"MGDC";

/// Container format version written into frames produced by this crate.
pub const FORMAT_VERSION: u8 = 0;

/// Payload encoding methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Method {
    /// Payload is stored verbatim, no compression.
    Stored = 0,
}

impl TryFrom<u8> for Method {
    type Error = Error;

    fn try_from(byte: u8) -> Result<Self, Error> {
        match byte {
            0 => Ok(Self::Stored),
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
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => write!(f, "input ended before the frame header was complete"),
            Self::BadMagic => write!(f, "input is not a mothergod frame (bad magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            Self::UnknownMethod(m) => write!(f, "unknown compression method {m}"),
        }
    }
}

impl std::error::Error for Error {}

const HEADER_LEN: usize = MAGIC.len() + 2;

/// Compresses `input` into a self-describing frame.
///
/// Until a real codec lands this always uses [`Method::Stored`], so the
/// output is `HEADER_LEN` bytes larger than the input.
#[must_use]
pub fn compress(input: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN + input.len());
    frame.extend_from_slice(&MAGIC);
    frame.push(FORMAT_VERSION);
    frame.push(Method::Stored as u8);
    frame.extend_from_slice(input);
    frame
}

/// Decodes a frame produced by [`compress`] back into the original bytes.
///
/// # Errors
///
/// Returns an [`Error`] when `input` is truncated, is not a mothergod
/// frame, or uses a version or method this build does not understand.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    let (header, payload) = input.split_at_checked(HEADER_LEN).ok_or(Error::Truncated)?;
    if header[..MAGIC.len()] != MAGIC {
        return Err(Error::BadMagic);
    }
    let version = header[MAGIC.len()];
    if version > FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let method = Method::try_from(header[MAGIC.len() + 1])?;
    match method {
        Method::Stored => Ok(payload.to_vec()),
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
