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

#[test]
#[should_panic(expected = "must fit a u32 shift")]
fn decode_table_log2_of_32_panics() {
    let _ = build_decode_table(&[0], &[1], 32);
}

#[test]
#[should_panic(expected = "freq must sum to exactly")]
fn decode_table_rejects_a_freq_that_does_not_sum_to_the_table_size() {
    let _ = build_decode_table(&[0, 0, 1], &[1, 1, 1], 2);
}

#[test]
#[should_panic(expected = "table_symbol must have exactly")]
fn decode_table_rejects_a_mismatched_table_symbol_length() {
    let _ = build_decode_table(&[0, 0, 1], &[2, 1, 1], 2);
}

#[test]
#[should_panic(expected = "is not a valid index into freq")]
fn decode_table_rejects_an_out_of_range_symbol() {
    let _ = build_decode_table(&[0, 0, 1, 5], &[2, 1, 1], 2);
}

#[test]
fn decode_table_matches_a_hand_computed_example() {
    // freq=[2,1,1], table_log2=2 (table_size=4), spread table [0,0,1,2]
    // (spread_matches_a_hand_computed_table's own example). symbolNext
    // starts [2,1,1]:
    //   u=0: s=0, next_state=2 -> highbit=1, nb_bits=1, base=(2<<1)-4=0
    //   u=1: s=0, next_state=3 -> highbit=1, nb_bits=1, base=(3<<1)-4=2
    //   u=2: s=1, next_state=1 -> highbit=0, nb_bits=2, base=(1<<2)-4=0
    //   u=3: s=2, next_state=1 -> highbit=0, nb_bits=2, base=(1<<2)-4=0
    let table = build_decode_table(&[0, 0, 1, 2], &[2, 1, 1], 2);
    assert_eq!(
        table,
        vec![
            DecodeSlot {
                symbol: 0,
                nb_bits: 1,
                new_state_base: 0
            },
            DecodeSlot {
                symbol: 0,
                nb_bits: 1,
                new_state_base: 2
            },
            DecodeSlot {
                symbol: 1,
                nb_bits: 2,
                new_state_base: 0
            },
            DecodeSlot {
                symbol: 2,
                nb_bits: 2,
                new_state_base: 0
            },
        ]
    );
}

#[test]
fn decode_table_symbol_field_matches_the_spread_table_exactly() {
    let counts = [37, 19, 0, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    let symbols: Vec<u32> = decode.iter().map(|slot| slot.symbol).collect();
    assert_eq!(symbols, spread);
}

#[test]
fn decode_table_bit_counts_and_bases_stay_in_range() {
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
        let spread = spread_symbols(&freq, table_log2);
        let decode = build_decode_table(&spread, &freq, table_log2);
        let table_size = 1u32 << table_log2;
        for slot in &decode {
            assert!(
                slot.nb_bits <= table_log2,
                "nb_bits {} exceeds table_log2 {table_log2}",
                slot.nb_bits
            );
            assert!(
                slot.new_state_base < table_size,
                "new_state_base {} not below table_size {table_size}",
                slot.new_state_base
            );
        }
    }
}

