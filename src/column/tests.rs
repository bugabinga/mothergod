use super::{bank_of, column_of};
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
fn bank_is_column_when_columns_fit_within_max_banks() {
    let (columns, len, max_banks) = (nz(8), 200, nz(8));
    for position in 0..len {
        assert_eq!(
            bank_of(position, columns, len, max_banks),
            column_of(position, columns, len)
        );
    }
}

#[test]
fn bank_wraps_columns_beyond_max_banks() {
    // 37 columns, 5 banks: every position's bank is its column mod 5,
    // however many columns the filter actually chose.
    let (columns, len, max_banks) = (nz(37), 200, nz(5));
    for position in 0..len {
        let bank = bank_of(position, columns, len, max_banks);
        assert_eq!(bank, column_of(position, columns, len) % 5);
        assert!(bank < max_banks.get());
    }
}
