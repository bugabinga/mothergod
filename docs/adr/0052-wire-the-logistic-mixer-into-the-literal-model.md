# ADR-0052: Wire the logistic mixer into the literal model

Status: accepted · Date: 2026-09-26 · Resolves `JOURNAL` S1-P8 (in full) · `FORMAT_VERSION` 4 → 5

## Context

`research/JOURNAL.md` S1-P8 is ROADMAP M3's eighth standing lead: a
logit-domain ("logistic") mixer over the same six expert banks
`literal::Literal`'s shipped linear mixer already reads, following the
PAQ-family `stretch`/blend/`squash` design (Mahoney 2005) instead of the
shipped exponentiated-gradient linear blend. Every prior slice
(S2-R20 through S2-A101) measured this idea as an ideal-cost pairing,
never driving a real `Encoder`/`Decoder`: `literal::LogisticMix` held its
own weights, step counts and `Sse` table, and
`Literal::ideal_cost_bits_logistic_pair` priced a byte through it and
through the shipped `ideal_cost_bits_sse` side by side, from the same
pre-update banks, so the two never diverged from measurement noise. The
last slice, S2-A101, accepted an annealed per-key learning rate
(`rate(n) = LOGISTIC_FLOOR_RATE + (LOGISTIC_INITIAL_RATE -
LOGISTIC_FLOOR_RATE) / (1 + n * LOGISTIC_RATE_DECAY)`) over S2-R25's flat
rate, on `bench::baseline`'s 11 train cases and both sealed-only kinds,
naming its own remaining scope: "the wiring slice (real-bitstream
measurement, `FORMAT_VERSION` bump, ADR, golden fixture, decode-path
determinism of `ln`/`exp`, speed)."

`ln`/`exp`/`stretch`/`squash` already satisfy decode-path determinism
before this ADR: `src/logistic.rs`'s `ln` is built from IEEE-754 basic
operations and bit manipulation only (no libm call), `squash` reuses
`literal::exp`, itself already vendored for the shipped linear mixer's
own gradient step (ADR-0024). No new determinism work was needed to
reach this ADR; S2-A101's own apparatus was already built decode-path
clean.

## Decision

