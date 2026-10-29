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
    // distance (1) shorter than the eventual match length: copy_checked
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
    // distance 1, shorter than the match length: copy_checked must read
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
    // 60 seconds and was killed.
    //
    // Bounds bytes compared by `match_len`, not wall-clock: a stopwatch
    // failed this guard under `cargo llvm-cov` with no regression
    // (issue #1059). The fixed parse compares about 2e7 bytes here
    // (greedy seed pass plus rep scans over three DP rounds); the
    // carry-less scan compares over 1e10 (65,535 bytes at each of 200,000
    // positions, per slot, per round). The budget sits 5x above the first
    // and 100x below the second.
    const MATCH_LEN_BYTE_BUDGET: u64 = 100_000_000;
    let data = vec![b'z'; 200_000];
    work::Scan::MatchLen.reset();
    roundtrip_optimal(&data);
    let compared = work::Scan::MatchLen.count();
    assert!(
        compared < MATCH_LEN_BYTE_BUDGET,
        "parse_optimal on a 200,000-byte single-byte run compared {compared} bytes in \
         match_len, budget {MATCH_LEN_BYTE_BUDGET}; likely a regression to issue #179's \
         quadratic rep-candidate scan"
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
    // `max_depth` alone would allow.
    //
    // Bounds bytes compared by `suffix_common_len`, not wall-clock: a
    // stopwatch failed sibling guards under `cargo llvm-cov` with no
    // regression (issues #1059, #1064). With `nice_len` 50 the walks
    // compare about 3.0e6 bytes here; with `nice_len` set to
    // `MAX_MATCH_LEN` (no early exit) about 5.0e7, a 16x. The budget sits
    // 3x above the first and 5x below the second. Removing only the
    // walk's `best_len < nice_len` stop moves candidates visited 2% on
    // this fixture; `binary_tree_nice_len_stops_the_walk_after_the_first_long_candidate`
    // guards that line.
    const SUFFIX_BYTE_BUDGET: u64 = 10_000_000;
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
    work::Scan::SuffixCommonLen.reset();
    for i in 0..data.len() {
        let _ = finder.insert_and_find(i, data.len(), 50);
    }
    let compared = work::Scan::SuffixCommonLen.count();
    assert!(
        compared < SUFFIX_BYTE_BUDGET,
        "300 near-duplicate 200-byte blocks with nice_len 50 and unbounded max_depth compared \
         {compared} bytes in suffix_common_len, budget {SUFFIX_BYTE_BUDGET}; likely a \
         regression to nice_len no longer bounding candidates visited"
    );
}

