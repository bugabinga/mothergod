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
  "mechanism": "why it worked/failed, one sentence",
  "commit": "sha or PR number, if merged"
}
```

Numbers are bits/byte deltas vs the current champion (negative = better).
`val_delta_bpb` comes from the sealed validation set — an accept requires
train improvement AND no validation regression.

Iterations it1–it41 happened in the founding session before this repo
existed; their full record lives in `research_state.json` (pending import,
see ROADMAP M1). Do not fabricate their rows here.

## corpus/POLICY.md

Rules for what may be benchmarked and how the corpus may grow. Read it before
adding data or quoting numbers.
