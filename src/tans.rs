//! tANS table normalization: [`normalize_frequencies`], a standalone
//! primitive for ROADMAP M5's speed-tier work (`research/JOURNAL.md`
//! S1-P6, "speed tier"), issue #447. Not a port: the founding session never
//! implemented ANS-family coding (grepped
//! `research/imports/session-1/mothergod.rs` clean of any tANS/rANS code),
//! so there is no archive behavior to carry forward, same situation
//! [`crate::sse`] and [`crate::ppm`] documented for their own leads
//! (ADR-0006).
//!
//! **The gap this closes.** S2-A77 cut `Literal::mix`'s per-byte cost by
//! autovectorizing its multiply-add pass; S2-A78 then proved the remaining
//! from-scratch `cum` rebuild cannot be made incremental without a
//! `FORMAT_VERSION`-bump-class change to what gets coded, and S2-A80 found
//! the crate's own `forbid(unsafe_code)` (ADR-0017) closes explicit AVX2
//! intrinsics regardless of any measurement. Both left the tANS fast path
//! (`ROADMAP.md` M5, "tANS fast path (level -1 mode)") as S1-P6's sole
//! remaining open scope: a from-scratch entropy coder with O(1)
//! table-lookup encode/decode, no per-byte rebuild to eliminate because
//! there is nothing to incrementally maintain in the first place.
//!
//! **This slice.** Every tANS/FSE-family coder needs a *normalized*
//! frequency table before it can build encode or decode tables from it: raw
//! symbol counts summed to an arbitrary total, rescaled onto a power-of-two
//! total (`1 << table_log2`) so the coder's state transform can use shifts
//! and masks instead of division. [`normalize_frequencies`] is that
//! rescaling step alone, standalone and not yet called from anywhere:
//! largest-remainder rounding (floor each symbol's ideal share, then settle
//! the total exactly by nudging the entries whose rounding lost or gained
//! the most, in a fixed deterministic order) so every originally-nonzero
//! symbol keeps a nonzero share -- a symbol tANS could never code
//! otherwise, the same "never seen" pitfall [`crate::ppm`]'s module doc
//! discusses for a different table shape.
//!
//! **Remaining S1-P6 scope.** Building the encode/decode state-transform
//! tables from a normalized frequency table (the "spread" step), the
//! coder's actual state machine, wiring it behind a new fast `Method`
//! variant, and the `FORMAT_VERSION` bump and real-bitstream measurement
//! that wiring needs.

