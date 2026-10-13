//! Decompression from a [`std::io::Read`], the idiom the genre's crates
//! share (`flate2::read::GzDecoder`, `zstd::stream::read::Decoder`).

use std::io::{self, Read};

use super::{
    Budget, CHECK_LEN, Error, Frames, HEADER_LEN, Method, STORED_LEN_BYTES, codec, declared_extent,
    extent_prefix_len, parse_header,
};

/// The most [`Decoder::fill`] asks for beyond the bytes the source has
/// delivered: a hostile length field costs a multiple of the input that
/// actually arrives, never what it declares.
const FILL_STEP: usize = 64 * 1024;

/// Where the decoder stands in the stream it reads.
#[derive(Clone, Copy)]
enum State {
    /// No frame read yet: end of input here is [`Error::Truncated`], because
    /// a stream is one or more frames (ADR-0067).
    Start,
    /// At a frame boundary: end of input here is the end of the stream.
    Between,
    /// The stream ended cleanly.
    Done,
    /// A read failed with this kind and every later read fails with it.
    Failed(io::ErrorKind),
}

/// Reads a stream of frames from `R` and yields the bytes they decode to,
/// the streaming counterpart of [`decompress_bounded`](crate::decompress_bounded).
///
/// A stream is any number of frames back to back (`docs/format/SPEC.md`,
/// "Streams"), and its end is the end of `R`. Bytes that are not a frame
/// after the last one are an error, never dropped.
///
/// The decoder holds one frame at a time. It reads the frame whole and
/// verifies its check value before decoding any of it, so no byte of a
/// damaged frame is ever returned: that is the property
/// [`Error::BadChecksum`] promises, and it is why the decoder is incremental
/// across frames and not within one. The buffers are the frame's coded bytes
/// and the bytes it decodes to, each up to twice its size because a growing
/// buffer doubles its capacity, and both stay allocated until a larger frame
/// replaces them. [`compress`](crate::compress) and
/// [`copy_encode`](crate::copy_encode) cut their input into frames of 16 MiB,
/// so a stream they wrote holds at most 32 MiB per buffer; a frame a
/// producer wrote longer is held as long as it is. A [`Method::Stored`]
/// frame from [`compress`](crate::compress) of an input past
/// [`u32::MAX`] bytes is one such frame. Wrap `R` in [`Read::take`] to bound
/// how much of a source the decoder may consume.
///
/// Every earlier frame's bytes are returned before a later frame's error, so
/// a stream cut or damaged partway reads as its good prefix and then fails.
/// A failure ends the stream: the failing read returns the error, with its
/// [`std::io::ErrorKind`] as [`Error`] converts it (`UnexpectedEof` for
/// truncation, `InvalidData` for corruption), and every later read returns
/// that kind again. That includes a source error of kind
/// [`WouldBlock`](io::ErrorKind::WouldBlock): the partial frame is dropped,
/// so a non-blocking source cannot be resumed.
///
/// ```
/// use std::io::Read;
///
/// let original = b"the quick brown fox jumps over the lazy dog".repeat(100);
/// let stream = mothergod::compress(&original);
/// let mut decoded = Vec::new();
/// mothergod::read::Decoder::new(stream.as_slice()).read_to_end(&mut decoded)?;
/// assert_eq!(decoded, original);
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct Decoder<R> {
    source: R,
    budget: Budget,
    /// The frame being read, header first. Scratch between frames.
    frame: Vec<u8>,
    /// Decoded bytes of the current frame, of which `out[pos..end]` is
    /// unread. A [`Method::Stored`] frame is served from its own bytes, so
    /// this holds the frame and `frame` the buffer it replaced.
    out: Vec<u8>,
    pos: usize,
    end: usize,
    state: State,
}

impl<R: Read> Decoder<R> {
    /// A decoder over `source`, bounded as [`decompress`](crate::decompress)
    /// is: no `Lz` frame decodes to more than [`codec::MAX_DECODED_LEN`].
    pub fn new(source: R) -> Self {
        Self::with_max_len(source, codec::MAX_DECODED_LEN)
    }

