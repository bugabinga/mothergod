# ADR-0065: The `Stored` payload declares its length; version 11

Status: accepted · Date: 2026-10-10 · Applies ADR-0063 · Resolves issue #988 · `FORMAT_VERSION` 10 to 11

## Context

A `Stored` payload was the data verbatim, so a frame cut short decoded to the bytes that arrived: exit 0, empty stderr, a prefix kept as the file (#988).
An `Lz` frame carries its declared output length and rejects the same cut.
Incompressible data (random bytes, archives, media) is the case that reaches `Stored`, so the frames most likely to be large had no truncation detection.
A decoder also cannot tell where an unlengthed `Stored` frame ends, which blocks #978's rule of one or more frames, trailing bytes an error.

## Decision

The `Stored` payload is a `u64` little-endian declared length, then that many data bytes.
A decoder rejects a short payload as `Truncated` and a long one as `Corrupt`; the field is compared with the bytes present and never allocates.
`FORMAT_VERSION` becomes 11.
Under ADR-0063 the bump retires version 10 in the same PR: both methods now require exactly the current version, `Stored` included, because a version 10 `Stored` payload would otherwise parse as a length and a body.
`tests/golden/v11-*` re-records the five version 10 pairs (the `Lz` payloads are byte-identical, only the version byte moves) and adds `v11-stored-incompressible`, the first fixture of the `Stored` method.
`tests/adversarial/` gains four `Stored` seeds.

## Consequences

An incompressible frame grows by 8 bytes; the Stored floor in `docs/format/SPEC.md` is `header + 8 + len(x)`.
No check value: a bit flip inside `Stored` data still decodes to wrong bytes with exit 0.
`Lz` has the same gap today, so it is a separate decision for both methods (#982's integrity row), not half of this one.
#978 inherits a `Stored` frame whose end is knowable.

## Rejected alternatives

**`u32` length, matching the `Lz` header.**
It saves 4 bytes per frame and caps `Stored` at 4 GiB, while `compress` stores inputs longer than `u32::MAX` bytes today.
The cap would turn that path into a panic or a new error for the largest, least compressible inputs.

**Length plus a check value for both methods.**
The genre's answer, and the larger design: algorithm, width, where it sits in `Lz`, and the decode-time cost.
It closes a different failure (corruption, not truncation) and deserves its own ADR.

**Keep version 10 `Stored` decodable.**
It needs a version-dependent parse of the same method byte, for frames no release has written.
