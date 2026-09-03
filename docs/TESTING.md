# Test strategy

What "tested" means for this project, in layers. Corpus rules live in
`research/corpus/POLICY.md`.

## Doctrine (ADR-0043)

Tests form a cost-tiered, number-audited portfolio. The required PR
gate stays fast and deterministic; everything expensive or statistical
runs on a schedule and alarms the fixer on red (ADR-0036) instead of
blocking PRs.

| Tier | When | Blocks PRs | What runs |
|---|---|---|---|
| Gate | every PR, required | yes | `cargo x check` stages + `ratio` |
| Advisory | every PR | no, annotates | `mutants-check` (diff-scoped) |
| Nightly | schedule | no, alarmed | fuzz with persistent corpus; structure-aware targets (#451) |
| Weekly | schedule | no, alarmed | monster matrix, large property profile (#452), coverage (#454), Miri (#456) |
| Monthly | schedule | no, alarmed | whole-crate mutation sweep (#455) |

Effectiveness is audited by the trust ledger (#449): fuzz CPU-hours,
new crashers, mutation score, and region coverage, rendered on the
status page and judged in the weekly digest. Mechanism: each scheduled
test workflow is meant to upload its own small `entry.json` artifact
(`trust-<role>-<run-id>-<attempt>`), never a committed file — the same
stateless-artifact pipeline `run-telemetry.py` already uses for run
economics, and for the same reason (a tracked file collects
concurrent-append conflicts, PR #34). `.github/scripts/trust-telemetry.py`
aggregates them into `site/trust-data.json` at deploy time, and
`status.html` renders them under "Is it tested?". Writers landed:
`fuzz-check`, timed fuzz seconds and new-crasher count per run (#462).
Writers pending: mutation score (#455) and region coverage (#454),
shown as not yet measured until their sweeps land.
Ledger numbers are maps, never gates; the only merge-blocking
checks are behavioral. Items carrying issue numbers are planned; the
layers below describe what runs today.

## Current automated cadence

Every PR retains the required job names `fmt`, `clippy`, `test`, `doc`, and
`ratio`. Rust-input PRs run the first four on Linux x64 stable through
`.github/actions/rust-ci`, each job delegating to its `cargo x` stage so CI
and the local gate share one command list (ADR-0029): formatting, Clippy,
all Cargo targets, doctests,
and warning-clean rustdoc output. `ratio` is the benchmark regression gate
(layer 7): it runs the `baseline_gate` binary directly, because it is not a
stage of x's quality gate. PRs touching no gate input receive successful
skips through the path filter. Those five names are the repository ruleset
contract.

The advisory `fuzz-check` workflow runs nightly at 02:13 UTC and on
manual dispatch: three `fuzz/` targets, 10 minutes each, Linux x64 only,
resuming from a corpus persisted across runs and minimized with `cmin`
(layer 3, #450). A found crasher fails the job, uploads
`fuzz/artifacts/`, and wakes the fixer through the alarm (ADR-0036).
Every run, crash or clean, also uploads its trust-ledger entry (#449).

The advisory `mutants-check` workflow runs on pull requests that touch
`src/` or the crate's build inputs, mutating only the changed lines
(layer 4). A whole-crate sweep runs on manual dispatch only.

The advisory `monster` workflow runs Saturdays at 03:17 UTC and on manual
dispatch, never on pull requests. Every runtime lane runs
`cargo test --all-targets` and `cargo test --doc` on stable and the root
`Cargo.toml` package's `rust-version`; the workflow reads that declaration
instead of copying the MSRV. The canonical `ubuntu-24.04` x64/glibc stable
lane instead runs the whole `cargo x check` gate natively.

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

Mutation, C ABI, and external E2E surfaces
are not part of current automation; their layers below remain plans owned
by their respective issues, and the monster workflow does not invent empty
targets for them. Golden determinism is covered twice: golden-frame tests
run in the existing `test` required check on every PR (layer 5), and the
weekly monster matrix runs them on every runtime target.

## 1. Round-trip and unit tests (Rust-input PRs)

- `decompress(compress(x)) == x` property tests over every generator class
  in the corpus policy (seeded, deterministic), plus edge sizes: empty, 1
  byte, exactly-window-sized, window+1.
- Every codec module (filter, parse, model, coder) gets unit tests for its
  own invariants — especially the ones the journal says were once implicit
  (rep-symbol/offset-bucket disjointness, stored floor).
- Every filter ships an exact-invertibility test (`unfilt(filt(x)) == x`)
  on data it targets AND data it doesn't. `delta`, `transpose`, and `bcj`
  have this as a `proptest` property (`#452`), the crate's first
  dev-dependency, swept with automatic shrinking; the hand-written
  examples stay as named anchors per the escalation ladder. `delta` and
  `transpose` sweep arbitrary data and stride/column count; `bcj` sweeps
  data biased toward its `0xE8`/`0xE9` opcodes, since uniform bytes hit
  either on only 2/256 draws and would rarely exercise the rewrite branch
  (`test-craft`'s "generate the distribution the codec targets"). The
  model/coder round-trip loops are still hand-rolled.
- Decode-API differential agreement (`decompress`, `decompress_bounded`,
  `decompress_to_writer` must all agree on every frame `compress` can
  produce) is a `proptest` property in `src/lib.rs` (`#452`), swept over a
  mix of noise and repeated patterns so both `Method::Stored` and
  `Method::Lz` frames get exercised. Case count is trimmed to 64 (a
  quarter of the filter properties' default 256): `compress` runs the
  full optimal-parse LZ encoder, far heavier per case than a filter
  round-trip.
- `lz::parse_greedy`/`parse_optimal` each carry a `replay(parse(x)) == x`
  proptest property (`src/lz.rs`, `#452`), swept over the same structural
  classes the module's hand-written `roundtrip_*` examples anchor
  individually: unstructured noise, a short pattern repeated (forces a
  Match/Rep token), a long single-byte run (the overlapping-distance copy
  path), and a low-period pattern (exercises the rep cache). At default
  case count (256): `parse_optimal`'s three-round DP is heavier per case
  than a filter round-trip, so data length stays under a few hundred
  bytes rather than trimming cases. `model::Model` and `coder`'s raw
  `Encoder`/`Decoder` (via its `FreqTable` test double) each carry the
  same `roundtrip_symbols(symbols, alphabet)` property, swept over
  alphabet size 2..64 and stream length 0..300 (`#452`). Still open
  (`#452`): porting any of these properties to the corpus policy's
  generator classes, currently only in the `bench` crate (`bench` depends
  on `mothergod`, so pulling them in as a root dev-dependency would be a
  cycle — a bigger, separate design question), and a `PROPTEST_CASES`
  weekly-scale-up profile, which no workflow sets yet.

## 2. Adversarial decode suite (Rust-input PRs)

The decoder's contract: **never panic, never overallocate, on any input.**

- In-repo `tests/adversarial/` seed corpus: truncations at every header
  boundary, bit-flips, declared-size lies (bombs), wrong magic/version,
  unknown methods, fuzz-found crashers (each promoted to a regression test
  with a comment naming the bug it caught).
- Tests assert graceful `Err`, never a panic; allocation stays bounded by a
  stated multiple of the declared output size.
- **Allocation-failure torture (#453).** The bound above stops decode from
  asking for too much; it does not prove decode survives the allocator
  actually refusing a request. `tests/torture.rs` (`cargo test --test
  torture`, `harness = false` so it stays a fast no-op under the default
  gate) counts every allocator call one decode makes, then re-runs it once
  per call in a fresh child process with that one call sabotaged, and
  checks the child's exit status for a signal or a panic rather than a
  graceful `Err`. `MOTHERGOD_TORTURE=1` opts in to the real sweep. Landed:
  `codec::decode`'s `output` buffer (`Error::OutOfMemory`), the one
  allocation whose size is attacker-controlled rather than fixed by the
  model tables. Still infallible: `Models::new`'s fixed-size tables and the
  `Delta`/`Transpose` filters' undo buffers, both found by the sweep, and
  `Bcj`'s undo buffer, the same shape by code inspection but not swept by
  any current fixture (none selects the `Bcj` candidate) — separable
  follow-up on #453, which stays open. Not yet on a schedule (needs a
  workflow-file change); run manually until it is.

## 3. Fuzzing (scheduled: `fuzz-check`, nightly)

- `cargo-fuzz` targets in `fuzz/` (`JOURNAL` S2-A25): `decode_arbitrary`
  (decode of arbitrary bytes must not panic) and `roundtrip`
  (`decompress(compress(x)) == x` for arbitrary `x`). `fuzz-check` runs
  both nightly (nightly toolchain, 10 minutes per target, Linux x64),
  not per-PR (`JOURNAL` S2-A53). The corpus persists across runs through
  the actions cache and is `cmin`-minimized after each clean run, so
  coverage compounds instead of restarting cold (#450). New crashers
  land in `tests/adversarial/` as regression seeds. Every run, crash or
  clean, uploads a trust-ledger entry (#449, above): fuzz seconds and
  new-crasher count, timed around the run itself so a crasher's early
  exit still reports true elapsed time.
- Still planned: an explicit allocation-limiter target beyond
  `MAX_DECODED_LEN`'s bound, and cross-OS fuzz coverage in `monster`.
- Issue #451 (zstd's `decodecorpus`/SQLite's `dbsqlfuzz` analog):
  `frame_gen.rs`, a deterministic generator of valid frames (inputs
  already proven, in `src/filters.rs`'s/`src/lz.rs`'s own unit tests, to
  drive every `filters::select::Candidate` kind and the rep cache,
  compressed through the real `mothergod::compress` so every returned
  frame is valid by construction); a third libFuzzer target,
  `frame_mutate`, that applies byte flips near a `frame_gen` frame so
  mutation explores the decoder's post-header state machine instead of
  mostly rediscovering `BadMagic`/`Truncated`; and `bin/seed_corpus.rs`,
  writing `frame_gen`'s output as corpus seeds (measured gain on
  `decode_arbitrary`, this build: libFuzzer's own single generated seed
  reaches cov 26/ft 27, `frame_gen`'s 12 seeds alone reach cov 418/ft
  859 before a single mutation runs). All three run in `fuzz-check`
  (#475/#480). A fourth target, `frame_recipe`, derives `Arbitrary` on
  `frame_gen::PreimageRecipe` — a small preimage-shape parameter space
  (repeated-byte runs, byte cycles, columnar drift, opcode-dense BCJ
  data, pseudo-random noise, each with a fuzzer-controlled length/seed)
  — so libFuzzer mutates in that shape space directly, unlike
  `frame_mutate`'s byte flips near an already-encoded frame: issue
  #451's remaining "structure-aware `Arbitrary`-over-token-structures"
  scope. `bin/seed_corpus.rs` now seeds it too, one file per
  `PreimageRecipe` variant found by searching a deterministic byte
  stream and decoding it back rather than hand-deriving `arbitrary`'s
  enum layout (measured gain: a cold `frame_recipe` run's first
  candidate fails to decode into any shape at all, 0 useful coverage;
  the 5 seeds alone reach cov 1703/ft 3230 before a single mutation
  runs). Not yet in `fuzz-check`: same admin-PAT workflow-file gate as
  before (issue #24, BDFL-only per `agents/GOVERNANCE.md`), filed as
  #492.

## 4. Mutation testing (`mutants-check`, per PR)

- `cargo-mutants` on the codec package, scoped to the lines a PR changed
  (`--in-diff` against the merge base). A surviving mutant is a missing test
  by definition, so the PR that created one goes red and names it, rather
  than a later sweep filing an issue about it.
- Timeouts do not red a PR: a mutant whose test run hangs is undecided, not
  evidence of a missing test. The verdict reads `mutants.out/missed.txt`,
  never the exit code, which conflates the two.
- Advisory, not a required check, until four weeks pass with no false
  positive. Missed mutants appear as annotations on the changed lines.
- Whole-crate sweeps run on manual dispatch, not a schedule: 1,531 mutants
  at ~3.8h is a one-time backlog measurement, not weekly news.
- Planned (#455): a monthly sharded whole-crate sweep, mutation score to
  the trust ledger, survivors in one refreshed issue.

## 5. Determinism

- `tests/golden/` (`JOURNAL` S2-A39) pins known input → known output per
  `FORMAT_VERSION`: `decompress(golden) == plaintext` and, for the
  current `FORMAT_VERSION`, `compress(plaintext) == golden`. Runs on
  every PR through the existing `test` required check, and weekly on
  every runtime target through the monster matrix; the Android lane gets
  the fixtures pushed to the emulator and pointed at via
  `MOTHERGOD_GOLDEN_DIR` (`.github/scripts/android-runner`).
- What that does and does not prove: decode is integer-only end to end
  (JOURNAL S1-A5), so the decode half of this test is a real
  cross-platform guarantee. The encoder is not — `lz.rs` pricing and
  `filters.rs` filter scoring keep `f64::log2` as encoder-only floats
  (`docs/adr/0024-no-libm-on-the-decode-path.md` decision 3), which
  libm does not promise bit-identical across targets — so the re-encode
  half is a regression pin per libm, not a guarantee. The monster matrix
  runs it anyway, and every libm tried so far (glibc, musl, MSVC CRT,
  mingw, Darwin) agrees on the committed fixtures; a re-encode failure on
  one platform only is that platform's libm disagreeing, a finding to
  record against ADR-0024's boundary, not a decode regression.
- Old-version frames stay decodable (CLAUDE.md rule 5): every historical
  `FORMAT_VERSION`'s golden pair is kept, never replaced, so this is a
  running test rather than a claim in a doc comment.
- An encoder-only change (no decode difference) is not a `FORMAT_VERSION`
  bump; it moves the current-version pair to `tests/golden/superseded/`
  (decode-only from then on) and regenerates the pair in `tests/golden/`,
  declared in the PR body with the measured justification, the
  `bench/baseline.json` pattern (issue #290's ruling; mechanics in
  `tests/golden.rs`'s module doc).

## 6. Differential oracle (during the M1 port)

- `research/imports/session-1/mothergod.rs` compiles standalone and is
  lossless-verified. While porting, compare the port against it: round-trip
  agreement on the corpus classes, and ratio within stated tolerance per
  dataset. Divergences are findings — either a port bug or an archive bug;
  journal them either way. The oracle is frozen; the port is what changes.

## 7. Benchmark regression gate (required `ratio` check)

- PRs fail on bits/byte regression vs `bench/baseline.json` beyond
  `baseline::TOLERANCE_BITS`, on the fixed-seed gate cases only (sealing
  rules in POLICY.md). The measurement is deterministic, so a red is a
  real regression, never noise; an accepted ratio trade updates
  `bench/baseline.json` in the same PR with the reason in the PR body.
  Ratio improvements that break layers 1–2 are rejected regardless — a
  faster-shrinking codec that panics on truncated input is a worse codec.
- The same `baseline_gate check` invocation also fails when
  `docs/benchmarks/canterbury.md` or `docs/benchmarks/silesia.md` embeds a
  `bench/baseline.json` fingerprint (`crate::baseline::fingerprint`) that
  no longer matches the committed file (issue #327): a baseline change is
  a deliberate signal the codec's measured behavior changed, so the
  held-out finals numbers can now be stale. A content check against the
  committed reports, not a regeneration: those two reports fetch real
  corpora over the network, too slow and non-hermetic to run on every PR.

## 8. Allocation torture (planned, #453)

- The curl/SQLite mechanism: count a passing decode's allocations,
  re-run failing the k-th allocation for every k, assert graceful `Err`,
  never a panic or abort. A test-only `#[global_allocator]` in its own
  integration-test binary. Every abort the sweep finds is a place decode
  grows memory before validating input, which is hard rule 2's audit.

## 9. Coverage map (planned, #454)

- Weekly cargo-llvm-cov region coverage, published to the trust ledger
  and status page with trend. Never a gate: a coverage target
  manufactures assertions. It feeds triage: a stalled number sends the
  worst-covered module to the heartbeat as one scoped issue.

## 10. UB insurance (planned, #456)

- Weekly `cargo miri test` lane in monster. The crate forbids unsafe,
  so Miri is insurance that the claim stays true transitively, priced
  by test selection (`cfg(miri)` excludes the slow storms).
