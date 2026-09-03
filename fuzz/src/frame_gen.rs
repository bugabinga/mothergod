//! Deterministic generator of valid mothergod frames (issue #451): this
//! codec's answer to zstd's `decodecorpus` and SQLite's `dbsqlfuzz`.
//! Random-byte fuzzing dies almost entirely at the header check
//! ([`mothergod::Error::BadMagic`]/`Truncated`), so the decoder's deep
//! state machine (rep offsets, filter undo, adaptive model state) goes
//! under-exercised. This module compresses crafted inputs, already proven
//! in `src/filters.rs`'s/`src/lz.rs`'s own unit tests to drive
//! `filters::select::pick` and `lz::parse_optimal` into every
//! [`mothergod::filters::select::Candidate`] kind and the rep cache, back
//! through the real [`mothergod::compress`] — so every returned frame is
//! valid by construction, through the same code path every real caller
//! uses, and can never drift from what [`mothergod::decompress`] accepts
//! the way a hand-rolled byte layout could.
//!
//! Preimage sizes stay in the tens-to-low-thousands of bytes, matching
//! what the crate's own unit tests already use for the same shapes. A
//! literal [`mothergod::lz::WINDOW`]-sized (1 MiB) preimage was measured
//! (this issue's implementation) at 1.6s per `compress` call under a
//! release build and had not finished a 1.2 MiB low-redundancy case after
//! 280s under a debug build, because `encode` trials every shortlisted
//! filter candidate through the full optimal-parse and entropy-coding
//! pipeline per call — disproportionate cost for one corpus seed.
//! `lz.rs`'s `optimal_with_window_reaches_a_repeat_a_smaller_window_would_miss`
//! and its greedy counterpart already cover window-boundary distance
//! behavior directly, without materializing a multi-megabyte buffer.

/// One named, deterministic preimage: the raw bytes handed to
/// [`mothergod::compress`], not a frame itself. Exposed alongside
/// [`frames`] so a caller seeding the always-valid `roundtrip` fuzz
/// target (which mutates preimages that must still round-trip through
/// `compress`, not already-encoded frames) can start from this list too.
#[must_use]
pub fn preimages() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("empty", Vec::new()),
        ("one_byte", vec![b'x']),
        (
            "all_literals_no_repeats",
            b"the quick brown fox jumps over a lazy dog".to_vec(),
        ),
        ("simple_repeat", b"abcdefgh".repeat(50)),
        (
            "structured_repeats_varying_distances",
            structured_repeats_varying_distances(),
        ),
        ("long_repeated_byte_run", vec![b'z'; 4000]),
        ("cyclic_bytes", (0..=255u8).cycle().take(5000).collect()),
        (
            "binary_zero_bytes",
            (0..1000u32).map(|i| (i % 251) as u8).collect(),
        ),
        ("delta_columnar_drift", columnar_drift(4, 2000)),
        ("bcj_opcode_dense", bcj_opcode_dense(1000)),
        ("transpose_columnar", columnar_drift(8, 2000)),
        (
            "pseudo_random_incompressible",
            pseudo_random(0xdead_beef, 2048),
        ),
    ]
}

/// [`preimages`], each compressed through the real encoder: the actual
/// valid frames this module exists to produce. Asserts every one
/// round-trips back through [`mothergod::decompress`] before returning
/// (mechanism item 4 of issue #451: "generate, round-trip, assert
/// equality and no panic"), so a future change that broke a generated
/// frame's validity fails loudly the moment any fuzz target or seeding
/// step calls this, rather than silently seeding a corpus with dead
/// weight.
///
/// # Panics
///
/// Panics if any preimage fails to round-trip through
/// `compress`/`decompress`: a bug in the codec, never a property of the
/// (fixed, in-repo) preimages themselves.
#[must_use]
pub fn frames() -> Vec<(&'static str, Vec<u8>)> {
    preimages()
        .into_iter()
        .map(|(name, data)| {
            let frame = mothergod::compress(&data);
            assert_eq!(
                mothergod::decompress(&frame).as_deref(),
                Ok(data.as_slice()),
                "frame_gen preimage {name:?} failed to roundtrip"
            );
            (name, frame)
        })
        .collect()
}

/// Small-step pseudo-random walk: a deterministic LCG selecting one of
/// five `{-2, -1, 0, 1, 2}` steps, matching `src/filters.rs`'s own
/// `next_step` test helper exactly. Small enough that consecutive
/// same-column bytes stay close (what `Candidate::Delta`/
/// `Candidate::Transpose` selection needs), never repeating exactly.
fn next_step(seed: &mut u32) -> u8 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    const WALK_STEPS: [u8; 5] = [0u8.wrapping_sub(2), 0u8.wrapping_sub(1), 0, 1, 2];
    let index = usize::try_from((*seed >> 24) % 5).unwrap_or(0);
    WALK_STEPS[index]
}

