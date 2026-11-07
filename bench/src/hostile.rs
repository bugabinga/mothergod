//! The worst-case decode of each coded method at its output ceiling (issue
//! #1915): a crafted payload that declares the ceiling and makes every
//! output byte take the method's dearest path, timed once per method in one
//! process, so the two wall times compare on one CPU.
//!
//! [`Method::Lz`]'s dearest byte is a literal (`research/JOURNAL.md`
//! S2-A27): its payload carries an empty range-coded stream, which the
//! decoder reads as zero bits, so every flag decodes to symbol 0, a literal,
//! and every literal pays the full mix. [`Method::Cm`] walks the whole
//! model for every byte whatever its stream, so its payload carries
//! pseudo-random bytes, the fewest that admit the ceiling under
//! [`cm_path::MAX_EXPANSION`].
//!
//! Both payloads name [`Candidate::Identity`], whose undo copies nothing.
//! A [`Reading`] counts the declared bytes and the decode's wall time;
//! [`time`] refuses to report a decode that stopped short, because a
//! rejected payload times the rejection, not the ceiling.

use std::hint::black_box;
use std::time::Instant;

use mothergod::Error;
use mothergod::cm_path;
use mothergod::codec;
use mothergod::filters::select::Candidate;

use crate::entropy_ladder;

/// A coded method whose decode work a hostile payload can drive to its
/// ceiling; `Stored` copies its payload and has none to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// [`mothergod::Method::Lz`], bounded by [`codec::MAX_DECODED_LEN`].
    Lz,
    /// [`mothergod::Method::Cm`], bounded by [`cm_path::MAX_OUTPUT_LEN`].
    Cm,
}

impl Method {
    /// Both, in the order [`time`] runs them.
    pub const ALL: [Self; 2] = [Self::Lz, Self::Cm];

    /// The most bytes one payload of this method decodes to.
    #[must_use]
    pub const fn ceiling(self) -> u32 {
        match self {
            Self::Lz => codec::MAX_DECODED_LEN,
            Self::Cm => cm_path::MAX_OUTPUT_LEN,
        }
    }

    /// The method's name as the frame format spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lz => "Lz",
            Self::Cm => "Cm",
        }
    }

    /// A payload declaring `declared_len` bytes that makes every one of them
    /// take this method's dearest decode path (the module docs say which).
    ///
    /// # Panics
    ///
    /// If `declared_len` is zero, which no ceiling is.
    #[must_use]
    pub fn payload(self, declared_len: u32) -> Vec<u8> {
        assert!(declared_len > 0, "a hostile payload declares some bytes");
        let mut out = Candidate::Identity.to_header_bytes().to_vec();
        out.extend_from_slice(&declared_len.to_le_bytes());
        match self {
            Self::Lz => {
                // As many tokens as bytes: a decode that returns every
                // declared byte decoded one byte per token.
                out.extend_from_slice(&declared_len.to_le_bytes());
                out.extend_from_slice(&0u64.to_le_bytes());
            }
            Self::Cm => {
                let stream_len = declared_len.div_ceil(cm_path::MAX_EXPANSION) as usize;
                let stream = entropy_ladder(8, stream_len, u64::from(declared_len));
                out.extend_from_slice(&(stream.len() as u64).to_le_bytes());
                out.extend(stream);
            }
        }
        out
    }

    /// `payload` decoded through this method's payload decoder, bounded by
    /// [`Self::ceiling`].
    ///
    /// # Errors
    ///
    /// Whatever the method's decoder returns.
    pub fn decode(self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        match self {
            Self::Lz => codec::decode(payload, self.ceiling()),
            Self::Cm => cm_path::decode_payload(payload, self.ceiling()),
        }
    }
}

/// One timed hostile decode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    /// The method decoded.
    pub method: Method,
    /// Bytes the payload declared and the decode returned.
    pub declared_len: u32,
    /// Wall time of the one decode call.
    pub seconds: f64,
}

impl Reading {
    /// Wall time per declared byte, in nanoseconds.
    #[must_use]
    pub fn ns_per_byte(&self) -> f64 {
        self.seconds * 1e9 / f64::from(self.declared_len)
    }

    /// The line `hostile_decode` prints for this reading.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "{}: {} bytes declared, {:.2} s, {:.0} ns/byte",
            self.method.name(),
            self.declared_len,
            self.seconds,
            self.ns_per_byte()
        )
    }
}

/// Times one decode of [`Method::payload`] at `declared_len`, the payload
/// built before the timer starts.
///
/// One call, not a best of several: at the ceilings a call takes minutes,
/// so timer noise is far below the runner's own variance.
///
/// # Panics
///
/// If the decode errs or returns other than `declared_len` bytes: the
/// payload no longer drives the method to `declared_len`, and the time
/// would be a rejection's.
#[must_use]
pub fn time(method: Method, declared_len: u32) -> Reading {
    let payload = method.payload(declared_len);
    let start = Instant::now();
    let decoded = black_box(method.decode(black_box(&payload)));
    let seconds = start.elapsed().as_secs_f64();
    let decoded = decoded.unwrap_or_else(|err| {
        panic!(
            "the hostile {} payload declaring {declared_len} bytes was rejected: {err:?}",
            method.name()
        )
    });
    assert_eq!(
        decoded.len(),
        declared_len as usize,
        "the hostile {} payload decoded short of its declared length",
        method.name()
    );
    Reading {
        method,
        declared_len,
        seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::{Method, time};

    #[test]
    fn each_payload_decodes_to_its_declared_length() {
        for method in Method::ALL {
            for declared_len in [1, 4096, 4097, 70_000] {
                let reading = time(method, declared_len);
                assert_eq!(reading.declared_len, declared_len, "{method:?}");
            }
        }
    }

    #[test]
    fn the_lz_payload_codes_no_stream_and_one_token_per_byte() {
        let payload = Method::Lz.payload(70_000);
        // Selector, declared length, token count, coded length.
        assert_eq!(payload.len(), 2 + 4 + 4 + 8);
        assert_eq!(payload[2..6], payload[6..10]);
    }

    #[test]
    fn the_cm_payload_holds_the_fewest_stream_bytes_its_decoder_admits() {
        for declared_len in [4096, 4097] {
            let payload = Method::Cm.payload(declared_len);
            let mut short = payload.clone();
            short.pop();
            let coded_len = u64::from_le_bytes(short[6..14].try_into().unwrap()) - 1;
            short[6..14].copy_from_slice(&coded_len.to_le_bytes());
            assert_eq!(
                Method::Cm.decode(&short),
                Err(mothergod::Error::Truncated),
                "{declared_len}"
            );
        }
    }

    #[test]
    fn a_payload_past_the_ceiling_is_refused() {
        for method in Method::ALL {
            let max = method.ceiling();
            assert_eq!(
                method.decode(&method.payload(max + 1)),
                Err(mothergod::Error::TooLarge { len: max + 1, max }),
                "{method:?}"
            );
        }
    }
}
