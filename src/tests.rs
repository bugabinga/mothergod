use super::*;

/// A complete `Method::Lz` frame around `payload`, check value included, for
/// tests that hand-craft a payload rather than going through [`compress`]:
/// several need a specific declared length or filter selector `compress`
/// itself would never choose.
fn lz_frame(payload: &[u8]) -> Vec<u8> {
    build_frame(Method::Lz, &[payload])
}

/// `MAGIC` + `FORMAT_VERSION` + `Method::Lz` and no payload or check value,
/// for the tests whose frame is cut short on purpose.
fn lz_frame_header() -> Vec<u8> {
    vec![
        MAGIC[0],
        MAGIC[1],
        MAGIC[2],
        MAGIC[3],
        FORMAT_VERSION,
        Method::Lz as u8,
    ]
}

#[test]
fn error_display_names_each_variant() {
    // Nothing else in the crate calls `Error::to_string`, so this is the
    // only thing standing between a `Display` edit and a silent user-facing
    // regression (mutants-check, PR #467: the whole function survived
    // whole-body replacement before this test existed).
    assert_eq!(
        Error::Truncated.to_string(),
        "input ended before the frame was complete"
    );
    assert_eq!(
        Error::BadMagic.to_string(),
        "input is not a mothergod frame (bad magic)"
    );
    assert_eq!(
        Error::UnsupportedVersion(9).to_string(),
        "unsupported format version 9"
    );
    assert_eq!(
        Error::UnknownMethod(0xFF).to_string(),
        "unknown compression method 255"
    );
    assert_eq!(Error::Corrupt.to_string(), "compressed payload is corrupt");
    assert_eq!(
        Error::BadChecksum.to_string(),
        "frame check value does not match its bytes"
    );
    assert_eq!(
        Error::TooLarge { len: 10, max: 5 }.to_string(),
        "output length 10 exceeds the decoder's bound (5 bytes)"
    );
    assert_eq!(
        Error::OutOfMemory.to_string(),
        "allocator could not satisfy a decode allocation"
    );
}

#[test]
fn each_error_converts_to_the_io_kind_that_names_it_without_a_payload() {
    use std::io::ErrorKind::{InvalidData, OutOfMemory, UnexpectedEof};
    let cases = [
        (Error::Truncated, UnexpectedEof),
        (Error::OutOfMemory, OutOfMemory),
        (Error::BadMagic, InvalidData),
        (Error::UnsupportedVersion(9), InvalidData),
        (Error::UnknownMethod(0xFF), InvalidData),
        (Error::Corrupt, InvalidData),
        (Error::BadChecksum, InvalidData),
        (Error::TooLarge { len: 10, max: 5 }, InvalidData),
    ];
    for (err, kind) in cases {
        let io = std::io::Error::from(err.clone());
        assert_eq!(io.kind(), kind, "{err:?}");
        // Boxing a payload is the allocation #479 removed from the decode path.
        assert!(io.get_ref().is_none(), "{err:?} carries a boxed payload");
    }
}

#[test]
fn write_error_converts_to_io_keeping_the_writers_own_error_whole() {
    let decode = std::io::Error::from(WriteError::Decode(Error::Truncated));
    assert_eq!(decode.kind(), std::io::ErrorKind::UnexpectedEof);

    let writer = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone");
    let io = std::io::Error::from(WriteError::Io(writer));
    assert_eq!(io.kind(), std::io::ErrorKind::BrokenPipe);
    assert_eq!(io.to_string(), "gone");
}

