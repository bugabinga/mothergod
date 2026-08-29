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
- **`silesia.md`**: the same shape as `canterbury.md`, over Silesia's 12
  individually pinned files (`research/JOURNAL.md` S2-A52).

**Read the `.md` files, not raw JSON**, for the current numbers.

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

```
cargo run -p mothergod-bench --release --features corpus-fetch --bin silesia_report
```

Silesia's counterpart, writes `silesia.md`. Measurement is parallel
(`mothergod_bench::reference::measure_all` runs one thread per file), so
this finishes in single-digit minutes on a multi-core machine rather than
the roughly half hour a serial pass over ~200 MB would take; also by
hand, re-run under the same conditions as `finals_report`.

Both `canterbury.md` and `silesia.md` embed the `bench/baseline.json`
fingerprint (`<!-- baseline-fingerprint: ... -->`) they were generated
against; `baseline_gate check` (the required `ratio` job) fails when a
committed fingerprint no longer matches, catching a baseline change that
left one of these stale (issue #327). Re-running either binary refreshes
its fingerprint along with its numbers.
