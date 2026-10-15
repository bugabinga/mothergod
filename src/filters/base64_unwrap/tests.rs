use super::*;

#[test]
fn encode_of_empty_is_none() {
    assert_eq!(encode(&[]), None);
}

#[test]
fn encode_unwraps_a_single_group() {
    // "YWJj" == base64("abc"): four bytes, nothing to recoup.
    assert_eq!(encode(b"YWJj"), Some(b"abc".to_vec()));
}

#[test]
fn encode_unwraps_valid_base64() {
    // "aGVsbG8gd29ybGQ=" == base64("hello world")
    let data = b"aGVsbG8gd29ybGQ=".as_slice();
    let unwrapped = encode(data).expect("valid base64 unwraps");
    assert_eq!(unwrapped, b"hello world");
    assert_eq!(decode(&unwrapped), data);
}

#[test]
fn encode_rejects_non_base64() {
    assert_eq!(encode(b"not base64 data!"), None);
}

#[test]
fn encode_rejects_invalid_padding_placement() {
    // '=' before the final group: alphabet-scan-eligible (same
    // length class) but not valid base64.
    assert_eq!(encode(b"AB==CDEF"), None);
}

#[test]
fn encode_rejects_non_canonical_padding_bits() {
    // "aGVsbG9=" decodes to 5 bytes but its trailing group carries
    // non-zero padding bits, so re-encoding would not reproduce
    // this exact string: must be rejected, not silently accepted.
    let data = b"aGVsbG9=".as_slice();
    let decoded_roundtrips = strict_decode(data).is_some_and(|d| b64_encode(&d) == data);
    assert!(!decoded_roundtrips, "fixture must exercise the guard");
    assert_eq!(encode(data), None);
}

#[test]
fn encode_rejects_a_line_wrapped_payload() {
    // A newline is outside the alphabet: the filter covers whole-input
    // base64 only, so MIME-style wrapping declines it.
    assert_eq!(encode(b"YWJj\nZGVm"), None);
    assert_eq!(encode(b"YWJjZGVm\n"), None);
}

#[test]
fn roundtrip_all_padding_lengths() {
    for msg in [
        "a",
        "ab",
        "abc",
        "abcd",
        "abcde",
        "abcdef",
        "abcdefg",
        "abcdefgh",
        "abcdefghi",
    ] {
        let wrapped = b64_encode(msg.as_bytes());
        let unwrapped = encode(&wrapped).unwrap_or_else(|| panic!("message {msg:?} unwraps"));
        assert_eq!(unwrapped, msg.as_bytes());
        assert_eq!(decode(&unwrapped), wrapped, "message {msg:?}");
    }
}

#[test]
fn roundtrip_binary_payload() {
    let raw: Vec<u8> = (0..=255u8).cycle().take(300).collect();
    let wrapped = b64_encode(&raw);
    assert_eq!(encode(&wrapped).as_deref(), Some(raw.as_slice()));
    assert_eq!(decode(&raw), wrapped);
}

#[test]
fn decode_is_total_over_every_length() {
    // The wire form is any byte string, so no filtered stream is invalid:
    // the padded text's length is 4 * ceil(len / 3) for every length.
    for len in 0..=9 {
        let filtered = vec![0xA5; len];
        assert_eq!(decode(&filtered).len(), len.div_ceil(3) * 4, "len {len}");
    }
}

#[test]
fn try_decode_matches_decode() {
    for len in [0, 1, 2, 3, 4, 100, 301] {
        let filtered: Vec<u8> = (0..=255u8).cycle().take(len).collect();
        assert_eq!(
            try_decode(&filtered).unwrap(),
            decode(&filtered),
            "len {len}"
        );
    }
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
fn strict_decode_rejects_empty_input() {
    assert_eq!(strict_decode(&[]), None);
}

#[test]
fn strict_decode_rejects_length_not_a_multiple_of_four() {
    assert_eq!(strict_decode(b"abc"), None);
}

#[test]
fn strict_decode_rejects_padding_in_a_non_final_group() {
    // Same fixture as `encode_rejects_invalid_padding_placement`,
    // asserted directly against `strict_decode`. `encode`'s outer
    // round-trip check (`b64_encode(&decoded) == data`) independently
    // rejects a wrongly-decoded result here, so it masks a mutant
    // that drops this guard (`||` -> `&&`, or `pad > 0` -> `pad < 0`,
    // both collapsing the guard to `pad > 2` alone) from any test
    // that only observes `encode`'s output.
    assert_eq!(strict_decode(b"AB==CDEF"), None);
}
