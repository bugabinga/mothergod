//! Hash-based match model: [`MatchModel`], a from-scratch context-hash
//! predictor for ROADMAP M3's eighth standing lead (`JOURNAL` S1-P8,
//! "GLN-style predictors / more experts"), independent of [`crate::lz`]'s
//! own optimal parse. The kind lpaq/zpaq/cmix call a "match model": a hash
//! table over the last few already-coded bytes, mapping that context to
//! the position it last occurred at, so a repeated context can predict the
//! byte that followed it before.
//!
//! **Why a second match mechanism, given [`crate::lz`] already finds
//! matches.** `research/JOURNAL.md` S2-R20 and S2-R22 both tried LZMA's own
//! "matched literal" byte — the byte at the LZ parse's current rep0
//! distance back from the position being coded — as an expert signal
//! (once as an additive mixer weight, once as an SSE calibration axis) and
//! both were rejected for the identical reason: real signal on
//! record-repeat data (`sqlite_like_records`, `json_records`), but a
//! regression on `gradient_image` (sealed), whose short matches/reps are
//! "largely incidental byte coincidences" on a smooth noisy surface rather
//! than genuine structural repeats — S2-R22's own closing line named this
//! as a caution for "any future S1-P3/S1-P8 slice that reaches for this
//! same byte again." This module deliberately does not: it never reads
//! `crate::lz::RepCache`, a [`crate::lz::Token::Match`]/`Token::Rep`, or
//! any LZ parse decision. Its own hash table is built purely from the
//! decoded byte stream, and its own confidence signal — how many
//! consecutive bytes a candidate has predicted correctly so far,
//! [`MatchModel::predict`]'s returned length — is meant to let a genuinely
//! long, confirmed run earn a mixing weight a short or freshly-guessed one
//! never does (`crate::literal`'s `MATCH_LEN_BUCKETS`, one axis
//! S2-R20/S2-R22's own unconditional signal never had).
//!
//! **Causality.** [`MatchModel::predict`] only ever reads `data[..position]`
//! (never `data[position]` or later): the byte being predicted is always
//! strictly in the model's own future relative to what it consults, the
//! same constraint a real encoder (which has not yet chosen the byte) and
//! a real decoder (which has not yet decoded it) both share. Every already
//! coded byte, whether it was originally coded as a literal or replayed
//! inside an LZ copy token, is available in `data` at its own position, so
//! the hash table indexes the whole reconstructed stream, not just the
//! literal sub-stream — matching how [`crate::literal::Context`]'s own
//! `prev1`/`prev2` fields already track bytes across copy tokens.

/// Bytes of trailing context [`MatchModel`] hashes to find a candidate.
/// Short enough that a genuine repeat need not be long to be found at all
/// (the model's own confirmed-length signal, not this constant, is what
/// separates a genuine repeat from an incidental one); matches this
/// project's other short-context experts in order of magnitude (`crate::
/// literal`'s order-2 bank keys on 2 bytes).
const CONTEXT_LEN: usize = 4;

/// `log2` of [`MatchModel`]'s hash table size. 16 bits (65,536 slots) is a
/// small fixed allocation; a hash collision here (two different
/// [`CONTEXT_LEN`]-byte contexts landing in the same slot) is expected at
/// this project's train-slice scale (`CASE_LEN`, 50,000 bytes) and is not
/// itself a correctness problem — [`MatchModel::predict`] compares the
/// actual context bytes before trusting a stored position, so a collision
/// is rejected, never mistaken for a real repeat.
const TABLE_BITS: u32 = 16;
/// Hash table size implied by [`TABLE_BITS`].
const TABLE_SIZE: usize = 1 << TABLE_BITS;

