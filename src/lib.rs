#![doc(
    html_logo_url = "https://raw.githubusercontent.com/bugabinga/mothergod/main/assets/logo.svg"
)]
#![forbid(unsafe_code)]
//! mothergod: general purpose compression.
//!
//! The library speaks a tiny framed container format. Every frame starts
//! with a magic number, a format version, and a method byte identifying
//! how the payload is encoded: `Stored` (no compression) or `Lz`
//! (optimal-parse LZ over an adaptive range coder, `research/JOURNAL.md`
//! S2-D2). [`compress`] always picks whichever produces the smaller frame.
//!
//! ```
//! let original = b"the quick brown fox jumps over the lazy dog".repeat(100);
//! let frame = mothergod::compress(&original);
//! assert!(frame.len() < original.len());
//! assert_eq!(mothergod::decompress(&frame), Ok(original));
//! ```

// Every module but `filters`, `read` and `write` is `#[doc(hidden)]`: the engine's
// internals, not a surface downstream crates are meant to call (ROADMAP M6).
// They stay `pub` for two reasons. `bench` and `fuzz` call into `lz`, `literal`
// and `codec`. And the live codec path does not reach the research
// primitives (`Model::ideal_cost_bits`, `codec`'s `compressed_len_*`
// and `ideal_cost_bits_*` experiment entry points, `Sse::contexts`), whose
// callers are tests: under `pub(crate)` they become `dead_code` errors with
// `-D warnings`. Narrowing waits on wiring or removing those items. `filters` is
// documented: S2-A2 judged it "a defensible standalone library surface on
// their own merits" (`research/JOURNAL.md`). `read` and `write` are the
// library idiom matrix's front page (ADR-0064, #979).
#[doc(hidden)]
pub mod bittree;
#[doc(hidden)]
pub mod codec;
#[doc(hidden)]
pub mod coder;
#[doc(hidden)]
pub mod column;
#[doc(hidden)]
pub mod crc32;
pub mod filters;
#[doc(hidden)]
pub mod literal;
#[doc(hidden)]
pub mod logistic;
#[doc(hidden)]
pub mod lz;
#[doc(hidden)]
pub mod model;
pub mod read;
#[doc(hidden)]
pub mod sse;
pub mod write;

/// First bytes of every mothergod frame.
pub const MAGIC: [u8; 4] = *b"MGDC";

/// Container format version written into frames produced by this crate.
///
/// Every bump is a bitstream format change (CLAUDE.md hard rule 5); the
/// history of what each version changed lives in `docs/format/SPEC.md` and
/// the ADRs it cites. This build decodes exactly this version for both
/// methods: versions 1 through 15 were retired outright, no release having
/// written any of them
/// (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`,
/// `docs/adr/0063-retire-format-versions-1-through-9.md`), and a frame
/// naming any other version is rejected as `UnsupportedVersion` before its
/// payload is read (`tests/golden/` pins the current version's
/// frames).
pub const FORMAT_VERSION: u8 = 17;

/// Payload encoding methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Method {
    /// Payload is a `u64` declared length, then the data verbatim, no
    /// compression.
    Stored = 0,
    /// Optimal-parse LZ tokens, entropy-coded by adaptive flag/length/
    /// offset/rep-slot models and a six-expert context-mixing literal
    /// model, over an adaptive range coder, behind whichever filter
    /// (delta, BCJ, transpose, or none) trial-selection found smallest.
    /// See [`codec`] for the payload layout.
    Lz = 1,
}

impl TryFrom<u8> for Method {
    type Error = Error;

    fn try_from(byte: u8) -> Result<Self, Error> {
        match byte {
            0 => Ok(Self::Stored),
            1 => Ok(Self::Lz),
            other => Err(Error::UnknownMethod(other)),
        }
    }
}

