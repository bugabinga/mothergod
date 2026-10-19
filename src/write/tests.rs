use std::io::ErrorKind;

use super::*;
use crate::test_support::{
    Xorshift32, bcj_call_dense_data, columnar_drift_data, repeated_sentence, serial_blocks,
    tabular_columns_data,
};
use crate::{compress, decompress};

const BLOCK: usize = 4096;

/// A sink that fails its first `failures` calls to `write` with `kind` and
/// accepts every one after; what it accepted is in `bytes`.
struct Flaky {
    bytes: Vec<u8>,
    failures: usize,
    kind: ErrorKind,
}

impl Write for &mut Flaky {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.failures > 0 {
            self.failures -= 1;
            return Err(self.kind.into());
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// What `write_all` of `input` in pieces of `chunk` bytes, then `finish`,
/// leaves in the sink, with blocks of `block_len`.
fn encode(input: &[u8], chunk: usize, block_len: usize) -> Vec<u8> {
    let mut encoder = Encoder::with_block_len(Vec::new(), block_len);
    for piece in input.chunks(chunk) {
        encoder.write_all(piece).expect("a Vec does not fail");
    }
    encoder.finish().expect("a Vec does not fail")
}

/// [`encode`] with `workers` blocks encoding at once.
fn encode_with(input: &[u8], block_len: usize, workers: usize) -> Vec<u8> {
    let mut encoder = Encoder::with_workers(Vec::new(), block_len, workers);
    encoder.write_all(input).expect("a Vec does not fail");
    encoder.finish().expect("a Vec does not fail")
}

/// Blocks of runs and noise, so the frames differ in method, size and time:
/// a pipeline that wrote them in the order they finished, not the order they
/// were cut, would not agree with the serial encode.
fn uneven_blocks(blocks: usize) -> Vec<u8> {
    let mut noise = Xorshift32::new(11).map(|word| word.to_le_bytes()[1]);
    let mut input = Vec::new();
    for index in 0..blocks {
        match index % 3 {
            0 => input.extend(std::iter::repeat_n(b'a', BLOCK)),
            1 => input.extend(noise.by_ref().take(BLOCK)),
            _ => input.extend(repeated_sentence().into_iter().cycle().take(BLOCK)),
        }
    }
    input
}

#[test]
fn the_workers_in_flight_never_change_the_bytes() {
    for blocks in [0, 1, 2, 3, 5, 8] {
        let input = uneven_blocks(blocks);
        let serial = serial_blocks(&input, BLOCK);
        for workers in [1, 2, 3, 4] {
            // Two blocks in flight and none written when the input ends on
            // a block edge: `finish` must add no empty frame.
            assert_eq!(
                encode_with(&input, BLOCK, workers),
                serial,
                "{blocks} blocks, {workers} workers"
            );
        }
        assert_eq!(decompress(&serial), Ok(input));
    }
}

#[test]
fn a_flush_waits_for_every_running_block_and_the_stream_continues_in_order() {
    let (first, second) = (uneven_blocks(3), uneven_blocks(2));
    for workers in [1, 2, 3] {
        let mut encoder = Encoder::with_workers(Vec::new(), BLOCK, workers);
        encoder.write_all(&first).unwrap();
        encoder.flush().unwrap();
        // Everything written so far has reached the sink, running blocks too.
        assert_eq!(
            encoder.sink,
            serial_blocks(&first, BLOCK),
            "{workers} workers"
        );
        encoder.write_all(&second).unwrap();
        let stream = encoder.finish().unwrap();
        assert_eq!(
            stream,
            [serial_blocks(&first, BLOCK), serial_blocks(&second, BLOCK)].concat(),
            "{workers} workers"
        );
    }
}

#[test]
fn every_method_and_filter_class_writes_the_bytes_compress_returns() {
    let classes: [(&str, Vec<u8>); 6] = [
        ("stored", b"hi".to_vec()),
        (
            "identity",
            include_bytes!("../../research/imports/session-1/mothergod.rs").to_vec(),
        ),
        ("delta", repeated_sentence()),
        ("reverse-delta", columnar_drift_data()),
        ("bcj", bcj_call_dense_data()),
        ("transpose", tabular_columns_data(8, 2000, 1)),
    ];
    for (name, data) in classes {
        let stream = encode(&data, usize::MAX, BLOCK_LEN);
        assert_eq!(stream, compress(&data), "{name}");
        assert_eq!(decompress(&stream), Ok(data), "{name}");
    }
}

#[test]
fn the_stream_is_the_serial_encode_whatever_the_write_sizes_and_the_block_edges() {
    let noise: Vec<u8> = Xorshift32::new(7)
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
        for chunk in [1, 1000, BLOCK, usize::MAX] {
            assert_eq!(
                encode(&input, chunk, BLOCK),
                serial_blocks(&input, BLOCK),
                "input of {} bytes written {chunk} at a time",
                input.len()
            );
        }
    }
}

#[test]
fn an_encoder_written_nothing_finishes_to_the_empty_frame() {
    let stream = Encoder::new(Vec::new()).finish().expect("a Vec");
    assert_eq!(stream, compress(b""));
    assert_eq!(decompress(&stream), Ok(Vec::new()));
}

#[test]
fn a_drop_writes_no_frame_and_a_full_block_is_framed_at_once_when_nothing_runs_beside_it() {
    let mut sink = Flaky {
        bytes: Vec::new(),
        failures: 0,
        kind: ErrorKind::Other,
    };
    let mut encoder = Encoder::with_block_len(&mut sink, BLOCK);
    encoder.write_all(&vec![b'a'; BLOCK - 1]).unwrap();
    drop(encoder);
    assert!(sink.bytes.is_empty(), "a drop writes no frame");

    let mut encoder = Encoder::with_workers(&mut sink, BLOCK, 1);
    encoder.write_all(&vec![b'a'; BLOCK]).unwrap();
    drop(encoder);
    assert_eq!(sink.bytes, compress(&vec![b'a'; BLOCK]), "a full block");
}

#[test]
fn flush_frames_the_buffered_bytes_and_a_flush_of_nothing_writes_nothing() {
    let mut encoder = Encoder::new(Vec::new());
    encoder.flush().unwrap();
    encoder.write_all(b"first").unwrap();
    encoder.flush().unwrap();
    encoder.flush().unwrap();
    encoder.write_all(b"second").unwrap();
    let stream = encoder.finish().unwrap();
    assert_eq!(
        stream,
        [compress(b"first"), compress(b"second")].concat(),
        "one frame per flushed run, none for the flushes of nothing"
    );
    assert_eq!(decompress(&stream), Ok(b"firstsecond".to_vec()));
}

#[test]
fn a_flush_after_the_last_byte_leaves_finish_nothing_to_add() {
    let mut encoder = Encoder::new(Vec::new());
    encoder.write_all(b"only").unwrap();
    encoder.flush().unwrap();
    assert_eq!(encoder.finish().unwrap(), compress(b"only"));
}

#[test]
fn a_sink_error_ends_the_encoder_and_no_later_byte_reaches_the_sink() {
    // Only the first call fails, so a later frame the encoder wrote anyway
    // would land in `bytes`.
    let mut sink = Flaky {
        bytes: Vec::new(),
        failures: 1,
        kind: ErrorKind::BrokenPipe,
    };
    let mut encoder = Encoder::with_workers(&mut sink, BLOCK, 1);
    let err = encoder.write_all(&vec![b'a'; BLOCK]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    for result in [encoder.write(b"x").map(drop), encoder.flush()] {
        assert_eq!(result.unwrap_err().kind(), ErrorKind::BrokenPipe);
    }
    let err = encoder.finish().map(drop).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    assert!(sink.bytes.is_empty(), "nothing after the failure");
}

#[test]
fn a_sink_error_ends_a_threaded_encoder_with_the_frames_before_it_intact() {
    for workers in [1, 2, 3] {
        let mut sink = Flaky {
            bytes: Vec::new(),
            failures: 1,
            kind: ErrorKind::BrokenPipe,
        };
        let mut encoder = Encoder::with_workers(&mut sink, BLOCK, workers);
        // Six blocks: the first frame's write fails whichever call reaches it.
        let wrote = encoder.write_all(&uneven_blocks(6));
        let err = wrote
            .and_then(|()| encoder.finish().map(drop))
            .expect_err("the first frame fails");
        assert_eq!(err.kind(), ErrorKind::BrokenPipe, "{workers} workers");
        assert!(sink.bytes.is_empty(), "{workers} workers: nothing after it");
    }
}

#[test]
fn a_sink_error_from_finish_or_flush_is_returned_not_swallowed() {
    for flush in [false, true] {
        let mut sink = Flaky {
            bytes: Vec::new(),
            failures: usize::MAX,
            kind: ErrorKind::PermissionDenied,
        };
        let mut encoder = Encoder::new(&mut sink);
        encoder.write_all(b"partial block").unwrap();
        let err = if flush {
            encoder.flush().unwrap_err()
        } else {
            encoder.finish().map(drop).unwrap_err()
        };
        assert_eq!(err.kind(), ErrorKind::PermissionDenied, "flush: {flush}");
    }
}

#[test]
fn write_takes_at_most_the_room_left_in_the_block() {
    let mut encoder = Encoder::with_block_len(Vec::new(), 8);
    assert_eq!(encoder.write(b"abcdef").unwrap(), 6);
    assert_eq!(encoder.write(b"ghijkl").unwrap(), 2);
    assert_eq!(encoder.write(b"ijkl").unwrap(), 4);
    assert_eq!(encoder.write(b"").unwrap(), 0);
}
