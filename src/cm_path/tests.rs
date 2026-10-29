use super::*;
use crate::codec::MAX_DECODED_LEN;
use crate::test_support::Xorshift32;

/// Every probability `model` hands `code_bit` while walking `data`, the
/// driver's order: each byte after its prefix under the literal context.
fn probabilities(model: &mut CmPath, data: &[u8]) -> Vec<f64> {
    let mut seen = Vec::new();
    let mut context = Context::default();
    for (position, &byte) in data.iter().enumerate() {
        let symbol = usize::from(byte);
        model.walk(context, &data[..position], |mid, p| {
            seen.push(p);
            symbol >= mid
        });
        context = context.after_literal(byte);
    }
    seen
}

fn random_bytes(seed: u32, len: usize) -> Vec<u8> {
    Xorshift32::new(seed)
        .take(len)
        .map(|x| x.to_le_bytes()[0])
        .collect()
}

fn round_trips(data: &[u8]) {
    let stream = encode(data);
    let len = u32::try_from(data.len()).expect("a test input is far below 4 GiB");
    assert_eq!(decode(&stream, len, MAX_DECODED_LEN).as_deref(), Ok(data));
}

#[test]
fn a_fresh_model_codes_the_first_byte_at_the_wire_models_probabilities() {
    let byte = b'q';
    let mut wire = Literal::new();
    let mut wire_mixer = SurpriseLogisticMix::new();
    let mut wire_p = Vec::new();
    wire.walk_logit_sse(Context::default(), &mut wire_mixer, |mid, p| {
        wire_p.push(clamp(p));
        usize::from(byte) >= mid
    });

    let p = probabilities(&mut CmPath::new(), &[byte]);

    assert_eq!(p.len(), wire_p.len());
    for (p, wire_p) in p.iter().zip(&wire_p) {
        assert!(
            (p - wire_p).abs() < 1e-12,
            "weight 1 on the wire model, 0 elsewhere: {p} vs {wire_p}"
        );
    }
}

#[test]
fn every_probability_is_inside_the_floor_and_a_rerun_walks_identically() {
    let data: Vec<u8> = (0u32..3000)
        .map(|i| u8::try_from(i * 7 % 251).expect("below 251"))
        .collect();
    let first = probabilities(&mut CmPath::new(), &data);
    let second = probabilities(&mut CmPath::new(), &data);
    assert!(
        first
            .iter()
            .all(|p| (PROBABILITY_FLOOR..=1.0 - PROBABILITY_FLOOR).contains(p))
    );
    assert_eq!(first, second, "no state leaks between fresh models");
}

/// The match model is what a whole-stream path has that the wire model
/// alone lacks: on a long verbatim repeat of random bytes, which no short
/// context predicts, the second copy must code far smaller than the
/// first, measured as real coded bytes.
#[test]
fn a_verbatim_repeat_of_random_bytes_codes_a_fraction_of_its_first_copy() {
    let half = random_bytes(0x2545_f491, 4000);
    let once = encode(&half).len();
    let twice = encode(&[half.as_slice(), half.as_slice()].concat()).len();
    assert!(twice - once < once / 8, "first {once}, both {twice}");
}

#[test]
fn round_trips_nothing_one_byte_and_every_byte_value() {
    round_trips(&[]);
    round_trips(&[0]);
    round_trips(&[255]);
    round_trips(&(0..=255).collect::<Vec<u8>>());
}

#[test]
fn round_trips_text_repeats_runs_and_noise() {
    let text = b"the quick brown fox jumps over the lazy dog; pack my box with five \
        dozen liquor jugs. "
        .repeat(60);
    round_trips(&text);
    round_trips(&vec![b'!'; 20_000]);
    round_trips(&random_bytes(7, 20_000));
    let half = random_bytes(11, 3000);
    round_trips(&[half.as_slice(), &text, half.as_slice()].concat());
}

