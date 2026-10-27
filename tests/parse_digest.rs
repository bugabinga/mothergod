//! The proof that an `lz.rs` refactor left the parse alone: one digest of
//! `parse_optimal`'s token stream and one of `parse_greedy`'s, over inputs
//! this file generates plus every `tests/golden/*.plaintext`.
//!
//! `tests/golden.rs` pins `compress()` on ten plaintexts, which a parse
//! change confined to long rep runs or the carry cache can pass. A refactor
//! that claims "no behaviour change" cites this test green instead of
//! building a throwaway hasher over inputs of its own choosing (#1524).
//!
//! The digest folds the semantic fields of each token (kind tag, literal
//! byte, `len`, distance or slot ordinal), never the `Debug` rendering, so a
//! rename of a variant or field leaves it intact: the refactor class this
//! test exists to prove neutral must not demand a regeneration.
//!
//! Regeneration follows `tests/golden.rs`'s encoder-only arm: a change that
//! picks a different valid token sequence updates the constant in the same
//! PR, declares it in the PR body with the measured justification (the
//! `bench/baseline.json` pattern), and an undeclared regeneration is a
//! review FAIL. Adding or editing a golden plaintext moves the digest too,
//! and is declared the same way. A mismatch prints one digest per input, so
//! the author can see which input moved.
//!
//! Like `tests/golden.rs`'s re-encode pin, the optimal digest holds as far as
//! `f64::log2` agrees between libms (`lz.rs`'s match pricing, ADR-0024
//! decision 3), and the weekly monster matrix runs it on every runtime target.
//! `MOTHERGOD_GOLDEN_DIR` overrides the plaintext directory off the build
//! host, as there.

use std::fmt::Write as _;
use std::path::PathBuf;

// Shared with the other fixture suites; this one reads golden_dir alone.
#[allow(dead_code)]
#[path = "support/fixture_dirs.rs"]
mod fixture_dirs;
use fixture_dirs::golden_dir;

use mothergod::lz::{RepSlot, Token, parse_greedy, parse_optimal};

/// Digest of the per-input digests of `parse_optimal`.
const OPTIMAL_DIGEST: u64 = 0x7101_0cfd_75b9_584a;
/// Digest of the per-input digests of `parse_greedy`.
const GREEDY_DIGEST: u64 = 0xbd01_80cb_88e5_acf6;

/// Seeded xorshift64*: the generated inputs must be identical on every
/// platform and toolchain, which `std` offers no source for.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

/// FNV-1a, 64 bit: a digest with a published definition, so a reviewer can
/// reproduce it without this file's code.
struct Fnv(u64);

impl Fnv {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

fn token_digest(tokens: &[Token]) -> u64 {
    let mut hash = Fnv::new();
    hash.write(&(tokens.len() as u64).to_le_bytes());
    for token in tokens {
        match *token {
            Token::Literal(byte) => hash.write(&[0, byte]),
            Token::Match { len, distance } => {
                hash.write(&[1]);
                hash.write(&len.to_le_bytes());
                hash.write(&distance.get().to_le_bytes());
            }
            Token::Rep { len, slot } => {
                let ordinal = match slot {
                    RepSlot::First => 0,
                    RepSlot::Second => 1,
                    RepSlot::Third => 2,
                };
                hash.write(&[2, ordinal]);
                hash.write(&len.to_le_bytes());
            }
        }
    }
    hash.0
}

/// Uniform random bytes over the alphabet `0..alphabet`.
fn alphabet_input(rng: &mut Rng, alphabet: u64, len: usize) -> Vec<u8> {
    (0..len)
        .map(|_| u8::try_from(rng.below(alphabet)).expect("alphabet is at most 256"))
        .collect()
}

/// The generated classes, then the golden plaintexts in name order:
/// `read_dir` order is the filesystem's, and a digest that depends on it is
/// not a pin.
fn inputs() -> Vec<(String, Vec<u8>)> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut inputs = vec![("zeros-300k".to_string(), vec![0u8; 300_000])];

    let sentence = b"the quick brown fox jumps over the lazy dog, again and again. ";
    let mut text: Vec<u8> = sentence.iter().copied().cycle().take(100_000).collect();
    text.extend(alphabet_input(&mut rng, 256, 20_000));
    inputs.push(("periodic-text-random-tail".to_string(), text));

    let mut records = Vec::with_capacity(8 * 40_000);
    for id in 0..40_000u32 {
        records.extend(id.to_le_bytes());
        records.extend([0xA5, 0x00, 0x7F, u8::try_from(id % 5).expect("below 5")]);
    }
    inputs.push(("records-8-byte-40k".to_string(), records));

    // 63 is under `parse_optimal`'s fall-back to `parse_greedy`, 5_000 and
    // 70_000 are over it.
    for alphabet in [2, 4, 16, 256] {
        for len in [63, 5_000, 70_000] {
            let data = alphabet_input(&mut rng, alphabet, len);
            inputs.push((format!("alphabet-{alphabet}-{len}"), data));
        }
    }

    let mut golden: Vec<PathBuf> = std::fs::read_dir(golden_dir())
        .expect("tests/golden must exist")
        .map(|entry| entry.expect("readable dir entry").path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("plaintext"))
        .collect();
    assert!(
        !golden.is_empty(),
        "golden corpus in tests/golden/ must not be empty"
    );
    golden.sort();
    for path in golden {
        let data = std::fs::read(&path).expect("readable golden plaintext");
        // The stem carries `FORMAT_VERSION`; the label need not.
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("UTF-8 stem");
        let label = stem.split_once('-').map_or(stem, |(_, rest)| rest);
        inputs.push((format!("golden-{label}"), data));
    }
    inputs
}

/// Folds the per-input digests into one, returning them beside it for the
/// failure message.
fn pin(parse: fn(&[u8]) -> Vec<Token>) -> (u64, String) {
    let mut total = Fnv::new();
    let mut lines = String::new();
    for (name, data) in inputs() {
        let digest = token_digest(&parse(&data));
        total.write(&digest.to_le_bytes());
        writeln!(lines, "  {digest:016x}  {name}").expect("write to String");
    }
    (total.0, lines)
}

fn assert_pinned(parse: fn(&[u8]) -> Vec<Token>, pinned: u64, what: &str) {
    let (digest, lines) = pin(parse);
    assert_eq!(
        digest, pinned,
        "{what} picked a different token sequence: digest {digest:#018x}, pinned {pinned:#018x}.\n\
         Per-input digests:\n{lines}\
         - A refactor that claims no behaviour change has changed behaviour.\n\
         - An encoder-only change updates the constant in this PR and declares the \
         regeneration in the PR body with the measured justification (see this \
         file's header); an undeclared regeneration is a review FAIL."
    );
}

#[test]
fn parse_optimal_token_stream_is_pinned() {
    assert_pinned(parse_optimal, OPTIMAL_DIGEST, "parse_optimal");
}

#[test]
fn parse_greedy_token_stream_is_pinned() {
    assert_pinned(parse_greedy, GREEDY_DIGEST, "parse_greedy");
}
