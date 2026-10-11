//! Streaming syntactic field-class classifier for JSON-shaped bytes:
//! [`FieldClass`], [`FieldState`], [`field_bank`]. Where [`crate::column`]
//! keys a context on a byte's position modulo a stride, this tracks what
//! kind of field the preceding bytes suggest, for records with no fixed
//! stride. Cheap rolling state, no lookahead, the shape
//! [`crate::literal::advance_word_hash`] uses for its alnum-run signal.
//!
//! Standalone: pure functions over a byte stream, not wired into
//! [`crate::literal`] or [`crate::codec`]. The wiring was measured and
//! rejected (`research/JOURNAL.md` S2-R15).

/// A byte's syntactic role in a JSON-shaped stream, independent of any
/// fixed column position.
///
/// Four classes, chosen to separate exactly the confusions a fixed-stride
/// context cannot: field delimiters, inter-field padding, numeric literal
/// digits, and everything else (which includes every byte of quoted-string
/// content, since a string's own letters carry no field-type signal this
/// classifier can cheaply add beyond "this is text").
///
/// Discriminants are [`field_bank`]'s class index.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FieldClass {
    /// Space, tab, newline, or carriage return, outside a quoted string.
    /// The default: the class assigned to the very first byte's incoming
    /// state, before anything has been observed.
    #[default]
    Whitespace = 0,
    /// A field/record delimiter outside a quoted string: `{ } [ ] : , "`.
    /// Includes the quote byte itself, both the one that opens a string
    /// and the one that closes it.
    Structural = 1,
    /// A digit, sign, or decimal point that could be part of a numeric
    /// literal, outside a quoted string: `-`, `.`, `0..=9`.
    Numeric = 2,
    /// Everything else: every byte inside a quoted string (including its
    /// own structural-looking characters, once escaped), bare identifiers,
    /// and any byte outside a quoted string this classifier has no
    /// sharper class for.
    Text = 3,
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
            let closes = !self.escaped && byte == b'"';
            return Self {
                in_quotes: !closes,
                escaped: !self.escaped && byte == b'\\',
                class: if closes {
                    FieldClass::Structural
                } else {
                    FieldClass::Text
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
/// four variants, times whether the state was inside a quoted string.
pub const FIELD_BANKS: usize = 8;

/// Maps a [`FieldState`] into `0..FIELD_BANKS`: the class (`0..4`) plus
/// whether the state is inside a quoted string (`+4`), so "text because
/// it's unclassified" and "text because it's string content" occupy
/// different banks even though [`FieldState::advance`] gives both the same
/// [`FieldClass::Text`].
#[must_use]
pub fn field_bank(state: FieldState) -> usize {
    state.class as usize | (usize::from(state.in_quotes) << 2)
}

#[cfg(test)]
mod tests;
