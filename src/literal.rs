//! Six-expert context-mixing literal model: [`Literal`], the S2-D2
//! entropy stage for the bytes an LZ parse ([`crate::lz`]) leaves as
//! literals, once the length/offset/flag streams (each a plain
//! [`crate::model::Model`]) have claimed everything else.
//!
//! Ported from the archive's `Lit`
//! (`research/imports/session-1/mothergod.rs`), not the code, per
//! ADR-0006: six order-N context predictors ("experts") each keep their
//! own order-0 frequency table over the 256 byte values in whatever
//! context they're keyed on (a rolling two-rate fast/slow order-1 hash,
//! an order-0 catch-all, a 12-bit order-2 hash, a position/nibble
//! "alignment" hash, and a 12-bit alnum-only rolling word hash), and
//! their per-symbol counts blend under weights the coder adapts after
//! every symbol via an exponentiated-gradient rule (Mahoney 2005),
//! context-sensitive: the weight vector itself is selected by a small
//! `(prev-byte nibble, after-copy)` key, so the mixer favors different
//! experts after a match than after a run of literals (`JOURNAL` S1-A4).
//!
//! **Known deviation, not yet resolved (`JOURNAL` S2-D3).** `JOURNAL`
//! S1-A5 records an "integer-only probability path" as accepted
//! architecture, specifically to retire the cross-platform determinism
//! hazard of `f64::exp()` (not guaranteed bit-identical across libm
//! implementations). That refactor postdates this archive (it is
//! transcript-only, per `research/imports/session-1/README.md`), so no
//! artifact of it exists to port from. This module keeps the archive's
//! `f64` weight update verbatim rather than inventing a fixed-point
//! replacement from scratch. It carries no live risk yet: nothing in
//! `src/` calls this module (same as every other S2-D2 slice), so no
//! bitstream depends on it. The eventual `Method`-wiring PR needs an ADR
//! and a `FORMAT_VERSION` bump anyway (hard rule 5, `CLAUDE.md`);
//! resolving `f64` vs. fixed-point belongs there, before any real frame
//! depends on bit-identical adaptive state across platforms.

use crate::coder::{Decoder, Encoder};

/// Number of context predictors blended for every literal byte.
const EXPERTS: usize = 6;

/// Number of distinct mixing-weight vectors: one per `(prev-byte nibble,
/// after-copy)` key (`JOURNAL` S1-A4's "context-sensitive MIX weights").
const WEIGHT_CONTEXTS: usize = 32;

/// Byte alphabet every context bank models.
const ALPHABET: usize = 256;
/// [`ALPHABET`] as a `u32`, spelled as its own literal instead of a cast
/// so no truncation lint applies to a compile-time-obvious value.
const ALPHABET_U32: u32 = 256;

// Bank layout, ported from the archive's `O_CF`/`O_CS`/`O_O0`/`O_O2`/
// `O_AL`/`O_WD`/`NB` (`research/imports/session-1/mothergod.rs`): one
// contiguous bank space, sliced per expert so [`banks`] can address any
// of them with a single base offset.
const FAST_BASE: usize = 0;
const FAST_BANKS: usize = 512;
const SLOW_BASE: usize = FAST_BASE + FAST_BANKS;
const SLOW_BANKS: usize = 512;
const ORDER0_BASE: usize = SLOW_BASE + SLOW_BANKS;
const ORDER0_BANKS: usize = 1;
const ORDER2_BASE: usize = ORDER0_BASE + ORDER0_BANKS;
const ORDER2_BANKS: usize = 4096;
const ALIGN_BASE: usize = ORDER2_BASE + ORDER2_BANKS;
const ALIGN_BANKS: usize = 64;
const WORD_BASE: usize = ALIGN_BASE + ALIGN_BANKS;
const WORD_BANKS: usize = 4096;
/// Total context banks across all six experts.
const BANKS: usize = WORD_BASE + WORD_BANKS;

/// Frequency increment for the fast-rate context expert (bank 0):
/// higher increment and lower rescale ceiling than the other five, so
/// it tracks recent bytes hardest and forgets fastest. Ported unchanged
/// from the archive's inline `(32u32, 6144u32)` special case for `e==0`.
const FAST_INCREMENT: u32 = 32;
const FAST_LIMIT: u32 = 6144;