#[test]
fn a_decode_failure_propagates_with_question_mark_in_an_io_result_fn() {
    fn read(frame: &[u8]) -> std::io::Result<Vec<u8>> {
        Ok(decompress(frame)?)
    }
    assert_eq!(
        read(&compress(b"abc")).expect("a frame decodes"),
        b"abc".to_vec()
    );
    assert_eq!(
        read(b"").expect_err("empty input").kind(),
        std::io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn roundtrip_empty() {
    assert_eq!(decompress(&compress(b"")), Ok(Vec::new()));
}

#[test]
fn roundtrip_data() {
    let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
    assert_eq!(decompress(&compress(&input)), Ok(input));
}

#[test]
fn compressible_input_picks_method_lz() {
    let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
    let frame = compress(&input);
    assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
    assert!(
        frame.len() < input.len(),
        "a 100x repeat should compress smaller than the input: {} -> {}",
        input.len(),
        frame.len()
    );
    assert_eq!(decompress(&frame), Ok(input));
}

#[test]
fn compressible_input_past_the_decode_bound_is_stored_and_round_trips() {
    // `decompress` rejects an `Lz` frame past `MAX_DECODED_LEN`, so
    // `compress` must not write one. Past the bound no encode runs, which
    // keeps this cheap.
    let input = vec![0u8; codec::MAX_DECODED_LEN as usize + 1];
    let frame = compress(&input);
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    assert!(decompress(&frame).is_ok_and(|output| output == input));
}

/// The methods of `stream`'s frames, in order.
fn frame_methods(stream: &[u8]) -> Vec<Method> {
    Frames::new(stream)
        .map(|frame| frame.expect("compress writes whole frames").method)
        .collect()
}

#[test]
fn input_past_the_block_length_is_one_frame_per_block_and_round_trips() {
    // Block 0 compresses, block 1 is incompressible so it falls back to
    // `Stored` on its own, and the short tail is a third frame: each
    // block chooses its method alone.
    const BLOCK: usize = 4096;
    let mut input = vec![b'a'; BLOCK];
    input.extend(
        test_support::Xorshift32::new(1)
            .take(BLOCK)
            .map(|word| word.to_le_bytes()[1]),
    );
    input.extend_from_slice(b"tail");
    let stream = compress_blocks(&input, BLOCK);
    assert_eq!(
        frame_methods(&stream),
        [Method::Lz, Method::Stored, Method::Stored]
    );
    assert_eq!(decompress(&stream), Ok(input));
}

#[test]
fn input_of_exactly_the_block_length_is_one_frame() {
    const BLOCK: usize = 4096;
    for len in [0, 1, BLOCK - 1, BLOCK] {
        let input = vec![b'a'; len];
        let stream = compress_blocks(&input, BLOCK);
        assert_eq!(frame_methods(&stream).len(), 1, "input of {len} bytes");
        assert_eq!(decompress(&stream), Ok(input));
    }
    let stream = compress_blocks(&vec![b'a'; BLOCK + 1], BLOCK);
    assert_eq!(frame_methods(&stream).len(), 2);
}

/// A reader that hands out `chunk` bytes per call, so `copy_encode_blocks`
/// cannot rely on one `read` filling a block.
struct Dribble<'a> {
    rest: &'a [u8],
    chunk: usize,
}

impl std::io::Read for Dribble<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.chunk.min(buf.len()).min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

#[test]
fn copy_encode_writes_the_bytes_compress_returns_at_every_block_boundary() {
    const BLOCK: usize = 4096;
    let noise: Vec<u8> = test_support::Xorshift32::new(7)
        .take(BLOCK)
        .map(|word| word.to_le_bytes()[1])
        .collect();
    let mut mixed = vec![b'a'; BLOCK];
    mixed.extend_from_slice(&noise);
    mixed.extend_from_slice(b"tail");
    for input in [
        Vec::new(),
        vec![b'a'],
        vec![b'a'; BLOCK - 1],
        vec![b'a'; BLOCK],
        vec![b'a'; BLOCK + 1],
        vec![b'a'; 3 * BLOCK],
        mixed,
    ] {
        for chunk in [1, 1000, usize::MAX] {
            let mut sink = Vec::new();
            copy_encode_blocks(
                Dribble {
                    rest: &input,
                    chunk,
                },
                &mut sink,
                BLOCK,
            )
            .expect("a slice and a Vec do not fail");
            assert_eq!(
                sink,
                compress_blocks(&input, BLOCK),
                "input of {} bytes read {chunk} at a time",
                input.len()
            );
            assert_eq!(decompress(&sink), Ok(input.clone()));
        }
    }
}

#[test]
fn copy_encode_reports_a_failing_source_and_a_failing_sink() {
    struct Failing;
    impl std::io::Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::PermissionDenied.into())
        }
    }
    impl std::io::Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut sink = Vec::new();
    let err = copy_encode(Failing, &mut sink).expect_err("the source failed");
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        sink.is_empty(),
        "no frame is written for a block never read"
    );
    let err = copy_encode(&b"abc"[..], Failing).expect_err("the sink failed");
    assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
}

#[test]
fn candidate_beats_incumbent_keeps_the_strictly_smaller_candidate_only() {
    assert!(
        candidate_beats_incumbent(3, 5),
        "a strictly smaller candidate must win"
    );
    assert!(
        !candidate_beats_incumbent(5, 5),
        "a tied candidate must not displace the incumbent"
    );
    assert!(
        !candidate_beats_incumbent(6, 5),
        "a larger candidate must not win"
    );
}

#[test]
fn retired_version_lz_frame_is_rejected_by_every_entry_point() {
    // `Method::Lz` frames of any version below FORMAT_VERSION were retired
    // (docs/adr/0063-retire-format-versions-1-through-9.md): their payloads
    // code under models this build no longer holds, so decoding one under
    // the current models would produce garbage or a misleading Corrupt.
    // Every entry point must name the version instead, never reach
    // codec::decode.
    let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
    let mut frame = compress(&input);
    assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
    for version in 0..FORMAT_VERSION {
        frame[MAGIC.len()] = version;
        assert_eq!(
            decompress(&frame),
            Err(Error::UnsupportedVersion(version)),
            "decompress, version {version}"
        );
        assert_eq!(
            decodes_incrementally(&frame),
            Err(Error::UnsupportedVersion(version)),
            "decodes_incrementally, version {version}"
        );
        let mut out = Vec::new();
        let err = decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out)
            .expect_err("a retired version must be rejected");
        assert_eq!(
            test_support::as_codec_error(&err),
            Some(&Error::UnsupportedVersion(version)),
            "decompress_to_writer, version {version}"
        );
    }
}