/// Errors produced when decoding a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Input ended before the frame was complete: inside the header, short
    /// of the length a [`Method::Stored`] payload or a [`Method::Lz`] coded
    /// stream declares, or short of the check value that closes the frame.
    /// Bytes after the last frame of a stream read as the start of one more,
    /// so they are this or [`Error::BadMagic`].
    Truncated,
    /// Input does not start with [`MAGIC`].
    BadMagic,
    /// Frame was written by a newer, incompatible format version.
    UnsupportedVersion(u8),
    /// Method byte does not name a known [`Method`].
    UnknownMethod(u8),
    /// Payload does not decode to a value consistent with itself (a
    /// declared length its content does not match, a match/rep distance
    /// reaching before the start of decoded output, or similar):
    /// adversarial or corrupted input, never a bug in this decoder.
    Corrupt,
    /// The frame's check value does not match its bytes: a bit flipped, or
    /// a byte changed, after [`compress`] wrote it. Reported before any of
    /// the payload is decoded, so a damaged frame writes nothing.
    BadChecksum,
    /// Payload's declared ([`Method::Lz`]) or actual ([`Method::Stored`])
    /// output length exceeds the bound in effect: [`codec::MAX_DECODED_LEN`]
    /// under [`decompress`], or a caller's own tighter `max_len` under
    /// [`decompress_bounded`]. `max` names whichever bound was actually
    /// violated, since the two can differ. A declared length alone is not
    /// bounded by the bytes that encode it: this format's adaptive models
    /// can make a handful of real payload bytes and a few million
    /// padding-decoded bytes indistinguishable by size, so the only sound
    /// bound is an explicit ceiling, checked before any allocation or
    /// decode work happens (`rust-craft` skill, allocation-discipline).
    TooLarge {
        /// The length that exceeded the bound.
        len: u32,
        /// The bound it exceeded (not always [`codec::MAX_DECODED_LEN`];
        /// see the variant's docs).
        max: u32,
    },
    /// The allocator could not satisfy an allocation partway through
    /// decode: the output buffer (`declared_len` bytes, already checked
    /// against the effective bound), a model's frequency table, or a
    /// filter's undo buffer. Hard rule 2 (`CLAUDE.md`) carves out no
    /// exception for allocator failure: each of those goes through a
    /// fallible reserve so this is an `Err`, not an abort
    /// (`tests/torture.rs`).
    OutOfMemory,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => write!(f, "input ended before the frame was complete"),
            Self::BadMagic => write!(f, "input is not a mothergod frame (bad magic)"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            Self::UnknownMethod(m) => write!(f, "unknown compression method {m}"),
            Self::Corrupt => write!(f, "compressed payload is corrupt"),
            Self::BadChecksum => write!(f, "frame check value does not match its bytes"),
            Self::TooLarge { len, max } => {
                write!(
                    f,
                    "output length {len} exceeds the decoder's bound ({max} bytes)"
                )
            }
            Self::OutOfMemory => write!(f, "allocator could not satisfy a decode allocation"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Error> for std::io::Error {
    /// The [`std::io::ErrorKind`] each variant names, and nothing else:
    /// `io::Error::from(kind)` allocates no payload, where wrapping `err`
    /// through `io::Error::new` would box it and could abort under
    /// allocator pressure, the failure [`WriteError`] exists to avoid
    /// (issue #479). A caller that needs the variant matches on [`Error`]
    /// before converting.
    ///
    /// [`Error::Truncated`] is `UnexpectedEof`, [`Error::OutOfMemory`] is
    /// `OutOfMemory`, and every other variant is `InvalidData`, because the
    /// bytes are what is wrong with them.
    fn from(err: Error) -> Self {
        Self::from(match err {
            Error::Truncated => std::io::ErrorKind::UnexpectedEof,
            Error::OutOfMemory => std::io::ErrorKind::OutOfMemory,
            Error::BadMagic
            | Error::UnsupportedVersion(_)
            | Error::UnknownMethod(_)
            | Error::Corrupt
            | Error::BadChecksum
            | Error::TooLarge { .. } => std::io::ErrorKind::InvalidData,
        })
    }
}

impl From<std::collections::TryReserveError> for Error {
    /// Every fallible allocation on the decode path fails as
    /// [`Error::OutOfMemory`], never a panic (hard rule 2).
    fn from(_: std::collections::TryReserveError) -> Self {
        Self::OutOfMemory
    }
}

/// Error from [`decompress_to_writer`]: either the frame failed to decode
/// ([`Error`], same variants and meaning as [`decompress`]'s) or `writer`
/// itself failed.
///
/// Two owned variants and not a single [`std::io::Error`]: wrapping a decode
/// error through `std::io::Error::other` boxes it twice with no fallible
/// sibling on stable Rust, so *every* decode error, `Truncated` on a two-byte
/// input included, could abort the process under allocator pressure, against
/// hard rule 2 (issue #479). Plain enum construction allocates nothing.
#[derive(Debug)]
pub enum WriteError {
    /// The frame did not decode; see [`Error`] for what each variant means.
    Decode(Error),
    /// `writer` itself failed.
    Io(std::io::Error),
}

impl core::fmt::Display for WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Decode(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(err) => Some(err),
            Self::Io(err) => Some(err),
        }
    }
}

impl From<WriteError> for std::io::Error {
    /// A writer's own [`std::io::Error`] comes back as it was; a decode
    /// failure converts as [`Error`] does, so neither arm allocates.
    fn from(err: WriteError) -> Self {
        match err {
            WriteError::Decode(err) => err.into(),
            WriteError::Io(err) => err,
        }
    }
}

impl From<Error> for WriteError {
    fn from(err: Error) -> Self {
        Self::Decode(err)
    }
}

impl From<std::io::Error> for WriteError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

