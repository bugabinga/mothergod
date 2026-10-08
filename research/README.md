# research/

The project's experimental memory. Agents: read `JOURNAL.md` before any codec
work; append to both files for every experiment.

## progress.jsonl schema

One JSON object per experiment, append-only:

```json
{
  "id": "it42",
  "date": "2026-08-20",
  "kind": "param | patch | literature | wild | corpus",
  "hypothesis": "one sentence",
  "verdict": "accepted | rejected",
  "train_delta_bpb": -0.12,
  "val_delta_bpb": -0.08,
  "corpus": "name@rev, slice size",
  "decode_cost_delta": 0.4,
  "mechanism": "why it worked/failed, one sentence",
  "commit": "sha or PR number, if merged"
}
```

Numbers are bits/byte deltas vs the current champion (negative = better).
The byte is the case's whole input, `CASE_LEN` for baseline cases, never a
sub-stream's symbol count: it174's first figures divided by literal count and
overstated the train mean 80x (PR #823). The bench crate's pairing driver
(#828) owns that division so no scratch binary redoes it.
`val_delta_bpb` comes from the sealed validation set — an accept requires
train improvement AND no validation regression.
For a `kind="patch"` whose sole purpose is enabling measurement and which has
no comparable benchmark, `train_delta_bpb` and `val_delta_bpb` are `null`.
Its verdict is `accepted` when the focused capability tests pass, otherwise
`rejected`; null deltas are invalid for every other record.

`decode_cost_delta` is required on a slice that changes what `decode` does
(a wiring slice, a format bump) and absent otherwise: the change in
`baseline_gate -- speed`'s decode cost, calibration steps per decoded byte,
against the champion measured on the same machine in the same session
(positive = slower). A slice whose cost lands past `speed::MAX_SLOWDOWN`
of `speed::BASELINE_DECODE_COST` fails `baseline_gate check`, so the constant
moves in that PR with the trade stated; ROADMAP's SPEED floor is defended
there, not by the ratio alone (#907).

Iterations it1–it41 happened in the founding session before this repo
existed; the surviving record (it1–it31) is archived verbatim in
`imports/session-1/` (`research_state.json`, `progress.jsonl` — note the
archive's older schema). This file is the new-era log only; do not mix the
two or fabricate archive rows here.

## corpus/POLICY.md

Rules for what may be benchmarked and how the corpus may grow. Read it before
adding data or quoting numbers.