#[test]
fn stored_frame_of_any_other_version_is_unsupported() {
    // Version 10 wrote a Stored payload with no declared length: the same
    // bytes now read as a length and a body, so it must not decode.
    let mut frame = compress(b"hi");
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    for version in [0, FORMAT_VERSION - 1] {
        frame[MAGIC.len()] = version;
        assert_eq!(
            decompress(&frame),
            Err(Error::UnsupportedVersion(version)),
            "version {version}"
        );
    }
}

/// One frame of each method: a short `Lz` frame and a `Stored` one.
fn one_frame_of_each_method() -> [(Vec<u8>, Vec<u8>); 2] {
    let text = b"the quick brown fox jumps over the lazy dog".repeat(20);
    let random: Vec<u8> = test_support::Xorshift32::new(0x9E37_79B9)
        .take(300)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();
    [text, random].map(|input| (compress(&input), input))
}

#[test]
fn every_single_bit_flip_of_a_frame_is_rejected_and_writes_nothing() {
    // #1020: a flip inside a payload once decoded to wrong bytes with
    // exit 0. The check value covers the whole frame, so no flip anywhere
    // in either method's frame gets through any entry point.
    let [(lz, _), (stored, _)] = one_frame_of_each_method();
    assert_eq!(lz[METHOD_OFFSET], Method::Lz as u8);
    assert_eq!(stored[METHOD_OFFSET], Method::Stored as u8);
    for frame in [lz, stored] {
        for bit in 0..frame.len() * 8 {
            let mut flipped = frame.clone();
            flipped[bit / 8] ^= 1 << (bit % 8);
            assert!(decompress(&flipped).is_err(), "decompress, bit {bit}");
            assert!(
                decodes_incrementally(&flipped).is_err(),
                "decodes_incrementally, bit {bit}"
            );
            let mut out = Vec::new();
            assert!(
                decompress_to_writer(&flipped, codec::MAX_DECODED_LEN, &mut out).is_err(),
                "decompress_to_writer, bit {bit}"
            );
            assert!(out.is_empty(), "bit {bit} wrote {} bytes", out.len());
        }
    }
}

#[test]
fn a_flip_past_the_header_is_a_checksum_error() {
    // A flip in the data, the coded stream, or the check value itself is
    // named as what it is, not as whatever the damaged payload decodes to.
    // The first payload byte is the `Lz` filter selector or a `Stored`
    // length's low byte, which the extent parse may reject first.
    for (frame, _) in one_frame_of_each_method() {
        for index in [frame.len() - CHECK_LEN - 1, frame.len() - 1] {
            let mut flipped = frame.clone();
            flipped[index] ^= 0x01;
            assert_eq!(
                decompress(&flipped),
                Err(Error::BadChecksum),
                "byte {index}"
            );
        }
    }
}

#[test]
fn a_damaged_frame_fails_the_stream_after_the_frames_before_it_decoded() {
    // The check is per frame, so frames before the damaged one were valid;
    // `decompress` returns nothing, `decompress_to_writer` has written them.
    let [(lz, lz_input), (mut stored, _)] = one_frame_of_each_method();
    let last = stored.len() - 1;
    stored[last] ^= 0x01;
    let stream = [lz, stored].concat();
    assert_eq!(decompress(&stream), Err(Error::BadChecksum));
    let mut out = Vec::new();
    let err = decompress_to_writer(&stream, codec::MAX_DECODED_LEN, &mut out)
        .expect_err("the second frame is damaged");
    assert_eq!(
        test_support::as_codec_error(&err),
        Some(&Error::BadChecksum)
    );
    assert_eq!(out, lz_input);
}

#[test]
fn every_prefix_of_a_stored_frame_is_rejected() {
    // #988: a Stored frame cut short once decoded to the bytes that
    // arrived, exit 0. Every strict prefix must fail, as an Lz one does.
    let input = b"hi there";
    let frame = compress(input);
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    for cut in 0..frame.len() {
        let err = decompress(&frame[..cut]).expect_err("a strict prefix must not decode");
        assert_eq!(err, Error::Truncated, "cut at {cut}");
    }
    assert_eq!(decompress(&frame), Ok(input.to_vec()));
}