const VERSION_OFFSET: usize = MAGIC.len();
const METHOD_OFFSET: usize = VERSION_OFFSET + 1;
const HEADER_LEN: usize = METHOD_OFFSET + 1;

/// Width of the check value that closes every frame: the CRC-32 of every
/// byte of the frame before it, header included, as a `u32` little-endian
/// (ADR-0068).
const CHECK_LEN: usize = size_of::<u32>();

/// One frame of a stream, split off the front of the bytes that remain.
struct Frame<'a> {
    method: Method,
    /// Exactly the frame's payload: the bytes its method says it spans.
    payload: &'a [u8],
    /// What the frame decodes to, known from its header alone.
    output_len: u64,
}

/// Width of the declared-length field that opens a [`Method::Stored`]
/// payload: `u64`, not the `u32` an `Lz` payload declares, because
/// [`compress`] stores inputs longer than `u32::MAX` bytes.
const STORED_LEN_BYTES: usize = size_of::<u64>();

/// The frames of a stream, in order, each split off by its header alone:
/// every frame names its own extent (a [`Method::Stored`] payload its data
/// length, a [`Method::Lz`] payload its coded length), so none is decoded
/// to find where it ends and a hostile length costs no allocation, only
/// the [`Error::Truncated`] of bytes that are not there. A frame is yielded
/// only once its check value matches, so a damaged one costs one pass over
/// its own bytes and no decode work ([`Error::BadChecksum`]). A stream is one
/// or more frames and nothing else: empty input yields [`Error::Truncated`],
/// and bytes after the last frame are not one, so they surface as the next
/// frame's [`Error::Truncated`] or [`Error::BadMagic`]. After an `Err` the
/// iterator is finished.
struct Frames<'a> {
    rest: &'a [u8],
    /// Set once a frame was yielded, so only the empty stream is an error.
    started: bool,
}

impl<'a> Frames<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            rest: input,
            started: false,
        }
    }

    /// Splits one frame off `rest`, its header checked by [`parse_header`]
    /// and its extent read by [`declared_extent`].
    fn split(&mut self) -> Result<Frame<'a>, Error> {
        let (header, tail) = self
            .rest
            .split_first_chunk::<HEADER_LEN>()
            .ok_or(Error::Truncated)?;
        let method = parse_header(*header)?;
        let (payload_len, output_len) = declared_extent(method, tail)?;
        let (checked, rest) = payload_len
            .checked_add(HEADER_LEN + CHECK_LEN)
            .and_then(|len| self.rest.split_at_checked(len))
            .ok_or(Error::Truncated)?;
        let (covered, check) = checked.split_at(HEADER_LEN + payload_len);
        if crc32::crc32(covered).to_le_bytes() != check {
            return Err(Error::BadChecksum);
        }
        self.rest = rest;
        let payload = &covered[HEADER_LEN..];
        Ok(Frame {
            method,
            payload,
            output_len,
        })
    }
}

/// The [`Method`] a frame `header` names, checked against [`MAGIC`] and
/// [`FORMAT_VERSION`]: a frame of either method must name exactly
/// [`FORMAT_VERSION`], the one version this build decodes. A version
/// above it is reported before an unknown method, so a newer build's
/// frame says "unsupported version", not "unknown method".
fn parse_header(header: [u8; HEADER_LEN]) -> Result<Method, Error> {
    if header[..MAGIC.len()] != MAGIC {
        return Err(Error::BadMagic);
    }
    let version = header[VERSION_OFFSET];
    if version > FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    let method = Method::try_from(header[METHOD_OFFSET])?;
    if version != FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }
    Ok(method)
}

/// How many bytes past a frame's header [`declared_extent`] reads for
/// `method`.
fn extent_prefix_len(method: Method) -> usize {
    match method {
        Method::Stored => STORED_LEN_BYTES,
        Method::Lz => codec::EXTENT_PREFIX_LEN,
    }
}

/// The payload length and output length a frame declares, read from the
/// first [`extent_prefix_len`] bytes of `after_header`, the bytes after its
/// header, and trusting nothing else: a caller that has not read the payload
/// yet learns how much of the input the frame spans, with no allocation and
/// no decode work. That the payload is present is the caller's check, and a
/// bad selector in a prefix cut short is [`Error::Corrupt`], not
/// [`Error::Truncated`], as it is with the whole frame in hand.
fn declared_extent(method: Method, after_header: &[u8]) -> Result<(usize, u64), Error> {
    match method {
        Method::Stored => {
            let declared = u64::from_le_bytes(*after_header.first_chunk().ok_or(Error::Truncated)?);
            let len = usize::try_from(declared).map_err(|_| Error::Truncated)?;
            let payload_len = STORED_LEN_BYTES.checked_add(len).ok_or(Error::Truncated)?;
            Ok((payload_len, declared))
        }
        Method::Lz => {
            let extent = codec::declared_extent(after_header)?;
            let output_len = extent.output_len;
            // The Stored floor (`docs/format/SPEC.md`, "Invariants"): no
            // conforming encoder writes an `Lz` payload longer than the
            // `Stored` payload of the same output, so a reader that has
            // seen only this header may refuse before it buffers a byte.
            if u64::try_from(extent.payload_len).map_err(|_| Error::Truncated)?
                > output_len + STORED_LEN_BYTES as u64
            {
                return Err(Error::Corrupt);
            }
            Ok((extent.payload_len, output_len))
        }
    }
}

