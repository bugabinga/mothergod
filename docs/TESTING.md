# Test strategy

What "tested" means for this project, in layers. Corpus rules live in
`research/corpus/POLICY.md`.

## Current automated cadence

Every PR retains the required job names `fmt`, `clippy`, `test`, and `doc`.
Rust-input PRs run them on Linux x64 stable through
`.github/actions/rust-ci`: formatting, Clippy, all Cargo targets, doctests,
and warning-clean rustdoc output. Non-Rust PRs receive four successful skips
through the path filter. Those four names are the repository ruleset contract.

The advisory `monster` workflow runs Saturdays at 03:17 UTC and on manual
dispatch, never on pull requests. Every runtime lane runs
`cargo test --all-targets` and `cargo test --doc` on stable and the root
`Cargo.toml` package's `rust-version`; the workflow reads that declaration
instead of copying the MSRV. The canonical `ubuntu-24.04` x64/glibc stable
lane additionally runs fmt, Clippy, and rustdoc once.

| Runtime | Hosted runner | Rust target |
|---|---|---|
| Linux x64, glibc | `ubuntu-24.04` | `x86_64-unknown-linux-gnu` |
| Linux x64, musl | `ubuntu-24.04` | `x86_64-unknown-linux-musl` |
| Linux ARM64, glibc | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` |
| Linux ARM64, musl | `ubuntu-24.04-arm` | `aarch64-unknown-linux-musl` |
| Windows x64, MSVC | `windows-2025` | `x86_64-pc-windows-msvc` |
| Windows x64, GNU | `windows-2025` | `x86_64-pc-windows-gnu` |
| Windows ARM64, MSVC | `windows-11-arm` | `aarch64-pc-windows-msvc` |
| Windows ARM64, GNU/LLVM | `windows-11-arm` | `aarch64-pc-windows-gnullvm` |
| macOS Intel, Darwin libc | `macos-26-intel` | `x86_64-apple-darwin` |
| macOS ARM64, Darwin libc | `macos-26` | `aarch64-apple-darwin` |
| Android 15 x86-64, Bionic | `ubuntu-24.04` plus KVM emulator | `x86_64-linux-android` |

Rust has no `aarch64-pc-windows-gnu` target; its supported native ARM64
GNU-family target is `aarch64-pc-windows-gnullvm`. The matrix explicitly
excludes alternate macOS slots because Rust exposes only the Darwin libc ABI
for each host architecture. Failed monster lanes update and reopen one
`bug`/`agent-system` issue with the run, job, lane identity, and first useful
failure.

Fuzzing, mutation, benchmark regression, C ABI, and external E2E surfaces
are not part of current automation; their layers below remain plans owned
by their respective issues, and the monster workflow does not invent empty
targets for them. Golden determinism is a partial exception: single-platform
golden-frame tests run in the existing `test` required check (layer 5), but
the multi-platform matrix that would prove cross-platform determinism is
still a plan.

## 1. Round-trip and unit tests (Rust-input PRs)

- `decompress(compress(x)) == x` property tests over every generator class
  in the corpus policy (seeded, deterministic), plus edge sizes: empty, 1
  byte, exactly-window-sized, window+1.
- Every codec module (filter, parse, model, coder) gets unit tests for its
  own invariants — especially the ones the journal says were once implicit
  (rep-symbol/offset-bucket disjointness, stored floor).
- Every filter ships an exact-invertibility test (`unfilt(filt(x)) == x`)
  on data it targets AND data it doesn't.

## 2. Adversarial decode suite (Rust-input PRs)

The decoder's contract: **never panic, never overallocate, on any input.**

- In-repo `tests/adversarial/` seed corpus: truncations at every header
  boundary, bit-flips, declared-size lies (bombs), wrong magic/version,
  unknown methods, fuzz-found crashers (each promoted to a regression test
  with a comment naming the bug it caught).
- Tests assert graceful `Err`, never a panic; allocation stays bounded by a
  stated multiple of the declared output size.

## 3. Fuzzing (planned, not implemented)

- `cargo-fuzz` targets: `decode(arbitrary bytes)` must not panic;
  `roundtrip(arbitrary bytes)` must be identity; `decode` under an
  allocation limiter must respect bounds.
- When implemented, it runs on a schedule (nightly toolchain, time-boxed),
  not per-PR. New crashers land in `tests/adversarial/` as regression seeds.

## 4. Mutation testing (planned, not implemented)

- `cargo-mutants` on the codec modules; surviving mutants become issues —
  a surviving mutant is a missing test by definition.

## 5. Determinism (partial: single-platform golden files exist)

- `tests/golden/` (`JOURNAL` S2-A39) pins known input → known output per
  `FORMAT_VERSION`: `decompress(golden) == plaintext` and, for the
  current `FORMAT_VERSION`, `compress(plaintext) == golden`. Runs on
  every PR through the existing `test` required check, on the one
  runner that check already uses.
- What that does and does not prove: decode is integer-only end to end
  (JOURNAL S1-A5), so the decode half of this test is a real
  cross-platform guarantee. The encoder is not — `lz.rs` pricing and
  `filters.rs` filter scoring keep `f64::log2` as encoder-only floats
  (`docs/adr/0024-no-libm-on-the-decode-path.md` decision 3), which
  libm does not promise bit-identical across targets — so the re-encode
  half only pins today's toolchain/runner, not "every platform" as this
  section once claimed without a test to back it. Still planned: a
  multi-platform CI matrix actually comparing encoder output across
  `docs/TESTING.md`'s runtime table, which needs a `.github/workflows/`
  change reserved for whoever holds `GH_ADMIN_TOKEN`
  (`agents/GOVERNANCE.md`, "Push identity").
- Old-version frames stay decodable (CLAUDE.md rule 5): every historical
  `FORMAT_VERSION`'s golden pair is kept, never replaced, so this is a
  running test rather than a claim in a doc comment.

## 6. Differential oracle (during the M1 port)

- `research/imports/session-1/mothergod.rs` compiles standalone and is
  lossless-verified. While porting, compare the port against it: round-trip
  agreement on the corpus classes, and ratio within stated tolerance per
  dataset. Divergences are findings — either a port bug or an archive bug;
  journal them either way. The oracle is frozen; the port is what changes.

## 7. Benchmark regression gate (planned, not implemented)

- PRs fail on bits/byte regression vs `bench/baseline.json` beyond stated
  noise bounds, on the train tier only (sealing rules in POLICY.md).
  Ratio improvements that break layers 1–2 are rejected regardless — a
  faster-shrinking codec that panics on truncated input is a worse codec.