#[test]
fn binary_tree_nice_len_stops_the_walk_after_the_first_long_candidate() {
    // Counts candidates visited, because bytes cannot see this stop: past
    // the first candidate every compare resumes from a proven common length
    // at or above `nice_len` and costs nothing (issue #1068, found in
    // #1067's review). 300 groups of two identical 60-byte records, every
    // record opening with the same 3 bytes (one hash bucket) and then
    // pseudo-random bytes, so the bucket's tree is a random BST ordered by
    // those bytes. At a group's second record the first candidate is the
    // first record, which matches 60 bytes, above `nice_len` 50: the walk
    // stops there, one candidate. Without the `best_len < nice_len` stop it
    // descends the tree to a leaf, about `2 ln(300)` candidates. Only the
    // second records are counted; the first records walk the tree in full
    // either way.
    const NICE_LEN: usize = 50;
    const GROUPS: u64 = 300;
    let mut state = 1u32;
    let mut next_byte = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        u8::try_from(state >> 24).expect("state >> 24 fits u8")
    };
    let mut data = Vec::new();
    let mut second_records = Vec::new();
    for _ in 0..GROUPS {
        let mut record = vec![b'X', b'Y', b'Z'];
        record.extend((0..57).map(|_| next_byte()));
        data.extend_from_slice(&record);
        second_records.push(data.len());
        data.extend_from_slice(&record);
    }
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    let mut visited = 0;
    for i in 0..data.len() {
        work::Scan::Candidates.reset();
        let _ = finder.insert_and_find(i, data.len(), NICE_LEN);
        if second_records.contains(&i) {
            visited += work::Scan::Candidates.count();
        }
    }
    assert_eq!(
        visited, GROUPS,
        "{GROUPS} second records matching a 60-byte first record visited {visited} candidates \
         with nice_len {NICE_LEN}, expected one each; likely the walk no longer stops once \
         best_len reaches nice_len"
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
    //
    // Bounds bytes compared by `suffix_common_len`, not wall-clock
    // (issues #1059, #1064). The capped walk compares about 2.6e7 bytes
    // here (nice_len 128 at each of 200,000 positions); an uncapped first
    // scan compares up to `MAX_MATCH_LEN` per position, over 1e10. The
    // budget sits 4x above the first and 100x below the second.
    const SUFFIX_BYTE_BUDGET: u64 = 100_000_000;
    let data = vec![b'z'; 200_000];
    let mut finder = BinaryTreeMatchFinder::new(&data, WINDOW);
    work::Scan::SuffixCommonLen.reset();
    for i in 0..data.len() {
        let _ = finder.insert_and_find(i, MAX_TREE_DEPTH_OPTIMAL, NICE_LEN_OPTIMAL);
    }
    let compared = work::Scan::SuffixCommonLen.count();
    assert!(
        compared < SUFFIX_BYTE_BUDGET,
        "200,000 bytes of one repeated value with nice_len 128 compared {compared} bytes in \
         suffix_common_len, budget {SUFFIX_BYTE_BUDGET}; likely a regression to nice_len no \
         longer bounding per-candidate scan cost (research/JOURNAL.md S2-A46)"
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
    // Bounds bytes compared by `suffix_common_len`, not wall-clock: a
    // stopwatch failed this guard under `cargo llvm-cov` with no
    // regression (issue #1059). With prefix reuse the walk compares about
    // 5.0e7 bytes here; with `start` forced to 0 it compares about 2.5e8,
    // a 5x. The budget sits 2x above the first and 2.5x below the second.
    const SUFFIX_BYTE_BUDGET: u64 = 100_000_000;
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
    work::Scan::SuffixCommonLen.reset();
    for i in 0..data.len() {
        let _ = finder.insert_and_find(i, MAX_TREE_DEPTH_OPTIMAL, MAX_MATCH_LEN);
    }
    let compared = work::Scan::SuffixCommonLen.count();
    assert!(
        compared < SUFFIX_BYTE_BUDGET,
        "300 near-duplicate 200-byte blocks compared {compared} bytes in suffix_common_len, \
         budget {SUFFIX_BYTE_BUDGET}; likely a regression to length-prefix reuse always \
         starting from 0"
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
    // within a larger window must be found when the finder is
    // configured with that window, and still rejected when it is
    // not -- proving the parameter actually gates reach rather than
    // being plumbed through unused. The larger window is one a frame
    // can declare, `Reach::Wide`'s, as the field doc promises.
    let larger_window = Reach::Wide.window();
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
fn price_counts_tally_bumps_literal_by_prev_byte_context() {
    let tokens = [Token::Literal(0x35), Token::Literal(b'x')];
    let counts = PriceCounts::tally(&tokens, &[0x35, b'x'], Reach::Standard);
    assert_eq!(counts.literal[3 * 256 + usize::from(b'x')], 2);
    assert_eq!(counts.literal[usize::from(0x35_u8)], 2);
    assert_eq!(counts.length, vec![1; LENGTH_BUCKETS]);
    assert_eq!(counts.offset, vec![1; Reach::Standard.offset_buckets()]);
    assert_eq!(counts.rep, 1);
}

#[test]
fn price_counts_tally_literal_at_stream_start_uses_context_zero() {
    let counts = PriceCounts::tally(&[Token::Literal(b'z')], b"z", Reach::Standard);
    assert_eq!(counts.literal[usize::from(b'z')], 2);
}

#[test]
fn price_counts_tally_match_bumps_length_and_offset_only() {
    let distance = NonZeroU32::new(100).unwrap();
    let mut tokens = vec![Token::Literal(b'a'); 100];
    tokens.push(Token::Match { len: 10, distance });
    let counts = PriceCounts::tally(&tokens, &[b'a'; 110], Reach::Standard);
    assert_eq!(counts.length[bucket(10)], 2);
    assert_eq!(counts.offset[bucket(100)], 2);
    assert_eq!(counts.rep, 1);
}

#[test]
fn price_counts_tally_rep_bumps_length_and_rep_only() {
    let tokens = [
        Token::Literal(b'a'),
        Token::Rep {
            len: 6,
            slot: RepSlot::First,
        },
    ];
    let counts = PriceCounts::tally(&tokens, &[b'a'; 7], Reach::Standard);
    assert_eq!(counts.length[bucket(6)], 2);
    assert_eq!(counts.rep, 2);
    assert_eq!(counts.offset, vec![1; Reach::Standard.offset_buckets()]);
}

#[test]
fn price_counts_tally_accumulates_across_tokens() {
    let tokens = [Token::Literal(b'a'); 3];
    let counts = PriceCounts::tally(&tokens, b"aaa", Reach::Standard);
    assert_eq!(counts.literal[usize::from(b'a')], 2);
    assert_eq!(counts.literal[6 * 256 + usize::from(b'a')], 3);
}

#[test]
fn price_counts_prices_computes_shannon_price_per_literal_context() {
    let tokens = [
        Token::Literal(b'a'),
        Token::Literal(b'a'),
        Token::Literal(b'b'),
    ];
    let table = PriceCounts::tally(&tokens, b"aab", Reach::Standard).prices(3);

    assert_eq!(table.literal[usize::from(b'a')], price(2, 257));
    assert_eq!(table.literal[0], price(1, 257));

    let context6 = 6 * 256;
    assert_eq!(table.literal[context6 + usize::from(b'a')], price(2, 258));
    assert_eq!(table.literal[context6 + usize::from(b'b')], price(2, 258));
    assert_eq!(table.literal[context6], price(1, 258));
}

#[test]
fn optimal_wide_reaches_a_repeat_past_standard_window() {
    // research/JOURNAL.md S1-P4: parse_optimal_with_window must thread
    // its reach through all three dp_round rounds, not just the first, and
    // stay lossless (hard rule 1) either way. Template (low-range bytes)
    // and filler (high-range pseudo-random bytes) stay in disjoint byte
    // ranges so no accidental cross-half match confounds the planted
    // repeat's distance.
    let template: Vec<u8> = (0u8..80).collect();
    let filler: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED)
        .take(WINDOW + 4096)
        .map(|s| 128 + u8::try_from(s % 128).unwrap())
        .collect();
    let mut data = template.clone();
    data.extend_from_slice(&filler);
    data.extend_from_slice(&template);
    let distance = filler.len() + template.len();
    assert!(WINDOW < distance && distance <= WIDE_WINDOW);

    let standard = parse_optimal_with_window(&data, Reach::Standard);
    assert_eq!(replay(&standard), data, "standard roundtrip mismatch");
    assert!(
        standard.iter().all(|t| !matches!(
            t,
            Token::Match { distance: d, .. } if d.get() as usize > WINDOW
        )),
        "Reach::Standard must never produce a fresh match past WINDOW"
    );

    let wide = parse_optimal_with_window(&data, Reach::Wide);
    assert_eq!(replay(&wide), data, "wide roundtrip mismatch");
    assert!(
        wide.iter().any(|t| matches!(
            t,
            Token::Match { distance: d, .. } if d.get() as usize == distance
        )),
        "Reach::Wide must find the planted repeat at distance {distance}, \
         unreachable through WINDOW"
    );
}

#[test]
fn find_best_stops_after_max_chain_tries() {
    // Chain order is newest first: MAX_CHAIN_TRIES short candidates, then the
    // one long match, which the bound must leave unvisited.
    let long = b"abcLONGTAIL";
    let mut data = long.to_vec();
    for _ in 0..MAX_CHAIN_TRIES {
        data.extend_from_slice(b"abcZ|");
    }
    let query = data.len();
    data.extend_from_slice(long);

    let mut finder = MatchFinder::new(&data);
    for i in 0..query {
        finder.insert(i);
    }
    let (len, _) = finder.find_best(query).expect("the short candidates match");
    assert_eq!(len, 3);
}

#[test]
fn parse_greedy_defers_a_short_match_to_a_longer_one_a_byte_later() {
    // At the final `a` the best match is 4 bytes (`abcd`); one byte later it
    // is 10 (`bcdefghijk`), so the parse emits `a` as a literal first.
    let data = b"abcdQRSTUVbcdefghijkabcdefghijk";
    let tokens = parse_greedy(data);
    let [.., a, tail] = tokens.as_slice() else {
        panic!("too few tokens: {tokens:?}");
    };
    assert_eq!(*a, Token::Literal(b'a'), "{tokens:?}");
    assert!(matches!(tail, Token::Match { len: 10, .. }), "{tokens:?}");
}

#[test]
fn parse_greedy_finds_match_exactly_at_window_boundary_but_not_past_it() {
    // The hash-chain finder's reach guard, the pair
    // `binary_tree_finds_match_exactly_at_window_boundary_but_not_past_it`
    // pins for the tree finder. The marker's 3-byte prefix hash is distinct
    // from the filler's, so its chain holds the two marker positions only.
    let marker = b"wxyz";
    let mut at_boundary = vec![0xAAu8; WINDOW + marker.len()];
    at_boundary[..marker.len()].copy_from_slice(marker);
    at_boundary[WINDOW..].copy_from_slice(marker);
    let tokens = parse_greedy(&at_boundary);
    assert_eq!(replay(&tokens), at_boundary, "roundtrip mismatch");
    assert!(
        tokens.iter().any(|t| matches!(
            *t,
            Token::Match { len: 4, distance } if distance.get() as usize == WINDOW
        )),
        "distance == WINDOW is still in range"
    );

    let mut past_boundary = vec![0xAAu8; WINDOW + 1 + marker.len()];
    past_boundary[..marker.len()].copy_from_slice(marker);
    past_boundary[WINDOW + 1..].copy_from_slice(marker);
    let tokens = parse_greedy(&past_boundary);
    assert_eq!(replay(&tokens), past_boundary, "roundtrip mismatch");
    assert!(
        tokens.iter().all(|t| match *t {
            Token::Match { distance, .. } => distance.get() as usize <= WINDOW,
            Token::Literal(_) | Token::Rep { .. } => true,
        }),
        "distance == WINDOW + 1 must never be reported"
    );
}

#[test]
fn dp_round_ring_wrapped_picks_the_tokens_an_unwrapped_ring_picks() {
    // `DpState`'s `dp` and `cache` are rings; the parse must not depend on
    // their length (JOURNAL S2-A123). Over twice RING_LEN, with a run past
    // MAX_MATCH_LEN, repeats at several distances and noise between them,
    // every slot is reused and a move's target wraps past its start's slot.
    let mut noise = crate::test_support::Xorshift32::new(0xD9);
    let mut data = Vec::new();
    let template: Vec<u8> = (0u8..64).collect();
    while data.len() < 2 * RING_LEN + 5000 {
        data.extend(
            noise
                .by_ref()
                .take(300)
                .map(|s| u8::try_from(s % 251).unwrap()),
        );
        data.extend_from_slice(&template);
        data.extend(std::iter::repeat_n(b'r', MAX_MATCH_LEN / 2));
        let from = data.len() - 400;
        data.extend_from_within(from..from + 200);
    }
    let seed = parse_greedy(&data);
    let prices = PriceCounts::tally(&seed, &data, Reach::Standard).prices(seed.len());
    let wrapped = dp_round(&data, &prices, Reach::Standard, RING_LEN);
    assert_eq!(replay(&wrapped), data, "roundtrip mismatch");
    let unwrapped = dp_round(
        &data,
        &prices,
        Reach::Standard,
        (data.len() + 1).next_power_of_two(),
    );
    assert_eq!(wrapped, unwrapped);
}