impl<'a> Iterator for Frames<'a> {
    type Item = Result<Frame<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.started && self.rest.is_empty() {
            return None;
        }
        self.started = true;
        let frame = self.split();
        if frame.is_err() {
            self.rest = &[];
        }
        Some(frame)
    }
}

/// The caller's `max_len` applied across a stream's frames, shared by
/// [`decompress_bounded`] and [`decompress_to_writer`] so the two can't
/// drift on it.
struct Budget {
    /// `Some` only when the caller's `max_len` is strictly tighter than
    /// [`codec::MAX_DECODED_LEN`]: then it caps the stream's total output,
    /// `Stored` frames included. At or above it there is no total cap and
    /// each `Lz` frame stays under [`codec::MAX_DECODED_LEN`], the only
    /// ceiling its worst-case decode time has been measured against. A
    /// `Stored` frame's length is read from `input`, never spoofable past
    /// what was already loaded, so [`codec::MAX_DECODED_LEN`] buys it no
    /// safety margin, only a compatibility break for large incompressible
    /// input; a stream of such frames is no different.
    cap: Option<u32>,
    /// Whether the output is held in memory (`decompress_bounded`) and not
    /// handed on as it is produced (`decompress_to_writer`). Then the `Lz`
    /// frames' total is capped at [`codec::MAX_DECODED_LEN`] whatever
    /// `max_len` says: an `Lz` frame's output is not bounded by its input
    /// bytes (a 1,440-byte frame decodes to 64 MiB), so memory held
    /// across frames would grow without any input-proportional limit.
    collects: bool,
    /// Output admitted so far.
    produced: u64,
    /// `Lz` output admitted so far.
    lz_produced: u64,
}

impl Budget {
    fn new(max_len: u32, collects: bool) -> Self {
        Self {
            cap: (max_len < codec::MAX_DECODED_LEN).then_some(max_len),
            collects,
            produced: 0,
            lz_produced: 0,
        }
    }

    /// Admits `frame`'s output against the total caps, before any decode
    /// work (`rust-craft` skill's allocation-discipline).
    fn admit(&mut self, method: Method, output_len: u64) -> Result<(), Error> {
        self.produced = self.produced.saturating_add(output_len);
        if let Some(max) = self.cap
            && self.produced > u64::from(max)
        {
            return Err(Error::TooLarge {
                len: u32::try_from(self.produced).unwrap_or(u32::MAX),
                max,
            });
        }
        if self.collects && method == Method::Lz {
            self.lz_produced = self.lz_produced.saturating_add(output_len);
            if self.lz_produced > u64::from(codec::MAX_DECODED_LEN) {
                return Err(Error::TooLarge {
                    len: u32::try_from(self.lz_produced).unwrap_or(u32::MAX),
                    max: codec::MAX_DECODED_LEN,
                });
            }
        }
        Ok(())
    }
}

/// The data of a [`Method::Stored`] frame's `payload`, past its declared
/// length, which [`Frames`] already checked against the bytes present.
fn stored_data(payload: &[u8]) -> &[u8] {
    payload.get(STORED_LEN_BYTES..).unwrap_or_default()
}

/// How fast an adaptive frequency table learns: the increment each
/// observation adds and the total above which every count halves. The two
/// travel as one value so a table cannot pair one rate's increment with
/// another's ceiling.
#[derive(Clone, Copy)]
pub(crate) struct Rate {
    increment: u32,
    limit: u32,
}

impl Rate {
    /// The archive's original rate, shared by every adaptive frequency table
    /// but [`literal::Literal`]'s fast-rate bank 0 (`INC`/`LIM` in
    /// `research/imports/session-1/mothergod.rs`).
    pub(crate) const DEFAULT: Self = Self::new(12, 65536);

    pub(crate) const fn new(increment: u32, limit: u32) -> Self {
        Self { increment, limit }
    }
}

