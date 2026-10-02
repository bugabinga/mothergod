use proptest::prelude::*;

use super::tests::roundtrip_symbols;

/// Alphabet size 2..64 (every hand-written example above falls in this
/// range), symbol stream length 0..300 (`Model::decode`'s scan is
/// `O(alphabet)` per symbol, so this stays cheap at proptest's default
/// case count without needing a `PROPTEST_CASES`-scaled profile the way
/// `lib.rs`'s heavier `compress`-driven property does).
fn symbols_and_alphabet() -> impl Strategy<Value = (usize, Vec<usize>)> {
    (2usize..64).prop_flat_map(|alphabet| {
        proptest::collection::vec(0..alphabet, 0..300).prop_map(move |symbols| (alphabet, symbols))
    })
}

proptest! {
    /// Every symbol decoded back matches what was encoded, swept over
    /// arbitrary alphabets and streams instead of one example per shape
    /// (mirrors `mod tests`' `roundtrip_symbols`-based examples).
    #[test]
    fn roundtrip_holds_for_arbitrary_symbol_streams((alphabet, symbols) in symbols_and_alphabet()) {
        roundtrip_symbols(&symbols, alphabet);
    }
}
