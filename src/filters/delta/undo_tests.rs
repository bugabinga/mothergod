use super::*;
use crate::test_support::nz;

/// Feeding [`Undo`] the output of [`encode`] one byte at a time
/// must reproduce the original input exactly, differentially
/// against the batch [`decode`] this type shadows.
fn undo_matches_decode(data: &[u8], stride: NonZeroUsize) {
    let encoded = encode(data, stride);
    let mut undo = Undo::try_new(stride).unwrap();
    let streamed: Vec<u8> = encoded.iter().map(|&byte| undo.apply(byte)).collect();
    assert_eq!(streamed, decode(&encoded, stride));
    assert_eq!(streamed, data);
}

#[test]
fn matches_decode_empty() {
    undo_matches_decode(&[], nz(1));
}

#[test]
fn matches_decode_shorter_than_stride() {
    undo_matches_decode(&[1, 2, 3], nz(8));
}

#[test]
fn matches_decode_various_strides() {
    let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    for stride in [1usize, 2, 3, 4, 8, 16, 96, 255] {
        undo_matches_decode(&data, nz(stride));
    }
}

#[test]
fn matches_decode_wrapping_overflow() {
    undo_matches_decode(&[0u8, 255, 1, 254, 2], nz(1));
}