/// Increments `freq[symbol]`/`*total` by `rate`'s increment, then halves
/// every entry of `freq` once `*total` exceeds `rate`'s limit, recomputing
/// `*total` from the halved counts.
///
/// The halving is `(f+1) >> 1`, so a symbol with any evidence keeps
/// frequency at least 1 and stays codable.
pub(crate) fn rescale_bank(freq: &mut [u32], total: &mut u32, symbol: usize, rate: Rate) {
    freq[symbol] += rate.increment;
    *total += rate.increment;
    if *total > rate.limit {
        let mut new_total = 0u32;
        for f in freq.iter_mut() {
            *f = (*f + 1) >> 1;
            new_total += *f;
        }
        *total = new_total;
    }
}

/// Builds a `Vec<T>` of `n` clones of `value`, returning `Err` instead of
/// aborting if the allocator cannot satisfy `n` (hard rule 2, `rust-craft`
/// skill's allocation-discipline, #453). `resize` after `try_reserve_exact`
/// never asks the allocator for more than was reserved.
pub(crate) fn try_filled_vec<T: Clone>(
    n: usize,
    value: T,
) -> Result<Vec<T>, std::collections::TryReserveError> {
    let mut v = Vec::new();
    v.try_reserve_exact(n)?;
    v.resize(n, value);
    Ok(v)
}

/// Fallible `data.to_vec()`: `Err` instead of an abort if the allocator
/// cannot satisfy `data.len()` bytes (#453).
pub(crate) fn try_vec_from_slice(
    data: &[u8],
) -> Result<Vec<u8>, std::collections::TryReserveError> {
    let mut v = Vec::new();
    v.try_reserve_exact(data.len())?;
    v.extend_from_slice(data);
    Ok(v)
}

/// Assembles a complete frame from `method` and its payload, the
/// concatenation of `parts`, so a large payload is copied once, closed by
/// the check value over everything before it.
fn build_frame(method: Method, parts: &[&[u8]]) -> Vec<u8> {
    let payload_len: usize = parts.iter().map(|part| part.len()).sum();
    let mut frame = Vec::with_capacity(HEADER_LEN + payload_len + CHECK_LEN);
    frame.extend_from_slice(&MAGIC);
    frame.push(FORMAT_VERSION);
    frame.push(method as u8);
    for part in parts {
        frame.extend_from_slice(part);
    }
    frame.extend_from_slice(&crc32::crc32(&frame).to_le_bytes());
    frame
}

/// A complete [`Method::Stored`] frame: the payload is `data`'s length as a
/// little-endian `u64`, then `data` verbatim.
fn build_stored_frame(data: &[u8]) -> Vec<u8> {
    let declared = u64::try_from(data.len()).expect("usize is at most 64 bits");
    build_frame(Method::Stored, &[&declared.to_le_bytes(), data])
}

/// Whether a `candidate_len`-byte encoding beats an `incumbent_len`-byte
/// incumbent in a shortest-wins search. Strict: a tie keeps the
/// incumbent, so search order is a stable tie-break. Shared by
/// [`compress`]'s [`Method::Lz`]-vs-[`Method::Stored`] choice and
/// [`codec::encode`]'s filter-candidate search. The convention is ours;
/// `docs/format/SPEC.md` only bounds the frame from above, which either
/// choice satisfies on a tie.
///
/// Its own unit test is the only one that pins "strictly smaller": every
/// candidate is a valid lossless encoding, so no round-trip or selection
/// test observes which one a tie picks (#390's mutation sweep).
pub(crate) fn candidate_beats_incumbent(candidate_len: usize, incumbent_len: usize) -> bool {
    candidate_len < incumbent_len
}

/// The input bytes one [`compress`] frame covers. The optimal parse holds
/// 21 bytes of state per input byte, so this sets the encoder's peak memory
/// at roughly 370 MB whatever the input length (#1341); the ratio a block
/// gives up by restarting its models is priced in `research/JOURNAL.md`
/// S2-A116.
///
/// The 21 bytes, by array of the DP pass (`lz::dp_round`):
///
/// | bytes | array | why it is that wide |
/// |---|---|---|
/// | 12 | `parent` | one `Option<Move>` per position: a tag, a length and a distance, walked backward by `reconstruct`, so it cannot be a ring |
/// | 4 | match finder `left` | one `u32` tree link per position, the window's candidates |
/// | 4 | match finder `right` | the other link |
/// | 1 | the input | borrowed, counted because the block holds it |
///
/// `dp` and `cache` are rings of 64 Ki entries, 1.25 MiB whatever the
/// block, because no move reaches more than `MAX_MATCH_LEN` past the
/// position being expanded; they were 8 and 12 bytes per position until
/// S2-A123. The token lists between rounds, the greedy seed's `prev` array
/// and the models sit on top and are not per-position constants: 16 MiB of
/// zeros peaks at 372 MB (S2-A123).
const BLOCK_LEN: usize = 16 * 1024 * 1024;

