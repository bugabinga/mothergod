//! Lightweight syntactic field-class classifier: [`FieldClass`],
//! [`FieldState`], [`field_bank`]. Standalone primitive, first slice of
//! `research/JOURNAL.md` S1-P2's own named remaining scope after fourteen
//! prior slices closed off both the intra-round pricing angle (S2-A51
//! through S2-R5) and the match-finder search-quality angle (S2-R10): "a
//! modeling primitive that reaches structure a binary-tree parse cannot
//! (e.g. a schema-aware/typed-field literal model, in the spirit of
//! S1-P5's per-column direction but for pre-transpose or mixed-type
//! records)". Targets the lead's own named residue: sqlite/json/jsonl.
//! Not yet wired into [`crate::literal`] or [`crate::codec`].
//!
//! **Why not [`crate::column`]'s own approach.** S1-P5's per-column expert
//! (`column::column_of`/`column_bank`, `ADR-0046`) keys a context on a
//! byte's position modulo a fixed stride, real *after*
//! [`crate::filters::transpose::encode`] has already regrouped a
//! fixed-width record format into column-major order. JSON, JSONL, and a
//! sqlite dump's row-major bytes have no such stride: a JSON object's
//! fields are delimited by syntax (`{ } [ ] : , "`), not by a fixed byte
//! offset, and a single row mixes field types (a quoted string next to a
//! bare number) that a stride-keyed context cannot separate. This module
//! answers the "for pre-transpose or mixed-type records" half of S1-P2's
//! own framing directly: instead of *where* a byte sits, it tracks *what
//! kind of field* the bytes immediately before it suggest, from a small
//! streaming state machine over JSON-shaped syntax (quote-toggling,
//! backslash-escape absorption, digit/structural/whitespace/text
//! classification), the same "cheap rolling state, no lookahead" shape
//! [`crate::literal::advance_word_hash`] already uses for its own
//! alnum-run signal.
//!
//! **Remaining scope.** This module is standalone: [`FieldState`] and
//! [`field_bank`] are pure functions over a byte stream, proven only by
//! their own unit tests below, not measured against
//! `bench::baseline` and not blended into [`crate::literal::Literal`]'s
//! mix. The next slice owed here, the same before-wiring ideal-cost
//! pairing `research/JOURNAL.md` S2-A69 used for S1-P5's column expert
//! (blend a field-class-keyed eighth expert into the shipped six-expert
//! mix), was tried and rejected (`JOURNAL` S2-R15): net train improvement
//! on both named targets (`sqlite_like_records` −0.001709 b/B,
//! `json_records` −0.000936 b/B) and on one sealed-only kind (`access_log`
//! −0.004075 b/B), but the other sealed-only kind, `gradient_image`,
//! regressed (+0.001598 b/B) — a validation regression fails corpus
//! policy's accept rule outright, independent of how small the train wins
//! are. Mechanism: `gradient_image`'s raw pixel bytes carry no JSON-shaped
//! syntax for [`FieldState::advance`] to track, so its bank assignment is
//! close to noise there, and blending that noise in as an eighth expert
//! costs a small but real mixing tax the six real experts' own weights do
//! not fully suppress. What is left, per the same shape S1-P3 reached
//! after its own repeated rejections (`JOURNAL` S2-R6's own closing note):
//! unclear, since the narrower "blended in, not replacing" hypothesis this
//! slice tested was the lead's own best-reasoned remaining branch.

/// A byte's syntactic role in a JSON-shaped stream, independent of any
/// fixed column position.
///
/// Four classes, chosen to separate exactly the confusions a fixed-stride
/// context cannot: field delimiters, inter-field padding, numeric literal
/// digits, and everything else (which includes every byte of quoted-string
/// content, since a string's own letters carry no field-type signal this
/// classifier can cheaply add beyond "this is text").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FieldClass {
    /// Space, tab, newline, or carriage return, outside a quoted string.
    /// The default: the class assigned to the very first byte's incoming
    /// state, before anything has been observed.
    #[default]
    Whitespace,
    /// A field/record delimiter outside a quoted string: `{ } [ ] : , "`.
    /// Includes the quote byte itself, both the one that opens a string
    /// and the one that closes it.
    Structural,
    /// A digit, sign, or decimal point that could be part of a numeric
    /// literal, outside a quoted string: `-`, `.`, `0..=9`.
    Numeric,
    /// Everything else: every byte inside a quoted string (including its
    /// own structural-looking characters, once escaped), bare identifiers,
    /// and any byte outside a quoted string this classifier has no
    /// sharper class for.
    Text,
}

