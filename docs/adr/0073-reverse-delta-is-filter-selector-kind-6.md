# ADR-0073: Reverse then delta is filter selector kind 6; version 16

Status: accepted · Date: 2026-10-16 · Applies ADR-0063 · Serves issue #1222 · `FORMAT_VERSION` 15 to 16

## Context

ADR-0072 wired `reverse` as a single-filter kind and recorded the composition it could not write: on `sqlite_like_records`, reversing the input and then differencing at stride 80 coded 20,292 bytes where no single filter coded fewer than 20,864 (`research/JOURNAL.md` S2-A119).
Issue #1222 asked for the composed selector, and for a per-field byte-order candidate beside it, on the grounds that most of the gain looked like most-significant-byte-first order inside each field.
The byte-order candidate came out smaller than the claim: reversing each 20-byte row coded 20,310 bytes (554 of the 572 won), and on real little-endian record data it won at most 0.5%, at a stride that was not the record width (S2-A120).
The composition is the part that moved real bytes: a 64 KiB slice of the `.rela.dyn` section of the `rustup` binary, 24-byte little-endian relocation records, codes 6,669 bytes as it stands and 6,168 after reverse then delta.

## Decision

The filter selector gains kind 6, `[6, stride]`: the frame data in reverse byte order, then `delta` at `stride`.
The decoder undoes the delta, then reverses.
Every filtered byte string is a valid kind 6 frame for each stride in `1..=255`, as for kind 1, and a zero stride is `Corrupt`.
The filter preserves length, so `Candidate::output_len` answers the declared length unchanged.
A kind 6 frame decodes through the whole-buffer path and is not streamed: the first output byte is the last undone byte.
`select::pick` ranks the delta strides on the reversed input's first `PROBE_LEN` bytes, which are the original's last, reversed.
It shortlists `Candidate::ReverseDelta(stride)` when a stride ranks above no filter, and the trial encode decides.
`FORMAT_VERSION` becomes 16.
Under ADR-0063 the bump retires version 15 in the same PR: the eight `v15-` golden pairs move to `v16-` with their check values recomputed, one pair is added for kind 6, and the adversarial seeds move to version 16 with the seed that named kind 6 as unknown moved to kind 7.

## Consequences

An input with any delta structure pays one more trial encode: the benchmark cases encode 6 to 14% slower, less than kind 5's 24 to 31% because `pick` skips the trial where no stride beats identity (S2-A120).
Decode cost does not measurably change: a kind 6 frame adds one in-place reversal and one undo buffer, the same pair kind 1 and kind 5 pay.
Whole-buffer decode means a kind 6 frame holds its output and the delta's undo buffer at once, as kinds 3 and 4 do.

## Rejected alternatives

**A stride-wise byte reorder as its own kind.**
It recovers 97% of the whole-input reversal's gain on the synthetic generator and at most 0.5% on the real record data tried, so it adds a kind for the one class the generator built.

**A reversal flag on every existing kind.**
It doubles every shortlisted trial, where kind 6 adds one, and the only composition measured to win is reversal with delta.

**Gating the trial on a proxy.**
The reversed delta's residues are the forward residues read backwards and negated, so an n-gram entropy ranks them as the forward ones (S2-A119 on why no entropy sees reversal).
On `sqlite_like_records` the proxy differs by 0.002 bits between directions where the trial differs by 2.7%, so the trial decides.
