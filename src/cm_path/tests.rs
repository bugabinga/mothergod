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
        decode(&[], u32::MAX, MAX_OUTPUT_LEN),
        Err(Error::TooLarge {
            len: u32::MAX,
            max: MAX_OUTPUT_LEN
        })
    );
}

/// A caller's bound past [`MAX_OUTPUT_LEN`] buys it nothing: issue
/// #1808's `u32::MAX` would otherwise reach 4 GiB of output and work, and
/// [`MAX_DECODED_LEN`] about 1,400 s of it. The ceiling itself is admitted,
/// so the empty stream falls to the floor instead.
#[test]
fn decode_caps_a_callers_bound_at_the_output_ceiling() {
    for bound in [MAX_DECODED_LEN, u32::MAX] {
        assert_eq!(
            decode(&[], MAX_OUTPUT_LEN + 1, bound),
            Err(Error::TooLarge {
                len: MAX_OUTPUT_LEN + 1,
                max: MAX_OUTPUT_LEN
            }),
            "bound {bound}"
        );
        assert_eq!(
            decode(&[], MAX_OUTPUT_LEN, bound),
            Err(Error::Truncated),
            "bound {bound}"
        );
    }
}

/// Issue #1808's repro, an empty stream declaring 4 MiB, cost 17.5 s of
/// walks; the floor refuses it, and every stream short of one byte per
/// [`MAX_EXPANSION`] output bytes, before any work.
#[test]
fn decode_refuses_a_stream_too_short_for_its_length() {
    assert_eq!(decode(&[], 4 << 20, MAX_DECODED_LEN), Err(Error::Truncated));
    assert_eq!(decode(&[], 1, MAX_DECODED_LEN), Err(Error::Truncated));
    assert_eq!(
        decode(&[0], MAX_EXPANSION + 1, MAX_DECODED_LEN),
        Err(Error::Truncated)
    );
    let at_the_floor = decode(&[0], MAX_EXPANSION, MAX_DECODED_LEN).expect("one byte holds it");
    assert_eq!(u32::try_from(at_the_floor.len()), Ok(MAX_EXPANSION));
    assert_eq!(decode(&[], 0, MAX_DECODED_LEN), Ok(Vec::new()));
}

/// The floor's worst case, independent of how well the model learns:
/// every node of every byte coded at the clamp's certainty, the cheapest
/// [`encode`] can ever code one. The stream still holds more than one
/// byte per [`MAX_EXPANSION`] output bytes, at the 1157-byte crossover
/// [`MAX_EXPANSION`]'s doc derives and far past it.
#[test]
fn the_cheapest_possible_stream_stays_above_the_floor() {
    for len in [0u32, 1, 1156, 1157, 1158, 4097, 1 << 20] {
        let mut encoder = Encoder::new();
        for node in 0..8 * len {
            let bit = node % 3 == 0;
            encoder.encode_bit(bit, clamp(f64::from(u8::from(bit))));
        }
        let stream = encoder.finish();
        assert!(
            u64::from(len) <= u64::from(MAX_EXPANSION) * stream.len() as u64,
            "{len} bytes in a {}-byte stream",
            stream.len()
        );
    }
}