/// Rescales `counts` onto a table of exactly `1 << table_log2` slots,
/// preserving every originally-nonzero entry's nonzero-ness.
///
/// Each symbol's ideal share is `counts[i] * target / total`, computed
/// exactly in `u64` and floored; a symbol with `counts[i] > 0` whose floor
/// rounds to zero is bumped to 1 first (nothing tANS could ever code
/// otherwise). Flooring (and the bump) leaves the sum at or below or above
/// `target` depending on how much each entry's fractional share was
/// discarded or added back, so the remaining slots (or excess) are settled
/// by the largest-remainder method: entries are ranked by
/// `(counts[i] * target) % total`, the exact fractional part flooring
/// discarded, and slots are added to (or removed from) the entries whose
/// rounding was least faithful to their true share first, breaking ties by
/// ascending index, this function's own tie-break choice for determinism.
/// Removing a slot only ever touches an entry already above 1, so a symbol
/// that started nonzero never reaches zero.
///
/// A symbol with `counts[i] == 0` always keeps a normalized frequency of 0:
/// this function only redistributes weight among symbols that occurred at
/// least once.
///
/// # Panics
///
/// Panics if `table_log2 >= 32` (`1 << table_log2` would not fit `u32`,
/// which every returned frequency is guaranteed to fit). Panics if the
/// number of distinct (nonzero) symbols in `counts` exceeds
/// `1 << table_log2`: no assignment of table slots can give every one of
/// them at least 1 out of fewer total slots than there are symbols to
/// cover, so this is a caller error (the caller chose too small a
/// `table_log2` for this alphabet), never a property of adversarial input
/// -- this primitive is not yet reachable from any decode path.
#[must_use]
pub fn normalize_frequencies(counts: &[u32], table_log2: u32) -> Vec<u32> {
    assert!(
        table_log2 < 32,
        "table_log2 must fit a u32 shift; got {table_log2}"
    );
    let target: u64 = 1u64 << table_log2;
    let total: u64 = counts.iter().map(|&c| u64::from(c)).sum();
    if total == 0 {
        return vec![0; counts.len()];
    }

    let distinct = counts.iter().filter(|&&c| c > 0).count() as u64;
    assert!(
        distinct <= target,
        "table_log2 too small: {distinct} distinct symbols need at least \
         {distinct} of the {target} slots a table_log2 of {table_log2} provides"
    );

    let mut freq: Vec<u64> = counts
        .iter()
        .map(|&c| {
            if c == 0 {
                0
            } else {
                ((u64::from(c) * target) / total).max(1)
            }
        })
        .collect();

    let remainder = |i: usize| (u64::from(counts[i]) * target) % total;
    let mut nonzero: Vec<usize> = (0..counts.len()).filter(|&i| counts[i] > 0).collect();
    let allocated: u64 = freq.iter().sum();

    if allocated < target {
        // Largest fractional remainder first: those entries' floor cost
        // them the most relative to their true share, so they are first in
        // line for the slot rounding otherwise discarded.
        nonzero.sort_by(|&a, &b| remainder(b).cmp(&remainder(a)).then(a.cmp(&b)));
        let mut deficit = target - allocated;
        let mut idx = 0;
        while deficit > 0 {
            freq[nonzero[idx % nonzero.len()]] += 1;
            deficit -= 1;
            idx += 1;
        }
    } else if allocated > target {
        // Smallest fractional remainder first: those entries' floor (or
        // the forced bump to 1) overshot their true share the most, so
        // they give a slot back first. distinct <= target guarantees a
        // reachable target with every entry staying >= 1 (see doc above).
        nonzero.sort_by(|&a, &b| remainder(a).cmp(&remainder(b)).then(a.cmp(&b)));
        let mut surplus = allocated - target;
        let mut idx = 0;
        while surplus > 0 {
            let i = nonzero[idx % nonzero.len()];
            if freq[i] > 1 {
                freq[i] -= 1;
                surplus -= 1;
            }
            idx += 1;
        }
    }

    freq.into_iter()
        .map(|f| u32::try_from(f).expect("every entry stays within target, itself a valid u32"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "must fit a u32 shift")]
    fn table_log2_of_32_panics() {
        let _ = normalize_frequencies(&[1, 1], 32);
    }

    #[test]
    #[should_panic(expected = "table_log2 too small")]
    fn more_distinct_symbols_than_slots_panics() {
        let _ = normalize_frequencies(&[1, 1, 1, 1, 1], 2);
    }

    #[test]
    fn all_zero_counts_stay_all_zero() {
        assert_eq!(normalize_frequencies(&[0, 0, 0], 4), vec![0, 0, 0]);
    }

    #[test]
    fn zero_entries_stay_zero_alongside_nonzero_ones() {
        let freq = normalize_frequencies(&[5, 0, 3, 0], 4);
        assert_eq!(freq[1], 0);
        assert_eq!(freq[3], 0);
        assert_eq!(freq[0] + freq[2], 16);
    }

    #[test]
    fn exact_power_of_two_counts_pass_through_unchanged() {
        assert_eq!(normalize_frequencies(&[1, 1, 1, 1], 2), vec![1, 1, 1, 1]);
        assert_eq!(normalize_frequencies(&[2, 2, 4], 3), vec![2, 2, 4]);
    }

    #[test]
    fn sums_to_exactly_the_target_total() {
        let cases: &[(&[u32], u32)] = &[
            (&[100, 1, 1], 3),
            (&[1, 1, 1], 2),
            (&[7, 5, 3, 1], 5),
            (&[1000, 1, 1, 1, 1, 1, 1, 1], 4),
            (&[3, 3, 3, 3, 3, 3, 3, 3, 3], 6),
            (&[255, 254, 253, 1], 10),
        ];
        for &(counts, table_log2) in cases {
            let freq = normalize_frequencies(counts, table_log2);
            let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
            assert_eq!(
                sum,
                1u64 << table_log2,
                "counts={counts:?} table_log2={table_log2} freq={freq:?}"
            );
        }
    }

    #[test]
    fn every_originally_nonzero_symbol_keeps_a_nonzero_share() {
        let counts = [1000, 1, 1, 1, 1, 1, 1, 1];
        let freq = normalize_frequencies(&counts, 4);
        for (i, &c) in counts.iter().enumerate() {
            if c > 0 {
                assert!(
                    freq[i] > 0,
                    "symbol {i} started nonzero but normalized to 0"
                );
            }
        }
    }

    #[test]
    fn a_dominant_symbol_gets_most_of_the_table() {
        let freq = normalize_frequencies(&[1000, 1, 1], 8);
        assert!(freq[0] > freq[1] && freq[0] > freq[2]);
        assert_eq!(freq[0] + freq[1] + freq[2], 256);
    }

    #[test]
    fn normalization_is_deterministic() {
        let counts = [37, 19, 0, 5, 5, 200, 1, 1, 1, 1, 1];
        let first = normalize_frequencies(&counts, 10);
        let second = normalize_frequencies(&counts, 10);
        assert_eq!(first, second);
    }

    #[test]
    fn distinct_equal_to_target_forces_every_entry_to_exactly_one() {
        let freq = normalize_frequencies(&[50, 30, 20, 5, 0], 2);
        assert_eq!(freq, vec![1, 1, 1, 1, 0]);
    }

    #[test]
    fn large_counts_do_not_overflow() {
        let counts = [u32::MAX, u32::MAX / 2, 1];
        let freq = normalize_frequencies(&counts, 12);
        let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
        assert_eq!(sum, 1u64 << 12);
        assert!(freq.iter().all(|&f| f > 0));
    }
}
