# mothergod bitstream format — DRAFT (FORMAT_VERSION 2)

Status: **unstable**. Anything may change until version 1 is frozen (ROADMAP
M4). This document is normative for the current code; code and spec must
change in the same PR.

## Frame layout

```
offset  size  field
0       4     magic: 0x4D 0x47 0x44 0x43 ("MGDC")
4       1     format version (currently 2)
5       1     method byte
6       ...   payload (method-defined)
```

A decoder MUST reject: input shorter than 6 bytes (`Truncated`), wrong magic
(`BadMagic`), version greater than it supports (`UnsupportedVersion`),
unknown method (`UnknownMethod`). A `Method::Lz` payload additionally
requires format version >= 2 (`codec::LZ_MIN_VERSION`): version 1 named a
different, incompatible `Lz` payload layout (ADR-0026, superseded by
ADR-0028), so a version-1 `Lz` frame is rejected as `UnsupportedVersion`
rather than parsed under the current layout.

## Methods

| byte | name   | payload |
|------|--------|---------|
| 0x00 | Stored | the original data, verbatim |
| 0x01 | Lz     | see below |

### `Lz` (`src/codec.rs`, `JOURNAL` S2-D2, ADR-0028)

A trial-selected filter (`src/filters.rs`: none, delta, BCJ, or
transpose — `filters::select::pick` shortlists candidates,
`codec::encode` keeps whichever produces the smallest payload) applied to
the frame data, then optimal-parse LZ tokens (`src/lz.rs`), entropy-coded
by adaptive flag/length/offset/rep-slot tables (`src/model.rs`) and a
six-expert context-mixing literal model (`src/literal.rs`), over an
adaptive range coder (`src/coder.rs`).

```
offset  size  field
0       2     filter selector: [kind, param]
2       4     declared output length, u32 LE
6       4     token count, u32 LE
10      ...   range-coded stream, of the FILTERED bytes
```

Filter selector `kind`: 0 (none), 1 (delta), 2 (BCJ), 3 (transpose).
`param` is the delta stride or transpose column count, `1..=255`; zero for
kinds that take none (0, 2). A decoder MUST reject any other `[kind, param]`
pair as `Corrupt` (`filters::select::Candidate::from_header_bytes`) — an
unrecognized kind, or a zero `param` on a kind that requires one. Every
filter this format defines preserves length, so "declared output length"
above is also the length of the *filtered* bytes: a decoder reconstructs
those first, checks their length against this field, and only then reverses
the filter to recover the original frame data.

Offset-bucket/rep-code disjointness (see the invariant below): a
`Token::Match`'s distance is coded as a bucket symbol plus residual bits
through the `offset` table; a `Token::Rep`'s slot (which of the 3-entry
repeat-offset cache to reuse) is coded as a symbol through the separate
`slot` table. The two never share a code space, so a rep-slot index can
never be misread as an offset bucket or vice versa — the shape of the
founding port bug this section's invariant exists to rule out.

## Invariants (binding on every future method)

- Lossless: decode(encode(x)) == x for all x.
- Stored floor: an encoder MUST NOT emit a frame larger than
  `header + len(x)` — fall back to Stored (JOURNAL S1-L1).
- Decoders never panic and allocate at most a bounded multiple of the
  declared output size for any input. For `Lz`, "declared output size" is
  the payload's own length field: the decoder never preallocates from it,
  only grows toward it, and rejects a token the instant it would exceed it.
  That field is itself capped (`codec::MAX_DECODED_LEN`, currently 256
  MiB): a ratio check against the payload's own byte count cannot bound
  this format's amplification, because its adaptive models saturate fast
  enough that a legitimate maximal-ratio frame and a forged header become
  indistinguishable by size alone (measured: a real encode already reaches
  a ~3,158:1 ratio at 60,000 input bytes, with the encoded size barely
  moving as input grows past that). The ceiling is a decoder policy, not a
  wire-format field, so raising it is not a `FORMAT_VERSION` bump; it is
  provisional pending `ROADMAP.md` M4's streaming/block API, the intended
  real fix for bounded-memory decode without a single hardcoded ceiling.
- Bit-identical output across platforms for the same input and version:
  IEEE-754 basic float operations (`+ - * /`) are correctly rounded and
  reproducible, but libm transcendentals are not, so nothing on the decode
  path may call one (ADR-0024). This supersedes this document's earlier
  "integer-only probability arithmetic" wording (JOURNAL S1-A5): S1-A5's
  full integer mixer remains a possible future direction (an M5 speed
  lead), not a correctness requirement.
- Known trap: rep-symbol/offset-bucket collision — the founding port bug.
  Any LZ method spec MUST state its offset-bucket/rep-code disjointness
  invariant explicitly.