/// Hard rule 2: any stream decodes to exactly the length asked for, a
/// truncated or flipped one to wrong bytes, never a panic.
#[test]
fn damaged_streams_decode_to_the_asked_length_without_panicking() {
    let data = b"abcabcabd".repeat(200);
    let stream = encode(&data);
    let len = u32::try_from(data.len()).expect("a test input is far below 4 GiB");

    for cut in [1, stream.len() / 2, stream.len() - 1] {
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
    for (length, bucket) in [
        (15, 15),
        (16, 16),
        (31, 16),
        (32, 17),
        (64, 18),
        (127, 18),
        (128, 19),
        (255, 19),
        (256, 20),
    ] {
        assert_eq!(length_bucket(length), bucket, "length {length}");
    }
}

#[test]
fn a_check_is_the_hashs_top_sixteen_bits() {
    assert_eq!(check_of(0xabcd_0000_0000_1234), 0xabcd);
    assert_eq!(check_of(0x0000_ffff_ffff_ffff), 0);
}

/// A followed match grows a byte per hit, past the verify limit that caps
/// the lookup that found it: the model follows its match, it does not
/// re-find it.
#[test]
fn a_followed_match_grows_past_the_verify_limit() {
    let block = random_bytes(13, 2 * MATCH_VERIFY_LIMIT);
    let data = block.repeat(2);
    let mut model = MatchModel::try_new().unwrap();
    let mut longest = 0;
    for i in 0..data.len() {
        if let Some((byte, length)) = model.predict(&data[..i]) {
            assert_eq!(byte, data[i], "a verbatim repeat predicts every byte");
            longest = longest.max(length);
        }
    }
    assert_eq!(longest, data.len() - 1 - block.len());
}

/// The coded stream of one mixed input, pinned: text, noise, a run and a
/// repeat longer than the verify limit, so every input of the mixer
/// speaks, then more noise than the order tables hold, so contexts collide
/// and evict and the hash decides which. The journal's numbers for this path (S2-A133 to S2-A138) were
/// read through this model, and the wiring slice ships the model that
/// produced them; a change here is a new candidate, re-read on train and
/// sealed, and this pin moves with its journal entry. Also a determinism
/// check: every lane that runs the suite codes the same bytes.
#[test]
fn the_coded_stream_of_a_mixed_input_is_pinned() {
    let text = b"the quick brown fox jumps over the lazy dog; pack my box with five \
        dozen liquor jugs. "
        .repeat(40);
    let noise = random_bytes(17, 2 * MATCH_VERIFY_LIMIT);
    let data = [
        text.as_slice(),
        &noise,
        &[b'='; 300],
        &noise,
        &text[..1000],
        &random_bytes(19, 1 << 16),
    ]
    .concat();

    let stream = encode(&data);

    assert_eq!(
        (stream.len(), crate::crc32::crc32(&stream)),
        (66_491, 2_313_715_878),
        "the model changed: re-read it as a candidate"
    );
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

/// Claimed from the set's last index, so a set that started anywhere but
/// its aligned first bucket would hold a different least-seen root.
#[test]
fn a_newcomer_evicts_the_least_seen_root_and_the_first_on_a_tie() {
    let mut table = second_set_owned([3, 1, 2, 1]);
    let before = table.clone();

    let taken = claim(&mut table, ORDER_WAYS + ORDER_WAYS - 1, 9);

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

/// Adaptation alone stalls short of the guard, the rounded step vanishing
/// first, so the guard is reached from a slot built one unit below one at
/// count zero, whose step toward one is a third of a unit and rounds up.
#[test]
fn a_step_that_rounds_to_one_holds_one_unit_below() {
    let unit = 1.0 / f64::from(1u32 << PROBABILITY_BITS);
    let mut slot = Slot(((1 << PROBABILITY_BITS) - 1) << COUNT_BITS);
    assert_eq!((slot.p(), slot.count()), (1.0 - unit, 0));

    slot.update(true);

    assert_eq!((slot.p(), slot.count()), (1.0 - unit, 1));
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

/// A little-endian `u32` counter stepping by a constant: `pick` shortlists
/// `Delta(4)`, which turns it into one repeated row, so a filter's trial is
/// the shortest and [`encode_payload`] must find it among the others.
fn counter_rows() -> Vec<u8> {
    (0u32..1500)
        .flat_map(|i| i.wrapping_mul(40_503).to_le_bytes())
        .collect()
}

fn payload_text() -> Vec<u8> {
    b"a frame declares its extent; a model learns its context. ".repeat(30)
}

#[test]
fn a_payload_round_trips_and_declares_its_extent() {
    let base64 = filters::base64_unwrap::try_decode(&random_bytes(29, 900))
        .expect("a small buffer allocates");
    for data in [
        Vec::new(),
        b"x".to_vec(),
        payload_text(),
        counter_rows(),
        random_bytes(23, 3000),
        base64,
    ] {
        let payload = encode_payload(&data, NonZeroUsize::MIN);
        assert_eq!(
            decode_payload(&payload, MAX_DECODED_LEN).as_deref(),
            Ok(data.as_slice())
        );
        assert_eq!(
            declared_extent(&payload[..EXTENT_PREFIX_LEN]),
            Ok(Extent {
                payload_len: payload.len(),
                output_len: data.len() as u64,
            }),
            "the prefix alone names the payload's span and its output"
        );
    }
}

#[test]
fn the_payload_keeps_the_shortest_trial_whatever_the_workers() {
    let data = counter_rows();
    let payload = encode_payload(&data, NonZeroUsize::MIN);
    let shortest = filters::select::pick(&data)
        .into_iter()
        .map(|candidate| encode(&codec::apply_filter(candidate, &data)).len())
        .min()
        .expect("pick shortlists the identity");
    assert_eq!(payload.len(), EXTENT_PREFIX_LEN + shortest);
    assert_ne!(
        payload[..SELECTOR_LEN],
        Candidate::Identity.to_header_bytes(),
        "a filter wins on counter rows"
    );
    for workers in [2, 3, 8] {
        let workers = NonZeroUsize::new(workers).expect("nonzero");
        assert_eq!(encode_payload(&data, workers), payload, "{workers} workers");
    }
}

/// Every cut is short of the header or of the stream it declares; the
/// prefix's own fields come first, so no cut reaches the model.
#[test]
fn a_cut_payload_is_truncated() {
    let payload = encode_payload(&payload_text(), NonZeroUsize::MIN);
    for cut in [
        0,
        1,
        SELECTOR_LEN,
        EXTENT_PREFIX_LEN - 1,
        EXTENT_PREFIX_LEN,
        payload.len() - 1,
    ] {
        assert_eq!(
            decode_payload(&payload[..cut], MAX_DECODED_LEN),
            Err(Error::Truncated),
            "cut at {cut}"
        );
    }
}

#[test]
fn a_byte_after_the_stream_is_corrupt() {
    let mut payload = encode_payload(&payload_text(), NonZeroUsize::MIN);
    payload.push(0);
    assert_eq!(
        decode_payload(&payload, MAX_DECODED_LEN),
        Err(Error::Corrupt)
    );
}

/// The path declares no reach, so a reach code in the kind byte's top bits
/// names no filter, as an unknown kind does: refused from the prefix alone,
/// even one cut short of its lengths.
#[test]
fn a_selector_naming_no_filter_is_corrupt() {
    let payload = encode_payload(&payload_text(), NonZeroUsize::MIN);
    for kind in [0x40, 0x80, 0xc0, 7] {
        let mut bad = payload.clone();
        bad[0] = kind;
        bad[1] = 0;
        assert_eq!(
            decode_payload(&bad, MAX_DECODED_LEN),
            Err(Error::Corrupt),
            "{kind:#x}"
        );
        assert_eq!(
            declared_extent(&bad[..SELECTOR_LEN]),
            Err(Error::Corrupt),
            "{kind:#x}"
        );
    }
}

#[test]
fn an_output_past_the_bound_is_too_large_before_any_work() {
    let data = payload_text();
    let payload = encode_payload(&data, NonZeroUsize::MIN);
    let len = u32::try_from(data.len()).expect("a test input is far below 4 GiB");
    assert_eq!(
        decode_payload(&payload, len - 1),
        Err(Error::TooLarge { len, max: len - 1 })
    );
    let mut bomb = payload[..EXTENT_PREFIX_LEN].to_vec();
    bomb[SELECTOR_LEN..SELECTOR_LEN + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    bomb[SELECTOR_LEN + 4..].copy_from_slice(&0u64.to_le_bytes());
    assert_eq!(
        decode_payload(&bomb, u32::MAX),
        Err(Error::TooLarge {
            len: u32::MAX,
            max: MAX_OUTPUT_LEN
        }),
        "a caller's bound past the ceiling buys nothing"
    );
}

/// The frame's output, not only the filtered bytes [`decode`] sees, meets
/// [`MAX_OUTPUT_LEN`]: one byte past it is refused from the header, and the
/// ceiling itself passes to the stream's floor.
#[test]
fn a_frame_output_past_the_ceiling_is_too_large() {
    let header = |declared_len: u32| {
        let mut payload = Candidate::Identity.to_header_bytes().to_vec();
        payload.extend_from_slice(&declared_len.to_le_bytes());
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload
    };
    assert_eq!(
        decode_payload(&header(MAX_OUTPUT_LEN + 1), MAX_DECODED_LEN),
        Err(Error::TooLarge {
            len: MAX_OUTPUT_LEN + 1,
            max: MAX_OUTPUT_LEN
        })
    );
    assert_eq!(
        decode_payload(&header(MAX_OUTPUT_LEN), MAX_DECODED_LEN),
        Err(Error::Truncated)
    );
}
