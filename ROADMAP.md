# Roadmap

Ordered milestones. The daily heartbeat picks the top unblocked item and ships
the smallest useful slice of it. Items marked `blocked-on-human` need the
operator. Research-flavored items defer to `research/JOURNAL.md` leads for
their ordering.

## M0 — Scaffolding ✅

Crate skeleton, v0 frame format (Stored), quality-gate CI, governance and
agent processes. Done 2026-08-20.

## M1 — Recover the founding-session codec

- [ ] `blocked-on-human` Operator uploads founding-session artifacts:
      `mothergod.rs` (codec v0.6), `autoresearch2.py`, `research_state.json`,
      `progress.jsonl`. Issue to track: label `blocked-on-human`.
- [ ] If artifacts arrive: port v0.6 into `src/` as reviewable modules
      (filters, parse, models, coder), tests per module, invariants written
      down (JOURNAL S1-A*).
- [ ] If artifacts do NOT arrive within 14 days: reimplement from
      `research/JOURNAL.md` — it is the spec. Start: stored floor → order-0
      adaptive AC → LZ greedy → filters → mixing, validating each stage
      against the journal's recorded behavior.

## M2 — Honest benchmarking (JOURNAL S1-D2)

- [ ] `bench/` harness: Silesia + Canterbury at pinned revisions, entropy
      ladder + markov-H8/2 generators, sealed validation split per
      `research/corpus/POLICY.md`.
- [ ] CI benchmark gate: PR fails on regression vs `bench/baseline.json`.
- [ ] Nightly/weekly report: bits/byte vs gzip/zstd/xz, per-dataset graphs
      rendered from `research/progress.jsonl` into `docs/benchmarks/`.

## M3 — Close the gaps (research program)

Work the journal's standing leads in order: SSE (S1-P1), btultra2-class parse
(S1-P2), PPM escape (S1-P3), large windows (S1-P4), per-column modeling
(S1-P5). Target: beat zstd -19 per-file on all of Silesia/Canterbury with
real bitstreams; then xz -9e.

## M4 — Production hardening

- [ ] cargo-fuzz targets: decoder-never-panics, round-trip, bomb resistance.
- [ ] cargo-mutants in scheduled CI; surviving mutants become issues.
- [ ] Streaming/block API, bounded-memory decode guarantees.
- [ ] Frozen format spec v1 (`docs/format/SPEC.md`) + `FORMAT_VERSION` 1.

## M5 — Speed tiers

Bit-decomposed fast models, tANS fast path (level -1 mode), explicit SIMD
blend, measured multi-core scaling (S1-P6).

## M6 — Release 0.1

- [ ] CLI binary (`mothergod` compress/decompress).
- [ ] GitHub release with binaries, agent-drafted changelog.
- [ ] `blocked-on-human` crates.io publish (operator holds the token).
