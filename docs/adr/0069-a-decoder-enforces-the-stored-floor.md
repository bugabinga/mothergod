# ADR-0069: A decoder enforces the Stored floor from the frame header

Status: accepted · Date: 2026-10-13 · Resolves issue #1137 · No `FORMAT_VERSION` change

## Context

`docs/format/SPEC.md` states the Stored floor as an encoder rule: a frame is never larger than the `Stored` frame of the same data.
`compress` honours it, and no decoder checked it, so the invariant lived in one implementation.
An `Lz` header declares `coded_len` as a `u64`, and `read::Decoder` buffers what the source delivers toward it before the check value can be verified.
A frame declaring 1 byte of output and `1 << 40` coded bytes, chained to a source of 400 MiB, took `VmHWM` from 13 MB to 422 MB under `with_max_len(src, 1 << 20)` (#1137).

## Decision

The decoder rejects an `Lz` frame whose payload is longer than a `Stored` payload of its declared output, as `Corrupt`.
The check sits in the shared extent parse (`declared_extent`), so the slice paths and the reader agree, and it runs on the header, before any payload byte is buffered.
The bound is `STORED_LEN_BYTES + declared output length`, so `max_len` bounds the payload the reader buffers to `max_len` plus a few bytes.

No `FORMAT_VERSION` bump: every frame a conforming encoder wrote already satisfies the floor, so none changes meaning.
`decode` is unchanged on every golden fixture.

## Consequences

An `Lz` frame forged below the floor, which no encoder emits, is `Corrupt` and not `Truncated`, even when its payload is also cut short.
A frame declaring fewer than 10 bytes of output cannot be `Lz` at all, since its 18-byte prefix alone exceeds the floor; `compress` already stores it.
`tests/adversarial/lz-over-stored-floor` pins the repro; hand-built header-only frames in the unit tests declare `floor_declared_len()`.

## Rejected alternatives

**Doc only, naming `Read::take` as the bound.**
It leaves the caller who set `max_len` holding a multiple of it, and the invariant in one implementation.

**A ratio bound on `coded_len` against `declared_len`.**
It would be a new rule; the floor is the one the format already states and the encoder already keeps.
