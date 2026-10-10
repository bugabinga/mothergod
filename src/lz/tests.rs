use super::*;

fn roundtrip(data: &[u8]) {
    let tokens = parse_greedy(data);
    assert_eq!(replay(&tokens), data, "roundtrip mismatch");
    for token in &tokens {
        if let Token::Match { len, .. } | Token::Rep { len, .. } = *token {
            assert!(
                (len as usize) <= MAX_MATCH_LEN,
                "token length {len} exceeds MAX_MATCH_LEN"
            );
        }
    }
}

#[test]
fn roundtrip_empty() {
    roundtrip(b"");
}

#[test]
fn roundtrip_single_byte() {
    roundtrip(b"x");
}

#[test]
fn roundtrip_all_literals_no_repeats() {
    roundtrip(b"the quick brown fox jumps over a lazy dog");
}

#[test]
fn roundtrip_simple_repeat_produces_a_match() {
    let data = b"abcdefgh".repeat(5);
    let tokens = parse_greedy(&data);
    assert_eq!(replay(&tokens), data);
    assert!(
        tokens
            .iter()
            .any(|t| matches!(t, Token::Match { .. } | Token::Rep { .. })),
        "a 5x repeat of an 8-byte pattern should produce at least one match or rep token"
    );
}

#[test]
fn roundtrip_run_length_exercises_overlapping_distance() {
    // distance (1) shorter than the eventual match length: copy_match
    // must read bytes it just wrote, not a disjoint source region.
    roundtrip(&vec![b'a'; 1000]);
}

#[test]
fn roundtrip_long_run_spans_multiple_tokens() {
    // Longer than MAX_MATCH_LEN: parse_greedy must split it into
    // several Match/Rep tokens rather than one oversized one.
    roundtrip(&vec![b'z'; 200_000]);
}

#[test]
fn roundtrip_alternating_pattern_exercises_rep_cache() {
    // "AB" x N then "CD" x N: after the first real match sets up a
    // repeat offset, later repeats of the same distance should reuse
    // the rep cache rather than re-encoding a fresh distance.
    let mut data = b"AB".repeat(50);
    data.extend(b"CD".repeat(50));
    let tokens = parse_greedy(&data);
    assert_eq!(replay(&tokens), data);
    assert!(tokens.iter().any(|t| matches!(t, Token::Rep { .. })));
}

#[test]
fn roundtrip_cyclic_data() {
    let data: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
    roundtrip(&data);
}

#[test]
fn roundtrip_structured_repeats_at_varying_distances() {
    // Two near-duplicate copies of a 26-byte block separated by
    // unrelated bytes, exercising matches at distances that are not
    // the rep cache's initial [1, 4, 8] and the one-step
    // lazy-matching check along the way.
    let base: Vec<u8> = (b'a'..=b'z').collect();
    let mut data = base.clone();
    data.push(b'-');
    data.extend_from_slice(&base[1..]);
    data.push(b'+');
    data.extend_from_slice(&base);
    roundtrip(&data);
}

#[test]
fn roundtrip_binary_data_with_zero_bytes() {
    let data: Vec<u8> = (0..1000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    roundtrip(&data);
}

fn roundtrip_optimal(data: &[u8]) {
    let tokens = parse_optimal(data);
    assert_eq!(replay(&tokens), data, "optimal-parse roundtrip mismatch");
    for token in &tokens {
        if let Token::Match { len, .. } | Token::Rep { len, .. } = *token {
            assert!(
                (len as usize) <= MAX_MATCH_LEN,
                "token length {len} exceeds MAX_MATCH_LEN"
            );
        }
    }
}

#[test]
fn optimal_roundtrip_empty() {
    roundtrip_optimal(b"");
}

#[test]
fn optimal_roundtrip_single_byte() {
    roundtrip_optimal(b"x");
}

#[test]
fn optimal_below_min_len_matches_greedy() {
    // Below OPTIMAL_MIN_LEN, parse_optimal short-circuits straight to
    // parse_greedy: same input, same tokens.
    let data = b"the quick brown fox";
    assert!(data.len() < OPTIMAL_MIN_LEN);
    assert_eq!(parse_optimal(data), parse_greedy(data));
}

#[test]
fn optimal_roundtrip_all_literals_no_repeats() {
    let data = b"the quick brown fox jumps over a lazy dog, then jumps back again";
    assert!(data.len() >= OPTIMAL_MIN_LEN);
    roundtrip_optimal(data);
}

#[test]
fn optimal_roundtrip_simple_repeat_produces_a_match_or_rep() {
    let data = b"abcdefgh".repeat(10);
    let tokens = parse_optimal(&data);
    assert_eq!(replay(&tokens), data);
    assert!(
        tokens
            .iter()
            .any(|t| matches!(t, Token::Match { .. } | Token::Rep { .. })),
        "a 10x repeat of an 8-byte pattern should produce at least one match or rep token"
    );
}

#[test]
fn optimal_roundtrip_run_length_exercises_overlapping_distance() {
    // distance 1, shorter than the match length: copy_match must read
    // bytes it just wrote.
    roundtrip_optimal(&vec![b'a'; 1000]);
}

#[test]
fn optimal_roundtrip_long_run_of_one_repeated_byte_stays_linear() {
    // Regression test for issue #179. dp_round visits every position
    // (needed to consider every possible token start, unlike
    // parse_greedy, which jumps ahead by a whole match's length), and
    // used to price every rep-cache slot at each one via a fresh
    // match_len scan with no carry-reuse equivalent to
    // next_match_candidate's: on a single-byte run the scan cost stayed
    // near MAX_MATCH_LEN at every position, making total cost quadratic
    // in the run length once it passed MAX_MATCH_LEN. 200,000 bytes
    // matches the issue's own repro (mirroring parse_greedy's own
    // 200,000-byte same-byte-run test); before the fix this took over
    // 60 seconds and was killed. The bound below is generous (the fixed
    // version measures under 2s in an unoptimized debug build on
    // ordinary hardware) so a slower CI runner doesn't flake, while
    // still failing well before a regression to the old quadratic cost
    // would let it run to completion.
    let data = vec![b'z'; 200_000];
    let start = std::time::Instant::now();
    roundtrip_optimal(&data);
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(15),
        "parse_optimal on a 200,000-byte single-byte run took {elapsed:?}, expected well \
         under 15s; likely a regression to issue #179's quadratic rep-candidate scan"
    );
}