    /// A decoder that fails with [`Error::TooLarge`] once the stream's output
    /// would pass `max_len` bytes, as
    /// [`decompress_to_writer`](crate::decompress_to_writer) does for its
    /// `max_len`: checked from each frame's header, before the frame's
    /// payload is read or any decode work is done. A `max_len` at or above
    /// [`codec::MAX_DECODED_LEN`] caps no total, only each `Lz` frame.
    pub fn with_max_len(source: R, max_len: u32) -> Self {
        Self {
            source,
            budget: Budget::new(max_len, false),
            frame: Vec::new(),
            out: Vec::new(),
            pos: 0,
            end: 0,
            state: State::Start,
        }
    }

    /// Grows `self.frame` toward `len` bytes from the source. It holds fewer
    /// afterwards only if the source ended first. Memory follows the bytes
    /// that arrive, never a declared length: each step asks for at most
    /// [`FILL_STEP`] bytes beyond what the source has delivered, and
    /// `try_reserve` grows amortized, so capacity stays under twice the
    /// bytes delivered plus [`FILL_STEP`]. `try_reserve_exact` would hold the
    /// bytes delivered exactly but reallocate, and so copy, at every step.
    ///
    /// [`Read::read_to_end`] would be shorter, but it aborts the process on
    /// a failed allocation where this returns [`Error::OutOfMemory`] (hard
    /// rule 2; `tests/torture.rs`).
    fn fill(&mut self, len: usize) -> io::Result<()> {
        while self.frame.len() < len {
            let have = self.frame.len();
            let step = (len - have).min(FILL_STEP);
            self.frame.try_reserve(step).map_err(Error::from)?;
            // Within the reserved capacity: `resize` allocates nothing.
            self.frame.resize(have + step, 0);
            let read = loop {
                match self.source.read(&mut self.frame[have..]) {
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
                    result => break result,
                }
            };
            let read = read.inspect_err(|_| self.frame.truncate(have))?;
            self.frame.truncate(have + read);
            if read == 0 {
                break;
            }
        }
        Ok(())
    }

    /// Reads and decodes the next frame into `out`, or reports the end of
    /// the stream as `false`.
    fn read_frame(&mut self) -> io::Result<bool> {
        self.frame.clear();
        self.fill(HEADER_LEN)?;
        if self.frame.is_empty() && matches!(self.state, State::Between) {
            return Ok(false);
        }
        let header = self.frame.first_chunk().ok_or(Error::Truncated)?;
        let method = parse_header(*header)?;

        let prefix_end = HEADER_LEN + extent_prefix_len(method);
        self.fill(prefix_end)?;
        let prefix = self.frame.get(HEADER_LEN..).ok_or(Error::Truncated)?;
        let (payload_len, output_len) = declared_extent(method, prefix)?;
        self.budget.admit(method, output_len)?;

        let total = payload_len
            .checked_add(HEADER_LEN + CHECK_LEN)
            .ok_or(Error::Truncated)?;
        self.fill(total)?;
        let frame = Frames::new(&self.frame).next().ok_or(Error::Truncated)??;
        match frame.method {
            Method::Lz => {
                self.out = codec::decode(frame.payload, codec::MAX_DECODED_LEN)?;
                self.pos = 0;
                self.end = self.out.len();
            }
            Method::Stored => {
                std::mem::swap(&mut self.frame, &mut self.out);
                self.pos = HEADER_LEN + STORED_LEN_BYTES;
                self.end = HEADER_LEN + payload_len;
            }
        }
        Ok(true)
    }
}

impl<R: Read> Read for Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // A frame may decode to nothing, so a read waits on the next one.
        while self.pos == self.end {
            match self.state {
                State::Done => return Ok(0),
                State::Failed(kind) => return Err(kind.into()),
                State::Start | State::Between => match self.read_frame() {
                    Ok(true) => self.state = State::Between,
                    Ok(false) => self.state = State::Done,
                    Err(err) => {
                        self.state = State::Failed(err.kind());
                        return Err(err);
                    }
                },
            }
        }
        let len = buf.len().min(self.end - self.pos);
        buf[..len].copy_from_slice(&self.out[self.pos..self.pos + len]);
        self.pos += len;
        Ok(len)
    }
}

#[cfg(test)]
mod tests;
