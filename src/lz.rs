//! LZ token stream: greedy/lazy parse and its inverse (`JOURNAL` S1-A3, M1
//! port, first slice of `research/JOURNAL.md` S2-D2).
//!
//! Ported from the archive's `lz` (`research/imports/session-1/
//! mothergod.rs`), not `lz_opt`: the archive's optimal-parse DP prices
//! candidates against its own lightweight frequency tables (not the real
//! entropy models, which still don't exist in this crate — `filters` is
//! ported, the coder is not). `lz_opt` also runs `lz` internally as its
//! price-seeding first pass, so this greedy/lazy parser is not a detour,
//! it is a real prerequisite. [`parse_greedy`] alone is already a usable
//! LZ front end (a real encoder could ship on it today) and [`replay`]
//! proves it losslessly reversible at the token level without needing
//! the entropy coder to exist.
//!
//! [`parse_optimal`] is the follow-up slice: the archive's `lz_opt`, a
//! two-round DP-priced optimal parse seeded by [`parse_greedy`]. See its
//! docs for one deliberate correctness fix over the archive's own DP.
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

/// [`MIN_REP_LEN`] as a `u32`, for [`dp_round`]'s price-table indexing
/// (lengths are priced as `u32` throughout, matching [`Token`]'s own
/// fields). A compile-time literal (2): always fits, so the truncation
/// lint below has nothing to actually catch.
#[allow(clippy::cast_possible_truncation)]
const MIN_REP_LEN_U32: u32 = MIN_REP_LEN as u32;

/// Longest run a single token can cover; longer repeats are split into
/// several tokens. Matches the archive's cap.
const MAX_MATCH_LEN: usize = 65535;

/// Above this length the one-step lazy-matching check ([`parse_greedy`])
/// is skipped: a match this long is already an obvious win, not worth the
/// one-token delay of checking whether the next position does better.
const LAZY_MAX_LEN: usize = 256;

/// Hash-chain positions [`MatchFinder::find_best`] walks before giving up,
/// for [`parse_greedy`]'s once-per-token search. Bounds worst-case parse
/// time on pathological (highly repetitive) input.
const MAX_CHAIN_TRIES: usize = 128;

/// Hash-chain positions [`MatchFinder::find_best`] walks for
/// [`parse_optimal`]'s once-per-position search: deeper than
/// [`MAX_CHAIN_TRIES`] because the DP amortizes the cost via the `carry`
/// reuse in [`dp_round`] rather than searching once per emitted token.
const MAX_CHAIN_TRIES_OPTIMAL: usize = 640;

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
pub(crate) struct RepCache([Distance; REP_SLOTS]);

impl RepCache {
    /// Starting cache before any real match has been seen. Matches the
    /// archive's initial `[1, 4, 8]`: plausible small distances, never
    /// read unless a rep candidate actually matches at one of them.
    ///
    /// Shared with [`crate::codec`]'s decode path (`JOURNAL` S2-D2): the
    /// rep cache's update rules are exactly the S1-A3 port bug's former
    /// hazard, so decode reuses this type instead of a second
    /// hand-written copy that could drift out of sync with what
    /// [`replay`] actually does.
    pub(crate) fn initial() -> Self {
        const ONE: Distance = NonZeroU32::new(1).unwrap();
        const FOUR: Distance = NonZeroU32::new(4).unwrap();
        const EIGHT: Distance = NonZeroU32::new(8).unwrap();
        Self([ONE, FOUR, EIGHT])
    }

    /// The distance currently cached at `slot`.
    pub(crate) fn get(&self, slot: RepSlot) -> Distance {
        self.0[slot.index()]
    }

    /// Moves `distance` to the front, used after a [`Token::Match`] on a
    /// distance not already the most-recently-used one; the two previous
    /// front slots slide back, and the third (oldest) is dropped.
    pub(crate) fn push_front(&mut self, distance: Distance) {
        self.0[2] = self.0[1];
        self.0[1] = self.0[0];
        self.0[0] = distance;
    }

