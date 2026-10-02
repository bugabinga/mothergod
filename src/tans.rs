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
//! **Fourth slice (S2-A83).** [`build_decode_table`] turns a spread
//! assignment into the entries a real tANS decoder indexes by state: for
//! each table slot, which symbol it decodes to, how many bits to pull off
//! the bitstream, and the baseline those bits are added to for the next
//! state (itself a valid index back into this same table). Each symbol's
//! occurrences, visited in slot order, are numbered consecutively starting
//! at that symbol's own normalized frequency -- the state range a
//! canonical tANS table reserves for it -- and a given occurrence number's
//! bit count and baseline fall straight out of that number's highest set
//! bit (`FSE_buildDTable`'s construction; no archive precedent, same check
//! the two slices before it ran).
//!
//! **Fifth slice (S2-A84).** [`build_encode_table`] inverts that same
//! per-symbol occurrence numbering: [`build_decode_table`] answers "slot
//! `p` decodes to which symbol, at which occurrence"; an encoder instead
//! already knows the symbol it is about to emit and needs the opposite
//! lookup, "symbol `s`'s occurrence `n` sits at which slot" -- the table
//! state a real tANS encoder transitions to.
//!
//! **Sixth slice (S2-A85).** [`encode_symbol`], [`encode_message`] and
//! [`decode_message`]: the coder's actual read/write state machine over
//! both tables. Decoding a slot is already fully specified by
//! [`DecodeSlot`] itself (index the table by state, read `nb_bits` bits,
//! add `new_state_base`), so [`decode_message`] applies that directly.
//! Encoding has no closed form yet ([`build_encode_table`]'s own docs
//! deferred it, "nothing yet reads it inside a hot loop to make that
//! optimization pay for itself"): [`encode_symbol`] instead searches a
//! symbol's occurrences for the one whose decode range covers the target
//! state, relying on the property that an FSE/tANS decode table's per-symbol
//! occurrence ranges partition `[0, table_size)` exactly and contiguously
//! (`decode_table_bit_counts_and_bases_stay_in_range`'s own bound is a
//! consequence of it). [`encode_message`] threads that backward over a
//! whole symbol sequence -- reverse order, the direction a stack-like ANS
//! state actually threads through -- and packs the resulting bits into a
//! real byte buffer (LSB-first, forward-readable) that [`decode_message`]
//! reads back. Both are standalone and not yet callable from any `Method`:
//! this closes the "read/write state machine" item, leaving `Method`
//! wiring and the `FORMAT_VERSION` bump as the only remaining scope.
//! S2-A86 (`bench/src/bin/tans_measure.rs`, not this module) then measured
//! that state machine for real: within noise of the order-0 entropy floor,
//! 21x-305x faster than the champion, exactly the ratio-for-speed trade a
//! fast tier needs.
//!
//! **This slice.** [`write_freq_table`] and [`read_freq_table`]: a decoder
//! has no access to the original byte counts [`normalize_frequencies`] was
//! computed from, so a real bitstream has to carry the normalized
//! frequency table itself -- the piece every slice through S2-A86 left
//! implicit by threading `freq` straight from `normalize_frequencies` into
//! `spread_symbols` in the same process. `read_freq_table` is also this
//! coder's first byte-level contact with untrusted input: it validates
//! `table_log2`, the declared alphabet length, every varint, and the
//! parsed table's sum fully before returning, so hard rule 2's
//! never-panics guarantee holds for whatever `Method` wiring calls it
//! next. Both are standalone and not yet callable from any `Method`.
//!
//! **Remaining S1-P6 scope.** Wiring this coder behind a new fast `Method`
//! variant, and the `FORMAT_VERSION` bump and real-bitstream measurement
//! that wiring needs.

/// Panics unless `table_log2` fits the `u32` shift every table operation
/// in this module uses to turn it into a table size.
fn assert_table_log2_fits_u32(table_log2: u32) {
    assert!(
        table_log2 < 32,
        "table_log2 must fit a u32 shift; got {table_log2}"
    );
}