/// Compresses `input` into a self-describing stream of frames.
///
/// Cuts `input` into blocks of 16 MiB, one frame each, so the
/// encoder's memory is bounded by the block and not by the input. Each frame
/// tries [`Method::Lz`] and falls back to [`Method::Stored`] whenever that
/// does not produce a smaller frame (`docs/format/SPEC.md`'s Stored-floor
/// invariant): tiny, incompressible, or already-compressed input. An input
/// longer than [`codec::MAX_DECODED_LEN`] bytes is one [`Method::Stored`]
/// frame, because [`decompress`] caps the `Lz` frames of a stream at that
/// total as [`Error::TooLarge`] and a stream this crate cannot read back is
/// not one it may write.
#[must_use]
pub fn compress(input: &[u8]) -> Vec<u8> {
    compress_blocks(input, BLOCK_LEN)
}

/// [`compress`] with the block length a parameter, so a test cuts at a
/// length it can afford. `block_len` is nonzero.
fn compress_blocks(input: &[u8], block_len: usize) -> Vec<u8> {
    if !u32::try_from(input.len()).is_ok_and(|len| len <= codec::MAX_DECODED_LEN) {
        return build_stored_frame(input);
    }
    if input.len() <= block_len {
        return compress_block(input, write::concurrent_encodes());
    }
    let mut encoder = write::Encoder::with_block_len(Vec::new(), block_len);
    std::io::Write::write_all(&mut encoder, input).expect("a Vec sink does not fail");
    encoder.finish().expect("a Vec sink does not fail")
}

/// Compresses everything `source` yields into `sink` as a stream of frames,
/// holding at most two blocks of 16 MiB at a time: the encoder's memory
/// follows the blocks and not the input, which [`compress`] cannot offer because it
/// takes the whole input as a slice. The block is not the peak: encoding one
/// needs far more working memory, and a stream of 256 MiB of zeros encodes
/// in under 1 GiB, a bound `.github/scripts/memory-check` asserts weekly and
/// `tests/fit/cli.txt` records as `streaming-memory`. It is
/// [`io::copy`](std::io::copy) into a [`write::Encoder`], which a caller who
/// produces the bytes instead of pulling them uses directly.
///
/// An input of at most [`codec::MAX_DECODED_LEN`] bytes writes the bytes
/// [`compress`] returns. A longer one writes `Lz` frames [`compress`] would
/// store, so its total `Lz` output passes what [`decompress`] and
/// [`decompress_bounded`] hold in memory and they reject it as
/// [`Error::TooLarge`]; [`decompress_to_writer`] carries any length.
///
/// # Errors
///
/// The first error `source` or `sink` returns. `sink` then holds the frames
/// of the blocks before the failing one, a stream that decodes to a prefix.
pub fn copy_encode<R: std::io::Read, W: std::io::Write>(source: R, sink: W) -> std::io::Result<()> {
    copy_encode_blocks(source, sink, BLOCK_LEN)
}

/// [`copy_encode`] with the block length a parameter, as [`compress_blocks`]
/// is for [`compress`]. `block_len` is nonzero.
fn copy_encode_blocks<R: std::io::Read, W: std::io::Write>(
    mut source: R,
    sink: W,
    block_len: usize,
) -> std::io::Result<()> {
    let mut encoder = write::Encoder::with_block_len(sink, block_len);
    std::io::copy(&mut source, &mut encoder)?;
    encoder.finish().map(drop)
}

/// One frame for `block`: [`Method::Lz`] if it is smaller, else
/// [`Method::Stored`]. Up to `workers` of the filter trials run at once
/// ([`codec::encode`]); the frame is the same for any `workers`.
fn compress_block(block: &[u8], workers: std::num::NonZeroUsize) -> Vec<u8> {
    let body = codec::encode(block, workers);
    if candidate_beats_incumbent(body.len(), block.len()) {
        return build_frame(Method::Lz, &[&body]);
    }
    build_stored_frame(block)
}

/// Decodes a frame produced by [`compress`] back into the original bytes.
/// A stream of frames back to back, as `cat a.mgdc b.mgdc` makes, decodes to
/// their outputs concatenated; bytes that are not a frame after the last
/// one are an error, never dropped (`docs/format/SPEC.md`, "Streams").
///
/// Equivalent to [`decompress_bounded`] with [`codec::MAX_DECODED_LEN`] as
/// the bound, the largest output this decoder's worst-case decode time has
/// been measured against. The `Lz` frames of a stream decode to at most that
/// much in total; a longer stream goes through [`decompress_to_writer`].
///
/// # Errors
///
/// Returns an [`Error`] when `input` is truncated, is not a mothergod
/// frame, uses a version or method this build does not understand, or (for
/// [`Method::Lz`]) is not internally consistent; see [`codec::decode`].
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, Error> {
    decompress_bounded(input, codec::MAX_DECODED_LEN)
}

