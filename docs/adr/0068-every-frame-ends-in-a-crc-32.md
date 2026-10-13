# ADR-0068: Every frame ends in a CRC-32 of its bytes; version 13

Status: accepted · Date: 2026-10-13 · Applies ADR-0063 · Resolves issue #1020 · `FORMAT_VERSION` 12 to 13

## Context

A bit flip inside a frame can decode to wrong bytes with exit status 0 (#1020).
On a `Stored` frame it always does: the data is verbatim and nothing checks it.
On an `Lz` frame the arithmetic coder caught most flips in one run (of 294 single-byte writes into a 314-byte frame, 291 decoded to a typed error and 3 to the input itself), and nothing rules out the flip it does not catch.
ADR-0065 and ADR-0067 each recorded the gap and deferred it to this decision.
The genre splits three to three (`tests/fit/lib.txt`, row `integrity-check`): gzip, xz and snap return an error on a flipped stream, while zstd, brotli and lz4_flex at their defaults decode wrong bytes for 1669 to 2073 of about 2200 single-bit flips.

## Decision

Every frame ends in a check value: the CRC-32 of every byte of the frame before it, magic and version and method included, as a `u32` little-endian.
CRC-32 is the gzip, zlib and xz polynomial (reflected `0xEDB88320`), so any tool that speaks it can reproduce the value.
It detects every burst of 32 bits or fewer, hence every single bit flip anywhere in the frame.

It is always on.
There is no flag and no second format: a flag is encoder-side API surface and a decode path that may or may not verify, and the 4 bytes are the price of the claim "a damaged frame is an error".

The decoder verifies it when it splits the frame off the stream, after the header and the extent parse (which locate the value) and before any payload is decoded or any byte is written.
A mismatch is the new `Error::BadChecksum`, `InvalidData` as an `io::Error`.
Every entry point shares that one place, so a damaged frame writes nothing, and the frames before it in a stream were already written.

`FORMAT_VERSION` becomes 13.
Under ADR-0063 the bump retires version 12 in the same PR.
`tests/golden/v13-*` re-records the six version 12 pairs; each frame is its version 12 frame with the version byte moved and the value appended, checked against `compress`'s output.
`tests/adversarial/` moves its seeds to version 13, re-seals those that must pass the check to reach their error, and gains `stored-bad-checksum`, `lz-bad-checksum` and `stored-truncated-check`.
The FIT row `integrity-check` flips to `holds`.

## Consequences

Every frame grows by 4 bytes, so a compressed input pays 4 bytes per 16 MiB block (ADR-0067's block split).
The Stored floor in `docs/format/SPEC.md` is `header + 8 + len(x) + 4`.
Decode makes one pass over each frame's bytes before it starts, at 380 MB/s here against 11 MB/s for `Lz` decode of Rust source.
The value is integrity, not authentication: an adversary computes it, and no bound in the decoder leans on it (hard rule 2 holds behind a forged check).
So the fuzz targets re-seal their mutated frames, or the check would turn away nearly every one before the models run.

## Rejected alternatives

**CRC of the decoded output, as gzip and xz do.**
It also catches a decoder that disagrees with its encoder, which the round-trip tests and the golden fixtures already guard.
It costs more: the pass is over the output, up to 256 MiB per `Lz` frame, and it comes after the decode work and after `decompress_to_writer` has written the damaged bytes.

**Opt-in, as zstd and lz4 default to.**
The probe's own numbers are what that default buys: most flips silent.
A decoder that cannot tell whether a frame carries a value cannot promise anything about it.

**A 64-bit check (XXH64, CRC-64).**
It costs 4 more bytes per frame to detect random corruption a CRC-32 already misses once in 2^32, and the single flips this guards against not at all.

**Cover the payload only.**
A flip of the method byte could then turn one method's payload into the other's and parse, with a check over bytes that no longer mean what they did.
Covering the header costs 6 bytes of CRC input.

**Report a mismatch as `Corrupt`.**
`Corrupt` says the payload contradicts itself: adversarial input or an encoder bug.
A mismatch says the bytes changed in storage or transit, and what the caller does about it differs (fetch the file again).