    /// Moves the distance at `slot` to the front, used after a
    /// [`Token::Rep`]; the slots ahead of it slide back by one, the slots
    /// behind it are untouched.
    pub(crate) fn promote(&mut self, slot: RepSlot) {
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

/// 3-byte prefix hash shared by every match finder in this module
/// ([`MatchFinder`] and [`BinaryTreeMatchFinder`]): factored out so their
/// notion of "candidates sharing a hash bucket" cannot drift apart between
/// the two structures.
///
/// Positions within 2 bytes of the end have no full prefix and all fall
/// into bucket 0; harmless everywhere it's called, since [`match_len`] (or
/// [`suffix_common_len`]) verifies real bytes before any hit from either
/// finder is trusted.
fn prefix_hash(data: &[u8], i: usize) -> usize {
    if i + 3 > data.len() {
        return 0;
    }
    let h =
        (usize::from(data[i]) << 10) ^ (usize::from(data[i + 1]) << 5) ^ usize::from(data[i + 2]);
    h & ((1 << HASH_BITS) - 1)
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

    /// Hash of the 3-byte prefix at `i`. See [`prefix_hash`].
    fn hash(&self, i: usize) -> usize {
        prefix_hash(self.data, i)
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
    /// 3-byte prefix, bounded by [`WINDOW`] and `max_tries` (callers pass
    /// [`MAX_CHAIN_TRIES`] or [`MAX_CHAIN_TRIES_OPTIMAL`]). Does not
    /// require `i` itself to have been [`insert`](Self::insert)ed;
    /// [`parse_greedy`]'s one-step lazy-matching probe relies on that to
    /// look ahead without mutating the chain.
    fn find_best(&self, i: usize, max_tries: usize) -> Option<(usize, Distance)> {
        let mut best_len = 0usize;
        let mut best_distance = None;
        let mut j = self.head[self.hash(i)];
        let mut tries = 0;
        while j != NO_POSITION && tries < max_tries {
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

/// Longest common run between `data[a..]` and `data[b..]`, given `a < b`
/// (so `data[a..]` is never shorter than `data[b..]`). Same cap as
/// [`match_len`] ([`MAX_MATCH_LEN`], remaining data from the later
/// position `b`), but compares two arbitrary earlier positions instead of
/// a position against itself at a backward distance — what
/// [`BinaryTreeMatchFinder`] needs to order two candidates' suffixes
/// against each other, not just measure one candidate's match length.
fn suffix_common_len(data: &[u8], a: usize, b: usize) -> usize {
    let max_len = (data.len() - b).min(MAX_MATCH_LEN);
    let mut len = 0;
    while len < max_len && data[a + len] == data[b + len] {
        len += 1;
    }
    len
}

/// `i`, or a `WINDOW`-bounded distance derived from it, as a `u32`. Every
/// caller in this module already bounds its argument below `u32::MAX`
/// (a position by `data.len()` fitting `u32` per [`parse_greedy`]'s own
/// precondition, a distance by [`WINDOW`]), so this never truncates.
fn to_u32(i: usize) -> u32 {
    u32::try_from(i).expect("bounded by data.len() or WINDOW, both well under u32::MAX")
}

/// Binary-tree match finder (`JOURNAL` S1-P2, "btultra2-class parse"'s
/// first slice): unlike `MatchFinder`'s hash chain, which walks
/// candidates in pure recency order and gives up after a fixed number of
/// tries, insertion here keeps each hash bucket as a binary search tree
/// ordered by the candidate's suffix bytes. [`Self::insert_and_find`]
/// both inserts the new position and returns the single longest match,
/// found by one downward walk that visits only the nodes on the
/// insertion path: for data whose match distribution keeps that path
/// shallow, far fewer comparisons than a hash chain has to make to
/// consider every same-bucket candidate.
///
/// With `max_depth` at least the bucket's true tree height, the match
/// returned is length-exact: it equals a brute-force scan of every
/// candidate in the bucket, proved by
/// `tests::binary_tree_matches_brute_force`. A shallower `max_depth`
/// trades that exactness for a bounded worst case, the same trade
/// `MatchFinder` already makes via `max_tries`.
///
/// Standalone primitive, not yet wired into [`parse_greedy`] or
/// [`parse_optimal`] — the same standalone-primitive-first shape
/// [`crate::sse::Sse`] shipped in S2-A40. Two things a wired-in successor
/// still needs that this slice does not implement: eviction of positions
/// older than [`WINDOW`] (the tree only grows, so nothing yet bounds it to
/// a 1 MiB working set the way `MatchFinder`'s recency-ordered chain
/// does by construction) and the length-prefix-reuse optimization real
/// bt4 match finders use to keep comparisons near the tree height rather
/// than the match length itself (`len0`/`len1` in the LZMA reference
/// implementation, deliberately not carried here — see
/// `suffix_common_len`, which always compares from scratch). Both are
/// speed/memory work, not correctness work; they wait for whichever slice
/// wires this into a parse pass.
pub struct BinaryTreeMatchFinder<'d> {
    data: &'d [u8],
    /// `head[hash]` is the current tree root for that hash bucket, or
    /// [`NO_POSITION`].
    head: Vec<u32>,
    /// `left[i]`/`right[i]`: the subtree of already-inserted positions
    /// whose suffix sorts before/after `i`'s, or [`NO_POSITION`]. Only
    /// meaningful once `i` has been passed to
    /// [`Self::insert_and_find`].
    left: Vec<u32>,
    right: Vec<u32>,
}

impl<'d> BinaryTreeMatchFinder<'d> {
    /// A finder with no positions inserted yet.
    #[must_use]
    pub fn new(data: &'d [u8]) -> Self {
        Self {
            data,
            head: vec![NO_POSITION; 1 << HASH_BITS],
            left: vec![NO_POSITION; data.len().max(1)],
            right: vec![NO_POSITION; data.len().max(1)],
        }
    }

    /// Inserts `i` into its hash bucket's tree and returns the longest
    /// match found among the candidates visited on the way down, bounded
    /// to at most `max_depth` of them (see the struct docs for what
    /// `max_depth` trades off). The match's distance is always within
    /// [`WINDOW`]; a candidate farther than that still participates in
    /// the tree's structure (it may still separate other candidates) but
    /// is never reported as a match.
    ///
    /// Each position must be inserted at most once, in increasing order
    /// — an LZ parse's own shape, not checked here: this type is
    /// encode-only, so its caller is this crate's own parser, never
    /// adversarial input.
    ///
    /// # Panics
    ///
    /// Panics if `i >= data.len()` (`data` from [`Self::new`]): reading
    /// past the end would be a caller bug, never something adversarial
    /// input can trigger.
    #[must_use]
    pub fn insert_and_find(&mut self, i: usize, max_depth: usize) -> Option<(usize, Distance)> {
        assert!(i < self.data.len(), "position out of range");
        let h = prefix_hash(self.data, i);
        let mut cur = self.head[h];
        self.head[h] = to_u32(i);

        // The walk splits the existing tree into a "less" chain (suffixes
        // sorting before i's) and a "greater" chain (sorting after),
        // which become i's left and right subtrees. `less_tail`/
        // `greater_tail` name the most recently attached node on each
        // side; `None` means that side is still empty, so the next node
        // found becomes `less_root`/`greater_root` (i's future child)
        // instead of some existing node's child.
        let mut less_root = NO_POSITION;
        let mut less_tail: Option<usize> = None;
        let mut greater_root = NO_POSITION;
        let mut greater_tail: Option<usize> = None;

        let mut best_len = 0usize;
        let mut best_distance: Option<Distance> = None;
        let mut depth = 0;

        while cur != NO_POSITION && depth < max_depth {
            let cur_pos = cur as usize;
            let common = suffix_common_len(self.data, cur_pos, i);
            let distance = i - cur_pos; // cur_pos was inserted earlier: i > cur_pos always.
            if common > best_len && distance <= WINDOW {
                best_len = common;
                best_distance = NonZeroU32::new(to_u32(distance));
            }
            if self.data.get(cur_pos + common) < self.data.get(i + common) {
                // cur's suffix sorts before i's: it belongs on the "less"
                // side, descend into its right subtree (candidates
                // between cur and i) to look for closer ones.
                if let Some(t) = less_tail {
                    self.right[t] = to_u32(cur_pos);
                } else {
                    less_root = to_u32(cur_pos);
                }
                less_tail = Some(cur_pos);
                cur = self.right[cur_pos];
            } else {
                // cur's suffix sorts after (or ties with, up to the
                // shorter one's end) i's: descend into its left subtree.
                if let Some(t) = greater_tail {
                    self.left[t] = to_u32(cur_pos);
                } else {
                    greater_root = to_u32(cur_pos);
                }
                greater_tail = Some(cur_pos);
                cur = self.left[cur_pos];
            }
            depth += 1;
        }
        // Whatever remains unlinked on either chain (the walk exhausted
        // max_depth, or ended naturally) is cut off rather than left
        // dangling: those deeper candidates are dropped from the tree,
        // the same bounded-search trade-off MatchFinder makes via
        // max_tries.
        if let Some(t) = less_tail {
            self.right[t] = NO_POSITION;
        }
        if let Some(t) = greater_tail {
            self.left[t] = NO_POSITION;
        }
        self.left[i] = less_root;
        self.right[i] = greater_root;

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
        let found = finder.find_best(i, MAX_CHAIN_TRIES);
        let match_len = found.map_or(0, |(len, _)| len);

        if (MIN_MATCH_LEN..LAZY_MAX_LEN).contains(&match_len)
            && rep_len + 1 < match_len
            && i + 1 < n
        {
            let next_len = finder
                .find_best(i + 1, MAX_CHAIN_TRIES)
                .map_or(0, |(len, _)| len);
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

// ---- optimal parse: DP-priced, in-DP rep cache, 2-round price iteration ----

/// Below this input length [`parse_optimal`] falls back to [`parse_greedy`]:
/// matches the archive's `n<64` short-circuit in `lz_opt` — the DP's fixed
/// setup cost (two full passes, hash-chain rebuild) isn't worth paying on
/// an input this small.
const OPTIMAL_MIN_LEN: usize = 64;

/// Match length at and above which [`dp_round`]'s normal-match search
/// reuses the previous position's result instead of re-walking the hash
/// chain (the archive's `carry`): a run this long at position `i` is
/// `len - 1` bytes long at `i + 1` too, at the same distance, since it is
/// the same underlying repeat.
const CARRY_MIN_LEN: usize = 64;

/// Match length [`dp_round`] additionally prices as a candidate when the
/// source distance is close (below [`SHORT_MATCH_MAX_DISTANCE`]): one
/// byte under [`MIN_MATCH_LEN`], cheap enough at a close distance to beat
/// a literal even though [`parse_greedy`] never considers it.
const SHORT_MATCH_LEN: u32 = 3;

/// Distance ceiling for the [`SHORT_MATCH_LEN`] candidate: beyond this an
/// offset field costs more than the length-3 match saves.
const SHORT_MATCH_MAX_DISTANCE: u32 = 4096;

/// Length candidates [`dp_round`] prices at each position instead of every
/// length up to the longest available match (the archive's `BOUND`):
/// log2-ish spaced breakpoints, cheap to price exhaustively, plus the
/// longest available length itself so a long match is never left
/// unpriced merely for falling between two breakpoints.
const LENGTH_STEPS: [u32; 13] = [4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 63];

/// Previous-byte high-nibble contexts the literal price table conditions
/// on, matching the archive's `lh` (16 contexts of 256 symbols each).
const LITERAL_CONTEXTS: usize = 16;

/// [`bucket`] values a match/rep length ever falls into: `bucket(1)` is 0,
/// `bucket(MAX_MATCH_LEN)` (65535) is 15, one slot of headroom kept as in
/// the archive's `lb`.
///
/// Shared with [`crate::codec`] (`JOURNAL` S2-D2): the length [`Model`]
/// `Method::Lz` drives has exactly this many symbols, so its alphabet
/// size has one source of truth instead of a copy that could drift.
///
/// [`Model`]: crate::model::Model
pub(crate) const LENGTH_BUCKETS: usize = 17;

/// [`bucket`] values a match distance ever falls into: `bucket(WINDOW)`
/// (2^20) is 20, matching the archive's `ob`.
///
/// Shared with [`crate::codec`], same reason as [`LENGTH_BUCKETS`].
pub(crate) const OFFSET_BUCKETS: usize = 21;

/// Flat price approximation for the flag bit selecting a literal over a
/// match/rep at this pre-coder stage (the archive's literal `+1.0`): cheap
/// enough not to warrant its own frequency model yet.
const FLAG_BIT_PRICE: f64 = 1.0;

/// Flat price approximation for the extra header bits a rep-slot choice
/// or a fresh match distance costs beyond its modeled price (the
/// archive's `+1.6`, used for both).
const EXTRA_HEADER_BITS_PRICE: f64 = 1.6;

/// `floor(log2(v))`: the price table's bucket index for a match length or
/// distance, matching the archive's `bkt`. `v` is always a real length
/// (>= [`MIN_REP_LEN`], the smallest length ever priced) or a real
/// [`Distance`] (never zero by construction), so `v.leading_zeros()` is
/// always < 32 and the subtraction below never underflows.
///
/// Shared with [`crate::codec`] (`JOURNAL` S2-D2), which buckets the same
/// two quantities the same way on both the encode and decode path: a
/// second implementation here is exactly the kind of drift the S1-A3
/// rep-symbol/offset-bucket collision came from.
pub(crate) fn bucket(v: u32) -> usize {
    debug_assert!(
        v > 0,
        "bucket() is only ever called on a length or a distance, never zero"
    );
    (u32::BITS - v.leading_zeros() - 1) as usize
}

/// `bucket_index` as a price, in bits: a value inside `bucket_index` needs
/// that many raw bits on top of the coder's own modeled price to pin down
/// which value in that log2-wide bucket it actually was.
fn extra_bits(bucket_index: usize) -> f64 {
    // bucket_index is always < OFFSET_BUCKETS (21), far below f64's exact
    // integer range (2^53): no precision loss.
    #[allow(clippy::cast_precision_loss)]
    {
        bucket_index as f64
    }
}

/// `-log2(freq / total)`: the Shannon price, in bits, of a symbol seen
/// `freq` times out of `total`.
#[allow(
    clippy::disallowed_methods,
    reason = "encoder-only: DP price table (ADR-0024 decision 3), a different but still-valid frame costs reproducibility, not correctness"
)]
fn price(freq: u32, total: u32) -> f64 {
    -(f64::from(freq) / f64::from(total)).log2()
}

/// Converts a flat frequency table (already Laplace-smoothed, every entry
/// >= 1) into its per-entry price table.
fn to_prices(counts: &[u32]) -> Vec<f64> {
    let total: u32 = counts.iter().sum();
    counts.iter().map(|&freq| price(freq, total)).collect()
}

/// Per-symbol coding price estimate (bits, via `-log2(p)`) [`dp_round`]
/// ranks candidate parses by. These numbers are never emitted; only the
/// [`Token`]s the DP chose between are ([`parse_optimal`]'s result).
struct PriceTable {
    /// Literal price by `(previous byte's high nibble, byte)`:
    /// [`LITERAL_CONTEXTS`] contexts of 256 symbols each.
    literal: Vec<f64>,
    /// Match/rep length price by [`bucket`], [`LENGTH_BUCKETS`] entries.
    length: Vec<f64>,
    /// Match distance price by [`bucket`], [`OFFSET_BUCKETS`] entries.
    offset: Vec<f64>,
    /// Flat price of choosing a rep over a normal match, independent of
    /// slot or length (the archive's `rp`, a single scalar).
    rep: f64,
}

/// Raw frequency counts [`PriceTable`] is derived from, tallied from a
/// token sequence (the archive's `lh`/`lb`/`ob`/`nrep`). Every entry
/// starts at 1 (Laplace smoothing): an unseen symbol still gets a finite,
/// merely expensive, price instead of `-log2(0) = infinity`.
struct PriceCounts {
    literal: Vec<u32>,
    length: Vec<u32>,
    offset: Vec<u32>,
    rep: u32,
}

impl PriceCounts {
    fn new() -> Self {
        Self {
            literal: vec![1; LITERAL_CONTEXTS * 256],
            length: vec![1; LENGTH_BUCKETS],
            offset: vec![1; OFFSET_BUCKETS],
            rep: 1,
        }
    }

    /// Tallies `tokens` (produced by replaying them over `data`, only to
    /// recover each literal's preceding-byte context; a token's own
    /// length/distance/slot need no data lookup).
    fn tally(tokens: &[Token], data: &[u8]) -> Self {
        let mut counts = Self::new();
        let mut pos = 0usize;
        for token in tokens {
            match *token {
                Token::Literal(byte) => {
                    let context = if pos > 0 {
                        usize::from(data[pos - 1] >> 4)
                    } else {
                        0
                    };
                    counts.literal[context * 256 + usize::from(byte)] += 1;
                    pos += 1;
                }
                Token::Match { len, distance } => {
                    counts.length[bucket(len)] += 1;
                    counts.offset[bucket(distance.get())] += 1;
                    pos += len as usize;
                }
                Token::Rep { len, .. } => {
                    counts.length[bucket(len)] += 1;
                    counts.rep += 1;
                    pos += len as usize;
                }
            }
        }
        counts
    }

    /// Derives a [`PriceTable`] from these counts. `total_tokens` prices
    /// the rep flag against the full token count (the archive's
    /// `toks.len() + 2`, a small Laplace pad matching the `+1` already
    /// folded into every frequency entry).
    fn prices(&self, total_tokens: usize) -> PriceTable {
        let mut literal = vec![0.0; LITERAL_CONTEXTS * 256];
        for context in 0..LITERAL_CONTEXTS {
            let row = &self.literal[context * 256..context * 256 + 256];
            let total: u32 = row.iter().sum();
            for (symbol, &freq) in row.iter().enumerate() {
                literal[context * 256 + symbol] = price(freq, total);
            }
        }
        let total_tokens = u32::try_from(total_tokens).expect(
            "token count bounded by input length, already checked to fit u32 by parse_greedy",
        );
        PriceTable {
            literal,
            length: to_prices(&self.length),
            offset: to_prices(&self.offset),
            rep: price(self.rep, total_tokens + 2) + EXTRA_HEADER_BITS_PRICE,
        }
    }
}

/// Which move [`dp_round`]'s DP used to reach a position: [`Token`]'s own
/// fields, packed for backtracking rather than emission order.
#[derive(Debug, Clone, Copy)]
enum Move {
    Literal,
    Match { len: u32, distance: Distance },
    Rep { len: u32, slot: RepSlot },
}

/// [`dp_round`]'s working state: cheapest price to reach each position
/// (`dp`), the move that achieved it (`parent`), and the repeat-offset
/// cache that move leaves behind (`cache`) — carried alongside `dp`/
/// `parent` because later positions' rep candidates need to know it
/// without re-deriving it from the whole path so far.
struct DpState {
    dp: Vec<f64>,
    parent: Vec<Option<Move>>,
    cache: Vec<RepCache>,
}

impl DpState {
    fn new(n: usize) -> Self {
        let mut dp = vec![f64::INFINITY; n + 1];
        dp[0] = 0.0;
        Self {
            dp,
            parent: vec![None; n + 1],
            cache: vec![RepCache::initial(); n + 1],
        }
    }

    /// Records reaching `target` via `mv` with total price `cost`, if
    /// that beats the best price already known for `target`.
    fn relax(&mut self, cost: f64, target: usize, mv: Move, new_cache: RepCache) {
        if cost < self.dp[target] {
            self.dp[target] = cost;
            self.parent[target] = Some(mv);
            self.cache[target] = new_cache;
        }
    }

    /// [`Self::relax`] for a [`Token::Rep`] candidate of `len` at `slot`,
    /// starting from position `i` with base price `base` (`self.dp[i]`,
    /// passed in rather than re-read so callers can loop over several
    /// candidate lengths without re-borrowing `self` immutably each time).
    fn relax_rep(
        &mut self,
        prices: &PriceTable,
        base: f64,
        i: usize,
        len: u32,
        slot: RepSlot,
        new_cache: RepCache,
    ) {
        let cost = base + prices.length[bucket(len)] + extra_bits(bucket(len)) + prices.rep;
        self.relax(cost, i + len as usize, Move::Rep { len, slot }, new_cache);
    }

    /// Prices every [`Token::Rep`] candidate at position `i`: for each of
    /// the three cached distances that actually matches here, a length-2
    /// candidate (always tried, matching the archive's unconditional
    /// `relax(2.min(lr))` once a rep of any length exists), each
    /// [`LENGTH_STEPS`] breakpoint at or under the real match length, and
    /// the real match length itself if it isn't already a breakpoint.
    fn relax_rep_candidates(
        &mut self,
        prices: &PriceTable,
        data: &[u8],
        i: usize,
        base: f64,
        reps: RepCache,
        rep_carry: &mut [Option<(Distance, usize, usize)>; REP_SLOTS],
    ) {
        for slot in RepSlot::ALL {
            let distance = reps.get(slot);
            let rep_len = rep_match_len(data, i, distance, rep_carry, slot.index());
            if rep_len < MIN_REP_LEN {
                continue;
            }
            let mut new_cache = reps;
            new_cache.promote(slot);
            let rep_len = match_len_as_u32(rep_len);
            self.relax_rep(prices, base, i, MIN_REP_LEN_U32, slot, new_cache);
            for &len in &LENGTH_STEPS {
                if len > rep_len {
                    break;
                }
                self.relax_rep(prices, base, i, len, slot, new_cache);
            }
            if !LENGTH_STEPS.contains(&rep_len) && rep_len > MIN_REP_LEN_U32 {
                self.relax_rep(prices, base, i, rep_len, slot, new_cache);
            }
        }
    }

    /// Prices the normal-match candidate found at position `i`
    /// ([`next_match_candidate`]): a length-[`SHORT_MATCH_LEN`] candidate
    /// when the distance is close enough, and — for a real
    /// [`MIN_MATCH_LEN`]-or-longer match — each [`LENGTH_STEPS`]
    /// breakpoint under [`CARRY_MIN_LEN`] plus the real match length
    /// itself, matching the archive's `l1>=4` branch.
    fn relax_match_candidate(
        &mut self,
        prices: &PriceTable,
        i: usize,
        base: f64,
        reps: RepCache,
        match_len_here: usize,
        distance_here: Distance,
    ) {
        let mut new_cache = reps;
        new_cache.push_front(distance_here);
        let offset_cost = prices.offset[bucket(distance_here.get())]
            + extra_bits(bucket(distance_here.get()))
            + EXTRA_HEADER_BITS_PRICE;

        if match_len_here == SHORT_MATCH_LEN as usize
            && distance_here.get() < SHORT_MATCH_MAX_DISTANCE
        {
            let cost = base
                + prices.length[bucket(SHORT_MATCH_LEN)]
                + extra_bits(bucket(SHORT_MATCH_LEN))
                + offset_cost;
            self.relax(
                cost,
                i + SHORT_MATCH_LEN as usize,
                Move::Match {
                    len: SHORT_MATCH_LEN,
                    distance: distance_here,
                },
                new_cache,
            );
        }

        if match_len_here < MIN_MATCH_LEN {
            return;
        }
        let full_len = match_len_as_u32(match_len_here);
        if (full_len as usize) < CARRY_MIN_LEN {
            for &len in &LENGTH_STEPS {
                if len > full_len {
                    break;
                }
                let cost =
                    base + prices.length[bucket(len)] + extra_bits(bucket(len)) + offset_cost;
                self.relax(
                    cost,
                    i + len as usize,
                    Move::Match {
                        len,
                        distance: distance_here,
                    },
                    new_cache,
                );
            }
        }
        let cost =
            base + prices.length[bucket(full_len)] + extra_bits(bucket(full_len)) + offset_cost;
        self.relax(
            cost,
            i + full_len as usize,
            Move::Match {
                len: full_len,
                distance: distance_here,
            },
            new_cache,
        );
    }
}

/// One DP pass: the cheapest token sequence for `data` under `prices`,
/// using [`parse_greedy`]'s hash-chain match finder and the same
/// rep-cache transition rules [`replay`] uses ([`RepCache::push_front`]
/// on a fresh [`Token::Match`], [`RepCache::promote`] on a
/// [`Token::Rep`]) so the chosen tokens replay exactly as this DP
/// costed them.
///
/// This is *not* a full port of the archive's `lz_opt` DP: on a fresh
/// match, the archive's own price simulation updates its rep cache by
/// deduplicating the new distance against the existing three slots
/// (dropping whichever slot already held it, instead of always dropping
/// the oldest). The archive's actual decoder (`decode`, not `lz_opt`)
/// never does this: it always shifts blindly, the same rule [`replay`]
/// already implements via [`RepCache::push_front`]. Porting the dedup
/// rule into this DP would let it choose a later `Token::Rep` slot based
/// on a cache state [`replay`] never reaches, corrupting round-trip
/// exactly when a fresh match's distance happens to coincide with an
/// already-cached one. Hard rule 1 makes that not a judgment call: this
/// DP's cache bookkeeping matches [`replay`] unconditionally instead.
fn dp_round(data: &[u8], prices: &PriceTable) -> Vec<Token> {
    let n = data.len();
    let mut state = DpState::new(n);
    let mut finder = MatchFinder::new(data);
    // (len, distance) of a long match found at the previous position,
    // still long enough at this position to skip a fresh hash-chain walk.
    let mut carry: Option<(usize, Distance)> = None;
    // Per-slot echo of `carry`, for `relax_rep_candidates`' match_len scan
    // (issue #179): unlike parse_greedy, which jumps ahead by a chosen
    // token's length, this loop visits every position, so a long run's
    // rep-length scan needs the same reuse the hash-chain search already
    // gets or it costs O(MAX_MATCH_LEN) per position.
    let mut rep_carry: [Option<(Distance, usize, usize)>; REP_SLOTS] = [None; REP_SLOTS];

    for i in 0..n {
        finder.insert(i);
        if !state.dp[i].is_finite() {
            // Never actually reached: dp[0] = 0 and the literal transition
            // below always advances dp[i] -> dp[i+1] when dp[i] is finite,
            // so every position is reachable by induction. Kept as a
            // defensive guard matching the archive's identical check.
            continue;
        }
        let base = state.dp[i];
        let reps = state.cache[i];

        let context = if i > 0 {
            usize::from(data[i - 1] >> 4)
        } else {
            0
        };
        let literal_cost =
            base + prices.literal[context * 256 + usize::from(data[i])] + FLAG_BIT_PRICE;
        state.relax(literal_cost, i + 1, Move::Literal, reps);

        state.relax_rep_candidates(prices, data, i, base, reps, &mut rep_carry);

        let match_candidate = next_match_candidate(&mut finder, &mut carry, i);
        if let Some((match_len_here, distance_here)) = match_candidate {
            state.relax_match_candidate(prices, i, base, reps, match_len_here, distance_here);
        }
    }

    reconstruct(data, &state.parent)
}

/// The normal-match candidate at position `i`: either the archive's
/// `carry` reuse (a long match found at `i - 1`, one byte shorter here,
/// same distance) or a fresh hash-chain search, which refreshes `carry`
/// for the next position when it finds another long match.
fn next_match_candidate(
    finder: &mut MatchFinder<'_>,
    carry: &mut Option<(usize, Distance)>,
    i: usize,
) -> Option<(usize, Distance)> {
    match *carry {
        Some((len, distance)) if len >= CARRY_MIN_LEN => {
            *carry = (len > CARRY_MIN_LEN).then_some((len - 1, distance));
            Some((len, distance))
        }
        _ => {
            let found = finder.find_best(i, MAX_CHAIN_TRIES_OPTIMAL);
            *carry = found
                .and_then(|(len, distance)| (len >= CARRY_MIN_LEN).then_some((len - 1, distance)));
            found
        }
    }
}

/// [`match_len`] against `distance` at `i`, reusing a previous position's
/// scan on the same distance the way [`next_match_candidate`]'s `carry`
/// reuses the hash-chain search: `match_len(data, i, d)` of `len` implies
/// `match_len(data, i + 1, d)` is at least `len - 1` (the same run of
/// equalities, shifted by one index), so once a scan finds a run at or past
/// [`CARRY_MIN_LEN`], later positions on the same distance decrement
/// instead of re-walking it.
///
/// Searches every entry in `rep_carry`, not just `rep_carry[store_at]`:
/// [`RepCache::promote`]/[`RepCache::push_front`] can move a distance to a
/// different slot index between one position and the next (ties between
/// equal-length candidates are common once a run passes [`MAX_MATCH_LEN`],
/// every slot capping at the same length), and a carry keyed purely by slot
/// index goes stale on every such reorder even though the distance itself,
/// and thus the scan, did not change. A slot-index-only version of this
/// cache regressed to a fresh [`MAX_MATCH_LEN`]-sized scan on most
/// positions past the first reorder, reproducing this function's own bug
/// (issue #179) one level up.
///
/// Each entry stores the position it was last measured at, not a
/// pre-decremented length: a distance can drop out of every slot for a
/// stretch of positions (evicted, then a later match happens to reintroduce
/// the same value) and come back stale relative to a fixed per-step
/// decrement, but `len - (i - measured_at)` is a valid lower bound on
/// `match_len(data, i, distance)` for any `i >= measured_at`, by the same
/// shifted-equalities argument extended from one step to `i - measured_at`
/// steps. On a miss, the fresh scan is recorded at `store_at` (the
/// caller's own slot index): eviction only needs to be cheap, not exact,
/// since a wrongly evicted entry costs one extra scan, not a correctness
/// bug.
fn rep_match_len(
    data: &[u8],
    i: usize,
    distance: Distance,
    rep_carry: &mut [Option<(Distance, usize, usize)>; REP_SLOTS],
    store_at: usize,
) -> usize {
    let hit = rep_carry.iter().enumerate().find_map(|(idx, entry)| {
        let (carried_distance, len, measured_at) = (*entry)?;
        (carried_distance == distance).then(|| (idx, len.saturating_sub(i - measured_at)))
    });
    if let Some((idx, remaining)) = hit
        && remaining >= CARRY_MIN_LEN
    {
        rep_carry[idx] = Some((distance, remaining, i));
        return remaining;
    }
    let len = match_len(data, i, distance);
    rep_carry[store_at] = (len >= CARRY_MIN_LEN).then_some((distance, len, i));
    len
}

/// Walks `parent` backward from `data.len()` to `0`, matching the archive's
/// reconstruction loop: at each step, the recorded move's length says how
/// far back its start was, and (for a literal) `data` at that start says
/// which byte it was.
fn reconstruct(data: &[u8], parent: &[Option<Move>]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut i = data.len();
    while i > 0 {
        let mv = parent[i].expect(
            "every position on the DP path was reached by a recorded relax; dp_round never \
             leaves a gap between 0 and data.len()",
        );
        let len = match mv {
            Move::Literal => 1,
            Move::Match { len, .. } | Move::Rep { len, .. } => len as usize,
        };
        i -= len;
        tokens.push(match mv {
            Move::Literal => Token::Literal(data[i]),
            Move::Match { len, distance } => Token::Match { len, distance },
            Move::Rep { len, slot } => Token::Rep { len, slot },
        });
    }
    tokens.reverse();
    tokens
}

/// Two-round DP-priced optimal parse (`JOURNAL` S1-A3, S2-D2's `lz_opt`
/// slice): a first pass with [`parse_greedy`] seeds a price table; two
/// rounds of `dp_round` each find the min-price path under the current
/// table, and round 0's resulting tokens reseed a sharper table for round
/// 1 (the archive re-derives its price tables from its own DP output
/// exactly once, not iterating to convergence). Below `OPTIMAL_MIN_LEN`
/// (64 bytes) the DP's fixed setup cost isn't worth paying: falls back to
/// [`parse_greedy`] directly, matching the archive.
///
/// See `dp_round`'s docs for one deliberate correctness fix over the
/// archive's own `lz_opt`: this DP's internal rep-cache bookkeeping always
/// matches what [`replay`] will actually do, even where the archive's own
/// price simulation diverges from its real decoder.
///
/// # Panics
///
/// Panics if `data` is longer than `u32::MAX` bytes, the same bound
/// [`parse_greedy`] enforces (this function always calls it first, as its
/// price-seeding first pass).
#[must_use]
pub fn parse_optimal(data: &[u8]) -> Vec<Token> {
    if data.len() < OPTIMAL_MIN_LEN {
        return parse_greedy(data);
    }
    let seed = parse_greedy(data);
    let prices = PriceCounts::tally(&seed, data).prices(seed.len());
    let first_round = dp_round(data, &prices);
    let prices = PriceCounts::tally(&first_round, data).prices(first_round.len());
    dp_round(data, &prices)
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

    fn roundtrip_optimal(data: &[u8]) {
        let tokens = parse_optimal(data);
        assert_eq!(replay(&tokens), data, "optimal-parse roundtrip mismatch");
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
    fn optimal_roundtrip_empty() {
        roundtrip_optimal(b"");
    }

    #[test]
    fn optimal_roundtrip_single_byte() {
        roundtrip_optimal(b"x");
    }

    #[test]
    fn optimal_below_min_len_matches_greedy() {
        // Below OPTIMAL_MIN_LEN, parse_optimal short-circuits straight to
        // parse_greedy: same input, same tokens.
        let data = b"the quick brown fox";
        assert!(data.len() < OPTIMAL_MIN_LEN);
        assert_eq!(parse_optimal(data), parse_greedy(data));
    }

    #[test]
    fn optimal_roundtrip_all_literals_no_repeats() {
        let data = b"the quick brown fox jumps over a lazy dog, then jumps back again";
        assert!(data.len() >= OPTIMAL_MIN_LEN);
        roundtrip_optimal(data);
    }

    #[test]
    fn optimal_roundtrip_simple_repeat_produces_a_match_or_rep() {
        let data = b"abcdefgh".repeat(10);
        let tokens = parse_optimal(&data);
        assert_eq!(replay(&tokens), data);
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Match { .. } | Token::Rep { .. })),
            "a 10x repeat of an 8-byte pattern should produce at least one match or rep token"
        );
    }

    #[test]
    fn optimal_roundtrip_run_length_exercises_overlapping_distance() {
        // distance 1, shorter than the match length: copy_match must read
        // bytes it just wrote.
        roundtrip_optimal(&vec![b'a'; 1000]);
    }

    #[test]
    fn optimal_roundtrip_long_run_of_one_repeated_byte_stays_linear() {
        // Regression test for issue #179. dp_round visits every position
        // (needed to consider every possible token start, unlike
        // parse_greedy, which jumps ahead by a whole match's length), and
        // used to price every rep-cache slot at each one via a fresh
        // match_len scan with no carry-reuse equivalent to
        // next_match_candidate's: on a single-byte run the scan cost stayed
        // near MAX_MATCH_LEN at every position, making total cost quadratic
        // in the run length once it passed MAX_MATCH_LEN. 200,000 bytes
        // matches the issue's own repro (mirroring parse_greedy's own
        // 200,000-byte same-byte-run test); before the fix this took over
        // 60 seconds and was killed. The bound below is generous (the fixed
        // version measures under 2s in an unoptimized debug build on
        // ordinary hardware) so a slower CI runner doesn't flake, while
        // still failing well before a regression to the old quadratic cost
        // would let it run to completion.
        let data = vec![b'z'; 200_000];
        let start = std::time::Instant::now();
        roundtrip_optimal(&data);
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(15),
            "parse_optimal on a 200,000-byte single-byte run took {elapsed:?}, expected well \
             under 15s; likely a regression to issue #179's quadratic rep-candidate scan"
        );
    }

    #[test]
    fn optimal_roundtrip_alternating_pattern_exercises_rep_cache() {
        let mut data = b"AB".repeat(50);
        data.extend(b"CD".repeat(50));
        let tokens = parse_optimal(&data);
        assert_eq!(replay(&tokens), data);
        assert!(tokens.iter().any(|t| matches!(t, Token::Rep { .. })));
    }

    #[test]
    fn optimal_roundtrip_cyclic_data() {
        let data: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        roundtrip_optimal(&data);
    }

    #[test]
    fn optimal_roundtrip_structured_repeats_at_varying_distances() {
        let base: Vec<u8> = (b'a'..=b'z').collect();
        let mut data = base.clone();
        data.push(b'-');
        data.extend_from_slice(&base[1..]);
        data.push(b'+');
        data.extend_from_slice(&base);
        data.push(b'~');
        data.extend_from_slice(&base);
        assert!(data.len() >= OPTIMAL_MIN_LEN);
        roundtrip_optimal(&data);
    }

    #[test]
    fn optimal_roundtrip_binary_data_with_zero_bytes() {
        let data: Vec<u8> = (0..1000u32)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();
        roundtrip_optimal(&data);
    }

    #[test]
    fn optimal_roundtrip_short_close_repeats() {
        // Dense 3-byte repeats at a distance under SHORT_MATCH_MAX_DISTANCE:
        // exercises dp_round's length-3 short-match candidate.
        let data = b"xyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyzxyz".to_vec();
        assert!(data.len() >= OPTIMAL_MIN_LEN);
        roundtrip_optimal(&data);
    }

    #[test]
    fn optimal_roundtrip_random_like_binary_never_worse_than_stored() {
        // A pseudo-random byte stream (no real structure): the DP must
        // still round-trip even when literals dominate.
        let data: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
            .take(500)
            .map(|state| u8::try_from(state % 256).unwrap())
            .collect();
        roundtrip_optimal(&data);
    }

    /// Independent reference for [`binary_tree_matches_brute_force`]:
    /// scans every earlier position sharing `i`'s hash bucket directly
    /// instead of walking a tree. Restricted to the same bucket because
    /// that's the population [`BinaryTreeMatchFinder`] (and
    /// [`MatchFinder`]) can ever see in the first place — a match whose
    /// candidate has a different 3-byte prefix hash than `i`'s is
    /// invisible to either, by construction, same as a length below 3
    /// bytes never entering either structure's hash table.
    fn brute_force_best(data: &[u8], i: usize) -> Option<(usize, Distance)> {
        let target_hash = prefix_hash(data, i);
        let mut best_len = 0usize;
        let mut best_distance = None;
        for cur in 0..i {
            if prefix_hash(data, cur) != target_hash {
                continue;
            }
            let len = suffix_common_len(data, cur, i);
            if len > best_len {
                best_len = len;
                best_distance = NonZeroU32::new(to_u32(i - cur));
            }
        }
        best_distance.map(|distance| (best_len, distance))
    }

    #[test]
    fn binary_tree_no_prior_positions_returns_none() {
        let data = b"abcdef";
        let mut finder = BinaryTreeMatchFinder::new(data);
        assert_eq!(finder.insert_and_find(0, data.len()), None);
    }

    #[test]
    fn binary_tree_finds_an_exact_repeat() {
        let data = b"abcabcabc";
        let mut finder = BinaryTreeMatchFinder::new(data);
        let mut found_at_6 = None;
        for i in 0..data.len() {
            let found = finder.insert_and_find(i, data.len());
            if let Some((len, distance)) = found {
                assert_eq!(
                    match_len(data, i, distance),
                    len,
                    "position {i}: reported length must be a real match at the reported distance"
                );
            }
            if i == 6 {
                found_at_6 = found;
            }
        }
        let (len, distance) = found_at_6.expect("position 6 repeats position 3's \"abc\"");
        assert_eq!(distance.get(), 3);
        assert_eq!(len, 3);
    }

    #[test]
    fn binary_tree_matches_brute_force() {
        // A small byte alphabet keeps 3-byte prefixes colliding often, so
        // hash buckets build up real tree structure to walk, unlike a
        // near-uniform 0..256 stream where most buckets stay tiny.
        let data: Vec<u8> = crate::test_support::Xorshift32::new(0xB17E_5EED)
            .take(400)
            .map(|state| u8::try_from(state % 5).unwrap())
            .collect();
        let mut finder = BinaryTreeMatchFinder::new(&data);
        for i in 0..data.len() {
            // max_depth covers every possible candidate in the bucket
            // (at most i of them), so the walk cannot be truncated before
            // it would reach the true best.
            let found = finder.insert_and_find(i, data.len());
            let expected_len = brute_force_best(&data, i).map(|(len, _)| len);
            assert_eq!(
                found.map(|(len, _)| len),
                expected_len,
                "position {i}: best length must equal a brute-force scan when max_depth covers every candidate"
            );
            if let Some((len, distance)) = found {
                assert_eq!(
                    match_len(&data, i, distance),
                    len,
                    "position {i}: reported length must be a real match at the reported distance"
                );
            }
        }
    }

    #[test]
    fn binary_tree_zero_max_depth_finds_nothing_but_stays_consistent() {
        let data = b"abcabcabc";
        let mut finder = BinaryTreeMatchFinder::new(data);
        let _ = finder.insert_and_find(0, data.len());
        let _ = finder.insert_and_find(1, data.len());
        let _ = finder.insert_and_find(2, data.len());
        // Position 3 shares position 0's hash bucket ("abc"); max_depth 0
        // must not panic, and correctly reports no match even though a
        // real one exists.
        assert_eq!(finder.insert_and_find(3, 0), None);
        // A later insert with a different hash bucket ("bca", shared with
        // position 1) is unaffected and still finds its match.
        assert!(finder.insert_and_find(4, data.len()).is_some());
    }

    #[test]
    fn binary_tree_caps_match_length_at_max_match_len() {
        let mut data = vec![b'x'; MAX_MATCH_LEN + 50];
        data.push(b'y');
        let mut finder = BinaryTreeMatchFinder::new(&data);
        let _ = finder.insert_and_find(0, 8);
        let (len, _distance) = finder
            .insert_and_find(1, 8)
            .expect("position 1 repeats position 0's run of 'x'");
        assert!(len <= MAX_MATCH_LEN);
    }
}
