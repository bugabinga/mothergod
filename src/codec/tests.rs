use super::*;
use crate::test_support::as_codec_error;

/// [`decode_to_writer`], collected into a `Vec<u8>` (which implements
/// [`std::io::Write`]) instead of streamed to a real sink, so tests can
/// compare its output byte for byte against [`decode`]'s.
fn decode_streaming(payload: &[u8], max_len: u32) -> Result<Vec<u8>, crate::WriteError> {
    let mut out = Vec::new();
    decode_to_writer(payload, max_len, &mut out)?;
    Ok(out)
}

/// Asserts both [`decode`] and [`decode_streaming`] reproduce `expected`
/// from `encoded`, the pairing every roundtrip fixture in this module
/// needs since the two share no code path (`decode_to_writer` is a
/// separate loop, tested here rather than trusted to inherit `decode`'s
/// coverage). `context` names the fixture in the failure message.
fn assert_decode_matches(encoded: &[u8], expected: &[u8], context: &str) {
    assert_eq!(
        decode(encoded, MAX_DECODED_LEN).as_deref(),
        Ok(expected),
        "decode mismatch: {context}"
    );
    assert_eq!(
        decode_streaming(encoded, MAX_DECODED_LEN)
            .expect("decode_to_writer must succeed whenever decode does, same payload"),
        expected,
        "streaming roundtrip mismatch: {context}"
    );
}

fn roundtrip(data: &[u8]) {
    let encoded = encode(data);
    assert_decode_matches(&encoded, data, "roundtrip");
}

#[test]
fn roundtrip_empty() {
    roundtrip(b"");
}

#[test]
fn roundtrip_single_byte() {
    roundtrip(b"x");
}

#[test]
fn roundtrip_all_literals_no_repeats() {
    roundtrip(b"the quick brown fox jumps over a lazy dog");
}

#[test]
fn roundtrip_simple_repeat_shrinks() {
    let data = b"abcdefgh".repeat(50);
    let encoded = encode(&data);
    assert!(
        encoded.len() < data.len(),
        "a 50x repeat of an 8-byte pattern should compress: {} -> {}",
        data.len(),
        encoded.len()
    );
    assert_decode_matches(&encoded, &data, "roundtrip");
}

#[test]
fn roundtrip_long_run_of_one_repeated_byte() {
    // Deliberately not lz.rs's 200_000-byte equivalent: encode here
    // always goes through lz::parse_optimal (never parse_greedy), and
    // parse_optimal's rep-candidate pricing is a linear match_len scan
    // at every position (see lz.rs's dp_round docs) — cost grows with
    // the square of a same-byte run once it passes MAX_MATCH_LEN
    // (65535). This project's own encoder hung for minutes at 200_000
    // bytes during this PR's development (`research/JOURNAL.md`
    // S2-A17), which is exactly why the test stays well under that
    // threshold rather than proving the hang here too.
    roundtrip(&vec![b'z'; 4000]);
}

#[test]
fn roundtrip_cyclic_data() {
    let data: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
    roundtrip(&data);
}

#[test]
fn roundtrip_pseudo_random_bytes() {
    let data: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
        .take(5000)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();
    roundtrip(&data);
}