#[test]
fn bytes_past_the_last_frame_are_not_a_frame() {
    // #978: trailing bytes were dropped in silence, exit 0. They are the
    // start of a next frame, and none of these is one.
    let frame = compress(b"hi there");
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    for (tail, err) in [
        (&b"x"[..], Error::Truncated),
        (&b"xyzxyz"[..], Error::BadMagic),
        (&frame[..frame.len() - 1], Error::Truncated),
    ] {
        let mut stream = frame.clone();
        stream.extend_from_slice(tail);
        assert_eq!(decompress(&stream), Err(err.clone()), "tail {tail:?}");
        assert_eq!(
            test_support::as_codec_error(
                &decompress_to_writer(&stream, u32::MAX, &mut Vec::new())
                    .expect_err("trailing bytes must not decode")
            ),
            Some(&err),
            "writer, tail {tail:?}"
        );
        assert_eq!(decodes_incrementally(&stream), Err(err));
    }
}

#[test]
fn a_stream_of_frames_decodes_to_their_payloads_back_to_back() {
    let (stream, plain) = test_support::mixed_stream();
    assert_eq!(decompress(&stream), Ok(plain.clone()));
    let mut out = Vec::new();
    decompress_to_writer(&stream, u32::MAX, &mut out).expect("a stream decodes");
    assert_eq!(out, plain);
}

#[test]
fn every_prefix_of_a_stream_is_a_stream_or_truncated() {
    // A cut inside a frame is Truncated; a cut on a frame boundary is a
    // shorter stream and decodes. Nothing else is possible.
    let (stream, plain) = test_support::mixed_stream();
    for cut in 0..stream.len() {
        match decompress(&stream[..cut]) {
            Ok(prefix) => assert!(plain.starts_with(&prefix), "cut at {cut}"),
            Err(err) => assert_eq!(err, Error::Truncated, "cut at {cut}"),
        }
    }
}

#[test]
fn max_len_bounds_a_streams_total_output() {
    // A bound tighter than MAX_DECODED_LEN caps the sum over frames, so a
    // stream of small frames is no way around it.
    let (stream, plain) = test_support::mixed_stream();
    let total = u32::try_from(plain.len()).expect("fits");
    assert_eq!(decompress_bounded(&stream, total), Ok(plain.clone()));
    let tight = total - 1;
    let expected = Err(Error::TooLarge {
        len: total,
        max: tight,
    });
    assert_eq!(decompress_bounded(&stream, tight), expected);
    let mut out = Vec::new();
    let err =
        decompress_to_writer(&stream, tight, &mut out).expect_err("the total exceeds the bound");
    assert_eq!(test_support::as_codec_error(&err), expected.as_ref().err());
}

#[test]
fn collected_lz_output_is_capped_in_total_but_streamed_output_is_not() {
    // Each frame is legal alone, their sum is not: memory held across
    // frames must not outgrow the bytes that paid for it, and an `Lz`
    // frame's output is not bounded by its input (#1085 review). The
    // rejection is from the headers, so the second frame's decode work
    // (about 2 s per MiB at this length) never runs.
    let (stream, plain) = test_support::mixed_stream();
    let big = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Identity.to_header_bytes(),
        codec::MAX_DECODED_LEN,
        codec::MAX_DECODED_LEN,
        &[],
    ));
    let mut over = stream.clone();
    over.extend(big.clone());
    // Two bytes of `plain` are the `Stored` "hi"; the rest is the `Lz` frame.
    let lz_len = u32::try_from(plain.len() - 2).expect("fits");
    let lz_total = lz_len + codec::MAX_DECODED_LEN;
    let expected = Err(Error::TooLarge {
        len: lz_total,
        max: codec::MAX_DECODED_LEN,
    });
    assert_eq!(decompress(&over), expected);
    assert_eq!(decompress_bounded(&over, u32::MAX), expected);
}

#[test]
fn only_a_collecting_budget_caps_the_lz_total() {
    // `decompress_to_writer` holds one write buffer however long the
    // stream is, so it is exempt from the `Lz` total the `Vec`-returning
    // functions carry; the block-split streams past 256 MiB rely on it.
    let lz = (Method::Lz, u64::from(codec::MAX_DECODED_LEN));
    let mut streaming = Budget::new(u32::MAX, false);
    assert_eq!(streaming.admit(lz.0, lz.1), Ok(()));
    assert_eq!(streaming.admit(lz.0, lz.1), Ok(()));
    let mut collecting = Budget::new(u32::MAX, true);
    assert_eq!(collecting.admit(lz.0, lz.1), Ok(()));
    assert_eq!(
        collecting.admit(lz.0, lz.1),
        Err(Error::TooLarge {
            len: 2 * codec::MAX_DECODED_LEN,
            max: codec::MAX_DECODED_LEN,
        })
    );
}

