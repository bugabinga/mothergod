use super::*;

/// `MAGIC` + `FORMAT_VERSION` + `Method::Lz` as a fresh `Vec<u8>`, for
/// tests that hand-craft a payload past it rather than going through
/// [`compress`]: several need a specific declared length or filter
/// selector `compress` itself would never choose.
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
        Error::TooLarge { len: 10, max: 5 }.to_string(),
        "output length 10 exceeds the decoder's bound (5 bytes)"
    );
    assert_eq!(
        Error::OutOfMemory.to_string(),
        "allocator could not satisfy a decode allocation"
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
fn stored_frame_with_bytes_past_its_declared_length_is_corrupt() {
    let mut frame = compress(b"hi there");
    assert_eq!(frame[METHOD_OFFSET], Method::Stored as u8);
    frame.push(0);
    assert_eq!(decompress(&frame), Err(Error::Corrupt));
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
fn stored_frame_is_never_larger_than_header_length_field_and_input() {
    // The Stored floor (docs/format/SPEC.md): every frame is at most this.
    for len in [0, 1, 2, 100] {
        let input = vec![0xA5u8; len];
        assert!(compress(&input).len() <= HEADER_LEN + STORED_LEN_BYTES + len);
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
    // A tiny frame declaring an output far past codec::MAX_DECODED_LEN
    // (and a matching token count, so the loop-iterations argument in
    // codec::decode's docs doesn't save it either): must reject before
    // doing any decode work, not just eventually. Public-API-level
    // regression for the amplification hazard codec.rs's unit tests
    // cover directly.
    let over = codec::MAX_DECODED_LEN + 1;
    let mut frame = lz_frame_header();
    frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
    frame.extend_from_slice(&over.to_le_bytes());
    frame.extend_from_slice(&over.to_le_bytes());
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
    let mut frame = lz_frame_header();
    frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
    frame.extend_from_slice(&over.to_le_bytes());
    frame.extend_from_slice(&over.to_le_bytes());
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
        let mut frame = lz_frame_header();
        frame.extend_from_slice(&candidate.to_header_bytes());
        assert_eq!(
            decodes_incrementally(&frame),
            Ok(true),
            "{candidate:?} should decode incrementally"
        );
    }
}

#[test]
fn transpose_candidate_does_not_decode_incrementally() {
    let mut frame = lz_frame_header();
    frame.extend_from_slice(
        &filters::select::Candidate::Transpose(test_support::nz(4)).to_header_bytes(),
    );
    assert_eq!(decodes_incrementally(&frame), Ok(false));
}

#[test]
fn decodes_incrementally_pins_the_lz_version_boundary() {
    // #388: three mutants survived on the version guard (`< false`, `<` ->
    // `==`, `<` -> `<=`) because no test distinguished it from a
    // version-blind one. Both sides of the boundary, on the same frame
    // shape so only the version byte differs.
    let mut frame = lz_frame_header();
    frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
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
    let mut frame = lz_frame_header();
    frame.extend_from_slice(&filters::select::Candidate::Identity.to_header_bytes());
    frame.extend_from_slice(&over.to_le_bytes());
    frame.extend_from_slice(&over.to_le_bytes());
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
