# ADR-0058: Wire the offset/length-state split into the codec

Status: accepted · Date: 2026-10-03 · Resolves `JOURNAL` S2-A111 (in full) · `FORMAT_VERSION` 8 → 9

## Context

`research/JOURNAL.md` S2-A111 is the same literature source's other axis,
not a variant of S2-A109/S2-A110's length-model split: LZMA
(`lzma-specification.txt`) prices a match's distance slot through one of
four independent probability trees selected by `len_to_pos_state`, a
coarse bucket of the match's own length, never by copy kind.
`codec::Models::offset` was one shared, unconditioned [`Model`] since
`S2-D2`'s own wiring, coding every `Token::Match`'s distance regardless of
that match's own length (`Token::Rep` never reaches `offset` at all: a
repeat prices through `models.slot` instead). `OffsetLenSplitState`/
`OffsetLenSplitSink` paired that shared baseline against a four-state
split candidate over each train/sealed case's real `lz::parse_optimal`
token stream, priced through `ideal_cost_bucketed`, the exact function
the real bitstream path already prices offsets through, the same no-gap
argument S2-A109 established for the length split. Train mean improved
-0.003526 b/B, 10 of 11 cases (`x86_dense_code` -0.016795 the largest), no
case regressed; both sealed cases cleared the accept bar (`access_log`
-0.009795, `gradient_image` -0.000289), every single measured case moving
the same direction or flat. Accepted, unwired, naming its own remaining
scope: the real-bitstream wiring slice (`FORMAT_VERSION` bump, ADR, decode
support for every earlier version), the same shape S2-A109 named before
S2-A110 closed it.

## Decision

**1. `codec::Models` gains an `offset_len` field, `[Model;
OFFSET_LEN_STATES]` (4), alongside the existing `offset` field.**
`offset` stays, unconditioned, for decoding frames below
`OFFSET_LEN_SPLIT_MIN_VERSION`; `offset_len` is the real coding path at
`OFFSET_LEN_SPLIT_MIN_VERSION` and above, the same
field-alongside-the-old-one shape `length_match`/`length_rep` already
took next to `length` for the length split. Unlike that pair, four states
do not read well as four named fields, so `offset_len` is an array,
indexed by `offset_len_state` (LZMA's `len_to_pos_state` formula, adapted
to this crate's `lz::MIN_MATCH_LEN` floor of 4 instead of LZMA's 2); both
the constant and the function move from the (now-deleted) experiment
apparatus into real plumbing unchanged.

