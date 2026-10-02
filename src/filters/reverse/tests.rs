use super::*;

#[test]
fn roundtrip_empty() {
    assert_eq!(decode(&encode(&[])), Vec::<u8>::new());
}

#[test]
fn roundtrip_single_byte() {
    assert_eq!(decode(&encode(&[42])), vec![42]);
}

#[test]
fn encode_reverses_byte_order() {
    let data = vec![1, 2, 3, 4, 5];
    assert_eq!(encode(&data), vec![5, 4, 3, 2, 1]);
}

#[test]
fn encode_twice_is_identity() {
    let data = vec![1, 2, 3, 4, 5];
    assert_eq!(encode(&encode(&data)), data);
}

#[test]
fn roundtrip_various_lengths() {
    let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    for len in [0usize, 1, 2, 3, 7, 255, 256, 1000] {
        let slice = &data[..len];
        assert_eq!(decode(&encode(slice)), slice, "len {len}");
    }
}

#[test]
fn palindrome_is_a_fixed_point() {
    let data = vec![1, 2, 3, 2, 1];
    assert_eq!(encode(&data), data);
}