#[test]
fn a_streamed_lz_total_past_max_decoded_len_is_written_not_rejected() {
    // Pins `decompress_to_writer`'s call site of `Budget::new` (#1088),
    // which `only_a_collecting_budget_caps_the_lz_total` cannot: the
    // concatenation of two large `Lz` outputs (ADR-0067) decodes through
    // the writer, one frame past the cap `admit` tests with `>`. Decode
    // work scales with output, so this costs about a minute in the test
    // profile whatever the frame size; it runs beside the rest of the lib
    // tests, which take longer in total.
    const FRAME_LEN: usize = 1 << 20;
    let frame = compress(&vec![0u8; FRAME_LEN]);
    assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
    let frames = (codec::MAX_DECODED_LEN as usize) / FRAME_LEN + 1;
    let stream = frame.repeat(frames);
    let mut out = std::io::sink();
    assert!(decompress_to_writer(&stream, u32::MAX, &mut out).is_ok());
}

#[test]
fn a_stream_decodes_incrementally_only_if_every_frame_does() {
    let (mut stream, _) = test_support::mixed_stream();
    assert_eq!(decodes_incrementally(&stream), Ok(true));
    let transpose = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Transpose(test_support::nz(4)).to_header_bytes(),
        test_support::floor_declared_len(),
        0,
        &[],
    ));
    stream.extend(transpose);
    assert_eq!(decodes_incrementally(&stream), Ok(false));

    let (mut stream, _) = test_support::mixed_stream();
    let base64 = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Base64.to_header_bytes(),
        test_support::floor_declared_len(),
        0,
        &[],
    ));
    stream.extend(base64);
    assert_eq!(decodes_incrementally(&stream), Ok(false));

    let (mut stream, _) = test_support::mixed_stream();
    let reverse = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Reverse.to_header_bytes(),
        test_support::floor_declared_len(),
        0,
        &[],
    ));
    stream.extend(reverse);
    assert_eq!(decodes_incrementally(&stream), Ok(false));

    let (mut stream, _) = test_support::mixed_stream();
    let reverse_delta = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::ReverseDelta(test_support::nz(20)).to_header_bytes(),
        test_support::floor_declared_len(),
        0,
        &[],
    ));
    stream.extend(reverse_delta);
    assert_eq!(decodes_incrementally(&stream), Ok(false));
}

#[test]
fn a_reverse_frame_round_trips_through_every_entry_point() {
    use std::io::Read as _;

    let rows = test_support::le_rows(2500);
    let frame = compress(&rows);
    assert_eq!(frame[HEADER_LEN..HEADER_LEN + 2], [5, 0], "picks Reverse");

    assert_eq!(decompress(&frame).as_deref(), Ok(rows.as_slice()));
    let len = u32::try_from(rows.len()).expect("small");
    assert_eq!(
        decompress_bounded(&frame, len).as_deref(),
        Ok(rows.as_slice())
    );
    assert_eq!(
        decompress_bounded(&frame, len - 1),
        Err(Error::TooLarge { len, max: len - 1 })
    );
    let mut written = Vec::new();
    decompress_to_writer(&frame, len, &mut written).expect("writes");
    assert_eq!(written, rows);
    assert_eq!(decodes_incrementally(&frame), Ok(false));

    let mut read = Vec::new();
    read::Decoder::new(frame.as_slice())
        .read_to_end(&mut read)
        .expect("reads");
    assert_eq!(read, rows);
}

#[test]
fn a_reverse_delta_frame_round_trips_through_every_entry_point() {
    use std::io::Read as _;

    let rows = test_support::le_rows(100);
    let frame = compress(&rows);
    assert_eq!(frame[HEADER_LEN], 6, "picks ReverseDelta");

    assert_eq!(decompress(&frame).as_deref(), Ok(rows.as_slice()));
    let len = u32::try_from(rows.len()).expect("small");
    assert_eq!(
        decompress_bounded(&frame, len).as_deref(),
        Ok(rows.as_slice())
    );
    assert_eq!(
        decompress_bounded(&frame, len - 1),
        Err(Error::TooLarge { len, max: len - 1 })
    );
    let mut written = Vec::new();
    decompress_to_writer(&frame, len, &mut written).expect("writes");
    assert_eq!(written, rows);
    assert_eq!(decodes_incrementally(&frame), Ok(false));

    let mut read = Vec::new();
    read::Decoder::new(frame.as_slice())
        .read_to_end(&mut read)
        .expect("reads");
    assert_eq!(read, rows);
}

