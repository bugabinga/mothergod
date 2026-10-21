//! CI regression gate over mothergod's bits/byte on the train corpus
//! generators (`research/JOURNAL.md` remaining S2-D1 scope: "the CI
//! baseline gate").
//!
//! [`cases`] builds one fixed-seed, fixed-length input per train-eligible
//! [`crate::DatasetKind`] (the sealed-only kinds, [`crate::DatasetKind::sealed_only`],
//! are excluded on purpose: a PR-time gate an agent reacts to and fixes is a
//! tuning loop, and running it against the sealed set would smuggle sealed
//! data into that loop through the back door,
//! `research/corpus/POLICY.md`'s "no agent ever tunes against it"), plus one
//! case per entropy-ladder target since a single [`crate::DatasetKind::EntropyLadder`]
//! case cannot name which of the ladder's five points it measured.
//! [`measure_all`] compresses every case with `mothergod::compress` and
//! records, per case, its bits/byte and a digest of the compressed bytes
//! (an [`Entry`]); [`format_baseline`]/[`parse_baseline`] round-trip that
//! [`Snapshot`] through `bench/baseline.json`; [`regressions`] compares a
//! freshly measured bits/byte map against the committed baseline, and
//! [`output_changes`] names the cases whose compressed bytes differ from the
//! committed digest, the one pin on `compress()`'s output over a broad fixed
//! input set (issue #1182).

use crate::DatasetKind;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;

/// Bytes generated per case. Small enough that every case's
/// `mothergod::compress` call finishes in well under a second in a release
/// build (measured: the slowest generator, `json_records`, took ~0.5s in a
/// debug build at this length); large enough that per-frame header
/// overhead (`docs/format/SPEC.md`) doesn't swamp a dataset kind's own
/// structure.
pub const CASE_LEN: usize = 50_000;

/// Fixed seed for every case. This gate compares today's bits/byte against
/// a committed number, so the input must be byte-identical across runs,
/// unlike [`crate::train_window`]'s deliberately rotating offset.
pub const CASE_SEED: u64 = 0xBA5E_11E5_BA5E_11E5;

/// Order-0 entropy targets the entropy ladder is measured at, matching
/// `research/corpus/POLICY.md`'s mandatory ladder.
const ENTROPY_LADDER_TARGETS: [u8; 5] = [1, 2, 4, 6, 8];

/// Absolute bits/byte a case may drift above its committed baseline before
/// [`regressions`] reports it. Chosen well above this measurement's own
/// run-to-run noise floor (none: every case is a fixed seed through a
/// deterministic codec) and small enough to catch a real ratio regression
/// rather than only a gross one; a change that intentionally improves or
/// costs ratio updates `bench/baseline.json` in the same PR instead of
/// widening this constant.
pub const TOLERANCE_BITS: f64 = 0.02;

/// One named regression-gate case: a stable identifier (the key
/// [`format_baseline`] writes to `bench/baseline.json`) and the bytes to
/// compress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// Stable name, this case's key in `bench/baseline.json`.
    pub name: &'static str,
    /// Input bytes: [`CASE_LEN`] bytes at [`CASE_SEED`] from the named
    /// generator (the entropy ladder additionally keyed by its target).
    pub data: Vec<u8>,
}

/// Stable case name for one entropy-ladder target.
///
/// # Panics
///
/// Panics if `bits` is not one of [`ENTROPY_LADDER_TARGETS`]; this is a
/// private helper called only from [`cases`], which only ever passes that
/// fixed list.
fn entropy_ladder_case_name(bits: u8) -> &'static str {
    match bits {
        1 => "entropy_ladder_h1",
        2 => "entropy_ladder_h2",
        4 => "entropy_ladder_h4",
        6 => "entropy_ladder_h6",
        8 => "entropy_ladder_h8",
        other => panic!("entropy_ladder_case_name: {other} is not an ENTROPY_LADDER_TARGETS entry"),
    }
}