#[test]
fn roundtrip_binary_data_with_zero_bytes() {
    let data: Vec<u8> = (0..1000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    roundtrip(&data);
}

#[test]
fn roundtrip_founding_archive_source() {
    // Named corpus (CLAUDE.md hard rule 4): the founding session's
    // archived codec, real structured Rust source, 25,524 bytes — the
    // same file `literal.rs`'s ADR-0024 accuracy test measures
    // against.
    let data: &[u8] = include_bytes!("../../research/imports/session-1/mothergod.rs");
    roundtrip(data);
}

/// Four independent small-step random walks, one per column, laid out
/// row-major with a 4-byte stride: consecutive same-column bytes drift
/// by a small step (`filters::select::pick` ranks `Candidate::Delta(4)`
/// top for exactly this shape — mirrors its own
/// `pick_selects_delta_for_columnar_drift` test), but consecutive raw
/// bytes belong to different, unrelated walks.
fn columnar_drift_data() -> Vec<u8> {
    const STEPS: [u8; 5] = [0u8.wrapping_sub(2), 0u8.wrapping_sub(1), 0, 1, 2];
    let mut rng = crate::test_support::Xorshift32::new(0x1234_5678);
    let mut walk = [64u8, 96, 160, 200];
    let rows = 2000usize;
    let mut data = Vec::with_capacity(rows * walk.len());
    for _ in 0..rows {
        for col in &mut walk {
            let state = rng.next().expect("Xorshift32 never terminates");
            let step = STEPS[usize::try_from(state % 5).unwrap_or(0)];
            *col = col.wrapping_add(step);
            data.push(*col);
        }
    }
    data
}

#[test]
fn roundtrip_columnar_drift_data_selects_delta_and_streams_it() {
    // Proves encode() actually wires a trial-selected filter into the
    // frame, not just plumbs pick() through unused; pins the kind byte
    // to 1 (Delta) rather than just non-identity, since this fixture is
    // also decode_undoable_streaming's only regression coverage for
    // Candidate::Delta (`research/JOURNAL.md` S1-P7) — a future trial-
    // cost change that made encode() prefer a different candidate here
    // would silently drop that coverage without this pin.
    let data = columnar_drift_data();
    let encoded = encode(&data);
    assert_eq!(
        encoded[0], 1,
        "columnar drift data should select Delta, got kind byte {}",
        encoded[0]
    );
    assert_decode_matches(&encoded, &data, "decode_undoable_streaming's Delta path");
}

#[test]
fn decode_undoable_streaming_delta_path_covers_copy_streamed_too() {
    // columnar_drift_data's random walk gives decode_undoable_streaming's
    // Delta path literal-only coverage: no fixed-distance repeat in that
    // fixture is long enough for lz::parse_optimal to price a match over
    // literals. Crafted directly against Candidate::Delta (bypassing
    // encode()'s trial selection) so copy_streamed's own undo call gets
    // exercised: a filtered stream built from a short repeating pattern
    // guarantees parse_optimal finds match/rep tokens for the repeats.
    let stride = crate::test_support::nz(4);
    let filtered: Vec<u8> = [1u8, 2, 3, 4].iter().copied().cycle().take(200).collect();
    let raw = filters::delta::decode(&filtered, stride);
    let mut frame = Candidate::Delta(stride).to_header_bytes().to_vec();
    frame.extend(encode_tokens(&filtered, None));

    assert_decode_matches(&frame, &raw, "Delta path with match/rep copies");
}

/// Many `call rel32` instructions, 20 bytes apart (room for a full
/// 5-byte instruction with no overlap), all targeting the same absolute
/// address (0): as raw relative offsets each instance's operand differs
/// by position, but `Candidate::Bcj` rewrites every one of them to the
/// same absolute bytes, which is what gives `encode()`'s trial a real
/// win to find here (mirrors `filters::select::pick`'s own
/// `pick_shortlists_bcj_for_opcode_dense_data` density fixture, but
/// with real operands instead of zeros so the rewrite actually creates
/// the repetition rather than leaving it accidentally already there).
fn bcj_call_dense_data() -> Vec<u8> {
    let mut data = vec![0x90u8; 4000];
    for (idx, chunk) in data.chunks_mut(20).enumerate() {
        chunk[0] = 0xE8;
        let post_addr = u32::try_from(idx * 20 + filters::bcj::INSTRUCTION_LEN)
            .expect("idx * 20 + 5 fits u32 for this fixture's size (4000)");
        let rel = 0u32.wrapping_sub(post_addr);
        chunk[1..filters::bcj::INSTRUCTION_LEN].copy_from_slice(&rel.to_le_bytes());
    }
    data
}

#[test]
fn roundtrip_bcj_call_dense_data_selects_bcj_and_streams_it() {
    // Same shape as roundtrip_columnar_drift_data_selects_delta_and_
    // streams_it: proves encode() actually selects Candidate::Bcj for a
    // real fixture (kind byte 2), not just that filters::bcj round-trips
    // in isolation, and that decode_to_writer's Bcj path
    // (research/JOURNAL.md S1-P7) reproduces decode()'s output exactly.
    let data = bcj_call_dense_data();
    let encoded = encode(&data);
    assert_eq!(
        encoded[0], 2,
        "call-dense data should select Bcj, got kind byte {}",
        encoded[0]
    );
    assert_decode_matches(&encoded, &data, "decode_undoable_streaming's Bcj path");
}

/// Fixed-width records whose columns each cycle through their own
/// period (`research/JOURNAL.md` S1-P5's target shape, same
/// construction as `tests/golden/v10-tabular-columns`), with a fraction
/// of bytes jittered off the clean pattern so the literal model, not
/// just `lz::parse_optimal`'s LZ matches, carries real weight — the
/// data this ADR-0046 slice's own real-bitstream measurement used.
fn tabular_columns_data(columns: usize, rows: usize, seed: u32) -> Vec<u8> {
    let periods: Vec<u32> = (0..columns)
        .map(|c| 3 + ((u32::try_from(c).unwrap() * 7 + seed) % 6))
        .collect();
    let bases: Vec<u8> = (0..columns)
        .map(|c| {
            u8::try_from(
                (u32::try_from(c).unwrap().wrapping_mul(0x1e)).wrapping_add(seed.wrapping_mul(3))
                    % 256,
            )
            .unwrap()
        })
        .collect();
    let mut rng =
        crate::test_support::Xorshift32::new(0x9e37_79b9 ^ seed.wrapping_mul(0x8551_4d97) | 1);
    let mut data = vec![0u8; columns * rows];
    for row in 0..rows {
        for (col, &period) in periods.iter().enumerate() {
            let phase = u32::try_from(row).unwrap() % period;
            let mut v = bases[col].wrapping_add(u8::try_from(phase).unwrap());
            let r = rng.next().expect("Xorshift32 never terminates");
            if r % 100 < 20 {
                let jitter = u8::try_from((r >> 8) % 40).unwrap();
                v = v.wrapping_add(jitter).wrapping_sub(20);
            }
            data[row * columns + col] = v;
        }
    }
    data
}

#[test]
fn roundtrip_tabular_columns_data_selects_transpose_and_streams_it() {
    // Same shape as roundtrip_columnar_drift_data_selects_delta_and_
    // streams_it and roundtrip_bcj_call_dense_data_selects_bcj_and_
    // streams_it: proves encode() actually selects Candidate::Transpose
    // for real data (kind byte 3), and that both decode() and
    // decode_streaming (which falls back to decode()'s whole-buffer
    // path for Transpose, JOURNAL S2-D4) reproduce the original bytes —
    // this is the real-wiring column-expert path (ADR-0046), not just
    // filters::transpose round-tripping in isolation.
    let data = tabular_columns_data(8, 2000, 1);
    let encoded = encode(&data);
    assert_eq!(
        encoded[0], 3,
        "tabular column data should select Transpose, got kind byte {}",
        encoded[0]
    );
    assert_decode_matches(&encoded, &data, "Transpose's whole-buffer fallback path");
}

#[test]
fn length_streams_through_both_split_models_roundtrip() {
    // A hand-built token stream over all-zero data (so any distance validly
    // replays, no real repeat structure needed): 4 literals, then 30
    // Token::Match tokens of length 50 at distance 1 (training length_match
    // on 50 alone), then 30 Token::Rep tokens of length 5 reusing the cached
    // distance (training length_rep on 5 alone). Pins that decode selects
    // the same model per kind as the encoder: a decode that read a Rep's
    // length through length_match would meet 30 observations of 50 and
    // desync the rest of the stream.
    let distance = NonZeroU32::new(1).expect("1 is not zero");
    let mut tokens = vec![
        Token::Literal(0),
        Token::Literal(0),
        Token::Literal(0),
        Token::Literal(0),
    ];
    tokens.extend(std::iter::repeat_n(Token::Match { len: 50, distance }, 30));
    tokens.extend(std::iter::repeat_n(
        Token::Rep {
            len: 5,
            slot: RepSlot::First,
        },
        30,
    ));
    let data = vec![0u8; 4 + 30 * 50 + 30 * 5];

    let mut frame = Candidate::Identity.to_header_bytes().to_vec();
    frame.extend(encode_tokens_with(&data, None, &tokens));

    assert_decode_matches(&frame, &data, "match and rep lengths, split models");
}

#[test]
fn length_match_and_rep_models_adapt_independently() {
    // Guards against a future refactor collapsing Models::length_match/
    // length_rep back onto one shared Model (which would still round-trip
    // correctly, since encode and decode would agree either way, so
    // length_streams_through_both_split_models_roundtrip can't catch it):
    // trains length_match on value 10 many times through
    // the real EncodeSink path, then checks that a fresh models.length_rep
    // (never trained) still prices 10 at its untrained, higher cost —
    // proving the two fields are genuinely independent state, not aliases.
    let mut models = Models::new();
    let mut ac = Encoder::new();
    for _ in 0..50 {
        EncodeSink {
            ac: &mut ac,
            column: None,
        }
        .length(&mut models, FlagKind::Match, 10);
    }
    let trained_match_cost = ideal_cost_bucketed(&mut models.length_match, 10);
    let untrained_rep_cost = ideal_cost_bucketed(&mut models.length_rep, 10);
    assert!(
        trained_match_cost < untrained_rep_cost,
        "trained length_match cost ({trained_match_cost}) should be cheaper than \
         untrained length_rep cost ({untrained_rep_cost}) for the same value"
    );
}

#[test]
fn offsets_through_length_keyed_models_roundtrip() {
    // A hand-built token stream over all-zero data (so any distance validly
    // replays, no real repeat structure needed): 4 literals, then 30
    // Token::Match tokens of length MIN_MATCH_LEN (offset_len_state 0) at
    // distance 1 (training offset_len[0] on distance 1 alone), then 30
    // Token::Match tokens of a far longer length (offset_len_state
    // OFFSET_LEN_STATES - 1, saturated) at distance 2 (training
    // offset_len[3] on distance 2 alone, starting fresh). Pins that decode
    // keys the offset model on the already-decoded length exactly as the
    // encoder does: a decode that shared one model across lengths would
    // meet 30 observations of distance 1 at the first long match and
    // desync the rest of the stream.
    let short_len = u32::try_from(lz::MIN_MATCH_LEN).expect("MIN_MATCH_LEN fits u32");
    let long_len = short_len + u32::try_from(OFFSET_LEN_STATES).expect("tiny constant fits u32");
    let distance1 = NonZeroU32::new(1).expect("1 is not zero");
    let distance2 = NonZeroU32::new(2).expect("2 is not zero");
    let mut tokens = vec![
        Token::Literal(0),
        Token::Literal(0),
        Token::Literal(0),
        Token::Literal(0),
    ];
    tokens.extend(std::iter::repeat_n(
        Token::Match {
            len: short_len,
            distance: distance1,
        },
        30,
    ));
    tokens.extend(std::iter::repeat_n(
        Token::Match {
            len: long_len,
            distance: distance2,
        },
        30,
    ));
    let data = vec![0u8; 4 + 30 * short_len as usize + 30 * long_len as usize];

    let mut frame = Candidate::Identity.to_header_bytes().to_vec();
    frame.extend(encode_tokens_with(&data, None, &tokens));

    assert_decode_matches(&frame, &data, "offsets keyed on match length");
}

#[test]
fn offset_models_adapt_independently_by_length_state() {
    // Guards against a future refactor collapsing Models::offset_len's
    // four entries back onto fewer states (which would still round-trip
    // correctly, since encode and decode would agree either way, so
    // offsets_through_length_keyed_models_roundtrip can't catch it): trains offset_len[0] (a length-MIN_MATCH_LEN match)
    // on distance 10 many times through the real EncodeSink path, then
    // checks that a fresh offset_len[OFFSET_LEN_STATES - 1] (never
    // trained, a far longer match's own state) still prices 10 at its
    // untrained, higher cost — proving the four entries are genuinely
    // independent state, not aliases.
    let mut models = Models::new();
    let mut ac = Encoder::new();
    let short_len = u32::try_from(lz::MIN_MATCH_LEN).expect("MIN_MATCH_LEN fits u32");
    for _ in 0..50 {
        EncodeSink {
            ac: &mut ac,
            column: None,
        }
        .offset(&mut models, short_len, 10);
    }
    let trained_cost = ideal_cost_bucketed(&mut models.offset_len[0], 10);
    let untrained_cost = ideal_cost_bucketed(&mut models.offset_len[OFFSET_LEN_STATES - 1], 10);
    assert!(
        trained_cost < untrained_cost,
        "trained offset_len[0] cost ({trained_cost}) should be cheaper than untrained \
         offset_len[OFFSET_LEN_STATES - 1] cost ({untrained_cost}) for the same value"
    );
}

#[test]
fn offset_len_state_saturates_on_a_long_match() {
    // Real callers only ever pass a Token::Match length, which can run
    // into the thousands on highly repetitive input: offset_len_state must
    // saturate into the last state instead of panicking or indexing past
    // Models::offset_len's bounds.
    assert_eq!(
        offset_len_state(u32::try_from(lz::MIN_MATCH_LEN).unwrap() + 1_000),
        OFFSET_LEN_STATES - 1
    );
}

#[test]
fn decode_undoable_streaming_bcj_path_covers_copy_streamed_too() {
    // A run of identical 5-byte instructions in the *filtered* stream
    // gives lz::parse_optimal real match/rep tokens to find, exercising
    // copy_streamed's own undo call — the raw (pre-filter) bytes this
    // decodes to are not themselves repetitive, since each instance's
    // absolute-to-relative rewrite depends on its own position, proving
    // filters::bcj::Undo recomputes that per position rather than
    // replaying whatever the first instance resolved to.
    let unit = [0xE8u8, 0x00, 0x00, 0x00, 0x00];
    let filtered: Vec<u8> = unit.iter().copied().cycle().take(200).collect();
    let raw = filters::bcj::decode(&filtered);
    let mut frame = Candidate::Bcj.to_header_bytes().to_vec();
    frame.extend(encode_tokens(&filtered, None));

    assert_decode_matches(&frame, &raw, "Bcj path with match/rep copies");
}

#[test]
fn decode_undoable_streaming_bcj_path_flushes_a_trailing_opcode_on_finish() {
    // The filtered stream ends on an 0xE8 opcode byte followed by only
    // 3 more bytes: one short of INSTRUCTION_LEN (5), so
    // filters::bcj::Undo::apply never resolves it and StreamUndo::Bcj's
    // `pending` is still non-empty when the token loop ends. Those 4
    // bytes only reach `writer` through StreamUndo::finish's flush
    // (codec.rs's own call, not filters::bcj::Undo::finish, which
    // undo_tests::matches_decode_too_short_for_any_instruction already
    // covers one layer below this one): turning that call into a no-op
    // silently drops them, a losslessness violation invisible to every
    // other Bcj fixture here because they all end on an instruction
    // boundary.
    let unit = [0xE8u8, 0x00, 0x00, 0x00, 0x00];
    let mut filtered: Vec<u8> = unit.iter().copied().cycle().take(200).collect();
    filtered.extend_from_slice(&[0xE8, 0x01, 0x02, 0x03]);
    let raw = filters::bcj::decode(&filtered);
    let mut frame = Candidate::Bcj.to_header_bytes().to_vec();
    frame.extend(encode_tokens(&filtered, None));

    assert_decode_matches(
        &frame,
        &raw,
        "StreamUndo::finish must flush a trailing buffered Bcj opcode",
    );
}

#[test]
fn truncated_header_is_rejected() {
    assert_eq!(decode(&[0u8; 4], MAX_DECODED_LEN), Err(Error::Truncated));
    assert_eq!(decode(&[], MAX_DECODED_LEN), Err(Error::Truncated));
}

#[test]
fn unknown_filter_selector_is_rejected_not_panicking() {
    // Kind byte 5 names no Candidate::from_header_bytes ever produces
    // (0=Identity, 1=Delta, 2=Bcj, 3=Transpose): an adversarial or
    // future-format payload, never a bug in this decoder.
    let mut payload = vec![5u8, 0u8];
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(decode(&payload, MAX_DECODED_LEN), Err(Error::Corrupt));
}

#[test]
fn declared_length_lie_is_rejected_not_overallocated() {
    // A payload claiming a huge (u32::MAX) declared output length but
    // a token count of 0: decode must reject the mismatch, and must
    // do so in zero loop iterations rather than trying to honor the
    // claim by preallocating or looping toward it (rust-craft skill,
    // allocation-discipline). u32::MAX exceeds MAX_DECODED_LEN, so
    // this actually now hits the TooLarge fast-reject path below
    // before token_count is even consulted; kept as Corrupt's own
    // regression case too since it predates that check.
    let mut payload = Candidate::Identity.to_header_bytes().to_vec();
    payload.extend_from_slice(&u32::MAX.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        decode(&payload, MAX_DECODED_LEN),
        Err(Error::TooLarge {
            len: u32::MAX,
            max: MAX_DECODED_LEN
        })
    );
}

#[test]
fn declared_length_over_the_max_is_rejected_before_any_work() {
    // The amplification hazard a ratio check cannot rule out (see
    // MAX_DECODED_LEN's docs): declared_len and token_count agree with
    // each other, both far past MAX_DECODED_LEN, with an empty coded
    // stream. Previously legal-but-slow under ADR-0026's declared-
    // length-bounds-work argument (true, but declared_len itself was
    // unbounded); now rejected in zero loop iterations.
    let over = MAX_DECODED_LEN + 1;
    let mut payload = Candidate::Identity.to_header_bytes().to_vec();
    payload.extend_from_slice(&over.to_le_bytes());
    payload.extend_from_slice(&over.to_le_bytes());
    assert_eq!(
        decode(&payload, MAX_DECODED_LEN),
        Err(Error::TooLarge {
            len: over,
            max: MAX_DECODED_LEN
        })
    );
}

#[test]
fn decode_clamps_a_max_len_above_max_decoded_len() {
    // decode is `#[doc(hidden)]` but still `pub`, reachable directly by
    // anything depending on this crate, not just decompress_bounded
    // (which always pre-clamps before calling in). Without decode's own
    // clamp, a caller passing u32::MAX here would relax the ceiling
    // past MAX_DECODED_LEN, the only value this decoder's worst-case
    // decode time has been measured against (S2-A27).
    let over = MAX_DECODED_LEN + 1;
    let mut payload = Candidate::Identity.to_header_bytes().to_vec();
    payload.extend_from_slice(&over.to_le_bytes());
    payload.extend_from_slice(&over.to_le_bytes());
    assert_eq!(
        decode(&payload, u32::MAX),
        Err(Error::TooLarge {
            len: over,
            max: MAX_DECODED_LEN
        })
    );
}

/// A `Method::Lz` payload (filter selector, header, range-coded stream) of
/// one [`Token::Match`] of length 4 at `distance` and nothing before it,
/// coded through the real [`EncodeSink`] so the bit-level framing is what
/// [`decode`] reads: the decoder meets exactly the distance named, never
/// garbage from a mismatched model. `declared_len` is 4, one match's worth.
fn single_match_payload(distance: u32) -> Vec<u8> {
    let mut models = Models::new();
    let mut ac = Encoder::new();
    models.flag[0].encode(&mut ac, FlagKind::Match.index());
    let mut sink = EncodeSink {
        ac: &mut ac,
        column: None,
    };
    sink.length(&mut models, FlagKind::Match, 4);
    sink.offset(&mut models, 4, distance);

    let mut payload = Candidate::Identity.to_header_bytes().to_vec();
    payload.extend_from_slice(&4u32.to_le_bytes()); // declared_len
    payload.extend_from_slice(&1u32.to_le_bytes()); // token_count
    payload.extend(ac.finish());
    payload
}

#[test]
fn bad_match_distance_is_rejected_not_panicking() {
    // Hand-crafted: one token (flag=Match), coded through a fresh
    // Encoder so the bit-level framing is real, at a position where no
    // output exists yet — the distance necessarily reaches before the
    // start of decoded output.
    let payload = single_match_payload(1);

    assert_eq!(decode(&payload, MAX_DECODED_LEN), Err(Error::Corrupt));
}

#[test]
fn ensure_room_rejects_growth_past_declared_len() {
    // Direct, not routed through decode(): output only ever grows, so
    // by the time a full decode loop finishes, the final `output.len()
    // != declared_len` check alone would already reject any payload
    // whose mid-stream growth ensure_room should have caught earlier,
    // with the exact same Err(Corrupt) — a decode()-level test can
    // never tell the two checks apart. This is the only place that
    // pins ensure_room itself is the one doing the rejecting (hard
    // rule 2, its own doc comment: "checked before every write rather
    // than trusted from the header").
    assert_eq!(ensure_room(6, 5, 10), Err(Error::Corrupt));
    assert_eq!(ensure_room(0, 1, 0), Err(Error::Corrupt));
    assert_eq!(ensure_room(5, 5, 10), Ok(()));
    assert_eq!(ensure_room(0, 0, 0), Ok(()));
}

#[test]
fn match_distance_beyond_window_is_rejected() {
    // OFFSET_BUCKETS (21) lets decode_bucketed represent distances up
    // to 2 * lz::WINDOW - 1, wider than any real encoder ever emits
    // (its match finder never searches past lz::WINDOW): a distance
    // one past the window must be rejected on its own, before
    // ensure_room or copy_checked's own bounds checks even run.
    let over_window = u32::try_from(lz::WINDOW).expect("WINDOW fits u32") + 1;
    let payload = single_match_payload(over_window);

    assert_eq!(decode(&payload, MAX_DECODED_LEN), Err(Error::Corrupt));
}

#[test]
fn ensure_within_window_accepts_the_boundary_and_rejects_one_past() {
    // The decode-level test above only ever exercises a distance one
    // past the window against an empty output, where copy_checked's own
    // bounds check (distance > output.len()) independently produces the
    // identical Err(Corrupt): #390's mutation sweep found `>` -> `==`
    // and `>` -> `>=` both survive the full suite because of it. Unit
    // testing the extracted comparison directly, at exactly the
    // boundary in both directions, is the only way to tell `>` apart
    // from its neighbors without driving a WINDOW-sized decode.
    let window = u32::try_from(lz::WINDOW).expect("WINDOW fits u32");
    assert_eq!(
        ensure_within_window(NonZeroU32::new(window).expect("WINDOW is not zero")),
        Ok(()),
        "a distance exactly at the window edge is legal"
    );
    assert_eq!(
        ensure_within_window(NonZeroU32::new(window + 1).expect("WINDOW + 1 is not zero")),
        Err(Error::Corrupt),
        "one past the window edge must be rejected"
    );
}

#[test]
fn caller_supplied_max_len_below_max_decoded_len_is_honored() {
    // A frame legal under MAX_DECODED_LEN can still be rejected by a
    // caller's tighter max_len, proving the parameter is a real
    // additional bound, not a synonym for the constant.
    let data = b"the quick brown fox jumps over a lazy dog";
    let encoded = encode(data);
    let declared_len = u32::try_from(data.len()).unwrap();
    assert_eq!(
        decode(&encoded, declared_len - 1),
        Err(Error::TooLarge {
            len: declared_len,
            max: declared_len - 1
        })
    );
    assert_eq!(
        decode(&encoded, declared_len).as_deref(),
        Ok(data.as_slice())
    );
}

#[test]
fn ideal_cost_bits_is_zero_on_empty_input() {
    assert!(ideal_cost_bits(b"").abs() < 1e-9);
}

#[test]
fn ideal_cost_bits_tracks_real_encoded_length_within_one_percent() {
    // Named corpus (CLAUDE.md hard rule 4): the founding session's
    // archived codec, real structured Rust source, 25,524 bytes — the
    // same fixture roundtrip_founding_archive_source and literal.rs's
    // vendored-exp accuracy test use. encode_tokens's real Encoder
    // output is compared past its 8-byte declared-length/token-count
    // header (no ideal_cost_bits call ever prices that header): summed
    // ideal cost is an estimate, not the real coder's bit-exact output
    // (integer cumulative-frequency division rounds; the coder also
    // pays a handful of flush bits at the end), so this checks
    // closeness, not equality — the same tolerance shape as
    // model.rs's and literal.rs's own ideal-cost accuracy tests.
    let data: &[u8] = include_bytes!("../../research/imports/session-1/mothergod.rs");

    let ideal_bits = ideal_cost_bits(data);

    let real = encode_tokens(data, None);
    #[allow(
        clippy::cast_precision_loss,
        reason = "encoded length is far below f64's exact integer range (2^53)"
    )]
    let real_bits = ((real.len() - TOKEN_HEADER_LEN) * 8) as f64;

    let relative_diff = (ideal_bits - real_bits).abs() / real_bits;
    assert!(
        relative_diff <= 0.01,
        "ideal cost: {ideal_bits} bits vs real encoded length: {real_bits} bits, \
         {relative_diff:.4} relative difference exceeds the 1% budget"
    );
}