/// Like [`decompress`], but rejects a stream whose total output would exceed
/// `max_len` bytes, checked frame by frame from its header before any
/// allocation or decode work (`rust-craft` skill's allocation-discipline).
/// An `Lz` frame is never allowed past [`codec::MAX_DECODED_LEN`] whatever
/// `max_len` says: that constant is the only ceiling this decoder's
/// worst-case decode time has been measured against (see its docs), so a
/// caller can tighten the bound for its own memory budget but never loosen
/// it past what has been proven safe. A `max_len` at or above it caps no
/// total of its own, but the `Lz` frames' output together is still held to
/// [`codec::MAX_DECODED_LEN`], because the returned `Vec` is memory and an
/// `Lz` frame's output is not bounded by its input bytes. `Stored` frames
/// add nothing to that total: each is as long as the input it was read from.
///
/// A caller embedding mothergod under a known memory budget (well below
/// [`codec::MAX_DECODED_LEN`]'s 256 MiB) should call this instead of
/// [`decompress`].
///
/// # Errors
///
/// Same as [`decompress`], plus [`Error::TooLarge`] whenever the output so
/// far, with the next frame's, would exceed a `max_len` strictly below
/// [`codec::MAX_DECODED_LEN`] (an explicit opt-in to a smaller memory
/// budget), or the [`Method::Lz`] frames' declared output lengths together
/// exceed [`codec::MAX_DECODED_LEN`]. `max_len` at or above
/// [`codec::MAX_DECODED_LEN`] never rejects a [`Method::Stored`] frame on
/// size alone: its length is read from `input`, so unlike [`Method::Lz`]'s
/// declared length it cannot be spoofed past the bytes present.
pub fn decompress_bounded(input: &[u8], max_len: u32) -> Result<Vec<u8>, Error> {
    let mut budget = Budget::new(max_len, true);
    let mut output: Option<Vec<u8>> = None;
    for frame in Frames::new(input) {
        let frame = frame?;
        budget.admit(frame.method, frame.output_len)?;
        let data = match frame.method {
            Method::Stored => try_vec_from_slice(stored_data(frame.payload))?,
            Method::Lz => codec::decode(frame.payload, codec::MAX_DECODED_LEN)?,
        };
        match &mut output {
            None => output = Some(data),
            Some(output) => {
                output.try_reserve(data.len())?;
                output.extend_from_slice(&data);
            }
        }
    }
    output.ok_or(Error::Truncated)
}

/// [`std::io::BufWriter`] for [`decompress_to_writer`], with its buffer built
/// through `try_reserve_exact`: `BufWriter::new` allocates infallibly and
/// would abort under allocator failure (hard rule 2, `tests/torture.rs`).
/// [`codec::decode_to_writer`] writes one filtered byte at a time
/// (`codec::StreamUndo::apply`), so an unbuffered `writer` would take a
/// `write_all` call per byte.
struct TryBufWriter<'w, W: std::io::Write> {
    inner: &'w mut W,
    buf: Vec<u8>,
}

impl<'w, W: std::io::Write> TryBufWriter<'w, W> {
    /// Matches [`std::io::BufWriter`]'s own default buffer size.
    const CAPACITY: usize = 8 * 1024;

    fn try_new(inner: &'w mut W) -> Result<Self, std::collections::TryReserveError> {
        let mut buf = Vec::new();
        buf.try_reserve_exact(Self::CAPACITY)?;
        Ok(Self { inner, buf })
    }

    /// Writes out and clears any bytes still buffered, without flushing
    /// `inner` itself (see [`std::io::Write::flush`] for that).
    fn flush_buf(&mut self) -> std::io::Result<()> {
        if !self.buf.is_empty() {
            self.inner.write_all(&self.buf)?;
            self.buf.clear();
        }
        Ok(())
    }
}

impl<W: std::io::Write> std::io::Write for TryBufWriter<'_, W> {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if data.len() > Self::CAPACITY - self.buf.len() {
            self.flush_buf()?;
        }
        if data.len() >= Self::CAPACITY {
            return self.inner.write(data);
        }
        // The flush above guarantees at least `CAPACITY - self.buf.len()`
        // room, and this arm is only reached when `data.len()` is at most
        // that: never grows `buf` past the capacity reserved in `try_new`.
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_buf()?;
        self.inner.flush()
    }
}

impl<W: std::io::Write> Drop for TryBufWriter<'_, W> {
    /// Best-effort flush on drop, matching [`std::io::BufWriter`]'s own
    /// `Drop` impl: a caller that returns through `?` before reaching an
    /// explicit `flush()` call still gets whatever was buffered so far.
    /// Errors are unobservable this late (no `Result` to return them
    /// through) and so are ignored here exactly as `BufWriter` ignores
    /// them.
    fn drop(&mut self) {
        let _ = self.flush_buf();
    }
}