/// Frequency increment and rescale ceiling for the other five experts.
/// Ported unchanged from the archive's `INC`/`LIM`, the same values
/// [`crate::model::Model`] uses for the flag/length/offset stages.
const DEFAULT_INCREMENT: u32 = 12;
const DEFAULT_LIMIT: u32 = 65536;

/// Exponentiated-gradient learning rate and weight clamp, ported
/// unchanged from the archive's inline `0.05`/`1e-4`/`1e4`.
const LEARNING_RATE: f64 = 0.05;
const MIN_WEIGHT: f64 = 1e-4;
const MAX_WEIGHT: f64 = 1e4;
/// Floor under the mixed-probability denominator so a weight update
/// never divides by (near) zero. Ported unchanged from the archive's
/// inline `1e-9`.
const MIN_DENOMINATOR: f64 = 1e-9;

/// `1 << 32` as a float, the fixed-point scale [`Literal::mix`] blends
/// expert probabilities under. Spelled as a literal instead of a cast so
/// no `u64 -> f64` conversion lint applies to what is, exactly, a power
/// of two well inside `f64`'s 53-bit mantissa.
const FIXED_POINT_SCALE: f64 = 4_294_967_296.0;

/// Per-byte modeling context [`Literal::encode`]/[`Literal::decode`]
/// read to select which banks blend at this position: the previous two
/// bytes (`0` before the start of output, matching the archive's
/// `fd[pos-1]`/`fd[pos-2]` boundary convention), the output position,
/// whether the previous token was a copy (LZ match or rep) rather than
/// a literal, and the rolling alnum-only word hash
/// ([`advance_word_hash`]).
///
/// [`Self::after_literal`]/[`Self::after_copy`] compute the next context
/// the same way the archive's `encode_body`/`decode` update
/// `(b1, b2, pos, am, wh)` after every token, so a caller driving an
/// encode pass and a decode pass from the same token stream reuses one
/// update rule instead of two copies that could drift apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Context {
    /// The byte immediately before `position`, or `0` at the start of
    /// output.
    pub prev1: u8,
    /// The byte two before `position`, or `0` near the start of output.
    pub prev2: u8,
    /// How many bytes precede this one in the output.
    pub position: usize,
    /// Whether the token immediately before this byte was a copy (match
    /// or rep) rather than a literal.
    pub after_copy: bool,
    /// Rolling hash over the alnum run leading up to this position
    /// (`JOURNAL` S1-A4's word-hash expert).
    pub word_hash: u32,
}

impl Context {
    /// The context for the byte after `byte` was coded as a literal.
    #[must_use]
    pub fn after_literal(self, byte: u8) -> Self {
        Self {
            prev1: byte,
            prev2: self.prev1,
            position: self.position + 1,
            after_copy: false,
            word_hash: advance_word_hash(self.word_hash, byte),
        }
    }

    /// The context for the byte after a copy (match or rep) token that
    /// replayed `bytes`.
    #[must_use]
    pub fn after_copy(self, bytes: &[u8]) -> Self {
        let word_hash = bytes
            .iter()
            .fold(self.word_hash, |wh, &b| advance_word_hash(wh, b));
        let (prev1, prev2) = match bytes.len() {
            0 => (self.prev1, self.prev2),
            1 => (bytes[0], self.prev1),
            n => (bytes[n - 1], bytes[n - 2]),
        };
        Self {
            prev1,
            prev2,
            position: self.position + bytes.len(),
            after_copy: true,
            word_hash,
        }
    }
}

/// Advances a rolling word hash by one byte. Ported unchanged from the
/// archive's `whup`: only alphanumeric bytes extend the hash, anything
/// else resets it to `0`, so the hash tracks how far into the current
/// alnum run a position is, never text spanning punctuation.
#[must_use]
pub fn advance_word_hash(word_hash: u32, byte: u8) -> u32 {
    if byte.is_ascii_alphanumeric() {
        word_hash.wrapping_mul(61).wrapping_add(u32::from(byte))
    } else {
        0
    }
}

