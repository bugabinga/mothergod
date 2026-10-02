use super::*;
use proptest::prelude::*;

proptest! {
    /// `decode(encode(x, stride), stride) == x` swept over arbitrary
    /// data and stride, both above and below the data length (the
    /// examples above anchor the specific edge cases this sweeps).
    #[test]
    fn roundtrips(
        data in proptest::collection::vec(any::<u8>(), 0..128),
        stride in 1usize..=255,
    ) {
        let stride = NonZeroUsize::new(stride).unwrap();
        prop_assert_eq!(decode(&encode(&data, stride), stride), data);
    }

    /// [`Undo`] undone one byte at a time must agree with the batch
    /// [`decode`] it shadows, on the same sweep as `roundtrips`.
    #[test]
    fn undo_matches_batch_decode(
        data in proptest::collection::vec(any::<u8>(), 0..128),
        stride in 1usize..=255,
    ) {
        let stride = NonZeroUsize::new(stride).unwrap();
        let encoded = encode(&data, stride);
        let mut undo = Undo::try_new(stride).unwrap();
        let streamed: Vec<u8> = encoded.iter().map(|&byte| undo.apply(byte)).collect();
        prop_assert_eq!(&streamed, &data);
        prop_assert_eq!(streamed, decode(&encoded, stride));
    }
}
