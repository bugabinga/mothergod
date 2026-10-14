//! Which [`crate::filters::transpose::encode`] column a transposed-stream
//! position belongs to, for the column-keyed literal expert
//! ([`crate::literal::ColumnExpertState`], ADR-0046).

use std::num::NonZeroUsize;

/// The column (`0..columns.get()`) holding the byte at `position` of
/// [`crate::filters::transpose::encode`]'s output, for pre-transpose data of
/// length `len`.
///
/// The output holds column `c` contiguously: `len / columns` bytes, one more
/// for the first `len % columns` columns. Those longer columns come first.
///
/// # Panics
///
/// Panics if `position >= len`. A decoder reads `len` from untrusted input,
/// so it must establish `position < len` before calling: `decode_tokens`
/// does, with `ensure_room` before each literal.
#[must_use]
pub fn column_of(position: usize, columns: NonZeroUsize, len: usize) -> usize {
    assert!(
        position < len,
        "position must be within the transposed stream"
    );
    let columns = columns.get();
    let rows = len / columns;
    let long_columns = len % columns;
    let long_span = long_columns * (rows + 1);
    if position < long_span {
        position / (rows + 1)
    } else {
        long_columns + (position - long_span) / rows
    }
}

/// [`column_of`] wrapped to `max_banks` banks: bank storage sizes from
/// `max_banks`, never from the frame's declared `columns`, so a hostile
/// frame cannot drive unbounded allocation (CLAUDE.md hard rule 2). A
/// modest column count keeps its own bank; an adversarial one aliases.
///
/// Panics as [`column_of`] does.
#[must_use]
pub fn bank_of(
    position: usize,
    columns: NonZeroUsize,
    len: usize,
    max_banks: NonZeroUsize,
) -> usize {
    column_of(position, columns, len) % max_banks.get()
}

#[cfg(test)]
mod tests;
