# mothergod bitstream format — DRAFT (FORMAT_VERSION 1)

Status: **unstable**. Anything may change until version 1 is frozen (ROADMAP
M4). This document is normative for the current code; code and spec must
change in the same PR.

## Frame layout

```
offset  size  field
0       4     magic: 0x4D 0x47 0x44 0x43 ("MGDC")
4       1     format version (currently 1)
5       1     method byte
6       ...   payload (method-defined)
```

A decoder MUST reject: input shorter than 6 bytes (`Truncated`), wrong magic
(`BadMagic`), version greater than it supports (`UnsupportedVersion`),
unknown method (`UnknownMethod`).

## Methods

| byte | name   | payload |
|------|--------|---------|
| 0x00 | Stored | the original data, verbatim |
| 0x01 | Lz     | see below |

### `Lz` (`src/codec.rs`, `JOURNAL` S2-D2)

Optimal-parse LZ tokens (`src/lz.rs`), entropy-coded by adaptive flag/
length/offset/rep-slot tables (`src/model.rs`) and a six-expert
context-mixing literal model (`src/literal.rs`), over an adaptive range
coder (`src/coder.rs`). No filter pass yet (`src/filters.rs` is ported but
not wired in): the payload always covers the raw frame data.

```
offset  size  field
0       4     declared output length, u32 LE
4       4     token count, u32 LE
8       ...   range-coded stream
```

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
