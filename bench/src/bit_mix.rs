//! Charter #1922's literal coder class (b),
//! [`mothergod::literal::bit_mix::BitMix`], priced by
//! [`crate::literal_pairing`] against the default level's literal model,
//! [`Literal::walk_logit_sse`], on the parse's literal stream
//! (`research/JOURNAL.md` S2-A159). Both sides are ideal cost.

use crate::cm_path::cost_bits;
use crate::literal_pairing::{self, PairingReport};
use mothergod::literal::bit_mix::BitMix;
use mothergod::literal::{Context, Literal, SurpriseLogisticMix};

/// The champion's pricer: [`Literal::walk_logit_sse`] under a fresh
/// [`SurpriseLogisticMix`], the walk every non-transposed `Lz` frame's
/// literals take.
pub fn champion() -> impl FnMut(&mut Literal, Context, u8) -> f64 {
    let mut mixer = SurpriseLogisticMix::new();
    move |model: &mut Literal, context, byte| {
        let symbol = usize::from(byte);
        let mut bits = 0.0;
        model.walk_logit_sse(context, &mut mixer, |mid, p| {
            let bit = symbol >= mid;
            bits += cost_bits(bit, p);
            bit
        });
        bits
    }
}

/// The candidate's pricer: a fresh [`BitMix`], ignoring the [`Literal`]
/// [`literal_pairing::literal_replay`] hands it.
pub fn candidate() -> impl FnMut(&mut Literal, Context, u8) -> f64 {
    let mut model = BitMix::new();
    move |_: &mut Literal, context, byte| {
        let symbol = usize::from(byte);
        let mut bits = 0.0;
        model.walk(context, |mid, p| {
            let bit = symbol >= mid;
            bits += cost_bits(bit, p);
            bit
        });
        bits
    }
}

/// The recorded experiment: [`candidate`] against [`champion`] over the
/// eleven train cases and both sealed kinds.
#[must_use]
pub fn train_and_sealed_delta_bpb() -> PairingReport {
    literal_pairing::train_and_sealed_delta_bpb(champion, candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal_pairing::{case_delta_bpb, literal_replay};

    /// A text whose parse leaves literals: words recombined, so the
    /// parse copies some and codes the rest.
    fn text() -> Vec<u8> {
        let words = ["mother", "god", "literal", "coder", "class", "bit", "mix"];
        (0..2_000)
            .flat_map(|i: usize| {
                let word = words[(i * 7 + i / 3) % words.len()];
                word.bytes().chain(std::iter::once(b' '))
            })
            .collect()
    }

    #[test]
    fn each_pricer_charges_every_literal_a_positive_cost_under_twelve_bits_a_node() {
        let data = text();
        for (name, bits) in [
            ("champion", literal_replay(&data, champion())),
            ("candidate", literal_replay(&data, candidate())),
        ] {
            let (literals, total) = bits;
            assert!(!literals.is_empty(), "{name}: the parse left no literal");
            #[allow(clippy::cast_precision_loss, reason = "a test buffer's literal count")]
            let ceiling = 8.0 * 12.0 * literals.len() as f64;
            assert!(
                total > 0.0 && total < ceiling,
                "{name}: {total} bits for {} literals",
                literals.len()
            );
        }
    }

    #[test]
    fn the_candidate_prices_differently_from_the_champion() {
        let delta = case_delta_bpb(&text(), champion(), candidate());
        assert!(delta.is_finite() && delta != 0.0, "delta {delta}");
    }
}