/// Stable case name and [`CASE_LEN`]/[`CASE_SEED`] bytes for one
/// non-ladder, train-eligible [`DatasetKind`]. `None` for
/// [`DatasetKind::EntropyLadder`] (handled separately by
/// [`entropy_ladder_case_name`], since it needs a ladder target) and for a
/// sealed-only kind (`DatasetKind::sealed_only`, excluded from this gate by
/// design).
fn dataset_case(kind: DatasetKind) -> Option<Case> {
    let (name, data): (&'static str, Vec<u8>) = match kind {
        DatasetKind::EntropyLadder | DatasetKind::AccessLog | DatasetKind::GradientImage => {
            return None;
        }
        DatasetKind::MarkovH82Trap => (
            "markov_h8_2_trap",
            crate::markov_h8_2_trap(CASE_LEN, CASE_SEED),
        ),
        DatasetKind::JsonRecords => ("json_records", crate::json_records(CASE_LEN, CASE_SEED)),
        DatasetKind::Base64Wrapped => {
            ("base64_wrapped", crate::base64_wrapped(CASE_LEN, CASE_SEED))
        }
        DatasetKind::InterleavedAudio16 => (
            "interleaved_audio16",
            crate::interleaved_audio16(CASE_LEN, CASE_SEED),
        ),
        DatasetKind::SqliteLikeRecords => (
            "sqlite_like_records",
            crate::sqlite_like_records(CASE_LEN, CASE_SEED),
        ),
        DatasetKind::X86DenseCode => ("x86_dense_code", crate::x86_dense_code(CASE_LEN, CASE_SEED)),
    };
    Some(Case { name, data })
}

/// Every regression-gate case: one per entropy-ladder target plus one per
/// non-ladder, non-sealed-only [`DatasetKind`].
#[must_use]
pub fn cases() -> Vec<Case> {
    let mut out: Vec<Case> = ENTROPY_LADDER_TARGETS
        .into_iter()
        .map(|bits| Case {
            name: entropy_ladder_case_name(bits),
            data: crate::entropy_ladder(bits, CASE_LEN, CASE_SEED),
        })
        .collect();
    out.extend(DatasetKind::ALL.into_iter().filter_map(dataset_case));
    out
}

/// Name of the one case [`reach_case`] builds, its key in
/// `bench/baseline.json`.
pub const REACH_CASE_NAME: &str = "wide_reach_repeat";

/// Bytes of the block [`reach_case`] repeats.
const REACH_BLOCK_LEN: usize = 128 * 1024;

/// Bytes between the two copies of that block's start: the standard match
/// reach, a block and slack, so the copy sits past [`mothergod::lz::WINDOW`]
/// and inside [`mothergod::lz::WIDE_WINDOW`] (`lz::Reach`).
const REACH_DISTANCE: usize = mothergod::lz::WINDOW + REACH_BLOCK_LEN + 64 * 1024;

/// Bytes in [`reach_case`]'s input: the distance to the second copy, then
/// that copy. The baseline page quotes it beside [`CASE_LEN`].
pub const REACH_CASE_LEN: usize = REACH_DISTANCE + REACH_BLOCK_LEN;

/// The gate's one input past the standard match reach (issue #1434): a
/// uniform-random block, [`crate::json_records`] filler, then the block
/// again one standard reach, a block and some slack after its first copy.
/// The block is incompressible on its own, so a parse that reaches the first copy saves
/// the whole second one and one that cannot pays for it in full: a
/// reach-class change moves this case's bits/byte by whole bits where the
/// [`CASE_LEN`] cases, all inside the standard reach, cannot feel it.
///
/// Not part of [`cases`]: its length is not [`CASE_LEN`], so it does not
/// enter the train mean (`research/README.md`) or the decode-speed
/// calibration, which both count [`cases`] only.
#[must_use]
pub fn reach_case() -> Case {
    let block = crate::entropy_ladder(8, REACH_BLOCK_LEN, CASE_SEED);
    let mut data = block.clone();
    data.extend(crate::json_records(
        REACH_DISTANCE - REACH_BLOCK_LEN,
        CASE_SEED,
    ));
    data.extend_from_slice(&block);
    Case {
        name: REACH_CASE_NAME,
        data,
    }
}

/// Bits/byte of `compressed_len` bytes encoding `original_len` original
/// bytes. `0.0` when `original_len` is `0` (nothing to measure a ratio
/// against).
#[allow(
    clippy::cast_precision_loss,
    reason = "byte counts here stay far below 2^53"
)]
#[must_use]
pub fn bits_per_byte(compressed_len: usize, original_len: usize) -> f64 {
    if original_len == 0 {
        return 0.0;
    }
    (compressed_len as f64) * 8.0 / (original_len as f64)
}