#[test]
fn a_base64_frame_round_trips_through_every_entry_point_and_budgets_its_output() {
    use std::io::Read as _;

    let wrapped = filters::base64_unwrap::decode(&test_support::record_text(60));
    let frame = compress(&wrapped);
    assert_eq!(frame[HEADER_LEN..HEADER_LEN + 2], [4, 0], "picks Base64");
    assert!(frame.len() < wrapped.len() / 4, "and it pays");

    assert_eq!(decompress(&frame).as_deref(), Ok(wrapped.as_slice()));
    let len = u32::try_from(wrapped.len()).expect("small");
    assert_eq!(
        decompress_bounded(&frame, len).as_deref(),
        Ok(wrapped.as_slice())
    );
    // The budget sees the output a frame decodes to, not its filtered bytes.
    assert_eq!(
        decompress_bounded(&frame, len - 1),
        Err(Error::TooLarge { len, max: len - 1 })
    );
    let mut written = Vec::new();
    decompress_to_writer(&frame, len, &mut written).expect("writes");
    assert_eq!(written, wrapped);
    let mut refused = Vec::new();
    assert!(decompress_to_writer(&frame, len - 1, &mut refused).is_err());
    assert!(refused.is_empty(), "rejected before a byte is written");
    assert_eq!(decodes_incrementally(&frame), Ok(false));

    let mut read = Vec::new();
    read::Decoder::new(frame.as_slice())
        .read_to_end(&mut read)
        .expect("reads");
    assert_eq!(read, wrapped);
}

#[test]
fn stored_frame_declaring_u64_max_is_truncated_not_allocated() {
    // The declared length is never an allocation size: a hostile one is
    // compared against the bytes actually present.
    let mut frame = compress(b"hi");
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    frame[HEADER_LEN..HEADER_LEN + STORED_LEN_BYTES].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(decompress(&frame), Err(Error::Truncated));
}

#[test]
fn lz_frame_declaring_a_huge_coded_length_is_truncated_not_allocated() {
    // The coded length is compared against the bytes present, so the
    // largest one the Stored floor admits is cut at the frame, not read.
    let mut frame = compress(&b"the quick brown fox ".repeat(50));
    assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
    test_support::forge_floor_extent(&mut frame, codec::MAX_DECODED_LEN);
    assert_eq!(decompress(&frame), Err(Error::Truncated));
}

#[test]
fn lz_frame_over_the_stored_floor_is_corrupt_from_its_header() {
    // An `Lz` payload longer than the `Stored` payload of the same output
    // cannot come from a conforming encoder (SPEC "Invariants", Stored
    // floor), so the header alone convicts it: one byte over is `Corrupt`,
    // at the floor it is only cut short.
    let mut frame = compress(&b"the quick brown fox ".repeat(50));
    test_support::forge_floor_extent(&mut frame, 1000);
    assert_eq!(decompress(&frame), Err(Error::Truncated));
    let coded_field = HEADER_LEN + codec::EXTENT_PREFIX_LEN - size_of::<u64>();
    let at_floor = u64::from_le_bytes(frame[coded_field..coded_field + 8].try_into().unwrap());
    frame[coded_field..coded_field + 8].copy_from_slice(&(at_floor + 1).to_le_bytes());
    assert_eq!(decompress(&frame), Err(Error::Corrupt));
    frame[coded_field..coded_field + 8].copy_from_slice(&(1u64 << 40).to_le_bytes());
    assert_eq!(decompress(&frame), Err(Error::Corrupt));
}

#[test]
fn stored_frame_is_never_larger_than_header_length_field_and_input() {
    // The Stored floor (docs/format/SPEC.md): every frame is at most this.
    for len in [0, 1, 2, 100] {
        let input = vec![0xA5u8; len];
        assert!(compress(&input).len() <= HEADER_LEN + STORED_LEN_BYTES + len + CHECK_LEN);
    }
}

#[test]
fn tiny_input_falls_back_to_stored() {
    // A handful of bytes: Method::Lz's 8-byte header alone already
    // exceeds this, so compress must pick Stored (the "Stored floor"
    // invariant, docs/format/SPEC.md).
    let input = b"hi";
    let frame = compress(input);
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    assert_eq!(decompress(&frame), Ok(input.to_vec()));
}

#[test]
fn incompressible_input_falls_back_to_stored_and_roundtrips() {
    let input: Vec<u8> = test_support::Xorshift32::new(0x9E37_79B9)
        .take(2000)
        .map(|state| u8::try_from(state % 256).unwrap())
        .collect();
    let frame = compress(&input);
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    assert_eq!(decompress(&frame), Ok(input));
}

#[test]
fn truncated_input_is_rejected() {
    assert_eq!(decompress(b"MGDC"), Err(Error::Truncated));
}

#[test]
fn bad_magic_is_rejected() {
    assert_eq!(decompress(b"NOPE\0\0data"), Err(Error::BadMagic));
}

#[test]
fn future_version_is_rejected() {
    let mut frame = compress(b"x");
    frame[MAGIC.len()] = FORMAT_VERSION + 1;
    assert_eq!(
        decompress(&frame),
        Err(Error::UnsupportedVersion(FORMAT_VERSION + 1))
    );
}

