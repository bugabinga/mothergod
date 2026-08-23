//! LZ token stream: greedy/lazy parse and its inverse (`JOURNAL` S1-A3, M1
//! port, first slice of `research/JOURNAL.md` S2-D2).
//!
//! Ported from the archive's `lz` (`research/imports/session-1/
//! mothergod.rs`), not `lz_opt`: the archive's optimal-parse DP prices
//! candidates against the entropy models' own frequency tables, which
//! don't exist in this crate yet (`filters` is ported, the coder is not).
//! `lz_opt` also runs `lz` internally as its price-seeding first pass, so
//! this greedy/lazy parser is not a detour, it is a real prerequisite.
//! [`parse_greedy`] alone is already a usable LZ front end (a real
//! encoder could ship on it today) and [`replay`] proves it losslessly
//! reversible at the token level without needing the entropy coder to
//! exist. The DP-priced optimal parse is a follow-up slice.
//!
//! Behavior ported, not code, per ADR-0006: the archive's single `find`
//! closure captured by both the rep-cache scan and the hash-chain search
//! becomes two named functions here (`match_len` and
//! `MatchFinder::find_best`), and the raw `(usize, usize)` `(0, 0)`
//! "no match" sentinel pair becomes `Option<(usize, Distance)>` — a match
//! can no longer be represented with a zero distance, closing the exact
//! confusion class the session-1 port bug came from (`rust-craft` skill,
//! type-precision).

use std::num::NonZeroU32;

/// Largest backward distance a match may reference (`JOURNAL` S1-A3: 1 MiB
/// window).
pub const WINDOW: usize = 1 << 20;

/// Shortest run [`parse_greedy`] emits as [`Token::Match`]. Below this a
/// literal costs fewer bits than a match's length+distance overhead.
const MIN_MATCH_LEN: usize = 4;

/// Shortest run [`parse_greedy`] emits as [`Token::Rep`]. Cheaper than
/// [`MIN_MATCH_LEN`] because a repeat costs no distance field, only a
/// cache slot.
const MIN_REP_LEN: usize = 2;

/// Longest run a single token can cover; longer repeats are split into
/// several tokens. Matches the archive's cap.
const MAX_MATCH_LEN: usize = 65535;

/// Above this length the one-step lazy-matching check ([`parse_greedy`])
/// is skipped: a match this long is already an obvious win, not worth the
/// one-token delay of checking whether the next position does better.
const LAZY_MAX_LEN: usize = 256;

/// Hash-chain positions [`MatchFinder::find_best`] walks before giving up.
/// Bounds worst-case parse time on pathological (highly repetitive) input.
const MAX_CHAIN_TRIES: usize = 128;

/// `log2` of the match-finder hash table's bucket count (a 3-byte prefix
/// hash).
const HASH_BITS: u32 = 17;

/// Repeat-offset cache slot count (`JOURNAL` S1-A3: 3-slot cache).
const REP_SLOTS: usize = 3;

/// Cache-empty sentinel in the match finder's hash chains: no valid
/// position, ever, since positions are assigned from `0`.
const NO_POSITION: u32 = u32::MAX;

/// A backward-copy distance, in bytes. Never zero: distance `0` would copy
/// from the byte about to be written, which is not a match. Kept distinct
/// from a match length (also stored as a plain `u32`) because the two are
/// trivially confusable at a call site and a swap silently produces a
/// bitstream that decodes to the wrong bytes (`rust-craft` skill,
/// type-precision: the session-1 port bug was exactly this shape, a
/// rep-symbol/offset-bucket collision).
pub type Distance = NonZeroU32;

/// Which slot of the repeat-offset cache a [`Token::Rep`] reuses.
///
/// An enum instead of a raw index: the cache always has exactly
/// `REP_SLOTS` slots, so a fourth variant or an out-of-range index
/// cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepSlot {
    /// Most recently used distance.
    First,
    /// Second most recently used distance.
    Second,
    /// Third most recently used distance.
    Third,
}

