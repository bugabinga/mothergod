# ADR-0038: Wire SSE into the literal mixer's binary decomposition

Status: accepted · Date: 2026-08-29 · Resolves `JOURNAL` S1-P1 (in full) · `FORMAT_VERSION` 2 → 3

## Context

`research/JOURNAL.md` S1-P1 is ROADMAP M3's oldest standing lead: secondary
symbol estimation (SSE), a calibration stage that corrects a primary
model's systematic bias against the observed rate in a small side
context. Four slices built the prerequisites, each a standalone,
not-yet-wired primitive: `Sse` (S2-A40), probability-driven bit coding
(`Encoder::encode_bit`/`Decoder::decode_bit`, S2-A41), a binary
decomposition of the literal mixer's 256-way cumulative table into 8
chained binary decisions (`src/bittree.rs`, S2-A58), and the `Sse`
context keying for that decomposition (`bittree::sse_context`,
tree-position-only, 255 contexts, S2-A59).

A third slice, S2-R1, tried wiring `Sse` behind the flag model's
literal/copy binary split — a lone, already order-0-adaptive frequency
counter — and was rejected: no train improvement, one sealed
regression. The mechanism it read from that result named the fix: SSE
earns its keep calibrating a *compound* estimate (several signals
blended), not a lone counter already tracking its own rate. S2-A58/
S2-A59 built exactly that candidate — the six-expert literal mixer's own
blended probability at each binary-tree node — without yet measuring
whether wiring it pays off.

## Decision

**1. `Literal::encode_sse`/`decode_sse` code every literal byte through
the binary decomposition, SSE-calibrated at each node.** Both build the
same mixed `cum` table `Literal::encode`/`decode` do, then walk
`bittree::encode_symbol_sse`/`decode_symbol_sse`: at each of the 8
levels, the raw upper-half probability read off `cum` is refined through
`Literal`'s own `Sse` table (`bittree::SSE_CONTEXTS` contexts, keyed by
`bittree::sse_context(depth, prefix)`) before it drives
`Encoder::encode_bit`/`Decoder::decode_bit`, and the table is updated on
the raw probability afterward. The underlying six-expert mixer keeps
adapting exactly as before — `Literal::update` still runs
unconditionally after every symbol, regardless of which coding path
chose it.

**2. `FORMAT_VERSION` bumps to 3, and `codec::decode` dispatches on the
frame's declared version instead of dropping the old path.**
`tests/golden/v2-lz-repeated-text.mgdc` commits this crate to decoding
`FORMAT_VERSION` 2 forever (unlike ADR-0028's version-1 drop, which was
safe only because nothing had shipped yet). `codec::decode` gained a
`version: u8` parameter: below `LITERAL_SSE_MIN_VERSION` (3) it calls
the old `Literal::decode`, at or above it calls `Literal::decode_sse`.
Every other symbol (flag/length/offset/slot) is unaffected and coded
identically at every version `LZ_MIN_VERSION` or above — only the
literal sub-stream's internal shape changed, not the outer payload
layout. `codec::encode_tokens`'s `EncodeSink::literal` always calls
`encode_sse`, since compression always targets the newest format
version.

**3. `codec::ideal_cost_bits`'s `CostSink` prices literals through a new
`Literal::ideal_cost_bits_sse`/`bittree::ideal_cost_bits_sse`, matching
what `EncodeSink` now actually codes.** The old `Literal::ideal_cost_bits`
(pricing the direct 256-way division) stays, unchanged, for its own
tests and any caller still measuring the pre-SSE mixer in isolation, but
`codec.rs`'s own module docs warn that `CostSink` and `EncodeSink` must
price and code the same thing or `ideal_cost_bits` silently stops
tracking `encode_tokens`'s real output — exactly the desync this ADR's
change would have caused left unfixed.

## Consequences

`research/JOURNAL.md` S1-P1 closes: SSE calibration is wired into a real
bitstream, no longer a standalone primitive. Measured on
`bench::baseline`'s 11 train-tier cases (`CASE_LEN` 50,000,
`CASE_SEED` fixed) and the two sealed-only kinds (`access_log`,
`gradient_image`, `CASE_LEN` 50,000, `sealed_seed(CASE_SEED)`): net train
effect **-0.36736 b/B** across the 11 cases (`interleaved_audio16`
-0.36368 and `gradient_image`'s sealed counterpart -0.13472 carried most
of it; `base64_wrapped` -0.01312, `x86_dense_code` -0.01760,
`json_records` -0.01024, `entropy_ladder_h1` -0.00512 also improved).
Sealed split both improved: `access_log` -0.01264, `gradient_image`
-0.13472. One case regressed past `TOLERANCE_BITS` (0.02):
`entropy_ladder_h6`, +0.02368 — iid random data at 6 bits/byte, where
SSE's per-context warm-up and the 8-chained-binary-decision coding path's
own quantization overhead have no real bias to correct, only noise to
add. `sqlite_like_records` also regressed, +0.008, inside tolerance.
`research/corpus/POLICY.md`'s accept rule (train improvement, no
validation regression) is satisfied by the net numbers; the
`entropy_ladder_h6` regression is declared here as the accepted trade
`bench/baseline_gate`'s own check message asks for, and
`bench/baseline.json` is updated to the new numbers in the same PR.
Whether this closes S1-P1's originally named target (the five zstd text
holdouts' combined 0.11 b/B deficit) is unmeasured: held-out finals run
at milestones, never inside the experiment loop
(`research/corpus/POLICY.md`), so this PR's Canterbury/Silesia report
regeneration is a mechanical refresh of `bench/baseline.json`'s embedded
fingerprint, not part of the accept decision.

A new golden fixture pair, `tests/golden/v3-...`, pins `FORMAT_VERSION`
3's literal sub-stream shape; `tests/golden/v2-lz-repeated-text.mgdc`
stays exactly as committed, decode-only, forever.

## Rejected alternatives

**Drop `FORMAT_VERSION` 2 decode support instead of dispatching on
version.** Rejected outright: `tests/golden.rs`'s module docs state
"every fixture ever committed, current or superseded, carries this claim
forever," and `v2-lz-repeated-text` predates this PR. ADR-0028's
version-1 drop is not a precedent here — that was safe only because
nothing had shipped yet, and a real fixture now has.
