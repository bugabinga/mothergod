//! Charter #1763's candidate, [`mothergod::cm_path::CmPath`], priced by
//! [`crate::whole_stream`] against the hybrid's real bitstream
//! (`research/JOURNAL.md` S2-A133, ADR-0077).
//!
//! The model lives in the codec crate, where [`mothergod::cm_path::encode`]
//! codes it through the range coder; this module prices the same walk at
//! ideal cost, and [`coded_bits`] reads the coded stream's real size.
//! `cost_bits` stays libm `log2` (`bench/clippy.toml`) because it prices a
//! decision, and a stream codes that decision instead.

use crate::literal_pairing::{self, PairingReport};
use crate::whole_stream;
use mothergod::cm_path::CmPath;
use mothergod::codec;
use mothergod::filters::select;
use mothergod::literal::Context;

/// Bits a decision costs: `bit` coded at `probability_of_one`, which
/// [`CmPath::walk`] has already clamped off certainty.
fn cost_bits(bit: bool, probability_of_one: f64) -> f64 {
    -(if bit {
        probability_of_one
    } else {
        1.0 - probability_of_one
    })
    .log2()
}

/// Bits `byte` costs after `prefix` under `context`, then every part of
/// `model` learns `byte`: the shape [`whole_stream::whole_stream_bits`]
/// hands a candidate.
pub fn price(model: &mut CmPath, context: Context, prefix: &[u8], byte: u8) -> f64 {
    let symbol = usize::from(byte);
    let mut bits = 0.0;
    model.walk(context, prefix, |mid, p| {
        let bit = symbol >= mid;
        bits += cost_bits(bit, p);
        bit
    });
    bits
}

/// The recorded experiment: a fresh [`CmPath`] per case over the 11
/// `baseline::cases()` and both sealed kinds, against `compress`'s real
/// bitstream (`research/JOURNAL.md` S2-A133).
#[must_use]
pub fn train_and_sealed_vs_real_bitstream() -> PairingReport {
    whole_stream::train_and_sealed_vs_real_bitstream(|| {
        let mut candidate = CmPath::new();
        move |context, prefix, byte| price(&mut candidate, context, prefix, byte)
    })
}

/// Bits of the shortest stream [`mothergod::cm_path::encode`] codes `data`
/// to behind any filter [`select::pick`] shortlists, each trial through a
/// fresh model: [`whole_stream::filtered_whole_stream_bits`]'s search with
/// the range coder in place of the ideal price. The stream alone, short of
/// whatever a frame carrying it would add around it.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "a single measurement case stays far below 2^53 bits"
)]
pub fn coded_bits(data: &[u8]) -> f64 {
    select::pick(data)
        .into_iter()
        .map(|filter| mothergod::cm_path::encode(&codec::apply_filter(filter, data)).len())
        .min()
        .map_or(0.0, |len| (len * 8) as f64)
}

