//! Compression into a [`std::io::Write`], the idiom the genre's crates
//! share (`flate2::write::GzEncoder`, `zstd::stream::write::Encoder`).

use std::io::{self, Write};

use super::{BLOCK_LEN, compress_block};

/// Compresses what is written to it into `W` as a stream of frames, the
/// streaming counterpart of [`compress`](crate::compress).
///
/// Bytes collect in a block of 16 MiB. A full block becomes one frame
/// written to `W`, so memory follows the block and not the total written;
/// the buffer is up to twice the block in capacity because a growing `Vec`
/// doubles, and the compressor's working set comes on top of it. That
/// working set, not the block, sets the peak: a stream of 256 MiB of zeros
/// encodes in under 1 GiB, a bound `.github/scripts/memory-check` asserts
/// weekly ([`copy_encode`](crate::copy_encode) has the same bound and is
/// built on this type). Nothing reaches `W` until a block fills, `flush` is called
/// or [`finish`](Self::finish) runs. Written without a `flush`, the stream
/// is byte for byte what [`copy_encode`](crate::copy_encode) writes for the
/// same input, however the writes were cut.
///
/// [`finish`](Self::finish) writes the last frame and must be called: an
/// encoder dropped without it writes nothing, and the stream it leaves
/// lacks the bytes of its last, partial block. A drop cannot report an I/O
/// error, which is why none is attempted.
///
/// [`flush`](Write::flush) writes the buffered bytes as a frame early, then
/// flushes `W`, as `Write::flush` asks. Each early frame restarts the
/// models, so a caller that flushes after every small write pays
/// `docs/format/SPEC.md`'s frame overhead and the ratio of tiny inputs on
/// each one. The stream stays valid.
///
/// A write error from `W` ends the encoder: the failing call returns it, and
/// every later call returns an error of the same [`io::ErrorKind`]. A
/// block whose frame failed partway cannot be written again without
/// duplicating or skipping bytes, so continuing would leave a stream that
/// decodes to something other than what was written.
///
/// ```
/// use std::io::Write;
///
/// let original = b"the quick brown fox jumps over the lazy dog".repeat(100);
/// let mut encoder = mothergod::write::Encoder::new(Vec::new());
/// encoder.write_all(&original)?;
/// let stream = encoder.finish()?;
/// assert_eq!(mothergod::decompress(&stream), Ok(original));
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct Encoder<W> {
    sink: W,
    /// Bytes written and not yet framed; fewer than `block_len` between calls.
    block: Vec<u8>,
    block_len: usize,
    /// A stream is one or more frames (ADR-0067): an encoder that wrote
    /// nothing still owes one, for the empty input.
    wrote_a_frame: bool,
    /// The kind of the sink error that ended the encoder, if one did.
    failed: Option<io::ErrorKind>,
}

impl<W: Write> Encoder<W> {
    /// An encoder that writes its frames to `sink`.
    pub fn new(sink: W) -> Self {
        Self::with_block_len(sink, BLOCK_LEN)
    }

    /// [`new`](Self::new) with the block length a parameter, so a test cuts
    /// at a length it can afford. `block_len` is nonzero.
    pub(crate) fn with_block_len(sink: W, block_len: usize) -> Self {
        Self {
            sink,
            block: Vec::new(),
            block_len,
            wrote_a_frame: false,
            failed: None,
        }
    }

    /// Writes the last frame and returns the sink, which is not flushed.
    /// An encoder that was written nothing since its last frame writes none,
    /// unless it has written no frame at all, and then one frame of the
    /// empty input, which a stream must hold.
    ///
    /// # Errors
    ///
    /// The error that ended the encoder, or the sink's error from writing
    /// the last frame. The sink is dropped with it.
    pub fn finish(mut self) -> io::Result<W> {
        self.check()?;
        if !self.block.is_empty() || !self.wrote_a_frame {
            self.emit()?;
        }
        Ok(self.sink)
    }

    /// The error that ended the encoder, if one did.
    fn check(&self) -> io::Result<()> {
        self.failed.map_or(Ok(()), |kind| Err(kind.into()))
    }

    /// Compresses the buffered bytes into one frame and writes it.
    fn emit(&mut self) -> io::Result<()> {
        let frame = compress_block(&self.block);
        self.block.clear();
        self.sink
            .write_all(&frame)
            .inspect_err(|err| self.failed = Some(err.kind()))?;
        self.wrote_a_frame = true;
        Ok(())
    }
}

impl<W: Write> Write for Encoder<W> {
    /// Takes bytes up to the end of the current block, so it may take fewer
    /// than `buf` holds; `write_all` loops.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.check()?;
        let take = buf.len().min(self.block_len - self.block.len());
        self.block.extend_from_slice(&buf[..take]);
        if self.block.len() == self.block_len {
            self.emit()?;
        }
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.check()?;
        if !self.block.is_empty() {
            self.emit()?;
        }
        self.sink.flush()
    }
}

#[cfg(test)]
mod tests;
