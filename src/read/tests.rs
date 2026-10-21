use std::io::ErrorKind;

use super::*;
use crate::test_support::{
    bcj_call_dense_data, columnar_drift_data, forge_floor_extent, lz_total_past_max_decoded_len,
    mixed_stream, repeated_sentence, tabular_columns_data,
};
use crate::{FORMAT_VERSION, MAGIC, METHOD_OFFSET, WriteError, compress, decompress_to_writer};

/// A source that hands out one byte per `read`, the shortest read a `Read`
/// may return without being at its end.
struct OneByte<'a>(&'a [u8]);

impl Read for OneByte<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let Some((&byte, rest)) = self.0.split_first() else {
            return Ok(0);
        };
        let Some(slot) = buf.first_mut() else {
            return Ok(0);
        };
        *slot = byte;
        self.0 = rest;
        Ok(1)
    }
}

/// What a reader yielded and the error that ended it, if any.
fn drain(mut reader: impl Read) -> (Vec<u8>, Option<ErrorKind>) {
    let mut out = Vec::new();
    let err = reader.read_to_end(&mut out).err().map(|err| err.kind());
    (out, err)
}

/// The same, from [`decompress_to_writer`], which writes every frame before
/// a failing one and none of the failing one.
fn drain_to_writer(stream: &[u8]) -> (Vec<u8>, Option<ErrorKind>) {
    let mut out = Vec::new();
    let err = decompress_to_writer(stream, codec::MAX_DECODED_LEN, &mut out)
        .err()
        .map(|err| io::Error::from(err).kind());
    (out, err)
}

#[test]
fn every_method_and_filter_class_round_trips_through_the_reader() {
    // The selector's first byte is the filter kind: 0 identity, 1 delta,
    // 2 bcj, 3 transpose, 6 reverse-delta (`codec` tests pin the same bytes).
    let classes: [(&str, Method, u8, Vec<u8>); 6] = [
        ("stored", Method::Stored, 0, b"hi".to_vec()),
        (
            "identity",
            Method::Lz,
            0,
            include_bytes!("../../research/imports/session-1/mothergod.rs").to_vec(),
        ),
        ("delta", Method::Lz, 1, repeated_sentence()),
        ("reverse-delta", Method::Lz, 6, columnar_drift_data()),
        ("bcj", Method::Lz, 2, bcj_call_dense_data()),
        ("transpose", Method::Lz, 3, tabular_columns_data(8, 2000, 1)),
    ];
    for (name, method, kind, data) in classes {
        let frame = compress(&data);
        assert_eq!(frame[METHOD_OFFSET], method as u8, "{name}: method");
        if method == Method::Lz {
            assert_eq!(frame[HEADER_LEN], kind, "{name}: filter kind");
        }
        assert_eq!(drain(Decoder::new(frame.as_slice())), (data.clone(), None));
        assert_eq!(drain(Decoder::new(OneByte(&frame))), (data, None), "{name}");
    }
}

#[test]
fn a_stream_of_frames_reads_as_their_payloads_back_to_back() {
    let (stream, plain) = mixed_stream();
    assert_eq!(drain(Decoder::new(stream.as_slice())), (plain, None));
}

#[test]
fn a_frame_that_decodes_to_nothing_is_not_the_end_of_the_stream() {
    let stream = [compress(b""), compress(b"abc"), compress(b"")].concat();
    assert_eq!(
        drain(Decoder::new(stream.as_slice())),
        (b"abc".to_vec(), None)
    );
}

#[test]
fn a_small_output_buffer_walks_a_frame_in_order() {
    let (stream, plain) = mixed_stream();
    let mut reader = Decoder::new(stream.as_slice());
    let mut out = Vec::new();
    let mut buf = [0u8; 7];
    loop {
        let len = reader.read(&mut buf).expect("a good stream reads");
        if len == 0 {
            break;
        }
        out.extend_from_slice(&buf[..len]);
    }
    assert_eq!(out, plain);
    assert_eq!(reader.read(&mut buf).expect("the end repeats"), 0);
}

#[test]
fn an_empty_buffer_reads_nothing_and_consumes_nothing() {
    let stream = compress(b"abc");
    let mut source = stream.as_slice();
    assert_eq!(Decoder::new(&mut source).read(&mut []).expect("no-op"), 0);
    assert_eq!(source, stream.as_slice());
}

