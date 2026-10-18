# ADR-0074: The parse engine is milestone M5

Status: accepted · Date: 2026-10-18 · Prompted by the operator's Telegram question of 2026-10-18 (when to tackle speed, ratio on large files and memory) and the steering that followed it (comparisons to the field at every level)

## Context

`ROADMAP.md` carried M5 as a program, Speed tiers: the SPEED line, "tracked, not yet optimized", with no finish line, and under ADR-0047 no tracker milestone and no items.
The heartbeat's ladder ranks milestone items, and on 2026-10-18 M7 and M8 were down to one open item each, so the ladder's next product work was deslop grade (#1339).

The operator asked when to tackle performance, optimality and memory.
Measured on the crate, the three are one mechanism.
The optimal parse holds about 42 bytes of state per input byte (`src/lib.rs`, `BLOCK_LEN`'s doc; JOURNAL S2-A116), which sets the 16 MiB block at about 700 MB.
The block bounds the parallel encode: four blocks live is about 2.8 GB against the 1 GiB `memory-check` pins, so the 2.0x measured from threads over blocks (S2-A122) is unwired (#1322).
Encode runs at 0.104 MB/s single-thread on `dickens`, 83% of it the optimal parse (S2-A121).
Every Silesia file exceeds the 1 MiB `lz::WINDOW` and 7 of 12 lose to zstd -19, while all 11 Canterbury files, each under it, win (`docs/benchmarks/`, 2026-10-16): a correlation S1-P4 names and no finals run has measured.

The finals compare the field at one level each, `gzip -9`, `zstd -19`, `xz -9e`, by size alone (`bench/src/reference.rs`).
A speed or memory claim has nothing beside it: mothergod's 0.040 MB/s Silesia encode sits in a table with no competitor rate.

Decode speed is a different case.
S1-P6 is exhausted at every slice the format allows: bit-decomposed coding shipped under the floor, tANS loses on bits/byte (S2-A88, S2-A90), SIMD is closed on decode (ADR-0053).
A faster decode is a second codec path and a format bump.

## Decision

M5 is a milestone, "M5: Parse engine", with a GitHub Milestone (ADR-0047), at the rank the program held: after M7 and M8, before M6.

Its definition of done is four items in order, each an issue in the milestone:
measure the field at every level, ratio with encode and decode MB/s and peak RSS on the finals, mothergod one row per level it offers, published and tracked on /status (#1340, with #1324);
budget the parse state per position and cut it, encoder-only (#1341);
spend the headroom twice, threads over blocks and trials under 1 GiB (#1322), then the window or block where a file has long-range structure, read once on the finals (#1342).
Measurement ranks first: a win the field cannot place is a number without a comparison (CLAUDE.md rule 4).

The SPEED scorecard line becomes position on the field's frontier: per corpus, the fastest competitor level that compresses at least as well as mothergod, and the best-compressing level that is at least as fast.
Peak memory rides the same table rather than a fifth outcome metric, because one measurement pass produces both and one row on /status reads them.
A level mothergod ships later is a row in that table, so the comparison holds for every level mothergod ever offers.

Decode speed stays outside M5.
Its trigger is a user's ask on an issue they open; no agent issue is filed, because an issue the ladder must not pick is a trap the ladder walks into.
Until then decode holds its ratchet (`baseline_gate check`, #907).

## Consequences

The heartbeat finds ranked speed and memory work once M7 and M8 close or block, and the status page shows M5 with item counts instead of "ongoing".
The bench-report pass grows by the competitor matrix at every level, tens of minutes serial on a runner and none in a session; its timeout moves with it.
"Speed tiers" leaves the roadmap as a name: a tier is one possible outcome of the parse engine work, not its definition.
ADR-0047's program and milestone kinds stand; M5 changes kind under them, and M3 remains the only program.

## Rejected alternatives

Leaving M5 a program and filing the three as plain issues: the ladder ranks by milestone and a program has no rank, which is the symptom #1339 reports.
Starting at decode: its same-format slices are exhausted, and the parse's cost is the product's ratio, so the encoder is where one change moves two axes.
A separate MEMORY metric: a second reading of the same table; it splits off the day memory moves without speed.
