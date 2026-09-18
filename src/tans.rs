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
//! **Third slice (S2-A82).** [`spread_symbols`]: assigns each of the
//! `1 << table_log2` table slots to exactly one symbol -- the "spread"
//! step -- scattering symbol `s` across exactly `freq[s]` slots by a fixed
//! odd stride (coprime to the power-of-two table size, so its orbit
//! covers every slot exactly once) instead of packing them contiguously,
//! so a table walked in slot order interleaves symbols close to their
//! frequency order rather than running one symbol at a time.
//!
//! **This slice.** [`build_decode_table`] turns a spread assignment into
//! the entries a real tANS decoder indexes by state: for each table slot,
//! which symbol it decodes to, how many bits to pull off the bitstream,
//! and the baseline those bits are added to for the next state (itself a
//! valid index back into this same table). Each symbol's occurrences,
//! visited in slot order, are numbered consecutively starting at that
//! symbol's own normalized frequency -- the state range a canonical tANS
//! table reserves for it -- and a given occurrence number's bit count and
//! baseline fall straight out of that number's highest set bit
//! (`FSE_buildDTable`'s construction; no archive precedent, same check
//! the two slices before it ran). Standalone and not yet called from
//! anywhere: nothing yet builds the matching encode table or reads/writes
//! real bits against this one.
//!
//! **This slice (S2-A84).** [`build_encode_table`]: the mirroring
//! encode-table construction, [`build_next_state_table`] and
//! [`build_encode_transforms`]'s exact inverse of [`build_decode_table`].
//! The encode register lives in `[table_size, 2 * table_size)` throughout
//! encoding rather than `[0, table_size)` (ANS's own doubling-range
//! invariant), so this is not a mechanical transpose of the decode
//! construction; it is derived and cross-checked against
//! [`build_decode_table`]'s own already-tested formulas rather than
//! transcribed from any reference, and
//! `encode_table_inverts_decode_table_for_every_register_value` proves the
//! two sides agree over every reachable register value, not just a
//! hand-worked example.
//!
//! **Remaining S1-P6 scope.** The coder's actual read/write state machine
//! over both tables, wiring it behind a new fast `Method` variant, and the
//! `FORMAT_VERSION` bump and real-bitstream measurement that wiring needs.

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

/// `floor(log2(x))`, the position of `x`'s highest set bit (`0` for
/// `x == 1`). Every call site in this module passes a per-symbol running
/// state that starts at that symbol's own normalized frequency
/// ([`normalize_frequencies`]'s postcondition: at least 1 for any symbol
/// [`spread_symbols`] ever actually places) and only grows from there, so
/// `x` is never 0 in practice.
fn highbit32(x: u32) -> u32 {
    debug_assert!(
        x > 0,
        "highbit32 is only ever called on a live tANS state, never 0"
    );
    x.ilog2()
}

/// One tANS decode-table slot: the symbol table state `u` decodes to, how
/// many bits the decoder reads off the bitstream from that state, and the
/// baseline those bits are added to for the next state -- itself a valid
/// index back into the same table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeSlot {
    /// The symbol this table state decodes to.
    pub symbol: u32,
    /// How many bits the decoder reads off the bitstream from this state.
    pub nb_bits: u32,
    /// Added to the bits just read to produce the next state.
    pub new_state_base: u32,
}