/// Panics unless `freq` is a valid [`normalize_frequencies`] output for
/// `table_log2`: `table_log2` fits `u32`, and `freq` sums to exactly
/// `1 << table_log2`. Returns that table size, the precondition every
/// table-building step below this one shares.
fn assert_normalized_freq(freq: &[u32], table_log2: u32) -> usize {
    assert_table_log2_fits_u32(table_log2);
    let table_size = 1usize << table_log2;
    let sum: u64 = freq.iter().map(|&f| u64::from(f)).sum();
    assert!(
        sum == table_size as u64,
        "freq must sum to exactly 1 << table_log2 ({table_size}); got {sum}. \
         Call normalize_frequencies first."
    );
    table_size
}

/// Panics unless `table_symbol` has exactly `table_size` entries, the
/// shape [`spread_symbols`]'s output always has.
fn assert_table_symbol_len(table_symbol: &[u32], table_size: usize) {
    assert!(
        table_symbol.len() == table_size,
        "table_symbol must have exactly 1 << table_log2 ({table_size}) entries; got {}",
        table_symbol.len()
    );
}

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
    assert_table_log2_fits_u32(table_log2);
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

/// Upper bound [`read_freq_table`] accepts for a deserialized
/// `table_log2`. S2-A86's measurement used `table_log2 = 10`; 16 leaves
/// headroom for tuning while keeping `1 << table_log2` -- the eventual
/// decode table's slot count, once a `Method` wires this primitive to a
/// real bitstream -- capped in the tens of thousands regardless of
/// how small the actual coded payload is. Checked once, here, at the
/// single point a future decode path first parses this untrusted byte,
/// rather than leaving each downstream table-building call site to
/// remember its own cap (`rust-craft` skill, allocation-discipline; hard
/// rule 2, `CLAUDE.md`).
pub(crate) const MAX_TABLE_LOG2: u32 = 16;

/// Writes `value` as an unsigned little-endian-base-128 varint: 7 payload
/// bits per byte, continuation bit (`0x80`) set on every byte but the
/// last. Standard LEB128; this module's only use for it is
/// [`write_freq_table`]'s per-entry encoding, so it stays private.
fn write_varint_u32(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let byte = u8::try_from(value & 0x7F).expect("masked to 7 bits, always fits u8");
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// Reads one [`write_varint_u32`]-encoded value off the front of `bytes`,
/// returning it alongside whatever bytes follow it -- the same
/// value-plus-remainder shape `crate::codec`'s own header parsing already
/// returns from `split_at_checked`-based helpers.
///
/// Never panics on adversarial input: accumulates in `u64` (5 bytes * 7
/// bits = 35, safely inside `u64` regardless of which bits are set, so no
/// intermediate shift can overflow) and only converts down to `u32` once
/// a terminating byte is found, rejecting a value too wide to fit
/// ([`Error::Corrupt`]) instead of silently truncating it. Rejects a 6th
/// continuation byte ([`Error::Corrupt`]): an encoded `u32` never needs
/// one, so a stream still requesting more at that point is malformed, not
/// merely large -- otherwise a crafted input could hold this loop reading
/// forever. Returns [`Error::Truncated`] if `bytes` runs out before a
/// terminating byte appears.
fn read_varint_u32(bytes: &[u8]) -> Result<(u32, &[u8]), crate::Error> {
    let mut value: u64 = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        if i == 5 {
            return Err(crate::Error::Corrupt);
        }
        value |= u64::from(byte & 0x7F) << (7 * i);
        if byte & 0x80 == 0 {
            return u32::try_from(value)
                .map(|v| (v, &bytes[i + 1..]))
                .map_err(|_| crate::Error::Corrupt);
        }
    }
    Err(crate::Error::Truncated)
}