/// 64-bit FNV-1a of `bytes`. Not cryptographic: [`Entry::digest`] only
/// detects an honest accidental change, never resists a deliberate one, so a
/// dependency-free hash is the right size (ADR-0002's zero-dependency rule
/// binds the core crate; this is bench tooling, but pulling in `sha2` for a
/// marker nobody attacks would tax every default-feature build for a
/// collision resistance this job never needed).
fn fnv1a(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

/// One case's record: its bits/byte and the digest of the compressed bytes
/// that produced it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entry {
    /// Bits/byte of the case's compressed form.
    pub bpb: f64,
    /// FNV-1a of the compressed bytes: unchanged output, not merely
    /// unchanged size.
    pub digest: u64,
}

/// Every case's [`Entry`], keyed by [`Case::name`]: what one
/// [`measure_all`] run or one `bench/baseline.json` holds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// Entries in name order, the order the file is written in.
    pub cases: BTreeMap<String, Entry>,
}

impl Snapshot {
    /// The bits/byte of every case, the map [`regressions`], [`deltas`] and
    /// [`hold_at_best`] work over.
    #[must_use]
    pub fn bpb(&self) -> BTreeMap<String, f64> {
        self.cases
            .iter()
            .map(|(name, entry)| (name.clone(), entry.bpb))
            .collect()
    }

    /// This snapshot with each case's bits/byte replaced by its value in
    /// `bpb`, digests kept; a case absent from `bpb` keeps its own. How a
    /// [`hold_at_best`] result becomes the snapshot to write.
    #[must_use]
    pub fn with_bpb(&self, bpb: &BTreeMap<String, f64>) -> Snapshot {
        let cases = self
            .cases
            .iter()
            .map(|(name, entry)| {
                let entry = Entry {
                    bpb: bpb.get(name).copied().unwrap_or(entry.bpb),
                    digest: entry.digest,
                };
                (name.clone(), entry)
            })
            .collect();
        Snapshot { cases }
    }
}

/// Compresses every [`cases`] entry through `mothergod::compress` and
/// records each one's bits/byte and output digest, keyed by [`Case::name`].
/// The train measurement: `measure` and `delta` use it, because their mean
/// counts [`CASE_LEN`]-byte cases only.
#[must_use]
pub fn measure_all() -> Snapshot {
    measure(&cases())
}

/// [`measure_all`] plus the [`reach_case`]: what `check` and `write` hold
/// `bench/baseline.json` to.
#[must_use]
pub fn measure_gate() -> Snapshot {
    let mut gate = cases();
    gate.push(reach_case());
    measure(&gate)
}

/// Compresses each of `cases` and records its [`Entry`].
fn measure(cases: &[Case]) -> Snapshot {
    let cases = cases
        .iter()
        .map(|case| {
            let compressed = mothergod::compress(&case.data);
            let entry = Entry {
                bpb: bits_per_byte(compressed.len(), case.data.len()),
                digest: fnv1a(&compressed),
            };
            (case.name.to_string(), entry)
        })
        .collect();
    Snapshot { cases }
}

/// Writes `snapshot` as the `bench/baseline.json` this module reads back
/// with [`parse_baseline`]: an object keyed by case name, keys sorted
/// (`snapshot.cases` is a [`BTreeMap`]), each value an object of `bpb`
/// rounded to six decimal places with the trailing zeros trimmed (`8.0`
/// prints `8`) and `digest` as 16 lowercase hex digits. That is the form
/// `cargo x fmt` leaves the file in, so a freshly written one already passes
/// `cargo x fmt --check` (issue #943). Not a general JSON writer: it only
/// needs to round-trip its own output, the same scope the `corpus` module's
/// manual TOML reader (behind the `corpus-fetch` feature) takes for
/// `bench/corpus.toml`.
#[must_use]
pub fn format_baseline(snapshot: &Snapshot) -> String {
    let mut out = String::from("{\n");
    let mut remaining = snapshot.cases.len();
    for (name, entry) in &snapshot.cases {
        remaining -= 1;
        let comma = if remaining == 0 { "" } else { "," };
        let fixed = format!("{:.6}", entry.bpb);
        let bpb = if fixed.contains('.') {
            fixed.trim_end_matches('0').trim_end_matches('.')
        } else {
            &fixed
        };
        writeln!(
            out,
            "  \"{name}\": {{\n    \"bpb\": {bpb},\n    \"digest\": \"{:016x}\"\n  }}{comma}",
            entry.digest
        )
        .expect("writing to a String never fails");
    }
    out.push_str("}\n");
    out
}