/// Builds the tANS decode table a real decoder would index by state:
/// slot `u`'s entry names which symbol state `u` decodes to, how many
/// bits to read next, and the baseline (`new_state_base`) those bits are
/// added to for the next state.
///
/// `table_symbol` is [`spread_symbols`]'s output (slot `u` holds the
/// symbol occupying it) and `freq` is the normalized frequency table
/// ([`normalize_frequencies`]'s output) it was spread from. Each symbol
/// `s`'s occurrences in `table_symbol`, visited in slot order, are
/// numbered consecutively starting at `freq[s]` (the state range a
/// canonical tANS table reserves for `s`): occurrence number `n`'s
/// `nb_bits` is `table_log2` minus `n`'s highest set bit, and its
/// `new_state_base` is `n` shifted left by that many bits with
/// `1 << table_log2` subtracted back off (`FSE_buildDTable`'s
/// construction; no archive precedent, same check [`spread_symbols`] and
/// [`normalize_frequencies`] ran).
///
/// # Panics
///
/// Panics if `table_log2 >= 32`, or if `freq` does not sum to exactly
/// `1 << table_log2` (same two preconditions [`spread_symbols`]
/// enforces). Panics if `table_symbol.len()` is not `1 << table_log2`, or
/// if any of its entries is not a valid index into `freq` -- both are
/// guaranteed when `table_symbol` is `spread_symbols`'s own output for
/// this exact `freq`, the only supported caller shape; this primitive is
/// not yet reachable from any decode path.
#[must_use]
pub fn build_decode_table(table_symbol: &[u32], freq: &[u32], table_log2: u32) -> Vec<DecodeSlot> {
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
    assert!(
        table_symbol.len() == table_size,
        "table_symbol must have exactly 1 << table_log2 ({table_size}) entries; got {}",
        table_symbol.len()
    );
    let table_size_u32 =
        u32::try_from(table_size).expect("table_log2 < 32 keeps table_size within u32");
    let table_size_u64 = u64::from(table_size_u32);

    let mut symbol_next: Vec<u32> = freq.to_vec();
    table_symbol
        .iter()
        .map(|&symbol| {
            let idx = symbol as usize;
            assert!(
                idx < symbol_next.len(),
                "table_symbol entry {symbol} is not a valid index into freq (len {})",
                symbol_next.len()
            );
            let next_state = symbol_next[idx];
            symbol_next[idx] += 1;
            let nb_bits = table_log2 - highbit32(next_state);
            let new_state_base =
                u32::try_from((u64::from(next_state) << nb_bits) - table_size_u64).expect(
                    "new_state_base lands in [0, table_size), which fits u32 whenever table_size does",
                );
            DecodeSlot {
                symbol,
                nb_bits,
                new_state_base,
            }
        })
        .collect()
}

/// One symbol's tANS encode-side transform: which of two adjacent bit
/// counts the current encode register needs to emit this symbol, and
/// where the resulting state lands in [`EncodeTable::next_state`].
///
/// The encode register lives in `[table_size, 2 * table_size)` throughout
/// encoding -- ANS's own doubling-range invariant -- rather than in
/// `[0, table_size)`, the range [`build_decode_table`] keeps its decode
/// state in: the leading bit that wider range always carries is exactly
/// the bit [`build_decode_table`]'s `new_state_base` computation strips
/// back off, so a value just decoded can be re-encoded (`table_size +`
/// the decode state) with no separate bookkeeping for how many bits it
/// took to arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodeTransform {
    /// Bits this symbol takes when the register is below
    /// `min_state_plus`; `max_bits_out + 1` bits at or above it. Named
    /// after FSE's own construction (`FSE_buildCTable`): the smaller of
    /// the two adjacent bit counts a frequency that is not a power of two
    /// needs, one for most of the register's range and one bit wider for
    /// the rest. The sole count for every register value when this
    /// symbol's frequency is 0, 1, or a power of two, in which case
    /// `min_state_plus` is unreachable.
    pub max_bits_out: u32,
    /// Register threshold: below it this symbol takes `max_bits_out`
    /// bits, at or above it `max_bits_out + 1`. `u32::MAX` when a single
    /// bit count covers the whole register range, which no valid
    /// register value ever reaches.
    pub min_state_plus: u32,
    /// Added to `register >> nb_bits_out` to index
    /// [`EncodeTable::next_state`] and find the state after encoding this
    /// symbol. Negative whenever this symbol's own frequency exceeds the
    /// combined frequency of every symbol before it, which is common for
    /// the first frequent symbol in the alphabet.
    pub delta_find_state: i64,
}