#[test]
fn decode_table_per_symbol_occurrence_numbers_are_consecutive_from_freq() {
    // Cross-checks the state-numbering scheme by recomputing each
    // occurrence's expected (nb_bits, new_state_base) from its
    // position among that symbol's own occurrences (occurrence n of a
    // symbol starting at freq[s] means next_state = freq[s] + n)
    // rather than reusing build_decode_table's running counter.
    let counts = [37, 19, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    let table_size: u64 = 1 << table_log2;

    let mut occurrence = vec![0u32; freq.len()];
    for (slot, &symbol) in decode.iter().zip(spread.iter()) {
        let n = occurrence[symbol as usize];
        occurrence[symbol as usize] += 1;
        let next_state = freq[symbol as usize] + n;
        let highbit = next_state.ilog2();
        let expected_nb_bits = table_log2 - highbit;
        let expected_base = (u64::from(next_state) << expected_nb_bits) - table_size;
        assert_eq!(slot.nb_bits, expected_nb_bits);
        assert_eq!(u64::from(slot.new_state_base), expected_base);
    }
}

#[test]
fn decode_table_is_deterministic() {
    let counts = [37, 19, 0, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let first = build_decode_table(&spread, &freq, table_log2);
    let second = build_decode_table(&spread, &freq, table_log2);
    assert_eq!(first, second);
}

#[test]
#[should_panic(expected = "must fit a u32 shift")]
fn encode_table_log2_of_32_panics() {
    let _ = build_encode_table(&[0], &[1], 32);
}

#[test]
#[should_panic(expected = "freq must sum to exactly")]
fn encode_table_rejects_a_freq_that_does_not_sum_to_the_table_size() {
    let _ = build_encode_table(&[0, 0, 1], &[1, 1, 1], 2);
}

#[test]
#[should_panic(expected = "table_symbol must have exactly")]
fn encode_table_rejects_a_mismatched_table_symbol_length() {
    let _ = build_encode_table(&[0, 0, 1], &[2, 1, 1], 2);
}

#[test]
#[should_panic(expected = "is not a valid index into freq")]
fn encode_table_rejects_an_out_of_range_symbol() {
    let _ = build_encode_table(&[0, 0, 1, 5], &[2, 1, 1], 2);
}

#[test]
fn encode_table_matches_a_hand_computed_example() {
    // freq=[2,1,1], table_log2=2, spread table [0,0,1,2]
    // (spread_matches_a_hand_computed_table's own example): symbol 0
    // occupies slots 0 and 1, symbol 1 occupies slot 2, symbol 2
    // occupies slot 3.
    let table = build_encode_table(&[0, 0, 1, 2], &[2, 1, 1], 2);
    assert_eq!(table, vec![vec![0, 1], vec![2], vec![3]]);
}

#[test]
fn encode_table_lengths_match_freq() {
    let counts = [37, 19, 0, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);
    assert_eq!(encode.len(), freq.len());
    for (symbol, entries) in encode.iter().enumerate() {
        assert_eq!(
            entries.len(),
            freq[symbol] as usize,
            "symbol {symbol} has {} encode entries, wanted freq {}",
            entries.len(),
            freq[symbol]
        );
    }
}

#[test]
fn encode_table_entries_are_sorted_ascending_by_slot() {
    // The single pass over table_symbol in slot order guarantees each
    // symbol's entries come out already sorted; an out-of-order entry
    // would mean two occurrences got swapped relative to their actual
    // slot positions.
    let counts = [37, 19, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);
    for entries in &encode {
        assert!(entries.windows(2).all(|w| w[0] < w[1]));
    }
}

#[test]
fn encode_table_is_the_exact_inverse_of_the_spread_table() {
    // Cross-check independent of build_encode_table's own scan: every
    // slot in the spread table for symbol s, collected in ascending
    // slot order, must equal that symbol's encode-table row exactly.
    let counts = [37, 19, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);
    for (symbol, entries) in encode.iter().enumerate() {
        let want: Vec<u32> = spread
            .iter()
            .enumerate()
            .filter(|&(_, &s)| s as usize == symbol)
            .map(|(p, _)| u32::try_from(p).unwrap())
            .collect();
        assert_eq!(*entries, want, "symbol {symbol}");
    }
}

#[test]
fn encode_table_round_trips_through_the_decode_table() {
    // The real property that matters: every (symbol, occurrence) the
    // encode table names, handed to the decode table by slot, decodes
    // back to that exact symbol.
    let counts = [37, 19, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);
    for (symbol, entries) in encode.iter().enumerate() {
        for &slot in entries {
            assert_eq!(
                decode[slot as usize].symbol as usize, symbol,
                "encode table for symbol {symbol} points at slot {slot}, \
                 which decodes to a different symbol"
            );
        }
    }
}

#[test]
fn encode_table_is_deterministic() {
    let counts = [37, 19, 0, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 10;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let first = build_encode_table(&spread, &freq, table_log2);
    let second = build_encode_table(&spread, &freq, table_log2);
    assert_eq!(first, second);
}

#[test]
fn encode_table_handles_a_zero_count_symbol() {
    // A symbol with freq 0 (never occurred) gets an empty encode row,
    // not a missing one or a panic.
    let freq = normalize_frequencies(&[5, 0, 3], 4);
    let spread = spread_symbols(&freq, 4);
    let encode = build_encode_table(&spread, &freq, 4);
    assert_eq!(encode.len(), 3);
    assert_eq!(encode[1], [] as [u32; 0]);
}

#[test]
fn bit_writer_reader_round_trip_arbitrary_widths() {
    let fields: &[(u32, u32)] = &[
        (0b1, 1),
        (0b101, 3),
        (0, 0),
        (0xff, 8),
        (0x1234_5678, 32),
        (0b11, 2),
        (0, 5),
        (0x7fff_ffff, 31),
    ];
    let mut writer = BitWriter::new();
    for &(value, nb_bits) in fields {
        writer.write(value, nb_bits);
    }
    let bytes = writer.finish();
    let mut reader = BitReader::new(&bytes);
    for &(value, nb_bits) in fields {
        let mask = if nb_bits == 32 {
            u32::MAX
        } else {
            (1u32 << nb_bits) - 1
        };
        assert_eq!(reader.read(nb_bits), value & mask, "nb_bits={nb_bits}");
    }
}

#[test]
#[should_panic(expected = "buffer ran out")]
fn bit_reader_panics_past_the_end_of_the_buffer() {
    let mut reader = BitReader::new(&[0xff]);
    assert_eq!(reader.read(8), 0xff);
    let _ = reader.read(1);
}

#[test]
#[should_panic(expected = "is not a valid index into encode_table")]
fn encode_symbol_rejects_an_out_of_range_symbol() {
    let freq = normalize_frequencies(&[2, 1, 1], 2);
    let spread = spread_symbols(&freq, 2);
    let encode = build_encode_table(&spread, &freq, 2);
    let decode = build_decode_table(&spread, &freq, 2);
    let _ = encode_symbol(0, 5, &encode, &decode);
}

#[test]
#[should_panic(expected = "never occurred (freq 0)")]
fn encode_symbol_rejects_a_zero_frequency_symbol() {
    let freq = normalize_frequencies(&[5, 0, 3], 4);
    let spread = spread_symbols(&freq, 4);
    let encode = build_encode_table(&spread, &freq, 4);
    let decode = build_decode_table(&spread, &freq, 4);
    let _ = encode_symbol(0, 1, &encode, &decode);
}

#[test]
fn encode_symbol_matches_a_hand_computed_example() {
    // freq=[2,1,1], table_log2=2, spread [0,0,1,2]
    // (spread_matches_a_hand_computed_table's example), decode table
    // (decode_table_matches_a_hand_computed_example's example):
    //   slot0: symbol0, nb_bits=1, base=0 (covers target 0..2)
    //   slot1: symbol0, nb_bits=1, base=2 (covers target 2..4)
    //   slot2: symbol1, nb_bits=2, base=0 (covers target 0..4)
    //   slot3: symbol2, nb_bits=2, base=0 (covers target 0..4)
    let freq = normalize_frequencies(&[2, 1, 1], 2);
    assert_eq!(freq, vec![2, 1, 1]);
    let spread = spread_symbols(&freq, 2);
    let encode = build_encode_table(&spread, &freq, 2);
    let decode = build_decode_table(&spread, &freq, 2);

    // Symbol 0, target_state 1: covered by slot0's [0,2) range.
    let step = encode_symbol(1, 0, &encode, &decode);
    assert_eq!(
        step,
        Encoded {
            nb_bits: 1,
            bits: 1,
            state: 0
        }
    );
    // Symbol 0, target_state 3: covered by slot1's [2,4) range.
    let step = encode_symbol(3, 0, &encode, &decode);
    assert_eq!(
        step,
        Encoded {
            nb_bits: 1,
            bits: 1,
            state: 1
        }
    );
    // Symbol 1, any target_state in [0,4): only slot2.
    let step = encode_symbol(3, 1, &encode, &decode);
    assert_eq!(
        step,
        Encoded {
            nb_bits: 2,
            bits: 3,
            state: 2
        }
    );
}

#[test]
fn encode_symbol_is_the_exact_inverse_of_a_decode_step() {
    // For every slot in the decode table, and every bits value that
    // slot's nb_bits admits, decoding from that slot reaches some
    // target_state; encoding that symbol against that target_state
    // must recover the original slot, bits, and nb_bits exactly.
    let counts = [37, 19, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 8;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);

    for (slot, info) in decode.iter().enumerate() {
        let slot = u32::try_from(slot).unwrap();
        let width = 1u32 << info.nb_bits;
        for bits in 0..width {
            let target_state = info.new_state_base + bits;
            let step = encode_symbol(target_state, info.symbol, &encode, &decode);
            assert_eq!(step.state, slot, "slot={slot} bits={bits}");
            assert_eq!(step.bits, bits, "slot={slot} bits={bits}");
            assert_eq!(step.nb_bits, info.nb_bits, "slot={slot} bits={bits}");
        }
    }
}

#[test]
fn encode_message_decode_message_round_trip() {
    let counts = [37, 19, 5, 5, 200, 1, 1, 1, 1, 1];
    let table_log2 = 8;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);

    let symbols: Vec<u32> = vec![0, 4, 4, 4, 1, 0, 0, 4, 2, 3, 0, 4, 5, 6, 7, 8, 9, 4, 0];
    let (bytes, initial_state) = encode_message(&symbols, &encode, &decode);
    let decoded = decode_message(&bytes, initial_state, symbols.len(), &decode);
    assert_eq!(decoded, symbols);
}

#[test]
fn encode_message_decode_message_round_trip_single_symbol_table() {
    // A one-symbol alphabet needs 0 bits per decode; the round trip
    // should still hold with an all-empty-looking bitstream.
    let table_log2 = 4;
    let freq = vec![1u32 << table_log2];
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);

    let symbols = vec![0u32; 25];
    let (bytes, initial_state) = encode_message(&symbols, &encode, &decode);
    assert_eq!(bytes, [] as [u8; 0]);
    let decoded = decode_message(&bytes, initial_state, symbols.len(), &decode);
    assert_eq!(decoded, symbols);
}

#[test]
fn encode_message_decode_message_round_trip_empty_message() {
    let counts = [3, 1, 1];
    let table_log2 = 2;
    let freq = normalize_frequencies(&counts, table_log2);
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    let encode = build_encode_table(&spread, &freq, table_log2);

    let (bytes, initial_state) = encode_message(&[], &encode, &decode);
    assert_eq!(bytes, [] as [u8; 0]);
    assert_eq!(initial_state, 0);
    let decoded = decode_message(&bytes, initial_state, 0, &decode);
    assert_eq!(decoded, [] as [u32; 0]);
}

#[test]
#[should_panic(expected = "is not a valid index into a decode_table")]
fn decode_message_rejects_an_out_of_range_initial_state() {
    let freq = normalize_frequencies(&[2, 1, 1], 2);
    let spread = spread_symbols(&freq, 2);
    let decode = build_decode_table(&spread, &freq, 2);
    let _ = decode_message(&[], 4, 0, &decode);
}

#[test]
fn single_symbol_whole_table_needs_zero_bits_and_is_an_identity_map() {
    // A source with only one possible symbol needs 0 bits per decode
    // (probability 1), and there is really only one conceptual state,
    // so every slot maps straight back to its own index.
    let table_log2 = 4;
    let table_size = 1usize << table_log2;
    let freq = vec![u32::try_from(table_size).unwrap()];
    let spread = spread_symbols(&freq, table_log2);
    let decode = build_decode_table(&spread, &freq, table_log2);
    for (u, slot) in decode.iter().enumerate() {
        assert_eq!(slot.symbol, 0);
        assert_eq!(slot.nb_bits, 0);
        assert_eq!(slot.new_state_base, u32::try_from(u).unwrap());
    }
}

#[test]
fn varint_round_trips_for_representative_values() {
    for value in [
        0,
        1,
        0x7F,   // largest single-byte value
        0x80,   // smallest two-byte value
        0x3FFF, // largest two-byte value
        0x4000, // smallest three-byte value
        u32::from(u16::MAX),
        u32::MAX / 2,
        u32::MAX,
    ] {
        let mut bytes = Vec::new();
        write_varint_u32(value, &mut bytes);
        let (read, rest) = read_varint_u32(&bytes).unwrap();
        assert_eq!(read, value, "value={value}");
        assert_eq!(rest, []);
    }
}

#[test]
fn read_varint_u32_reads_only_its_own_bytes_and_leaves_the_rest() {
    let mut bytes = Vec::new();
    write_varint_u32(300, &mut bytes);
    bytes.extend_from_slice(&[1, 2, 3]);
    let (value, rest) = read_varint_u32(&bytes).unwrap();
    assert_eq!(value, 300);
    assert_eq!(rest, [1, 2, 3]);
}

#[test]
fn read_varint_u32_rejects_a_sixth_continuation_byte() {
    // 5 bytes cover every u32 (5*7=35 >= 32 bits); a 6th still
    // requesting more is malformed, not merely a larger value.
    let bytes = [0x80, 0x80, 0x80, 0x80, 0x80, 0x00];
    assert_eq!(read_varint_u32(&bytes), Err(crate::Error::Corrupt));
}

#[test]
fn read_varint_u32_rejects_a_value_too_wide_for_u32() {
    // 5 bytes, each contributing bits, whose combined value exceeds
    // u32::MAX (the high nibble of the 5th byte pushes past bit 31).
    let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0x1F];
    assert_eq!(read_varint_u32(&bytes), Err(crate::Error::Corrupt));
}

