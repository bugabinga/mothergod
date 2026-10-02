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
mod tests;
