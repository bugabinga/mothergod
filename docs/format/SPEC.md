# mothergod bitstream format (FORMAT_VERSION 17)

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
4       1     format version (currently 17)
5       1     method byte
6       ...   payload (method-defined)
end-4   4     check value: CRC-32 of every byte before it, u32 LE
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
declared length (ADR-0065); 12 gave the `Lz` payload a declared coded
length, so a frame's end is knowable and frames chain into a stream
(ADR-0067); 13 closed each frame with a check value (ADR-0068); 14 added
the base64 filter and derived a frame's output length from its filtered
length (ADR-0071); 15 added the reverse filter (ADR-0072); 16 added
reverse then delta (ADR-0073); 17 declared the match reach in the selector
(ADR-0075).

## Check value (ADR-0068)

Every frame ends in the CRC-32 of the frame's bytes from the magic through
the last payload byte, as a `u32` LE. The CRC is CRC-32/ISO-HDLC, the one
gzip, zlib and xz use: reflected polynomial `0xEDB88320`, initial value and
final XOR `0xFFFFFFFF`, and `"123456789"` checks to `0xCBF43926`. It is
always present, for both methods, and never optional.

The check value follows the payload's extent, which the method names (the
`Stored` data length, the `Lz` coded length), so a decoder finds it without
decoding. A decoder MUST verify it before it decodes the payload or writes
any byte of the frame, and MUST reject a mismatch as `BadChecksum`. A frame
with fewer bytes than its extent and check value need is `Truncated`. The
parse errors that locate the check value (version, method, extent) come
first, so a flip of those fields may surface as them.

The value detects corruption; it is not authentication. Anyone can compute
it for forged bytes, so no decoder bound may rely on it (invariants below).

## Streams (ADR-0067)

A compressed stream is one or more frames, back to back, decoded in order;
its output is their outputs concatenated. Every frame names its own extent
(the `Stored` data length, the `Lz` coded length) and ends in its check
value, so a decoder finds where one ends without decoding it. Nothing else may follow the last frame: a
decoder MUST reject bytes that are not a frame, as the next frame's
`Truncated` (fewer than 6 bytes, or a payload or check value cut short) or
`BadMagic`,
and an empty stream is `Truncated`. The frame count is not bounded: each
frame costs at least its own input bytes. A caller's output bound below
`codec::MAX_DECODED_LEN` covers the stream's total, and a frame whose
output would carry the total past it is `TooLarge` before any of its
decode work. At or above that ceiling no total covers `Stored` frames, each as long as the
input it was read from. The `Lz` frames of a stream held in memory
(`decompress`, `decompress_bounded`) are capped together at the same
ceiling, since an `Lz` frame's output is not bounded by its input; a decoder
that hands its output on (`decompress_to_writer`) holds none across frames and
caps each `Lz` frame alone, so a stream of any length decodes under the same
per-frame worst case a single frame has.

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
data bytes than the field declares, as `Truncated`. Bytes past the declared
length are the next frame's. The field is never an allocation size: it is
compared with the bytes present. It is a `u64` because an encoder stores
inputs longer than `u32::MAX` bytes.

### `Lz` (`src/codec.rs`, `JOURNAL` S2-D2, ADR-0028, ADR-0038)

A trial-selected filter (`src/filters.rs`: none, delta, BCJ, transpose,
base64 unwrap, reverse, or reverse then delta — `filters::select::pick`
shortlists candidates, `codec::encode` keeps whichever produces the smallest
payload) applied to
the frame data, then optimal-parse LZ tokens (`src/lz.rs`), entropy-coded
by adaptive flag/length/offset/rep-slot tables (`src/model.rs`) and a
six-expert context-mixing literal model (`src/literal.rs`), over an
adaptive range coder (`src/coder.rs`).

```
offset  size  field
0       2     selector: [kind | reach << 7, param]
2       4     declared length of the filtered bytes, u32 LE
6       4     token count, u32 LE
10      8     coded length: the range-coded stream's byte count, u64 LE
18      ...   range-coded stream, of the FILTERED bytes
```

A decoder MUST reject a payload holding fewer stream bytes than the coded
length declares as `Truncated`; bytes past it are the next frame's. The
field is never an allocation size: it is compared with the bytes present,
as `Stored`'s is, and it is a `u64` for the same reason. A decoder MUST
also reject a payload longer than a `Stored` payload of the same output, an
`Lz` payload of more than `8 + output length` bytes (coded length plus 18,
against the output length plus 8), as `Corrupt`, from the header alone: the Stored floor below makes it one no conforming encoder writes
(ADR-0069).

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

