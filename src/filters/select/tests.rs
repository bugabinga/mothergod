use super::*;

/// Small-step lookup for an xorshift-free LCG-driven random walk:
/// wrapping-u8 equivalents of -2..=2, indexed by `seed % 5`. Table
/// form sidesteps signed/unsigned cast lints entirely, instead of
/// converting a signed step through `as`.
const WALK_STEPS: [u8; 5] = [0u8.wrapping_sub(2), 0u8.wrapping_sub(1), 0, 1, 2];

/// Advances `seed` (an LCG state) and returns the next
/// [`WALK_STEPS`] entry it selects.
fn next_step(seed: &mut u32) -> u8 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let index = usize::try_from((*seed >> 24) % 5).unwrap_or(0);
    WALK_STEPS[index]
}

#[test]
fn pick_always_includes_identity() {
    for data in [&b""[..], b"x", b"abababababababab", &[0u8; 5000]] {
        assert!(
            pick(data).contains(&Candidate::Identity),
            "no identity candidate for {data:?}"
        );
    }
}

#[test]
fn pick_selects_delta_for_columnar_drift() {
    // 4 independent small random walks, one per column, interleaved
    // row-major: consecutive same-column values (stride 4) differ
    // by a small step, but consecutive raw bytes belong to
    // unrelated walks and look noisy.
    let mut seeds = [0x1234_5678u32, 0x9abc_def0, 0x0f0f_f0f0, 0x1357_9bdf];
    let mut walk = [64u8, 96, 160, 200];
    let rows = 2000usize;
    let mut data = Vec::with_capacity(rows * 4);
    for _ in 0..rows {
        for col in 0..4 {
            walk[col] = walk[col].wrapping_add(next_step(&mut seeds[col]));
            data.push(walk[col]);
        }
    }
    let candidates = pick(&data);
    assert_eq!(
        candidates[0],
        Candidate::Delta(NonZeroUsize::new(4).unwrap())
    );
}

#[test]
fn pick_shortlists_bcj_for_opcode_dense_data() {
    let mut data = vec![0x90u8; 1000];
    for chunk in data.chunks_mut(20) {
        chunk[0] = 0xE8;
    }
    assert!(pick(&data).contains(&Candidate::Bcj));
}

#[test]
fn pick_does_not_shortlist_bcj_for_sparse_opcodes() {
    let data = vec![0x90u8; 100_000];
    assert!(!pick(&data).contains(&Candidate::Bcj));
}

#[test]
fn pick_skips_transpose_below_minimum_length() {
    let mut data = vec![0u8; MIN_TRANSPOSE_LEN - 1];
    for (i, b) in data.iter_mut().enumerate() {
        *b = u8::from(i % 4 == 0) * 200;
    }
    assert!(
        !pick(&data)
            .iter()
            .any(|c| matches!(c, Candidate::Transpose(_)))
    );
}

#[test]
fn pick_shortlists_transpose_for_column_structured_data() {
    // 8 independent small random walks, one per column, interleaved
    // row-major: the value at a given position stays close to the
    // same column's previous-row value, but jumps arbitrarily
    // relative to its raw immediate predecessor (a different
    // column's unrelated walk).
    let columns = 8usize;
    let mut seeds: Vec<u32> = (0..columns)
        .map(|c| {
            let c = u32::try_from(c).unwrap_or(0);
            0x9e37_79b9u32.wrapping_mul(c + 1)
        })
        .collect();
    let mut walk = vec![128u8; columns];
    let rows = 2000usize;
    let mut data = vec![0u8; columns * rows];
    for row in 0..rows {
        for col in 0..columns {
            walk[col] = walk[col].wrapping_add(next_step(&mut seeds[col]));
            data[row * columns + col] = walk[col];
        }
    }
    let candidates = pick(&data);
    assert!(
        candidates
            .iter()
            .any(|c| matches!(c, Candidate::Transpose(_))),
        "no transpose candidate; got {candidates:?}"
    );
}

#[test]
fn header_bytes_round_trip_every_candidate_kind() {
    for candidate in [
        Candidate::Identity,
        Candidate::Delta(NonZeroUsize::new(1).unwrap()),
        Candidate::Delta(NonZeroUsize::new(96).unwrap()),
        Candidate::Bcj,
        Candidate::Transpose(NonZeroUsize::new(2).unwrap()),
        Candidate::Transpose(NonZeroUsize::new(96).unwrap()),
    ] {
        let bytes = candidate.to_header_bytes();
        assert_eq!(
            Candidate::from_header_bytes(bytes),
            Some(candidate),
            "round trip failed for {candidate:?} via {bytes:?}"
        );
    }
}

#[test]
fn header_bytes_reject_unknown_kind() {
    assert_eq!(Candidate::from_header_bytes([4, 0]), None);
    assert_eq!(Candidate::from_header_bytes([255, 255]), None);
}

#[test]
fn header_bytes_reject_zero_param_for_parameterized_kinds() {
    assert_eq!(Candidate::from_header_bytes([1, 0]), None);
    assert_eq!(Candidate::from_header_bytes([3, 0]), None);
}

#[test]
fn header_bytes_reject_nonzero_param_for_parameterless_kinds() {
    assert_eq!(Candidate::from_header_bytes([0, 1]), None);
    assert_eq!(Candidate::from_header_bytes([2, 1]), None);
}