/// Serializes `freq` for embedding in a frame payload, the piece missing
/// between a normalized frequency table and a real bitstream: a decoder
/// has no access to the original byte counts [`normalize_frequencies`]
/// was computed from, so the table itself must ride in the compressed
/// output. Layout: `table_log2` as a varint, `freq.len()` as a varint
/// (this module's primitives never assume a 256-symbol alphabet, so
/// neither does this serialization), then each entry as a varint, in
/// order. [`read_freq_table`] reads this back, fully validating what an
/// adversarial decoder input claims instead of trusting it.
///
/// # Panics
///
/// Panics under the same precondition [`build_decode_table`] and
/// [`build_encode_table`] already share: `freq` must be
/// [`normalize_frequencies`]'s own output for `table_log2` (its entries
/// sum to exactly `1 << table_log2`). This primitive is not yet reachable
/// from any decode path -- only an encoder, which always calls it on its
/// own `normalize_frequencies` output, calls this today.
#[must_use]
pub fn write_freq_table(freq: &[u32], table_log2: u32) -> Vec<u8> {
    assert_normalized_freq(freq, table_log2);
    let mut out = Vec::new();
    write_varint_u32(table_log2, &mut out);
    let len = u32::try_from(freq.len())
        .expect("caller's own alphabet size, never large enough to overflow u32");
    write_varint_u32(len, &mut out);
    for &f in freq {
        write_varint_u32(f, &mut out);
    }
    out
}