#[test]
fn ideal_cost_bits_is_lower_for_repetitive_than_random_data() {
    // A sanity check the accuracy test above can't give directly: the
    // ideal-cost pass must actually reflect the LZ/model pipeline's own
    // sense of compressibility, not just track real encoded length on
    // one fixture. A 50x repeat of an 8-byte pattern (long enough to
    // clear lz::OPTIMAL_MIN_LEN) must cost far fewer bits per byte than
    // pseudo-random bytes of the same length.
    let repetitive = b"abcdefgh".repeat(50);
    let random: Vec<u8> = crate::test_support::Xorshift32::new(0x1234_5678)
        .take(repetitive.len())
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();

    #[allow(
        clippy::cast_precision_loss,
        reason = "byte lengths here are tiny, far below f64's exact integer range (2^53)"
    )]
    let (repetitive_len, random_len) = (repetitive.len() as f64, random.len() as f64);
    let repetitive_bpb = ideal_cost_bits(&repetitive) / repetitive_len;
    let random_bpb = ideal_cost_bits(&random) / random_len;
    assert!(
        repetitive_bpb < random_bpb / 2.0,
        "repetitive data's ideal cost ({repetitive_bpb} bits/byte) should be far below \
         random data's ({random_bpb} bits/byte)"
    );
}

