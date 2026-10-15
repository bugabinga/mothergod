# ADR-0071: Base64 unwrap is filter selector kind 4, and an Lz frame's output length is derived from its filtered length; version 14

Status: accepted · Date: 2026-10-15 · Applies ADR-0063 · Serves issue #1123 · `FORMAT_VERSION` 13 to 14

## Context

`filters::base64_unwrap` was ported from the founding session, where it was the single biggest ratio drop (`research/JOURNAL.md` S1-A2), and left unwired: `crate::codec` selected only delta, transpose and BCJ.
Base64 hides the bytes underneath it behind a 3-to-4 phase shift, so the literal model and the match finder see a stream whose contexts mean nothing.
Unwrapping a whole-input base64 payload before the parse takes `base64_wrapped` from 0.532 to 0.395 bits per byte on real bitstreams (S2-A118).
Every filter so far preserves length, and the frame leans on that twice: the header's declared length is both the filtered and the output length, and a reader budgets a frame's output (`Budget`, the Stored floor, `max_len`) from that field alone, before it decodes a byte.
The unwrap is the first filter that changes length, and its old module form prefixed a flag byte to say whether it had applied.

## Decision

The filter selector gains kind 4, `[4, 0]`: the frame's filtered bytes are the standard-base64 decoding of the frame data, which the decoder re-wraps with `=` padding.
The encoder selects it only when the whole block is canonical standard base64 (`base64_unwrap::encode` decodes it and re-encodes the result to prove it), so the selector replaces the flag byte, and the trial encode against the other candidates decides whether it pays.

The header's declared length stays the length of the filtered bytes, the tokens' fill target.
A frame's output length is derived from it: `4 * ceil(declared / 3)` for kind 4, the declared length for every other kind (`Candidate::output_len`).
Every reader that budgets a frame asks that function, so `max_len`, `Budget` and the Stored floor bound the bytes a frame produces, not the bytes it reconstructs first.
A decoder rejects a frame whose output exceeds the clamped `max_len` as `TooLarge` before it allocates, and any filtered length is a valid frame: the re-wrap is total, so no filtered stream is invalid on its own.

A kind 4 frame decodes through the whole-buffer path and is not streamed, like transpose.
`FORMAT_VERSION` becomes 14.
Under ADR-0063 the bump retires version 13 in the same PR: the six `v13-` golden pairs move to `v14-` with their check values recomputed, the adversarial seeds move to version 14, and a `v14-base64-wrapped` pair pins kind 4.

## Consequences

A base64 input at most 16 MiB (ADR-0067's block) pays one more trial encode and a linear unwrap check; an input that is not base64 pays a scan bounded at 4096 bytes, and only when its length is a multiple of 4.
Peak decode memory for a kind 4 frame is the filtered bytes plus the output, about 1.75 times the output, inside the existing ceiling because the ceiling now applies to the output.
The filter covers a whole block of canonical base64 and nothing else: MIME and PEM wrap at 64 or 76 columns with newlines, and base64 embedded in a larger file, and both decline.
Whether either is worth a filter is unmeasured and waits for its own entry.

## Rejected alternatives

**Keep the flag byte and a `Candidate` that always applies.**
A byte of every frame's filtered stream, and a second place besides the selector that says what the decoder does.

**Declare the output length in the header and derive the filtered length.**
The filtered length is the one the decoder allocates and fills the tokens against, so every existing check (`ensure_room`, the final length compare) stays untouched when it is the declared one.
The reverse puts a division in each of them.

**Stream the re-wrap.**
Three filtered bytes make four output bytes, so a per-byte undo needs a group buffer and a trailing-padding rule at the end of the stream.
It is the same work transpose avoided, for a filter that fires on a narrow input class.