/// A line [`parse_baseline`] could not read as part of a case in the shape
/// [`format_baseline`] writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineParseError {
    /// 1-based line number.
    pub line: usize,
    /// The line's content, for the error message.
    pub content: String,
}

impl fmt::Display for BaselineParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bench/baseline.json line {}: not a case in the shape `baseline_gate write` emits, got `{}`",
            self.line, self.content
        )
    }
}

impl std::error::Error for BaselineParseError {}

/// Parses text in the shape [`format_baseline`] writes back into a
/// [`Snapshot`]. Not a general JSON reader: it understands exactly one
/// shape (an opening `{`, per case a `"name": {` line, a `"bpb": number`
/// line, a `"digest": "<16 hex digits>"` line and a closing `}`, an
/// optional trailing comma on any line, a closing `}`) and rejects anything
/// else line by line, the same deliberate scope limit the `corpus`
/// module's manual TOML reader (behind the `corpus-fetch` feature) takes
/// for `bench/corpus.toml`.
///
/// # Errors
///
/// Returns [`BaselineParseError`] naming the first line that is not blank,
/// `{`, `}`, or part of a case in that shape, a case missing its `bpb` or
/// its `digest`, or the line that opened a case the text never closes.
pub fn parse_baseline(text: &str) -> Result<Snapshot, BaselineParseError> {
    /// The case being read: its name, the line that opened it, and the
    /// fields seen so far.
    struct Open {
        name: String,
        line: usize,
        bpb: Option<f64>,
        digest: Option<u64>,
    }
    let mut cases = BTreeMap::new();
    let mut open: Option<Open> = None;
    let mut opened_at = String::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim().trim_end_matches(',');
        let err = || BaselineParseError {
            line: idx + 1,
            content: raw_line.to_string(),
        };
        if line.is_empty() {
            continue;
        }
        let Some(case) = open.as_mut() else {
            if line == "{" || line == "}" {
                continue;
            }
            let (key, value) = split_pair(line).ok_or_else(err)?;
            if value != "{" {
                return Err(err());
            }
            opened_at = raw_line.to_string();
            open = Some(Open {
                name: key.to_string(),
                line: idx + 1,
                bpb: None,
                digest: None,
            });
            continue;
        };
        if line == "}" {
            let (Some(bpb), Some(digest)) = (case.bpb, case.digest) else {
                return Err(err());
            };
            cases.insert(case.name.clone(), Entry { bpb, digest });
            open = None;
            continue;
        }
        let (key, value) = split_pair(line).ok_or_else(err)?;
        match key {
            "bpb" => case.bpb = Some(value.parse().map_err(|_| err())?),
            "digest" => {
                let hex = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .filter(|hex| hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
                    .ok_or_else(err)?;
                case.digest = Some(u64::from_str_radix(hex, 16).map_err(|_| err())?);
            }
            _ => return Err(err()),
        }
    }
    match open {
        Some(case) => Err(BaselineParseError {
            line: case.line,
            content: opened_at,
        }),
        None => Ok(Snapshot { cases }),
    }
}

/// `"key": value` as the unquoted key and the trimmed value text; `None`
/// when the line has no colon or its key is not quoted.
fn split_pair(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some((key, value.trim()))
}

/// A case whose measured bits/byte regressed past [`TOLERANCE_BITS`] above
/// its committed baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct Regression {
    /// The regressed case's name, [`Case::name`].
    pub name: String,
    /// Bits/byte committed in `bench/baseline.json`.
    pub baseline_bpb: f64,
    /// Bits/byte just measured.
    pub measured_bpb: f64,
}

