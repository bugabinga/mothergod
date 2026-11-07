# ADR-0082: The `Cm` model adds record and sparse contexts and a calibration stage; version 21

Status: accepted · Date: 2026-11-07 · Applies ADR-0079 and ADR-0063 · Serves ADR-0077's live thread · `FORMAT_VERSION` 20 to 21

## Context

ADR-0081 left `cm_path::CmPath` with the inputs lpaq1 hashes for text: orders 2 to 6, a word and a word pair.
Its mixed probability went to the coder uncalibrated, and nothing in it read data laid out in fixed-width records or sample columns, which paq8 models with a record model and sparse contexts and lpaq1 refines with an APM stage.
Priced over every byte of `bench/`'s train cases in one loop session, four calibration tables, a record column and row context, three sparse contexts and a faster mixer step lower the path's mean by 0.0806 b/B, the sealed kinds by 0.079 (`access_log`) and 0.061 (`gradient_image`); a second mixer selected by tree node, a lower counter limit, a text column context, a slower mixer step and both neighbouring match minimum lengths each priced worse (`research/JOURNAL.md` S2-A151).
A `Cm` frame's bits are the model's probabilities, so any change to its inputs changes what `decode` must compute.

## Decision

`CmPath` mixes twelve hashed contexts after the literal and match inputs, in this order: orders 2 to 6; the current word with the byte before it; the bytes one and two records above; the byte one record above with the last byte; the previous word with the current one; bytes 3 and 2 back; bytes 2 and 4 back; bytes 4 and 8 back.
The record length is the latest gap above one at which some byte value recurred three times running, kept as running state updated once per byte.
The mixer's step is 0.007.
Four calibration tables refine the mixed probability `p`, each keyed by the bit's tree node and, beside it, nothing, the last byte, eight bits of the order-2 hash, or the match model's state; the coded probability is `((p + 3 a0) / 4 + a1 + a2 + a3) / 4`, clamped to the floor.
Every table has a constant size and starts fresh per frame.

`FORMAT_VERSION` becomes 21, and under ADR-0063 the bump retires version 20 in the same PR.

## Consequences

`-4` frames that carry `Cm` get smaller and none grows on `bench/`'s cases; `-1` to `-3` frames change only their version byte and check value.
A `Cm` decode does about 80% more work per byte, measured in the speed gate's calibration unit; the default level's decode cost is unchanged, so the speed ratchet does not move.
A hostile `Cm` frame at the 16 MiB cap decodes 79% slower in wall time, 115 s to 205 s on one runner, still under `Lz`'s worst case; `cm_path::MAX_OUTPUT_LEN` holds the measurement.
The model's tables grow by five 4 MiB context tables and about 37 MB of calibration bins, still constant and allocated fallibly; `-4`'s encode peak stays the hybrid trial's, which ends before the `Cm` trials start.
A file `-4` wrote under version 20 no longer decodes and must be re-compressed, which no release has had to promise against (ADR-0050).