/// Reads a [`write_freq_table`]-encoded frequency table back off `bytes`,
/// fully validating instead of trusting: this is the boundary where a
/// future `Method`'s decode path first touches untrusted bytes for this
/// coder, so every check hard rule 2 (`CLAUDE.md`) needs lives here once,
/// rather than at each downstream table-building call site.
///
/// Returns the parsed `table_log2`, the frequency table, and whatever
/// bytes came after it (the coded message itself, once a `Method` wires
/// one).
///
/// # Errors
///
/// [`Error::Truncated`] if `bytes` ends before a complete table does.
/// [`Error::Corrupt`] if: `table_log2` exceeds this module's own
/// `MAX_TABLE_LOG2` cap; the declared alphabet length exceeds the bytes
/// actually remaining (each entry needs at least one byte to encode, so
/// this also bounds the returned `Vec`'s allocation by `bytes.len()`,
/// never by the untrusted length field alone -- the same "cap a hostile
/// length field against what the input can actually supply" discipline
/// `crate::codec`'s own declared-length checks use); any varint is
/// malformed (an overlong or overflowing encoding); or the parsed entries
/// do not sum to exactly `1 << table_log2`, the invariant
/// [`normalize_frequencies`] always produces and
/// [`build_decode_table`]/[`build_encode_table`] both require.
///
/// [`Error::Truncated`]: crate::Error::Truncated
/// [`Error::Corrupt`]: crate::Error::Corrupt
pub fn read_freq_table(bytes: &[u8]) -> Result<(u32, Vec<u32>, &[u8]), crate::Error> {
    let (table_log2, rest) = read_varint_u32(bytes)?;
    if table_log2 > MAX_TABLE_LOG2 {
        return Err(crate::Error::Corrupt);
    }
    let (len, mut rest) = read_varint_u32(rest)?;
    let len = usize::try_from(len).map_err(|_| crate::Error::Corrupt)?;
    if len > rest.len() {
        return Err(crate::Error::Corrupt);
    }
    let mut freq = Vec::with_capacity(len);
    let mut sum: u64 = 0;
    for _ in 0..len {
        let (f, remaining) = read_varint_u32(rest)?;
        sum += u64::from(f);
        freq.push(f);
        rest = remaining;
    }
    if sum != 1u64 << table_log2 {
        return Err(crate::Error::Corrupt);
    }
    Ok((table_log2, freq, rest))
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
    let table_size = assert_normalized_freq(freq, table_log2);

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
    let table_size = assert_normalized_freq(freq, table_log2);
    assert_table_symbol_len(table_symbol, table_size);
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

/// Builds the tANS encode table: for symbol `s`, `table[s][n]` is the slot
/// [`build_decode_table`] assigned to `s`'s occurrence number `n` (the
/// same numbering [`build_decode_table`]'s own docs describe, consecutive
/// starting at `freq[s]`, `n` here counted from 0 rather than from
/// `freq[s]`). An encoder that has already chosen the next symbol to emit
/// and knows which occurrence of it this is looks the slot up here instead
/// of the decoder's direction (slot -> symbol).
///
/// `table_symbol` is [`spread_symbols`]'s output and `freq` is the
/// normalized frequency table it was spread from, the same two inputs
/// [`build_decode_table`] takes; this function makes one pass over
/// `table_symbol` in slot order, so `table[s]` always comes out sorted by
/// slot position ascending, matching the order symbols actually occupy
/// their occurrence range in.
///
/// # Panics
///
/// Panics if `table_log2 >= 32`, or if `freq` does not sum to exactly
/// `1 << table_log2`, or if `table_symbol.len()` is not `1 << table_log2`
/// (the same three preconditions [`build_decode_table`] enforces). Panics
/// if any of `table_symbol`'s entries is not a valid index into `freq` --
/// guaranteed when `table_symbol` is `spread_symbols`'s own output for
/// this exact `freq`, the only supported caller shape; this primitive is
/// not yet reachable from any decode path.
#[must_use]
pub fn build_encode_table(table_symbol: &[u32], freq: &[u32], table_log2: u32) -> Vec<Vec<u32>> {
    let table_size = assert_normalized_freq(freq, table_log2);
    assert_table_symbol_len(table_symbol, table_size);

    let mut table: Vec<Vec<u32>> = freq
        .iter()
        .map(|&f| Vec::with_capacity(f as usize))
        .collect();
    for (position, &symbol) in table_symbol.iter().enumerate() {
        let idx = symbol as usize;
        assert!(
            idx < table.len(),
            "table_symbol entry {symbol} is not a valid index into freq (len {})",
            table.len()
        );
        let position = u32::try_from(position).expect("table_size fits u32 since table_log2 < 32");
        table[idx].push(position);
    }
    table
}

/// Accumulates bits least-significant-bit first into a growing byte
/// buffer. Private: message-level packing is this slice's whole job, and
/// nothing outside it has any use yet for a bit-at-a-time writer.
struct BitWriter {
    bytes: Vec<u8>,
    acc: u64,
    nb_bits: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            acc: 0,
            nb_bits: 0,
        }
    }

    /// Appends the low `nb_bits` bits of `value`, least-significant-bit
    /// first. A `nb_bits` of 0 is a no-op, the shape every call in this
    /// module makes for a single-symbol table (see
    /// `single_symbol_whole_table_needs_zero_bits_and_is_an_identity_map`).
    ///
    /// # Panics
    ///
    /// Panics if `nb_bits > 32`: every field this module ever packs comes
    /// from a `table_log2 < 32` table, so `nb_bits` never legitimately
    /// exceeds 32.
    fn write(&mut self, value: u32, nb_bits: u32) {
        assert!(
            nb_bits <= 32,
            "nb_bits {nb_bits} exceeds the 32-bit fields this module ever packs"
        );
        if nb_bits == 0 {
            return;
        }
        // `nb_bits <= 32` (asserted above), so `1u64 << nb_bits` never
        // exceeds `1u64 << 32`, well within u64's 64 bits: no separate
        // nb_bits == 32 case is needed the way it would be at u32's own
        // width.
        let mask = (1u64 << nb_bits) - 1;
        self.acc |= (u64::from(value) & mask) << self.nb_bits;
        self.nb_bits += nb_bits;
        while self.nb_bits >= 8 {
            self.bytes.push((self.acc & 0xff) as u8);
            self.acc >>= 8;
            self.nb_bits -= 8;
        }
    }

    /// Flushes any partial trailing byte, zero-padded in its unused high
    /// bits, and returns the packed buffer.
    fn finish(mut self) -> Vec<u8> {
        if self.nb_bits > 0 {
            self.bytes.push((self.acc & 0xff) as u8);
        }
        self.bytes
    }
}

