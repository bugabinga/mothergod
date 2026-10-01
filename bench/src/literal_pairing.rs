//! One pairing driver for champion-vs-candidate ideal-cost measurement
//! over the literal sub-stream, so no scratch binary redoes
//! `lz::parse_optimal` plus the champion/candidate replay by hand (issue
//! #828). S2-A104, S2-R27 and S2-A106 each wrote that replay again; the
//! third (`bench/src/bin/scratch_logit_sse.rs`, deleted with its verdict)
//! divided its bit delta by the case's literal-byte count instead of its
//! whole length, overstating the train mean 80x and `base64_wrapped` 19x
//! (PR #823). [`literal_replay`] is the shared single-case primitive
//! (also `tans_literal_measure`'s own champion pricer, so the walk has a
//! second caller and the lint gate never sees it dead);
//! [`train_and_sealed_delta_bpb`] is the standard pairing: the eleven
//! `baseline::cases()` plus `access_log`/`gradient_image` sealed at
//! `sealed_seed(baseline::CASE_SEED)`, same length, denominated by
//! `research/README.md`'s schema (`CASE_LEN`, never the literal count).

use crate::baseline::{self, CASE_LEN, CASE_SEED};
use crate::sealed_seed;
use mothergod::literal::{Context, Literal};
use mothergod::lz::{self, Token};

/// Walks `data`'s real optimal parse in coding order
/// (`lz::parse_optimal`, `Context::after_literal`/`after_copy`), pricing
/// each literal byte through `price` against a freshly constructed
/// [`Literal`] model, and returns the literal bytes in coding order
/// alongside `price`'s summed bits. Match/rep spans advance the walked
/// `Context` without pricing: this measures the literal sub-stream alone,
/// same convention every ideal-cost pairing in `research/JOURNAL.md`'s
/// S2-A1xx line has used.
pub fn literal_replay<P>(data: &[u8], mut price: P) -> (Vec<u8>, f64)
where
    P: FnMut(&mut Literal, Context, u8) -> f64,
{
    let tokens = lz::parse_optimal(data);
    let mut model = Literal::new();
    let mut context = Context::default();
    let mut pos = 0usize;
    let mut literal_bytes = Vec::new();
    let mut bits = 0.0f64;

    for token in &tokens {
        match *token {
            Token::Literal(byte) => {
                bits += price(&mut model, context, byte);
                literal_bytes.push(byte);
                context = context.after_literal(byte);
                pos += 1;
            }
            Token::Match { len, .. } | Token::Rep { len, .. } => {
                let end = pos + len as usize;
                context = context.after_copy(&data[pos..end]);
                pos = end;
            }
        }
    }
    (literal_bytes, bits)
}

/// One case's ideal-cost delta between a candidate and the champion,
/// `(candidate_bits - champion_bits) / data.len()`: `research/README.md`'s
/// schema denominator, the whole case, never the literal count alone, so a
/// mostly-match buffer is not inflated the way it174 was.
#[allow(
    clippy::cast_precision_loss,
    reason = "a single measurement case stays far below 2^53 bytes (CASE_LEN is 50,000)"
)]
pub fn case_delta_bpb<C, D>(data: &[u8], champion: C, candidate: D) -> f64
where
    C: FnMut(&mut Literal, Context, u8) -> f64,
    D: FnMut(&mut Literal, Context, u8) -> f64,
{
    let (_, champion_bits) = literal_replay(data, champion);
    let (_, candidate_bits) = literal_replay(data, candidate);
    (candidate_bits - champion_bits) / data.len() as f64
}

/// One named case's [`case_delta_bpb`] result.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseDelta {
    /// Case name (`baseline::Case::name`, or `"access_log (sealed)"`/
    /// `"gradient_image (sealed)"` for the two sealed kinds).
    pub name: &'static str,
    /// `(candidate_bits - champion_bits) / CASE_LEN`.
    pub delta_bpb: f64,
}

/// A full [`train_and_sealed_delta_bpb`] run: the shape
/// `research/progress.jsonl` and a `research/JOURNAL.md` entry expect.
#[derive(Debug, Clone, PartialEq)]
pub struct PairingReport {
    /// One [`CaseDelta`] per `baseline::cases()` entry, same order.
    pub train: Vec<CaseDelta>,
    /// Mean of `train`'s deltas (`progress.jsonl`'s `train_delta_bpb`).
    pub train_mean: f64,
    /// Sum of `train`'s deltas.
    pub train_sum: f64,
    /// `access_log (sealed)` then `gradient_image (sealed)`
    /// (`progress.jsonl`'s `val_delta_bpb` is the sealed set's own mean or,
    /// as every S2-A1xx entry on this lead has, both named individually).
    pub sealed: Vec<CaseDelta>,
}

