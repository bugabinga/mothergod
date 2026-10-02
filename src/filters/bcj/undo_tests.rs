use super::*;

/// Feeding [`Undo`] the output of [`encode`] one byte at a time,
/// then [`Undo::finish`], must reproduce the original input exactly,
/// differentially against the batch [`decode`] this type shadows.
fn undo_matches_decode(data: &[u8]) {
    let encoded = encode(data);
    let mut undo = Undo::try_new().unwrap();
    let mut streamed = Vec::with_capacity(data.len());
    for &byte in &encoded {
        streamed.extend_from_slice(undo.apply(byte).as_slice());
    }
    streamed.extend_from_slice(undo.finish().as_slice());
    assert_eq!(streamed, decode(&encoded));
    assert_eq!(streamed, data);
}

#[test]
fn matches_decode_empty() {
    undo_matches_decode(&[]);
}

#[test]
fn matches_decode_too_short_for_any_instruction() {
    undo_matches_decode(&[0xE8, 0x01, 0x02, 0x03]);
}

#[test]
fn matches_decode_no_opcode_present() {
    undo_matches_decode(&[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
}

#[test]
fn matches_decode_various_data() {
    let data: Vec<u8> = (0..=255u8).cycle().take(2000).collect();
    undo_matches_decode(&data);
}

#[test]
fn matches_decode_adjacent_instructions() {
    let mut data = vec![];
    for _ in 0..20 {
        data.push(0xE8);
        data.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    }
    undo_matches_decode(&data);
}

#[test]
fn matches_decode_wrapping_overflow() {
    undo_matches_decode(&[0xE8, 0xFF, 0xFF, 0xFF, 0xFF]);
}

#[test]
fn matches_decode_e9_jmp() {
    undo_matches_decode(&[0xE9, 0x10, 0x00, 0x00, 0x00]);
}