/// Bank indices for [`Context`]'s six experts, and the mixing-weight
/// index that goes with them. Ported unchanged from the archive's
/// `Lit::banks`.
fn banks(context: Context) -> ([usize; EXPERTS], usize) {
    let prev1 = usize::from(context.prev1);
    let prev2 = usize::from(context.prev2);
    let after_copy = usize::from(context.after_copy);
    let rate_context = prev1 | (after_copy * 256);
    let order2 = ((prev1 << 8) | prev2) & 0xFFF;
    let align = ((context.position & 3) << 4) | (prev1 >> 4);
    let word_hash =
        usize::try_from(context.word_hash & 0xFFF).expect("masked to 12 bits, always fits usize");
    let weight_index = (prev1 >> 4) | (after_copy * 16);
    (
        [
            FAST_BASE + rate_context,
            SLOW_BASE + rate_context,
            ORDER0_BASE,
            ORDER2_BASE + order2,
            ALIGN_BASE + align,
            WORD_BASE + word_hash,
        ],
        weight_index,
    )
}

/// Six-expert context-mixing model over literal bytes. See the module
/// docs for the port source and the open `f64` determinism question.
#[derive(Debug, Clone)]
pub struct Literal {
    /// `BANKS * ALPHABET` per-symbol frequencies, bank-major.
    freq: Vec<u32>,
    /// Per-bank frequency totals; always equals the sum of that bank's
    /// 256 `freq` entries, the same invariant [`crate::model::Model`]
    /// leans on for panic-free decoding.
    total: Vec<u32>,
    /// Per-weight-context mixing weights, one `[f64; EXPERTS]` per
    /// [`WEIGHT_CONTEXTS`] key.
    weights: Vec<[f64; EXPERTS]>,
}

impl Default for Literal {
    fn default() -> Self {
        Self::new()
    }
}

impl Literal {
    /// A fresh model: every bank starts at frequency 1 per symbol (total
    /// 256, nothing ever impossible to code), every mixing weight starts
    /// at 1.0 (experts start equally trusted).
    #[must_use]
    pub fn new() -> Self {
        Self {
            freq: vec![1u32; BANKS * ALPHABET],
            total: vec![ALPHABET_U32; BANKS],
            weights: vec![[1.0; EXPERTS]; WEIGHT_CONTEXTS],
        }
    }

