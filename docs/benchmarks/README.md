# Benchmarks

Two generated snapshots, kept separate because they measure different
tiers of `research/corpus/POLICY.md`'s corpus and neither should be read
as the other:

- **`baseline.md`/`baseline.svg`**: mothergod's bits/byte on
  `bench/baseline.json`'s fixed regression-gate cases
  (`research/JOURNAL.md` S2-A35) — one case per entropy-ladder target plus
  one per train-eligible structured dataset kind (`bench::DatasetKind`),
  all generated in-repo. Mothergod against itself, run over run; not a
  claim about beating anything.
- **`canterbury.md`**: real, named-corpus numbers against the pinned
  reference compressors (`research/JOURNAL.md` S2-A37) — mothergod,
  `gzip -9`, `zstd -19`, and `xz -9e`, each run on the actual Canterbury
  corpus (`research/corpus/POLICY.md`'s held-out finals). This is the
  first table in this directory CLAUDE.md rule 4's "X bits/byte on
  \<corpus\>" applies to without qualification.

**Read the `.md` files, not raw JSON**, for the current numbers.

## What this is not, yet

ROADMAP M2 wants "bits/byte vs gzip/zstd/xz, per-dataset graphs ... into
`docs/benchmarks/`". `canterbury.md` is the gzip/zstd/xz-comparison half of
that line; one thing is still missing, named as remaining S2-D1/M2 scope,
not silently dropped:

- **No Silesia.** Canterbury only (~2.7 MB, under a minute of
  `mothergod::compress` time). The full ~200 MB Silesia corpus would run
  this codec's optimal-parse LZ for on the order of half an hour
  (measured: 5.3 MB in 39s, ~0.14 MB/s) — too slow for a by-hand run;
  `bin/finals_report.rs`'s module doc has the full reasoning. Silesia
  numbers most naturally land as an extension of the weekly
  `corpus-fetch-check.yml` (issue #231's outcome), which already fetches
  the pinned corpus on a schedule.

## Regenerating

```
cargo run -p mothergod-bench --release --bin render_baseline_graph
cargo x fmt -- docs/benchmarks/baseline.svg
```

Reads `bench/baseline.json`, writes `baseline.svg` and `baseline.md`; the
`fmt` pass picks up the canonical SVG indentation `cargo x fmt --check`
expects, same shape as `site/status-data.json`'s own generate-then-fmt
step. Not
on a schedule; re-run by hand after `bench/baseline.json` changes, same
as `baseline_gate`'s own `write` mode.

```
cargo run -p mothergod-bench --release --features corpus-fetch --bin finals_report
```

Fetches Canterbury (pin-verified, cached under `target/bench-corpus-cache`),
writes `canterbury.md`. Markdown is linted, not formatted
(`cargo x lint -- docs/benchmarks/canterbury.md`). Also by hand; re-run
whenever the codec or a reference-compressor version changes enough to be
worth re-measuring.
