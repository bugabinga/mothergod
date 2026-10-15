# ADR-0072: Reverse is filter selector kind 5, shortlisted for every input; version 15

Status: accepted · Date: 2026-10-15 · Applies ADR-0063 · Serves issue #1123 · `FORMAT_VERSION` 14 to 15

## Context

`filters::reverse` was ported from the founding session, where it was accepted with the note that right-anchored structure is real (`research/JOURNAL.md` S1-A2), and left unwired.
ADR-0071 wired its sibling `base64_unwrap` and left #1123 open for this one, with the finish condition: wired behind a journal ACCEPT or deleted behind a journal REJECT.
Measured as a frame filter on real bitstreams, reversing wins on two of the 13 benchmark cases: 0.05% on `interleaved_audio16` and 0.69% on the sealed `access_log` generator, and no case regresses (S2-A119).
`select::pick` cannot predict it: every n-gram entropy of a byte string equals the same entropy read backwards, so the shortlist's proxies see no difference between a string and its reversal.

## Decision

The filter selector gains kind 5, `[5, 0]`: the frame's filtered bytes are the frame data in reverse order, and the decoder reverses them back.
The filter preserves length, so the declared length is the output length and `Candidate::output_len` answers it unchanged.
`select::pick` shortlists `Candidate::Reverse` for every input, and the trial encode decides, as it does for every other candidate.
Every filtered byte string is a valid kind 5 frame, so the decoder rejects nothing the encoder could not have written.
A kind 5 frame decodes through the whole-buffer path and is not streamed: the first output byte is the last filtered byte.
`FORMAT_VERSION` becomes 15.
Under ADR-0063 the bump retires version 14 in the same PR: the six `v14-` golden pairs move to `v15-` with their check values recomputed, one is re-recorded because the current encoder now writes it as kind 5, one pair is added for kind 5, and the adversarial seeds move to version 15 with the seed that named kind 5 as unknown moved to kind 6.

## Consequences

An input at most 16 MiB (ADR-0067's block) pays one more trial encode: the benchmark cases encode 25 to 31% slower (S2-A119).
Encode speed is tracked and not gated (ROADMAP SPEED), so the cost is stated and not defended by a threshold.
Decode cost does not measurably change: a kind 5 frame adds one copy and one in-place reversal of the output.
Peak decode memory for a kind 5 frame is the filtered bytes plus the output, twice the output, inside the existing ceiling.
The filter is applied to the raw input and never stacked: reversal followed by a delta stride took `sqlite_like_records` from 20,864 to 20,292 bytes in the measurement that preceded this one, a gain the single-filter selector cannot express.
That is a different design, a composed selector, and waits for its own entry.

## Rejected alternatives

**Delete the filter.**
It wins on two classes without regressing any, and the finish condition #1123 names for a delete is a REJECT.

**Shortlist it behind a detector.**
No count over n-grams distinguishes a string from its reversal, so a detector would be a new proxy with no measurement behind it, which is the proxy approach S1-A1 recorded as falsified for filter selection.

**Stream the reversal.**
The first output byte is the last filtered byte, so a streaming decoder holds the whole stream anyway.
