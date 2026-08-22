//! Fixed-stride delta filter (`JOURNAL` S1-A2, first M1 port slice).
//!
//! Delta filters trade each byte for its wrapping difference from the byte
//! `stride` positions earlier. They win on data with fixed-width records
//! where corresponding columns of adjacent rows are numerically close (for
//! example interleaved multi-channel audio samples); `JOURNAL` S1-R1 shows
//! the same transform loses on text, where the numeric difference of
//! letters is *more* scattered than the letters themselves. Trial selection
//! (`JOURNAL` S1-A1), not a static rule, decides when to apply this filter
//! — this module only provides the reversible transform itself. Stride
//! selection, the other filter kinds (transpose, BCJ, base64-unwrap,
//! reverse), and wiring behind a [`crate::Method`] variant are follow-up
//! slices (`JOURNAL` S2-D2).

use std::num::NonZeroUsize;

/// Replaces each byte at index `i >= stride` with its wrapping difference
/// from the byte at `i - stride`; the first `stride` bytes are left as-is.
///
/// Reversible by [`decode`] with the same `stride`. `stride` is
/// [`NonZeroUsize`] because a zero stride would subtract every byte from
/// itself, zeroing the data instead of transforming it reversibly — the
/// type rules out that misuse instead of a runtime check.
#[must_use]
pub fn encode(data: &[u8], stride: NonZeroUsize) -> Vec<u8> {
    let stride = stride.get();
    let mut out = data.to_vec();
    for i in stride..out.len() {
        out[i] = data[i].wrapping_sub(data[i - stride]);
    }
    out
}

/// Inverts [`encode`] with the same `stride`.
#[must_use]
pub fn decode(data: &[u8], stride: NonZeroUsize) -> Vec<u8> {
    let stride = stride.get();
    let mut out = data.to_vec();
    for i in stride..out.len() {
        out[i] = out[i].wrapping_add(out[i - stride]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nz(v: usize) -> NonZeroUsize {
        NonZeroUsize::new(v).unwrap()
    }

    #[test]
    fn roundtrip_empty() {
        let s = nz(1);
        assert_eq!(decode(&encode(&[], s), s), Vec::<u8>::new());
    }

    #[test]
    fn roundtrip_single_byte() {
        let s = nz(1);
        assert_eq!(decode(&encode(&[42], s), s), vec![42]);
    }

    #[test]
    fn roundtrip_shorter_than_stride() {
        // stride longer than the data: every byte is left untouched, both
        // directions are the identity.
        let data = vec![1, 2, 3];
        let s = nz(8);
        assert_eq!(encode(&data, s), data);
        assert_eq!(decode(&data, s), data);
    }

    #[test]
    fn roundtrip_various_strides() {
        let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        for stride in [1usize, 2, 3, 4, 8, 16, 96, 255, 1000, 1001] {
            let s = nz(stride);
            let encoded = encode(&data, s);
            assert_eq!(decode(&encoded, s), data, "stride {stride}");
        }
    }

    #[test]
    fn encode_is_identity_for_first_stride_bytes() {
        let data = vec![10, 20, 30, 40, 50];
        let s = nz(2);
        let encoded = encode(&data, s);
        assert_eq!(&encoded[..2], &data[..2]);
    }

    #[test]
    fn wrapping_arithmetic_survives_overflow() {
        // Constructed so intermediate subtraction underflows u8; must still
        // round-trip losslessly via wrapping ops.
        let data = vec![0u8, 255, 1, 254, 2];
        let s = nz(1);
        assert_eq!(decode(&encode(&data, s), s), data);
    }
}
