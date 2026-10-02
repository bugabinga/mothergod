use proptest::prelude::*;

use super::{
    build_decode_table, build_encode_table, decode_message, encode_message, normalize_frequencies,
    read_freq_table, spread_symbols, write_freq_table,
};

/// Every originally-nonzero symbol stays nonzero
/// ([`normalize_frequencies`]'s own guarantee) and every count here
/// is drawn `>= 1`, so the whole `0..alphabet` range is always a
/// valid, encodable symbol -- no filtering needed before generating
/// symbol sequences over it. `table_log2` is picked before the
/// alphabet and caps it at `1 << table_log2`, [`normalize_frequencies`]'s
/// own precondition (one table slot per distinct symbol, at least).
fn freq_table_log2_and_symbols() -> impl Strategy<Value = (Vec<u32>, u32, Vec<u32>)> {
    (3u32..10)
        .prop_flat_map(|table_log2| {
            let max_alphabet = (1usize << table_log2).min(12);
            (
                prop::collection::vec(1u32..500, 2..=max_alphabet),
                Just(table_log2),
            )
        })
        .prop_flat_map(|(counts, table_log2)| {
            let freq = normalize_frequencies(&counts, table_log2);
            let alphabet = freq.len();
            prop::collection::vec(0..u32::try_from(alphabet).unwrap(), 0..64)
                .prop_map(move |symbols| (freq.clone(), table_log2, symbols))
        })
}

proptest! {
    /// Every symbol [`encode_message`] packs, [`decode_message`]
    /// recovers exactly, over arbitrary alphabets, table sizes, and
    /// symbol streams -- the property that matters about this
    /// slice, mirroring `mod tests`' own hand-picked examples.
    #[test]
    fn encode_decode_message_round_trips_for_arbitrary_symbol_sequences(
        (freq, table_log2, symbols) in freq_table_log2_and_symbols()
    ) {
        let spread = spread_symbols(&freq, table_log2);
        let decode = build_decode_table(&spread, &freq, table_log2);
        let encode = build_encode_table(&spread, &freq, table_log2);

        let (bytes, initial_state) = encode_message(&symbols, &encode, &decode);
        let decoded = decode_message(&bytes, initial_state, symbols.len(), &decode);
        prop_assert_eq!(decoded, symbols);
    }

    /// [`read_freq_table`] recovers exactly what [`write_freq_table`]
    /// wrote, over arbitrary alphabets and table sizes, and leaves
    /// nothing unconsumed when nothing followed the table -- the
    /// serialization counterpart of this module's own round-trip
    /// property above.
    #[test]
    fn write_then_read_freq_table_round_trips_for_arbitrary_tables(
        (freq, table_log2, _symbols) in freq_table_log2_and_symbols()
    ) {
        let bytes = write_freq_table(&freq, table_log2);
        let (read_log2, read_freq, rest) = read_freq_table(&bytes).unwrap();
        prop_assert_eq!(read_log2, table_log2);
        prop_assert_eq!(read_freq, freq);
        prop_assert!(rest.is_empty());
    }
}
