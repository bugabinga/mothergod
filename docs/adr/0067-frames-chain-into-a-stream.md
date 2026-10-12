# ADR-0067: Frames chain into a stream; the `Lz` payload declares its coded length; version 12

Status: accepted · Date: 2026-10-12 · Applies ADR-0063 · Serves issue #978 (decode half) · `FORMAT_VERSION` 11 to 12

## Context

`decompress` decoded the first frame of its input and dropped the rest: `cat a.mgdc b.mgdc | mothergod -d` printed `a` and exited 0, and trailing garbage was never reported (#978).
`cat a.gz b.gz | gunzip`, `xz -dc`, `zstd -dc` and `bzip2 -dc` decode every unit and report trailing bytes.
The rule they share is one or more frames, decoded in order, anything else an error.
It also frees the encoder to cut a large input into blocks, one frame each, which bounds its memory; that slice is its own PR.

A decoder can only chain frames if each names where it ends.
ADR-0065 gave `Stored` that.
An `Lz` payload ended where its range-coded stream did, and the stream does not know: the range decoder pads a short stream with implicit zero bits and the encoder's final flush is not a length the decoder can recover from how much it consumed.

## Decision

The `Lz` payload carries a `u64` little-endian coded length after the token count: the byte count of the range-coded stream that follows.
A decoder rejects a payload with fewer stream bytes as `Truncated`; bytes past it are the next frame's.
Like ADR-0065's field it is compared with the bytes present and never sizes an allocation.

A stream is one or more frames; `decompress`, `decompress_bounded`, `decompress_to_writer` and `decodes_incrementally` walk all of them.
Bytes that are not a frame surface as the next frame's `Truncated` or `BadMagic`, so no error variant is added.
`decodes_incrementally` answers `true` only if every frame does.

`max_len` keeps its meaning: a bound below `codec::MAX_DECODED_LEN` caps the stream's total output, checked from the frame header before any decode work.
At or above it no total covers `Stored` frames, which had none and are as long as the input they were read from.
The `Lz` frames of a stream returned as a `Vec` (`decompress`, `decompress_bounded`) are capped together at `MAX_DECODED_LEN`, checked frame by frame from the headers, so decode work stops within one frame of the cap, because an `Lz` frame's output is not bounded by its input: 100 copies of a 1,440-byte frame of 64 MiB of zeros made `decompress` allocate 6.7 GB (reviewer's repro on the first draft of this ADR's PR).
`decompress_to_writer` holds one write buffer however long the stream is, so it caps each `Lz` frame alone and carries the multi-gigabyte streams.
A hostile stream therefore buys no more decode work per input byte than one frame did, and no more held memory than `MAX_DECODED_LEN` plus the input.

`FORMAT_VERSION` becomes 12.
Under ADR-0063 the bump retires version 11 in the same PR.
`tests/golden/v12-*` re-records the six version 11 pairs; each stream is byte-identical to version 11 and only the version byte and the new field differ, checked by `cmp` against the committed frames.
`tests/adversarial/` moves its seeds to version 12 and gains four: `lz-truncated-stream`, `lz-coded-length-max`, `frame-trailing-garbage`, `frame-trailing-partial-frame`.

## Consequences

An `Lz` frame grows by 8 bytes, a `Stored` frame by none.
`stored-trailing-bytes` was `Corrupt` and is now `Truncated`: the extra bytes are a frame too short to be one.
`Corrupt` still covers an `Lz` stream whose tokens do not account for the declared output.
Block-split encode (#978's other half) changes no bytes until this lands, which it now can.
No check value: a bit flip inside a payload can still decode to wrong bytes (#1020).

## Rejected alternatives

**`u32` coded length, matching the other `Lz` fields.**
It saves 4 bytes per frame.
No bound short of `u64` is proved for what the coder emits from an adversarial `u32::MAX`-byte input (a bit costs up to 16 bits at the coder's probability floor), so the encoder would carry a panic or a silent saturation for a case `Stored` wins anyway.

**Decode to find the end.**
The decoder primes 32 bits and pads past the end, so where it stands after the last token is not where the stream ends.

**A sentinel or end-of-frame marker in the stream.**
It costs bits per frame inside the model and still leaves `Stored` to delimit separately.

**A total cap at `MAX_DECODED_LEN` on every frame, `Stored` included, for `decompress`.**
It would be new behavior for `Stored` data, which round-trips past that length today, and `Stored` memory is bounded by the input already.
Only the `Lz` total is capped.
A caller that needs more than `MAX_DECODED_LEN` out of the block-split streams the next PR emits uses `decompress_to_writer`; `compress` and `decompress` agree past that length only for `Stored` output (#1086).