impl Regression {
    /// `measured_bpb - baseline_bpb`; positive by construction (that is
    /// what makes it a regression).
    #[must_use]
    pub fn delta(&self) -> f64 {
        self.measured_bpb - self.baseline_bpb
    }
}

/// Compares `measured` against `baseline`, reporting every case that grew
/// by more than [`TOLERANCE_BITS`] bits/byte. A case present only in
/// `measured` (no committed baseline yet, e.g. a newly added kind) or only
/// in `baseline` (a case this build no longer produces) is not a
/// regression; [`format_baseline`] over `measured` is how a baseline update
/// picks it up.
#[must_use]
pub fn regressions(
    baseline: &BTreeMap<String, f64>,
    measured: &BTreeMap<String, f64>,
) -> Vec<Regression> {
    measured
        .iter()
        .filter_map(|(name, &measured_bpb)| {
            let &baseline_bpb = baseline.get(name)?;
            (measured_bpb > baseline_bpb + TOLERANCE_BITS).then(|| Regression {
                name: name.clone(),
                baseline_bpb,
                measured_bpb,
            })
        })
        .collect()
}

/// One case's move between a baseline and a measurement, whatever its
/// sign: [`regressions`] reports only the growth past
/// [`TOLERANCE_BITS`], this is every case's number.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseDelta {
    /// The case's name, [`Case::name`].
    pub name: String,
    /// Bits/byte in the baseline.
    pub baseline_bpb: f64,
    /// Bits/byte just measured.
    pub measured_bpb: f64,
}

impl CaseDelta {
    /// `measured_bpb - baseline_bpb`; negative is better.
    #[must_use]
    pub fn delta(&self) -> f64 {
        self.measured_bpb - self.baseline_bpb
    }
}

/// Every case present in both `baseline` and `measured`, in name order. A
/// case on one side only has no number to subtract, the same scope
/// [`regressions`] takes.
#[must_use]
pub fn deltas(
    baseline: &BTreeMap<String, f64>,
    measured: &BTreeMap<String, f64>,
) -> Vec<CaseDelta> {
    measured
        .iter()
        .filter_map(|(name, &measured_bpb)| {
            let &baseline_bpb = baseline.get(name)?;
            Some(CaseDelta {
                name: name.clone(),
                baseline_bpb,
                measured_bpb,
            })
        })
        .collect()
}

/// The cases of `measured` whose compressed bytes differ from the digest
/// `baseline` commits, in name order: a changed stream of equal size, an
/// improvement, a regression within [`TOLERANCE_BITS`] all land here, which
/// [`regressions`] sees none of. A case with no committed digest is listed
/// (nothing pins its output yet); a case only in `baseline` is not, the
/// scope [`regressions`] takes.
#[must_use]
pub fn output_changes(baseline: &Snapshot, measured: &Snapshot) -> Vec<String> {
    measured
        .cases
        .iter()
        .filter(|(name, entry)| {
            baseline
                .cases
                .get(*name)
                .is_none_or(|committed| committed.digest != entry.digest)
        })
        .map(|(name, _)| name.clone())
        .collect()
}

/// The map [`format_baseline`] should write over `baseline` after a
/// measurement, and the cases it held back. Each case takes
/// `min(baseline, measured)` compared at the six decimals the file stores,
/// so a case that grew keeps its old, better value and the next
/// [`regressions`] pass still measures the growth against the best known
/// size; a case absent from `baseline` is written as measured. A
/// regeneration built on this cannot loosen the gate (hard rule 3): a
/// deliberate loosening is a hand edit of the JSON that a reviewer reads.
/// The held cases come back as [`CaseDelta`]s so the caller can name both
/// numbers.
#[must_use]
pub fn hold_at_best(
    baseline: &BTreeMap<String, f64>,
    measured: &BTreeMap<String, f64>,
) -> (BTreeMap<String, f64>, Vec<CaseDelta>) {
    let mut written = measured.clone();
    let mut held = Vec::new();
    for case in deltas(baseline, measured) {
        let as_written = (case.measured_bpb * 1e6).round() / 1e6;
        if as_written > case.baseline_bpb {
            written.insert(case.name.clone(), case.baseline_bpb);
            held.push(case);
        }
    }
    (written, held)
}