impl RepSlot {
    /// All slots, most- to least-recently-used order.
    const ALL: [Self; REP_SLOTS] = [Self::First, Self::Second, Self::Third];

    /// This slot's position in [`RepCache`]'s backing array.
    const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
            Self::Third => 2,
        }
    }
}

/// One step of an LZ parse: a literal byte, a match against an arbitrary
/// earlier distance, or a match against one of the three most recently
/// used distances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    /// A single byte copied verbatim.
    Literal(u8),
    /// Copy `len` bytes from `distance` bytes back.
    Match {
        /// Number of bytes copied.
        len: u32,
        /// How far back the copy source starts.
        distance: Distance,
    },
    /// Copy `len` bytes from the distance cached at `slot`.
    Rep {
        /// Number of bytes copied.
        len: u32,
        /// Which cached distance to reuse.
        slot: RepSlot,
    },
}

/// `len` is always at most [`MAX_MATCH_LEN`] (65535, well under
/// `u32::MAX`): every call site bounds it before this conversion runs, so
/// the cast never truncates.
fn match_len_as_u32(len: usize) -> u32 {
    u32::try_from(len).expect("match length bounded by MAX_MATCH_LEN, always fits u32")
}

/// The 3-slot repeat-offset cache carried through a parse (`JOURNAL`
/// S1-A3): a distance a real match just used is cheap to reuse, so both
/// [`parse_greedy`] and [`replay`] track the same three most-recently-used
/// distances and must reorder them identically on every hit or the two
/// fall out of sync.
#[derive(Debug, Clone, Copy)]
struct RepCache([Distance; REP_SLOTS]);

impl RepCache {
    /// Starting cache before any real match has been seen. Matches the
    /// archive's initial `[1, 4, 8]`: plausible small distances, never
    /// read unless a rep candidate actually matches at one of them.
    fn initial() -> Self {
        const ONE: Distance = NonZeroU32::new(1).unwrap();
        const FOUR: Distance = NonZeroU32::new(4).unwrap();
        const EIGHT: Distance = NonZeroU32::new(8).unwrap();
        Self([ONE, FOUR, EIGHT])
    }

    /// The distance currently cached at `slot`.
    fn get(&self, slot: RepSlot) -> Distance {
        self.0[slot.index()]
    }

    /// Moves `distance` to the front, used after a [`Token::Match`] on a
    /// distance not already the most-recently-used one; the two previous
    /// front slots slide back, and the third (oldest) is dropped.
    fn push_front(&mut self, distance: Distance) {
        self.0[2] = self.0[1];
        self.0[1] = self.0[0];
        self.0[0] = distance;
    }

    /// Moves the distance at `slot` to the front, used after a
    /// [`Token::Rep`]; the slots ahead of it slide back by one, the slots
    /// behind it are untouched.
    fn promote(&mut self, slot: RepSlot) {
        let distance = self.get(slot);
        let idx = slot.index();
        for i in (1..=idx).rev() {
            self.0[i] = self.0[i - 1];
        }
        self.0[0] = distance;
    }
}

/// Longest run starting at `i` that matches the window starting
/// `distance` bytes earlier, capped at [`MAX_MATCH_LEN`] and by the data
/// actually remaining. Zero when `distance` reaches before the start of
/// `data`, or when the very first bytes compared differ.
fn match_len(data: &[u8], i: usize, distance: Distance) -> usize {
    let distance = distance.get() as usize;
    if distance > i {
        return 0;
    }
    let mut len = 0;
    while len < MAX_MATCH_LEN && i + len < data.len() && data[i - distance + len] == data[i + len] {
        len += 1;
    }
    len
}

/// Longest match against each of `reps`' three cached distances at `i`,
/// and which slot won. Ties keep the earliest (most-recently-used) slot:
/// the comparison below is strict, so an equal-length match at a later
/// slot never displaces an earlier one, matching the archive.
fn best_rep(data: &[u8], reps: RepCache, i: usize) -> (usize, RepSlot) {
    let mut best_len = 0;
    let mut best_slot = RepSlot::First;
    for slot in RepSlot::ALL {
        let len = match_len(data, i, reps.get(slot));
        if len > best_len {
            best_len = len;
            best_slot = slot;
        }
    }
    (best_len, best_slot)
}

