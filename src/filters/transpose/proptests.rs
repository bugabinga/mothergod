use super::*;
use proptest::prelude::*;

proptest! {
    /// `decode(encode(x, columns), columns) == x` swept over
    /// arbitrary data and column counts, both above and below the
    /// data length (the examples above anchor the specific edge
    /// cases this sweeps).
    #[test]
    fn roundtrips(
        data in proptest::collection::vec(any::<u8>(), 0..128),
        columns in 1usize..=255,
    ) {
        let columns = NonZeroUsize::new(columns).unwrap();
        prop_assert_eq!(decode(&encode(&data, columns), columns), data);
    }
}