#[test]
fn every_cut_and_every_flipped_bit_reads_as_decompress_to_writer_does() {
    // The reader and the slice path share their frame parsing, and this is
    // what keeps them agreeing on the bytes returned and the error kind.
    let (stream, _) = mixed_stream();
    for cut in 0..stream.len() {
        assert_eq!(
            drain(Decoder::new(&stream[..cut])),
            drain_to_writer(&stream[..cut]),
            "cut at {cut}"
        );
    }
    for bit in 0..stream.len() * 8 {
        let mut flipped = stream.clone();
        flipped[bit / 8] ^= 1 << (bit % 8);
        assert_eq!(
            drain(Decoder::new(flipped.as_slice())),
            drain_to_writer(&flipped),
            "bit {bit}"
        );
    }
}

#[test]
fn a_damaged_frame_returns_none_of_its_bytes() {
    let mut frame = compress(&b"the quick brown fox ".repeat(50));
    let last = frame.len() - 1;
    frame[last] ^= 1;
    assert_eq!(
        drain(Decoder::new(frame.as_slice())),
        (Vec::new(), Some(ErrorKind::InvalidData))
    );
}

#[test]
fn bytes_after_the_last_frame_fail_after_the_frames_before_them() {
    // Fewer bytes than a header read as the start of one more frame, cut
    // short; a header's worth that is not one is bad magic.
    for (tail, kind) in [
        (&b"junk"[..], ErrorKind::UnexpectedEof),
        (&b"junkjunkjunk"[..], ErrorKind::InvalidData),
    ] {
        let (mut stream, plain) = mixed_stream();
        stream.extend_from_slice(tail);
        assert_eq!(
            drain(Decoder::new(stream.as_slice())),
            (plain, Some(kind)),
            "tail {tail:?}"
        );
    }
}

#[test]
fn an_empty_source_is_truncated() {
    assert_eq!(
        drain(Decoder::new(&[][..])),
        (Vec::new(), Some(ErrorKind::UnexpectedEof))
    );
}

#[test]
fn a_failure_ends_the_stream_and_repeats() {
    let mut reader = Decoder::new(&b"MGDC"[..]);
    let mut buf = [0u8; 4];
    for _ in 0..3 {
        let err = reader.read(&mut buf).expect_err("a cut header fails");
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }
}

#[test]
fn a_source_error_comes_back_as_it_was() {
    struct Broken<'a>(&'a [u8]);
    impl Read for Broken<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match (self.0.split_first(), buf.first_mut()) {
                (Some((&byte, rest)), Some(slot)) => {
                    *slot = byte;
                    self.0 = rest;
                    Ok(1)
                }
                _ => Err(io::Error::from(ErrorKind::BrokenPipe)),
            }
        }
    }
    let frame = compress(b"abc");
    let source = Broken(&frame[..frame.len() - 1]);
    assert_eq!(
        drain(Decoder::new(source)),
        (Vec::new(), Some(ErrorKind::BrokenPipe))
    );
}

/// The `Read` contract lets a call with an empty buffer return `Ok(0)`,
/// which a decoder must not mistake for the end of the stream, so it asks
/// only with room to fill: this source panics on a call without it.
struct NeedsRoom<'a>(&'a [u8]);

impl Read for NeedsRoom<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert!(!buf.is_empty(), "read called with an empty buffer");
        let n = buf.len().min(self.0.len());
        buf[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        Ok(n)
    }
}

/// A source that fails each `read` with `Interrupted` before it succeeds,
/// the retry `Read` callers owe it.
struct Interrupting<'a> {
    rest: &'a [u8],
    pending: bool,
}

impl Read for Interrupting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.pending = !self.pending;
        if self.pending {
            return Err(io::Error::from(ErrorKind::Interrupted));
        }
        let n = buf.len().min(self.rest.len());
        buf[..n].copy_from_slice(&self.rest[..n]);
        self.rest = &self.rest[n..];
        Ok(n)
    }
}

#[test]
fn a_frame_filled_to_its_length_asks_the_source_for_nothing_more() {
    let stream = [
        compress(b"hi"),
        compress(&b"the quick brown fox ".repeat(50)),
    ]
    .concat();
    let (out, err) = drain(Decoder::new(NeedsRoom(&stream)));
    assert_eq!((out.len(), err), (2 + 20 * 50, None));
}

#[test]
fn an_interrupted_read_is_retried_not_returned() {
    let payload = b"the quick brown fox ".repeat(50);
    let stream = [compress(b"hi"), compress(&payload)].concat();
    let source = Interrupting {
        rest: &stream,
        pending: false,
    };
    // Not `read_to_end`: it retries `Interrupted` itself, and a decoder that
    // returned it would repeat it forever, a hang where a failure is meant.
    let mut reader = Decoder::new(source);
    let mut out = Vec::new();
    let mut buf = [0; 4096];
    loop {
        match reader
            .read(&mut buf)
            .expect("Interrupted is retried inside")
        {
            0 => break,
            n => out.extend_from_slice(&buf[..n]),
        }
    }
    assert_eq!(out, [b"hi".as_slice(), &payload].concat());
}