/// [`EncodeTransform`] for one symbol of the given `freq`, given `total`,
/// the sum of every preceding symbol's frequency.
///
/// Frequency 0 or 1 take the sole-bit-count form directly: a symbol that
/// never occurs is never encoded (`max_bits_out`/`min_state_plus` are
/// placeholders no valid call site reaches), and a symbol occurring
/// exactly once has exactly one occurrence number (`freq` itself,
/// [`build_decode_table`]'s own numbering), always
/// `table_log2 - highbit32(freq)` bits (`highbit32(1) == 0`, so exactly
/// `table_log2`). Any other frequency has two adjacent bit counts:
/// `max_bits_out = table_log2 - 1 - highbit32(freq - 1)` is
/// [`build_decode_table`]'s own per-occurrence bit count
/// (`table_log2 - highbit32(next_state)`) at the occurrence numbers large
/// enough for a power-of-two-or-above `next_state`, one bit more below
/// that threshold; `min_state_plus = freq << (max_bits_out + 1)` is that
/// threshold carried into register space (derived from, and verified
/// against, [`build_decode_table`]'s own numbering by
/// `encode_table_inverts_decode_table_for_every_register_value` below,
/// not transcribed from any reference).
fn encode_transform_for(freq: u32, table_log2: u32, total: i64) -> EncodeTransform {
    match freq {
        0 => EncodeTransform {
            max_bits_out: table_log2,
            min_state_plus: u32::MAX,
            delta_find_state: 0,
        },
        1 => EncodeTransform {
            max_bits_out: table_log2,
            min_state_plus: u32::MAX,
            delta_find_state: total - 1,
        },
        f => {
            let max_bits_out = table_log2 - 1 - highbit32(f - 1);
            let min_state_plus = f << (max_bits_out + 1);
            EncodeTransform {
                max_bits_out,
                min_state_plus,
                delta_find_state: total - i64::from(f),
            }
        }
    }
}

/// Builds one [`EncodeTransform`] per symbol from a normalized frequency
/// table ([`normalize_frequencies`]'s postcondition: entries summing to
/// `1 << table_log2`).
///
/// # Panics
///
/// Panics if `table_log2 >= 31`: unlike [`normalize_frequencies`] and
/// [`spread_symbols`], this function's output describes the encode
/// register, which ranges over `[table_size, 2 * table_size)`, one bit
/// wider than `table_size` itself, so the headroom this bound leaves is
/// one bit tighter than theirs. Panics if `freq` does not sum to exactly
/// `1 << table_log2`.
#[must_use]
pub fn build_encode_transforms(freq: &[u32], table_log2: u32) -> Vec<EncodeTransform> {
    assert!(
        table_log2 < 31,
        "table_log2 must leave room for the encode register's \
         [table_size, 2 * table_size) range to fit u32; got {table_log2}"
    );
    let table_size: u64 = 1u64 << table_log2;
    let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
    assert!(
        sum == table_size,
        "freq must sum to exactly 1 << table_log2 ({table_size}); got {sum}. \
         Call normalize_frequencies first."
    );

    let mut total: i64 = 0;
    freq.iter()
        .map(|&f| {
            let transform = encode_transform_for(f, table_log2, total);
            total += i64::from(f);
            transform
        })
        .collect()
}