/// Matches longer than [`MATCH_VERIFY_LIMIT`] and past the last
/// [`length_bucket`]: the match model grows them a byte at a time on both
/// sides.
#[test]
fn round_trips_a_repeat_longer_than_the_verify_limit() {
    let block = random_bytes(3, 3 * MATCH_VERIFY_LIMIT);
    round_trips(&block.repeat(4));
}

#[test]
fn decode_rejects_a_length_over_its_bound_before_any_work() {
    assert_eq!(
        decode(&[], 1001, 1000),
        Err(Error::TooLarge {
            len: 1001,
            max: 1000
        })
    );
    assert_eq!(
        decode(&[], u32::MAX, MAX_DECODED_LEN),
        Err(Error::TooLarge {
            len: u32::MAX,
            max: MAX_DECODED_LEN
        })
    );
}

/// Hard rule 2: any stream decodes to exactly the length asked for, a
/// truncated or flipped one to wrong bytes, never a panic.
#[test]
fn damaged_streams_decode_to_the_asked_length_without_panicking() {
    let data = b"abcabcabd".repeat(200);
    let stream = encode(&data);
    let len = u32::try_from(data.len()).expect("a test input is far below 4 GiB");

    for cut in [0, 1, stream.len() / 2, stream.len() - 1] {
        let decoded = decode(&stream[..cut], len, MAX_DECODED_LEN).expect("only the bound fails");
        assert_eq!(decoded.len(), data.len());
    }
    let mut flipped = stream.clone();
    flipped[stream.len() / 3] ^= 0x10;
    let decoded = decode(&flipped, len, MAX_DECODED_LEN).expect("only the bound fails");
    assert_eq!(decoded.len(), data.len());
    assert_ne!(decoded, data, "a flipped bit changes what decodes");

    let noise = random_bytes(5, 512);
    let decoded = decode(&noise, 4096, MAX_DECODED_LEN).expect("only the bound fails");
    assert_eq!(decoded.len(), 4096);
}

#[test]
fn the_match_model_predicts_only_from_a_verified_earlier_context() {
    let mut model = MatchModel::try_new().unwrap();
    let data = b"abcdefgh--abcdefgX";
    let predictions: Vec<_> = (0..data.len()).map(|i| model.predict(&data[..i])).collect();

    assert!(
        predictions[..16].iter().all(Option::is_none),
        "{predictions:?}"
    );
    assert_eq!(
        predictions[16],
        Some((b'g', 6)),
        "abcdef recurs, g follows it"
    );
    assert_eq!(
        predictions[17],
        Some((b'h', 7)),
        "the hit extends the match"
    );
    assert_eq!(
        model.predict(data),
        None,
        "X broke the match, nothing recurs"
    );
}