#[test]
fn read_varint_u32_rejects_truncated_input() {
    assert_eq!(read_varint_u32(&[]), Err(crate::Error::Truncated));
    // Continuation bit set, then nothing: never terminates.
    assert_eq!(read_varint_u32(&[0x80]), Err(crate::Error::Truncated));
}

#[test]
fn write_then_read_freq_table_round_trips() {
    let cases: &[(&[u32], u32)] = &[
        (&[1, 1, 1], 2),
        (&[100, 1, 1], 3),
        (&[7, 5, 3, 1], 5),
        (&[255, 254, 253, 1], 10),
    ];
    for &(counts, table_log2) in cases {
        let freq = normalize_frequencies(counts, table_log2);
        let bytes = write_freq_table(&freq, table_log2);
        let (read_log2, read_freq, rest) = read_freq_table(&bytes).unwrap();
        assert_eq!(read_log2, table_log2, "counts={counts:?}");
        assert_eq!(read_freq, freq, "counts={counts:?}");
        assert!(rest.is_empty(), "counts={counts:?}");
    }
}

#[test]
fn read_freq_table_leaves_trailing_bytes_for_the_caller() {
    let freq = normalize_frequencies(&[3, 1], 2);
    let mut bytes = write_freq_table(&freq, 2);
    bytes.extend_from_slice(&[0xAB, 0xCD]);
    let (_, _, rest) = read_freq_table(&bytes).unwrap();
    assert_eq!(rest, [0xAB, 0xCD]);
}