#[test]
fn optimal_roundtrip_alternating_pattern_exercises_rep_cache() {
    let mut data = b"AB".repeat(50);
    data.extend(b"CD".repeat(50));
    let tokens = parse_optimal(&data);
    assert_eq!(replay(&tokens), data);
    assert!(tokens.iter().any(|t| matches!(t, Token::Rep { .. })));
}

#[test]
fn optimal_roundtrip_cyclic_data() {
    let data: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
    roundtrip_optimal(&data);
}

#[test]
fn optimal_roundtrip_structured_repeats_at_varying_distances() {
    let base: Vec<u8> = (b'a'..=b'z').collect();
    let mut data = base.clone();
    data.push(b'-');
    data.extend_from_slice(&base[1..]);
    data.push(b'+');
    data.extend_from_slice(&base);
    data.push(b'~');
    data.extend_from_slice(&base);
    assert!(data.len() >= OPTIMAL_MIN_LEN);
    roundtrip_optimal(&data);
}

#[test]
fn optimal_roundtrip_binary_data_with_zero_bytes() {
    let data: Vec<u8> = (0..1000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    roundtrip_optimal(&data);
}

#[test]
fn optimal_roundtrip_short_close_repeats() {
    // Dense 3-byte repeats at a distance under SHORT_MATCH_MAX_DISTANCE:
    // exercises dp_round's length-3 short-match candidate.
    let data = b"xyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyz".to_vec();
    assert!(data.len() >= OPTIMAL_MIN_LEN);
    roundtrip_optimal(&data);
}

#[test]
fn optimal_with_window_reaches_a_repeat_a_smaller_window_would_miss() {
    // research/JOURNAL.md S1-P4: parse_optimal_with_window must thread
    // its window parameter through all three dp_round rounds, not just
    // the first, and produce a lossless parse either way. Template
    // (low-range bytes) and filler (high-range pseudo-random bytes)
    // stay in disjoint byte ranges so no accidental cross-half match
    // confounds the planted repeat's distance.
    let template: Vec<u8> = (0u8..80).collect();
    let filler: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED)
        .take(420)
        .map(|s| 128 + u8::try_from(s % 128).unwrap())
        .collect();
    let mut data = template.clone();
    data.extend_from_slice(&filler);
    data.extend_from_slice(&template);
    let distance = filler.len() + template.len();

    let small_window = 300;
    let large_window = 700;
    assert!(small_window < distance && distance <= large_window);

    let small = parse_optimal_with_window(&data, small_window);
    assert_eq!(replay(&small), data);
    assert!(
        small.iter().all(|t| !matches!(
            t,
            Token::Match { distance: d, .. } if d.get() as usize > small_window
        )),
        "a window of {small_window} must never produce a fresh match past it"
    );

    let large = parse_optimal_with_window(&data, large_window);
    assert_eq!(replay(&large), data);
    assert!(
        large.iter().any(|t| matches!(
            t,
            Token::Match { distance: d, .. } if d.get() as usize == distance
        )),
        "a window of {large_window} must find the planted repeat at distance {distance}, \
         unreachable through {small_window}"
    );
}

