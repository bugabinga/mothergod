# mothergod bitstream format (FORMAT_VERSION 11)

Status: **normative, versioned** (ADR-0050). This document describes the
current code; code and spec change in the same PR, and evolution adds a
version via a `FORMAT_VERSION` bump. A version is retired only by its own
ADR: one no release has written goes at once, decode path and fixture
included; one a release has written goes only after a later release that
still reads it and writes its successor has shipped and `CHANGELOG.md`
has named the retirement, so a user can re-compress first. Until the
first release, a `FORMAT_VERSION` bump retires its predecessor in the
same PR, so the decoder reads exactly one version (ADR-0063). From the
first version a 1.0 build writes, no version is ever retired.

## Frame layout

```
offset  size  field
0       4     magic: 0x4D 0x47 0x44 0x43 ("MGDC")
4       1     format version (currently 11)
5       1     method byte
6       ...   payload (method-defined)
```

A decoder MUST reject: input shorter than 6 bytes (`Truncated`), wrong magic
(`BadMagic`), version greater than it supports (`UnsupportedVersion`),
unknown method (`UnknownMethod`). A frame of either method additionally
requires exactly the current format version: every earlier version was
retired outright under ADR-0050, no release ever having written one
(ADR-0063), so a decoder rejects a frame naming any other version as
`UnsupportedVersion` before it reads the payload. Version history: 1 named
a different `Lz` payload layout (ADR-0026, superseded by ADR-0028); 2
added the 2-byte filter selector; 3 coded each literal as SSE-calibrated
binary decisions (ADR-0038); 4 added the column expert for `Transpose`
frames (ADR-0046); 5 through 7 moved other candidates to a logit-domain
mixer, a learned-baseline rate schedule and stretch-domain SSE bins
(ADR-0052, ADR-0054, ADR-0055); 8 split the length model by token kind
(ADR-0057); 9 split the offset model by match length (ADR-0058); 10 added
the length residual trees (ADR-0061); 11 gave the `Stored` payload a
declared length (ADR-0065).

## Methods

| byte | name   | payload |
|------|--------|---------|
| 0x00 | Stored | declared length, then the original data, verbatim |
| 0x01 | Lz     | see below |

### `Stored` (ADR-0065)

```
offset  size  field
0       8     declared data length, u64 LE
8       ...   the original data, verbatim
```

A decoder MUST reject a payload shorter than 8 bytes, or holding fewer
data bytes than the field declares, as `Truncated`, and one holding more
as `Corrupt`. The field is never an allocation size: it is compared with
the bytes present. It is a `u64` because an encoder stores inputs longer
than `u32::MAX` bytes. There is no check value: a bit flip inside the data
is not detected (ADR-0065).

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

**Literal sub-stream (ADR-0038, ADR-0046, ADR-0052, ADR-0054, ADR-0055).**
Each literal byte is 8 chained binary decisions over the symbol alphabet
(`bittree`'s chain-rule decomposition). A frame whose filter selector does
not name `Candidate::Transpose` codes them through a logit-domain mixer:
each of the six experts' per-node probability is mapped to the logit
domain, blended under a weight vector whose per-key rate is a learned
baseline (a fast EMA of each key's own squared prediction error read
against a slower EMA of the identical signal), mapped back, and calibrated
through a stretch-domain SSE table (evenly spaced bins in logit space,
concentrating resolution near 0/1) keyed on tree position
(`literal::Literal::encode_logit_sse`/`decode_logit_sse`). A frame whose
selector names `Candidate::Transpose` codes its literals one step further
still: a column-keyed seventh expert (`column::column_of`/`bank_of`,
keyed on the byte's position among the transposed stream's columns) is
blended into the six-expert mix before a linear-domain SSE-calibrated
binary-tree coding (`literal::Literal::encode_column`/`decode_column`).
`flag` and `slot` are coded identically regardless of candidate; a decoder
dispatches the literal sub-stream on the frame's already-parsed filter
selector.

**The `length` symbol (ADR-0057).** A `Token::Match`'s length and a
`Token::Rep`'s length are coded through two independent adaptive models,
selected by kind: a rep token's length tends to come from a different
distribution than a freshly found match's (`research/JOURNAL.md`
S2-A109/S2-A110). This applies to every candidate, `Candidate::Transpose`
included, since the length symbol sits outside the literal sub-stream.

**A `Token::Match`'s `offset` symbol (ADR-0058).** A match's distance is
coded through one of four independent adaptive models, selected by a
coarse, saturating bucket of the match's own length: a short match's
distance tends to come from a different distribution than a long match's
(`research/JOURNAL.md` S2-A111/S2-A112). This applies to every candidate,
`Candidate::Transpose` included, and `Token::Rep` never reaches `offset`
at all (a repeat prices through `slot` instead, choosing which cached
distance to reuse, never a fresh one).

**A copy length's residual bits (ADR-0061).** A length is a bucket symbol
(`lz::bucket`) plus the residual bits below that bucket. Its top four
residual bits (all of them in buckets 0 through 4), most significant
first, are each coded through an adaptive binary model chosen by the bits
already coded in that value and by the bucket: one tree per bucket for
`Token::Match` lengths and an independent one per bucket for `Token::Rep`
lengths. Any bits below those four are raw 50/50 bits. A fresh tree prices
a bit at exactly one bit, so the gain is adaptation alone: record-shaped
data repeats exact lengths (`research/JOURNAL.md` S2-A114/S2-A115). A
distance's residual bits are raw.

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
  `header + 8 + len(x)` (the header and the `Stored` length field) — fall
  back to Stored (JOURNAL S1-L1).
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