#[test]
fn fresh_residual_tree_costs_exactly_the_raw_residual_bits() {
    // A fresh binary Model prices either bit at exactly 1.0, so before any
    // update a tree costs what raw residual bits cost: the whole S2-A114
    // gain comes from adaptation, none from accounting.
    for value in [1u32, 2, 3, 7, 16, 31, 1_000, 65_535, (1 << 16) + 12_345] {
        let mut tree = ResidualTree::new(lz::LENGTH_BUCKETS);
        let raw = f64::from(bucket_bits(lz::bucket(value)));
        let cost = tree.ideal_cost_bits(value);
        assert!((cost - raw).abs() < 1e-12, "value {value}: {cost} vs {raw}");
    }
}

#[test]
fn residual_tree_learns_only_its_modeled_bits() {
    // 1_000 sits in bucket 9: four modeled residual bits, five raw. A
    // repeated value drives the modeled four toward free, never the raw
    // five, so the cost converges to just above 5 and never below it.
    let mut tree = ResidualTree::new(lz::LENGTH_BUCKETS);
    let mut last = f64::INFINITY;
    for _ in 0..50 {
        last = tree.ideal_cost_bits(1_000);
    }
    assert!(last > 5.0 && last < 5.5, "converged cost {last}");
}

#[test]
fn residual_tree_buckets_do_not_share_nodes() {
    // Training bucket 9 must leave bucket 10's walk at its fresh price.
    let mut tree = ResidualTree::new(lz::LENGTH_BUCKETS);
    for _ in 0..50 {
        let _ = tree.ideal_cost_bits(1_000);
    }
    let cost = tree.ideal_cost_bits(2_000);
    assert!((cost - 10.0).abs() < 1e-12, "bucket 10 cost {cost}");
}