/// Hash-chain match finder over a 3-byte prefix hash, one per [`parse_greedy`]
/// call. Bounded to a fixed-size hash table plus one `u32` per input byte
/// (`prev`); the `prev` allocation is proportional to the caller's own
/// input, already held in full by the caller, not a hostile amplification
/// (`rust-craft` skill, allocation-discipline: that hazard is about the
/// *decoder* trusting an attacker-controlled length field, which this
/// encode-only structure never reads).
struct MatchFinder<'d> {
    data: &'d [u8],
    /// `head[hash]` is the most recently inserted position with that
    /// hash, or [`NO_POSITION`].
    head: Vec<u32>,
    /// `prev[i]` is the position inserted just before `i` with the same
    /// hash, or [`NO_POSITION`]; walking it from `head[hash]` visits every
    /// candidate newest-first.
    prev: Vec<u32>,
}

impl<'d> MatchFinder<'d> {
    fn new(data: &'d [u8]) -> Self {
        Self {
            data,
            head: vec![NO_POSITION; 1 << HASH_BITS],
            prev: vec![NO_POSITION; data.len().max(1)],
        }
    }

    /// Hash of the 3-byte prefix at `i`. Positions within 2 bytes of the
    /// end have no full prefix and all fall into bucket 0; harmless, since
    /// [`match_len`] verifies real bytes before any hash-chain hit is
    /// trusted.
    fn hash(&self, i: usize) -> usize {
        let d = self.data;
        if i + 3 > d.len() {
            return 0;
        }
        let h = (usize::from(d[i]) << 10) ^ (usize::from(d[i + 1]) << 5) ^ usize::from(d[i + 2]);
        h & ((1 << HASH_BITS) - 1)
    }

    /// Records `i` as a match candidate for future positions sharing its
    /// 3-byte prefix hash.
    fn insert(&mut self, i: usize) {
        let h = self.hash(i);
        self.prev[i] = self.head[h];
        // parse_greedy asserts data.len() fits u32 before constructing
        // this finder; i < data.len() always.
        self.head[h] = u32::try_from(i).expect("position fits in u32, checked by parse_greedy");
    }

    /// Best match ending at `i`, found by walking the hash chain for `i`'s
    /// 3-byte prefix, bounded by [`WINDOW`] and [`MAX_CHAIN_TRIES`]. Does
    /// not require `i` itself to have been [`insert`](Self::insert)ed;
    /// [`parse_greedy`]'s one-step lazy-matching probe relies on that to
    /// look ahead without mutating the chain.
    fn find_best(&self, i: usize) -> Option<(usize, Distance)> {
        let mut best_len = 0usize;
        let mut best_distance = None;
        let mut j = self.head[self.hash(i)];
        let mut tries = 0;
        while j != NO_POSITION && tries < MAX_CHAIN_TRIES {
            let j_pos = j as usize;
            let offset = i - j_pos; // j was inserted at an earlier position: i > j_pos always.
            if offset > WINDOW {
                break;
            }
            if offset > 0 {
                // offset <= WINDOW (2^20) here, always fits u32.
                let offset_u32 =
                    u32::try_from(offset).expect("offset <= WINDOW, checked above, fits u32");
                let distance =
                    NonZeroU32::new(offset_u32).expect("offset > 0, checked by the branch above");
                let len = match_len(self.data, i, distance);
                if len > best_len {
                    best_len = len;
                    best_distance = Some(distance);
                }
            }
            j = self.prev[j_pos];
            tries += 1;
        }
        best_distance.map(|distance| (best_len, distance))
    }
}