/// Reads bits least-significant-bit first from a byte buffer, matching
/// [`BitWriter`]'s packing order. Private, for the same reason
/// [`BitWriter`] is.
struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
    acc: u64,
    nb_bits: u32,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            acc: 0,
            nb_bits: 0,
        }
    }

    /// Reads and returns the next `nb_bits` bits, least-significant-bit
    /// first.
    ///
    /// # Panics
    ///
    /// Panics if `nb_bits > 32` (same bound [`BitWriter::write`]
    /// enforces), or if the buffer runs out before `nb_bits` bits have
    /// been supplied -- caller error (asking for more than
    /// [`BitWriter`] wrote), never a property of adversarial input: this
    /// primitive is not yet reachable from any decode path.
    fn read(&mut self, nb_bits: u32) -> u32 {
        assert!(
            nb_bits <= 32,
            "nb_bits {nb_bits} exceeds the 32-bit fields this module ever packs"
        );
        while self.nb_bits < nb_bits {
            let &byte = self.bytes.get(self.pos).unwrap_or_else(|| {
                panic!(
                    "buffer ran out after {} bytes before {nb_bits} bits were \
                     supplied; caller asked for more than BitWriter wrote",
                    self.pos
                )
            });
            self.acc |= u64::from(byte) << self.nb_bits;
            self.nb_bits += 8;
            self.pos += 1;
        }
        // Same `nb_bits <= 32` bound as `BitWriter::write`: no separate
        // nb_bits == 32 case needed against u64's own 64-bit width.
        let mask = (1u64 << nb_bits) - 1;
        let value = u32::try_from(self.acc & mask)
            .expect("mask keeps the result within nb_bits <= 32 bits, which fits u32");
        self.acc >>= nb_bits;
        self.nb_bits -= nb_bits;
        value
    }
}

/// The result of one tANS encode step: how many bits to emit and their
/// value, plus the state a decoder would have been in immediately before
/// emitting `symbol` -- what a real encoder threads backward into the
/// preceding symbol's own [`encode_symbol`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoded {
    /// How many bits to emit for this step.
    pub nb_bits: u32,
    /// The bits to emit, right-justified in the low `nb_bits` bits.
    pub bits: u32,
    /// The state a decoder would have been in immediately before this
    /// step, i.e. the state to encode the preceding symbol against.
    pub state: u32,
}

/// One tANS encode step, the inverse of a [`DecodeSlot`] lookup: given
/// `target_state` (the state a decoder would be in immediately *after*
/// decoding `symbol`) and `symbol` itself, finds the unique occurrence of
/// `symbol` whose decode range covers `target_state` and returns the bits
/// that occurrence emits, how many, and the state a decoder would have
/// been in immediately before -- a valid slot to index `decode_table`
/// with for the preceding symbol's own step.
///
/// Relies on the property that [`build_decode_table`]'s per-symbol
/// occurrence ranges (`[new_state_base, new_state_base + 2^nb_bits)`)
/// partition `[0, table_size)` exactly and contiguously, so exactly one
/// occurrence of any given symbol covers any given `target_state`. No
/// closed form yet ([`build_encode_table`]'s own docs deferred one): this
/// searches `encode_table[symbol]`'s occurrences in order, which is at
/// most `freq[symbol]` decode-table lookups.
///
/// # Panics
///
/// Panics if `symbol` is not a valid index into `encode_table`, or if its
/// row is empty (`freq[symbol] == 0`: it never occurred, so it could
/// never have been encoded) -- caller error, not yet reachable from any
/// decode path. Panics (via the `expect`) if no occurrence covers
/// `target_state`, which would mean `encode_table`/`decode_table` were
/// not built from the same `spread_symbols` output for the same `freq`,
/// the only supported caller shape.
#[must_use]
pub fn encode_symbol(
    target_state: u32,
    symbol: u32,
    encode_table: &[Vec<u32>],
    decode_table: &[DecodeSlot],
) -> Encoded {
    let idx = symbol as usize;
    assert!(
        idx < encode_table.len(),
        "symbol {symbol} is not a valid index into encode_table (len {})",
        encode_table.len()
    );
    let row = &encode_table[idx];
    assert!(
        !row.is_empty(),
        "symbol {symbol} never occurred (freq 0), cannot encode it"
    );
    let slot = row
        .iter()
        .copied()
        .find(|&slot| {
            let info = &decode_table[slot as usize];
            let width = 1u32 << info.nb_bits;
            target_state >= info.new_state_base && target_state - info.new_state_base < width
        })
        .unwrap_or_else(|| {
            panic!(
                "no occurrence of symbol {symbol} covers state {target_state}; \
                 encode_table/decode_table must come from the same spread_symbols \
                 output for the same freq"
            )
        });
    let info = &decode_table[slot as usize];
    Encoded {
        nb_bits: info.nb_bits,
        bits: target_state - info.new_state_base,
        state: slot,
    }
}