/// Values spanning every length bucket: each bucket's smallest and largest
/// value and two between, repeated so the trees adapt mid-stream.
fn residual_tree_values() -> Vec<u32> {
    let mut values = Vec::new();
    for _ in 0..4 {
        for b in 0..lz::LENGTH_BUCKETS {
            let low = 1u32 << b;
            let high = (low << 1) - 1;
            values.extend([low, low + (high - low) / 3, high - (high - low) / 5, high]);
        }
    }
    values
}

#[test]
fn residual_tree_decode_inverts_encode_in_every_bucket() {
    let values = residual_tree_values();
    let mut tree = ResidualTree::new(lz::LENGTH_BUCKETS);
    let mut ac = Encoder::new();
    for &value in &values {
        tree.encode(&mut ac, value);
    }
    let bytes = ac.finish();

    let mut tree = ResidualTree::try_new(lz::LENGTH_BUCKETS).expect("small allocation");
    let mut ac = Decoder::new(&bytes);
    for &value in &values {
        // The caller decodes the bucket symbol first; here it is known.
        assert_eq!(tree.decode(&mut ac, lz::bucket(value)), value);
    }
}

#[test]
fn residual_tree_decode_leaves_the_encoders_node_state() {
    // Decoding any injective relabeling of the nodes (`base - node` for
    // `base + node`) still reproduces the values, because both trees start
    // fresh. Only the layout invariant tells them apart: after the same
    // stream, the decoder's nodes must sit where the encoder's did, so a
    // walk priced through either tree costs the same.
    let values = residual_tree_values();
    let mut encoder_tree = ResidualTree::new(lz::LENGTH_BUCKETS);
    let mut ac = Encoder::new();
    for &value in &values {
        encoder_tree.encode(&mut ac, value);
    }
    let bytes = ac.finish();

    let mut decoder_tree = ResidualTree::new(lz::LENGTH_BUCKETS);
    let mut ac = Decoder::new(&bytes);
    for &value in &values {
        assert_eq!(decoder_tree.decode(&mut ac, lz::bucket(value)), value);
    }
    for &value in &values {
        let encoded = encoder_tree.ideal_cost_bits(value);
        let decoded = decoder_tree.ideal_cost_bits(value);
        assert!(
            (encoded - decoded).abs() < 1e-12,
            "value {value}: encoder tree prices {encoded}, decoder tree {decoded}"
        );
    }
}

