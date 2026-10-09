use super::*;
use proptest::prelude::*;

/// Pure noise (drives `compress` to `Method::Stored`, the incompressible
/// control) mixed with a short pattern repeated many times (drives it to
/// `Method::Lz`), so the sweep exercises both frame shapes rather than
/// only whichever one uniform random bytes happen to produce
/// (`test-craft`'s "generate the distribution the codec targets").
fn data() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        proptest::collection::vec(any::<u8>(), 0..512),
        (proptest::collection::vec(any::<u8>(), 1..32), 1usize..16)
            .prop_map(|(pattern, reps)| pattern.repeat(reps)),
    ]
}

proptest! {
    // `compress` runs the full optimal-parse LZ encoder, far heavier
    // per case than the filter round-trips this property's siblings in
    // `filters.rs` sweep at proptest's default 256 cases; a quarter of
    // that keeps this property's slice of the required gate under 10s
    // (measured locally) while still sweeping both frame shapes above.
    // `ProptestConfig::with_cases` would pin the literal regardless of
    // `PROPTEST_CASES`, which a local heavier run sets (no workflow
    // does, so every CI leg runs `ProptestConfig::default()`'s case
    // count for the `filters.rs` siblings and 64 here). Read the env var
    // ourselves instead so the fast-profile default stays 64 but a
    // local override wins.
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(64),
        ..ProptestConfig::default()
    })]

    /// `decompress`, `decompress_bounded` (at `codec::MAX_DECODED_LEN`),
    /// and `decompress_to_writer` must agree on every frame `compress`
    /// can produce (#452). `decompress` is `decompress_bounded` at that
    /// same bound, so the only genuine differential leg here is
    /// `decompress_to_writer`; `decompress_bounded`'s own distinguishing
    /// behavior, a tighter caller-supplied `max_len`, is covered by
    /// `decompress_bounded_rejects_a_stored_frame_over_its_own_tighter_bound`
    /// and its `Lz` sibling, not by this property.
    #[test]
    fn decode_apis_agree(input in data()) {
        let frame = compress(&input);
        let via_decompress = decompress(&frame).unwrap();
        prop_assert_eq!(&via_decompress, &input);
        prop_assert_eq!(
            decompress_bounded(&frame, codec::MAX_DECODED_LEN),
            Ok(via_decompress.clone())
        );
        let mut out = Vec::new();
        decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out).unwrap();
        prop_assert_eq!(out, via_decompress);
    }
}