**2. `TokenSink::offset` gains a `len: u32` parameter, threaded by
`walk_tokens` from the `Token::Match` arm's own already-destructured
length, the same arm that already feeds `TokenSink::length`.** This is
the one place this wiring slice's shape diverges from ADR-0057's: that
ADR's own "Rejected alternatives" section rejected tracking `kind` as
sink-local mutable state in `TokenSink::length`'s case because
`walk_tokens` already had it in hand at the one call site; the deleted
`OffsetLenSplitSink`'s own `last_match_len` field was exactly the
sink-local-tracking shape that reasoning argued against, carried here
only because the pre-wiring experiment's `TokenSink::offset` signature
could not change without also changing `CostSink`/`PairedTokenSink`. The
wiring slice is the natural point to fix it: `len` becomes a real
parameter, `EncodeSink::offset` and `CostSink::offset` both pick
`offset_len[offset_len_state(len)]` unconditionally (compression always
targets the newest version); `PairedTokenSink::offset` (the PPM-expert
experiment's sink) does too, since its baseline half must keep equaling
`ideal_cost_bits` exactly, the same guard `ppm_expert_experiment_baseline_
is_exactly_ideal_cost_bits` already checks for `length`. `decode_tokens`
gains an `offset_split: bool` field (bundled with the existing
`length_split: bool` into one `DecodeGates` struct, since an eighth plain
`bool` parameter on that function trips `clippy::too_many_arguments`): a
new `decode_offset` helper mirrors `decode_length`'s own shape, selecting
`offset_len[offset_len_state(len)]` when set, the shared `offset`
otherwise, both through the same `decode_bucketed`. `decode` and
`decode_undoable_streaming` compute it once, `version >=
OFFSET_LEN_SPLIT_MIN_VERSION`, mirroring `LiteralPath::for_version`'s own
once-per-frame read.

**3. `FORMAT_VERSION` bumps to 9, gated on version alone, not
candidate.** Same shape as `LENGTH_SPLIT_MIN_VERSION`: the offset symbol
sits outside the literal sub-stream every `*_MIN_VERSION` gate above it
covers, so it applies to every candidate identically.

**4. The now-superseded ideal-cost-pairing apparatus is deleted in this
PR.** `OffsetLenSplitState`, `OffsetLenSplitSink`, and
`ideal_cost_bits_offset_length_split_experiment` are gone;
`OFFSET_LEN_STATES` and `offset_len_state` stay, repurposed from
experiment-only state into the real coding path's own plumbing. Three
tests take the deleted module's place:
`offset_length_split_is_gated_on_version_alone` (same shape as the length
split's own `length_split_is_gated_on_version_alone`, proving the version
check is live dispatch), `offset_models_adapt_independently_by_length_state`
(proving the four `offset_len` entries are genuinely separate state, not
aliases a future refactor could collapse without a round-trip test
noticing), and `offset_len_state_saturates_on_a_long_match` (the deleted
module's own saturation test, carried over unchanged since the function it
guards did not change).

## Consequences

`research/JOURNAL.md` S2-A111 closes: the offset-length-state split is
wired into a real bitstream, no longer ideal-cost-only.
`tests/golden/v9-lz-repeated-text` (the same plaintext as
`v8-lz-repeated-text`, re-encoded: the real encoder still selects
`Candidate::Delta`) pins the new path; every version-3 through version-8
fixture stays committed, decode-only, forever (ADR-0041/ADR-0050).

Real-bitstream measurement (`mothergod::compress`, this run's sandbox,
`bench/baseline.json`'s 11 train-tier cases plus both sealed-only kinds):
see `research/JOURNAL.md`'s closing S2-A112 entry for the full per-case
numbers. Train mean improves -0.002836 b/B, every one of the 11 cases
improved or flat, none regressed: closer to S2-A111's own ideal-cost
prediction (-0.003526) than the length split's real/ideal gap
(S2-A110), though still smaller case by case (`x86_dense_code`'s real
-0.011200 against its predicted -0.016795, the same direction, about
two-thirds the magnitude). Both sealed cases improve: `access_log`
-0.009920, `gradient_image` -0.000320.

`bench/baseline.json` and `docs/benchmarks/{canterbury,silesia}.md` are
**not** regenerated in this PR, departing from ADR-0057's own precedent.
`cargo run -p mothergod-bench --release --bin baseline_gate -- check`
passes against the *unchanged* committed baseline (no case regresses past
`TOLERANCE_BITS`, 0.02 b/B; every real delta measured above is far
inside it), so the required `ratio` CI check is unaffected either way.
Writing fresh baseline numbers would retire the committed
`baseline-fingerprint` both held-out-final reports embed
(`bench/src/bin/baseline_gate.rs`, issue #327), forcing a same-PR
regeneration of both; `silesia_report` fetched cleanly from this run's
sandbox (`sun.aei.polsl.pl`, hash-verified against `bench/corpus.toml`'s
pin), but `corpus.canterbury.ac.nz` answered every attempt with HTTP 403
regardless of retry or user agent, confirmed general internet egress
otherwise works (`github.com`, `example.com` both reachable). Refreshing
the committed ratio numbers and the two finals reports together is
explicit follow-up work for a session whose network reaches both hosts,
not silently dropped.

## Rejected alternatives

**Keep `TokenSink::offset`'s signature unchanged and track the
triggering match's length as sink-local state, the way the deleted
`OffsetLenSplitSink`'s own `last_match_len` field did.** Rejected for the
real wiring, for the same reason ADR-0057 rejected the equivalent shape
for `length`'s `kind`: `walk_tokens` already has the match's length in
hand at the one call site that invokes `offset` (`Token::Match`'s own
destructured `len`, the same binding `TokenSink::length` already
receives), so passing it through is strictly less state than every
`TokenSink` implementor re-deriving and caching it one call earlier. The
pre-wiring experiment carried the sink-local shape only because changing
`TokenSink::offset`'s signature at that stage would have forced `CostSink`
and `PairedTokenSink` to carry a now-unused parameter for a candidate not
yet accepted; the wiring slice removes that constraint.

**A fifth or sixth `offset_len_state`, finer than LZMA's own four.**
Rejected without measurement: S2-A111's own accepted candidate used
LZMA's four-state formula unchanged, matching the literature source
exactly rather than introducing an untested free parameter; a finer split
is a new, separate candidate (richer state costs real bias/variance per
`S1-L4`, the same tax S2-A109's own three regressions paid), not scope
this wiring slice should absorb.
