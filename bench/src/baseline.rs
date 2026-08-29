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
/// is a [`BTreeMap`]), six decimal digits per value. Not a general JSON
/// writer — it only needs to round-trip its own output, the same scope
/// the `corpus` module's manual TOML reader (behind the `corpus-fetch`
/// feature) takes for `bench/corpus.toml`.
#[must_use]
pub fn format_baseline(measurements: &BTreeMap<String, f64>) -> String {
    let mut out = String::from("{\n");
    let mut remaining = measurements.len();
    for (name, bpb) in measurements {
        remaining -= 1;
        let comma = if remaining == 0 { "" } else { "," };
        writeln!(out, "  \"{name}\": {bpb:.6}{comma}").expect("writing to a String never fails");
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

/// Short, stable fingerprint of a baseline measurement map: [`format_baseline`]'s
/// canonical text (sorted keys, six-decimal values) run through a 64-bit
/// FNV-1a hash, printed as 16 lowercase hex digits. Two maps with the same
/// values fingerprint identically regardless of the source file's exact
/// bytes (key order, whitespace), since both go through the same canonical
/// formatter first.
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
    let canonical = format_baseline(measurements);
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

#[cfg(test)]
mod tests {
    use super::{
        BaselineParseError, CASE_LEN, TOLERANCE_BITS, bits_per_byte, cases, fingerprint,
        format_baseline, parse_baseline, regressions,
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
        assert!(regressions(&baseline, &measured).is_empty());
    }

    #[test]
    fn regressions_is_empty_within_tolerance() {
        let mut baseline = BTreeMap::new();
        baseline.insert("a".to_string(), 2.0);
        let mut measured = BTreeMap::new();
        measured.insert("a".to_string(), 2.0 + TOLERANCE_BITS);
        assert!(regressions(&baseline, &measured).is_empty());
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
        assert!(regressions(&baseline, &measured).is_empty());
    }

    #[test]
    fn regressions_improvement_is_not_a_regression() {
        let mut baseline = BTreeMap::new();
        baseline.insert("a".to_string(), 2.0);
        let mut measured = BTreeMap::new();
        measured.insert("a".to_string(), 1.0);
        assert!(regressions(&baseline, &measured).is_empty());
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let mut measurements = BTreeMap::new();
        measurements.insert("a".to_string(), 1.5);
        measurements.insert("b".to_string(), 7.999_999);
        assert_eq!(fingerprint(&measurements), fingerprint(&measurements));
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
}
