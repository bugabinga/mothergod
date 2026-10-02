use super::{FIELD_BANKS, FieldClass, FieldState, field_bank};

/// Folds every byte of `bytes` through [`FieldState::advance`] from
/// the default state, returning the final state.
fn fold(bytes: &[u8]) -> FieldState {
    bytes
        .iter()
        .fold(FieldState::default(), |state, &byte| state.advance(byte))
}

#[test]
fn default_state_is_whitespace_outside_quotes() {
    let state = FieldState::default();
    assert_eq!(state.class, FieldClass::Whitespace);
    assert!(!state.in_quotes);
    assert!(!state.escaped);
}

#[test]
fn structural_bytes_classify_outside_quotes() {
    for &byte in b"{}[]:," {
        let state = FieldState::default().advance(byte);
        assert_eq!(state.class, FieldClass::Structural, "byte {byte}");
        assert!(!state.in_quotes, "byte {byte} must not open a string");
    }
}

#[test]
fn opening_quote_is_structural_and_enters_quotes() {
    let state = FieldState::default().advance(b'"');
    assert_eq!(state.class, FieldClass::Structural);
    assert!(state.in_quotes);
}

#[test]
fn closing_quote_is_structural_and_leaves_quotes() {
    let state = fold(br#""a""#);
    assert_eq!(state.class, FieldClass::Structural);
    assert!(!state.in_quotes);
}

#[test]
fn digits_and_sign_and_dot_are_numeric_outside_quotes() {
    for &byte in b"-.0123456789" {
        let state = FieldState::default().advance(byte);
        assert_eq!(state.class, FieldClass::Numeric, "byte {byte}");
    }
}

#[test]
fn whitespace_bytes_classify_outside_quotes() {
    for &byte in b" \t\n\r" {
        let state = FieldState::default().advance(byte);
        assert_eq!(state.class, FieldClass::Whitespace, "byte {byte}");
    }
}

#[test]
fn bare_letters_are_text_outside_quotes() {
    let state = FieldState::default().advance(b'x');
    assert_eq!(state.class, FieldClass::Text);
    assert!(!state.in_quotes);
}

#[test]
fn digits_inside_a_quoted_string_are_text_not_numeric() {
    // `"123"`: the digits sit between an opening and (eventually) a
    // closing quote, so they are string content, not a numeric field.
    let state = fold(br#""1"#);
    assert_eq!(state.class, FieldClass::Text);
    assert!(state.in_quotes);
}

#[test]
fn structural_looking_bytes_inside_a_quoted_string_are_text() {
    // `"{"`: a brace inside an open string is string content, not a
    // structural delimiter.
    let state = fold(br#""{"#);
    assert_eq!(state.class, FieldClass::Text);
    assert!(state.in_quotes);
}

#[test]
fn escaped_quote_inside_a_string_does_not_close_it() {
    // `"a\"b"`: the escaped quote is text, and the string stays open
    // through the trailing `b`.
    let state = fold(br#""a\"b"#);
    assert!(state.in_quotes, "escaped quote must not close the string");
    assert_eq!(state.class, FieldClass::Text);
}

#[test]
fn escaped_quote_then_real_close_quote_ends_the_string() {
    let state = fold(br#""a\"b""#);
    assert!(!state.in_quotes);
    assert_eq!(state.class, FieldClass::Structural);
}

#[test]
fn double_backslash_does_not_leave_a_dangling_escape() {
    // `"a\\"` (a backslash then a closing quote): the second `\`
    // completes the escape pair, so the following `"` must close the
    // string rather than being absorbed as escaped text.
    let state = fold(b"\"a\\\\\"");
    assert!(!state.in_quotes);
    assert_eq!(state.class, FieldClass::Structural);
}

#[test]
fn empty_string_reopens_and_closes_immediately() {
    let state = fold(br#""""#);
    assert!(!state.in_quotes);
    assert_eq!(state.class, FieldClass::Structural);
}

#[test]
fn a_realistic_json_record_classifies_every_byte_as_expected() {
    // `{"n":12,"s":"ab"}` — walk it and check the class at each
    // position lines up with what a human reading the JSON would call
    // that byte's field role.
    let bytes = br#"{"n":12,"s":"ab"}"#;
    let expected = [
        FieldClass::Structural, // {
        FieldClass::Structural, // " (opens "n")
        FieldClass::Text,       // n
        FieldClass::Structural, // " (closes "n")
        FieldClass::Structural, // :
        FieldClass::Numeric,    // 1
        FieldClass::Numeric,    // 2
        FieldClass::Structural, // ,
        FieldClass::Structural, // " (opens "s")
        FieldClass::Text,       // s
        FieldClass::Structural, // " (closes "s")
        FieldClass::Structural, // :
        FieldClass::Structural, // " (opens "ab")
        FieldClass::Text,       // a
        FieldClass::Text,       // b
        FieldClass::Structural, // " (closes "ab")
        FieldClass::Structural, // }
    ];
    assert_eq!(bytes.len(), expected.len());
    let mut state = FieldState::default();
    for (i, (&byte, &want)) in bytes.iter().zip(expected.iter()).enumerate() {
        state = state.advance(byte);
        assert_eq!(state.class, want, "byte index {i} ({byte})");
    }
}

#[test]
fn field_bank_never_exceeds_field_banks() {
    let mut state = FieldState::default();
    // Drive every reachable state across a mixed sample so this check
    // is not limited to states reachable from the default alone.
    for &byte in br#"{"a":1,"b":"x\"y"}  -.	"# {
        state = state.advance(byte);
        assert!(field_bank(state) < FIELD_BANKS, "byte {byte}");
    }
}

#[test]
fn field_bank_separates_quoted_text_from_unquoted_text() {
    let quoted = fold(br#""x"#); // inside an open string
    let unquoted = FieldState::default().advance(b'x'); // bare text
    assert_eq!(quoted.class, FieldClass::Text);
    assert_eq!(unquoted.class, FieldClass::Text);
    assert_ne!(
        field_bank(quoted),
        field_bank(unquoted),
        "same FieldClass, different in_quotes, must land in different banks"
    );
}

#[test]
fn field_bank_is_a_pure_function_of_class_and_in_quotes() {
    // Two different byte histories that land on the same
    // (class, in_quotes) pair — `{` and `}`, both Structural and
    // outside any string — must produce the same bank.
    let a = FieldState::default().advance(b'{');
    let b = FieldState::default().advance(b'}');
    assert_eq!(a.class, FieldClass::Structural);
    assert_eq!(b.class, FieldClass::Structural);
    assert!(!a.in_quotes && !b.in_quotes);
    assert_eq!(field_bank(a), field_bank(b));
}