/// `columns` independent small random walks, interleaved row-major:
/// consecutive same-column bytes drift by a small step, but consecutive
/// raw bytes belong to unrelated walks. With `columns == 4` this is
/// `src/filters.rs`'s `pick_selects_delta_for_columnar_drift` input,
/// verified there to make `filters::select::pick` shortlist
/// `Candidate::Delta(4)` first; with `columns == 8` it is
/// `pick_shortlists_transpose_for_column_structured_data`'s input,
/// verified to shortlist a `Candidate::Transpose`.
fn columnar_drift(columns: usize, rows: usize) -> Vec<u8> {
    let mut seeds: Vec<u32> = (0..columns)
        .map(|c| 0x9e37_79b9u32.wrapping_mul(u32::try_from(c).unwrap_or(0) + 1))
        .collect();
    let mut walk = vec![128u8; columns];
    let mut data = vec![0u8; columns * rows];
    for row in 0..rows {
        for (col, w) in walk.iter_mut().enumerate() {
            *w = w.wrapping_add(next_step(&mut seeds[col]));
            data[row * columns + col] = *w;
        }
    }
    data
}

/// `0xE8` (`call rel32`) opcode every 20 bytes, otherwise `0x90` (`nop`):
/// `src/filters.rs`'s `pick_shortlists_bcj_for_opcode_dense_data` input,
/// verified there to make `filters::select::pick` shortlist
/// `Candidate::Bcj`. `len` is a parameter, not a fixed 1000, so
/// [`PreimageRecipe::BcjDense`] can drive it through varying sizes.
fn bcj_opcode_dense(len: usize) -> Vec<u8> {
    let mut data = vec![0x90u8; len];
    for chunk in data.chunks_mut(20) {
        chunk[0] = 0xE8;
    }
    data
}

/// Two near-duplicate copies of a 26-byte block separated by unrelated
/// bytes: `src/lz.rs`'s
/// `roundtrip_structured_repeats_at_varying_distances` input, exercising
/// match/rep distances beyond the rep cache's initial `[1, 4, 8]` seed.
fn structured_repeats_varying_distances() -> Vec<u8> {
    let base: Vec<u8> = (b'a'..=b'z').collect();
    let mut data = base.clone();
    data.push(b'-');
    data.extend_from_slice(&base[1..]);
    data.push(b'+');
    data.extend_from_slice(&base);
    data
}

/// Uniform pseudo-random bytes: no filter or match/rep structure to
/// exploit, so `encode` should pick `Method::Stored` (a plain header plus
/// raw bytes beats every encoding once nothing compresses).
fn pseudo_random(mut seed: u32, len: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(len);
    for _ in 0..len {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        data.push((seed >> 24) as u8);
    }
    data
}

/// One dimension (a length, a row count) a recipe hands to a generator
/// below. Capped well under the tens-to-low-thousands range the
/// hand-picked [`preimages`] already use: this module's doc comment
/// measured a 1 MiB preimage at 1.6s/`compress` in release and unfinished
/// after 280s in debug, because `encode` trials every filter candidate
/// through the full optimal-parse pipeline per call. `columns` in
/// [`PreimageRecipe::ColumnarDrift`] is capped separately and tighter,
/// since it multiplies against a row count rather than standing alone.
const MAX_DIM: usize = 4096;

fn bound_dim(raw: u16) -> usize {
    usize::from(raw) % (MAX_DIM + 1)
}

/// Structured recipe for one preimage (issue #451, mechanism item 3):
/// deriving `Arbitrary` here, rather than on raw bytes, lets libFuzzer
/// mutate this small parameter space directly. A mutation stays a
/// recognizable member of the same shape family `filters::select::pick`
/// keys off (a columnar drift stays columnar, a repeat stays a repeat) as
/// it explores sizes and seeds [`preimages`]'s fixed dozen never tries,
/// rather than [`frame_mutate`](super)'s byte flips near an
/// already-encoded frame, which mutate in frame *bytes*, not in the
/// preimage's shape.
#[derive(Debug, Clone, arbitrary::Arbitrary)]
pub enum PreimageRecipe {
    /// `len` copies of `byte`: heavy single-offset repeat structure, the
    /// shape `long_repeated_byte_run` fixes at 4000 bytes of `b'z'`.
    RepeatedByte { byte: u8, len: u16 },
    /// The 0..=255 byte cycle `cyclic_bytes` fixes at 5000 bytes, varying
    /// length only: distance-256 repeat structure throughout.
    Cyclic { len: u16 },
    /// `columnar_drift`'s shape, generalized: `columns` capped at 64 so
    /// `columns * rows` cannot run away the way an unbounded `u16 * u16`
    /// would (frame_gen's own doc comment on why whole-buffer size
    /// matters here). `pick_selects_delta_for_columnar_drift` and
    /// `pick_shortlists_transpose_for_column_structured_data` (in
    /// `src/filters.rs`) key off exactly this shape at columns 4 and 8;
    /// other column counts explore neighboring filter-selection behavior
    /// those two fixed cases do not.
    ColumnarDrift { columns: u8, rows: u16 },
    /// `bcj_opcode_dense`'s shape, generalized over length: `0xE8` every
    /// 20 bytes amid `0x90` filler.
    BcjDense { len: u16 },
    /// `pseudo_random`'s shape, generalized: uniform bytes with no filter
    /// or match/rep structure to exploit, varying seed and length so
    /// `encode`'s `Method::Stored` fallback sees more than one input.
    PseudoRandom { seed: u32, len: u16 },
}

