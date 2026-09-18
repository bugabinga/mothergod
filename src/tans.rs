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
//! **This slice.** [`encode_symbol`], [`encode_message`] and
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
//!
//! **Remaining S1-P6 scope.** Wiring this coder behind a new fast `Method`
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
        assert!(encode[1].is_empty());
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
        assert!(bytes.is_empty());
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
        assert!(bytes.is_empty());
        assert_eq!(initial_state, 0);
        let decoded = decode_message(&bytes, initial_state, 0, &decode);
        assert!(decoded.is_empty());
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
}

// Not under Miri: interpretation costs 300-5000x per case on this crate
// (measured, issue #456), the storm multiplies that by its case count,
// and `mod tests`' own examples already walk the same round-trip shape
// for UB observation (`coder.rs`'s `mod proptests` header comment records
// the same reasoning for the entropy coder this one parallels).
#[cfg(test)]
#[cfg(not(miri))]
mod proptests {
    use proptest::prelude::*;

    use super::{
        build_decode_table, build_encode_table, decode_message, encode_message,
        normalize_frequencies, spread_symbols,
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
    }
}
