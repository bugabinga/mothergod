# ADR-0046: Wire the column expert into the literal mixer

Status: accepted · Date: 2026-09-17 · Resolves `JOURNAL` S1-P5 (in full) · `FORMAT_VERSION` 3 → 4

## Context

`research/JOURNAL.md` S1-P5 is ROADMAP M3's fifth standing lead: per-column
modeling after `filters::transpose`. Four prior slices built the
prerequisites, each a standalone, not-yet-wired primitive: `column::column_of`/
`column_bank` (S2-A64/S2-A66), and two ideal-cost-pairing measurements
(`Literal::ideal_cost_bits_column_expert_pair[_sse]`, S2-A69/S2-A76) that
blend a column-keyed seventh expert into the six-expert mix and price it
without driving a real `Encoder`/`Decoder`. Both measurements accepted:
S2-A69 (pre-SSE) improved every case tried; S2-A76 (through the SSE
calibration stage every real byte already pays) still improved net train and
the sealed case, though the win shrinks once SSE partially absorbs the same
systematic bias. Both left "the real wiring itself... and a real-bitstream
measurement to confirm this ideal-cost signal survives contact with the
actual coder" as S1-P5's remaining scope.

## Decision

**1. `Literal::encode_column`/`decode_column` code a `Candidate::Transpose`
frame's literal sub-stream through the seven-expert mix.** Both build
`Literal::mix7`'s `cum` table (the six real experts plus `ColumnExpertState`'s
bank, keyed by `column::column_bank(column::column_of(position, columns,
len), MAX_COLUMN_BANKS)`), then code it through
`bittree::encode_symbol_sse`/`decode_symbol_sse` calibrated by
`ColumnExpertState`'s own `Sse` table — independent of `Literal`'s own `sse`
field, the same separation S2-A76 measured. The six real experts adapt
exactly as `Literal::encode_sse` leaves them: `Literal::update` still runs,
on the same six-way `mixed` estimate it always has, never perturbed by the
column expert — the same layering the SSE stage itself already uses on top
of the six-expert mix (ADR-0038), not a new coupling.
`ColumnExpertState`'s own weight and bank adapt separately via the existing
`Literal::update_column_expert`, against the seven-way mixed estimate that
was actually coded, in the same order S2-A76's own pairing measured
(`update_column_expert` first, reading the six real experts' pre-update
state, then the six-expert `update`). `MAX_COLUMN_BANKS` (256) is a fixed
decoder-chosen constant, never the frame's own `columns` field, per
`column::column_bank`'s own hard-rule-2 argument.

**2. `FORMAT_VERSION` bumps to 4, gated on both version and candidate.**
`codec::decode` already dispatches the literal sub-stream on the frame's
declared version (`LITERAL_SSE_MIN_VERSION`); it now also reads the
already-parsed filter selector: at `COLUMN_EXPERT_MIN_VERSION` (4) and
above, a `Candidate::Transpose` frame codes through `decode_column` instead
of `decode_sse`, every other candidate (and every candidate below version 4)
unaffected. `encode_tokens` gained an `Option<NonZeroUsize>` `columns`
parameter, `Some` exactly when `encode`'s current trial candidate is
`Candidate::Transpose`; `EncodeSink::literal` picks `encode_column` over
`encode_sse` the same way. `tests/golden/v3-*.mgdc` pin every version-3
literal path (both `Candidate::Transpose` via `v3-tabular-columns` and every
other candidate) forever; a new `tests/golden/v4-tabular-columns` pair pins
the new path.

**3. Every other candidate's coding is untouched at version 4.** Only a
`Candidate::Transpose` frame's literal sub-stream changes shape; `Identity`/
`Delta`/`Bcj` code identically to version 3 (just under a version-4 header
byte). `decode_to_writer`'s streaming path already falls back to `decode`'s
whole-buffer path for `Candidate::Transpose` unconditionally (`JOURNAL`
S2-D4): the column-expert path never needs a streaming counterpart.

## Consequences

`research/JOURNAL.md` S1-P5 closes: the column expert is wired into a real
bitstream, no longer ideal-cost-only. Measured (real bitstreams, this run's
sandbox):