impl PreimageRecipe {
    /// Materializes this recipe's raw bytes, the preimage a fuzz target
    /// hands to [`mothergod::compress`].
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        match *self {
            PreimageRecipe::RepeatedByte { byte, len } => vec![byte; bound_dim(len)],
            PreimageRecipe::Cyclic { len } => (0..=255u8).cycle().take(bound_dim(len)).collect(),
            PreimageRecipe::ColumnarDrift { columns, rows } => {
                columnar_drift(usize::from(columns) % 65, bound_dim(rows))
            }
            PreimageRecipe::BcjDense { len } => bcj_opcode_dense(bound_dim(len)),
            PreimageRecipe::PseudoRandom { seed, len } => pseudo_random(seed, bound_dim(len)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use mothergod::filters::select::{Candidate, pick};

    use super::*;

    /// Byte offset of the frozen header's method byte
    /// (`docs/format/SPEC.md`, stable since ADR-0041): `MAGIC` (4 bytes)
    /// plus the 1-byte format version. Derived from the public `MAGIC`
    /// constant rather than duplicated as a literal, so this stays in
    /// step if `MAGIC`'s length ever did (it will not: the layout is
    /// frozen), and documents the frame layout it depends on rather than
    /// asserting a magic number silently.
    fn method_byte(frame: &[u8]) -> u8 {
        frame[mothergod::MAGIC.len() + 1]
    }

    #[test]
    fn frames_round_trip_and_cover_both_methods() {
        let frames = frames();
        assert!(
            frames.iter().any(|(_, f)| method_byte(f) == 0),
            "no generated frame chose Method::Stored"
        );
        assert!(
            frames.iter().any(|(_, f)| method_byte(f) == 1),
            "no generated frame chose Method::Lz"
        );
    }

    #[test]
    fn columnar_drift_selects_delta_and_transpose_as_documented() {
        let data = columnar_drift(4, 2000);
        assert_eq!(
            pick(&data)[0],
            Candidate::Delta(NonZeroUsize::new(4).unwrap()),
            "4-column drift should shortlist Delta(4) first, same as \
             filters.rs's pick_selects_delta_for_columnar_drift"
        );

        let data = columnar_drift(8, 2000);
        assert!(
            pick(&data)
                .iter()
                .any(|c| matches!(c, Candidate::Transpose(_))),
            "8-column drift should shortlist a Transpose candidate, same \
             as filters.rs's pick_shortlists_transpose_for_column_structured_data"
        );
    }

    #[test]
    fn bcj_opcode_dense_selects_bcj_as_documented() {
        assert!(
            pick(&bcj_opcode_dense(1000)).contains(&Candidate::Bcj),
            "opcode-dense data should shortlist Bcj, same as filters.rs's \
             pick_shortlists_bcj_for_opcode_dense_data"
        );
    }

    #[test]
    fn preimage_recipe_variants_round_trip() {
        let recipes = [
            PreimageRecipe::RepeatedByte {
                byte: b'z',
                len: 4000,
            },
            PreimageRecipe::Cyclic { len: 5000 },
            PreimageRecipe::ColumnarDrift {
                columns: 4,
                rows: 2000,
            },
            PreimageRecipe::BcjDense { len: 1000 },
            PreimageRecipe::PseudoRandom {
                seed: 0xdead_beef,
                len: 2048,
            },
            // Every field at its type's max: MAX_DIM/column-cap bounding
            // must hold even at the widest input Arbitrary can hand this
            // enum, the case a corpus-less fuzz run reaches first.
            PreimageRecipe::ColumnarDrift {
                columns: u8::MAX,
                rows: u16::MAX,
            },
        ];
        for recipe in recipes {
            let data = recipe.to_bytes();
            let frame = mothergod::compress(&data);
            assert_eq!(
                mothergod::decompress(&frame).as_deref(),
                Ok(data.as_slice()),
                "{recipe:?} failed to roundtrip"
            );
        }
    }

    #[test]
    fn preimage_recipe_dimensions_stay_bounded() {
        let recipe = PreimageRecipe::PseudoRandom {
            seed: 0,
            len: u16::MAX,
        };
        assert!(recipe.to_bytes().len() <= MAX_DIM);

        let recipe = PreimageRecipe::ColumnarDrift {
            columns: u8::MAX,
            rows: u16::MAX,
        };
        assert!(recipe.to_bytes().len() <= 64 * MAX_DIM);
    }
}
