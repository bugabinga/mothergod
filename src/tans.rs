//! tANS table construction: [`normalize_frequencies`] and
//! [`spread_symbols`], standalone primitives for ROADMAP M5's speed-tier
//! work (`research/JOURNAL.md` S1-P6, "speed tier"), issue #447. Not a
//! port: the founding session never implemented ANS-family coding (grepped
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
//! **First slice (S2-A81).** [`normalize_frequencies`]: raw symbol counts
//! summed to an arbitrary total, rescaled onto a power-of-two total
//! (`1 << table_log2`) so the coder's state transform can use shifts and
//! masks instead of division.
//!
//! **This slice.** Every tANS/FSE-family coder next assigns each of the
//! `1 << table_log2` table slots to exactly one symbol -- the "spread"
//! step -- before it can build encode or decode transition tables from
//! that assignment. [`spread_symbols`] is that assignment alone: symbol
//! `s` occupies exactly `freq[s]` slots, scattered by a fixed odd stride
//! (coprime to the power-of-two table size, so its orbit covers every
//! slot exactly once) instead of packed contiguously, so a table walked in
//! slot order interleaves symbols close to their frequency order rather
//! than running one symbol at a time -- the shape the decode table built
//! from it depends on. Standalone and not yet called from anywhere.
//!
//! **Remaining S1-P6 scope.** Building the encode/decode transition tables
//! from a spread assignment (state deltas and next-state arithmetic), the
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
        // they give slots back first. distinct <= target guarantees a
        // reachable target with every entry staying >= 1 (see doc above).
        // One forward pass: each entry gives up as much of its headroom
        // (freq[i] - 1) as the remaining surplus needs before the pass
        // moves on, rather than 1 slot per entry per revisit. The prior
        // round-robin form re-scanned every already-exhausted entry on
        // every single-slot removal, O(nonzero.len() * surplus) when
        // almost all entries sit at the floor and one dominant entry
        // holds the whole surplus (measured quadratic, PR #576 review).
        nonzero.sort_by(|&a, &b| remainder(a).cmp(&remainder(b)).then(a.cmp(&b)));
        let mut surplus = allocated - target;
        for i in nonzero {
            if surplus == 0 {
                break;
            }
            let headroom = freq[i] - 1;
            let take = headroom.min(surplus);
            freq[i] -= take;
            surplus -= take;
        }
    }

    freq.into_iter()
        .map(|f| u32::try_from(f).expect("every entry stays within target, itself a valid u32"))
        .collect()
}

/// The stride a table of `table_size` slots advances by per placement in
/// [`spread_symbols`]. Any odd stride is coprime to a power-of-two
/// `table_size`, so repeatedly adding it mod `table_size` visits every
/// slot exactly once before returning to the start -- the only property
/// [`spread_symbols`]'s correctness depends on. The magnitude (half the
/// table plus an eighth of it plus three) is FSE's own constant, chosen to
/// scatter placements rather than cluster them; forcing the low bit is
/// this function's own fix for `table_size == 8`, where the constant
/// alone lands on 8, itself even.
fn spread_stride(table_size: usize) -> usize {
    ((table_size >> 1) + (table_size >> 3) + 3) | 1
}

