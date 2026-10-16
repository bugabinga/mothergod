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
//! reports bits/byte; [`format_baseline`]/[`parse_baseline`] round-trip that
//! map through `bench/baseline.json`; [`regressions`] compares a freshly
//! measured map against the committed baseline.

use crate::DatasetKind;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;
use std::path::Path;

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

/// Compresses every [`cases`] entry through `mothergod::compress` and
/// reports each one's bits/byte, keyed by [`Case::name`].
#[must_use]
pub fn measure_all() -> BTreeMap<String, f64> {
    cases()
        .iter()
        .map(|case| {
            let compressed = mothergod::compress(&case.data);
            (
                case.name.to_string(),
                bits_per_byte(compressed.len(), case.data.len()),
            )
        })
        .collect()
}

/// Writes `measurements` as the `bench/baseline.json` this module reads
/// back with [`parse_baseline`]: a flat JSON object, keys sorted (`measurements`
/// is a [`BTreeMap`]), each value rounded to six decimal places with the
/// trailing zeros trimmed (`8.0` prints `8`). That is the form `cargo x fmt`
/// leaves a number in, so a freshly written file already passes
/// `cargo x fmt --check` (issue #943). Not a general JSON writer: it only
/// needs to round-trip its own output, the same scope the `corpus` module's
/// manual TOML reader (behind the `corpus-fetch` feature) takes for
/// `bench/corpus.toml`.
#[must_use]
pub fn format_baseline(measurements: &BTreeMap<String, f64>) -> String {
    render(measurements, |bpb| {
        let fixed = format!("{bpb:.6}");
        if fixed.contains('.') {
            fixed
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        } else {
            fixed
        }
    })
}

/// The one object layout, with each value rendered by `value`:
/// [`format_baseline`] writes the trimmed form, [`fingerprint`] hashes the
/// fixed six-decimal one.
fn render(measurements: &BTreeMap<String, f64>, value: fn(f64) -> String) -> String {
    let mut out = String::from("{\n");
    let mut remaining = measurements.len();
    for (name, &bpb) in measurements {
        remaining -= 1;
        let comma = if remaining == 0 { "" } else { "," };
        writeln!(out, "  \"{name}\": {}{comma}", value(bpb))
            .expect("writing to a String never fails");
    }
    out.push_str("}\n");
    out
}

/// A line [`parse_baseline`] could not read as `"<name>": <bpb>`.
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
            "bench/baseline.json line {}: expected `\"name\": bpb`, got `{}`",
            self.line, self.content
        )
    }
}

impl std::error::Error for BaselineParseError {}

/// Parses text in the shape [`format_baseline`] writes back into a
/// name-to-bits/byte map. Not a general JSON reader: it understands exactly
/// one object-of-numbers shape (an opening `{`, one `"name": number` pair
/// per line with an optional trailing comma, a closing `}`) and rejects
/// anything else line by line, the same deliberate scope limit
/// the `corpus` module's manual TOML reader (behind the `corpus-fetch`
/// feature) takes for `bench/corpus.toml`.
///
/// # Errors
///
/// Returns [`BaselineParseError`] naming the first line that is not blank,
/// `{`, `}`, or a `"name": number` pair.
pub fn parse_baseline(text: &str) -> Result<BTreeMap<String, f64>, BaselineParseError> {
    let mut out = BTreeMap::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim().trim_end_matches(',');
        if line.is_empty() || line == "{" || line == "}" {
            continue;
        }
        let err = || BaselineParseError {
            line: idx + 1,
            content: raw_line.to_string(),
        };
        let (key_part, value_part) = line.split_once(':').ok_or_else(err)?;
        let key = key_part.trim();
        let key = key
            .strip_prefix('"')
            .and_then(|k| k.strip_suffix('"'))
            .ok_or_else(err)?;
        let value: f64 = value_part.trim().parse().map_err(|_| err())?;
        out.insert(key.to_string(), value);
    }
    Ok(out)
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

