use super::*;

#[test]
fn roundtrip_empty() {
    assert_eq!(decode(&encode(&[])), Vec::<u8>::new());
}

#[test]
fn roundtrip_too_short_for_any_instruction() {
    // An 0xE8 opcode with fewer than 4 trailing bytes has no full
    // operand to rewrite; both directions are the identity.
    let data = vec![0xE8, 0x01, 0x02, 0x03];
    assert_eq!(encode(&data), data);
    assert_eq!(decode(&data), data);
}

#[test]
fn encode_rewrites_e8_call_operand() {
    // call rel32 at offset 0, relative operand 0x10 -> absolute
    // target 0x10 + (0 + 5) = 0x15.
    let data = vec![0xE8, 0x10, 0x00, 0x00, 0x00];
    assert_eq!(encode(&data), vec![0xE8, 0x15, 0x00, 0x00, 0x00]);
}

#[test]
fn encode_rewrites_e9_jmp_operand() {
    let data = vec![0xE9, 0x10, 0x00, 0x00, 0x00];
    assert_eq!(encode(&data), vec![0xE9, 0x15, 0x00, 0x00, 0x00]);
}

#[test]
fn encode_is_identity_when_no_opcode_present() {
    let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
    assert_eq!(encode(&data), data);
}

#[test]
fn roundtrip_various_data() {
    // 0xE8 == 232 and 0xE9 == 233 fall inside this cycle, so the
    // scan is actually exercised, not just a no-op pass-through.
    let data: Vec<u8> = (0..=255u8).cycle().take(2000).collect();
    assert_eq!(decode(&encode(&data)), data);
}

#[test]
fn roundtrip_adjacent_instructions() {
    let mut data = vec![];
    for _ in 0..20 {
        data.push(0xE8);
        data.extend_from_slice(&[0x00, 0x01, 0x00, 0x00]);
    }
    assert_eq!(decode(&encode(&data)), data);
}

#[test]
fn roundtrip_wrapping_overflow() {
    // Operand large enough that adding the post-instruction
    // address wraps u32; must still round-trip losslessly.
    let data = vec![0xE8, 0xFF, 0xFF, 0xFF, 0xFF];
    assert_eq!(decode(&encode(&data)), data);
}