**1. `Literal::encode_logistic`/`decode_logistic` code a literal byte
through `LogisticMix` instead of `Literal::encode_sse`/`decode_sse`, for
every candidate except `Candidate::Transpose`.** Both share one walk
(`Literal::logistic_code_bit`, the `LogisticMix` counterpart of
`bittree::walk_sse`): each bit-tree node stretches the six real experts'
current bank estimates, blends them under `logistic`'s own
`weight_index`-keyed vector, squashes back, refines through `logistic`'s
own `Sse` table (independent of `Literal::sse`, the same separation
ADR-0046 already keeps for the column expert's own table), codes or
decodes the bit, then takes one gradient step on `logistic`'s weights at
`logistic_rate(steps, LOGISTIC_RATE_DECAY)`. After the walk, both call
`Literal::update` exactly as `encode_sse` does: the six real experts'
banks and the shipped linear mixer's own weights adapt unperturbed,
never touched by the logit-domain mix, the same layering ADR-0046 already
established for the seventh, column-keyed expert. `decay` is no longer a
parameter: `LOGISTIC_RATE_DECAY` is now the one fixed constant every
byte codes under, S2-A101's own train-optimum.

**2. `FORMAT_VERSION` bumps to 5, gated on version alone (not
candidate).** `codec::Models` gains a `logistic: LogisticMix` field,
constructed alongside `literal` (`Models::new`/`try_new`, the latter
using `LogisticMix::try_new`, added for this decode path per hard rule
2). `codec::encode_tokens`'s `EncodeSink` and `codec::decode`'s `VecSink`
now pick `encode_logistic`/`decode_logistic` over
`encode_sse`/`decode_sse` for any candidate whose `column` state is
`None` (compression always targets `FORMAT_VERSION`, so `EncodeSink`
picks unconditionally; `decode` gates on `version >=
codec::LOGISTIC_MIN_VERSION`, 5). A `Candidate::Transpose` frame is
unaffected at every version: `COLUMN_EXPERT_MIN_VERSION`'s gate still
selects `encode_column`/`decode_column` at version 4 and above,
regardless of `LOGISTIC_MIN_VERSION` — the logit-domain mix does not yet
reach the seventh, column-keyed expert, a separate lead, not this one's
scope. `codec::decode_to_writer`'s streaming path
(`decode_undoable_streaming`, `StreamingSink`) gates the same way: it
never carried a version parameter before this ADR because every version
it decoded shared one literal path, so this decision threads `version`
through it, matching `decode`'s own gate.

**3. `codec::CostSink`'s literal pricing (ROADMAP M2's ideal-cost
accounting) switches from `ideal_cost_bits_sse` to a new
`Literal::ideal_cost_bits_logistic`.** `TokenSink`'s own contract
(`codec.rs`'s module docs) requires `CostSink` and `EncodeSink` to price
and code the same thing; every `CostSink` caller here parses raw data
with no filter selection, so `Candidate::Transpose`'s `encode_column`
path never arises and `ideal_cost_bits_logistic` alone keeps the
contract. `ideal_cost_bits_logistic` mirrors `ideal_cost_bits_sse`
exactly: sums `-log2` of each node's refined probability through the
same `logistic_code_bit` walk, then updates state identically to
`encode_logistic`.

**4. The now-superseded ideal-cost-pairing apparatus is deleted in this
PR, not left for a later deslop.** `Literal::logistic_cost_bits` and
`ideal_cost_bits_logistic_pair`, and `codec::ideal_cost_bits_logistic_mix_experiment`/
`_at`, are gone: the code a wiring slice ships must be the code that
produced the recorded number, and a private measurement-only candidate
with no caller is dead code the lint gate refuses
(`compression-experiment` skill). Their dedicated tests are replaced by
round-trip and state-parity tests on `encode_logistic`/`decode_logistic`,
the same shape ADR-0046's own `encode_column` test suite already takes.

## Consequences

`research/JOURNAL.md` S1-P8 closes: the logistic mixer is wired into a
real bitstream, no longer ideal-cost-only. `tests/golden/v5-lz-repeated-text`
(the same plaintext as `v3-lz-repeated-text`, re-encoded — the real
encoder selects `Candidate::Delta(45)`, still non-`Transpose`, so this
fixture exercises `encode_logistic`) pins the new path; every version-3
and version-4 fixture stays committed, decode-only, forever
(ADR-0041/ADR-0050: no version already written is ever retired).

Real-bitstream measurement (`mothergod::compress`, this run's sandbox,
`bench/baseline.json`'s 11 train-tier cases plus both sealed-only kinds):
see `research/JOURNAL.md`'s closing S1-P8 entry for the full per-case
numbers. Net effect matches S2-A101's own ideal-cost reading in sign and
rough magnitude on every case tried; the literal-heavy cases (`interleaved_audio16`,
`x86_dense_code`, the record-format kinds) improve, `entropy_ladder_h6`
regresses by the same small margin S2-A101 already named as this
schedule's own richness tax.

## Rejected alternatives

**Gate the logistic path on candidate as well as version, the way
`COLUMN_EXPERT_MIN_VERSION` gates the column expert.** Rejected: unlike
the column expert (additive, keyed on a per-candidate parameter that only
`Candidate::Transpose` carries), the logistic mixer reads nothing
candidate-specific — it is a straight replacement for the plain
six-expert SSE path, applicable to every candidate that path already
served. Gating it on candidate too would need a reason to exclude some
candidate from the improvement measured across all of them, and none of
S2-A101's own measurement gave one.

**Blend the logit-domain mix into `Candidate::Transpose`'s seven-expert
mix too, in this same PR.** Rejected as out of scope: S2-A101's own
apparatus, and every slice before it, measured the six-expert case only;
folding the seventh, column-keyed expert into a logit-domain blend is an
uncharacterized interaction (the same kind ADR-0046's own SSE-interaction
question was, for the linear mixer), a future lead's question, not a
straight port of an already-measured number.