/// FNV-1a over `context`'s bytes, masked into `[0, TABLE_SIZE)`. Not
/// cryptographic and not incremental (recomputed fresh over
/// [`CONTEXT_LEN`] bytes at every candidate position): this only needs an
/// even spread over a small table, never collision resistance, and
/// [`CONTEXT_LEN`] is short enough that recomputing costs nothing an
/// incremental rolling hash would meaningfully save.
fn hash_context(context: &[u8]) -> usize {
    const FNV_OFFSET_BASIS: u32 = 0x811c_9dc5;
    const FNV_PRIME: u32 = 0x0100_0193;
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in context {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    (hash as usize) & (TABLE_SIZE - 1)
}

/// A from-scratch context-hash match predictor over an arbitrary byte
/// buffer. See the module docs for the mechanism and why it is
/// independent of [`crate::lz`]'s own parse.
#[derive(Debug, Clone)]
pub struct MatchModel {
    /// One slot per hashed context: `0` means "never indexed", otherwise
    /// `position + 1` of the most recent occurrence (the `+1` frees `0` as
    /// the empty sentinel, since position `0` is itself a valid index).
    table: Vec<u32>,
    /// The history position currently being followed (its own next byte,
    /// `data[ptr]`, is this model's current prediction), or `None` when no
    /// candidate is active.
    ptr: Option<u32>,
    /// How many consecutive predictions this `ptr`'s lineage has gotten
    /// right so far; `0` for a candidate just found by a fresh hash lookup
    /// (unconfirmed) as well as for no active candidate at all — the two
    /// are distinguished by [`MatchModel::predict`]'s `Option`, not by this
    /// field alone.
    len: u32,
    /// How many positions of a `data` buffer this model has already folded
    /// into `table`; [`MatchModel::index_up_to`]'s own resume point, so
    /// repeated [`MatchModel::predict`] calls over a growing prefix of the
    /// same buffer never re-index a position twice.
    indexed_up_to: usize,
}

impl MatchModel {
    /// A fresh model: no history indexed, no candidate active.
    #[must_use]
    pub fn new() -> Self {
        Self {
            table: vec![0u32; TABLE_SIZE],
            ptr: None,
            len: 0,
            indexed_up_to: 0,
        }
    }

    /// Folds every not-yet-indexed position of `data` up to (excluding)
    /// `target` into [`Self::table`]. Safe to call with a `target` at or
    /// below [`Self::indexed_up_to`] (a no-op then), or past `data.len()`
    /// (clamped: a position needs its own `data[position]` to exist before
    /// it can be indexed at all, so this never reads past the buffer);
    /// [`Self::predict`] always calls this first so its own lookups only
    /// ever see positions strictly before the one being predicted.
    fn index_up_to(&mut self, data: &[u8], target: usize) {
        let target = target.min(data.len());
        while self.indexed_up_to < target {
            let position = self.indexed_up_to;
            if position >= CONTEXT_LEN {
                let hash = hash_context(&data[position - CONTEXT_LEN..position]);
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "position is an index into this experiment's own train/sealed cases (CASE_LEN 50,000, far below u32::MAX); this module is not wired to real decode this slice, so no adversarial length reaches it"
                )]
                {
                    self.table[hash] = position as u32 + 1;
                }
            }
            self.indexed_up_to += 1;
        }
    }

    /// Predicts the byte at `position` in `data` from history strictly
    /// before it, and how many consecutive bytes the candidate producing
    /// that prediction has gotten right so far (`0` for a fresh,
    /// unconfirmed candidate). `None` when no candidate context has ever
    /// recurred: this model is silent exactly where it has nothing to say,
    /// never a guess dressed as a floor (the same shape `crate::literal`'s
    /// `PpmExpertState` already established for its own zero-frequency
    /// "unseen" case).
    ///
    /// Mutates this model's own `ptr`/`len` bookkeeping (continuing an
    /// active candidate, or replacing it with a freshly hashed one) but
    /// never `table`; call [`Self::observe`] once the actual byte at
    /// `position` is known to record whether this prediction held and to
    /// let [`Self::index_up_to`] fold `position` itself in on the next
    /// call.
    ///
    /// # Panics
    ///
    /// Never: every slice this indexes (`data[position - CONTEXT_LEN
    /// ..position]`) is guarded by the same bound check that selects it,
    /// and every table lookup is masked into `table`'s own fixed length by
    /// [`hash_context`].
    #[must_use]
    pub fn predict(&mut self, data: &[u8], position: usize) -> Option<(u8, u32)> {
        self.index_up_to(data, position);

        if let Some(ptr) = self.ptr {
            let candidate = ptr as usize;
            if candidate < position && candidate < data.len() {
                return Some((data[candidate], self.len));
            }
        }

        if position >= CONTEXT_LEN && position <= data.len() {
            let this_context = &data[position - CONTEXT_LEN..position];
            let hash = hash_context(this_context);
            let stored = self.table[hash];
            if stored != 0 {
                let candidate = (stored - 1) as usize;
                // A hash match alone is not enough: TABLE_SIZE (65,536) is
                // far smaller than the number of 4-byte contexts a real
                // file's length can produce, so most hash hits on
                // low-structure data are collisions, not genuine repeats.
                // Comparing the actual CONTEXT_LEN bytes rejects a
                // collision instead of letting it masquerade as a "fresh"
                // (len 0) candidate — without this check, every collision
                // still counts as a confident guess until this expert's
                // own weight has paid enough wrong predictions to learn
                // otherwise.
                if candidate >= CONTEXT_LEN
                    && candidate < position
                    && data[candidate - CONTEXT_LEN..candidate] == *this_context
                {
                    self.ptr = Some(stored - 1);
                    self.len = 0;
                    return Some((data[candidate], 0));
                }
            }
        }

        self.ptr = None;
        self.len = 0;
        None
    }

    /// Records whether `actual` (the real byte at the position
    /// [`Self::predict`] was just called for) matches `predicted` (that
    /// call's own return value): a hit extends `ptr` to follow the same
    /// candidate one byte further and grows `len`; anything else (a miss,
    /// or no candidate at all) drops the candidate, so the next
    /// [`Self::predict`] call starts a fresh hash lookup.
    pub fn observe(&mut self, predicted: Option<(u8, u32)>, actual: u8) {
        match (self.ptr, predicted) {
            (Some(ptr), Some((predicted_byte, _))) if predicted_byte == actual => {
                self.ptr = Some(ptr + 1);
                self.len = self.len.saturating_add(1);
            }
            _ => {
                self.ptr = None;
                self.len = 0;
            }
        }
    }
}