- `bench/baseline.json`'s 11 fixed train-tier cases: no regression
  (`baseline_gate check` passes). 10 of 11 are byte-identical to their
  committed number — none of them select `Candidate::Transpose` in the real
  encoder trial. `entropy_ladder_h6` (iid noise at 6 bits/byte) does select
  `Candidate::Transpose(96)` in both the unpatched and patched build (a
  `filters::select::pick` heuristic artifact on this specific fixed-seed
  sample, unrelated to this ADR — the entropy-margin probe is noisy on true
  iid data), so it is not entirely inert here: 6.179200 -> 6.178240 bpb, a
  -0.000960 change, two orders of magnitude inside `TOLERANCE_BITS` (0.02).
  Not updated in `bench/baseline.json`: the drift is incidental (iid noise
  is not this lead's target shape) and well inside the tolerance the gate
  already exists to absorb, so updating it would force an unrelated
  Silesia/Canterbury finals regeneration (`baseline_gate`'s fingerprint
  check) for a number that has not actually moved past its own gate.
- Two purpose-built train/sealed pairs (synthetic fixed-width tabular data,
  each column cycling through its own period with ~20% of bytes jittered
  off the clean pattern so the literal model, not just LZ repeats, carries
  real weight — `research/JOURNAL.md`'s own entry has the exact
  construction), both real encoder selects reliably reduce to
  `Candidate::Transpose(96)` on both codec versions: 8-column shape, train
  8464 -> 8298 bytes (**-0.055333 bpb**), sealed 8400 -> 8240 bytes
  (**-0.053333 bpb**); 20-column shape, train 22556 -> 22344 bytes
  (**-0.028267 bpb**), sealed 22643 -> 22432 bytes (**-0.028134 bpb**). All
  four improve; `research/corpus/POLICY.md`'s accept rule (train
  improvement, no validation regression) is satisfied on both pairs.
- Sealed-only kinds `gradient_image`/`access_log` (`bench::baseline`'s own
  exclusion list): both select `Candidate::Identity` at this generator's
  default parameters on both codec versions, so both are byte-identical,
  neither a regression nor evidence either way for this lead — `column.rs`'s
  own doc already names why (`filters::select::TRANSPOSE_COLUMNS`'s fixed
  candidate list, `{2,3,...,96}`, does not include `gradient_image`'s true
  200-column width, and no scanned candidate's real encoded size beats
  `Identity` for either fixture at these lengths). Widening that candidate
  list, or adding a `DatasetKind` that reliably selects `Transpose` at its
  default parameters, is separate scope from this ADR (a `filters::select`
  heuristic question, not a literal-model question).

A new golden fixture pair, `tests/golden/v4-tabular-columns`, pins
`FORMAT_VERSION` 4's `Candidate::Transpose` literal sub-stream shape;
`tests/golden/v3-tabular-columns` stays exactly as committed, decode-only,
forever (ADR-0041: version 3 decodes forever, this ADR only adds a version,
never drops one).

## Rejected alternatives

**Couple the six real experts' weight update to the seven-way mixed
estimate instead of the layered design above** (i.e., `update` and the
column expert's update share one gradient step against one `mixed`, rather
than the six real weights adapting via their own independent `update` call
exactly as `encode_sse` already leaves them). Rejected: this would diverge
from the exact update order S2-A69/S2-A76's ideal-cost pairing already
measured and accepted, reintroducing exactly the risk those two slices
existed to retire before spending the real-wiring slice, for no
demonstrated benefit — and it duplicates `update`'s six-expert gradient step
in a new method instead of reusing it, against the reuse ladder. The layered
design costs nothing extra: it reuses `Literal::update` and
`Literal::update_column_expert` verbatim, adding only two thin
`encode_column`/`decode_column` methods.

**Widen `filters::select::TRANSPOSE_COLUMNS` or the entropy margin so
`Candidate::Transpose` (and therefore this ADR's new coding path) is
selected more often**, e.g. to make `gradient_image`'s 200-column structure
reachable. Rejected as out of this ADR's scope: `pick`'s candidate list and
margin are a filter-selection heuristic question, orthogonal to whether the
literal model helps once `Transpose` is already chosen. A future issue can
propose widening it on its own evidence.
