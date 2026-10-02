use super::*;
use crate::test_support::nz;

#[test]
fn roundtrip_empty() {
    let c = nz(1);
    assert_eq!(decode(&encode(&[], c), c), Vec::<u8>::new());
}

#[test]
fn roundtrip_single_byte() {
    let c = nz(1);
    assert_eq!(decode(&encode(&[42], c), c), vec![42]);
}

#[test]
fn roundtrip_fewer_rows_than_columns() {
    // columns wider than the data: every row has one element, so
    // both directions are the identity.
    let data = vec![1, 2, 3];
    let c = nz(8);
    assert_eq!(encode(&data, c), data);
    assert_eq!(decode(&data, c), data);
}

#[test]
fn roundtrip_various_column_counts() {
    let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    for columns in [1usize, 2, 3, 4, 8, 16, 96, 255, 1000, 1001] {
        let c = nz(columns);
        let encoded = encode(&data, c);
        assert_eq!(decode(&encoded, c), data, "columns {columns}");
    }
}

#[test]
fn encode_groups_by_column() {
    // 2 columns, 3 rows (last row short): [a0 a1 | b0 b1 | c0] ->
    // column 0 then column 1.
    let data = vec![b'a', b'A', b'b', b'B', b'c'];
    let c = nz(2);
    assert_eq!(encode(&data, c), vec![b'a', b'b', b'c', b'A', b'B']);
}

#[test]
fn single_column_is_identity() {
    let data = vec![10, 20, 30, 40, 50];
    let c = nz(1);
    assert_eq!(encode(&data, c), data);
    assert_eq!(decode(&data, c), data);
}