#[test]
fn greedy_with_window_reaches_a_repeat_a_smaller_window_would_miss() {
    // research/JOURNAL.md S1-P4's remaining scope named parse_greedy's
    // own hash-chain finder as "still hardcoded to WINDOW, unexamined";
    // same shape as optimal_with_window_reaches_a_repeat_a_smaller_
    // window_would_miss above, proving the parameter actually gates
    // parse_greedy_with_window's reach end to end, losslessly either way.
    let template: Vec<u8> = (0u8..80).collect();
    let filler: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED)
        .take(420)
        .map(|s| 128 + u8::try_from(s % 128).unwrap())
        .collect();
    let mut data = template.clone();
    data.extend_from_slice(&filler);
    data.extend_from_slice(&template);
    let distance = filler.len() + template.len();

    let small_window = 300;
    let large_window = 700;
    assert!(small_window < distance && distance <= large_window);

    let small = parse_greedy_with_window(&data, small_window);
    assert_eq!(replay(&small), data);
    assert!(
        small.iter().all(|t| !matches!(
            t,
            Token::Match { distance: d, .. } if d.get() as usize > small_window
        )),
        "a window of {small_window} must never produce a fresh match past it"
    );

    let large = parse_greedy_with_window(&data, large_window);
    assert_eq!(replay(&large), data);
    assert!(
        large.iter().any(|t| matches!(
            t,
            Token::Match { distance: d, .. } if d.get() as usize == distance
        )),
        "a window of {large_window} must find the planted repeat at distance {distance}, \
         unreachable through {small_window}"
    );
}

#[test]
fn optimal_roundtrip_random_like_binary_never_worse_than_stored() {
    // A pseudo-random byte stream (no real structure): the DP must
    // still round-trip even when literals dominate.
    let data: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
        .take(500)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();
    roundtrip_optimal(&data);
}

/// Independent reference for [`binary_tree_matches_brute_force`]:
/// scans every earlier position sharing `i`'s hash bucket directly
/// instead of walking a tree. Restricted to the same bucket because
/// that's the population [`BinaryTreeMatchFinder`] (and
/// [`MatchFinder`]) can ever see in the first place — a match whose
/// candidate has a different 3-byte prefix hash than `i`'s is
/// invisible to either, by construction, same as a length below 3
/// bytes never entering either structure's hash table.
fn brute_force_best(data: &[u8], i: usize) -> Option<(usize, Distance)> {
    let target_hash = prefix_hash(data, i);
    let mut best_len = 0usize;
    let mut best_distance = None;
    for cur in 0..i {
        if prefix_hash(data, cur) != target_hash {
            continue;
        }
        let len = suffix_common_len(data, cur, i, 0, MAX_MATCH_LEN);
        if len > best_len {
            best_len = len;
            best_distance = NonZeroU32::new(to_u32(i - cur));
        }
    }
    best_distance.map(|distance| (best_len, distance))
}

#[test]
fn binary_tree_no_prior_positions_returns_none() {
    let data = b"abcdef";
    let mut finder = BinaryTreeMatchFinder::new(data, WINDOW);
    assert_eq!(finder.insert_and_find(0, data.len(), MAX_MATCH_LEN), None);
}

#[test]
fn binary_tree_finds_an_exact_repeat() {
    let data = b"abcabcabc";
    let mut finder = BinaryTreeMatchFinder::new(data, WINDOW);
    let mut found_at_6 = None;
    for i in 0..data.len() {
        let found = finder.insert_and_find(i, data.len(), MAX_MATCH_LEN);
        if let Some((len, distance)) = found {
            assert_eq!(
                match_len(data, i, distance),
                len,
                "position {i}: reported length must be a real match at the reported distance"
            );
        }
        if i == 6 {
            found_at_6 = found;
        }
    }
    let (len, distance) = found_at_6.expect("position 6 repeats position 3's \"abc\"");
    assert_eq!(distance.get(), 3);
    assert_eq!(len, 3);
}

#[test]
fn binary_tree_matches_brute_force() {
    // A small byte alphabet keeps 3-byte prefixes colliding often, so
    // hash buckets build up real tree structure to walk, unlike a
    // near-uniform 0..256 stream where most buckets stay tiny.
    let data: Vec<u8> = crate::test_support::Xorshift32::new(0xB17E_5EED)
        .take(400)
        .map(|state| u8::try_from(state % 5).unwrap())
        .collect();
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    for i in 0..data.len() {
        // max_depth covers every possible candidate in the bucket
        // (at most i of them), so the walk cannot be truncated before
        // it would reach the true best.
        let found = finder.insert_and_find(i, data.len(), MAX_MATCH_LEN);
        let expected_len = brute_force_best(&data, i).map(|(len, _)| len);
        assert_eq!(
            found.map(|(len, _)| len),
            expected_len,
            "position {i}: best length must equal a brute-force scan when max_depth covers every candidate"
        );
        if let Some((len, distance)) = found {
            assert_eq!(
                match_len(&data, i, distance),
                len,
                "position {i}: reported length must be a real match at the reported distance"
            );
        }
    }
}