#[test]
fn read_freq_table_rejects_table_log2_above_the_cap() {
    let mut bytes = Vec::new();
    write_varint_u32(MAX_TABLE_LOG2 + 1, &mut bytes);
    write_varint_u32(0, &mut bytes); // alphabet length, never reached
    assert_eq!(read_freq_table(&bytes), Err(crate::Error::Corrupt));
}

#[test]
fn read_freq_table_rejects_an_alphabet_length_past_the_remaining_bytes() {
    let mut bytes = Vec::new();
    write_varint_u32(4, &mut bytes); // table_log2
    write_varint_u32(1_000_000, &mut bytes); // far more entries than follow
    write_varint_u32(1, &mut bytes); // one lone entry, nowhere near enough
    assert_eq!(read_freq_table(&bytes), Err(crate::Error::Corrupt));
}

#[test]
fn read_freq_table_rejects_entries_that_do_not_sum_to_the_table_size() {
    let mut bytes = Vec::new();
    write_varint_u32(2, &mut bytes); // table_log2 -> table_size 4
    write_varint_u32(3, &mut bytes); // alphabet length
    for entry in [1, 1, 1] {
        // sums to 3, not the required 4
        write_varint_u32(entry, &mut bytes);
    }
    assert_eq!(read_freq_table(&bytes), Err(crate::Error::Corrupt));
}

#[test]
fn read_freq_table_rejects_every_truncation_without_panicking() {
    // Whether a given cut lands mid-varint (Truncated) or leaves a
    // declared length with nothing behind it (Corrupt) depends on
    // exactly where it falls; either is an acceptable, non-panicking
    // rejection (hard rule 2), so this only pins "never Ok, never a
    // panic" rather than which variant.
    let freq = normalize_frequencies(&[3, 1], 2);
    let bytes = write_freq_table(&freq, 2);
    for cut in 0..bytes.len() {
        assert!(read_freq_table(&bytes[..cut]).is_err(), "cut={cut}");
    }
}