impl Default for MatchModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicts_nothing_before_enough_context_exists() {
        let data = b"ab";
        let mut model = MatchModel::new();
        assert_eq!(model.predict(data, 0), None);
        assert_eq!(model.predict(data, 1), None);
    }

    #[test]
    fn predicts_nothing_on_an_empty_buffer() {
        let mut model = MatchModel::new();
        assert_eq!(model.predict(&[], 0), None);
    }

    #[test]
    fn predicts_nothing_the_first_time_a_context_is_seen() {
        // "abcd" followed by 'X' has never occurred before position 4, so
        // there is nothing yet to predict from.
        let data = b"abcdX";
        let mut model = MatchModel::new();
        assert_eq!(model.predict(data, 4), None);
    }

    #[test]
    fn predicts_the_byte_that_followed_a_repeated_context_last_time() {
        // "abcd" is followed by 'X' at position 4, then recurs at position
        // 9 (data[5..9] == "abcd" again); by the time the model reaches
        // position 9, it should predict 'X' from the first occurrence.
        let data = b"abcdXabcd";
        let mut model = MatchModel::new();
        for position in 0..9 {
            let predicted = model.predict(data, position);
            model.observe(predicted, data[position]);
        }
        let predicted = model.predict(data, 9);
        assert_eq!(predicted.map(|(byte, _)| byte), Some(b'X'));
    }

    #[test]
    fn match_length_grows_while_predictions_keep_hitting() {
        // Two back-to-back copies of a 6-byte pattern: once the model
        // locks onto the first copy at the second copy's start, every
        // subsequent byte in the second copy should extend the same
        // candidate and grow `len`.
        let pattern = b"abcdef";
        let mut data = pattern.to_vec();
        data.extend_from_slice(pattern);
        data.extend_from_slice(pattern);

        let mut model = MatchModel::new();
        let mut lengths = Vec::new();
        for position in 0..data.len() {
            let predicted = model.predict(data.as_slice(), position);
            if let Some((_, len)) = predicted {
                lengths.push(len);
            }
            model.observe(predicted, data[position]);
        }
        // Once a candidate is confirmed correct repeatedly, len should
        // reach at least a few consecutive hits within the third copy.
        assert!(lengths.iter().any(|&len| len >= 3), "lengths={lengths:?}");
    }

    #[test]
    fn a_miss_resets_the_candidate_and_length() {
        let data = b"abcdXabcdY";
        let mut model = MatchModel::new();
        for position in 0..9 {
            let predicted = model.predict(data, position);
            model.observe(predicted, data[position]);
        }
        // Position 9 predicts 'X' (from the first "abcd" occurrence), but
        // the actual byte is 'Y': a miss.
        let predicted_at_9 = model.predict(data, 9);
        assert_eq!(predicted_at_9.map(|(byte, _)| byte), Some(b'X'));
        model.observe(predicted_at_9, data[9]);
        assert_eq!(model.ptr, None);
        assert_eq!(model.len, 0);
    }

    #[test]
    fn never_predicts_a_context_whose_only_occurrence_is_still_ahead() {
        // At position 4, the only occurrence of context "wxyz" indexed so
        // far is none at all: the buffer's second "wxyz" (positions 5..9)
        // has not been reached yet, so a causal model has nothing to find.
        let data = b"wxyzAwxyzB";
        let mut model = MatchModel::new();
        assert_eq!(model.predict(data, 4), None);
    }

    #[test]
    fn predict_alone_backfills_positions_a_copy_token_would_skip() {
        // codec.rs's real sink only calls predict()/observe() for literal
        // bytes, never for the bytes an LZ copy token replays, yet the
        // hash table must still index those positions too (this model's
        // own history is the whole reconstructed byte stream, not just
        // the literal sub-stream, `crate::literal::Context`'s own
        // convention). Simulate that gap directly: predict() at position 9
        // is called on a fresh model with no prior predict/observe calls
        // at all, so any indexing it needs must come from its own
        // backfill, not from having been walked one byte at a time.
        let data = b"wxyzAwxyzB";
        let mut model = MatchModel::new();
        let predicted = model.predict(data, 9);
        assert_eq!(predicted.map(|(byte, _)| byte), Some(b'A'));
    }

    #[test]
    fn repeated_predict_calls_at_the_same_position_are_stable() {
        let data = b"abcdXabcd";
        let mut model = MatchModel::new();
        for position in 0..9 {
            let predicted = model.predict(data, position);
            model.observe(predicted, data[position]);
        }
        let first = model.predict(data, 9);
        let second = model.predict(data, 9);
        assert_eq!(first, second);
    }
}
