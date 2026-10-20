# ADR-0075: A frame declares its match reach, 1 MiB or 2 MiB; version 17

Status: accepted · Date: 2026-10-20 · Applies ADR-0063 · Serves issue #1342 · `FORMAT_VERSION` 16 to 17

## Context

Every Silesia file exceeds the 1 MiB match window and 7 of 12 lost to `zstd -19`; all 11 Canterbury files sit under it and win (issue #1342).
A blanket 2 MiB reach shrank 11 of 12 Silesia files by 0.02 b/B on the token stage, and lost on the sealed set and on `interleaved_audio16` (`research/JOURNAL.md` S2-R35).
No proxy gate separates the two: S2-R9 and S2-R11 each found a detector that fired where the far matches lost.
The decoder rejected any distance past `lz::WINDOW`, and the streaming ring was `WINDOW` bytes, so a 2 MiB reach was a format change.

## Decision

A `Method::Lz` payload's selector carries the reach in the top bit of its `kind` byte: clear is `Reach::Standard`, distances up to 1 MiB; set is `Reach::Wide`, distances up to `2^21 - 1`, the last distance the offset code's top bucket can name.
A decoder rejects a distance past the declared reach as `Corrupt` and sizes the streaming ring to it: 1 MiB for a standard frame, `2^21 - 1` bytes for a wide one.
A decoder with the bit set reads the remaining 7 bits as before; a kind with no candidate stays `Corrupt`.
The encoder picks by trial.
For input longer than `lz::WINDOW` it encodes the winning filter candidate once more at `Reach::Wide` and keeps that body only when strictly smaller; a tie, and every input of 1 MiB or less, stays standard.
`FORMAT_VERSION` becomes 17.
Under ADR-0063 the bump retires version 16 in the same PR: the nine `v16-` golden pairs move to `v17-` with their check values recomputed, one pair is added for a wide frame, and the adversarial seeds move to version 17.

## Consequences

The sealed set and any input where the wide parse loses are byte-identical to a standard encode, so the gate holds by construction instead of by a detector.
The cost is encode time: the second parse runs after the filter trials, 1.3x to 1.5x wall-clock on Silesia files over 1 MiB (`research/JOURNAL.md` S2-A127).
A decoder streaming a wide frame holds 2 MiB of ring where a standard frame holds 1 MiB; whole-buffer decode is unchanged.
A hostile frame can declare the wide reach for the price of one bit, which costs it a fixed 2 MiB and no more, because the declared length still bounds the output.
Match finder state is per position in the block, not per distance, so the wide reach adds no encoder memory.

## Rejected alternatives

**A detector that turns the wide reach on.**
Three detector designs failed on three data classes (S2-R9, S2-R11, S2-A95); the trial measures what a detector guessed.

**Wide reach for every frame.**
It loses on `interleaved_audio16` and both sealed kinds (S2-R35), and the sealed set is the guard.

**A reach past `2^21 - 1`.**
It needs a wider offset code; it is a format change of its own and unmeasured (S2-R35 "Not measured").