#[test]
fn length_buckets_are_monotone_and_in_range() {
    let buckets: Vec<usize> = (0..1000).map(length_bucket).collect();
    assert!(buckets.windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(buckets[0], 0);
    assert_eq!(*buckets.last().unwrap(), LENGTH_BUCKETS - 1);
}

/// An input at or past either end of `[0, 1]` would hand [`logistic::ln`]
/// a zero or infinite odds ratio; the clamp holds it to the floor's logit
/// instead.
#[test]
fn stretch_of_a_certain_input_is_the_floors_finite_logit() {
    let bound = logistic::stretch(1.0 - PROBABILITY_FLOOR);
    for (p, expected) in [(0.0, -bound), (-1.0, -bound), (1.0, bound), (2.0, bound)] {
        let x = stretch(p);
        assert!((x - expected).abs() < 1e-12, "stretch({p}) = {x}");
    }
    assert!(stretch(0.5).abs() < 1e-15);
}

/// 64 bytes at a 64-byte alignment are exactly one line, the layout
/// [`Bucket`]'s doc promises.
#[test]
fn a_bucket_fills_exactly_one_cache_line() {
    assert_eq!(std::mem::size_of::<Bucket>(), 64);
    assert_eq!(std::mem::align_of::<Bucket>(), 64);
}

/// A table of two sets whose buckets hold checks 1 to 4 in the second
/// set, each root seen `visits[way]` times.
fn second_set_owned(visits: [u16; ORDER_WAYS]) -> Vec<Bucket> {
    let mut table = vec![Bucket::FRESH; 2 * ORDER_WAYS];
    for (way, &n) in visits.iter().enumerate() {
        let bucket = &mut table[ORDER_WAYS + way];
        bucket.check = u16::try_from(way + 1).expect("a way fits u16");
        for _ in 0..n {
            bucket.slots[0].update(true);
        }
    }
    table
}

#[test]
fn an_owner_finds_its_bucket_from_any_index_in_its_set() {
    let mut table = second_set_owned([1, 1, 1, 1]);
    let before = table.clone();
    for index in ORDER_WAYS..2 * ORDER_WAYS {
        assert_eq!(claim(&mut table, index, 3), ORDER_WAYS + 2);
    }
    assert_eq!(table[ORDER_WAYS + 2].slots, before[ORDER_WAYS + 2].slots);
}

#[test]
fn a_newcomer_evicts_the_least_seen_root_and_the_first_on_a_tie() {
    let mut table = second_set_owned([3, 1, 2, 1]);
    let before = table.clone();

    let taken = claim(&mut table, ORDER_WAYS + 2, 9);

    assert_eq!(taken, ORDER_WAYS + 1, "count 1 twice, the first wins");
    assert_eq!(table[taken].check, 9);
    assert_eq!(table[taken].slots, Bucket::FRESH.slots, "evicted whole");
    for kept in [ORDER_WAYS, ORDER_WAYS + 2, ORDER_WAYS + 3] {
        assert_eq!(table[kept].check, before[kept].check);
        assert_eq!(table[kept].slots, before[kept].slots);
    }
    assert!(
        table[..ORDER_WAYS].iter().all(|b| b.check == 0),
        "the first set is untouched"
    );
}

#[test]
fn a_slot_adapts_as_adapt_does_within_one_unit() {
    let unit = 1.0 / f64::from(1u32 << PROBABILITY_BITS);
    let mut slot = Slot::FRESH;
    let (mut p, mut count) = (0.5, 0);
    assert_eq!((slot.p(), slot.count()), (p, count));
    for bit in [true, true, false, true, false, false, true] {
        slot.update(bit);
        (p, count) = adapt(p, count, bit);
        assert!((slot.p() - p).abs() <= unit, "{} vs {p}", slot.p());
        assert_eq!(slot.count(), count);
    }
}

/// A probability driven to certainty would round to one, which the field
/// cannot hold: it would carry into nothing and wrap. It holds one unit
/// below, and the count beside it is intact at the limit.
#[test]
fn a_slot_driven_to_certainty_stays_in_its_field() {
    for bit in [true, false] {
        let mut slot = Slot::FRESH;
        for _ in 0..4 * usize::from(COUNTER_LIMIT) {
            slot.update(bit);
        }
        assert_eq!(slot.count(), COUNTER_LIMIT);
        let p = slot.p();
        assert!((0.0..1.0).contains(&p), "{p}");
        assert!(if bit { p > 0.999 } else { p < 0.001 }, "{p}");
    }
}

#[test]
fn a_counter_learns_fast_when_fresh_and_its_count_stops_at_the_limit() {
    let mut counter = Counter::try_new(1).unwrap();
    counter.update(0, true);
    let after_one = counter.p(0);
    assert!((after_one - (0.5 + 0.5 / 1.5)).abs() < 1e-6, "{after_one}");
    for _ in 0..2 * usize::from(COUNTER_LIMIT) {
        counter.update(0, true);
    }
    assert_eq!(counter.count[0], COUNTER_LIMIT);
    assert!(counter.p(0) > 0.99);
}
