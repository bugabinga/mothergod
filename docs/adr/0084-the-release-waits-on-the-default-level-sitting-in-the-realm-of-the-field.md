# ADR-0084: The release waits on the default level sitting in the realm of the field

Status: accepted · Date: 2026-11-07 · Prompted by the operator's Telegram decision of 2026-11-07 (no crates.io release until compression speeds are in the realm of the competition; the roadmap runs to the end goal and stays current) · Supersedes ADR-0074's decode-speed clause · Commissions issue #1922

## Context

ADR-0074 kept decode speed outside milestone M5 and set its trigger as a user's ask on an issue they open; ADR-0080 kept the default level at `-3` on the reader's cost.
`ROADMAP.md` let release 0.1 (M6) gate on its own items alone, with the ratio ladder's standing published on /status and promised by no release.
Both decisions were made with the operator's question of 2026-10-18 on the record and no gate on speed.

The 2026-11-06 report pass (`docs/benchmarks/silesia.md`, `canterbury.md`, AMD EPYC 7763, one pass) reads the default level at 1.976 b/B, 0.132 MB/s encode and 2.447 MB/s decode on Silesia, against xz -9e at 1.829, 2.029 and 68.872; xz -4 is smaller and faster both ways.
On Canterbury the default writes the smallest output on the field, 1.347 against xz -9e's 1.403, at 0.249 and 3.606 MB/s against 1.728 and 51.711.
Every level of gzip, zstd and xz decodes between 56 and 586 MB/s on Silesia and between 48 and 92 on Canterbury; mothergod's `-1` to `-3` decode between 1.9 and 2.4 on Silesia and between 3.5 and 4.3 on Canterbury, and `-4` under 0.4 on both.

The decoder is the literal model: S2-R37 measured it at 86% of the cheapest encoder tier's time, so no parse change goes under it.
S1-P6 is exhausted within the current format (S2-A78: the per-byte cumulative rebuild does not distribute over a prefix sum; S2-A88 and S2-A90: tANS as an automatic method loses on ratio), and ADR-0074 named the way out as a second codec path and a format bump.
S2-A88 and S2-A90 judged ratio alone.

The frontier's two neighbours say whether a level is dominated, not by how much; Canterbury's default is undominated at an encode rate seven times under the field's slowest level.
"In the realm" needs a number a report can print.

## Decision

Release 0.1 waits on milestone M9, Speed, which ranks first in `ROADMAP.md`.

M9's bar: the default level holds rung 1 of the RATIO ladder, the aggregate under zstd -19 on each finals corpus, and its encode and decode MB/s are each at least half of xz -9e's in the same report pass, on both finals.
xz -9e is the reference because it is the slowest level the field ships; half, because a user weighing the field's slowest level against a smaller output accepts a factor of two and not an order of magnitude.
The bar is the operator's to move.
Its reading is a share the finals reports print and /status renders (issue #1921), so moving it is one number.

Decode speed is in scope: the trigger ADR-0074 set has fired, the operator's ask standing as the user's.
The route is the one ADR-0074 named, a codec-of-record change for the default level under its own ADR and `FORMAT_VERSION` bump, worked as charter #1922 under ADR-0062 ahead of the ratio program's live thread, because the release waits on it and a program never finishes.
S2-A88 and S2-A90 may be re-run under the changed condition, the objective being rate at rung 1 rather than ratio alone (CLAUDE.md "What this project is").
`-4` is untouched (ADR-0080): the bar binds the default level, and a slow level may stay slow where the reader is told.

The roadmap runs to the end goal: M10, Release 1.0, is the last deliverable, where ADR-0050's decode-forever promise starts; the ratio and users programs continue past it, and no later milestone is named until evidence names it.
The weekly deep survey holds the roadmap to the merge log and the tracker under the checks its skill names.

## Consequences

#537 and #644 stay `blocked-on-human` with a changed ask: after M9 closes.
Heartbeat duty 4c's top item is #1922; the ratio program's live thread (ADR-0077's model thread at `-4`, S1-P4) waits behind it or rides idle budget.
Rung 1's room may be spent for the bar, 0.020 b/B on Silesia and 0.122 on Canterbury on the 2026-11-06 pass; a default that loses rung 1 is out of bounds.
The wiring slice bumps the format and retires its predecessor (ADR-0063), and the decode ratchet's constant moves with the trade stated (#907).
The herald's surface moves (issue #1923): speed, not ratio, heads the roadmap, and the pre-alpha paragraph names the bar.
The `-4` ratio thread, 15% under `-3` on Canterbury across versions 19 to 22, slows to what the budget leaves after #1922.

## Rejected alternatives

**Make `-1` the default.**
It encodes Silesia at 1.962 MB/s, near xz -9e's rate, and loses rung 1 at 2.361 b/B against zstd -19's 1.997 while decoding at 2.205 MB/s, thirty times under xz -9e.
A level switch moves the encode number and not the decoder.

**Ship 0.1 now behind the README's "It is slow" paragraph and let speed follow.**
The operator's call, made the other way.
A 0.1 nobody runs for its speed buys no USERS evidence and freezes nothing a user would keep.

**Define the realm as "on the frontier".**
Undominated is not in the realm: the Canterbury default qualifies today at an order of magnitude under the slowest competitor on both axes.