#[test]
fn residual_tree_price_matches_what_it_codes() {
    // CostSink must price what EncodeSink codes: the ideal cost of a
    // stream and its real coded size agree to the coder's few bytes of
    // flush and quantization slack.
    let values = residual_tree_values();
    let mut priced = ResidualTree::new(lz::LENGTH_BUCKETS);
    let ideal: f64 = values.iter().map(|&v| priced.ideal_cost_bits(v)).sum();
    let mut coded = ResidualTree::new(lz::LENGTH_BUCKETS);
    let mut ac = Encoder::new();
    for &value in &values {
        coded.encode(&mut ac, value);
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "a few hundred bytes of test output, far below f64's 2^53"
    )]
    let real = (ac.finish().len() * 8) as f64;
    assert!(
        (real - ideal).abs() < 64.0,
        "coded {real} bits vs priced {ideal} bits"
    );
}

#[test]
fn length_residual_trees_roundtrip_through_a_frame() {
    // 30 matches of length 50 (bucket 5, five residual bits, the top four
    // modeled) over all-zero data: pins that decode reads the residual bits
    // through the same trees the encoder wrote them with, where a decoder
    // reading them raw would desync.
    let distance = NonZeroU32::new(1).expect("1 is not zero");
    let mut tokens = vec![Token::Literal(0); 4];
    tokens.extend(std::iter::repeat_n(Token::Match { len: 50, distance }, 30));
    let data = vec![0u8; 4 + 30 * 50];

    let mut frame = Candidate::Identity.to_header_bytes().to_vec();
    frame.extend(encode_tokens_with(&data, None, &tokens));

    assert_decode_matches(&frame, &data, "residual bits through trees");
}

#[test]
fn length_residual_trees_adapt_independently_by_kind() {
    // Trains the match tree through the real EncodeSink path, then checks
    // the rep tree still prices the same value at its fresh, higher cost:
    // two genuinely separate trees, not aliases a round-trip cannot tell.
    let mut models = Models::new();
    let mut ac = Encoder::new();
    for _ in 0..50 {
        EncodeSink {
            ac: &mut ac,
            column: None,
        }
        .length(&mut models, FlagKind::Match, 50);
    }
    let trained = models.length_match_residual.ideal_cost_bits(50);
    let untrained = models.length_rep_residual.ideal_cost_bits(50);
    assert!(
        trained < untrained,
        "trained match tree ({trained}) should undercut the untrained rep tree ({untrained})"
    );
}

#[test]
fn ideal_cost_bits_ppm_expert_experiment_is_zero_on_empty_input() {
    let (baseline, with_ppm) = ideal_cost_bits_ppm_expert_experiment(b"");
    assert!(baseline.abs() < 1e-9);
    assert!(with_ppm.abs() < 1e-9);
}

#[test]
fn ideal_cost_bits_ppm_expert_experiment_stays_finite_and_positive() {
    // research/JOURNAL.md S1-P3: no accuracy claim here, just that the
    // paired walk runs to completion and both totals land somewhere
    // sane — the actual accept/reject verdict is a train/sealed
    // measurement recorded in the journal, not a unit test assertion.
    let data: &[u8] = include_bytes!("../../research/imports/session-1/mothergod.rs");
    let (baseline, with_ppm) = ideal_cost_bits_ppm_expert_experiment(data);
    assert!(
        baseline.is_finite() && baseline > 0.0,
        "baseline={baseline}"
    );
    assert!(
        with_ppm.is_finite() && with_ppm > 0.0,
        "with_ppm={with_ppm}"
    );
}

/// A fresh [`Models`]/[`PpmExpertState`] pair, for asserting
/// [`PairedTokenSink`]'s ppm-paired literal against a value computed
/// independently from the same starting state.
fn fresh_ppm_expert_sink() -> (Models, PpmExpertState) {
    (Models::new(), PpmExpertState::new())
}

#[test]
fn paired_token_sink_non_literal_events_add_the_same_cost_to_both_totals() {
    let mut expected_models = Models::new();
    let expected = expected_models.flag[1].ideal_cost_bits(FlagKind::Match.index())
        + ideal_cost_bucketed(&mut expected_models.length_match, 17)
        + ideal_cost_bucketed(&mut expected_models.offset_len[offset_len_state(17)], 123)
        + expected_models.slot.ideal_cost_bits(2);

    let mut models = Models::new();
    let mut sink = PairedTokenSink {
        cost: PairedCost::default(),
        pair_literal: |_: &mut Models, _: Context, _: u8| {
            unreachable!("this test never calls TokenSink::literal")
        },
    };
    sink.flag(&mut models, 1, FlagKind::Match);
    sink.length(&mut models, FlagKind::Match, 17);
    sink.offset(&mut models, 17, 123);
    sink.slot(&mut models, 2);

    assert!((sink.cost.baseline - expected).abs() < 1e-9);
    assert!((sink.cost.candidate - expected).abs() < 1e-9);
}