#[test]
fn max_len_rejects_a_frame_from_its_header_before_reading_its_payload() {
    let first = compress(b"hi");
    let second = compress(&b"the quick brown fox ".repeat(50));
    assert_eq!(second[METHOD_OFFSET], Method::Lz as u8);
    let stream = [first.as_slice(), &second].concat();
    let total = 2 + 20 * 50;
    let mut source = stream.as_slice();
    let (out, err) = drain(Decoder::with_max_len(&mut source, total - 1));
    assert_eq!(err, Some(ErrorKind::InvalidData));
    assert_eq!(out, b"hi", "the frame before the one over the bound");
    // The second frame's payload would not be admitted, so it stayed in
    // the source: only its header and the extent prefix were consumed.
    let consumed = stream.len() - source.len();
    assert_eq!(
        consumed,
        first.len() + HEADER_LEN + codec::EXTENT_PREFIX_LEN
    );
    let (out, err) = drain(Decoder::with_max_len(stream.as_slice(), total));
    assert_eq!((out.len(), err), (2 + 20 * 50, None));
}

#[test]
fn a_hostile_declared_length_is_truncated_not_read_into_memory() {
    let mut stored = compress(b"hi");
    stored[HEADER_LEN..HEADER_LEN + STORED_LEN_BYTES].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut lz = compress(&b"the quick brown fox ".repeat(50));
    forge_floor_extent(&mut lz, codec::MAX_DECODED_LEN);
    for frame in [stored, lz] {
        assert_eq!(
            drain(Decoder::new(frame.as_slice())),
            (Vec::new(), Some(ErrorKind::UnexpectedEof))
        );
    }
}

#[test]
fn an_lz_header_over_the_stored_floor_buffers_no_payload() {
    // #1137: a frame that declares 10 bytes of output and a huge coded length
    // is convicted by its header, so `with_max_len` holds the header and
    // not what the source would go on to send.
    let mut frame = compress(&b"the quick brown fox ".repeat(50));
    forge_floor_extent(&mut frame, 10);
    let coded_field = HEADER_LEN + codec::EXTENT_PREFIX_LEN - size_of::<u64>();
    frame[coded_field..coded_field + size_of::<u64>()].copy_from_slice(&(1u64 << 40).to_le_bytes());
    let sent = 400 << 20;
    let mut tail = io::repeat(0).take(sent);
    let reader = Decoder::with_max_len(frame.as_slice().chain(&mut tail), 1 << 20);
    assert_eq!(drain(reader), (Vec::new(), Some(ErrorKind::InvalidData)));
    assert!(sent - tail.limit() <= FILL_STEP as u64, "the tail was read");
}

#[test]
fn the_reader_agrees_with_the_slice_path_on_what_the_error_is() {
    let stream = compress(b"hi");
    let err = decompress_to_writer(&stream[..stream.len() - 1], u32::MAX, &mut Vec::new())
        .expect_err("a cut frame fails");
    assert!(matches!(err, WriteError::Decode(Error::Truncated)));
    assert_eq!(
        drain(Decoder::new(&stream[..stream.len() - 1])).1,
        Some(io::Error::from(err).kind())
    );
}

#[test]
fn a_bad_selector_is_corrupt_even_when_the_source_ends_after_it() {
    let mut cut = MAGIC.to_vec();
    cut.extend_from_slice(&[FORMAT_VERSION, Method::Lz as u8, 0, 1]);
    assert_eq!(
        drain(Decoder::new(cut.as_slice())),
        (Vec::new(), Some(ErrorKind::InvalidData))
    );
    assert_eq!(drain(Decoder::new(cut.as_slice())), drain_to_writer(&cut));
}

#[test]
fn a_streamed_lz_total_past_max_decoded_len_is_read_not_rejected() {
    // Pins `Decoder::with_max_len`'s call site of `Budget::new` (#1273),
    // which `only_a_collecting_budget_caps_the_lz_total` cannot: a reader
    // over the concatenation of large `Lz` outputs (ADR-0067) reads to the
    // end, as `decompress_to_writer` writes it (#1088).
    let stream = lz_total_past_max_decoded_len();
    let copied = io::copy(&mut Decoder::new(stream.as_slice()), &mut io::sink())
        .expect("the stream decodes to its end");
    assert!(copied > u64::from(codec::MAX_DECODED_LEN));
}