/// The train mean of `deltas`, the denominator `research/README.md`
/// fixes: every case's whole [`CASE_LEN`]-byte input counts once, so the
/// mean of the per-case bits/byte deltas. `0.0` for no cases.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "a case count stays far below 2^53"
)]
pub fn mean_delta(deltas: &[CaseDelta]) -> f64 {
    if deltas.is_empty() {
        return 0.0;
    }
    deltas.iter().map(CaseDelta::delta).sum::<f64>() / deltas.len() as f64
}

#[cfg(test)]
mod tests {
    use super::{
        BaselineParseError, CASE_LEN, Entry, REACH_BLOCK_LEN, REACH_CASE_LEN, REACH_CASE_NAME,
        Snapshot, TOLERANCE_BITS, bits_per_byte, cases, deltas, fnv1a, format_baseline,
        hold_at_best, mean_delta, output_changes, parse_baseline, reach_case, regressions,
    };
    use std::collections::BTreeMap;

    /// A snapshot of `(name, bpb, digest)` rows.
    fn snapshot(rows: &[(&str, f64, u64)]) -> Snapshot {
        let cases = rows
            .iter()
            .map(|&(name, bpb, digest)| (name.to_string(), Entry { bpb, digest }))
            .collect();
        Snapshot { cases }
    }

    #[test]
    fn cases_covers_every_train_eligible_kind_and_the_full_ladder() {
        let names: Vec<&str> = cases().iter().map(|c| c.name).collect();
        for expected in [
            "entropy_ladder_h1",
            "entropy_ladder_h2",
            "entropy_ladder_h4",
            "entropy_ladder_h6",
            "entropy_ladder_h8",
            "markov_h8_2_trap",
            "json_records",
            "base64_wrapped",
            "interleaved_audio16",
            "sqlite_like_records",
            "x86_dense_code",
        ] {
            assert!(names.contains(&expected), "missing case {expected}");
        }
        assert!(
            !names.contains(&"access_log") && !names.contains(&"gradient_image"),
            "sealed-only kinds must not appear in the regression gate: {names:?}"
        );
    }

    #[test]
    fn cases_are_exactly_case_len_bytes() {
        for case in cases() {
            assert_eq!(case.data.len(), CASE_LEN, "case {} wrong length", case.name);
        }
    }

    #[test]
    fn reach_case_repeats_its_block_past_the_standard_reach_inside_the_wide_one() {
        let case = reach_case();
        assert_eq!(case.data.len(), REACH_CASE_LEN);
        // The first copy starts at 0, so the second's offset is the distance.
        let distance = case.data.len() - REACH_BLOCK_LEN;
        assert_eq!(case.data[..REACH_BLOCK_LEN], case.data[distance..]);
        assert!(
            distance > mothergod::lz::WINDOW,
            "the standard reach sees it"
        );
        assert!(
            distance <= mothergod::lz::WIDE_WINDOW,
            "the wide reach cannot see it"
        );
        assert_eq!(case.name, REACH_CASE_NAME);
        assert!(
            cases().iter().all(|c| c.name != REACH_CASE_NAME),
            "the train cases and the reach case are disjoint"
        );
    }

