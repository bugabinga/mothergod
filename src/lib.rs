#![doc(
    html_logo_url = "https://raw.githubusercontent.com/bugabinga/mothergod/main/assets/logo.svg"
)]
#![forbid(unsafe_code)]
//! mothergod — general purpose compression.
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

// Every module but `filters` is `#[doc(hidden)]`: the engine's internals, not
// a surface downstream crates are meant to call (ROADMAP M6). They stay `pub`
// for two reasons. `bench` and `fuzz` call into `lz`, `literal` and `codec`.
// And the live codec path does not reach the research primitives (`Ppm`,
// `Model::ideal_cost_bits`, `bittree`'s symbol and ideal-cost functions,
// `codec`'s `compressed_len_*` and `ideal_cost_bits_*` experiment entry
// points, `fieldtype`, `Sse::contexts`), whose callers are tests: under
// `pub(crate)` they become `dead_code` errors with `-D warnings`. Narrowing
// waits on wiring or removing those items. `filters` is documented: S2-A2
// judged it "a defensible standalone library surface on their own merits"
// (`research/JOURNAL.md`).
#[doc(hidden)]
pub mod bittree;
#[doc(hidden)]
pub mod codec;
#[doc(hidden)]
pub mod coder;
#[doc(hidden)]
pub mod column;
#[doc(hidden)]
pub mod fieldtype;
pub mod filters;
#[doc(hidden)]
pub mod literal;
#[doc(hidden)]
pub mod logistic;
#[doc(hidden)]
pub mod lz;
#[doc(hidden)]
pub mod model;
#[doc(hidden)]
pub mod ppm;
#[doc(hidden)]
pub mod sse;

/// First bytes of every mothergod frame.
pub const MAGIC: [u8; 4] = *b"MGDC";

/// Container format version written into frames produced by this crate.
///
/// Every bump is a bitstream format change (CLAUDE.md hard rule 5); the
/// history of what each version changed lives in `docs/format/SPEC.md` and
/// the ADRs it cites. This build decodes exactly this version for both
/// methods: versions 1 through 11 were retired outright, no release having
/// written any of them
/// (`docs/adr/0050-the-decode-forever-promise-starts-at-1-0.md`,
/// `docs/adr/0063-retire-format-versions-1-through-9.md`), and a frame
/// naming any other version is rejected as `UnsupportedVersion` before its
/// payload is read (`tests/golden/v12-*.mgdc` pins the current version's
/// frames).
pub const FORMAT_VERSION: u8 = 12;

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
    /// Input ended before the frame was complete: inside the header, or
    /// short of the length a [`Method::Stored`] payload or a [`Method::Lz`]
    /// coded stream declares. Bytes after the last frame of a stream read
    /// as the start of one more, so they are this or [`Error::BadMagic`].
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
    /// The allocator could not satisfy some fixed-size allocation partway
    /// through decode: the output buffer's reservation (`declared_len`
    /// bytes, already checked against the effective bound before this
    /// point), a model's adaptive frequency table, or a filter's undo
    /// buffer. A real allocator failure this far into decode is rare, but
    /// hard rule 2 (`CLAUDE.md`) does not carve out an exception for it:
    /// every one of those allocations goes through `try_reserve_exact`
    /// specifically so this returns an `Err` instead of the process
    /// aborting (`rust-craft` skill's allocation-discipline, torture-swept
    /// by `tests/torture.rs`, #453).
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
            | Error::TooLarge { .. } => std::io::ErrorKind::InvalidData,
        })
    }
}

impl From<std::collections::TryReserveError> for Error {
    /// Every `try_reserve`/`try_new` allocation this crate's decode path
    /// makes fails the same way: [`Error::OutOfMemory`], never a panic
    /// (hard rule 2). One impl instead of a `.map_err(|_| Error::OutOfMemory)`
    /// at each of the decoder's fallible-allocation call sites.
    fn from(_: std::collections::TryReserveError) -> Self {
        Self::OutOfMemory
    }
}