#[test]
fn unknown_method_is_rejected() {
    let mut frame = compress(b"x");
    frame[MAGIC.len() + 1] = 0xFF;
    assert_eq!(decompress(&frame), Err(Error::UnknownMethod(0xFF)));
}

#[test]
fn lz_declared_length_over_the_max_is_rejected() {
    // A tiny frame declaring an output past codec::MAX_DECODED_LEN: must
    // reject on the declared length. The token count is zero so a mutated
    // guard falls through to a fast `Corrupt`, not minutes of decoding
    // (#1124); the adversarial seed `lz-declared-size-amplification-bomb`
    // pins the matching-token-count amplification shape.
    let over = codec::MAX_DECODED_LEN + 1;
    let frame = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Identity.to_header_bytes(),
        over,
        0,
        &[],
    ));
    assert_eq!(
        decompress(&frame),
        Err(Error::TooLarge {
            len: over,
            max: codec::MAX_DECODED_LEN
        })
    );
}

#[test]
fn decompress_matches_decompress_bounded_at_the_max() {
    let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
    let frame = compress(&input);
    assert_eq!(
        decompress(&frame),
        decompress_bounded(&frame, codec::MAX_DECODED_LEN)
    );
}

#[test]
fn decompress_bounded_rejects_an_lz_frame_over_its_own_tighter_bound() {
    // Legal under codec::MAX_DECODED_LEN, but a caller with a smaller
    // memory budget must still be able to reject it before any decode
    // work (ROADMAP M4's bounded-memory decode guarantee).
    let input = b"the quick brown fox jumps over the lazy dog".repeat(100);
    let frame = compress(&input);
    assert_eq!(frame[METHOD_OFFSET], Method::Lz as u8);
    let declared_len = u32::try_from(input.len()).unwrap();
    assert_eq!(
        decompress_bounded(&frame, declared_len - 1),
        Err(Error::TooLarge {
            len: declared_len,
            max: declared_len - 1
        })
    );
    assert_eq!(decompress_bounded(&frame, declared_len), Ok(input));
}

#[test]
fn decompress_bounded_rejects_a_stored_frame_over_its_own_tighter_bound() {
    // decompress_bounded must bound Method::Stored by its declared
    // length rather than only ever bounding Method::Lz.
    let input = b"hi";
    let frame = compress(input);
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    assert_eq!(
        decompress_bounded(&frame, 1),
        Err(Error::TooLarge {
            len: u32::try_from(input.len()).unwrap(),
            max: 1
        })
    );
    assert_eq!(
        decompress_bounded(&frame, u32::try_from(input.len()).unwrap()),
        Ok(input.to_vec())
    );
}

#[test]
fn decompress_bounded_clamps_a_max_len_above_max_decoded_len() {
    // A caller passing u32::MAX must not bypass MAX_DECODED_LEN: the
    // clamp, not the caller's value, is the real ceiling.
    let over = codec::MAX_DECODED_LEN + 1;
    let frame = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Identity.to_header_bytes(),
        over,
        0,
        &[],
    ));
    assert_eq!(
        decompress_bounded(&frame, u32::MAX),
        Err(Error::TooLarge {
            len: over,
            max: codec::MAX_DECODED_LEN
        })
    );
}

#[test]
fn decompress_roundtrips_a_stored_payload_past_max_decoded_len() {
    // The regression this guards: decompress must stay exactly as
    // unbounded for Method::Stored as it was before decompress_bounded
    // existed, since the payload length is read from `input` itself,
    // never spoofable past it (see decompress_bounded's docs). Builds
    // the frame directly rather than via compress(), which would run
    // the full LZ encoder over 256+ MiB just to hit the Stored
    // fallback; this test's target is decompress, not compress.
    let payload = vec![0xA5u8; (codec::MAX_DECODED_LEN + 1) as usize];
    let frame = build_stored_frame(&payload);
    assert_eq!(decompress(&frame), Ok(payload));
}

#[test]
fn stored_frame_decodes_incrementally() {
    assert_eq!(decodes_incrementally(&compress(b"hi")), Ok(true));
}

#[test]
fn non_transpose_candidates_decode_incrementally() {
    for candidate in [
        filters::select::Candidate::Identity,
        filters::select::Candidate::Delta(test_support::nz(4)),
        filters::select::Candidate::Bcj,
    ] {
        let frame = lz_frame(&test_support::lz_payload(
            candidate.to_header_bytes(),
            test_support::floor_declared_len(),
            0,
            &[],
        ));
        assert_eq!(
            decodes_incrementally(&frame),
            Ok(true),
            "{candidate:?} should decode incrementally"
        );
    }
}

