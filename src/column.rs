//! Which [`crate::filters::transpose::encode`] column a transposed-stream
//! position belongs to (`research/JOURNAL.md` S1-P5, per-column modeling
//! after transpose). Standalone primitive, same order S1-P1/S1-P2/S1-P3/
//! S1-P4 each opened their own first slice with (S2-A40, S2-A42, S2-A57,
//! S2-A61): not yet wired into [`crate::literal::Literal`] or `codec.rs`.
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
//! `& 0xFFF`, `ALIGN_BASE`'s `position & 3`). Remaining S1-P5 scope: an
//! actual column-index-keyed expert bank in `Literal`, threading the
//! `columns` filter selection already knows down to it, a `FORMAT_VERSION`
//! bump, and a real bpb measurement.

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
/// Panics if `position >= len`: every caller already knows the
/// transposed stream's length by construction (`transpose::encode` never
/// changes it), so an out-of-range `position` is a caller bug, not
/// adversarial input -- nothing on the decode path calls this
/// (`transpose::decode` inverts the byte layout directly, without ever
/// needing a per-position column index).
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
/// A future column-index-keyed literal expert (`research/JOURNAL.md`
/// S1-P5's remaining scope) must size its bank storage from a constant
/// alone, never from the frame's declared `columns`: a decoder reads
/// `columns` from untrusted compressed input, so sizing bank storage to
/// it directly would let a hostile frame drive unbounded allocation
/// (CLAUDE.md hard rule 2). Wrapping modulo a fixed `max_banks` is the
/// same convention `literal.rs`'s existing experts already use to keep
/// an unbounded context bounded (`ORDER2_BASE`'s `& 0xFFF`, `WORD_BASE`'s
/// `& 0xFFF`, `ALIGN_BASE`'s `position & 3`): real separation for the
/// common case this lead targets (structured data with a modest column
/// count), aliasing rather than allocating for an adversarial one.
#[must_use]
pub fn column_bank(column: usize, max_banks: NonZeroUsize) -> usize {
    column % max_banks.get()
}

#[cfg(test)]
mod tests {
    use super::{column_bank, column_of};
    use crate::test_support::nz;

    /// Ground truth independent of [`column_of`]'s closed form: replays
    /// `transpose::encode`'s own nested loop (`src/filters.rs`), recording
    /// which column produced each output position instead of copying a
    /// byte. Any divergence from `column_of` means the closed form
    /// disagrees with the filter it is meant to describe.
    fn naive_column_of_each_position(columns: usize, len: usize) -> Vec<usize> {
        let mut out = vec![0usize; len];
        let mut pos = 0usize;
        for start in 0..columns {
            let mut i = start;
            while i < len {
                out[pos] = start;
                pos += 1;
                i += columns;
            }
        }
        out
    }

    #[test]
    fn matches_naive_replay_across_many_shapes() {
        for len in [0usize, 1, 2, 3, 5, 7, 16, 17, 100, 257, 1000] {
            for columns in [1usize, 2, 3, 4, 7, 8, 16, 96, 255, 1000, 1001] {
                let naive = naive_column_of_each_position(columns, len);
                for (position, &expected) in naive.iter().enumerate() {
                    assert_eq!(
                        column_of(position, nz(columns), len),
                        expected,
                        "len={len} columns={columns} position={position}"
                    );
                }
            }
        }
    }

    #[test]
    fn single_column_is_always_column_zero() {
        for position in 0..10 {
            assert_eq!(column_of(position, nz(1), 10), 0);
        }
    }

    #[test]
    fn exact_division_gives_equal_length_columns() {
        // 8 bytes, 4 columns, no remainder: each column is exactly 2 bytes,
        // so positions 0-1 are column 0, 2-3 are column 1, and so on.
        let columns = nz(4);
        let expected = [0, 0, 1, 1, 2, 2, 3, 3];
        for (position, &column) in expected.iter().enumerate() {
            assert_eq!(column_of(position, columns, 8), column);
        }
    }

    #[test]
    fn remainder_columns_are_wider_and_come_first() {
        // 5 bytes, 2 columns: rows=2, long_columns=1, so column 0 gets 3
        // bytes and column 1 gets 2, matching filters::transpose's own
        // encode_groups_by_column test (`[a,A,b,B,c]` -> `[a,b,c,A,B]`).
        let columns = nz(2);
        let expected = [0, 0, 0, 1, 1];
        for (position, &column) in expected.iter().enumerate() {
            assert_eq!(column_of(position, columns, 5), column);
        }
    }

    #[test]
    fn more_columns_than_data_makes_every_position_its_own_column() {
        // columns wider than the data: every row has at most one element
        // (filters::transpose's own roundtrip_fewer_rows_than_columns
        // shape), so position i is column i.
        for position in 0..3 {
            assert_eq!(column_of(position, nz(8), 3), position);
        }
    }

    #[test]
    #[should_panic(expected = "position must be within the transposed stream")]
    fn position_at_len_panics() {
        let _ = column_of(5, nz(2), 5);
    }

    #[test]
    fn column_bank_is_identity_when_columns_fit_within_max_banks() {
        for column in 0..8 {
            assert_eq!(column_bank(column, nz(8)), column);
        }
    }

    #[test]
    fn column_bank_wraps_columns_beyond_max_banks() {
        // 10 columns, 4 banks: columns 4-9 alias onto banks 0-1-2-3-0-1,
        // the same wraparound `% max_banks.get()` computes directly.
        let max_banks = nz(4);
        for column in 0..10 {
            assert_eq!(column_bank(column, max_banks), column % 4);
        }
    }

    #[test]
    fn column_bank_never_exceeds_max_banks() {
        let max_banks = nz(3);
        for column in 0..100 {
            assert!(column_bank(column, max_banks) < max_banks.get());
        }
    }

    #[test]
    fn column_bank_of_a_real_column_of_result_stays_bounded() {
        // End-to-end: every position's real column (from column_of) maps
        // into a fixed 5-bank space, regardless of how many columns the
        // filter actually chose.
        let columns = nz(37);
        let len = 200;
        let max_banks = nz(5);
        for position in 0..len {
            let column = column_of(position, columns, len);
            assert!(column_bank(column, max_banks) < max_banks.get());
        }
    }
}