/// The two sealed-only cases every pairing in this lead measures,
/// `CASE_LEN` bytes at `sealed_seed(CASE_SEED)`: `baseline::cases()`
/// excludes both by design (`research/corpus/POLICY.md`'s "no agent ever
/// tunes against it"), so a pairing driver that also wants the sealed
/// read generates them itself, same length and seed rule.
fn sealed_cases() -> [(&'static str, Vec<u8>); 2] {
    let seed = sealed_seed(CASE_SEED);
    [
        ("access_log (sealed)", crate::access_log(CASE_LEN, seed)),
        (
            "gradient_image (sealed)",
            crate::gradient_image(CASE_LEN, seed),
        ),
    ]
}

/// The standard ideal-cost pairing: every `baseline::cases()` train case
/// plus both sealed cases, each priced twice from independent fresh state
/// (a new [`Literal`] per [`literal_replay`] call, a new `champion_for_case`/
/// `candidate_for_case` instance per data case so neither pricer's own
/// extra state, e.g. a mixer `Literal`'s ideal-cost methods take alongside
/// `Context`, carries over between unrelated corpora). `champion_for_case`/
/// `candidate_for_case` build a fresh pricing closure for each case in
/// turn; a pricer with no extra state beyond `Literal` itself (e.g.
/// `Literal::ideal_cost_bits_sse`) can return the same method reference
/// every time.
pub fn train_and_sealed_delta_bpb<CF, C, DF, D>(
    mut champion_for_case: CF,
    mut candidate_for_case: DF,
) -> PairingReport
where
    CF: FnMut() -> C,
    C: FnMut(&mut Literal, Context, u8) -> f64,
    DF: FnMut() -> D,
    D: FnMut(&mut Literal, Context, u8) -> f64,
{
    let train: Vec<CaseDelta> = baseline::cases()
        .into_iter()
        .map(|case| CaseDelta {
            name: case.name,
            delta_bpb: case_delta_bpb(&case.data, champion_for_case(), candidate_for_case()),
        })
        .collect();
    let train_sum: f64 = train.iter().map(|c| c.delta_bpb).sum();
    #[allow(
        clippy::cast_precision_loss,
        reason = "train case count is baseline::cases().len(), well under 2^53"
    )]
    let train_mean = train_sum / train.len() as f64;

    let sealed: Vec<CaseDelta> = sealed_cases()
        .into_iter()
        .map(|(name, data)| CaseDelta {
            name,
            delta_bpb: case_delta_bpb(&data, champion_for_case(), candidate_for_case()),
        })
        .collect();

    PairingReport {
        train,
        train_mean,
        train_sum,
        sealed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `case_delta_bpb` must divide by `data.len()`, not by the literal
    /// count: the it174 bug (PR #823) divided by literal count instead and
    /// overstated a mostly-match buffer's delta 19x. A buffer that is
    /// almost all one repeated byte parses to a handful of leading
    /// literals and one long match, so its literal count is far below its
    /// length; a champion/candidate pair that charges a fixed extra cost
    /// per literal makes the two denominators give visibly different
    /// answers.
    #[test]
    fn case_delta_bpb_divides_by_case_length_not_literal_count() {
        let data = vec![b'x'; 10_000];
        let extra_bits_per_literal = 3.0;
        let delta = case_delta_bpb(
            &data,
            |model: &mut Literal, context, byte| model.ideal_cost_bits_sse(context, byte),
            |model: &mut Literal, context, byte| {
                model.ideal_cost_bits_sse(context, byte) + extra_bits_per_literal
            },
        );

        let (literal_bytes, _) = literal_replay(&data, |model: &mut Literal, context, byte| {
            model.ideal_cost_bits_sse(context, byte)
        });
        let literal_count = literal_bytes.len();
        assert!(
            literal_count < data.len(),
            "a long run of one byte must parse to far fewer literals than its length"
        );

        #[allow(clippy::cast_precision_loss, reason = "test data stays tiny")]
        let expected_by_length = extra_bits_per_literal * literal_count as f64 / data.len() as f64;
        #[allow(clippy::cast_precision_loss, reason = "test data stays tiny")]
        let wrong_by_literal_count =
            extra_bits_per_literal * literal_count as f64 / literal_count as f64;

        assert!(
            (delta - expected_by_length).abs() < 1e-9,
            "expected {expected_by_length}, got {delta}"
        );
        assert!(
            (delta - wrong_by_literal_count).abs() > 1e-6,
            "delta must not match the literal-count denominator ({wrong_by_literal_count}): \
             that is the it174 bug this function exists to prevent"
        );
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "champion and candidate are the identical closure over the identical replay, \
                  so the delta is the literal 0.0 (bits - itself), not a computed float \
                  tolerance comparison would be the wrong check for"
    )]
    fn case_delta_bpb_is_zero_when_candidate_equals_champion() {
        let data = baseline::cases().into_iter().next().unwrap().data;
        let delta = case_delta_bpb(
            &data,
            |model: &mut Literal, context, byte| model.ideal_cost_bits_sse(context, byte),
            |model: &mut Literal, context, byte| model.ideal_cost_bits_sse(context, byte),
        );
        assert_eq!(delta, 0.0);
    }

    #[test]
    #[allow(
        clippy::float_cmp,
        reason = "champion and candidate are the identical closure over the identical replay, \
                  so every delta, and the mean/sum of all-zero deltas, is the literal 0.0"
    )]
    fn train_and_sealed_delta_bpb_covers_every_baseline_case_plus_both_sealed_kinds() {
        let report = train_and_sealed_delta_bpb(
            || |model: &mut Literal, context, byte| model.ideal_cost_bits_sse(context, byte),
            || |model: &mut Literal, context, byte| model.ideal_cost_bits_sse(context, byte),
        );
        assert_eq!(report.train.len(), baseline::cases().len());
        assert_eq!(report.sealed.len(), 2);
        assert_eq!(report.sealed[0].name, "access_log (sealed)");
        assert_eq!(report.sealed[1].name, "gradient_image (sealed)");
        // Candidate equals champion everywhere, so every delta and the
        // train mean/sum collapse to zero.
        assert!(report.train.iter().all(|c| c.delta_bpb == 0.0));
        assert_eq!(report.train_mean, 0.0);
        assert_eq!(report.train_sum, 0.0);
        assert!(report.sealed.iter().all(|c| c.delta_bpb == 0.0));
    }
}