#[test]
fn paired_token_sink_ppm_expert_literal_adds_the_baseline_and_with_ppm_pair_separately() {
    // Enough repeats for the additive PPM bank's own history to pull
    // away from the six-expert mix's baseline, so a sink that swaps or
    // drops one half of the pair is distinguishable from a correct one.
    let bytes = b"aaaaaaaaaaaaaaaaaab";

    let (mut expected_models, mut expected_ppm_state) = fresh_ppm_expert_sink();
    let (mut models, mut ppm_state) = fresh_ppm_expert_sink();
    let mut sink = PairedTokenSink {
        cost: PairedCost::default(),
        pair_literal: |models: &mut Models, context: Context, byte: u8| {
            models
                .literal
                .ideal_cost_bits_ppm_expert_pair(context, byte, &mut ppm_state)
        },
    };

    let mut context = Context::default();
    let mut expected_baseline_total = 0.0;
    let mut expected_with_ppm_total = 0.0;
    for &byte in bytes {
        let (baseline, with_ppm) = expected_models.literal.ideal_cost_bits_ppm_expert_pair(
            context,
            byte,
            &mut expected_ppm_state,
        );
        expected_baseline_total += baseline;
        expected_with_ppm_total += with_ppm;
        sink.literal(&mut models, context, byte);
        context = context.after_literal(byte);
    }

    assert!(
        (expected_baseline_total - expected_with_ppm_total).abs() > 1e-6,
        "baseline_total={expected_baseline_total} with_ppm_total={expected_with_ppm_total}, \
         test cannot tell the pair's two halves apart"
    );
    assert!((sink.cost.baseline - expected_baseline_total).abs() < 1e-6);
    assert!((sink.cost.candidate - expected_with_ppm_total).abs() < 1e-6);
}