/// Greedy parse with one-step lazy matching and the 3-slot repeat-offset
/// cache (`JOURNAL` S1-A3, the archive's `lz`; see the module docs for why
/// this precedes `lz_opt` rather than skipping to it).
///
/// # Panics
///
/// Panics if `data` is longer than `u32::MAX` bytes: match positions are
/// stored as `u32` in the hash-chain match finder. Nothing in this crate
/// exceeds that yet; a future streaming/block API will chunk input well
/// under this bound rather than lift it.
#[must_use]
pub fn parse_greedy(data: &[u8]) -> Vec<Token> {
    let n = data.len();
    assert!(
        u32::try_from(n).is_ok(),
        "lz::parse_greedy: input longer than u32::MAX is not supported yet"
    );
    let mut tokens = Vec::new();
    let mut reps = RepCache::initial();
    let mut finder = MatchFinder::new(data);
    let mut i = 0usize;
    while i < n {
        finder.insert(i);
        let (rep_len, rep_slot) = best_rep(data, reps, i);
        let found = finder.find_best(i);
        let match_len = found.map_or(0, |(len, _)| len);

        if (MIN_MATCH_LEN..LAZY_MAX_LEN).contains(&match_len)
            && rep_len + 1 < match_len
            && i + 1 < n
        {
            let next_len = finder.find_best(i + 1).map_or(0, |(len, _)| len);
            if next_len > match_len {
                tokens.push(Token::Literal(data[i]));
                i += 1;
                continue;
            }
        }

        if rep_len >= MIN_REP_LEN && rep_len + 1 >= match_len {
            for k in i + 1..i + rep_len {
                finder.insert(k);
            }
            tokens.push(Token::Rep {
                len: match_len_as_u32(rep_len),
                slot: rep_slot,
            });
            reps.promote(rep_slot);
            i += rep_len;
        } else if match_len >= MIN_MATCH_LEN {
            let (_, distance) =
                found.expect("match_len >= MIN_MATCH_LEN only when find_best found one");
            for k in i + 1..i + match_len {
                finder.insert(k);
            }
            tokens.push(Token::Match {
                len: match_len_as_u32(match_len),
                distance,
            });
            reps.push_front(distance);
            i += match_len;
        } else {
            tokens.push(Token::Literal(data[i]));
            i += 1;
        }
    }
    tokens
}

/// Copies `len` bytes to the end of `out` from `distance` bytes before its
/// current end, one byte at a time so a distance shorter than `len` (a
/// run, not a disjoint repeat) still reproduces the source correctly —
/// each copied byte becomes visible to later reads in the same call, the
/// way `extend_from_slice` from a fixed source region would not.
fn copy_match(out: &mut Vec<u8>, len: u32, distance: Distance) {
    let distance = distance.get() as usize;
    let start = out
        .len()
        .checked_sub(distance)
        .expect("match distance must not reach before the output produced so far");
    for k in 0..len as usize {
        let byte = out[start + k];
        out.push(byte);
    }
}

/// Reconstructs the original bytes from `tokens`, the inverse of
/// [`parse_greedy`]. Not decode of a real bitstream yet: there is no
/// entropy coder (`research/JOURNAL.md` S2-D2), so `Token` is not wire
/// format, only this crate's own intermediate representation. It is the
/// same replay loop a real decoder will need, and it is what proves
/// `parse_greedy` losslessly reversible today, ahead of the coder.
///
/// # Panics
///
/// Panics if `tokens` was not produced by [`parse_greedy`] starting from
/// the same initial cache state: a distance reaching before the start of
/// the output so far is an internal-invariant violation, not malformed
/// external input (`rust-craft` skill, panic-discipline — `Token` is not
/// wire format, so this is not the decoder hard rule 2 guards).
#[must_use]
pub fn replay(tokens: &[Token]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut reps = RepCache::initial();
    for token in tokens {
        match *token {
            Token::Literal(byte) => out.push(byte),
            Token::Match { len, distance } => {
                copy_match(&mut out, len, distance);
                reps.push_front(distance);
            }
            Token::Rep { len, slot } => {
                let distance = reps.get(slot);
                copy_match(&mut out, len, distance);
                reps.promote(slot);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
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
        // distance (1) shorter than the eventual match length: copy_match
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
}
