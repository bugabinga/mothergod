use super::*;
use crate::test_support::nz;

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
    // stride longer than the data: every byte is left untouched,
    // both directions are the identity.
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
    // Constructed so intermediate subtraction underflows u8; must
    // still round-trip losslessly via wrapping ops.
    let data = vec![0u8, 255, 1, 254, 2];
    let s = nz(1);
    assert_eq!(decode(&encode(&data, s), s), data);
}