/// Error from [`decompress_to_writer`]: either the frame failed to decode
/// ([`Error`], same variants and meaning as [`decompress`]'s) or `writer`
/// itself failed.
///
/// Two owned variants instead of the single [`std::io::Error`] this type
/// replaced (downcast via [`std::io::Error::get_ref`]): that shape needed
/// `std::io::Error::other` to wrap every decode error, which boxes its
/// payload through two allocations with no fallible sibling on stable Rust
/// (`Box::new` twice: once into `Box<dyn Error + Send + Sync>`, once into
/// `io::Error`'s own `Custom` struct). That meant *every* decode error —
/// even `Truncated` on a two-byte input — could abort the process under
/// real allocator pressure, violating hard rule 2 (issue #479). Plain enum
/// construction allocates nothing.
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
/// the [`Error::Truncated`] of bytes that are not there. A stream is one
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

    /// Splits one frame off `rest`, checked against [`MAGIC`] and
    /// [`FORMAT_VERSION`]: a frame of either method must name exactly
    /// [`FORMAT_VERSION`], the one version this build decodes. A version
    /// above it is reported before an unknown method, so a newer build's
    /// frame says "unsupported version", not "unknown method".
    fn split(&mut self) -> Result<Frame<'a>, Error> {
        let (header, tail) = self
            .rest
            .split_at_checked(HEADER_LEN)
            .ok_or(Error::Truncated)?;
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
        let (payload_len, output_len) = match method {
            Method::Stored => {
                let (declared, data) = tail
                    .split_first_chunk::<STORED_LEN_BYTES>()
                    .ok_or(Error::Truncated)?;
                let declared = u64::from_le_bytes(*declared);
                match usize::try_from(declared) {
                    Ok(len) if len <= data.len() => (STORED_LEN_BYTES + len, declared),
                    _ => return Err(Error::Truncated),
                }
            }
            Method::Lz => {
                let extent = codec::extent(tail)?;
                (extent.payload_len, u64::from(extent.output_len))
            }
        };
        let (payload, rest) = tail.split_at(payload_len);
        self.rest = rest;
        Ok(Frame {
            method,
            payload,
            output_len,
        })
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
    fn admit(&mut self, frame: &Frame<'_>) -> Result<(), Error> {
        self.produced = self.produced.saturating_add(frame.output_len);
        if let Some(max) = self.cap
            && self.produced > u64::from(max)
        {
            return Err(Error::TooLarge {
                len: u32::try_from(self.produced).unwrap_or(u32::MAX),
                max,
            });
        }
        if self.collects && frame.method == Method::Lz {
            self.lz_produced = self.lz_produced.saturating_add(frame.output_len);
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

/// Frequency increment and rescale ceiling of every adaptive frequency table
/// at the archive's original rate (`INC`/`LIM` in
/// `research/imports/session-1/mothergod.rs`). [`literal::Literal`]'s
/// fast-rate bank 0 is the one table that tunes its own
/// `FAST_INCREMENT`/`FAST_LIMIT`.
pub(crate) const DEFAULT_RESCALE_INCREMENT: u32 = 12;
/// See [`DEFAULT_RESCALE_INCREMENT`].
pub(crate) const DEFAULT_RESCALE_LIMIT: u32 = 65536;

/// Increments `freq[symbol]`/`*total` by `increment`, then halves every
/// entry of `freq` once `*total` exceeds `limit`, recomputing `*total` from
/// the halved counts.
///
/// The halving is `(f+1) >> 1`, so a symbol with any evidence keeps
/// frequency at least 1 and stays codable.
pub(crate) fn rescale_bank(
    freq: &mut [u32],
    total: &mut u32,
    symbol: usize,
    increment: u32,
    limit: u32,
) {
    freq[symbol] += increment;
    *total += increment;
    if *total > limit {
        let mut new_total = 0u32;
        for f in freq.iter_mut() {
            *f = (*f + 1) >> 1;
            new_total += *f;
        }
        *total = new_total;
    }
}

/// Codes `symbol` through `encoder` at the sub-range `freq[..symbol].sum()..
/// +freq[symbol]` out of `denom`. `denom` is the caller's: `total`, or
/// `total + distinct` under [`ppm::Ppm`]'s escape band.
pub(crate) fn encode_symbol(encoder: &mut coder::Encoder, freq: &[u32], symbol: usize, denom: u64) {
    let low: u32 = freq[..symbol].iter().sum();
    let high = low + freq[symbol];
    encoder.encode(u64::from(low), u64::from(high), denom);
}

/// Finds the symbol whose cumulative-frequency range in `freq` contains
/// `target` (from [`coder::Decoder::target`]), returning it with the
/// `[low, high)` range [`coder::Decoder::decode`] must consume: the inverse
/// of [`encode_symbol`].
///
/// Never runs past the end of `freq`: callers bound `target` to
/// `freq.iter().sum()` (for [`ppm::Ppm`], after peeling off its escape
/// share), so the scan finds a symbol whatever bytes produced `target`.
pub(crate) fn scan_for_target(freq: &[u32], target: u64) -> (usize, u64, u64) {
    let mut symbol = 0;
    let mut low = 0u64;
    while low + u64::from(freq[symbol]) <= target {
        low += u64::from(freq[symbol]);
        symbol += 1;
    }
    let high = low + u64::from(freq[symbol]);
    (symbol, low, high)
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
/// concatenation of `parts`, so a large payload is copied once.
fn build_frame(method: Method, parts: &[&[u8]]) -> Vec<u8> {
    let payload_len: usize = parts.iter().map(|part| part.len()).sum();
    let mut frame = Vec::with_capacity(HEADER_LEN + payload_len);
    frame.extend_from_slice(&MAGIC);
    frame.push(FORMAT_VERSION);
    frame.push(method as u8);
    for part in parts {
        frame.extend_from_slice(part);
    }
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
/// about 42 bytes of state per input byte (#1094), so this sets the encoder's
/// peak memory at roughly 700 MB whatever the input length; the ratio a
/// block gives up by restarting its models is priced in `research/JOURNAL.md`
/// S2-A116.
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
        return compress_block(input);
    }
    let mut stream = Vec::new();
    for block in input.chunks(block_len) {
        stream.extend_from_slice(&compress_block(block));
    }
    stream
}

/// One frame for `block`: [`Method::Lz`] if it is smaller, else
/// [`Method::Stored`].
fn compress_block(block: &[u8]) -> Vec<u8> {
    let body = codec::encode(block);
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
/// [`Method::Lz`]) is not internally consistent — see [`codec::decode`].
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
/// [`decompress`] directly: ROADMAP M4's bounded-memory decode guarantee,
/// ahead of and independent from a future streaming/block API.
///
/// # Errors
///
/// Same as [`decompress`], plus [`Error::TooLarge`] whenever the output so
/// far, with the next frame's, would exceed a `max_len` strictly below
/// [`codec::MAX_DECODED_LEN`] (an explicit opt-in to a smaller memory
/// budget), or the [`Method::Lz`] frames' declared output lengths together
/// exceed [`codec::MAX_DECODED_LEN`]. `max_len` at or above
/// [`codec::MAX_DECODED_LEN`] never rejects a [`Method::Stored`] frame on
/// size alone: its payload length is read directly from `input`, never
/// spoofable past what was already loaded, so unlike [`Method::Lz`]'s
/// declared-length field, [`codec::MAX_DECODED_LEN`] buys it no safety
/// margin, only a compatibility break for large incompressible input.
pub fn decompress_bounded(input: &[u8], max_len: u32) -> Result<Vec<u8>, Error> {
    let mut budget = Budget::new(max_len, true);
    let mut output: Option<Vec<u8>> = None;
    for frame in Frames::new(input) {
        let frame = frame?;
        budget.admit(&frame)?;
        let data = match frame.method {
            Method::Stored => stored_data(frame.payload).to_vec(),
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

/// Minimal write-buffering wrapper, standing in for [`std::io::BufWriter`]
/// on [`decompress_to_writer`]'s real decode path: `BufWriter::new` itself
/// allocates its buffer through an infallible path, which would abort the
/// process under a real allocator failure instead of returning an `Err`
/// (hard rule 2, `rust-craft` skill's allocation-discipline,
/// `tests/torture.rs`, #453). [`codec::decode_to_writer`]'s streaming path
/// writes one filtered byte at a time
/// (`codec::StreamUndo::apply`), so wrapping `writer` unbuffered would turn
/// each into its own `write_all` call; this preserves that batching while
/// building its buffer through `try_reserve_exact`.
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
/// Same frame-level checks, in the same order (shared with it through the
/// same private header-parsing and bound-checking helpers), so the two
/// can't drift.
///
/// A [`Method::Lz`] frame whose encoder picked
/// [`filters::select::Candidate::Identity`], `Delta`, or `Bcj` streams:
/// filters whose undo step runs sequentially with small fixed lookback or
/// lookahead (a no-op, a stride, or a 5-byte call/jmp instruction), so the
/// decoded LZ token stream needs no whole-buffer pass afterward, and
/// `codec::decode_to_writer` bounds memory to [`lz::WINDOW`] (1 MiB) for any
/// of them regardless of the frame's declared length (`research/JOURNAL.md`
/// S1-P7/S2-D5/S2-A74, ROADMAP M4's bounded-memory decode guarantee).
/// [`Method::Stored`] writes its payload slice of `input` through the same
/// bounded writer and allocates no decoded buffer, so resident memory is the
/// input plus the write buffer. [`filters::select::Candidate::Transpose`]
/// alone falls back to a whole-buffer decode followed by one bulk write: no
/// worse than [`decompress_bounded`], just never streamed.
/// [`decodes_incrementally`] tells a caller which case a frame is without
/// decoding it, though this function does not require calling it first.
///
/// # Errors
///
/// [`WriteError::Decode`] for anything [`decompress_bounded`] would itself
/// return as an `Err`, plus [`Error::OutOfMemory`] if the allocator cannot
/// satisfy the write buffer's own allocation; [`WriteError::Io`] if
/// `writer` itself fails. Neither variant boxes its payload through
/// `std::io::Error::other` (issue #479): both are plain enum
/// construction, so a decode error can never abort the process under
/// allocator pressure the way the wrapped form could.
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
        budget.admit(&frame)?;
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

/// Reports whether `input`'s frame can decode with the output produced in
/// address order and bounded lookback, without doing any of that decode
/// work itself: a precondition a future streaming/block API needs, checked
/// here first because it does not have one uniform answer
/// (`research/JOURNAL.md` S2-D4, ROADMAP M4).
///
/// [`Method::Stored`] always answers `true`: there is no filter step, so
/// nothing about a bound on resident memory depends on its content.
/// [`Method::Lz`]'s answer depends on which filter its encoder picked —
/// [`filters::select::Candidate::Identity`], `Delta`, and `Bcj` all undo
/// sequentially with small fixed lookback or lookahead (a no-op, a stride,
/// or a 5-byte call/jmp instruction), and [`decompress_to_writer`] streams
/// all three, but `Candidate::Transpose`'s decode writes scattered across
/// the *entire* buffer in column-major order, so it needs the whole buffer
/// resident regardless of how a streaming decoder is otherwise built. This
/// predicate is checked independently of [`decompress_to_writer`]'s own
/// dispatch rather than the two sharing one classification, so a caller can
/// ask the question before committing to either API, and the two staying in
/// sync is a property tests can verify rather than an invariant the code
/// silently assumes.
///
/// # Errors
///
/// Same as [`decompress`]'s header-parsing errors
/// ([`Error::Truncated`], [`Error::BadMagic`], [`Error::UnsupportedVersion`],
/// [`Error::UnknownMethod`]), plus [`Error::Corrupt`] when a [`Method::Lz`]
/// frame's filter selector does not name a real [`filters::select::Candidate`]
/// — everything short of actually decoding the payload.
pub fn decodes_incrementally(input: &[u8]) -> Result<bool, Error> {
    let mut incremental = true;
    for frame in Frames::new(input) {
        let frame = frame?;
        if frame.method == Method::Lz {
            let selector = frame.payload.first_chunk().ok_or(Error::Truncated)?;
            let candidate =
                filters::select::Candidate::from_header_bytes(*selector).ok_or(Error::Corrupt)?;
            incremental &= !matches!(candidate, filters::select::Candidate::Transpose(_));
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