/// Builds the tANS "next state" table: [`EncodeTransform::delta_find_state`],
/// added to `register >> nb_bits_out`, indexes here to find the register's
/// next value.
///
/// Indexed the same way [`build_decode_table`]'s occurrence numbering
/// groups slots by symbol: entry `base[s] + n` (`base[s]` the sum of
/// `freq[0..s]`, `n` in `0..freq[s]`) holds the encode register value
/// (`table_size + slot`, already carrying the register's own leading bit)
/// for the slot [`spread_symbols`] gave that symbol's `n`-th occurrence --
/// this function's exact inverse: [`spread_symbols`] maps slot to symbol,
/// this maps (symbol, occurrence) back to slot.
///
/// `table_symbol` is [`spread_symbols`]'s output and `freq` is the
/// normalized frequency table it was spread from, the same two
/// [`build_decode_table`] takes.
///
/// # Panics
///
/// Panics if `table_log2 >= 31` (one bit tighter than
/// [`build_decode_table`]'s own `< 32`, for the same register-range reason
/// [`build_encode_transforms`] documents). Panics if `freq` does not sum
/// to exactly `1 << table_log2`, or if `table_symbol` does not have
/// exactly that many entries, or if any of its entries is not a valid
/// index into `freq` -- the same three preconditions
/// [`build_decode_table`] enforces.
#[must_use]
pub fn build_next_state_table(table_symbol: &[u32], freq: &[u32], table_log2: u32) -> Vec<u32> {
    assert!(
        table_log2 < 31,
        "table_log2 must leave room for table_size + slot to fit u32; got {table_log2}"
    );
    let table_size = 1usize << table_log2;
    let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
    assert!(
        sum == table_size as u64,
        "freq must sum to exactly 1 << table_log2 ({table_size}); got {sum}. \
         Call normalize_frequencies first."
    );
    assert!(
        table_symbol.len() == table_size,
        "table_symbol must have exactly 1 << table_log2 ({table_size}) entries; got {}",
        table_symbol.len()
    );
    let table_size_u32 =
        u32::try_from(table_size).expect("table_log2 < 31 keeps table_size within u32");

    let mut cursor: Vec<u32> = Vec::with_capacity(freq.len());
    let mut running = 0u32;
    for &f in freq {
        cursor.push(running);
        running += f;
    }

    let mut table = vec![0u32; table_size];
    for (slot, &symbol) in table_symbol.iter().enumerate() {
        let idx = symbol as usize;
        assert!(
            idx < cursor.len(),
            "table_symbol entry {symbol} is not a valid index into freq (len {})",
            cursor.len()
        );
        let write_at = cursor[idx];
        cursor[idx] += 1;
        let slot_u32 =
            u32::try_from(slot).expect("slot < table_size fits u32 whenever table_size does");
        table[write_at as usize] = table_size_u32 + slot_u32;
    }
    table
}

/// The tANS encode side, mirroring [`build_decode_table`]'s decode side:
/// [`next_state`](EncodeTable::next_state) and one [`EncodeTransform`] per
/// symbol, together enough to compute, for a symbol and the current
/// encode register, how many bits to emit and the register's next value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodeTable {
    /// See [`build_next_state_table`].
    pub next_state: Vec<u32>,
    /// One [`EncodeTransform`] per symbol, indexed by symbol id.
    pub transforms: Vec<EncodeTransform>,
}

