//! Which [`crate::filters::transpose::encode`] column a transposed-stream
//! position belongs to (`research/JOURNAL.md` S1-P5, per-column modeling
//! after transpose). Standalone primitive, same order S1-P1/S1-P2/S1-P3/
//! S1-P4 each opened their own first slice with (S2-A40, S2-A42, S2-A57,
//! S2-A61); since wired into [`crate::literal::Literal`]
//! (`encode_column`/`decode_column`) and `codec.rs`'s decode loop
//! (`dc32411`, `FORMAT_VERSION` 4, `research/JOURNAL.md` S2-A79).
//!
//! `transpose::encode` regroups a row-major byte stream into column-major
//! order so a downstream model with only short-range context can see a
//! column's own regularity as adjacency (`JOURNAL` S1-A2, `filters.rs`'s
//! own module doc). But that adjacency only carries a column's structure
//! *within* a run of consecutive same-column bytes; the mixer's own
//! `position`-keyed "alignment" expert (`literal.rs`, fixed `position & 3`)
//! has no notion of *which* column a byte is in, only a period-4 phase --
//! useful for interleaved fixed-width records, not for a filter that has
//! already grouped by column. A context keyed on the real column index
//! would let the mixer separate one column's distribution from the next
//! immediately at a column boundary, instead of only after re-adapting
//! from a few bytes of the wrong column's evidence. [`column_of`] is the
//! arithmetic that context needs: which column produced the byte at a
//! given position in the transposed stream. [`column_bank`] is the second
//! piece: a decoder reads `columns` from untrusted compressed input, so a
//! future expert's bank storage must size from a constant, never from
//! `columns` directly (CLAUDE.md hard rule 2) -- `column_bank` wraps
//! [`column_of`]'s unbounded result into a fixed-size bank space, the same
//! convention `literal.rs`'s existing experts already use (`ORDER2_BASE`'s
//! `& 0xFFF`, `ALIGN_BASE`'s `position & 3`). S1-P5 is resolved, real
//! wiring and real-bitstream measurement included; see ADR-0046.

use std::num::NonZeroUsize;

/// Returns which column (`0..columns.get()`) the byte at `position` in
/// [`crate::filters::transpose::encode`]'s output belongs to, given the
/// pre-transpose data length `len`.
///
/// `transpose::encode` groups by column contiguously: column `c` collects
/// every `data[i]` with `i % columns.get() == c`, in increasing `i` order,
/// one column fully before the next. Column `c`'s length is therefore
/// `len / columns.get()` (`rows`), plus one more for the first
/// `len % columns.get()` columns (`long_columns`) -- the rows' leftover
/// bytes, same split `transpose`'s own `encode_groups_by_column` test
/// exercises for `columns=2`. This inverts that grouping without
/// replaying the loop: the `long_columns` longer columns sit first in the
/// output (each `rows + 1` bytes wide), then the remaining columns
/// (each `rows` bytes wide).
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

/// Maps [`column_of`]'s unbounded column index into a fixed-size bank
/// space, `column % max_banks.get()`.
///
/// [`crate::literal::ColumnExpertState`], the column-index-keyed literal
/// expert wired into `codec.rs` (`research/JOURNAL.md` S1-P5, ADR-0046),
/// sizes its bank storage from a constant alone, never from the frame's
/// declared `columns`: a decoder reads `columns` from untrusted compressed
/// input, so sizing bank storage to it directly would let a hostile frame
/// drive unbounded allocation (CLAUDE.md hard rule 2). Wrapping modulo a
/// fixed `max_banks` is the same convention `literal.rs`'s existing
/// experts already use to keep an unbounded context bounded
/// (`ORDER2_BASE`'s `& 0xFFF`, `WORD_BASE`'s `& 0xFFF`, `ALIGN_BASE`'s
/// `position & 3`): real separation for the common case this lead targets
/// (structured data with a modest column count), aliasing rather than
/// allocating for an adversarial one.
#[must_use]
pub fn column_bank(column: usize, max_banks: NonZeroUsize) -> usize {
    column % max_banks.get()
}

/// [`column_of`] then [`column_bank`] in one call: the bank the byte at
/// `position` in a `columns`-wide, `len`-long transposed stream belongs to,
/// wrapped into a fixed `max_banks`-size space. `codec.rs`'s two
/// column-expert literal sites (`EncodeSink` and `decode`'s
/// `FLAG_LITERAL` arm) both need exactly this pair in this order; this is
/// that pairing, named once. Same panic condition as [`column_of`]:
/// `position < len` must already hold.
#[must_use]
pub fn bank_of(
    position: usize,
    columns: NonZeroUsize,
    len: usize,
    max_banks: NonZeroUsize,
) -> usize {
    column_bank(column_of(position, columns, len), max_banks)
}

#[cfg(test)]
mod tests;