/// [`train_and_sealed_vs_real_bitstream`]'s cases and champion with the
/// candidate's bits [`coded_bits`]: the real coder's size in place of the
/// ideal price, so a case's delta against that report is the coder's
/// overhead (`research/JOURNAL.md` S2-A140).
#[must_use]
pub fn coded_train_and_sealed_vs_real_bitstream() -> PairingReport {
    literal_pairing::pair_cases(|data| whole_stream::delta_vs_real_bitstream(data, coded_bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mothergod::literal::{Literal, SurpriseLogisticMix};

    /// Prices `data` through `model`, byte by byte, as the driver does.
    fn price_all(model: &mut CmPath, data: &[u8]) -> Vec<f64> {
        let mut context = Context::default();
        data.iter()
            .enumerate()
            .map(|(position, &byte)| {
                let bits = price(model, context, &data[..position], byte);
                context = context.after_literal(byte);
                bits
            })
            .collect()
    }

    #[test]
    fn a_fresh_candidate_prices_the_first_byte_as_the_wire_model_does() {
        let byte = b'q';
        let mut wire = Literal::new();
        let mut wire_mixer = SurpriseLogisticMix::new();
        let mut wire_bits = 0.0;
        wire.walk_logit_sse(Context::default(), &mut wire_mixer, |mid, p| {
            let bit = usize::from(byte) >= mid;
            wire_bits += cost_bits(bit, p);
            bit
        });

        let bits = price(&mut CmPath::new(), Context::default(), &[], byte);

        assert!(
            (bits - wire_bits).abs() < 1e-9,
            "weight 1 on the wire model, 0 elsewhere: {bits} vs {wire_bits}"
        );
    }

    #[test]
    fn every_byte_costs_finite_positive_bits_and_a_rerun_prices_identically() {
        let data: Vec<u8> = (0u32..3000)
            .map(|i| u8::try_from(i * 7 % 251).expect("below 251"))
            .collect();
        let first = price_all(&mut CmPath::new(), &data);
        let second = price_all(&mut CmPath::new(), &data);
        assert!(first.iter().all(|b| b.is_finite() && *b > 0.0));
        assert_eq!(first, second, "no state leaks between fresh candidates");
    }

    /// The range coder pays the ideal price plus its quantization and
    /// flush: on text the model learns, the real stream lands within a
    /// fraction of a percent and a few bytes of the bits this module
    /// prices, so a verdict read at ideal cost is the size a frame would
    /// carry.
    #[test]
    fn the_coded_stream_is_the_priced_bits_within_the_coders_overhead() {
        let data = b"the quick brown fox jumps over the lazy dog; pack my box with \
            five dozen liquor jugs. sphinx of black quartz, judge my vow. "
            .repeat(80);

        let priced: f64 = price_all(&mut CmPath::new(), &data).iter().sum();
        let coded = mothergod::cm_path::encode(&data).len();

        #[allow(
            clippy::cast_precision_loss,
            reason = "a coded test stream is far below 2^53 bytes"
        )]
        let coded_bits = (coded * 8) as f64;
        assert!(coded_bits >= priced - 8.0, "{coded_bits} vs {priced}");
        assert!(
            coded_bits <= priced * 1.005 + 64.0,
            "{coded_bits} vs {priced}"
        );
    }

    /// A little-endian `u32` counter stepping by a constant: `pick`
    /// shortlists `Delta(4)`, which turns it into one repeated row, so the
    /// shortest trial is not the identity's and [`coded_bits`] must find
    /// it among the others.
    fn counter_rows() -> Vec<u8> {
        (0u32..1500)
            .flat_map(|i| i.wrapping_mul(40_503).to_le_bytes())
            .collect()
    }

    #[test]
    fn coded_bits_is_the_shortest_shortlisted_trial_in_bits() {
        let data = counter_rows();
        let trials: Vec<usize> = select::pick(&data)
            .into_iter()
            .map(|filter| mothergod::cm_path::encode(&codec::apply_filter(filter, &data)).len())
            .collect();
        let identity = mothergod::cm_path::encode(&data).len();
        let shortest = *trials.iter().min().expect("pick shortlists the identity");
        assert!(shortest < identity, "a filter must win here: {trials:?}");

        let bits = coded_bits(&data);

        #[allow(
            clippy::cast_precision_loss,
            reason = "a test stream is far below 2^53 bits"
        )]
        let expected = (shortest * 8) as f64;
        assert!((bits - expected).abs() < 1e-9, "{bits} vs {expected}");
    }

    /// The coded search lands where the priced search does, within the
    /// coder's overhead, so the two reports differ by that overhead alone.
    #[test]
    fn coded_bits_is_the_filtered_price_within_the_coders_overhead() {
        let data = counter_rows();
        let priced = whole_stream::filtered_whole_stream_bits(&data, || {
            let mut candidate = CmPath::new();
            move |context, prefix, byte| price(&mut candidate, context, prefix, byte)
        });

        let coded = coded_bits(&data);

        assert!(coded >= priced - 8.0, "{coded} vs {priced}");
        assert!(coded <= priced * 1.005 + 64.0, "{coded} vs {priced}");
    }
}