/// Builds the complete tANS encode side from the same three inputs
/// [`build_decode_table`] takes: [`build_next_state_table`] and
/// [`build_encode_transforms`], run once each.
///
/// # Panics
///
/// Whatever either of those two panics on; see their own docs.
#[must_use]
pub fn build_encode_table(table_symbol: &[u32], freq: &[u32], table_log2: u32) -> EncodeTable {
    EncodeTable {
        next_state: build_next_state_table(table_symbol, freq, table_log2),
        transforms: build_encode_transforms(freq, table_log2),
    }
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
    #[should_panic(expected = "must leave room for the encode register's")]
    fn encode_transforms_table_log2_of_31_panics() {
        let _ = build_encode_transforms(&[1, 1], 31);
    }

    #[test]
    #[should_panic(expected = "freq must sum to exactly")]
    fn encode_transforms_rejects_a_freq_that_does_not_sum_to_the_table_size() {
        let _ = build_encode_transforms(&[1, 1, 1], 2);
    }

    #[test]
    #[should_panic(expected = "must leave room for table_size + slot")]
    fn next_state_table_log2_of_31_panics() {
        let _ = build_next_state_table(&[0], &[1], 31);
    }

    #[test]
    #[should_panic(expected = "freq must sum to exactly")]
    fn next_state_table_rejects_a_freq_that_does_not_sum_to_the_table_size() {
        let _ = build_next_state_table(&[0, 0, 1], &[1, 1, 1], 2);
    }

    #[test]
    #[should_panic(expected = "table_symbol must have exactly")]
    fn next_state_table_rejects_a_mismatched_table_symbol_length() {
        let _ = build_next_state_table(&[0, 0, 1], &[2, 1, 1], 2);
    }

    #[test]
    #[should_panic(expected = "is not a valid index into freq")]
    fn next_state_table_rejects_an_out_of_range_symbol() {
        let _ = build_next_state_table(&[0, 0, 1, 5], &[2, 1, 1], 2);
    }

    #[test]
    fn encode_transforms_matches_a_hand_computed_example() {
        // freq=[2,1,1], table_log2=2 (table_size=4): every occurrence of
        // symbol 0 (freq 2, a power of two) needs 1 bit uniformly
        // (build_decode_table's own hand-computed example above agrees:
        // both its occurrences get nb_bits=1), so max_bits_out=1 and
        // min_state_plus is unreachable... except 2 is *not* freq==1, so
        // it takes the general formula: max_bits_out = 2-1-highbit32(1) =
        // 2-1-0 = 1, min_state_plus = 2<<2 = 8 = 2*table_size, unreachable
        // by any valid register (< 2*table_size), consistent with the
        // uniform bit count a power-of-two frequency needs.
        // Symbols 1 and 2 (freq 1) take the sole-bit-count path directly:
        // max_bits_out=table_log2=2, min_state_plus=u32::MAX.
        let transforms = build_encode_transforms(&[2, 1, 1], 2);
        assert_eq!(
            transforms,
            vec![
                EncodeTransform {
                    max_bits_out: 1,
                    min_state_plus: 8,
                    delta_find_state: -2,
                },
                EncodeTransform {
                    max_bits_out: 2,
                    min_state_plus: u32::MAX,
                    delta_find_state: 1,
                },
                EncodeTransform {
                    max_bits_out: 2,
                    min_state_plus: u32::MAX,
                    delta_find_state: 2,
                },
            ]
        );
    }

    #[test]
    fn next_state_table_matches_a_hand_computed_example() {
        // table_symbol=[0,0,1,2] (spread_symbols's own hand-computed
        // example above), freq=[2,1,1], table_log2=2 (table_size=4):
        // symbol 0's two occurrences sit at slots 0 and 1, symbol 1's one
        // occurrence at slot 2, symbol 2's one occurrence at slot 3, so
        // the table (grouped by symbol, offset by table_size=4) is
        // [4,5, 6, 7] read off directly in slot order.
        let table = build_next_state_table(&[0, 0, 1, 2], &[2, 1, 1], 2);
        assert_eq!(table, vec![4, 5, 6, 7]);
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
    fn encode_table_inverts_decode_table_for_every_register_value() {
        // The real cross-check: for every table slot u (a decode state)
        // and every bit pattern decode could have read to land there,
        // reconstructing the corresponding encode register and running it
        // through EncodeTransform + EncodeTable::next_state must recover
        // exactly the same slot u, offset into register space
        // (table_size + u) -- proving build_encode_table is
        // build_decode_table's exact functional inverse over every
        // reachable register value, not just a hand-picked example.
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
            let encode = build_encode_table(&spread, &freq, table_log2);
            let table_size = 1u32 << table_log2;

            for (u, slot) in decode.iter().enumerate() {
                let u = u32::try_from(u).unwrap();
                let transform = encode.transforms[slot.symbol as usize];
                for bits in 0..(1u32 << slot.nb_bits) {
                    let register = slot.new_state_base + bits + table_size;
                    let nb_bits_out = if register < transform.min_state_plus {
                        transform.max_bits_out
                    } else {
                        transform.max_bits_out + 1
                    };
                    assert_eq!(
                        nb_bits_out, slot.nb_bits,
                        "counts={counts:?} table_log2={table_log2} u={u} bits={bits}"
                    );
                    let index = i64::from(register >> nb_bits_out) + transform.delta_find_state;
                    let index = usize::try_from(index).unwrap_or_else(|_| {
                        panic!(
                            "negative next_state index: \
                             counts={counts:?} table_log2={table_log2} u={u} bits={bits}"
                        )
                    });
                    assert_eq!(
                        encode.next_state[index],
                        table_size + u,
                        "counts={counts:?} table_log2={table_log2} u={u} bits={bits}"
                    );
                }
            }
        }
    }
}