/// Rolling classifier state: [`FieldState::advance`] folds in one byte at
/// a time, the same shape [`crate::literal::Context::after_literal`] folds
/// a byte into `word_hash`.
///
/// `in_quotes` and `escaped` together track just enough to tell a
/// structural quote from string content: `escaped` is only ever `true`
/// for the one state produced immediately after an unescaped `\` inside a
/// quoted string, absorbing the next byte as [`FieldClass::Text`]
/// regardless of what it is (so `\"` inside a string never closes it, and
/// `\\` never leaves a dangling escape).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FieldState {
    /// Whether the byte just classified left the state inside an open,
    /// unterminated quoted string.
    pub in_quotes: bool,
    /// Whether the byte just classified was an unescaped `\` inside a
    /// quoted string, so the next byte is absorbed as string content
    /// unconditionally.
    pub escaped: bool,
    /// This state's own class: the class of the byte that produced it,
    /// or [`FieldClass::default`] for the state before any byte has been
    /// observed.
    pub class: FieldClass,
}

impl FieldState {
    /// The state after folding `byte` in.
    #[must_use]
    pub fn advance(self, byte: u8) -> Self {
        if self.in_quotes {
            if self.escaped {
                return Self {
                    in_quotes: true,
                    escaped: false,
                    class: FieldClass::Text,
                };
            }
            return match byte {
                b'\\' => Self {
                    in_quotes: true,
                    escaped: true,
                    class: FieldClass::Text,
                },
                b'"' => Self {
                    in_quotes: false,
                    escaped: false,
                    class: FieldClass::Structural,
                },
                _ => Self {
                    in_quotes: true,
                    escaped: false,
                    class: FieldClass::Text,
                },
            };
        }
        let class = match byte {
            b'{' | b'}' | b'[' | b']' | b':' | b',' | b'"' => FieldClass::Structural,
            b' ' | b'\t' | b'\n' | b'\r' => FieldClass::Whitespace,
            b'-' | b'.' | b'0'..=b'9' => FieldClass::Numeric,
            _ => FieldClass::Text,
        };
        Self {
            in_quotes: byte == b'"',
            escaped: false,
            class,
        }
    }
}

/// Number of distinct values [`field_bank`] can return: [`FieldClass`]'s
/// four variants, times whether the state that produced them was inside a
/// quoted string, so a caller keying a context bank on this state always
/// sizes its storage from this constant, never from anything unbounded
/// (`crate::column::column_bank`'s own convention, CLAUDE.md hard rule 2 —
/// moot here since every input to [`field_bank`] is already bounded by
/// construction, but kept as the one named constant a future bank-sizing
/// caller reads instead of re-deriving `4 * 2`).
pub const FIELD_BANKS: usize = 8;

/// Maps a [`FieldState`] into `0..FIELD_BANKS`: the class (`0..4`) plus
/// whether the state is inside a quoted string (`+4`), so "text because
/// it's unclassified" and "text because it's string content" occupy
/// different banks even though [`FieldState::advance`] gives both the same
/// [`FieldClass::Text`].
#[must_use]
pub fn field_bank(state: FieldState) -> usize {
    let class_index = match state.class {
        FieldClass::Whitespace => 0,
        FieldClass::Structural => 1,
        FieldClass::Numeric => 2,
        FieldClass::Text => 3,
    };
    class_index | (usize::from(state.in_quotes) << 2)
}

#[cfg(test)]
mod tests {
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
}
