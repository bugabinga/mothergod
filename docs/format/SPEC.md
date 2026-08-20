# mothergod bitstream format — DRAFT (FORMAT_VERSION 0)

Status: **unstable**. Anything may change until version 1 is frozen (ROADMAP
M4). This document is normative for the current code; code and spec must
change in the same PR.

## Frame layout

```
offset  size  field
0       4     magic: 0x4D 0x47 0x44 0x43 ("MGDC")
4       1     format version (currently 0)
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

Planned methods (from the research architecture; each lands with its own spec
section, tests, and ADR): filtered/LZ/CM pipeline, pure-CM arm, fast tANS arm.

## Invariants (binding on every future method)

- Lossless: decode(encode(x)) == x for all x.
- Stored floor: an encoder MUST NOT emit a frame larger than
  `header + len(x)` — fall back to Stored (JOURNAL S1-L1).
- Decoders never panic and allocate at most a bounded multiple of the
  declared output size for any input.
- Bit-identical output across platforms for the same input and version:
  integer-only probability arithmetic in the coded path (JOURNAL S1-A5).
- Known trap: rep-symbol/offset-bucket collision — the founding port bug.
  Any LZ method spec MUST state its offset-bucket/rep-code disjointness
  invariant explicitly.
