use super::*;
use proptest::prelude::*;

/// The structural classes `mod tests`' hand-written examples cover:
/// unstructured noise, a short pattern repeated (forces a Match/Rep
/// token), a long single-byte run (the overlapping-distance copy
/// `copy_checked`/`copy_match` must get right), and a low-period
/// pattern (drives repeated matches at one distance, exercising the
/// rep cache). Lengths span both sides of `OPTIMAL_MIN_LEN` (64), the
/// threshold below which `parse_optimal` short-circuits to
/// `parse_greedy`. Kept under a few hundred bytes: `parse_optimal`'s
/// three-round DP is heavier per case than a filter round-trip
/// (`filters.rs`'s proptests, which sweep at the crate default 256
/// cases on cheap byte-for-byte transforms), so this stays fast enough
/// for the required gate without trimming the case count.
fn data() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        proptest::collection::vec(any::<u8>(), 0..400),
        (proptest::collection::vec(any::<u8>(), 1..16), 1usize..40)
            .prop_map(|(pattern, reps)| pattern.repeat(reps)),
        (any::<u8>(), 0usize..400).prop_map(|(byte, len)| vec![byte; len]),
        (any::<u8>(), 2usize..90, 40usize..400).prop_map(|(base, period, len)| {
            (0..len)
                .map(|i| base.wrapping_add(u8::try_from(i % period).unwrap_or(0)))
                .collect()
        }),
    ]
}

proptest! {
    /// `replay(parse_optimal(data)) == data` (mirrors `roundtrip_optimal`
    /// above) and every token's length stays within `MAX_MATCH_LEN`,
    /// swept over [`data`] instead of one example per shape.
    #[test]
    fn optimal_parse_roundtrips(data in data()) {
        let tokens = parse_optimal(&data);
        prop_assert_eq!(replay(&tokens), data);
        for token in &tokens {
            if let Token::Match { len, .. } | Token::Rep { len, .. } = *token {
                prop_assert!((len as usize) <= MAX_MATCH_LEN);
            }
        }
    }

    /// Same property, `parse_greedy` (mirrors `roundtrip` above).
    #[test]
    fn greedy_parse_roundtrips(data in data()) {
        let tokens = parse_greedy(&data);
        prop_assert_eq!(replay(&tokens), data);
        for token in &tokens {
            if let Token::Match { len, .. } | Token::Rep { len, .. } = *token {
                prop_assert!((len as usize) <= MAX_MATCH_LEN);
            }
        }
    }
}
