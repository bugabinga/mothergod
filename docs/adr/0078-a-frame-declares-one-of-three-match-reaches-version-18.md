# ADR-0078: A frame declares one of three match reaches, up to 16 MiB; version 18

Status: accepted · Date: 2026-10-29 · Supersedes ADR-0075 · Applies ADR-0063 · Serves journal lead S1-P4 · `FORMAT_VERSION` 17 to 18

## Context

ADR-0075 let a frame declare a 2 MiB reach, `2^21 - 1`, the last distance the 21-symbol offset alphabet can name.
A `compress` block is 16 MiB, so repeats between 2 and 16 MiB back stayed invisible, and six of the seven Silesia losses to `zstd -19` sat on files larger than 2 MiB (`research/JOURNAL.md` S2-A128).
A wider reach needs a wider offset alphabet, and an adaptive model's probabilities depend on its alphabet size, so widening the one alphabet every frame uses would move every existing frame's bits for buckets none of them can use.
The measurement is in `research/JOURNAL.md` S2-A137.

## Decision

A `Method::Lz` payload's selector carries the reach in the top two bits of its `kind` byte, and the low 6 bits name the filter:

| code | reach | largest distance | offset alphabet |
|---|---|---|---|
| 0 | `Reach::Standard` | `2^20` | 21 |
| 1 | `Reach::Wide` | `2^21 - 1` | 21 |
| 2 | `Reach::Far` | `2^24 - 1` | 24 |
| 3 | rejected as `Corrupt` | | |

The offset alphabet is `floor(log2(window)) + 1` of the declared reach, for both the coder's offset models and the parse's price table.
A decoder rejects a distance past the declared reach as `Corrupt`, and its streaming ring holds the reach's bytes or the declared length, whichever is fewer.
The encoder picks by trial.
At the default level, input longer than 1 MiB adds one wide parse of the winning filter candidate, as under ADR-0075. Input longer than `2^21 - 1` bytes then adds one far parse of the same candidate.
Each wider body is kept only when strictly smaller than the body already kept.
`FORMAT_VERSION` becomes 18, and under ADR-0063 the bump retires version 17 in the same PR.

## Consequences

A standard or wide frame's body is byte-identical to version 17's, because its alphabet is unchanged; only the selector byte, version byte and check value move.
Where the far parse loses, the payload is the one a version 17 encoder wrote, so no input grows.
The cost is encode time: a third parse on input over 2 MiB, run after the wide one.
Peak encode memory rises little, because match finder state is per position in the block, not per distance.
A streaming decoder holds up to 16 MiB of ring for a far frame. A hostile short frame that declares the far reach costs only its declared length, because the ring is bounded by it.
The fastest two levels keep their version 17 reach choice. They never write a far frame.

## Rejected alternatives

**One 24-symbol alphabet for every reach.**
It is the simpler table, and it changes every narrow frame's bits.
A probe at `-2` that applied it moved `mr` +0.19% and `x-ray` +0.75% on Silesia, where a far parse finds nothing to gain (S2-A137).

**A far reach at the fast level, in place of the wide one.**
The same probe shows the far parse loses on some files when no trial guards it. The fast level has no trial to spend.

**An extra selector byte for the reach.**
The filter kinds fit 6 bits with room to spare, so two bits of the existing byte cost nothing.
