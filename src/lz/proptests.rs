use super::*;
use proptest::prelude::*;

/// The structural classes `mod tests`' hand-written examples cover:
/// unstructured noise, a short pattern repeated (forces a Match/Rep
/// token), a long single-byte run (the overlapping-distance copy
/// `copy_checked` must get right), and a low-period
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

/// The copy semantics both chunked copies must keep, one byte at a time:
/// `len` pushes of the byte `distance` before the end, each pushed byte
/// readable by the next.
fn copy_by_byte(history: &mut Vec<u8>, distance: usize, len: usize) {
    let start = history.len() - distance;
    for k in 0..len {
        history.push(history[start + k]);
    }
}

/// One step of a streaming decode: literal bytes, then a copy whose
/// distance the index picks among the legal ones.
fn window_steps() -> impl Strategy<Value = Vec<(Vec<u8>, prop::sample::Index, usize)>> {
    proptest::collection::vec(
        (
            proptest::collection::vec(any::<u8>(), 0..8),
            any::<prop::sample::Index>(),
            0usize..150,
        ),
        1..40,
    )
}

proptest! {
    /// `copy_checked` appends what a byte-at-a-time copy appends, over
    /// distances from 1 (a run) to the whole prefix and lengths past
    /// several periods, where its chunks double.
    #[test]
    fn copy_checked_matches_a_byte_at_a_time_copy(
        prefix in proptest::collection::vec(any::<u8>(), 1..200),
        distance in any::<prop::sample::Index>(),
        len in 0u32..600,
    ) {
        let distance = distance.index(prefix.len()) + 1;
        let mut expected = prefix.clone();
        copy_by_byte(&mut expected, distance, len as usize);
        let mut output = prefix;
        let distance_nz = NonZeroU32::new(u32::try_from(distance).unwrap()).unwrap();
        copy_checked(&mut output, len, distance_nz).unwrap();
        prop_assert_eq!(output, expected);
    }

    /// `Window::copy_back` emits, in order, what a byte-at-a-time copy
    /// over the unbounded history appends, across rings small enough that
    /// most copies wrap, and leaves the ring holding the history's tail.
    #[test]
    fn window_copy_back_matches_a_byte_at_a_time_copy(
        capacity in 1usize..48,
        steps in window_steps(),
    ) {
        let mut window = Window::try_new(Reach::Standard, capacity).unwrap();
        let mut history = Vec::new();
        for (literals, distance, len) in steps {
            for byte in literals {
                window.push(byte);
                history.push(byte);
            }
            if history.is_empty() {
                continue;
            }
            let distance = distance.index(history.len().min(capacity)) + 1;
            let before = history.len();
            copy_by_byte(&mut history, distance, len);
            let mut emitted = Vec::new();
            window.copy_back(distance, len, |run| {
                emitted.extend_from_slice(run);
                Ok::<(), Error>(())
            }).unwrap();
            prop_assert_eq!(&emitted[..], &history[before..]);
            prop_assert_eq!(window.written_len(), history.len());
        }
        let held = history.len().min(capacity);
        if held > 0 {
            let mut tail = Vec::new();
            window.copy_back(held, held, |run| {
                tail.extend_from_slice(run);
                Ok::<(), Error>(())
            }).unwrap();
            prop_assert_eq!(&tail[..], &history[history.len() - held..]);
        }
    }
}
