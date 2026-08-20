# Founding-session artifacts (imported 2026-08-20)

Primary sources behind `research/JOURNAL.md`'s S1-* entries, from the
founding research session (2026-08-19). Read-only archive: port from these,
never edit them.

## In-tree (Rust + data only, per ADR-0006)

| File | What | Notes |
|---|---|---|
| `mothergod.rs` | The Rust codec — single file, zero deps. **This is the M1 port source.** | Header comment says "v0.2" but the code carries the later features (6-expert DOD arena with two-rate counters and word-hash expert, context-sensitive MIX weights, optimal-parse DP with in-DP rep cache, transpose/BCJ/auto-stride filters, block-parallel mode) — the v0.5/v0.6 generation with a stale header. **Import-verified 2026-08-20**: compiles with `rustc -O` (2 warnings), lossless round-trip on Rust source (3.01 b/B), 256 KB of libc ELF (3.39 b/B), text (2.76 b/B); random 100 KB → 8.001 b/B stored floor, exactly as journaled. Encode ~0.2 MB/s — max-compression CM territory. Also usable as a differential oracle while porting (see `docs/TESTING.md`). |
| `research_state.json` | Resumable loop state (data) | Champion genome, filter/corpus code snippets, journal for it1–it31 with train/val deltas. |
| `progress.jsonl` | Machine-readable experiment log, it5–it31 (data) | Older schema than `research/README.md`'s new-era schema (per-dataset score tables, not deltas); never mix with `research/progress.jsonl`. |

## In git history only (the Python harness)

The Python research harness — `autoresearch.py`, `autoresearch2.py`,
`corpus.py` — was imported, **verified working**, then removed from the tree
to keep the project single-language (ADR-0006). It is preserved verbatim at
commit `1a3b1c8` and retrievable with:

```sh
git show 1a3b1c8:research/imports/session-1/autoresearch2.py
```

Verification record (2026-08-20): with all three files present,
`python3 autoresearch2.py status` (deps: `zstandard`; corpus reads
`/usr/share/doc`, libc, `/etc/ssl/certs` from a Debian-ish host) runs
end-to-end and reproduces the it31 champion's sealed-validation scores
exactly — VAL TOTAL 20.697, per-dataset identical to `progress.jsonl`
it30/31 (train slices rotate by iteration, so TRAIN varies as designed).
To consult the oracle, extract those files into a scratch directory outside
the repo, run read-only, and quote results as clearly-marked historical
model-cost numbers only. `autoresearch.py` also contains the original
invertibility guard and the sort-bytes fixture behind JOURNAL S1-L5.

## Provenance

Downloaded from the founding Claude session by the operator, uploaded
2026-08-20. The session transcript (not in-repo) additionally describes
it32–it41 (Silesia/Canterbury runs, transpose/BCJ ports) whose artifacts
postdate this archive; where archive and journal disagree, say so in a
journal entry rather than silently picking one.
