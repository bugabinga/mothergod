# ADR-0054: Wire the surprise rate schedule into the literal model

Status: accepted · Date: 2026-09-28 · `FORMAT_VERSION` 5 → 6

## Context

`ROADMAP.md` M3 named two untried threads once ADR-0052 closed `research/JOURNAL.md`
S1-P8: a rate schedule for the logit-domain mixer's per-key step size keyed
on something other than a step count, and a from-scratch GLN gating
architecture. `research/JOURNAL.md` S2-R26 tried the first as an EMA of
squared prediction error read against one shared `ERROR_RATE_MAX_VARIANCE`
constant, and rejected it: that constant could not tell a context's
stable residual uncertainty (a smooth, structurally noisy source whose
error never converges near zero) apart from genuine distribution drift,
so a converged-but-noisy context (`interleaved_audio16`, `gradient_image`)
read every node as permanent surprise.

S2-A104 turned S2-R26's own diagnosis into a fix, `literal::SurpriseLogisticMix`:
read the fast error EMA against that same key's own slower EMA of the
identical signal (a learned baseline) instead of a shared constant. Swept
over nine `recent_decay` points, ideal-cost pairing against the shipped
`LogisticMix` path: train mean improved from `recent_decay` 0.9 through
0.999, best at 0.98; both sealed cases (`access_log`, `gradient_image`)
improved at every point tried, `gradient_image` the first slice in this
lead's S2-R20 through S2-R26 history to improve rather than sink the
candidate. Accepted, unwired: S2-A104's own remaining-scope note named the
wiring slice's cost exactly as S2-A101's own note did for ADR-0052:
`FORMAT_VERSION` bump, ADR, golden fixture. `SurpriseLogisticMix`'s own
`recent_decay` default (`SURPRISE_RECENT_DECAY`, added at 0.9 as an
unrelated convenience default for callers that didn't sweep it) is
corrected to 0.98 in this same PR, the actual accepted point, since it now
drives a real bitstream rather than an experiment's own default argument.

## Decision

**1. `Literal::encode_logistic_surprise`/`decode_logistic_surprise` code a
literal byte through `SurpriseLogisticMix` instead of
`Literal::encode_logistic`/`decode_logistic`, for every candidate except
`Candidate::Transpose`.** Both share one walk
(`Literal::surprise_code_bit`, `Literal::logistic_code_bit`'s counterpart):
each bit-tree node stretches and blends the six real experts exactly as
the logistic path does, refines through `SurpriseLogisticMix`'s own `Sse`
table (independent of `LogisticMix`'s and `Literal`'s own), codes or
decodes the bit, then takes one gradient step at
`surprise_rate(recent_sq_error, baseline_sq_error)` and advances both EMAs
at `SURPRISE_RECENT_DECAY`/`SURPRISE_BASELINE_DECAY`. After the walk, both
call `Literal::update` exactly as `encode_logistic` does: the six real
experts' banks and the shipped linear mixer's own weights adapt
unperturbed, the same layering ADR-0052 already established.

**2. `FORMAT_VERSION` bumps to 6, gated on version alone (not candidate).**
`codec::Models` gains a `surprise: SurpriseLogisticMix` field, constructed
alongside `logistic` (`Models::new`/`try_new`, the latter using
`SurpriseLogisticMix::try_new`, added for this decode path per hard rule
2). `codec::encode_tokens`'s `EncodeSink` and `codec::decode`'s `VecSink`
now pick `encode_logistic_surprise`/`decode_logistic_surprise` over
`encode_logistic`/`decode_logistic` for any candidate whose `column` state
is `None` (compression always targets `FORMAT_VERSION`, so `EncodeSink`
picks unconditionally; `decode` gates on `version >=
codec::SURPRISE_MIN_VERSION`, 6). `codec::decode`'s and
`codec::decode_to_writer`'s streaming path (`VecSink`, `StreamingSink`)
replace their own two-state `logistic: bool` field with a three-state
`LiteralPath` enum (`Sse`/`Logistic`/`LogisticSurprise`) derived once from
the frame's declared version, so a version between the two thresholds
cannot dispatch to "both" or "neither" mixer. A `Candidate::Transpose`
frame is unaffected at every version: `COLUMN_EXPERT_MIN_VERSION`'s gate
still selects `encode_column`/`decode_column` regardless of either
logit-domain gate.

**3. `codec::CostSink`'s literal pricing switches from
`ideal_cost_bits_logistic` to `Literal::ideal_cost_bits_logistic_surprise`.**
Same `TokenSink` contract ADR-0052 established: `CostSink` and
`EncodeSink` must price and code the same thing, and every `CostSink`
caller here parses raw data with no filter selection, so
`Candidate::Transpose`'s `encode_column` path never arises.

**4. The now-superseded sweep entry point is deleted in this PR.**
`Literal::ideal_cost_bits_logistic_surprise_at` (S2-A104's own
`recent_decay`-parameterized sweep caller) is gone: `ideal_cost_bits_logistic_surprise`
folds its body in directly at the registered `SURPRISE_RECENT_DECAY`,
mirroring `ideal_cost_bits_logistic`'s own shape, which never carried a
parameterized counterpart. Its dedicated sweep-agreement test is replaced
by round-trip and state-parity tests on
`encode_logistic_surprise`/`decode_logistic_surprise`, the same shape
ADR-0052's own test suite took for `encode_logistic`/`decode_logistic`.

## Consequences

`research/JOURNAL.md` S2-A104's own remaining scope closes: the
learned-baseline rate schedule is wired into a real bitstream, no longer
ideal-cost-only. `tests/golden/v6-lz-repeated-text` (the same plaintext as
`v5-lz-repeated-text`, re-encoded) pins the new path; every version-3
through version-5 fixture stays committed, decode-only, forever
(ADR-0041/ADR-0050: no version already written is ever retired).

Real-bitstream measurement: see `research/JOURNAL.md`'s closing S2-A105
entry for the full per-case numbers.

`ROADMAP.md` M3's two once-untried threads are now down to one: this ADR
closes the rate-schedule thread, leaving the from-scratch GLN gating
architecture as the only remaining untried option before M3 falls back to
a literature idea or a wild swing.

## Rejected alternatives

**Leave `SURPRISE_RECENT_DECAY` at its pre-wiring default (0.9) and pass
0.98 explicitly at each real call site instead.** Rejected: S2-A104's own
sweep chose 0.98 as the single best-supported point, and every call site
this PR wires (`encode_logistic_surprise`, `decode_logistic_surprise`,
`ideal_cost_bits_logistic_surprise`) needs the identical value — the
decoder must replay the same schedule the encoder used, so a schedule
constant with two different call sites disagreeing on it would be a
latent desync bug waiting for a future edit to trip. Correcting the
constant itself, once, is the same shape ADR-0052 gave `LOGISTIC_RATE_DECAY`.

**Gate `SurpriseLogisticMix` on candidate as well as version, the way
`COLUMN_EXPERT_MIN_VERSION` gates the column expert.** Rejected for the
same reason ADR-0052 rejected it for the logistic mixer: this schedule
reads nothing candidate-specific, a straight replacement for the rate
schedule the six-expert logit-domain mix already used for every candidate
that path served.
