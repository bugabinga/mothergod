# ADR-0077: The ratio program bets on the model, not the parse

Status: accepted · Date: 2026-10-28 · Commissions issue #1763 · Prompted by the operator's question of 2026-10-28 (Telegram) and journal S2-R37

## Context

The shipped codec is a filter bank, an optimal-parse LZ and a six-expert context-mixing literal coder (journal S1-A1 to S1-A4, CLAUDE.md "What this project is").
On the 2026-10-26 Silesia pass it writes 2.005 b/B at 0.211 MB/s encode, 3.464 MB/s decode and 910 MiB peak; xz -4 writes 1.968 b/B at 4.6 MB/s, 82 MB/s and 50 MiB, and every xz level above -4 is smaller and faster both ways (`docs/benchmarks/silesia.md`).
On Canterbury the same codec writes the smallest output on the field, 1.347 b/B against xz -9e's 1.403 (`docs/benchmarks/canterbury.md`).

S2-R37 measured where the time goes: the decoder, which is the model alone, spends 86% of the cheapest encoder tier's time, so no parse change reaches under it; no encoder tier lands on the field's frontier.
Seven of the eight S1-P leads are resolved or exhausted, the journal's words being "pending a genuinely new mechanism and not a variant" (ROADMAP M3).
S1-L6 names the hybrid's cap: LZ consumes the repetition before the literal model sees it, and three match-signal mechanisms (S2-R20, S2-R22, S2-R23) lost on the leftovers.
Single-digit MB/s is the context-mixing class, where the literature's instances (lpaq, zpaq, paq8) land well below xz; this codec pays that class's time for LZMA's ratio.

The budget ran the other way.
Over the fourteen days to 2026-10-28 (`sessions roles --since 14d`, read 18:50 UTC) the director took 99 runs and $592 of the fleet's $894 projected, on the top rung, shipping the factory; the maintainer, the one seat on the journal since ADR-0062, took 177 runs at $91 on the floor rung, and `research/progress.jsonl` holds two modeling experiments in ISO weeks 42 to 44 against twenty capability patches.
Issue #897 suspended the director's thrift "until the agent-system backlog is down"; on 2026-10-28 the backlog is eleven enhancements, zero bugs, zero red checks, zero stalled PRs, and the suspension had outlived its condition.

## Decision

The hybrid's Silesia ceiling is accepted as measured.
The ratio program's live thread is a context-mixing path over every byte of the stream, a match model as an expert and no LZ tokens, commissioned as charter #1763 under ADR-0062 and worked by heartbeat duty 4c ahead of S1-P4's remainder.
Its verdict is the corpus policy's; a positive sealed verdict opens a wiring slice under its own ADR and format bump, and the hybrid is the codec of record until then and the fast levels after.

The product's niche is stated: the smallest output at single-digit MB/s, lossless-proof and deterministic across platforms.
The speed frontier is read against that class, xz's upper levels and the context-mixing field, never against zstd's.

The budget follows the bet.
The seat on the journal runs the strongest affordable rung, opus first and sonnet under thrift; the director's clock tick halves to every eight hours and its thrift block returns.
The values live in `agents/models.json` and the Telegram worker's `CLOCK`, under ADR-0031 and ADR-0027.

## Consequences

Heartbeat duty 4c is where the project's main work happens; bugs, blockers and red PRs still outrank it, and a charter outranks the journal's standing lead as ADR-0062 wrote.
The director's delta loop runs three times a day; alarms, operator events and Telegram dispatches wake it off the clock as before, and a stall waits up to eight hours for a rescue instead of four.
Fleet cost falls, in the same reading's projected list cost: every director run arrives by dispatch (the clock, Telegram and the alarm share that door), so the clock's share is not a column, and the arithmetic uses its ceiling, 84 ticks a fortnight falling to 42, about $250 at the director's $5.98 a run; the maintainer's 177 runs at $0.51 grow to about $0.80 at the per-token rate the curator pays on opus ($1.02 a run at 16k output tokens), about $50 more.
CLAUDE.md's architecture target line, the `Method` table in `docs/format/SPEC.md` and the M5 decode ratchet move with the wiring slice, not with this record; the M5 clause that a faster decode is a second codec path is untouched, because this path is the smaller and slower one.
A negative sealed verdict is a measurement: the direction is re-decided on it under a superseding ADR, and the two configuration values move back under the same levers.

## Rejected alternatives

**Keep working the hybrid's leads.**
The gap to xz -9e on Silesia is 8.8% of the codec's size (9.6% over xz's); S2-A128's reach shipped 0.3 to 2.4% per file and flipped no loss, and the journal's remaining headers ask for a new mechanism.

**Chase zstd's speed class.**
S2-A90 measured a static fast coder losing on ten of ten literal streams inside this format, S2-A88 on nine of ten whole buffers, and zstd exists.

**Re-seat a researcher.**
ADR-0062's escape hatch for thin slices is the maintainer's ladder, not a second seat on the same journal.
