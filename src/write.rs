//! Compression into a [`std::io::Write`], the idiom the genre's crates
//! share (`flate2::write::GzEncoder`, `zstd::stream::write::Encoder`).

use std::collections::VecDeque;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use super::{BLOCK_LEN, compress_block};

/// Encodes of one block of input that run at once at most, whether the block
/// is one of a stream's or one of the trials of a block's filter search:
/// two, because an encode's working set on real data is about 509 MiB
/// (`mozilla`, 520,928 KiB serially) and the bound the CLI and the library
/// state is 1 GiB, which holds two and not three (`research/JOURNAL.md`
/// S2-A125). A machine with one core
/// encodes serially, in the calling thread, as before.
const MAX_CONCURRENT_ENCODES: usize = 2;

/// The encodes of a block that run at once on this machine: its cores, up to
/// [`MAX_CONCURRENT_ENCODES`]. A stream's blocks and one block's trials
/// share the bound and never nest: the last block of a stream, alone, spends
/// it on trials.
pub(crate) fn concurrent_encodes() -> NonZeroUsize {
    thread::available_parallelism().map_or(NonZeroUsize::MIN, |cores| {
        cores.min(NonZeroUsize::new(MAX_CONCURRENT_ENCODES).expect("two is nonzero"))
    })
}

/// A full block on its way to a frame, in the order the blocks were cut.
type InFlight = JoinHandle<Vec<u8>>;

/// Compresses what is written to it into `W` as a stream of frames, the
/// streaming counterpart of [`compress`](crate::compress).
///
/// Bytes collect in a block of 16 MiB. A full block becomes one frame
/// written to `W`, so memory follows the blocks in flight and not the total
/// written; the buffer is up to twice the block in capacity because a growing
/// `Vec` doubles, and the compressor's working set comes on top of it. That
/// working set, not the block, sets the peak: a stream of 256 MiB of zeros
/// encodes in under 1 GiB, a bound `.github/scripts/memory-check` asserts
/// weekly ([`copy_encode`](crate::copy_encode) has the same bound and is
/// built on this type). Nothing reaches `W` until a block fills, `flush` is called
/// or [`finish`](Self::finish) runs, and a full block's frame waits for its
/// encode. Written without a `flush`, the stream
/// is byte for byte what [`copy_encode`](crate::copy_encode) writes for the
/// same input, however the writes were cut.
///
/// On a machine with two or more cores, two blocks encode at once, each on
/// a thread of its own while the caller fills the next, so a stream of
/// several blocks takes less wall-clock time and holds two working sets
/// instead of one: the 1 GiB bound is for the pair. The last block, when
/// none is running, spends the two threads on its filter search instead, so a
/// stream of one block takes less wall-clock time at the same bound. Frames
/// reach `W` in block order and are the bytes a serial encode writes. A full block
/// whose predecessors are still encoding makes the `write` that filled it
/// wait for the oldest; a sink error from that frame comes back from that
/// call, one block later than a serial encoder would report it, and the
/// sink still holds exactly the frames before it. An encoder dropped
/// mid-stream leaves its running threads to finish and discard their
/// frames.
///
/// [`finish`](Self::finish) writes the last frame and must be called: an
/// encoder dropped without it writes nothing, and the stream it leaves
/// lacks the bytes of its last, partial block and of every full block still
/// encoding. A drop cannot report an I/O error, which is why none is
/// attempted.
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
    /// Blocks encoding on their own threads, oldest first; never more than
    /// `workers`. Empty when `workers` is one.
    in_flight: VecDeque<InFlight>,
    workers: NonZeroUsize,
    /// A stream is one or more frames (ADR-0067): an encoder that cut no
    /// block still owes one, for the empty input. Set when a block is cut,
    /// not when its frame reaches the sink, which a running block has not.
    cut_a_block: bool,
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
        Self::with_workers(sink, block_len, concurrent_encodes())
    }

    /// [`with_block_len`](Self::with_block_len) with the blocks encoded at
    /// once a parameter, so a test compares one worker against several.
    pub(crate) fn with_workers(sink: W, block_len: usize, workers: NonZeroUsize) -> Self {
        Self {
            sink,
            block: Vec::new(),
            block_len,
            in_flight: VecDeque::new(),
            workers,
            cut_a_block: false,
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
        if !self.block.is_empty() || !self.cut_a_block {
            self.emit(true)?;
        }
        self.drain()?;
        Ok(self.sink)
    }

    /// The error that ended the encoder, if one did.
    fn check(&self) -> io::Result<()> {
        self.failed.map_or(Ok(()), |kind| Err(kind.into()))
    }

    /// Compresses the buffered bytes into one frame, on a thread of its own
    /// when `workers` allows more than one block at once.
    ///
    /// Waits for the oldest running block first when `workers` are already
    /// running: the invariant behind [`MAX_CONCURRENT_ENCODES`]'s memory
    /// figure. A `last` block with none running is the only encode there is
    /// and will be, so it runs here and spends all `workers` on its filter
    /// trials instead (`codec::encode`); any other block's trials run
    /// serially, because a block cut after it would encode beside them and
    /// make three.
    fn emit(&mut self, last: bool) -> io::Result<()> {
        self.cut_a_block = true;
        if self.workers == NonZeroUsize::MIN || (last && self.in_flight.is_empty()) {
            let frame = compress_block(&self.block, self.workers);
            self.block.clear();
            return self.write_frame(&frame);
        }
        while self.in_flight.len() >= self.workers.get() {
            self.write_oldest()?;
        }
        // The thread and this call share the block, so a refused spawn
        // (the process is out of threads) falls back to encoding here.
        let block = Arc::new(std::mem::take(&mut self.block));
        let shared = Arc::clone(&block);
        let spawned =
            thread::Builder::new().spawn(move || compress_block(&shared, NonZeroUsize::MIN));
        if let Ok(handle) = spawned {
            self.in_flight.push_back(handle);
        } else {
            let frame = compress_block(&block, NonZeroUsize::MIN);
            self.drain()?;
            self.write_frame(&frame)?;
        }
        Ok(())
    }

    /// Joins the oldest running block and writes its frame.
    fn write_oldest(&mut self) -> io::Result<()> {
        let Some(handle) = self.in_flight.pop_front() else {
            return Ok(());
        };
        // A panic on the encoding thread is the panic a serial encode raises.
        let frame = handle
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        self.write_frame(&frame)
    }

    /// Writes every running block's frame, oldest first.
    fn drain(&mut self) -> io::Result<()> {
        while !self.in_flight.is_empty() {
            self.write_oldest()?;
        }
        Ok(())
    }

    /// Writes one frame to the sink; a failure ends the encoder.
    fn write_frame(&mut self, frame: &[u8]) -> io::Result<()> {
        self.sink
            .write_all(frame)
            .inspect_err(|err| self.failed = Some(err.kind()))
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
            self.emit(false)?;
        }
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.check()?;
        if !self.block.is_empty() {
            self.emit(false)?;
        }
        self.drain()?;
        self.sink.flush()
    }
}

#[cfg(test)]
mod tests;
