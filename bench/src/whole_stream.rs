//! Whole-stream ideal-cost pairing: a candidate model priced over every
//! byte of a case, against the champion's real bitstream on the same
//! bytes (charter #1763, ADR-0077).
//!
//! [`crate::literal_pairing`] prices a candidate inside the parse's
//! literal stream, so the LZ parse has already removed the repetition the
//! candidate would model (`research/JOURNAL.md` S1-L6). Here no parse
//! runs: [`whole_stream_bits`] walks `data` once, byte by byte, every byte
//! a literal, and the candidate sees the whole prefix.
//!
//! The champion is `mothergod::compress` at its default level, the number
//! `bench/baseline.json` holds. Two things the delta does not net out,
//! both small against a [`crate::baseline::CASE_LEN`] case: the candidate pays no
//! frame header, and sees the raw bytes where the champion may have run a
//! filter first (`research/JOURNAL.md` S1-A2). A positive delta is a
//! candidate worse than the bitstream; the verdict rules are
//! `research/corpus/POLICY.md`'s, unchanged.

use crate::baseline::bits_per_byte;
use crate::literal_pairing::{self, PairingReport};
use mothergod::literal::Context;

/// Walks `data` in order, pricing each byte through `price` and returning
/// the summed bits.
///
/// `price` receives the byte's [`Context`] (every earlier byte advanced as
/// a literal, so `after_copy` is never set), the bytes before it, and the
/// byte itself. A candidate model with state lives in the closure, fresh
/// per call; a candidate that needs only the prefix, a match model, takes
/// it from the second argument, never from a copy of `data` of its own.
pub fn whole_stream_bits<P>(data: &[u8], mut price: P) -> f64
where
    P: FnMut(Context, &[u8], u8) -> f64,
{
    let mut context = Context::default();
    let mut bits = 0.0f64;
    for (position, &byte) in data.iter().enumerate() {
        bits += price(context, &data[..position], byte);
        context = context.after_literal(byte);
    }
    bits
}

/// One case's `(candidate_bits / data.len()) - champion_bpb`:
/// `research/README.md`'s schema denominator, the case's whole length, and
/// the champion the real `compress` output over the same bytes. `0.0` for
/// an empty `data`, the convention [`bits_per_byte`] already holds.
#[allow(
    clippy::cast_precision_loss,
    reason = "a single measurement case stays far below 2^53 bytes (CASE_LEN is 50,000)"
)]
pub fn case_delta_vs_real_bitstream<P>(data: &[u8], candidate: P) -> f64
where
    P: FnMut(Context, &[u8], u8) -> f64,
{
    if data.is_empty() {
        return 0.0;
    }
    let champion_bpb = bits_per_byte(mothergod::compress(data).len(), data.len());
    whole_stream_bits(data, candidate) / data.len() as f64 - champion_bpb
}