#[test]
fn transpose_candidate_does_not_decode_incrementally() {
    let frame = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Transpose(test_support::nz(4)).to_header_bytes(),
        test_support::floor_declared_len(),
        0,
        &[],
    ));
    assert_eq!(decodes_incrementally(&frame), Ok(false));
}

#[test]
fn decodes_incrementally_pins_the_lz_version_boundary() {
    // #388: three mutants survived on the version guard (`< false`, `<` ->
    // `==`, `<` -> `<=`) because no test distinguished it from a
    // version-blind one. Both sides of the boundary, on the same frame
    // shape so only the version byte differs.
    let mut frame = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Identity.to_header_bytes(),
        test_support::floor_declared_len(),
        0,
        &[],
    ));
    frame[MAGIC.len()] = FORMAT_VERSION - 1;
    assert_eq!(
        decodes_incrementally(&frame),
        Err(Error::UnsupportedVersion(FORMAT_VERSION - 1))
    );
    frame[MAGIC.len()] = FORMAT_VERSION;
    assert_eq!(decodes_incrementally(&frame), Ok(true));
}

#[test]
fn malformed_filter_selector_is_rejected_as_corrupt() {
    // [0, 1]: kind 0 (Identity) never carries a nonzero param, so
    // Candidate::from_header_bytes names no real candidate for it
    // (filters.rs's own unit tests cover the same byte pair).
    let mut frame = lz_frame_header();
    frame.extend_from_slice(&[0, 1]);
    assert_eq!(decodes_incrementally(&frame), Err(Error::Corrupt));
}

#[test]
fn truncated_filter_selector_is_rejected() {
    let mut frame = lz_frame_header();
    frame.push(0);
    assert_eq!(decodes_incrementally(&frame), Err(Error::Truncated));
}

#[test]
fn decodes_incrementally_shares_header_errors_with_decompress() {
    let mut frame = compress(b"hi");
    frame[0] = frame[0].wrapping_add(1);
    assert_eq!(decodes_incrementally(&frame), Err(Error::BadMagic));
}

#[test]
fn decompress_to_writer_matches_decompress_for_stored_and_lz_frames() {
    for input in [
        b"hi".to_vec(),
        b"the quick brown fox jumps over the lazy dog".repeat(100),
    ] {
        let frame = compress(&input);
        let via_decompress = decompress(&frame).unwrap();
        let mut out = Vec::new();
        decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out).unwrap();
        assert_eq!(out, via_decompress);
    }
}

#[test]
fn decompress_to_writer_rejects_an_lz_frame_over_the_max() {
    let over = codec::MAX_DECODED_LEN + 1;
    let frame = lz_frame(&test_support::lz_payload(
        filters::select::Candidate::Identity.to_header_bytes(),
        over,
        0,
        &[],
    ));
    let mut out = Vec::new();
    let err = decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out)
        .expect_err("declared length past MAX_DECODED_LEN must be rejected");
    assert_eq!(
        test_support::as_codec_error(&err),
        Some(&Error::TooLarge {
            len: over,
            max: codec::MAX_DECODED_LEN
        })
    );
}

#[test]
fn decompress_to_writer_rejects_a_stored_frame_over_its_own_tighter_bound() {
    let input = b"hi";
    let frame = compress(input);
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    let mut out = Vec::new();
    let err = decompress_to_writer(&frame, 1, &mut out)
        .expect_err("a Stored payload over a caller's tighter bound must be rejected");
    assert_eq!(
        test_support::as_codec_error(&err),
        Some(&Error::TooLarge {
            len: u32::try_from(input.len()).unwrap(),
            max: 1
        })
    );
    let mut out = Vec::new();
    decompress_to_writer(&frame, u32::try_from(input.len()).unwrap(), &mut out).unwrap();
    assert_eq!(out, input);
}

#[test]
fn decompress_to_writer_rejects_unsupported_version() {
    let mut frame = compress(b"hi");
    frame[VERSION_OFFSET] = FORMAT_VERSION + 1;
    let mut out = Vec::new();
    let err = decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut out)
        .expect_err("a newer format version must be rejected");
    assert_eq!(
        test_support::as_codec_error(&err),
        Some(&Error::UnsupportedVersion(FORMAT_VERSION + 1))
    );
}

#[test]
fn decompress_to_writer_propagates_writer_errors_unwrapped() {
    struct FailingWriter;
    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }

    let frame = compress(b"hi");
    let mut writer = FailingWriter;
    let err = decompress_to_writer(&frame, codec::MAX_DECODED_LEN, &mut writer)
        .expect_err("a writer that always fails must surface its error");
    assert!(
        test_support::as_codec_error(&err).is_none(),
        "a writer failure is not a decode Error and must not downcast to one"
    );
    match err {
        WriteError::Io(err) => assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe),
        WriteError::Decode(err) => panic!("expected WriteError::Io, got Decode({err:?})"),
    }
}
