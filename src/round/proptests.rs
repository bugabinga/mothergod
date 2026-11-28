use proptest::prelude::*;

use super::tests::reference;
use super::*;

proptest! {
    /// Any finite `f64` bit pattern inside the precondition rounds as
    /// `f64::round` does: the bit-pattern strategy reaches every exponent,
    /// where a range strategy would crowd the large magnitudes.
    #[test]
    fn matches_f64_round_on_any_bit_pattern_inside_the_limit(bits in any::<u64>()) {
        let x = f64::from_bits(bits);
        prop_assume!(x.abs() < EXACT_LIMIT);
        prop_assert_eq!(nearest(x), reference(x));
    }

    /// The fractions rounding is decided by, on integers of every size up
    /// to the limit: the bit-pattern sweep rarely lands on a tie.
    #[test]
    fn matches_f64_round_near_a_tie_at_any_magnitude(
        whole in -(1_i64 << 51)..(1_i64 << 51),
        quarter in 0_u8..4,
    ) {
        #[allow(clippy::cast_precision_loss, reason = "|whole| < 2^51, exact in f64")]
        let x = whole as f64 + f64::from(quarter) / 4.0;
        for probe in [x.next_down(), x, x.next_up()] {
            prop_assert_eq!(nearest(probe), reference(probe));
        }
    }
}