#[test]
fn ideal_cost_bits_with_window_drops_once_a_repeat_becomes_reachable() {
    // research/JOURNAL.md S1-P4: a repeat past the window is priced as
    // fresh literals; the same repeat within a larger window is priced
    // as a match, so the real adaptive models must report meaningfully
    // fewer bits once `window` grows to reach it. Same disjoint
    // template/filler byte ranges as lz::tests::
    // optimal_with_window_reaches_a_repeat_a_smaller_window_would_miss,
    // so the only structure in `data` is the planted repeat itself.
    let template: Vec<u8> = (0u8..80).collect();
    let filler: Vec<u8> = crate::test_support::Xorshift32::new(0x5EED)
        .take(420)
        .map(|s| 128 + u8::try_from(s % 128).unwrap())
        .collect();
    let mut data = template.clone();
    data.extend_from_slice(&filler);
    data.extend_from_slice(&template);

    let small_window = 300;
    let large_window = 700;
    let small_bits = ideal_cost_bits_with_window(&data, small_window);
    let large_bits = ideal_cost_bits_with_window(&data, large_window);
    assert!(
        large_bits < small_bits - f64::from(u16::try_from(template.len()).unwrap()),
        "a window reaching the planted repeat ({large_bits} bits) should cost at least a \
         template's worth of bits less than one that cannot ({small_bits} bits)"
    );
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "both sides run the identical token stream through the identical models \
              (lz::parse_optimal_adaptive_window's gate is unconditionally false for \
              data.len() <= lz::WINDOW), so exact equality, not tolerance, is the claim"
)]
fn ideal_cost_bits_adaptive_window_matches_wired_window_within_window() {
    // research/JOURNAL.md S1-P4's next slice after S2-A92:
    // lz::parse_optimal_adaptive_window's own gate is unconditionally
    // false for data.len() <= lz::WINDOW (lz::tests::
    // adaptive_window_matches_wired_window_within_window proves the
    // token streams are identical at that scale), so the models-wired
    // cost through this codec-layer entry point must match the wired
    // one exactly too, not just approximately.
    let data: Vec<u8> = (0..5000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    assert_eq!(
        ideal_cost_bits_adaptive_window(&data),
        ideal_cost_bits(&data)
    );
}

#[test]
fn compressed_len_with_window_matches_wired_encode_tokens_at_wired_window() {
    // Real-Encoder counterpart to the fixed-window ideal-cost test
    // above: lz::parse_optimal(data) is defined as
    // parse_optimal_with_window(data, WINDOW), so
    // compressed_len_with_window at that same window must match
    // encode_tokens's own wired length exactly, not just
    // approximately.
    let data: Vec<u8> = (0..5000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    assert_eq!(
        compressed_len_with_window(&data, lz::WINDOW),
        encode_tokens(&data, None).len()
    );
}

#[test]
fn compressed_len_adaptive_window_matches_wired_encode_tokens_within_window() {
    // Real-Encoder counterpart to
    // ideal_cost_bits_adaptive_window_matches_wired_window_within_window,
    // same reason: lz::parse_optimal_adaptive_window's gate is
    // unconditionally false for data.len() <= lz::WINDOW, so this
    // capability's real compressed length must match encode_tokens's
    // own wired length exactly, not just approximately, below that
    // scale. Not yet a decode()-level round-trip test: the whole point
    // of this capability (research/JOURNAL.md S1-P4) is measuring a
    // window past lz::WINDOW, and encode_tokens is not wired to use
    // one, so ensure_within_window still rejects a payload that
    // exercises it — the same caveat ideal_cost_bits_adaptive_window
    // has always carried.
    let data: Vec<u8> = (0..5000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    assert_eq!(
        compressed_len_adaptive_window(&data),
        encode_tokens(&data, None).len()
    );
}

#[test]
fn compressed_len_with_seed_and_search_window_matches_matched_window_call() {
    // compressed_len_with_window(data, w) is
    // parse_optimal_with_seed_and_search_window(data, lz::WINDOW, w)
    // by lz::parse_optimal_with_window's own documented contract, so
    // this capability must match it exactly at seed_window = lz::WINDOW,
    // not just approximately.
    let data: Vec<u8> = (0..5000u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    assert_eq!(
        compressed_len_with_seed_and_search_window(&data, lz::WINDOW, lz::WINDOW),
        compressed_len_with_window(&data, lz::WINDOW)
    );
}

#[test]
fn streaming_rejects_bad_match_distance_not_panicking() {
    // Same hand-crafted single-Match-token payload as
    // bad_match_distance_is_rejected_not_panicking, driven through
    // decode_to_writer's own loop instead of decode's: the two loops
    // are separate code, so this checks the new one's wiring directly
    // rather than trusting decode's coverage to also prove it.
    let payload = single_match_payload(1);

    let err = decode_streaming(&payload, MAX_DECODED_LEN)
        .expect_err("distance reaching before the start of output must be rejected");
    assert_eq!(as_codec_error(&err), Some(&Error::Corrupt));
}

#[test]
fn streaming_rejects_match_distance_beyond_window() {
    let over_window = u32::try_from(lz::WINDOW).expect("WINDOW fits u32") + 1;
    let payload = single_match_payload(over_window);

    let err = decode_streaming(&payload, MAX_DECODED_LEN)
        .expect_err("a distance past lz::WINDOW must be rejected");
    assert_eq!(as_codec_error(&err), Some(&Error::Corrupt));
}

#[test]
fn streaming_rejects_declared_length_over_the_max_before_any_work() {
    let over = MAX_DECODED_LEN + 1;
    let mut payload = Candidate::Identity.to_header_bytes().to_vec();
    payload.extend_from_slice(&over.to_le_bytes());
    payload.extend_from_slice(&over.to_le_bytes());
    let err = decode_streaming(&payload, MAX_DECODED_LEN)
        .expect_err("declared length past MAX_DECODED_LEN must be rejected");
    assert_eq!(
        as_codec_error(&err),
        Some(&Error::TooLarge {
            len: over,
            max: MAX_DECODED_LEN
        })
    );
}

#[test]
fn streaming_falls_back_to_decode_for_transpose_declared_length_over_the_max() {
    // Same amplification-hazard shape as the Identity case above, but
    // routed through decode_to_writer's fallback branch (the one
    // candidate, Transpose, that still needs decode's own whole-buffer
    // path), which must reject before calling it, not after. Identity,
    // Delta, and Bcj all stream instead (research/JOURNAL.md S1-P7).
    let over = MAX_DECODED_LEN + 1;
    let mut payload = Candidate::Transpose(crate::test_support::nz(2))
        .to_header_bytes()
        .to_vec();
    payload.extend_from_slice(&over.to_le_bytes());
    payload.extend_from_slice(&over.to_le_bytes());
    let err = decode_streaming(&payload, MAX_DECODED_LEN)
        .expect_err("declared length past MAX_DECODED_LEN must be rejected");
    assert_eq!(
        as_codec_error(&err),
        Some(&Error::TooLarge {
            len: over,
            max: MAX_DECODED_LEN
        })
    );
}

#[test]
fn streaming_propagates_the_writer_error_unwrapped() {
    // Distinguishes the two error origins decode_to_writer's docs
    // promise: a decode error comes back as WriteError::Decode
    // (as_codec_error unwraps it above), but a failure from the writer
    // itself must come back as WriteError::Io, carrying exactly what
    // the writer produced, not re-wrapped.
    struct FailingWriter;
    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let data = b"abcdefgh".repeat(50);
    let encoded = encode(&data);
    assert_eq!(
        encoded[0], 0,
        "expected Candidate::Identity for this fixture"
    );
    let mut writer = FailingWriter;
    let err = decode_to_writer(&encoded, MAX_DECODED_LEN, &mut writer)
        .expect_err("a writer that always fails must surface its error");
    assert!(
        as_codec_error(&err).is_none(),
        "a writer failure is not a decode Error and must not downcast to one"
    );
    match err {
        crate::WriteError::Io(err) => assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe),
        crate::WriteError::Decode(err) => {
            panic!("expected WriteError::Io, got Decode({err:?})")
        }
    }
}

/// [`TokenSink`] that isolates a token stream's literal cost through
/// [`price_literal`], ignoring flag/length/offset/slot: the whole-file
/// [`ideal_cost_bits`] and [`PairedTokenSink`]'s baseline share every
/// non-literal price function but use two different literal formulas on
/// purpose (plain [`crate::literal::Literal::ideal_cost_bits`] for the
/// baseline vs. the wired
/// [`crate::literal::Literal::ideal_cost_bits_logistic_surprise_logit_sse`]
/// for `price_literal`), so comparing the two totals directly would charge
/// that intentional difference as drift. Subtracting this sink's total from
/// [`ideal_cost_bits`]'s leaves the non-literal subtotal, the only part
/// [`ppm_expert_experiment_baseline_non_literal_subtotal_matches_ideal_cost_bits`]
/// needs to compare.
#[derive(Default)]
struct LiteralOnlyCostSink {
    bits: f64,
}

impl TokenSink for LiteralOnlyCostSink {
    fn flag(&mut self, _models: &mut Models, _flag_table: usize, _kind: FlagKind) {}

    fn literal(&mut self, models: &mut Models, context: Context, byte: u8) {
        self.bits += price_literal(models, context, byte);
    }

    fn length(&mut self, _models: &mut Models, _kind: FlagKind, _value: u32) {}

    fn offset(&mut self, _models: &mut Models, _len: u32, _value: u32) {}

    fn slot(&mut self, _models: &mut Models, _symbol: usize) {}
}

#[test]
fn ppm_expert_experiment_baseline_non_literal_subtotal_matches_ideal_cost_bits() {
    // Two Token::Match runs plus one Token::Rep (verified via
    // lz::parse_optimal's own token kinds below), so every non-literal
    // TokenSink method this guards (flag, length, offset, slot) is
    // actually exercised, not just flag/length on a literal-only walk.
    let data = b"abcabcabcabcXXXabcabcabcabcXXXabcabcabcabc".to_vec();
    let tokens = lz::parse_optimal(&data);
    assert!(
        tokens.iter().any(|t| matches!(t, Token::Match { .. }))
            && tokens.iter().any(|t| matches!(t, Token::Rep { .. })),
        "fixture must exercise both Token::Match and Token::Rep: {tokens:?}"
    );

    let mut literal_only_models = Models::new();
    let mut literal_only = LiteralOnlyCostSink::default();
    walk_tokens(&tokens, &data, &mut literal_only_models, &mut literal_only);
    let non_literal_from_whole = ideal_cost_bits(&data) - literal_only.bits;

    let mut models = Models::new();
    let mut paired = PairedTokenSink {
        cost: PairedCost::default(),
        pair_literal: |_: &mut Models, _: Context, _: u8| (0.0, 0.0),
    };
    walk_tokens(&tokens, &data, &mut models, &mut paired);

    assert!(
        (non_literal_from_whole - paired.cost.baseline).abs() < 1e-9,
        "non_literal_from_whole={non_literal_from_whole} paired_baseline={}",
        paired.cost.baseline
    );
}