    #[test]
    fn cases_are_deterministic() {
        let first: Vec<Vec<u8>> = cases().into_iter().map(|c| c.data).collect();
        let second: Vec<Vec<u8>> = cases().into_iter().map(|c| c.data).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn bits_per_byte_is_zero_for_empty_input() {
        assert!((bits_per_byte(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn bits_per_byte_matches_a_known_ratio() {
        // 1 compressed byte per 8 original bytes is exactly 1 bit/byte.
        assert!((bits_per_byte(1, 8) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn baseline_round_trips_through_format_and_parse() {
        let original = snapshot(&[("a", 1.5, 0), ("b", 7.999_999, u64::MAX)]);
        let text = format_baseline(&original);
        let parsed = parse_baseline(&text).expect("format_baseline's own output must parse");
        assert_eq!(parsed, original);
    }

    #[test]
    fn committed_baseline_is_what_format_baseline_writes() {
        let committed = include_str!("../baseline.json");
        let parsed = parse_baseline(committed).expect("committed baseline must parse");
        assert_eq!(
            format_baseline(&parsed),
            committed,
            "bench/baseline.json must be byte-identical to what `baseline_gate write` emits"
        );
    }

    #[test]
    fn format_baseline_trims_zeros_and_rounds_to_six_places() {
        let text = format_baseline(&snapshot(&[
            ("a", 0.530_56, 1),
            ("b", 8.0, 2),
            ("c", 10.0, 3),
            ("d", 2.370_564_9, 255),
        ]));
        assert_eq!(
            text,
            "{\n  \"a\": {\n    \"bpb\": 0.53056,\n    \"digest\": \"0000000000000001\"\n  },\n  \
             \"b\": {\n    \"bpb\": 8,\n    \"digest\": \"0000000000000002\"\n  },\n  \
             \"c\": {\n    \"bpb\": 10,\n    \"digest\": \"0000000000000003\"\n  },\n  \
             \"d\": {\n    \"bpb\": 2.370565,\n    \"digest\": \"00000000000000ff\"\n  }\n}\n"
        );
    }

    #[test]
    fn format_baseline_sorts_keys() {
        let text = format_baseline(&snapshot(&[("zebra", 1.0, 0), ("apple", 2.0, 0)]));
        assert!(text.find("apple").unwrap() < text.find("zebra").unwrap());
    }

    #[test]
    fn parse_baseline_rejects_a_malformed_line() {
        let err = parse_baseline("{\n  not valid\n}\n").unwrap_err();
        assert_eq!(
            err,
            BaselineParseError {
                line: 2,
                content: "  not valid".to_string()
            }
        );
    }

    #[test]
    fn parse_baseline_rejects_the_flat_form_this_file_used_to_have() {
        assert!(parse_baseline("{\n  \"a\": 1.5\n}\n").is_err());
    }

    /// One well-formed case to damage.
    const CASE: &str =
        "{\n  \"a\": {\n    \"bpb\": 1,\n    \"digest\": \"00000000000000ff\"\n  }\n}\n";

    #[test]
    fn parse_baseline_rejects_a_case_missing_a_field_or_with_a_bad_one() {
        for damaged in [
            CASE.replace("    \"bpb\": 1,\n", ""),
            CASE.replace(",\n    \"digest\": \"00000000000000ff\"", ""),
            CASE.replace("00000000000000ff", "ff"),
            CASE.replace("00000000000000ff", "000000000000000g"),
            CASE.replace("\"bpb\": 1", "\"bpb\": one"),
            CASE.replace("\"bpb\"", "\"size\""),
        ] {
            assert!(parse_baseline(&damaged).is_err(), "accepted {damaged}");
        }
    }

    #[test]
    fn parse_baseline_names_the_line_of_a_case_the_text_never_closes() {
        let unclosed = CASE.replace("\n  }\n}\n", "\n");
        let err = parse_baseline(&unclosed).unwrap_err();
        assert_eq!(err.line, 2);
        assert_eq!(err.content, "  \"a\": {");
    }

    #[test]
    fn parse_baseline_accepts_a_trailing_comma_or_not() {
        let with_comma = CASE.replace("\n  }\n", ",\n  },\n");
        assert_eq!(
            parse_baseline(&with_comma).unwrap(),
            parse_baseline(CASE).unwrap()
        );
    }

    #[test]
    fn parse_baseline_accepts_the_empty_object() {
        assert!(parse_baseline("{\n}\n").unwrap().cases.is_empty());
    }

    #[test]
    fn fnv1a_matches_the_published_vectors() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn output_changes_names_a_new_digest_at_equal_bits_per_byte() {
        let baseline = snapshot(&[("same", 2.0, 1), ("moved", 2.0, 2), ("gone", 2.0, 3)]);
        let measured = snapshot(&[("same", 2.0, 1), ("moved", 2.0, 9), ("new", 2.0, 4)]);
        assert_eq!(output_changes(&baseline, &measured), ["moved", "new"]);
        assert_eq!(output_changes(&baseline, &baseline), Vec::<String>::new());
    }

    #[test]
    fn snapshot_with_bpb_replaces_numbers_and_keeps_digests() {
        let measured = snapshot(&[("a", 2.0, 7), ("b", 3.0, 8)]);
        let held = BTreeMap::from([("a".to_string(), 1.5)]);
        assert_eq!(
            measured.with_bpb(&held),
            snapshot(&[("a", 1.5, 7), ("b", 3.0, 8)])
        );
        assert_eq!(measured.bpb(), map(&[("a", 2.0), ("b", 3.0)]));
    }

    #[test]
    fn regressions_is_empty_when_measured_matches_baseline() {
        let mut baseline = BTreeMap::new();
        baseline.insert("a".to_string(), 2.0);
        let measured = baseline.clone();
        assert_eq!(regressions(&baseline, &measured), []);
    }

    #[test]
    fn regressions_is_empty_within_tolerance() {
        let mut baseline = BTreeMap::new();
        baseline.insert("a".to_string(), 2.0);
        let mut measured = BTreeMap::new();
        measured.insert("a".to_string(), 2.0 + TOLERANCE_BITS);
        assert_eq!(regressions(&baseline, &measured), []);
    }

    #[test]
    fn regressions_reports_a_case_past_tolerance() {
        let mut baseline = BTreeMap::new();
        baseline.insert("a".to_string(), 2.0);
        let mut measured = BTreeMap::new();
        measured.insert("a".to_string(), 2.0 + TOLERANCE_BITS + 0.001);
        let regs = regressions(&baseline, &measured);
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].name, "a");
        assert!((regs[0].delta() - (TOLERANCE_BITS + 0.001)).abs() < 1e-9);
    }

    #[test]
    fn regressions_ignores_cases_missing_from_either_side() {
        let mut baseline = BTreeMap::new();
        baseline.insert("old_only".to_string(), 2.0);
        let mut measured = BTreeMap::new();
        measured.insert("new_only".to_string(), 20.0);
        assert_eq!(regressions(&baseline, &measured), []);
    }

    #[test]
    fn regressions_improvement_is_not_a_regression() {
        let mut baseline = BTreeMap::new();
        baseline.insert("a".to_string(), 2.0);
        let mut measured = BTreeMap::new();
        measured.insert("a".to_string(), 1.0);
        assert_eq!(regressions(&baseline, &measured), []);
    }

    fn map(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs.iter().map(|&(n, v)| (n.to_string(), v)).collect()
    }

    #[test]
    fn deltas_reports_both_signs_and_skips_one_sided_cases() {
        let baseline = map(&[("better", 2.0), ("worse", 2.0), ("old_only", 1.0)]);
        let measured = map(&[("better", 1.5), ("worse", 2.5), ("new_only", 9.0)]);
        let found = deltas(&baseline, &measured);
        let names: Vec<&str> = found.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["better", "worse"]);
        assert!((found[0].delta() + 0.5).abs() < 1e-9);
        assert!((found[1].delta() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn mean_delta_is_the_per_case_mean_and_zero_for_no_cases() {
        let baseline = map(&[("a", 2.0), ("b", 2.0), ("c", 2.0)]);
        let measured = map(&[("a", 1.0), ("b", 2.0), ("c", 2.5)]);
        let mean = mean_delta(&deltas(&baseline, &measured));
        assert!((mean + 0.5 / 3.0).abs() < 1e-9, "got {mean}");
        assert!(mean_delta(&[]).abs() < f64::EPSILON);
    }

    #[test]
    fn hold_at_best_keeps_a_grown_case_and_takes_a_shrunk_or_new_one() {
        let baseline = map(&[
            ("grown", 2.0),
            ("shrunk", 2.0),
            ("same", 2.0),
            ("old_only", 1.0),
        ]);
        let measured = map(&[
            ("grown", 2.01),
            ("shrunk", 1.5),
            ("same", 2.000_000_04),
            ("new_only", 9.0),
        ]);
        let (written, held) = hold_at_best(&baseline, &measured);
        assert_eq!(
            written,
            map(&[
                ("grown", 2.0),
                ("new_only", 9.0),
                ("same", 2.000_000_04),
                ("shrunk", 1.5)
            ])
        );
        assert_eq!(
            held.len(),
            1,
            "only a case growing past the file's six decimals is held"
        );
        assert_eq!(held[0].name, "grown");
        assert!((held[0].baseline_bpb - 2.0).abs() < f64::EPSILON);
        assert!((held[0].measured_bpb - 2.01).abs() < f64::EPSILON);
    }
}
