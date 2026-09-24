# mothergod bitstream format (FORMAT_VERSION 4)

Status: **normative, versioned** (ADR-0050). This document describes the
current code; code and spec change in the same PR, and evolution adds a
version via a `FORMAT_VERSION` bump. A version is retired only by its own
ADR: one no release has written goes at once, decode path and fixture
included; one a release has written goes only after a later release that
still reads it and writes its successor has shipped and `CHANGELOG.md`
has named the retirement, so a user can re-compress first. From the first
version a 1.0 build writes, no version is ever retired.

## Frame layout

```
offset  size  field
0       4     magic: 0x4D 0x47 0x44 0x43 ("MGDC")
4       1     format version (currently 4)
5       1     method byte
6       ...   payload (method-defined)
```

A decoder MUST reject: input shorter than 6 bytes (`Truncated`), wrong magic
(`BadMagic`), version greater than it supports (`UnsupportedVersion`),
unknown method (`UnknownMethod`). A `Method::Lz` payload additionally
requires format version >= 3 (`codec::LZ_MIN_VERSION`): version 1 named a
different, incompatible `Lz` payload layout (ADR-0026, superseded by
ADR-0028), so a version-1 `Lz` frame is rejected as `UnsupportedVersion`
rather than parsed under the current layout; version 2 named this same
outer layout but coded its literal sub-stream through a direct 256-way
range division, and was retired outright under ADR-0050, no release ever
having written it. Versions 3 and 4 `Lz` frames share the same outer
payload layout below; only the literal sub-stream's internal shape
differs between them (see "Lz" below, ADR-0038, ADR-0046) — a decoder
dispatches on the declared version (and, for version 4, the frame's own
filter selector) rather than rejecting either of them.

## Methods

| byte | name   | payload |
|------|--------|---------|
| 0x00 | Stored | the original data, verbatim |
| 0x01 | Lz     | see below |

### `Lz` (`src/codec.rs`, `JOURNAL` S2-D2, ADR-0028, ADR-0038)

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

**Literal sub-stream shape is version- (and, at version 4, candidate-)
gated (ADR-0038, ADR-0046).** At format version 3 (`codec::LZ_MIN_VERSION`)
and above, each literal byte is 8 chained binary decisions over the
six-expert mixer's cumulative table
(`bittree::encode_symbol`/`decode_symbol`'s chain-rule decomposition),
each calibrated by a secondary symbol estimation (SSE) stage keyed on
tree position (`bittree::sse_context`, 255 contexts) before it drives the
range coder (`literal::Literal::encode_sse`/`decode_sse`). At format
version 4 and above (`codec::COLUMN_EXPERT_MIN_VERSION`), a frame whose
filter selector names `Candidate::Transpose` codes its literals one step
further still: a column-keyed seventh expert (`column::column_of`/
`column_bank`, keyed on the byte's position among the transposed stream's
columns) is blended into the six-expert mix before the same
SSE-calibrated binary-tree coding (`literal::Literal::encode_column`/
`decode_column`); every other candidate at version 4 codes its literals
exactly as version 3 does. Every other symbol in the stream
(flag/length/offset/slot) is coded identically regardless of version or
candidate; a decoder dispatches only the literal sub-stream, on the
frame's declared version and (at version 4) its own already-parsed filter
selector.

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
  `mothergod::decompress_bounded` lets a caller tighten this ceiling to its
  own memory budget (clamped, never raised, since 256 MiB is the only value
  this decoder's worst-case decode time has been measured against) — a
  first, additive slice of M4's bounded-memory decode guarantee, not the
  streaming/block API itself (`research/JOURNAL.md` S1-P7, S2-A71).
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
