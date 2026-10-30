# ADR-0079: A frame may code every byte through one model, at level 4; version 19

Status: accepted · Date: 2026-10-30 · Applies ADR-0077 and ADR-0063 · Serves issue #1775 · `FORMAT_VERSION` 18 to 19

## Context

ADR-0077 commissioned a context-mixing path over every byte, a match model among its inputs and no LZ tokens, and made a positive sealed verdict the trigger for a wiring slice under its own format bump.
The path, `cm_path::CmPath`, codes through the range coder and passed: on `bench/`'s train cases its whole frames are 0.226 b/B smaller than `compress`'s on average and both sealed kinds about 0.15 b/B smaller (`research/JOURNAL.md` S2-A133 to S2-A142).
Its decode costs 6.7x to 6.9x the hybrid's on the speed gate's cases (S2-A141), because it models every byte where the hybrid copies matches.
It has not been read on the finals, where blocks are 16 MiB and its order tables are sized for 50 KB cases (S2-R39).

## Decision

Method byte 2, `Cm`, names a payload of one filter selector, the filtered length, the coded length and one range-coded stream of the filtered bytes through a fresh `CmPath` (`docs/format/SPEC.md`).
The selector is the `Lz` payload's filter selector without a reach: the path has no match window, so a reach code is `Corrupt`.
Every invariant the format binds on a method holds: the declared output is checked against the ceiling before any allocation, the Stored floor holds from the header, and every table allocates fallibly at a constant size.

Only the new level `-4` (`Level::Extreme`) writes it.
That level runs the default level's search, then trial-codes the path behind every shortlisted filter, and keeps a `Cm` frame only when it is strictly smaller than the `Lz` one.
The default stays the hybrid, so the decode-speed ratchet and every default frame are untouched.

`FORMAT_VERSION` becomes 19, and under ADR-0063 the bump retires version 18 in the same PR.

## Consequences

A user who asks for the smallest output gets it on data the path models better, and pays for it in decode time on exactly those frames; where the hybrid wins, `-4` writes `-3`'s frame.
Version 18 frames re-encode at `-3` byte-identical but for the version byte and check value.
A `Cm` frame decodes whole-buffer: its match model reads anywhere in the output so far, so `decompress_to_writer` never streams one.
A hostile `Cm` frame can demand decode work up to `codec::MAX_DECODED_LEN`, bounded by its stream's length at 4,096 output bytes per stream byte, at the path's per-byte cost, several times the `Lz` worst case per output byte.
The default switch is a later encoder-only decision on the finals read; it needs no format change, and it moves the decode ratchet then, under ADR-0077.

## Rejected alternatives

**The path at the default level, as ADR-0077 anticipated.**
Every default user would pay the decode cost before the finals measure what they gain, and the speed gate's constant would move on synthetic cases alone.

**A filter-less payload.**
Filters in front of the path cut its train mean by 0.176 b/B against the raw path (S2-A135); dropping the selector would forfeit that for two bytes.
