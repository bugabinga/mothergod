# Founding-session artifacts (imported 2026-08-20)

Verbatim artifacts from the founding research session (2026-08-19), uploaded
by the operator. These are the primary sources behind `research/JOURNAL.md`'s
S1-* entries. Treat as read-only archive: port from them, never edit them.

| File | What | Notes |
|---|---|---|
| `mothergod.rs` | The Rust codec — single file, zero deps | Header comment says "v0.2" but the code carries the later features (6-expert DOD arena with two-rate counters and word-hash expert, context-sensitive MIX weights, optimal-parse DP with in-DP rep cache, transpose/BCJ/auto-stride filters, block-parallel mode) — i.e. the v0.5/v0.6 generation with a stale header. **Import-verified 2026-08-20**: compiles with `rustc -O` (2 warnings), lossless round-trip on Rust source (3.01 b/B), 256 KB of libc ELF (3.39 b/B), session text (2.76 b/B); random 100 KB → 8.001 b/B stored floor, exactly as journaled. Encode ~0.2 MB/s — max-compression CM territory. |
| `autoresearch2.py` | Research-loop harness + Python codec v3 | The later of two uploaded revisions (adds `algn` alignment expert, `dpcand`, `selo1`/`seltrial` filter-selection arms; the earlier revision was a strict subset and was not kept). Line 2 `exec`s the base primitives from `autoresearch.py` (below). |
| `autoresearch.py` | Harness base + the original 5-iteration loop | Supplies the primitives `autoresearch2.py` builds on (`sd`/`usd` delta filters, `FILTERS`, `GENOME`, `CTX`, adaptive model `M`, `bkt`, greedy `lz`) plus the first-generation loop: LLM-proposal schema, the invertibility guard that famously killed the "sort all bytes" filter (JOURNAL S1-L5 — the sort-bytes fixture is literally in this file), and canned fixtures used when no API key is present. Its `llm_call` targets the direct API — archive only; the in-repo loop is subscription-driven per ADR-0004. |
| `corpus.py` | Corpus generator | Train/val datasets, entropy ladder (`iid-H*`), the markov-H8/2 trap, b64/zipped rows. Reads host paths (`/usr/share/doc`, libc, `/etc/ssl/certs`) — environment-dependent by design; pin what it produces, not the script, when the bench harness lands (M2). |
| `research_state.json` | Resumable loop state | Champion genome, filter/corpus code snippets, and the journal for it1–it31 with train/val deltas. |
| `progress.jsonl` | Machine-readable experiment log, it5–it31 | Schema differs slightly from the new-era schema in `research/README.md` (per-dataset score tables instead of deltas); do not mix the files — new experiments go in `research/progress.jsonl`. |

Provenance: downloaded from the founding Claude session by the operator and
uploaded 2026-08-20. The session transcript (not in-repo) additionally
describes it32–it41 (Silesia/Canterbury runs, transpose/BCJ ports) whose
artifacts postdate these files; where this archive and the journal disagree,
say so in a journal entry rather than silently picking one.

**Harness verified complete 2026-08-20**: with all files present,
`python3 autoresearch2.py status` (deps: `zstandard`; corpus reads
`/usr/share/doc`, libc, `/etc/ssl/certs` from a Debian-ish host) runs
end-to-end and reproduces the it31 champion's sealed-validation scores
exactly (VAL TOTAL 20.697, per-dataset identical to `progress.jsonl` it30/31;
train slices rotate by iteration, so TRAIN varies as designed). The loop is
resumable at it31.