Selector `kind`: its low 7 bits name the filter, 0 (none), 1 (delta), 2 (BCJ), 3 (transpose),
4 (base64 unwrap, ADR-0071), 5 (byte reversal, ADR-0072), 6 (byte reversal,
then delta, ADR-0073). `param` is the delta stride or transpose column count,
`1..=255`; zero for kinds that take none (0, 2, 4, 5). A decoder MUST
reject any other `[kind, param]` pair as `Corrupt`
(`filters::select::Candidate::from_header_bytes`) — an unrecognized kind, or
a zero `param` on a kind that requires one, or a nonzero one on a kind that
takes none. The top bit of the `kind` byte is the frame's *reach*
(ADR-0075): clear, a match distance may be at most 1 MiB (`2^20`); set, at
most `2^21 - 1`, the last distance the offset code's top bucket can name. A
decoder MUST reject a match distance past the declared reach as `Corrupt`,
and a decoder that streams a frame holds a ring of that many bytes. The
encoder's choice of reach is free (`codec::encode` tries the wide reach on
input longer than 1 MiB and keeps it when the payload is strictly smaller),
so the reach is no part of what a filter means. The declared length is the length of the *filtered* bytes: a
decoder reconstructs those first, checks their length against this field,
and only then reverses the filter to recover the original frame data.

The *output length* is what the frame decodes to, derived from the declared
length by the selector's kind (`filters::select::Candidate::output_len`).
Kinds 0 to 3, 5 and 6 preserve length, so it is the declared length. Kind 4
stores the standard-base64 decoding of the frame data, and the decoder re-wraps it
to `4 * ceil(declared / 3)` bytes with `=` padding: 3 bytes to 4, a
trailing 1 or 2 bytes to a padded group. Every filtered byte string is a
valid kind 4 frame, so the re-wrap rejects nothing. An encoder MUST emit
kind 4 only when the frame data is canonical standard base64 (RFC 4648
alphabet, length a multiple of 4, `=` only as the last group's final 1 or 2
characters, padding bits zero), so the re-wrap reproduces it exactly. A
decoder MUST reject a frame whose output length exceeds its output ceiling
(below) as `TooLarge`, from the header alone, before it allocates; that
output length, not the declared one, is also what a stream's total budget
and the Stored floor read.

Kind 5 stores the frame data in reverse byte order, and the decoder reverses
it back, whole-buffer. Every filtered byte string is a valid kind 5 frame,
and an encoder MAY emit it for any input.

Kind 6 stores the delta of the reversed frame data: the frame data in reverse
byte order, then each byte minus the byte `param` positions before it
(wrapping, as kind 1), the first `param` bytes unchanged. The decoder undoes
the delta, then reverses. Every filtered byte string is a valid kind 6 frame
for each `param` in `1..=255`, and an encoder MAY emit it for any input.
Like kind 5 it decodes whole-buffer.

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
  `header + 8 + len(x) + 4` (the header, the `Stored` length field and the
  check value) — fall back to Stored (JOURNAL S1-L1). A decoder MUST reject
  an `Lz` frame that breaks it as `Corrupt` before reading its payload
  (ADR-0069).
- Decoders never panic and allocate at most a bounded multiple of the
  declared output size for any input. For `Lz`, "declared output size" is
  the payload's own length field, which bounds the filtered bytes, and the
  output length derived from it is what the ceiling below caps: the decoder
  never preallocates from it, only grows toward it, and rejects a token the
  instant it would exceed it. That output length is itself capped (`codec::MAX_DECODED_LEN`, currently 256
  MiB): a ratio check against the payload's own byte count cannot bound
  this format's amplification, because its adaptive models saturate fast
  enough that a legitimate maximal-ratio frame and a forged header become
  indistinguishable by size alone (measured: a real encode already reaches
  a ~3,158:1 ratio at 60,000 input bytes, with the encoded size barely
  moving as input grows past that). The ceiling is a decoder policy, not a
  wire-format field, so raising it is not a `FORMAT_VERSION` bump. It bounds
  one `Lz` frame's output and the total `Lz` output of a stream the buffered
  `mothergod::decompress` holds in memory; it is no file-size limit, because
  `mothergod::compress` cuts its input into 16 MiB frames and
  `mothergod::decompress_to_writer` carries a stream of any length
  (ADR-0067). `mothergod::decompress_bounded` lets a caller tighten this
  ceiling to its own memory budget (clamped, never raised, since 256 MiB is
  the only value this decoder's worst-case decode time has been measured
  against; `research/JOURNAL.md` S1-P7, S2-A71).
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