#[test]
fn binary_tree_zero_max_depth_finds_nothing_but_stays_consistent() {
    let data = b"abcabcabc";
    let mut finder = BinaryTreeMatchFinder::new(data, WINDOW);
    let _ = finder.insert_and_find(0, data.len(), MAX_MATCH_LEN);
    let _ = finder.insert_and_find(1, data.len(), MAX_MATCH_LEN);
    let _ = finder.insert_and_find(2, data.len(), MAX_MATCH_LEN);
    // Position 3 shares position 0's hash bucket ("abc"); max_depth 0
    // must not panic, and correctly reports no match even though a
    // real one exists.
    assert_eq!(finder.insert_and_find(3, 0, MAX_MATCH_LEN), None);
    // A later insert with a different hash bucket ("bca", shared with
    // position 1) is unaffected and still finds its match.
    assert!(
        finder
            .insert_and_find(4, data.len(), MAX_MATCH_LEN)
            .is_some()
    );
}

#[test]
fn binary_tree_caps_match_length_at_max_match_len() {
    let mut data = vec![b'x'; MAX_MATCH_LEN + 50];
    data.push(b'y');
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let _ = finder.insert_and_find(0, 8, MAX_MATCH_LEN);
    let (len, _distance) = finder
        .insert_and_find(1, 8, MAX_MATCH_LEN)
        .expect("position 1 repeats position 0's run of 'x'");
    assert!(len <= MAX_MATCH_LEN);
}

#[test]
fn binary_tree_nice_len_bounds_candidates_on_deep_correct_chains() {
    // The 300 near-duplicate 200-byte blocks from
    // `binary_tree_near_duplicate_blocks_benefit_from_prefix_reuse`:
    // unlike a single repeated byte (where BST pruning already visits
    // only one candidate per insert, `research/JOURNAL.md` S2-A44), a
    // per-block varying byte gives every insert a genuinely deep,
    // correctly-pruned chain of ever-closer candidates to walk past --
    // each one an ~100-byte match, cheap enough per comparison that
    // candidate *count*, not per-candidate cost, dominates here. `nice_len`
    // 50 (below every block's true match length) stops each walk after
    // its first candidate instead of the up to `data.len()` candidates
    // `max_depth` alone would allow. Measured by hand: ~69ms with
    // `nice_len` 50 at this same (unbounded) `max_depth`, vs. ~228ms
    // with `nice_len` set to `MAX_MATCH_LEN` (i.e. no early exit) — a
    // real, if modest, ~3.3x on data shaped like this. The bound below
    // leaves generous headroom for slower CI hardware.
    let template: Vec<u8> = (0..200u32)
        .map(|i| u8::try_from(i % 251).expect("i % 251 fits u8"))
        .collect();
    let mut data = Vec::new();
    for copy in 0..300u16 {
        let mut block = template.clone();
        block[100] = u8::try_from(copy % 256).expect("copy % 256 fits u8");
        data.extend_from_slice(&block);
    }
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let start = std::time::Instant::now();
    for i in 0..data.len() {
        let _ = finder.insert_and_find(i, data.len(), 50);
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "300 near-duplicate 200-byte blocks with nice_len 50 and unbounded max_depth took \
         {elapsed:?}, expected well under 3s; likely a regression to nice_len no longer \
         bounding candidates visited"
    );
}

#[test]
fn binary_tree_nice_len_caps_reported_length_when_true_match_is_longer() {
    // nice_len now bounds suffix_common_len's own scan (S2-A46), not
    // just how many candidates get visited: a true match longer than
    // nice_len is reported as exactly nice_len, the "good enough, stop
    // paying to confirm more" trade the struct docs describe.
    let mut data = vec![b'x'; 300];
    data.push(b'y');
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let _ = finder.insert_and_find(0, data.len(), MAX_MATCH_LEN);
    let (len, distance) = finder
        .insert_and_find(1, data.len(), 50)
        .expect("position 1 repeats position 0's run of 'x'");
    assert_eq!(distance.get(), 1);
    assert_eq!(
        len, 50,
        "nice_len must cap the reported length itself, not just stop visiting more candidates"
    );
}