/// Like [`decompress_bounded`], but writes the decoded bytes to `writer`
/// incrementally instead of collecting them into one returned `Vec<u8>`.
/// Same frame-level checks, in the same order, through the same `Frames`
/// and `Budget`.
///
/// A [`Method::Lz`] frame whose encoder picked
/// [`filters::select::Candidate::Identity`], `Delta`, or `Bcj` streams:
/// filters whose undo step runs sequentially with small fixed lookback or
/// lookahead (a no-op, a stride, or a 5-byte call/jmp instruction), so the
/// decoded LZ token stream needs no whole-buffer pass afterward, and
/// `codec::decode_to_writer` bounds memory to the frame's declared
/// [`lz::Reach`] (1 MiB, or 2 MiB for a wide frame) for any of them
/// regardless of the frame's declared length.
/// [`Method::Stored`] writes its payload slice of `input` through the same
/// bounded writer and allocates no decoded buffer, so resident memory is the
/// input plus the write buffer. [`filters::select::Candidate::Transpose`],
/// [`filters::select::Candidate::Base64`],
/// [`filters::select::Candidate::Reverse`] and
/// [`filters::select::Candidate::ReverseDelta`] fall back to a whole-buffer
/// decode followed by one bulk write: no worse than [`decompress_bounded`],
/// just never streamed.
/// [`decodes_incrementally`] tells a caller which case a frame is without
/// decoding it, though this function does not require calling it first.
///
/// # Errors
///
/// [`WriteError::Decode`] for anything [`decompress_bounded`] would itself
/// return as an `Err`, plus [`Error::OutOfMemory`] if the allocator cannot
/// satisfy the write buffer's own allocation; [`WriteError::Io`] if
/// `writer` itself fails.
pub fn decompress_to_writer<W: std::io::Write>(
    input: &[u8],
    max_len: u32,
    writer: &mut W,
) -> Result<(), WriteError> {
    use std::io::Write as _;

    let mut budget = Budget::new(max_len, false);
    let mut writer = TryBufWriter::try_new(writer).map_err(Error::from)?;
    for frame in Frames::new(input) {
        let frame = frame?;
        budget.admit(frame.method, frame.output_len)?;
        match frame.method {
            Method::Stored => writer.write_all(stored_data(frame.payload))?,
            Method::Lz => {
                codec::decode_to_writer(frame.payload, codec::MAX_DECODED_LEN, &mut writer)?;
            }
        }
    }
    writer.flush()?;
    Ok(())
}

/// Reports whether every frame of `input` streams under
/// [`decompress_to_writer`] (the filters it lists), without doing any of
/// that decode work itself. The answer is not uniform across frames, hence
/// the check.
///
/// [`Method::Stored`] always answers `true`: there is no filter step, so
/// nothing about a bound on resident memory depends on its content.
/// [`Method::Lz`]'s answer depends on which filter its encoder picked.
/// `Transpose` writes scattered across the *entire* buffer in column-major
/// order, `Base64` has no per-byte undo, and `Reverse` and `ReverseDelta`
/// emit the last byte first, so those need the whole buffer resident. The
/// `matches!` below is a second copy of `decompress_to_writer`'s own dispatch.
///
/// # Errors
///
/// Same as [`decompress`]'s header-parsing errors
/// ([`Error::Truncated`], [`Error::BadMagic`], [`Error::UnsupportedVersion`],
/// [`Error::UnknownMethod`]), plus [`Error::BadChecksum`] for a frame whose
/// check value does not match, and [`Error::Corrupt`] when a [`Method::Lz`]
/// frame's filter selector does not name a real [`filters::select::Candidate`]:
/// everything short of actually decoding the payload.
pub fn decodes_incrementally(input: &[u8]) -> Result<bool, Error> {
    let mut incremental = true;
    for frame in Frames::new(input) {
        let frame = frame?;
        if frame.method == Method::Lz {
            let selector = frame.payload.first_chunk().ok_or(Error::Truncated)?;
            let candidate =
                filters::select::Candidate::from_header_bytes(*selector).ok_or(Error::Corrupt)?;
            incremental &= !matches!(
                candidate,
                filters::select::Candidate::Transpose(_)
                    | filters::select::Candidate::Base64
                    | filters::select::Candidate::Reverse
                    | filters::select::Candidate::ReverseDelta(_)
            );
        }
    }
    Ok(incremental)
}

/// Shared test-only fixtures multiple modules' test suites had each
/// hand-rolled a copy of.
#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;

// Not under Miri: same rationale as `coder.rs`'s `mod proptests` header
// comment (interpretation cost, issue #456).
#[cfg(test)]
#[cfg(not(miri))]
mod proptests;