/// Encodes `symbols` (each a valid index into `encode_table`/`freq`,
/// with a nonzero frequency) into a packed byte buffer, using the tANS
/// tables built from that same `freq`. Returns the buffer and the state
/// [`decode_message`] must be given to decode it back.
///
/// Processes `symbols` in reverse: tANS state threads backward from a
/// fixed seed (state 0, arbitrary -- [`decode_message`] never assumes
/// any particular value, it only receives whatever this function
/// returns), each step's [`encode_symbol`] call producing the bits for
/// one symbol and the state to encode the symbol before it against.
/// Collecting those bits in encode order and reversing once, before
/// packing, is what lets [`decode_message`] read the returned buffer
/// forward from byte 0 instead of needing to read backward from its end.
///
/// # Panics
///
/// Panics under the same conditions [`encode_symbol`] does, for any
/// symbol in `symbols`.
#[must_use]
pub fn encode_message(
    symbols: &[u32],
    encode_table: &[Vec<u32>],
    decode_table: &[DecodeSlot],
) -> (Vec<u8>, u32) {
    let mut state = 0u32;
    let mut steps: Vec<(u32, u32)> = Vec::with_capacity(symbols.len());
    for &symbol in symbols.iter().rev() {
        let step = encode_symbol(state, symbol, encode_table, decode_table);
        steps.push((step.nb_bits, step.bits));
        state = step.state;
    }
    steps.reverse();
    let mut writer = BitWriter::new();
    for (nb_bits, bits) in steps {
        writer.write(bits, nb_bits);
    }
    (writer.finish(), state)
}

/// Decodes `count` symbols from `bytes`, starting from `initial_state`
/// (the state [`encode_message`] returned alongside the buffer being
/// decoded), using `decode_table`.
///
/// Each step indexes `decode_table` by the current state, reads
/// [`DecodeSlot::nb_bits`] bits off `bytes`, and adds them to
/// [`DecodeSlot::new_state_base`] for the next state -- [`DecodeSlot`]'s
/// own docs already fully specify this step, so this function is the
/// loop over it, plus the actual bit reads [`encode_symbol`]'s
/// counterpart never had to perform.
///
/// # Panics
///
/// Panics if `initial_state` is not a valid index into `decode_table`.
/// Panics if `bytes` runs out before `count` symbols have been decoded
/// -- caller error (asking for more symbols than were encoded, or
/// passing a `count`/`decode_table` that do not match the buffer's
/// origin), never a property of adversarial input: this primitive is not
/// yet reachable from any decode path.
#[must_use]
pub fn decode_message(
    bytes: &[u8],
    initial_state: u32,
    count: usize,
    decode_table: &[DecodeSlot],
) -> Vec<u32> {
    assert!(
        (initial_state as usize) < decode_table.len(),
        "initial_state {initial_state} is not a valid index into a decode_table of length {}",
        decode_table.len()
    );
    let mut reader = BitReader::new(bytes);
    let mut state = initial_state;
    let mut symbols = Vec::with_capacity(count);
    for _ in 0..count {
        let slot = &decode_table[state as usize];
        symbols.push(slot.symbol);
        let bits = reader.read(slot.nb_bits);
        state = slot.new_state_base + bits;
    }
    symbols
}

#[cfg(test)]
mod tests;

// Not under Miri: same rationale as `coder.rs`'s `mod proptests` header
// comment (interpretation cost, issue #456).
#[cfg(test)]
#[cfg(not(miri))]
mod proptests;
