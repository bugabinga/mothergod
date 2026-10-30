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
//! `bench/baseline.json` holds. Both sides choose among the same filters:
//! the candidate prices every filter [`select::pick`] shortlists through
//! [`codec::apply_filter`] and keeps the cheapest, as the codec's search
//! keeps the smallest body (`research/JOURNAL.md` S1-A2, S2-A135). What
//! the delta does not net out: the candidate pays no frame header and no
//! filter selector, which favours it. The verdict rules are
//! `research/corpus/POLICY.md`'s, unchanged.

use crate::baseline::bits_per_byte;
use crate::literal_pairing::{self, PairingReport};
use mothergod::codec;
use mothergod::filters::select;
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

/// The fewest bits [`whole_stream_bits`] prices `data` at behind any filter
/// [`select::pick`] shortlists for it, each filter's bytes through a fresh
/// model from `candidate_for_trial`: the search `codec::encode` runs, with
/// the candidate in the parse's seat.
pub fn filtered_whole_stream_bits<DF, D>(data: &[u8], mut candidate_for_trial: DF) -> f64
where
    DF: FnMut() -> D,
    D: FnMut(Context, &[u8], u8) -> f64,
{
    select::pick(data)
        .into_iter()
        .map(|filter| whole_stream_bits(&codec::apply_filter(filter, data), candidate_for_trial()))
        .fold(f64::INFINITY, f64::min)
}

/// One case's `(candidate_bits / data.len()) - champion_bpb`:
/// `research/README.md`'s schema denominator, the case's whole length, and
/// the champion the real `compress` output over the same bytes. The
/// candidate's bits are [`filtered_whole_stream_bits`], so both sides
/// choose among the same filters. `0.0` for an empty `data`, the
/// convention [`bits_per_byte`] already holds.
#[allow(
    clippy::cast_precision_loss,
    reason = "a single measurement case stays far below 2^53 bytes (CASE_LEN is 50,000)"
)]
pub fn case_delta_vs_real_bitstream<DF, D>(data: &[u8], candidate_for_trial: DF) -> f64
where
    DF: FnMut() -> D,
    D: FnMut(Context, &[u8], u8) -> f64,
{
    if data.is_empty() {
        return 0.0;
    }
    let champion_bpb = bits_per_byte(mothergod::compress(data).len(), data.len());
    filtered_whole_stream_bits(data, candidate_for_trial) / data.len() as f64 - champion_bpb
}

/// The standard whole-stream pairing: every `baseline::cases()` train case
/// plus both sealed cases, each filter trial priced by a fresh closure from
/// `candidate_for_trial` (a candidate's state never carries between
/// unrelated corpora, nor between two filterings of one) against
/// `mothergod::compress` on the same bytes.
/// Same cases, same order and same report as
/// [`literal_pairing::train_and_sealed_delta_bpb`], so a verdict reads
/// both alike.
pub fn train_and_sealed_vs_real_bitstream<DF, D>(mut candidate_for_trial: DF) -> PairingReport
where
    DF: FnMut() -> D,
    D: FnMut(Context, &[u8], u8) -> f64,
{
    literal_pairing::pair_cases(|data| case_delta_vs_real_bitstream(data, &mut candidate_for_trial))
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
    /// `!` is outside the base64 alphabet, so every shortlisted filter
    /// keeps the length.
    #[test]
    fn case_delta_is_candidate_bpb_minus_real_bpb_over_the_case_length() {
        let data = vec![b'!'; 10_000];
        let champion_bpb = bits_per_byte(mothergod::compress(&data).len(), data.len());
        assert!(champion_bpb < 0.1, "a one-byte run is nearly free");

        let delta = case_delta_vs_real_bitstream(&data, || |_, _, _| FLAT_BITS);

        assert!(
            (delta - (FLAT_BITS - champion_bpb)).abs() < 1e-9,
            "expected {}, got {delta}",
            FLAT_BITS - champion_bpb
        );
    }

    /// A run of `x` is canonical base64, so `pick` shortlists the unwrap,
    /// whose output is three bytes for every four: a flat candidate behind
    /// it costs 6 bits per original byte, the cheapest trial, and the
    /// driver keeps it over every length-preserving filter's 8.
    #[test]
    fn the_candidate_pays_its_cheapest_shortlisted_filter() {
        let data = vec![b'x'; 10_000];
        assert!(select::pick(&data).contains(&select::Candidate::Base64));

        let bits = filtered_whole_stream_bits(&data, || |_, _, _| FLAT_BITS);

        assert!((bits - 6.0 * 10_000.0).abs() < 1e-9, "got {bits}");
    }

    #[test]
    fn case_delta_of_an_empty_case_is_zero() {
        let delta = case_delta_vs_real_bitstream(&[], || |_, _, _| panic!("no byte to price"));
        assert!(delta.abs() < 1e-12, "got {delta}");
    }

    /// The six shipped experts over the whole stream, the charter's first
    /// try, run through the driver end to end on a case where the answer
    /// is known in sign: bytes that cost under 8 bits each through the
    /// model must land the candidate below the flat price, and the same
    /// model must cost more than the champion's LZ on a long repeat. The
    /// period is longer than any delta stride `pick` tries, so no filter
    /// turns the repeat into a run.
    #[test]
    fn six_experts_over_the_whole_stream_price_a_repeat_above_the_real_bitstream() {
        let sentence: &[u8] = b"the quick brown fox jumps over the lazy dog; pack my box with \
            five dozen liquor jugs. sphinx of black quartz, judge my vow. ";
        assert!(sentence.len() > 96, "past `select`'s widest delta stride");
        let data: Vec<u8> = sentence.iter().copied().cycle().take(20_000).collect();

        let delta = case_delta_vs_real_bitstream(&data, || {
            let mut model = Literal::new();
            move |context, _, byte| model.ideal_cost_bits_sse(context, byte)
        });
        let flat_delta = case_delta_vs_real_bitstream(&data, || |_, _, _| FLAT_BITS);

        assert!(delta < flat_delta, "the model learns the text: {delta}");
        assert!(
            delta > 0.0,
            "a period this long is the parse's job: {delta}"
        );
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
