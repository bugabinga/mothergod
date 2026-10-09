//! Which [`crate::filters::transpose::encode`] column a transposed-stream
//! position belongs to, for the column-index-keyed literal expert
//! ([`crate::literal::ColumnExpertState`], `research/JOURNAL.md` S1-P5,
//! ADR-0046).
//!
//! The mixer's `position & 3` alignment expert knows a period-4 phase, not
//! which column a byte is in. [`column_of`] is the arithmetic that context
//! needs. [`bank_of`] wraps its result into a fixed-size bank space: a
//! decoder reads `columns` from untrusted compressed input, so bank storage
//! sizes from a constant, never from `columns` (CLAUDE.md hard rule 2).

use std::num::NonZeroUsize;

/// Returns which column (`0..columns.get()`) the byte at `position` in
/// [`crate::filters::transpose::encode`]'s output belongs to, given the
/// pre-transpose data length `len`.
///
/// `transpose::encode` groups by column contiguously: column `c` collects
/// every `data[i]` with `i % columns.get() == c`, in increasing `i` order,
/// one column fully before the next. Column `c`'s length is therefore
/// `len / columns.get()` (`rows`), plus one more for the first
/// `len % columns.get()` columns (`long_columns`). This inverts that
/// grouping without replaying the loop: the `long_columns` longer columns
/// sit first in the output (each `rows + 1` bytes wide), then the remaining
/// columns (each `rows` bytes wide).
///
/// # Panics
///
/// Panics if `position >= len`. On the encode path every caller already
/// knows the transposed stream's length by construction (`transpose::encode`
/// never changes it), so an out-of-range `position` there is a caller bug.
/// On the decode path (`codec.rs`'s column-expert literal decode) `len` is
/// `declared_len`, read from untrusted compressed input, so the caller must
/// establish `position < len` itself before calling: `codec.rs` does this
/// with `ensure_room(output.len(), 1, declared_len)?` immediately before
/// each call, which rejects a token that would grow `output` past
/// `declared_len` and so guarantees `context.position < declared_len` by
/// the time `column_of` runs.
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

/// [`column_of`] wrapped into a fixed-size bank space,
/// `column % max_banks.get()`: the bank the byte at `position` in a
/// `columns`-wide, `len`-long transposed stream belongs to.
///
/// Bank storage sizes from `max_banks` alone, never from the frame's
/// declared `columns`, so a hostile frame cannot drive unbounded allocation
/// (CLAUDE.md hard rule 2). Wrapping is the convention `literal.rs`'s
/// experts already use to bound a context (`ORDER2_BASE`'s `& 0xFFF`,
/// `ALIGN_BASE`'s `position & 3`): real separation for a modest column
/// count, aliasing for an adversarial one.
///
/// Same panic condition as [`column_of`]: `position < len` must already hold.
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