#[test]
fn binary_tree_nice_len_bounds_per_candidate_scan_cost_on_repeated_byte_run() {
    // The issue #179 shape S2-A44 measured and could not fix: a low
    // nice_len there still let the first candidate's suffix_common_len
    // scan run to a full MAX_MATCH_LEN before nice_len was ever
    // consulted between candidates (32.9s on this exact fixture).
    // S2-A46 additionally caps each candidate's own scan at nice_len,
    // so the first candidate here now costs O(nice_len) instead of
    // O(MAX_MATCH_LEN), and the walk stops immediately after (its
    // capped common length already reaches nice_len).
    let data = vec![b'z'; 200_000];
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let start = std::time::Instant::now();
    for i in 0..data.len() {
        let _ = finder.insert_and_find(i, MAX_TREE_DEPTH_OPTIMAL, NICE_LEN_OPTIMAL);
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "200,000 bytes of one repeated value with nice_len 128 took {elapsed:?}, expected \
         well under 5s; likely a regression to nice_len no longer bounding per-candidate \
         scan cost (research/JOURNAL.md S2-A46)"
    );
}

#[test]
fn binary_tree_near_duplicate_blocks_benefit_from_prefix_reuse() {
    // 300 copies of a 200-byte template, each differing in exactly one
    // byte (index 100): every block-start position shares the same
    // 3-byte prefix hash and a 100-byte common prefix with every other
    // block, then a per-block-varying byte that gives the tree real
    // left/right branching (unlike a single repeated byte, where every
    // candidate ties and lands on the same side, see
    // `research/JOURNAL.md` S2-A43 -- length-prefix reuse cannot help
    // there, since the untouched side's bound never leaves 0). This
    // shape approximates S1-P2's named target (sqlite/json/jsonl-like
    // near-duplicate records), not a pathological single-byte run.
    // Measured by hand before this guard existed: unoptimized (`start`
    // forced to 0) took ~970ms here, this optimization ~280ms, a real
    // ~3.5x. The bound below leaves generous headroom for slower CI
    // hardware while still catching a regression back to the
    // unoptimized cost.
    let template: Vec<u8> = (0..200u32)
        .map(|i| u8::try_from(i % 251).expect("i % 251 fits u8"))
        .collect();
    let mut data = Vec::new();
    for copy in 0..300u16 {
        let mut block = template.clone();
        block[100] = u8::try_from(copy % 256).expect("copy % 256 fits u8");
        data.extend_from_slice(&block);
    }
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let start = std::time::Instant::now();
    for i in 0..data.len() {
        let _ = finder.insert_and_find(i, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "300 near-duplicate 200-byte blocks took {elapsed:?} to insert, expected well \
         under 3s; likely a regression to length-prefix reuse always starting from 0"
    );
}

#[test]
fn binary_tree_finds_match_exactly_at_window_boundary_but_not_past_it() {
    let mut at_boundary = vec![0xAAu8; WINDOW + 3];
    at_boundary[0..3].copy_from_slice(b"xyz");
    at_boundary[WINDOW..WINDOW + 3].copy_from_slice(b"xyz");
    let mut finder = BinaryTreeMatchFinder::new(&at_boundary, WINDOW);
    let _ = finder.insert_and_find(0, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);
    let (_, distance) = finder
        .insert_and_find(WINDOW, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN)
        .expect("distance == WINDOW is still in range");
    assert_eq!(distance.get() as usize, WINDOW);

    let mut past_boundary = vec![0xAAu8; WINDOW + 4];
    past_boundary[0..3].copy_from_slice(b"xyz");
    past_boundary[WINDOW + 1..WINDOW + 4].copy_from_slice(b"xyz");
    let mut finder = BinaryTreeMatchFinder::new(&past_boundary, WINDOW);
    let _ = finder.insert_and_find(0, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);
    assert_eq!(
        finder.insert_and_find(WINDOW + 1, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN),
        None,
        "distance == WINDOW + 1 must never be reported"
    );
}

#[test]
fn binary_tree_larger_window_finds_matches_the_default_window_would_miss() {
    // S1-P4's first slice: window is now a per-instance parameter,
    // not just the wired WINDOW constant. A distance past WINDOW but
    // within a caller-chosen larger window must be found when the
    // finder is configured with that larger window, and still
    // rejected when it is not -- proving the parameter actually
    // gates reach rather than being plumbed through unused.
    let larger_window = WINDOW * 2;
    let mut data = vec![0xAAu8; larger_window + 3];
    data[0..3].copy_from_slice(b"xyz");
    data[larger_window..larger_window + 3].copy_from_slice(b"xyz");

    let mut wide_finder = BinaryTreeMatchFinder::new(&data, larger_window);
    let _ = wide_finder.insert_and_find(0, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);
    let (_, distance) = wide_finder
        .insert_and_find(larger_window, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN)
        .expect("distance == larger_window is in range for a finder configured with it");
    assert_eq!(distance.get() as usize, larger_window);

    let mut default_finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let _ = default_finder.insert_and_find(0, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);
    assert_eq!(
        default_finder.insert_and_find(larger_window, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN),
        None,
        "distance == larger_window must stay unreachable through the default-WINDOW finder \
         the wired parse actually uses"
    );
}

#[test]
fn binary_tree_evicts_stale_positions_from_the_tree_structure() {
    // Same shape as the boundary test above, but checked structurally:
    // a match past WINDOW was already unreachable through the public
    // API before this fix (filtered at report time). What's new is
    // that the stale node is gone from the tree itself, not just
    // skipped when reporting -- proven by walking every position
    // reachable from the bucket's root after the second insert.
    let mut data = vec![0xAAu8; WINDOW + 4];
    data[0..3].copy_from_slice(b"xyz");
    data[WINDOW + 1..WINDOW + 4].copy_from_slice(b"xyz");
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let _ = finder.insert_and_find(0, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);
    let _ = finder.insert_and_find(WINDOW + 1, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);

    let h = prefix_hash(&data, 0);
    let mut stack = vec![finder.head[h]];
    let mut reachable = Vec::new();
    while let Some(cur) = stack.pop() {
        if cur == NO_POSITION {
            continue;
        }
        let p = cur as usize;
        reachable.push(p);
        stack.push(finder.left[p]);
        stack.push(finder.right[p]);
    }
    assert_eq!(
        reachable,
        vec![WINDOW + 1],
        "position 0 must be evicted from the tree once it falls past WINDOW, not just \
         excluded from the reported match"
    );
}

#[test]
fn price_counts_observe_bumps_literal_by_prev_byte_context() {
    let mut counts = PriceCounts::new();
    counts.observe(Token::Literal(b'x'), Some(0x35));
    assert_eq!(counts.literal[3 * 256 + usize::from(b'x')], 2);
    assert_eq!(counts.length, vec![1; LENGTH_BUCKETS]);
    assert_eq!(counts.offset, vec![1; OFFSET_BUCKETS]);
    assert_eq!(counts.rep, 1);
}

#[test]
fn price_counts_observe_literal_at_stream_start_uses_context_zero() {
    let mut counts = PriceCounts::new();
    counts.observe(Token::Literal(b'z'), None);
    assert_eq!(counts.literal[usize::from(b'z')], 2);
}

#[test]
fn price_counts_observe_match_bumps_length_and_offset_only() {
    let mut counts = PriceCounts::new();
    let distance = NonZeroU32::new(100).unwrap();
    counts.observe(Token::Match { len: 10, distance }, Some(0));
    assert_eq!(counts.length[bucket(10)], 2);
    assert_eq!(counts.offset[bucket(100)], 2);
    assert_eq!(counts.rep, 1);
}

#[test]
fn price_counts_observe_rep_bumps_length_and_rep_only() {
    let mut counts = PriceCounts::new();
    counts.observe(
        Token::Rep {
            len: 6,
            slot: RepSlot::First,
        },
        Some(0),
    );
    assert_eq!(counts.length[bucket(6)], 2);
    assert_eq!(counts.rep, 2);
    assert_eq!(counts.offset, vec![1; OFFSET_BUCKETS]);
}

#[test]
fn price_counts_observe_accumulates_across_calls() {
    let mut counts = PriceCounts::new();
    counts.observe(Token::Literal(b'a'), None);
    counts.observe(Token::Literal(b'a'), None);
    assert_eq!(counts.literal[usize::from(b'a')], 3);
}

#[test]
fn price_counts_prices_computes_shannon_price_per_literal_context() {
    let mut counts = PriceCounts::new();
    counts.observe(Token::Literal(b'a'), None);
    counts.observe(Token::Literal(b'a'), None);
    counts.observe(Token::Literal(b'b'), Some(b'a'));
    let table = counts.prices(3);

    assert_eq!(table.literal[usize::from(b'a')], price(3, 258));
    assert_eq!(table.literal[0], price(1, 258));

    let context6 = 6 * 256;
    assert_eq!(table.literal[context6 + usize::from(b'b')], price(2, 257));
    assert_eq!(table.literal[context6], price(1, 257));
}

#[test]
fn price_counts_observe_matches_tally_for_the_same_sequence() {
    let data = b"abracadabra";
    let tokens = parse_greedy(data);
    let via_tally = PriceCounts::tally(&tokens, data);

    let mut via_observe = PriceCounts::new();
    let mut pos = 0usize;
    for token in &tokens {
        let prev_byte = if pos > 0 { Some(data[pos - 1]) } else { None };
        via_observe.observe(*token, prev_byte);
        pos += match *token {
            Token::Literal(_) => 1,
            Token::Match { len, .. } | Token::Rep { len, .. } => len as usize,
        };
    }

    assert_eq!(via_tally.literal, via_observe.literal);
    assert_eq!(via_tally.length, via_observe.length);
    assert_eq!(via_tally.offset, via_observe.offset);
    assert_eq!(via_tally.rep, via_observe.rep);
}

/// Finds an [`DETECTOR_ANCHOR_LEN`]-byte value whose own window
/// satisfies [`is_content_defined_anchor`] at position 0, so tests can
/// plant a repeat that the content-defined detector is actually able to
/// select (an arbitrary literal like `b"anchor42"` has no reason to
/// satisfy a hash condition). Also excludes the all-zero value so it
/// stays distinguishable from the zero-filled padding
/// `planted_repeat_content_defined` uses elsewhere.
fn find_content_defined_anchor_value() -> [u8; DETECTOR_ANCHOR_LEN] {
    for n in 1u64.. {
        let candidate = n.to_le_bytes();
        if is_content_defined_anchor(&candidate, 0) {
            return candidate;
        }
    }
    unreachable!("density ~1/DETECTOR_STRIDE guarantees a hit well before u64 exhausts")
}

/// Finds a single filler byte whose repeated [`DETECTOR_ANCHOR_LEN`]-byte
/// window is never itself a content-defined anchor, so padding built
/// from it contributes zero entries to
/// [`likely_benefits_from_larger_window_content_defined_confirmed`]'s
/// `last_seen` map regardless of how often it repeats — isolating a
/// planted anchor as the only detectable recurrence.
fn find_non_anchor_filler_byte() -> u8 {
    for byte in 0u8..=255 {
        if !is_content_defined_anchor(&[byte; DETECTOR_ANCHOR_LEN], 0) {
            return byte;
        }
    }
    unreachable!("density ~1/DETECTOR_STRIDE guarantees most byte values are non-anchors")
}

/// Builds `far_offset + DETECTOR_ANCHOR_LEN` bytes of a single
/// non-anchor filler byte ([`find_non_anchor_filler_byte`], so no
/// unrelated recurrence in the padding can produce a spurious
/// detection), then stamps a content-defined anchor value
/// ([`find_content_defined_anchor_value`]) at position 0 and again at
/// `far_offset`. `far_offset` is not required to be any particular
/// alignment: anchor selection is a pure function of a window's own
/// bytes ([`is_content_defined_anchor`]), so this can plant a repeat at
/// any distance and still have both occurrences sampled.
fn planted_repeat_content_defined(anchor: [u8; DETECTOR_ANCHOR_LEN], far_offset: usize) -> Vec<u8> {
    let filler = find_non_anchor_filler_byte();
    let mut data = vec![filler; far_offset + DETECTOR_ANCHOR_LEN];
    data[..DETECTOR_ANCHOR_LEN].copy_from_slice(&anchor);
    data[far_offset..].copy_from_slice(&anchor);
    data
}

#[test]
fn confirmed_detector_does_not_underflow_below_anchor_len() {
    // Regression for the reviewer-caught panic on PR #634: data.len() <
    // DETECTOR_ANCHOR_LEN but > base_window skipped the early return and
    // underflowed `data.len() - DETECTOR_ANCHOR_LEN` in a bare `for`
    // loop. This detector's `while i + ANCHOR_LEN <= len` shape degrades
    // to zero iterations instead, which this proves does not panic.
    // DETECTOR_ANCHOR_LEN is 8, so 3 bytes is short of it by construction.
    assert!(!likely_benefits_from_larger_window_content_defined_confirmed(&[1, 2, 3], 0));
}

/// Same shape as [`planted_repeat_content_defined`], but plants
/// [`DETECTOR_ANCHOR_LEN`] + [`DETECTOR_CONFIRM_LEN`] matching bytes
/// (`anchor` then `tail`) at both occurrences, giving
/// [`likely_benefits_from_larger_window_content_defined_confirmed`]'s
/// extra confirmation step a real run to agree on.
fn planted_repeat_content_defined_confirmed(
    anchor: [u8; DETECTOR_ANCHOR_LEN],
    tail: [u8; DETECTOR_CONFIRM_LEN],
    far_offset: usize,
) -> Vec<u8> {
    let filler = find_non_anchor_filler_byte();
    let run_len = DETECTOR_ANCHOR_LEN + DETECTOR_CONFIRM_LEN;
    let mut data = vec![filler; far_offset + run_len];
    data[..DETECTOR_ANCHOR_LEN].copy_from_slice(&anchor);
    data[DETECTOR_ANCHOR_LEN..run_len].copy_from_slice(&tail);
    data[far_offset..far_offset + DETECTOR_ANCHOR_LEN].copy_from_slice(&anchor);
    data[far_offset + DETECTOR_ANCHOR_LEN..far_offset + run_len].copy_from_slice(&tail);
    data
}

#[test]
fn confirmed_detector_is_true_when_the_repeat_extends_past_the_anchor() {
    // A real long-range recurrence: both occurrences agree on the
    // anchor AND the confirmation run past it, so requiring that
    // agreement must not cost the true positive the unconfirmed
    // detector already finds.
    let anchor = find_content_defined_anchor_value();
    let tail = [0xAAu8; DETECTOR_CONFIRM_LEN];
    let far_offset = 5 * DETECTOR_STRIDE + 7;
    let base_window = far_offset - 1;
    let data = planted_repeat_content_defined_confirmed(anchor, tail, far_offset);
    assert!(likely_benefits_from_larger_window_content_defined_confirmed(&data, base_window));
}

#[test]
fn confirmed_detector_is_false_on_an_anchor_match_that_does_not_extend() {
    // The exact failure mode research/JOURNAL.md S2-R9's second finding
    // named: the anchor recurs (a real 8-byte match, not a hash
    // collision) but the bytes past it differ, so there is no long
    // match behind it. S2-R9 measured an anchor-only check firing
    // `true` on `access_log` for exactly this shape; the confirmation
    // step exists to reject it.
    let anchor = find_content_defined_anchor_value();
    let far_offset = 5 * DETECTOR_STRIDE + 7;
    let base_window = far_offset - 1;
    let mut data = planted_repeat_content_defined_confirmed(
        anchor,
        [0xAAu8; DETECTOR_CONFIRM_LEN],
        far_offset,
    );
    // Diverge only the far occurrence's confirmation run so the anchor
    // itself still matches at both positions.
    let tail_start = far_offset + DETECTOR_ANCHOR_LEN;
    data[tail_start..tail_start + DETECTOR_CONFIRM_LEN].fill(0xBBu8);

    assert!(
        !likely_benefits_from_larger_window_content_defined_confirmed(&data, base_window),
        "confirmed detector must reject an anchor match that does not extend"
    );
}

#[test]
fn confirmed_detector_is_false_when_confirmation_would_run_past_data_end() {
    // A candidate whose confirmation window would read past data's end
    // cannot be confirmed either way; it must be treated as a miss,
    // not panic on an out-of-bounds slice.
    let anchor = find_content_defined_anchor_value();
    let far_offset = 5 * DETECTOR_STRIDE + 7;
    let base_window = far_offset - 1;
    // planted_repeat_content_defined only lays down the anchor itself
    // at far_offset, so data ends right after it: the far occurrence's
    // confirmation window necessarily runs past the end.
    let data = planted_repeat_content_defined(anchor, far_offset);
    assert_eq!(data.len(), far_offset + DETECTOR_ANCHOR_LEN);

    assert!(!likely_benefits_from_larger_window_content_defined_confirmed(&data, base_window));
}

#[test]
fn adaptive_window_matches_wired_window_within_window() {
    // data.len() <= WINDOW makes the gate's own base_window early
    // return unconditional (research/JOURNAL.md S1-P4's next slice
    // after S2-A92), so the adaptive parse must be byte-for-byte the
    // wired parse: this is the branch every real input at or under
    // today's WINDOW takes.
    let data: Vec<u8> = (0..5000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    assert_eq!(parse_optimal_adaptive_window(&data), parse_optimal(&data));
}

#[test]
fn adaptive_window_roundtrips_a_confirmed_repeat_past_window() {
    // The scenario this slice exists for: a real recurrence whose
    // distance exceeds the wired WINDOW but stays under
    // ADAPTIVE_WINDOW. First proves the gate actually fires on this
    // input (the precondition the rest of the test depends on), then
    // proves CLAUDE.md hard rule 1 holds through the grown window: the
    // token stream this produces is the first in this crate to carry a
    // real match distance beyond WINDOW end to end through
    // parse_optimal's real three-round DP, not just dp_round called
    // directly with a wider window parameter.
    let anchor = find_content_defined_anchor_value();
    let tail = [0xAAu8; DETECTOR_CONFIRM_LEN];
    let far_offset = WINDOW + 5 * DETECTOR_STRIDE + 7;
    let data = planted_repeat_content_defined_confirmed(anchor, tail, far_offset);
    assert!(
        far_offset < ADAPTIVE_WINDOW,
        "planted distance must stay reachable at ADAPTIVE_WINDOW"
    );
    assert!(
        likely_benefits_from_larger_window_content_defined_confirmed(&data, WINDOW),
        "gate must fire so this test exercises the grown-window branch"
    );

    let tokens = parse_optimal_adaptive_window(&data);
    assert_eq!(replay(&tokens), data, "roundtrip mismatch");
    assert!(
        tokens.iter().any(|token| matches!(
            *token,
            Token::Match { distance, .. } if distance.get() as usize > WINDOW
        )),
        "expected at least one match reaching past WINDOW, proving the grown window was searched"
    );
}
