use super::*;
use proptest::prelude::*;

/// Bytes biased toward `0xE8`/`0xE9`, the opcodes [`rewrite`]
/// treats specially. Uniform bytes hit either on only 2/256 draws,
/// so a sweep over pure `any::<u8>()` would mostly exercise the
/// pass-through branch and rarely the rewrite one it exists to
/// test (`test-craft`'s "generate the distribution the codec
/// targets").
fn byte_with_opcodes() -> impl Strategy<Value = u8> {
    prop_oneof![
        6 => any::<u8>(),
        2 => Just(0xE8u8),
        2 => Just(0xE9u8),
    ]
}

proptest! {
    /// `decode(encode(x)) == x` swept over data weighted toward
    /// opcode bytes (the examples above anchor the specific
    /// adjacency and overflow edge cases this sweeps).
    #[test]
    fn roundtrips(data in proptest::collection::vec(byte_with_opcodes(), 0..256)) {
        prop_assert_eq!(decode(&encode(&data)), data);
    }

    /// [`Undo`] fed one byte at a time, then [`Undo::finish`], must
    /// agree with the batch [`decode`] it shadows, on the same
    /// sweep as `roundtrips`.
    #[test]
    fn undo_matches_batch_decode(data in proptest::collection::vec(byte_with_opcodes(), 0..256)) {
        let encoded = encode(&data);
        let mut undo = Undo::try_new().unwrap();
        let mut streamed = Vec::with_capacity(data.len());
        for &byte in &encoded {
            streamed.extend_from_slice(undo.apply(byte).as_slice());
        }
        streamed.extend_from_slice(undo.finish().as_slice());
        prop_assert_eq!(&streamed, &data);
        prop_assert_eq!(streamed, decode(&encoded));
    }
}
