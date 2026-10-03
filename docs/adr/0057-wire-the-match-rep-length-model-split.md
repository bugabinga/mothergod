# ADR-0057: Wire the match/rep length-model split into the codec

Status: accepted · Date: 2026-10-03 · Resolves `JOURNAL` S2-A109 (in full) · `FORMAT_VERSION` 7 → 8

## Context

`research/JOURNAL.md` S2-A109 is a new literature idea, not tied to any
existing standing lead (ROADMAP M3: no standing lead was open): LZMA
(7-Zip's `lzma-specification.txt`) codes match lengths and repeated-offset
lengths through two independent coders (`LenCoder`/`RepLenCoder`), never
one shared table. `codec::Models::length` was one order-0 [`Model`]
shared by every `Token::Match` and `Token::Rep` length regardless of
which kind produced it, unconditioned since `S2-D2`'s own wiring.
`LengthSplitState`/`LengthSplitSink` paired that shared baseline against a
split candidate over each train/sealed case's real `lz::parse_optimal`
token stream, priced through `ideal_cost_bucketed` — the exact function
the real bitstream path already prices length/offset through, unlike the
literal mixer's own SSE-calibration gap (S2-R19). Train mean improved
-0.002514 b/B, 7 of 11 cases, led by `sqlite_like_records` (-0.015901);
both sealed cases cleared the accept bar (`access_log` -0.000768,
`gradient_image` flat at 0.000000). Accepted, unwired, naming its own
remaining scope: the real-bitstream wiring slice (`FORMAT_VERSION` bump,
ADR, decode support for every earlier version), the same shape S1-P3's
own additive PPM expert (S2-A100) is still waiting on.

## Decision

**1. `codec::Models` gains `length_match`/`length_rep` fields alongside
the existing `length` field.** `length` stays, unconditioned, for
decoding frames below `LENGTH_SPLIT_MIN_VERSION`; `length_match`/
`length_rep` are the real coding path at `LENGTH_SPLIT_MIN_VERSION` and
above, the same three-fields-alongside-the-old-one shape `logistic`/
`surprise`/`logit_sse` already take next to `literal` for the literal
sub-stream's own version ladder.

**2. `TokenSink::length` gains a `kind: FlagKind` parameter, threaded by
`walk_tokens` from the `Token::Match`/`Token::Rep` arm it is already
matching on.** `EncodeSink::length` and `codec::CostSink::length` both
pick `length_match`/`length_rep` by `kind` unconditionally (compression
always targets the newest version, mirroring every literal-mixer
`EncodeSink` branch's own unconditional pick); `PairedTokenSink::length`
(`ideal_cost_bits_ppm_expert_experiment`'s sink) does too, since its
baseline half must keep equaling `ideal_cost_bits` exactly, the existing
`ppm_expert_experiment_baseline_is_exactly_ideal_cost_bits` guard already
checks. `decode_tokens` gains a `length_split: bool` parameter instead of
promoting `length` into `DecodeSink` (unlike the literal sub-stream,
`offset`/`slot` are not version-gated, so only `length`'s own two call
sites need the branch, not a new sink method every `DecodeSink` impl
would have to carry): `true` selects `length_match`/`length_rep` by kind,
`false` the shared `length`, both through the same `decode_bucketed`.
`decode` and `decode_undoable_streaming` compute it once, `version >=
LENGTH_SPLIT_MIN_VERSION`, mirroring `LiteralPath::for_version`'s own
once-per-frame read.

**3. `FORMAT_VERSION` bumps to 8, gated on version alone, not
candidate.** Unlike every literal-mixer `*_MIN_VERSION` gate, this one
carries no `Candidate::Transpose` carve-out: the length symbol sits
outside the literal sub-stream those gates cover, so it applies to every
candidate identically.

**4. The now-superseded ideal-cost-pairing apparatus is deleted in this
PR.** `LengthSplitState`, `LengthSplitSink`,
`ideal_cost_bits_length_split_experiment`, and their dedicated test are
gone: the code a wiring slice ships must be the code that produced the
recorded number, and a private measurement-only candidate with no caller
left behind would be dead code the lint gate refuses
(`compression-experiment` skill). Two tests take their place:
`length_split_is_gated_on_version_alone` (same shape as the literal
sub-stream's own `*_is_gated_on_version_alone` suite, proving the version
check is live dispatch) and `length_match_and_rep_models_adapt_independently`
(proving `length_match`/`length_rep` are genuinely separate state, not
aliases a future refactor could collapse without a round-trip test
noticing, since encode and decode would still agree either way).

## Consequences

`research/JOURNAL.md` S2-A109 closes: the length-model split is wired
into a real bitstream, no longer ideal-cost-only.
`tests/golden/v8-lz-repeated-text` (the same plaintext as
`v5-lz-repeated-text`, re-encoded — the real encoder still selects
`Candidate::Delta`) pins the new path; every version-3 through version-7
fixture stays committed, decode-only, forever (ADR-0041/ADR-0050).

Real-bitstream measurement (`mothergod::compress`, this run's sandbox,
`bench/baseline.json`'s 11 train-tier cases plus both sealed-only kinds):
see `research/JOURNAL.md`'s closing S2-A110 entry for the full per-case
numbers. Train mean improves (-0.000727 b/B) but by much less than
S2-A109's own ideal-cost reading predicted (-0.002514), and
`sqlite_like_records` — S2-A109's largest predicted win (-0.015901) — is
flat in the real encoder (0.000000): `codec::encode`'s own filter trial
selects a candidate against the *filtered* byte stream, while
S2-A109's ideal-cost pairing walked `lz::parse_optimal` against the *raw*
input directly, so the two measure different match/rep structure for the
same named case. The same shape of gap ADR-0052's own wiring slice
(S2-A102) already documented once for the literal mixer: an ideal-cost
pairing measures a model in isolation, a wiring slice measures it inside
the whole encoder's own candidate/parse selection. No train or sealed
case regresses either way.

## Rejected alternatives

**Promote `length` into `DecodeSink` as a new trait method, the way
`literal` already is.** Rejected: `literal`'s own version gate depends on
*both* the frame's version and (for `Candidate::Transpose`) its
candidate, so each sink needs its own per-frame dispatch state
(`LiteralPath`/`column`). `length`'s gate is version-only and the same
for every candidate, and both its call sites already sit inside
`decode_tokens` itself (never inside a `DecodeSink` impl) — a plain
`bool` parameter threaded into one shared function is less surface than
widening every current and future `DecodeSink` implementor with a method
only two call sites would ever use.

**Keep `kind` out of `TokenSink::length`'s signature and track it as
sink-local mutable state instead, the way the deleted `LengthSplitSink`'s
own `last_kind` field did.** Rejected: `walk_tokens` already knows
whether it is walking a `Token::Match` or `Token::Rep` arm at the one
call site that invokes `length`, so passing `kind` through is strictly
less state than every `TokenSink` implementor re-deriving and caching it
from `flag`'s own call one step earlier. Each of `EncodeSink`/`CostSink`/
`PairedTokenSink` still needs a `FlagKind::Literal` branch ruled out when
picking `length_match` vs `length_rep` (`kind` is a three-variant enum,
not a two-variant one), but `split_length_model` holds that one
`unreachable!` for all three instead of each sink re-deriving its own
copy the way the deleted `LengthSplitSink::length` did.
