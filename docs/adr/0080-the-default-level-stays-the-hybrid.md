# ADR-0080: The default level stays the hybrid

Status: accepted · Date: 2026-11-05 · Applies ADR-0077 and ADR-0079 · Serves issue #1870

## Context

ADR-0079 wired the context-mixing path at `-4` alone and left the default's choice to a later encoder-only decision on the finals read.
ADR-0077 had anticipated the path as the codec of record, with the hybrid kept for the fast levels, and named the product's niche as the smallest output at single-digit MB/s.

The first report pass with a `-4` row reads both levels on one machine (`docs/benchmarks/silesia.md` and `canterbury.md`, frontier sections, bench-report run 41994577267 at d2c0a92, AMD EPYC 7763, one pass):

| corpus | level | b/B | encode MB/s | decode MB/s | peak MiB |
|---|---|---|---|---|---|
| Silesia | `-3` | 1.976185 | 0.134 | 2.363 | 951.7 |
| Silesia | `-4` | 1.736011 | 0.043 | 0.163 | 951.7 |
| Canterbury | `-3` | 1.347100 | 0.251 | 3.517 | 56.6 |
| Canterbury | `-4` | 1.143402 | 0.055 | 0.356 | 81.9 |

`-4` is 12.2% smaller on Silesia and 15.1% on Canterbury, and both aggregates sit under xz -9e's.
On Silesia `-3` is off the frontier: xz -4 is smaller and faster both ways.

The writer picks the level and the reader pays the decode.
In the same reports, decode speed across every level of gzip, zstd and xz varies at most 1.44x within one tool (zstd on Silesia), and across mothergod `-1` to `-3` at most 1.17x on Silesia and 1.21x on Canterbury.
`-4` decodes 14.5x slower than `-3` on Silesia and 9.9x on Canterbury: Silesia's 212 MB take about 22 minutes to decompress instead of 90 seconds.

## Decision

The default level stays `-3`, the hybrid.
The path stays opt-in at `-4`.

The reason is the decode trade: a default that writes `Cm` frames charges every reader of every default file 10x to 15x the decode time, a cost no level of gzip, zstd or xz imposes, and `-4` decodes below 1 MB/s on both finals, outside the niche ADR-0077 named.
The ratio gain stays available to every writer who asks for it.

## Consequences

`speed::BASELINE_DECODE_COST` and every default frame are untouched.
The default stays dominated by xz -4 on Silesia; closing that is the ratio program's work at `-3`, not a level switch.
Where a smaller file outweighs the reader's time, the CLI help and `Level::Extreme`'s doc already name `-4` and its decode cost.
The choice is re-decided under a superseding ADR when a report pass reads `-4` at 1 MB/s decode or faster on both finals, or when a user asks for the switch on an issue.

## Rejected alternatives

**Switch the default to `-4`.**
It is the only level on the Silesia frontier and the smallest output on both finals, and a default user would get the codec's best ratio without knowing to ask.
It loses on the reader's cost, above, which a ratio figure does not price.

**Decide after a per-file `-4` read.**
Whether `-4` wins or ties every Silesia file against zstd -19 is unread, but it moves the RATIO ladder, not the decode cost that decides this.
