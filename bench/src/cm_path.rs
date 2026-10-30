//! Charter #1763's candidate, [`mothergod::cm_path::CmPath`], priced by
//! [`crate::whole_stream`] against the hybrid's real bitstream
//! (`research/JOURNAL.md` S2-A133, ADR-0077).
//!
//! The model lives in the codec crate, where [`mothergod::cm_path::encode`]
//! codes it through the range coder; this module prices the same walk at
//! ideal cost. `cost_bits` stays libm `log2` (`bench/clippy.toml`) because
//! it prices a decision, and a stream codes that decision instead.

use crate::literal_pairing::PairingReport;
use crate::whole_stream;
use mothergod::cm_path::CmPath;
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

/// The recorded experiment: a fresh [`CmPath`] per filter trial, sized by
/// the trial's bytes, over the 11 train cases and both sealed kinds at
/// `len` bytes each, against `compress`'s real bitstream
/// (`research/JOURNAL.md` S2-A133; `CASE_LEN` is `baseline::cases()`,
/// S2-A140 reads a longer `len`).
#[must_use]
pub fn train_and_sealed_vs_real_bitstream(len: usize) -> PairingReport {
    whole_stream::train_and_sealed_vs_real_bitstream(len, |trial_len| {
        let mut candidate = CmPath::new(trial_len);
        move |context, prefix, byte| price(&mut candidate, context, prefix, byte)
    })
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

        let bits = price(&mut CmPath::new(1), Context::default(), &[], byte);

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
        let first = price_all(&mut CmPath::new(data.len()), &data);
        let second = price_all(&mut CmPath::new(data.len()), &data);
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

        let priced: f64 = price_all(&mut CmPath::new(data.len()), &data).iter().sum();
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
}
