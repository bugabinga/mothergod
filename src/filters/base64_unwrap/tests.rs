use super::*;

#[test]
fn roundtrip_empty() {
    assert_eq!(decode(&encode(&[])), Vec::<u8>::new());
}

#[test]
fn roundtrip_too_short_to_try() {
    // Below MIN_LEN: always passed through, even though "YWJj" (7
    // bytes short by one) would otherwise decode cleanly.
    let data = b"YWJj".as_slice();
    assert_eq!(&decode(&encode(data)), data);
    assert_eq!(encode(data)[0], 0);
}

#[test]
fn encode_unwraps_valid_base64() {
    // "aGVsbG8gd29ybGQ=" == base64("hello world")
    let data = b"aGVsbG8gd29ybGQ=".as_slice();
    let encoded = encode(data);
    assert_eq!(encoded[0], 1);
    assert_eq!(&encoded[1..], b"hello world");
    assert_eq!(decode(&encoded), data);
}

#[test]
fn encode_passes_through_non_base64() {
    let data = b"not base64 data!".as_slice();
    let encoded = encode(data);
    assert_eq!(encoded[0], 0);
    assert_eq!(&encoded[1..], data);
    assert_eq!(decode(&encoded), data);
}

#[test]
fn encode_passes_through_invalid_padding_placement() {
    // '=' before the final group: alphabet-scan-eligible (same
    // length class) but not valid base64.
    let data = b"AB==CDEF".as_slice();
    let encoded = encode(data);
    assert_eq!(encoded[0], 0);
    assert_eq!(decode(&encoded), data);
}

#[test]
fn encode_passes_through_non_canonical_padding_bits() {
    // "aGVsbG9=" decodes to 5 bytes but its trailing group carries
    // non-zero padding bits, so re-encoding would not reproduce
    // this exact string: must pass through, not silently accept.
    let data = b"aGVsbG9=".as_slice();
    let decoded_roundtrips = try_decode(data).is_some_and(|d| b64_encode(&d) == data);
    assert!(!decoded_roundtrips, "fixture must exercise the guard");
    let encoded = encode(data);
    assert_eq!(encoded[0], 0);
    assert_eq!(decode(&encoded), data);
}

#[test]
fn roundtrip_all_padding_lengths() {
    // Shorter messages wrap to fewer than MIN_LEN base64 bytes and
    // are covered by `roundtrip_too_short_to_try` instead.
    for msg in [
        "abcd",
        "abcde",
        "abcdef",
        "abcdefg",
        "abcdefgh",
        "abcdefghi",
    ] {
        let wrapped = b64_encode(msg.as_bytes());
        let encoded = encode(&wrapped);
        assert_eq!(encoded[0], 1, "message {msg:?} should unwrap");
        assert_eq!(&encoded[1..], msg.as_bytes());
        assert_eq!(decode(&encoded), wrapped, "message {msg:?}");
    }
}

#[test]
fn decode_of_empty_is_empty() {
    assert_eq!(decode(&[]), Vec::<u8>::new());
}

#[test]
fn roundtrip_binary_payload() {
    let raw: Vec<u8> = (0..=255u8).cycle().take(300).collect();
    let wrapped = b64_encode(&raw);
    let encoded = encode(&wrapped);
    assert_eq!(encoded[0], 1);
    assert_eq!(decode(&encoded), wrapped);
}

#[test]
fn is_base64_byte_rejects_non_alphabet_characters() {
    assert!(is_base64_byte(b'A'));
    assert!(is_base64_byte(b'='));
    assert!(!is_base64_byte(b'!'));
}

#[test]
fn decode_char_inverts_alphabet_and_rejects_everything_else() {
    for (value, &symbol) in ALPHABET.iter().enumerate() {
        let value = u8::try_from(value).unwrap();
        assert_eq!(decode_char(symbol), Some(value), "symbol {symbol:?}");
    }
    for b in 0..=u8::MAX {
        if !ALPHABET.contains(&b) {
            assert_eq!(decode_char(b), None, "byte {b:?}");
        }
    }
}

#[test]
fn try_decode_rejects_empty_input() {
    assert_eq!(try_decode(&[]), None);
}

#[test]
fn try_decode_rejects_length_not_a_multiple_of_four() {
    assert_eq!(try_decode(b"abc"), None);
}

#[test]
fn try_decode_rejects_padding_in_a_non_final_group() {
    // Same fixture as `encode_passes_through_invalid_padding_placement`,
    // asserted directly against `try_decode`. `encode`'s outer
    // round-trip check (`b64_encode(&decoded) == data`) independently
    // rejects a wrongly-decoded result here, so it masks a mutant
    // that drops this guard (`||` -> `&&`, or `pad > 0` -> `pad < 0`,
    // both collapsing the guard to `pad > 2` alone) from any test
    // that only observes `encode`'s output.
    assert_eq!(try_decode(b"AB==CDEF"), None);
}
