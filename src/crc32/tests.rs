use super::*;

#[test]
fn matches_the_published_check_values() {
    // The CRC catalogue's check value for CRC-32/ISO-HDLC, the empty
    // message (the initial and final XOR cancel), and one byte.
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"a"), 0xE8B7_BE43);
}

#[test]
fn every_single_bit_flip_changes_the_value() {
    let message: Vec<u8> = (0..64u8).map(|i| i.wrapping_mul(37)).collect();
    let want = crc32(&message);
    for bit in 0..message.len() * 8 {
        let mut flipped = message.clone();
        flipped[bit / 8] ^= 1 << (bit % 8);
        assert_ne!(crc32(&flipped), want, "flip of bit {bit}");
    }
}