    /// Blends the six experts' banks under the current mixing weights
    /// into a cumulative-frequency table over the 256 byte values.
    /// Ported unchanged from the archive's `Lit::cum`: every symbol's
    /// mixed count gets `+1` (a Laplace floor, the same "nothing is ever
    /// impossible to code" guarantee [`crate::model::Model`] gives by
    /// starting every frequency at 1), so `cum` is always strictly
    /// increasing and `cum[ALPHABET]` is always the true total passed to
    /// the coder.
    fn mix(&self, bank_indices: &[usize; EXPERTS], weight_index: usize) -> [u64; ALPHABET + 1] {
        let weights = &self.weights[weight_index];
        let weight_sum: f64 = weights.iter().sum();
        let mut scale = [0u64; EXPERTS];
        for expert in 0..EXPERTS {
            let normalized = weights[expert] / weight_sum;
            let bank_total = f64::from(self.total[bank_indices[expert]]);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "fixed-point scale factor: normalized weight is in (0,1], bank_total > 0, the product is always non-negative and truncation is the intended floor"
            )]
            {
                scale[expert] = ((normalized * FIXED_POINT_SCALE) / bank_total) as u64;
            }
        }
        let mut cum = [0u64; ALPHABET + 1];
        let mut acc = 0u64;
        for symbol in 0..ALPHABET {
            let mut mixed = 0u64;
            for expert in 0..EXPERTS {
                let freq = u64::from(self.freq[bank_indices[expert] * ALPHABET + symbol]);
                mixed += scale[expert] * freq;
            }
            acc += (mixed >> 16) + 1;
            cum[symbol + 1] = acc;
        }
        cum
    }

    /// Adapts mixing weights toward whichever experts predicted `symbol`
    /// best (exponentiated gradient, Mahoney 2005), then updates every
    /// expert's own frequency table the same way
    /// [`crate::model::Model::encode`]/`decode` do. Ported unchanged
    /// from the archive's `Lit::upd`.
    fn update(&mut self, bank_indices: &[usize; EXPERTS], weight_index: usize, symbol: usize) {
        let mut estimate = [0f64; EXPERTS];
        for (expert, bank) in bank_indices.iter().enumerate() {
            estimate[expert] =
                f64::from(self.freq[bank * ALPHABET + symbol]) / f64::from(self.total[*bank]);
        }
        let weights = &mut self.weights[weight_index];
        let weight_sum: f64 = weights.iter().sum();
        let mixed: f64 = (0..EXPERTS)
            .map(|expert| weights[expert] * estimate[expert])
            .sum::<f64>()
            / weight_sum;
        let denominator = mixed.max(MIN_DENOMINATOR);
        for expert in 0..EXPERTS {
            let gradient = LEARNING_RATE * (estimate[expert] - mixed) / denominator;
            weights[expert] = (weights[expert] * gradient.exp()).clamp(MIN_WEIGHT, MAX_WEIGHT);
        }
        for (expert, &bank) in bank_indices.iter().enumerate() {
            let (increment, limit) = if expert == 0 {
                (FAST_INCREMENT, FAST_LIMIT)
            } else {
                (DEFAULT_INCREMENT, DEFAULT_LIMIT)
            };
            self.freq[bank * ALPHABET + symbol] += increment;
            self.total[bank] += increment;
            if self.total[bank] > limit {
                let mut total = 0u32;
                for f in &mut self.freq[bank * ALPHABET..bank * ALPHABET + ALPHABET] {
                    *f = (*f + 1) >> 1;
                    total += *f;
                }
                self.total[bank] = total;
            }
        }
    }

    /// Codes `byte` through `encoder` under `context`, then updates
    /// every expert bank and the mixing weights.
    pub fn encode(&mut self, encoder: &mut Encoder, context: Context, byte: u8) {
        let (bank_indices, weight_index) = banks(context);
        let cum = self.mix(&bank_indices, weight_index);
        let symbol = usize::from(byte);
        encoder.encode(cum[symbol], cum[symbol + 1], cum[ALPHABET]);
        self.update(&bank_indices, weight_index, symbol);
    }

    /// Decodes one byte from `decoder` under `context`, then updates the
    /// model the same way [`Self::encode`] did, keeping both sides in
    /// lockstep.
    ///
    /// Never panics on adversarial `decoder` state: [`Decoder::target`]
    /// is mathematically bounded to `[0, total)`, and the mixed
    /// cumulative-frequency table is built so every symbol contributes
    /// at least `1`, so `cum[ALPHABET]` always exceeds any in-range
    /// target and the scan below always finds a symbol before running
    /// past the table.
    #[must_use]
    pub fn decode(&mut self, decoder: &mut Decoder, context: Context) -> u8 {
        let (bank_indices, weight_index) = banks(context);
        let cum = self.mix(&bank_indices, weight_index);
        let total = cum[ALPHABET];
        let target = decoder.target(total);
        let mut symbol = 0usize;
        while cum[symbol + 1] <= target {
            symbol += 1;
        }
        decoder.decode(cum[symbol], cum[symbol + 1], total);
        self.update(&bank_indices, weight_index, symbol);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "symbol is a scan index bounded by ALPHABET (256), always fits u8"
        )]
        {
            symbol as u8
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_bytes(bytes: &[u8]) {
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in bytes {
            model.encode(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let encoded = enc.finish();

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(&encoded);
        let mut got = Vec::with_capacity(bytes.len());
        for _ in bytes {
            let b = model.decode(&mut dec, context);
            context = context.after_literal(b);
            got.push(b);
        }
        assert_eq!(got, bytes);
    }

    #[test]
    fn empty_stream_round_trips() {
        roundtrip_bytes(&[]);
    }

    #[test]
    fn single_byte_round_trips() {
        roundtrip_bytes(b"x");
    }

    #[test]
    fn skewed_repeat_round_trips() {
        roundtrip_bytes(&b"aaaaaaaaaaaaaaaaaaaaaaaaaab".repeat(20));
    }

    #[test]
    fn full_alphabet_cycles_round_trip() {
        let bytes: Vec<u8> = (0..2000).map(|i| u8::try_from(i % 256).unwrap()).collect();
        roundtrip_bytes(&bytes);
    }

    #[test]
    fn ascii_text_round_trips() {
        let text = b"the quick brown fox jumps over the lazy dog, again and again.".repeat(50);
        roundtrip_bytes(&text);
    }

    #[test]
    fn pseudo_random_bytes_round_trip() {
        // xorshift32: deterministic, no external RNG dependency. 5000
        // bytes crosses every bank's rescale threshold repeatedly,
        // including the fast expert's low 6144 ceiling.
        let mut state = 0x1234_5678u32;
        let mut bytes = Vec::with_capacity(5000);
        for _ in 0..5000 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            bytes.push(u8::try_from(state % 256).unwrap());
        }
        roundtrip_bytes(&bytes);
    }

    #[test]
    fn copy_tokens_interleave_with_literals_round_trip() {
        // The shape Method-wiring will actually drive this with: literal
        // runs broken up by simulated LZ copy tokens, each shifting
        // `after_copy` and re-deriving `prev1`/`prev2` from the copied
        // bytes rather than from `after_literal`'s single-byte update.
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        let literal_runs: &[&[u8]] = &[b"hello ", b"world", b" repeat repeat repeat"];
        let copy_runs: &[&[u8]] = &[b"repeat repeat", b"o", b""];
        for (lits, copy) in literal_runs.iter().zip(copy_runs.iter()) {
            for &b in *lits {
                model.encode(&mut enc, context, b);
                context = context.after_literal(b);
            }
            context = context.after_copy(copy);
        }
        let encoded = enc.finish();
        let total_literals: usize = literal_runs.iter().map(|r| r.len()).sum();

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(&encoded);
        let mut got = Vec::with_capacity(total_literals);
        for (lits, copy) in literal_runs.iter().zip(copy_runs.iter()) {
            for _ in *lits {
                let b = model.decode(&mut dec, context);
                context = context.after_literal(b);
                got.push(b);
            }
            context = context.after_copy(copy);
        }
        let expected: Vec<u8> = literal_runs
            .iter()
            .flat_map(|r| r.iter().copied())
            .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn context_after_literal_tracks_previous_bytes_and_position() {
        let context = Context::default();
        let context = context.after_literal(b'a');
        assert_eq!(context.prev1, b'a');
        assert_eq!(context.prev2, 0);
        assert_eq!(context.position, 1);
        assert!(!context.after_copy);
        let context = context.after_literal(b'b');
        assert_eq!(context.prev1, b'b');
        assert_eq!(context.prev2, b'a');
        assert_eq!(context.position, 2);
    }

    #[test]
    fn context_after_copy_of_zero_bytes_keeps_previous_bytes() {
        let context = Context::default().after_literal(b'a').after_literal(b'b');
        let after = context.after_copy(&[]);
        assert_eq!(after.prev1, context.prev1);
        assert_eq!(after.prev2, context.prev2);
        assert_eq!(after.position, context.position);
        assert!(after.after_copy);
    }

    #[test]
    fn context_after_copy_of_one_byte_shifts_prev1_into_prev2() {
        let context = Context::default().after_literal(b'a');
        let after = context.after_copy(b"z");
        assert_eq!(after.prev1, b'z');
        assert_eq!(after.prev2, b'a');
        assert_eq!(after.position, 2);
    }

    #[test]
    fn context_after_copy_of_many_bytes_uses_last_two() {
        let after = Context::default().after_copy(b"hello");
        assert_eq!(after.prev1, b'o');
        assert_eq!(after.prev2, b'l');
        assert_eq!(after.position, 5);
    }

    #[test]
    fn word_hash_extends_on_alnum_and_resets_on_punctuation() {
        let hash_a = advance_word_hash(0, b'a');
        let hash_ab = advance_word_hash(hash_a, b'b');
        assert_ne!(hash_a, 0);
        assert_ne!(hash_ab, hash_a);
        assert_eq!(advance_word_hash(hash_ab, b' '), 0);
        assert_eq!(advance_word_hash(hash_ab, b'.'), 0);
    }

    #[test]
    fn decoding_truncated_stream_does_not_panic() {
        let bytes: Vec<u8> = (0..200).map(|i| u8::try_from(i % 5).unwrap()).collect();
        let mut model = Literal::new();
        let mut context = Context::default();
        let mut enc = Encoder::new();
        for &b in &bytes {
            model.encode(&mut enc, context, b);
            context = context.after_literal(b);
        }
        let encoded = enc.finish();
        let truncated = &encoded[..encoded.len() / 2];

        let mut model = Literal::new();
        let mut context = Context::default();
        let mut dec = Decoder::new(truncated);
        for _ in &bytes {
            let b = model.decode(&mut dec, context);
            context = context.after_literal(b);
        }
        // No panic is the assertion: decoded bytes past the real data are
        // whatever implicit-zero bits produce, never treated as ground
        // truth here.
    }
}
