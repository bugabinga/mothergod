# Benchmarks

`baseline.md` and `baseline.svg` are a generated snapshot of mothergod's
bits/byte on `bench/baseline.json`'s fixed regression-gate cases
(`research/JOURNAL.md` S2-A35): one case per entropy-ladder target plus one
per train-eligible structured dataset kind (`bench::DatasetKind`), all
generated in-repo, none of them Silesia or Canterbury.

**Read `baseline.md`, not raw JSON**, for the current numbers: a markdown
table plus the bar chart, both generated together so they can't drift apart.

## What this is not, yet

ROADMAP M2 wants "bits/byte vs gzip/zstd/xz, per-dataset graphs ... into
`docs/benchmarks/`". This is the graph-rendering half of that line
(`research/JOURNAL.md` S2-D1), on the only real, named-corpus numbers this
crate can measure today. Two things are still missing, both named as
remaining S2-D1/M2 scope, not silently dropped:

- **No gzip/zstd/xz comparison.** `bench/baseline.json` is mothergod
  against itself, run over run. A reference-compressor column needs a
  harness that also runs gzip/zstd/xz on the same bytes; that harness
  doesn't exist yet.
- **No Silesia/Canterbury.** The held-out finals are fetched and cached by
  `bench::corpus` (behind the `corpus-fetch` feature), but nothing measures
  mothergod's ratio on them yet, and CLAUDE.md rule 4 means a number here
  would need to name that corpus explicitly, distinct from the generator
  cases above.

## Regenerating

```
cargo run -p mothergod-bench --release --bin render_baseline_graph
cargo x fmt -- docs/benchmarks/baseline.svg
```

Reads `bench/baseline.json`, writes `baseline.svg` and `baseline.md`; the
`fmt` pass picks up the canonical SVG indentation `cargo x fmt --check`
expects, same shape as `site/status-data.json`'s own generate-then-fmt
step. Not
yet on a schedule: wiring a `.github/workflows/` file needs
`GH_ADMIN_TOKEN` (`agents/GOVERNANCE.md`, "Push identity"), the same gap
that leaves the CI baseline gate itself unwired
(`research/JOURNAL.md` S2-A35). Re-run by hand after `bench/baseline.json`
changes, same as `baseline_gate`'s own `write` mode.
