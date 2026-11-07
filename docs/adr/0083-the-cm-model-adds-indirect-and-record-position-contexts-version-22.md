# ADR-0083: The `Cm` model adds indirect and record-position contexts and a seen-order selector; version 22

Status: accepted · Date: 2026-11-07 · Applies ADR-0079 and ADR-0063 · Serves ADR-0077's live thread · `FORMAT_VERSION` 21 to 22

## Context

ADR-0082 left `cm_path::CmPath` with twelve hashed contexts, one stretched input each, a mixer whose weights were selected by match length and nibble, and four calibration tables.
paq8l feeds each context map to its mixer twice, as a logit and in the linear domain, beside a bias, selects mixer weights by how much context the hashed orders hold, and models what followed a byte's earlier occurrences (its indirect model) and where in a record a byte sits.
Priced over every byte of `bench/`'s train cases in one loop session, those mechanisms and a few neighbours lower the path's mean by 0.0131 b/B, the sealed kinds by 0.053 (`access_log`) and 0.075 (`gradient_image`); a second match model with a 24-byte minimum, other linear-input scales, a sparse context of bytes 1 and 3 back, a slower mixer step, a distance model, digits inside words, a column context with the last byte, and dropping ADR-0082's record column each priced worse or no better (`research/JOURNAL.md` S2-A152).
A `Cm` frame's bits are the model's probabilities, so any change to its inputs changes what `decode` must compute.

## Decision

`CmPath` mixes twenty hashed contexts after the literal and match inputs, in this order: orders 2 to 6 and 8; ADR-0082's word, record column, record row, word pair and three sparse contexts; the last byte with the two bytes that followed its last two occurrences; the same for the last two bytes; the last four bytes' high nibbles; the same indirect context for the last three bytes, hashed to a 2^16-entry table; the position within the current record length, with one record above, alone, and with one and two records above.
Each context enters the mixer as its stretched probability, then all of them again as `2 (p - 1/2)`, then a constant bias of 1.
The mixer's weight set is chosen by the longest order whose slot at the node has learned a bit (seven classes), the match length's class (none, 1 to 15, longer) and the nibble.
A fifth calibration table, keyed by that longest order beside the node, joins the blend: `((p + 3 a0) / 4 + a1 + a2 + a3 + a4) / 5`.
The mixer's step is 0.005.
Every table has a constant size and starts fresh per frame.

`FORMAT_VERSION` becomes 22, and under ADR-0063 the bump retires version 21 in the same PR.

## Consequences

`-4` frames that carry `Cm` get smaller on `bench/`'s record, sample, text and code cases; three iid or low-order cases grow by 1 to 4 bytes of 50,000, and a 3,000-byte golden input grows 14 bytes, the richer model's warm-up cost (JOURNAL S1-L4). `-1` to `-3` frames change only their version byte and check value.
A `Cm` decode does about 29% more work per byte, measured in the speed gate's calibration unit; the default level's decode cost is unchanged, so the speed ratchet does not move.
A hostile `Cm` frame at the 16 MiB cap decodes 27% slower in wall time, still under `Lz`'s worst case on the figures at hand; `cm_path::MAX_OUTPUT_LEN` holds the measurement.
The model's tables grow by eight 4 MiB context tables, 256 KiB of indirect history and a small calibration table, still constant and allocated fallibly.
A file `-4` wrote under version 21 no longer decodes and must be re-compressed, which no release has had to promise against (ADR-0050).