/// The standard whole-stream pairing: every `baseline::cases()` train case
/// plus both sealed cases, each priced by a fresh closure from
/// `candidate_for_case` (a candidate's state never carries between
/// unrelated corpora) against `mothergod::compress` on the same bytes.
/// Same cases, same order and same report as
/// [`literal_pairing::train_and_sealed_delta_bpb`], so a verdict reads
/// both alike.
pub fn train_and_sealed_vs_real_bitstream<DF, D>(mut candidate_for_case: DF) -> PairingReport
where
    DF: FnMut() -> D,
    D: FnMut(Context, &[u8], u8) -> f64,
{
    literal_pairing::pair_cases(|data| case_delta_vs_real_bitstream(data, candidate_for_case()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline;
    use mothergod::literal::Literal;

    /// Bits one byte costs under a model that knows nothing.
    const FLAT_BITS: f64 = 8.0;

    #[test]
    fn whole_stream_bits_hands_each_byte_its_prefix_and_literal_context() {
        let data = b"abc abc";
        let mut seen = Vec::new();
        let bits = whole_stream_bits(data, |context, prefix, byte| {
            seen.push((context, prefix.to_vec(), byte));
            1.0
        });

        assert_eq!(seen.len(), data.len());
        assert!((bits - 7.0).abs() < 1e-12, "summed bits, got {bits}");
        let mut expected = Context::default();
        for (position, (context, prefix, byte)) in seen.iter().enumerate() {
            assert_eq!(*context, expected, "context at {position}");
            assert_eq!(prefix.as_slice(), &data[..position], "prefix at {position}");
            assert_eq!(*byte, data[position]);
            assert!(
                !context.after_copy,
                "no copy token exists on this path, so no byte follows one"
            );
            expected = expected.after_literal(*byte);
        }
    }

    #[test]
    fn whole_stream_bits_of_nothing_is_zero_and_prices_nothing() {
        let bits = whole_stream_bits(&[], |_, _, _| panic!("no byte to price"));
        assert!(bits.abs() < 1e-12, "got {bits}");
    }

    /// The delta is the candidate's bits over the case's length minus the
    /// real bitstream's bits over the same length, not over the bitstream's
    /// own size: a flat 8 bits/byte candidate against a buffer the
    /// champion collapses to a few bytes must land near 8, never near 0.
    #[test]
    fn case_delta_is_candidate_bpb_minus_real_bpb_over_the_case_length() {
        let data = vec![b'x'; 10_000];
        let champion_bpb = bits_per_byte(mothergod::compress(&data).len(), data.len());
        assert!(champion_bpb < 0.1, "a one-byte run is nearly free");

        let delta = case_delta_vs_real_bitstream(&data, |_, _, _| FLAT_BITS);

        assert!(
            (delta - (FLAT_BITS - champion_bpb)).abs() < 1e-9,
            "expected {}, got {delta}",
            FLAT_BITS - champion_bpb
        );
    }

    #[test]
    fn case_delta_of_an_empty_case_is_zero() {
        let delta = case_delta_vs_real_bitstream(&[], |_, _, _| panic!("no byte to price"));
        assert!(delta.abs() < 1e-12, "got {delta}");
    }

    /// The six shipped experts over the whole stream, the charter's first
    /// try, run through the driver end to end on a case where the answer
    /// is known in sign: bytes that cost under 8 bits each through the
    /// model must land the candidate below the flat price, and the same
    /// model must cost more than the champion's LZ on a long repeat.
    #[test]
    fn six_experts_over_the_whole_stream_price_a_repeat_above_the_real_bitstream() {
        let data: Vec<u8> = b"the quick brown fox jumps over the lazy dog. "
            .iter()
            .copied()
            .cycle()
            .take(20_000)
            .collect();

        let mut model = Literal::new();
        let delta = case_delta_vs_real_bitstream(&data, |context, _, byte| {
            model.ideal_cost_bits_sse(context, byte)
        });
        let flat_delta = case_delta_vs_real_bitstream(&data, |_, _, _| FLAT_BITS);

        assert!(delta < flat_delta, "the model learns the text: {delta}");
        assert!(delta > 0.0, "a 45-byte period is the parse's job: {delta}");
    }

    #[test]
    fn the_standard_pairing_reads_the_same_cases_as_the_literal_pairing() {
        let report = train_and_sealed_vs_real_bitstream(|| |_, _, _| FLAT_BITS);
        let literal_report = literal_pairing::train_and_sealed_delta_bpb(
            || |_: &mut Literal, _, _| 0.0,
            || |_: &mut Literal, _, _| 0.0,
        );

        let names = |r: &PairingReport| -> Vec<&'static str> {
            r.train.iter().chain(&r.sealed).map(|c| c.name).collect()
        };
        assert_eq!(names(&report), names(&literal_report));
        assert_eq!(report.train.len(), baseline::cases().len());
        assert_eq!(report.sealed.len(), 2);
    }
}