/// Short, stable fingerprint of a baseline measurement map: the canonical
/// text (sorted keys, fixed six-decimal values, `1.500000`) run through a
/// 64-bit FNV-1a hash, printed as 16 lowercase hex digits. Two maps with the
/// same values fingerprint identically regardless of the source file's exact
/// bytes (key order, whitespace), since both go through the same layout
/// first.
///
/// The hash input is frozen at the fixed six-decimal form, not
/// [`format_baseline`]'s trimmed one: the committed finals reports embed
/// this digest, and a different input moves it without the baseline moving.
///
/// The held-out-final reports (`bench::finals::format_report`) embed this
/// for the `bench/baseline.json` they were generated against, so
/// `baseline_gate` can detect a baseline that moved without those reports
/// following (issue #327) — a content invariant, not a PR-diff heuristic,
/// so it holds regardless of which commit last touched which file. Not a
/// cryptographic hash: this only needs to detect an honest accidental
/// mismatch, never resist a deliberate one, so a dependency-free 64-bit
/// FNV-1a is the right size for the job (ADR-0002's zero-dependency rule
/// binds the core crate; this is bench tooling, but pulling in `sha2` — see
/// this crate's `Cargo.toml` — for a freshness marker nobody attacks would
/// tax every default-feature build for a collision resistance this job
/// never needed).
#[must_use]
pub fn fingerprint(measurements: &BTreeMap<String, f64>) -> String {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    let canonical = render(measurements, |bpb| format!("{bpb:.6}"));
    let mut hash = FNV_OFFSET_BASIS;
    for byte in canonical.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

/// Reads `bench/baseline.json` under `root`, parses it, and fingerprints
/// it: the read-parse-fingerprint sequence `finals_report` and
/// `silesia_report` both need before compressing anything (issue #330,
/// this exact block regrew once already after it95 first consolidated it).
///
/// # Errors
///
/// A human-readable message if the file cannot be read or its content
/// fails [`parse_baseline`].
pub fn load_and_fingerprint(root: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(root.join("bench/baseline.json"))
        .map_err(|err| format!("failed to read bench/baseline.json: {err}"))?;
    let baseline = parse_baseline(&text).map_err(|err| err.to_string())?;
    Ok(fingerprint(&baseline))
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
        BaselineParseError, CASE_LEN, TOLERANCE_BITS, bits_per_byte, cases, deltas, fingerprint,
        format_baseline, hold_at_best, mean_delta, parse_baseline, regressions,
    };
    use std::collections::BTreeMap;

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
        let mut measurements = BTreeMap::new();
        measurements.insert("a".to_string(), 1.5);
        measurements.insert("b".to_string(), 7.999_999);
        let text = format_baseline(&measurements);
        let parsed = parse_baseline(&text).expect("format_baseline's own output must parse");
        assert_eq!(parsed.len(), measurements.len());
        for (name, &value) in &measurements {
            assert!(
                (parsed[name] - value).abs() < 1e-6,
                "{name} did not round-trip"
            );
        }
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
        let mut measurements = BTreeMap::new();
        measurements.insert("a".to_string(), 0.530_56);
        measurements.insert("b".to_string(), 8.0);
        measurements.insert("c".to_string(), 10.0);
        measurements.insert("d".to_string(), 2.370_564_9);
        assert_eq!(
            format_baseline(&measurements),
            "{\n  \"a\": 0.53056,\n  \"b\": 8,\n  \"c\": 10,\n  \"d\": 2.370565\n}\n"
        );
    }

    #[test]
    fn format_baseline_sorts_keys() {
        let mut measurements = BTreeMap::new();
        measurements.insert("zebra".to_string(), 1.0);
        measurements.insert("apple".to_string(), 2.0);
        let text = format_baseline(&measurements);
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
    fn parse_baseline_accepts_a_trailing_comma_or_not() {
        let with_comma = parse_baseline("{\n  \"a\": 1.0,\n  \"b\": 2.0\n}\n").unwrap();
        let without_comma = parse_baseline("{\n  \"a\": 1.0,\n  \"b\": 2.0,\n}\n").unwrap();
        assert_eq!(with_comma, without_comma);
    }

    #[test]
    fn parse_baseline_accepts_the_empty_object() {
        assert!(parse_baseline("{\n}\n").unwrap().is_empty());
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

    #[test]
    fn fingerprint_is_deterministic() {
        let mut measurements = BTreeMap::new();
        measurements.insert("a".to_string(), 1.5);
        measurements.insert("b".to_string(), 7.999_999);
        assert_eq!(fingerprint(&measurements), fingerprint(&measurements));
    }

    #[test]
    fn fingerprint_input_is_frozen_at_six_fixed_decimals() {
        // Computed from `{\n  "a": 1.500000,\n  "b": 8.000000\n}\n` before
        // issue #943 trimmed the written form; the finals reports embed it.
        let mut measurements = BTreeMap::new();
        measurements.insert("a".to_string(), 1.5);
        measurements.insert("b".to_string(), 8.0);
        assert_eq!(fingerprint(&measurements), "2939a726f123b9c0");
    }

    #[test]
    fn fingerprint_ignores_map_insertion_order() {
        let mut forward = BTreeMap::new();
        forward.insert("a".to_string(), 1.5);
        forward.insert("b".to_string(), 2.5);
        let mut backward = BTreeMap::new();
        backward.insert("b".to_string(), 2.5);
        backward.insert("a".to_string(), 1.5);
        assert_eq!(fingerprint(&forward), fingerprint(&backward));
    }

    #[test]
    fn fingerprint_changes_when_a_value_changes() {
        let mut before = BTreeMap::new();
        before.insert("a".to_string(), 1.5);
        let mut after = BTreeMap::new();
        after.insert("a".to_string(), 1.500_001);
        assert_ne!(fingerprint(&before), fingerprint(&after));
    }

    #[test]
    fn fingerprint_changes_when_a_key_changes() {
        let mut before = BTreeMap::new();
        before.insert("a".to_string(), 1.5);
        let mut after = BTreeMap::new();
        after.insert("z".to_string(), 1.5);
        assert_ne!(fingerprint(&before), fingerprint(&after));
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
            format_baseline(&written),
            "{\n  \"grown\": 2,\n  \"new_only\": 9,\n  \"same\": 2,\n  \"shrunk\": 1.5\n}\n"
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
