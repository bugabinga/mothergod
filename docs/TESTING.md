# Test strategy

What "tested" means for this project, in layers. CI's `quality-gate` runs
layer 1 on every change; the deeper layers have their own cadence. Corpus
rules live in `research/corpus/POLICY.md`.

## 1. Round-trip and unit tests (every PR)

- `decompress(compress(x)) == x` property tests over every generator class
  in the corpus policy (seeded, deterministic), plus edge sizes: empty, 1
  byte, exactly-window-sized, window+1.
- Every codec module (filter, parse, model, coder) gets unit tests for its
  own invariants — especially the ones the journal says were once implicit
  (rep-symbol/offset-bucket disjointness, stored floor).
- Every filter ships an exact-invertibility test (`unfilt(filt(x)) == x`)
  on data it targets AND data it doesn't.

## 2. Adversarial decode suite (every PR)

The decoder's contract: **never panic, never overallocate, on any input.**

- In-repo `tests/adversarial/` seed corpus: truncations at every header
  boundary, bit-flips, declared-size lies (bombs), wrong magic/version,
  unknown methods, fuzz-found crashers (each promoted to a regression test
  with a comment naming the bug it caught).
- Tests assert graceful `Err`, never a panic; allocation stays bounded by a
  stated multiple of the declared output size.

## 3. Fuzzing (scheduled, M4)

- `cargo-fuzz` targets: `decode(arbitrary bytes)` must not panic;
  `roundtrip(arbitrary bytes)` must be identity; `decode` under an
  allocation limiter must respect bounds.
- Runs on a schedule (nightly toolchain, time-boxed), not per-PR. New
  crashers land in `tests/adversarial/` as regression seeds.

## 4. Mutation testing (scheduled, M4)

- `cargo-mutants` on the codec modules; surviving mutants become issues —
  a surviving mutant is a missing test by definition.

## 5. Determinism (per-PR once cross-platform CI exists)

- Same input + same version ⇒ byte-identical bitstream on every platform
  (the integer-only probability path exists precisely for this, JOURNAL
  S1-A5). Golden-file tests pin known input → known output per
  `FORMAT_VERSION`; changing a golden file without a version bump fails.
- Old-version frames stay decodable (CLAUDE.md rule 5): keep one tiny
  golden frame per historical `FORMAT_VERSION`.

## 6. Differential oracle (during the M1 port)

- `research/imports/session-1/mothergod.rs` compiles standalone and is
  lossless-verified. While porting, compare the port against it: round-trip
  agreement on the corpus classes, and ratio within stated tolerance per
  dataset. Divergences are findings — either a port bug or an archive bug;
  journal them either way. The oracle is frozen; the port is what changes.

## 7. Benchmark regression gate (M2)

- PRs fail on bits/byte regression vs `bench/baseline.json` beyond stated
  noise bounds, on the train tier only (sealing rules in POLICY.md).
  Ratio improvements that break layers 1–2 are rejected regardless — a
  faster-shrinking codec that panics on truncated input is a worse codec.
