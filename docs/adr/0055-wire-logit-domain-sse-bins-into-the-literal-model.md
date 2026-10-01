# ADR-0055: Wire logit-domain SSE bins into the literal model

Status: accepted · Date: 2026-10-01 · `FORMAT_VERSION` 6 → 7

## Context

`ROADMAP.md` M3 named two untried threads once ADR-0052 closed `research/JOURNAL.md`
S1-P8: a rate schedule for the logit-domain mixer's per-key step size keyed
on something other than a step count, and a from-scratch GLN gating
architecture. ADR-0054 closed the first thread (S2-A104/S2-A105), leaving
the GLN thread as the only one M3 names as still open — but S2-A106 found a
second, mechanically distinct candidate first: `crate::sse::Sse`'s own
module doc records a deliberate deviation from the classic APM (Mahoney
2005), linear-domain bin spacing instead of log-domain, because at the time
(S2-A40) this crate had no deterministic transcendental pair to build
`stretch`/`squash` from. `crate::logistic` (S2-A101) built exactly that
pair for the logit-domain mixer and it already ships on the real coding
path (`FORMAT_VERSION` 5+), so the blocker no longer holds.

S2-A106 built `crate::sse::LogitSse`, a structural mirror of `Sse` whose
bins live at evenly spaced points in `stretch`-space instead of linear
probability space, and measured it through the bench crate's pairing
driver (`mothergod_bench::literal_pairing::train_and_sealed_delta_bpb`,
issue #828) against `SurpriseLogisticMix`'s own shipped ideal cost. Train
mean improved (-0.000166 b/B, 5 of 11 cases), both sealed cases improved
(`access_log` -0.001004, `gradient_image` -0.040248, the largest single
sealed move this mixer lineage's own ideal-cost history has measured).
Accepted, unwired: S2-A106's own remaining-scope note named the wiring
slice's cost exactly as S2-A104's own note did for ADR-0054: `FORMAT_VERSION`
bump, ADR, golden fixture.

## Decision

**1. `Literal::encode_logit_sse`/`decode_logit_sse` code a literal byte
through `SurpriseLogisticMixLogitSse` instead of
`Literal::encode_logistic_surprise`/`decode_logistic_surprise`, for every
candidate except `Candidate::Transpose`.** Both share one walk
(`Literal::logit_sse_code_bit`, `Literal::surprise_code_bit`'s
counterpart, already built by S2-A106 for ideal-cost measurement): each
bit-tree node stretches and blends the six real experts exactly as the
surprise path does, refines through `SurpriseLogisticMixLogitSse`'s own
`LogitSse` table (independent of `SurpriseLogisticMix`'s and `Literal`'s
own), codes or decodes the bit, then takes one gradient step at
`surprise_rate(recent_sq_error, baseline_sq_error)` and advances both
EMAs at `SURPRISE_RECENT_DECAY`/`SURPRISE_BASELINE_DECAY` — the identical
rate schedule ADR-0054 shipped, untouched; only the calibration table's
bin spacing differs. After the walk, both call `Literal::update` exactly
as `encode_logistic_surprise` does: the six real experts' banks and the
shipped linear mixer's own weights adapt unperturbed, the same layering
every prior wiring slice in this lineage established.

**2. `FORMAT_VERSION` bumps to 7, gated on version alone (not
candidate).** `codec::Models` gains a `logit_sse: SurpriseLogisticMixLogitSse`
field, constructed alongside `surprise` (`Models::new`/`try_new`, the
latter using `SurpriseLogisticMixLogitSse::try_new`, added for this
decode path per hard rule 2, which in turn needed
`crate::sse::LogitSse::try_new`, added alongside it). `codec::encode_tokens`'s
`EncodeSink` and `codec::decode`'s `VecSink` now pick `encode_logit_sse`/
`decode_logit_sse` over `encode_logistic_surprise`/`decode_logistic_surprise`
for any candidate whose `column` state is `None` (compression always
targets `FORMAT_VERSION`, so `EncodeSink` picks unconditionally; `decode`
gates on `version >= codec::LOGIT_SSE_MIN_VERSION`, 7). `codec::decode`'s
and `codec::decode_to_writer`'s streaming path (`VecSink`, `StreamingSink`)
extend their own three-state `LiteralPath` enum (`Sse`/`Logistic`/
`LogisticSurprise`) to a fourth state (`LogitSse`), still derived once
from the frame's declared version, so a version between any two
thresholds still cannot dispatch to more than one mixer at once. A
`Candidate::Transpose` frame is unaffected at every version:
`COLUMN_EXPERT_MIN_VERSION`'s gate still selects `encode_column`/
`decode_column` regardless of any logit-domain gate.

**3. `codec::CostSink`'s literal pricing switches from
`ideal_cost_bits_logistic_surprise` to
`Literal::ideal_cost_bits_logistic_surprise_logit_sse`.** Same
`TokenSink` contract ADR-0052/ADR-0054 established: `CostSink` and
`EncodeSink` must price and code the same thing, and every `CostSink`
caller here parses raw data with no filter selection, so
`Candidate::Transpose`'s `encode_column` path never arises.

**4. No sweep entry point to delete.** Unlike S2-A104 (ADR-0054), S2-A106
never carried a parameterized sweep: `LogitSse`'s bin spacing has no
free parameter this candidate explored a range of, so this PR's only
cleanup is the scratch measurement binaries, already deleted with their
respective verdicts (S2-A106's own `scratch_logit_sse.rs`, and this
wiring slice's own `scratch_sealed_real.rs`, used once to confirm the
sealed real-bitstream deltas against the ideal-cost prediction).

## Consequences

`research/JOURNAL.md` S2-A106's own remaining scope closes: the
stretch-domain SSE calibration is wired into a real bitstream, no longer
ideal-cost-only. `tests/golden/v7-lz-repeated-text` (the same plaintext
as `v6-lz-repeated-text`, re-encoded) pins the new path; every version-3
through version-6 fixture stays committed, decode-only, forever
(ADR-0041/ADR-0050: no version already written is ever retired).

Real-bitstream measurement: see `research/JOURNAL.md`'s closing S2-A108
entry for the full per-case numbers. Train mean is thin (-0.000145 b/B,
6 of 11 cases improve, `sqlite_like_records` the largest single
regression at +0.006080, well inside `TOLERANCE_BITS`), both sealed
cases improve (`access_log` -0.001440, `gradient_image` -0.040320), and
both held-out finals improve in aggregate (Canterbury 1.366878 →
1.365936, Silesia 2.057119 → 2.056363 bits/byte) — `docs/benchmarks/canterbury.md`
and `docs/benchmarks/silesia.md` regenerated in this same PR per the
`ratio` gate's own staleness check.

`ROADMAP.md` M3's once-two untried threads are now down to the one it
already named before this candidate surfaced: the from-scratch GLN
gating architecture, still the only thread M3 names as open; S2-A106's
own candidate was a literature idea outside that enumerated pair
(ROADMAP's own "no standing lead is open" framing), not a slice of it.

## Rejected alternatives

**Leave `Sse` as the production calibration table and ship `LogitSse` as
a second, parallel table a caller chooses between.** Rejected: nothing in
this lineage's own history keeps two interchangeable calibration tables
live on the real coding path at once (ADR-0038 retired the uncalibrated
path outright when SSE first shipped); `LogitSse`'s own measurement
claims a real win on every case basis the corpus policy checks, so there
is no case for keeping the dominated table wired rather than retired to
`tests/golden/superseded/`'s own forever-decodable record.

**Gate `SurpriseLogisticMixLogitSse` on candidate as well as version, the
way `COLUMN_EXPERT_MIN_VERSION` gates the column expert.** Rejected for
the same reason ADR-0052/ADR-0054 rejected it for their own mixers: this
calibration table reads nothing candidate-specific, a straight
replacement for the SSE table the six-expert logit-domain mix already
used for every candidate that path served.