/// Assigns each of the `1 << table_log2` table slots to exactly one
/// symbol, from a normalized frequency table -- one whose entries already
/// sum to `1 << table_log2`, [`normalize_frequencies`]'s postcondition.
///
/// Symbol `s` occupies exactly `freq[s]` of the returned table's slots.
/// Slots fill in stride order (see `spread_stride`) rather than
/// contiguously per symbol, so reading the result in slot order
/// interleaves symbols instead of running one symbol at a time -- the
/// classic FSE/tANS "spread" step, and the shape a decode table built from
/// this assignment depends on.
///
/// # Panics
///
/// Panics if `table_log2 >= 32` (`1 << table_log2` would not fit the `u32`
/// shift every caller of this module uses, same bound
/// [`normalize_frequencies`] enforces). Panics if `freq` does not sum to
/// exactly `1 << table_log2`: this function assigns slots to symbols by
/// construction and has no notion of a slot left over or a symbol short of
/// its share, so a mismatched total is a caller error, never a property of
/// adversarial input -- this primitive is not yet reachable from any
/// decode path.
#[must_use]
pub fn spread_symbols(freq: &[u32], table_log2: u32) -> Vec<u32> {
    assert!(
        table_log2 < 32,
        "table_log2 must fit a u32 shift; got {table_log2}"
    );
    let table_size = 1usize << table_log2;
    let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
    assert!(
        sum == table_size as u64,
        "freq must sum to exactly 1 << table_log2 ({table_size}); got {sum}. \
         Call normalize_frequencies first."
    );

    let mask = table_size - 1;
    let stride = spread_stride(table_size);

    let mut table = vec![0u32; table_size];
    let mut position = 0usize;
    for (symbol, &count) in freq.iter().enumerate() {
        let symbol = u32::try_from(symbol)
            .expect("alphabet size fits u32, same bound freq's caller respects");
        for _ in 0..count {
            table[position] = symbol;
            position = (position + stride) & mask;
        }
    }
    table
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
    fn surplus_removal_stays_near_linear_with_one_dominant_outlier() {
        // The shape PR #576 review measured quadratic on: n-1 symbols
        // pinned at the forced-nonzero floor of 1 (their natural share
        // floors to 0 and gets bumped), one dominant symbol whose own
        // floor absorbs the entire resulting surplus. Round-robin removal
        // re-scanned every already-exhausted entry once per single-slot
        // removal (750ms at n=20,000); the single forward pass this test
        // guards removes each entry's headroom in one visit instead.
        // table_log2=15 keeps distinct (n) <= target (32,768) while
        // staying far enough below `total` that every floor-1 entry needs
        // the forced bump, which is what produces the surplus (as opposed
        // to a deficit) this branch handles. Bound leaves generous
        // headroom for slower CI hardware while still catching a
        // regression back to the O(n * surplus) form.
        let n = 20_000;
        let mut counts = vec![1u32; n];
        counts[0] = 1_000_000;
        let table_log2 = 15;
        let start = std::time::Instant::now();
        let freq = normalize_frequencies(&counts, table_log2);
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_millis(200),
            "n={n} with one dominant outlier took {elapsed:?}, expected well under 200ms; \
             likely a regression to the round-robin O(n * surplus) surplus-removal form"
        );
        let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
        assert_eq!(sum, 1u64 << table_log2);
        assert!(freq.iter().all(|&f| f > 0));
    }

    #[test]
    fn large_counts_do_not_overflow() {
        let counts = [u32::MAX, u32::MAX / 2, 1];
        let freq = normalize_frequencies(&counts, 12);
        let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
        assert_eq!(sum, 1u64 << 12);
        assert!(freq.iter().all(|&f| f > 0));
    }

    #[test]
    #[should_panic(expected = "must fit a u32 shift")]
    fn spread_table_log2_of_32_panics() {
        let _ = spread_symbols(&[1, 1], 32);
    }

    #[test]
    #[should_panic(expected = "freq must sum to exactly")]
    fn spread_rejects_a_freq_that_does_not_sum_to_the_table_size() {
        let _ = spread_symbols(&[1, 1, 1], 2);
    }

    #[test]
    fn spread_matches_a_hand_computed_table() {
        // table_size=4, stride = ((4>>1)+(4>>3)+3)|1 = 5, mask=3.
        // position: 0 -[+5&3=1]-> 1 -[+5&3=2]-> 2 -[+5&3=3]-> 3 -[+5&3=0]-> 0
        assert_eq!(spread_symbols(&[2, 1, 1], 2), vec![0, 0, 1, 2]);
    }

    #[test]
    fn spread_places_every_symbol_the_right_number_of_times() {
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
            let table = spread_symbols(&freq, table_log2);
            assert_eq!(table.len(), 1usize << table_log2);
            for (symbol, &want) in freq.iter().enumerate() {
                let got = table
                    .iter()
                    .filter(|&&s| s == u32::try_from(symbol).unwrap())
                    .count();
                assert_eq!(
                    got, want as usize,
                    "symbol {symbol} placed {got} times, wanted {want}"
                );
            }
        }
    }

    #[test]
    fn spread_never_writes_the_same_slot_twice() {
        // Reimplements the position sequence independently of
        // spread_symbols to catch a bug that both places every symbol the
        // right number of times overall (the check above) and still
        // collides two placements onto the same slot: a slot's final
        // value can equal 0 (a real symbol) whether or not it was ever
        // actually visited, so an overwritten slot and an untouched one
        // can silently balance each other's count in that check alone.
        let counts = [37, 19, 5, 5, 200, 1, 1, 1, 1, 1];
        let table_log2 = 10;
        let freq = normalize_frequencies(&counts, table_log2);
        let table_size = 1usize << table_log2;
        let mask = table_size - 1;
        let stride = spread_stride(table_size);

        let mut seen = vec![false; table_size];
        let mut position = 0usize;
        for &count in &freq {
            for _ in 0..count {
                assert!(!seen[position], "slot {position} written twice");
                seen[position] = true;
                position = (position + stride) & mask;
            }
        }
        assert!(seen.iter().all(|&s| s), "not every slot was written");
    }

    #[test]
    fn spread_is_deterministic() {
        let counts = [37, 19, 0, 5, 5, 200, 1, 1, 1, 1, 1];
        let freq = normalize_frequencies(&counts, 10);
        let first = spread_symbols(&freq, 10);
        let second = spread_symbols(&freq, 10);
        assert_eq!(first, second);
    }

    #[test]
    fn spread_handles_a_single_slot_table() {
        assert_eq!(spread_symbols(&[1], 0), vec![0]);
    }

    #[test]
    fn spread_handles_table_size_eight_where_the_bare_constant_is_even() {
        // stride's magnitude alone ((8>>1)+(8>>3)+3 = 8) is even and would
        // divide table_size=8, collapsing every placement onto slot 0;
        // spread_stride's `| 1` is what keeps this case correct.
        let freq = normalize_frequencies(&[5, 2, 1], 3);
        let table = spread_symbols(&freq, 3);
        assert_eq!(table.len(), 8);
        for (symbol, &want) in freq.iter().enumerate() {
            let got = table
                .iter()
                .filter(|&&s| s == u32::try_from(symbol).unwrap())
                .count();
            assert_eq!(got, want as usize);
        }
    }
}
