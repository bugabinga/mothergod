# ADR-0081: The `Cm` model adds order 2 and two word contexts; version 20

Status: accepted · Date: 2026-11-07 · Applies ADR-0079 and ADR-0063 · Serves ADR-0077's live thread · `FORMAT_VERSION` 19 to 20

## Context

ADR-0079 put `cm_path::CmPath` in a frame with the inputs its charter found: the `Lz` literal model, a match model and hashed orders 3 to 6.
lpaq1, the design the path follows, also hashes order 2 and a word context; paq8 adds a pair of words.
Priced over every byte of `bench/`'s train cases, those three inputs lower the path's mean by 0.0103 b/B and both sealed kinds by about 0.026; an order-1 table and dropping order 5 each priced worse (`research/JOURNAL.md` S2-A150).
A `Cm` frame's bits are the model's probabilities, so any change to its inputs changes what `decode` must compute.

## Decision

`CmPath` mixes seven hashed contexts after the literal and match inputs: orders 2 to 6, then the current word with the byte before it, then the previous word with the current one.
A word is a maximal run of ASCII letters, case folded; the word pair skips the non-letters between its words.
Both word contexts are kept as running hashes updated once per byte, so a walk costs the same however long a word runs.
Every new table has the existing constant size and starts fresh per frame.

`FORMAT_VERSION` becomes 20, and under ADR-0063 the bump retires version 19 in the same PR.

## Consequences

`-4` frames that carry `Cm` get smaller; `-1` to `-3` frames change only their version byte and check value.
A `Cm` decode does about 18% more work per byte, measured in the speed gate's calibration unit; the default level's decode cost is unchanged, so the speed ratchet does not move.
The model's tables grow by two 4 MiB order tables, still constant and allocated fallibly.
A file `-4` wrote under version 19 no longer decodes and must be re-compressed, which no release has had to promise against (ADR-0050).
