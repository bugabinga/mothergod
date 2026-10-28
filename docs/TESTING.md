# Test strategy

What "tested" means for this project, in layers. Corpus rules live in
`research/corpus/POLICY.md`.

## Doctrine (ADR-0043)

Tests form a cost-tiered, number-audited portfolio. The required PR
gate stays fast and deterministic; everything expensive or statistical
runs on a schedule and alarms the fixer on red (ADR-0036) instead of
blocking PRs. A red scheduled check re-runs after the next push to main
(`recheck`, one retry per six hours, #1069), so a fix proves itself within
hours rather than at the next cron.

| Tier | When | Blocks PRs | What runs |
|---|---|---|---|
| Gate | every PR, required | yes | `cargo x check` stages + `ratio` |
| Advisory | every PR | no, annotates; survivors of a merged PR become a `bug` issue | `mutants-check` (diff-scoped) |
| Nightly | schedule | no, alarmed | fuzz with persistent corpus; structure-aware targets (#451); allocation torture (`torture-check`) |
| Weekly | schedule | no, alarmed | monster matrix, coverage, CLI memory bound (`memory-check`), Miri (#456) |
| Monthly | schedule | no, alarmed | whole-crate mutation sweep (`mutants-sweep`, sharded) |

Effectiveness is audited by the trust ledger (#449): fuzz CPU-hours,
new crashers, mutation score, and region coverage, rendered on the
status page and judged in the weekly digest. Mechanism: each scheduled
test workflow writes its own small `entry.json`, uploaded as the artifact
`trust-<role>-<run-id>-<attempt>` for 90 days and written as one row of
the `trust` table in D1 by `.github/scripts/trust-row.py` (ADR-0060,
#891 slice 4), never a committed file (a tracked file collects
concurrent-append conflicts, PR #34). The artifact is the record a human
opens; the row is the series, reconciled from the artifacts every Sunday.
`.github/scripts/trust-telemetry.py` aggregates the table into
`site/trust-data.json` at deploy time, cumulative since the first row,
and `status.html` renders it under "Is it tested?". Writers landed:
`fuzz-check`, timed fuzz seconds and new-crasher count per run (#462);
`coverage-check`, region coverage percent per run (#454);
`mutants-sweep`, mutation score percent per monthly sweep (#455).
Ledger numbers are maps, never gates; the only merge-blocking
checks are behavioral. An issue number is a pointer to the issue that
tracked the work; a row not yet built says "planned" in its own cell.

## Two classes (ADR-0056)

Tests are two artifacts with different owners and bars.

**Scaffold tests** are what an implementer writes beside code to see it
work: inline unit tests on private helpers, hand-computed pins of
intermediate values, examples of a function's intent. A practice, not a
deliverable. The implementer writes as many as the work needs, nobody
else writes them, nobody maintains them: a scaffold test is deleted with
the code it pins, and no replacement is owed when a suite layer states
the function's observable claim. A surviving mutant in scaffold
territory is a map reading, not debt.

**The suite** is the engineered artifact: the layers below that state a
contract (the public API, the format, a module invariant the journal
names, a published claim) and name the failure class they catch. A suite
test is written against the contract, never against an implementation's
intermediate values, so a model change does not rewrite it. Location is
not the class: a module invariant pinned inline is suite; a hand-computed
pin in `tests/` would be scaffold.

A change owes the suite exactly what its class demands, in the same PR;
the reviewer checks this table, not the test count.

| Change class | Owed to the suite, same PR |
|---|---|
| Codec behavior: filter, parse, model, coder | round-trip on the change's target data class (hard rule 1), a property or example at layer 1 |
| Decode-path fix: panic, bound, hang, abort | the failing input as a `tests/adversarial/` seed named for the bug, or a torture case for an allocation (layer 2) |
| Encoder-only output change | the regenerated current-version golden pair, declared with its measurement (layer 5, #290) |
| Format change | `FORMAT_VERSION` bump, a new golden pair; before the first release the predecessor and its pairs are retired in the same PR, after it every old pair is kept (layer 5, hard rule 5, ADR-0063) |
| New or changed decode API | the decode-API agreement property covers it (layer 1) |
| Published number, version, date or recipe in README, site or CHANGELOG | `tests/claims.rs` reads it from its source |
| Experiment apparatus, measurement sinks, pre-wiring candidates | nothing: scaffold only, deleted by the wiring or rejection slice |
| A new suite instrument: fuzz target, torture fixture, strategy | its own M7 issue, never a rider on a feature PR |

Module tests live beside product code, not inside it:
`src/<module>/tests.rs`, declared by `#[cfg(test)] mod tests;`, so a
product file reads in one pass and test lines count by file name.

## Current automated cadence

Every PR retains the required job names ci.yml's header lists. Rust-input
PRs run `fmt`, `clippy`, `test` and `doc` on Linux x64 stable through
`.github/actions/rust-ci`, each job delegating to its `cargo x` stage so CI
and the local gate share one command list (ADR-0029): formatting, Clippy,
all Cargo targets, doctests,
and warning-clean rustdoc output. `ratio` is the benchmark regression gate
(layer 7): it runs the `baseline_gate` binary directly, because it is not a
stage of x's quality gate. PRs touching no gate input receive successful
skips through the path filter. `worker` runs the node tests under `infra/`
and `.github/`, the prompt pins among them (`prompt-bytes`). Those six
names, listed in ci.yml's header, are the repository ruleset contract.

The advisory `fuzz-check` workflow runs nightly at 02:13 UTC and on
manual dispatch: three `fuzz/` targets, 10 minutes each, Linux x64 only,
resuming from a corpus persisted across runs and minimized with `cmin`
(layer 3, #450). A found crasher fails the job, uploads
`fuzz/artifacts/`, and wakes the fixer through the alarm (ADR-0036).
Every run, crash or clean, also uploads its trust-ledger entry (#449).

The advisory `torture-check` workflow runs nightly at 01:31 UTC and on
manual dispatch: `tests/torture.rs` with `MOTHERGOD_TORTURE=1`, release
profile, every fixture through every decode API (layer 2, #791). An
abort fails the job, named by fixture, API and call index in the log, and
wakes the fixer through the alarm. No ledger entry: the sweep is a gate
on a rule, not a number.

The advisory `memory-check` workflow runs Saturdays at 06:29 UTC and on
manual dispatch: `.github/scripts/memory-check` on the release CLI, 256
MiB of zeros plus one byte (17 frames) and then 36 MiB of seeded random
words (dense short matches, the token lists zeros barely grows), each
through encode and then decode, each child's peak resident set read from
the kernel (#1109, #1396; the claim is CHANGELOG.md's "under 1 GiB however
long the input"). A round trip that
differs or a peak at or over 1 GiB fails the job, named in the footer,
and wakes the fixer through the alarm. No ledger entry: the bound is the
gate, and the readings print beside it in the log, so a peak climbing
toward the bound is visible before it crosses. Weekly, not per PR,
because the run takes minutes in release; the script's docstring carries
the logged run time. A seat that needs the number runs the script instead
of a `time(1)` recipe of its own.

The advisory `mutants-check` workflow runs on pull requests that touch
`src/` or the crate's build inputs, mutating only the changed lines
(layer 4).

The advisory `mutants-sweep` workflow runs on the first of each month at
07:41 UTC and on manual dispatch: every mutant of `src/` under
`tests/mutants.toml`, split across ten shards of `cargo mutants --shard`,
merged by `.github/scripts/mutants-sweep` (layer 4, #455). The mutation
score, caught over caught plus missed, goes to the trust ledger; it is
written null when a shard is missing or was cut by its budget, because a
ratio over part of the crate is not the crate's score. Decode-safety
survivors and undecided mutants land in one refreshed `product` + `bug`
issue, minus the lines an open `mutants-debt` issue already carries;
scaffold survivors are counted per file and left in the run's artifact.
Never a gate; the workflow header carries the measured cost and the
cadence decision.

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
| FreeBSD 15 x86-64, libc | `ubuntu-24.04` plus KVM guest | `x86_64-unknown-freebsd` |

Rust has no `aarch64-pc-windows-gnu` target; its supported native ARM64
GNU-family target is `aarch64-pc-windows-gnullvm`. The matrix explicitly
excludes alternate macOS slots because Rust exposes only the Darwin libc ABI
for each host architecture. Failed monster lanes update and reopen one
`bug`/`product` issue with the run, job, lane identity, and first useful
failure.

The Android lane runs each test executable on the emulator after
`.github/scripts/android-runner` pushes it with the golden and adversarial
fixtures, and nothing else: a test that spawns a sibling binary by its
`CARGO_BIN_EXE_*` path finds a host path that does not exist on the device.
Such a test carries `#[cfg_attr(target_os = "android", ignore = "...")]`
with this reason, or its file sits behind `cfg(not(target_os = "android"))`
when every test in it spawns (`tests/cli.rs`). #518 stayed red a week;
#1588 was caught by the first weekly run after the tests landed, because no
PR lane runs Android.

The FreeBSD lane builds and tests inside the guest, so the spawn guard
does not apply there; the guest installs the toolchain from rustup's
FreeBSD binary at the version the host resolved, so the lane shares the
one MSRV reader.

The advisory `coverage-check` workflow runs Sundays at 04:51 UTC and on
manual dispatch: `cargo llvm-cov --package mothergod --json`, region
granularity (layer 9, #454). The crate percentage goes to the trust
ledger; the three worst-covered files print as `::notice::` annotations
for whoever picks up the triage. Never a gate.

Mutation, C ABI, and external E2E surfaces
are not part of current automation; their layers below remain plans owned
by their respective issues, and the monster workflow does not invent empty
targets for them. Golden determinism is covered twice: golden-frame tests
run in the existing `test` required check on every PR (layer 5), and the
weekly monster matrix runs them on every runtime target.

## 1. Round-trip and unit tests (Rust-input PRs)

The properties, the edge sizes and the named invariants here are suite;
the rest of a module's inline tests are scaffold ("Two classes" above).

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
  cycle — a bigger, separate design question). The weekly
  `PROPTEST_CASES` scale-up is declined (#495): the daily fuzzer sweeps
  the same legs, `roundtrip` including the writer. `PROPTEST_CASES`
  stays for a local heavier run.

## 2. Adversarial decode suite (Rust-input PRs)

The decoder's contract: **never panic, never overallocate, on any input.**

- In-repo `tests/adversarial/` seed corpus: truncations at every header
  boundary, bit-flips, declared-size lies (bombs), wrong magic/version,
  unknown methods, fuzz-found crashers (each promoted to a regression test
  with a comment naming the bug it caught).
- Tests assert graceful `Err`, never a panic, and each seed is pinned to
  the `Error` variant it must reach (`PINS` in `tests/adversarial.rs`), so a
  seed an earlier check rejects cannot pass for the check it is named for;
  allocation stays bounded by a stated multiple of the declared output size.
- **Integrity probe of a PR's own input (#1210).** `cargo run -p
  mothergod-bench --release --bin frame_probe -- <file>...` compresses each
  file and decodes every truncation and single-bit flip of its frame,
  exhaustive below `--max-decodes` each (default 20000) and a seeded sample
  across the frame above it. It prints `bytes`, `flips` and `truncations`
  (each `<decoded> of <offered>`, so a capped run states its coverage) and
  `silent` per file and exits non-zero on any silent wrong output. The probe
  is `tests/fit/integrity.rs`, shared with the FIT row `integrity-check`.
  Files over one 16 MiB block are refused: several frames make a truncation
  at a boundary a valid stream.
- **Allocation-failure torture (#453).** The bound above stops decode from
  asking for too much; it does not prove decode survives the allocator
  actually refusing a request. `tests/torture.rs` (`cargo test --test
  torture`, `harness = false` so it stays a fast no-op under the default
  gate) counts every allocator call one decode makes, then re-runs it once
  per call in a fresh child process with that one call sabotaged, and
  checks the child's exit status for a signal or a panic rather than a
  graceful `Err`. `MOTHERGOD_TORTURE=1` opts in to the real sweep. Covered:
  every golden fixture and adversarial seed through `decompress`,
  `decompress_to_writer` and `read::Decoder`, so `codec::decode`'s
  `output` buffer, `Models::try_new`'s tables and the `Delta`/`Transpose`
  undo buffers are all fallible and swept. No fixture selects the `Bcj`
  candidate (filter selector kind 2), so `Bcj`'s undo buffer is fallible
  by code inspection and unswept. Scheduled: the advisory `torture-check`
  workflow runs the sweep nightly at 01:31 UTC and on manual dispatch,
  red on any abort and alarmed to the BDFL (#791).

## 3. Fuzzing (scheduled: `fuzz-check`, nightly)

- `cargo-fuzz` targets in `fuzz/` (`JOURNAL` S2-A25): `decode_arbitrary`
  (decode of arbitrary bytes must not panic) and `roundtrip`
  (`decompress(compress(x)) == x` for arbitrary `x`), plus the
  structure-aware pair below. `fuzz-check` runs them nightly (nightly
  toolchain, Linux x64), not per-PR (`JOURNAL` S2-A53). It does not name
  its targets: it fuzzes whatever `cargo fuzz list` reports, so a target
  added to `fuzz/Cargo.toml` is fuzzed the next night with no
  workflow-file change. The nightly's wall clock is the budget
  (`FUZZ_BUDGET_S`, 1800s) and the target count is the divisor, so a new
  target costs the others depth rather than costing runner minutes; the
  step fails loudly rather than shrink a slice below 60s. The corpus
  persists across runs through the actions cache and is
  `cmin`-minimized after each clean run, so
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
  mostly rediscovering `BadMagic`/`Truncated` (it also re-seals each
  flipped frame with the check value of ADR-0068, as `decode_arbitrary`
  does its raw bytes, or `BadChecksum` would turn away nearly every one); and `bin/seed_corpus.rs`,
  writing `frame_gen`'s output as corpus seeds (measured gain on
  `decode_arbitrary`, this build: libFuzzer's own single generated seed
  reaches cov 26/ft 27, `frame_gen`'s 12 seeds alone reach cov 418/ft
  859 before a single mutation runs). A fourth target, `frame_recipe`,
  derives `Arbitrary` on
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
  runs).

## 4. Mutation testing (`mutants-check`, per PR)

- `cargo-mutants` on the codec package, scoped to the lines a PR changed
  (`--in-diff` against the merge base). A surviving mutant is a missing
  suite test where the suite makes the claim the mutation breaks, and a
  map reading elsewhere (ADR-0056, "Two classes" above); the PR that
  created one goes red and names it, so the reviewer reads it on the diff.
- Timeouts do not red a PR: a mutant whose test run hangs is undecided, not
  evidence of a missing test. The verdict reads `mutants.out/missed.txt`,
  never the exit code, which conflates the two.
- Advisory, not a required check, until four weeks pass with no false
  positive. Missed mutants appear as annotations on the changed lines.
- Mutants no test can decide are excluded in `tests/mutants.toml`, each
  with its proof (#391's ruling); its header says why it does not live
  in `.cargo/`.
- A red that finishes after the PR merged has no reader on the thread, so
  `.github/scripts/mutants-debt` files that PR's survivors in decode-safety
  territory (hard rule 2's claim, #390's class) as one `product`+`bug`
  issue, slot a of the heartbeat's product queue; survivors outside it are
  scaffold territory and are not filed. The territory rule is executable
  in the script, and its docstring is the contract. An open PR's red stays
  on the diff, where the review reads it. A mutate step cut by its budget
  leaves mutants listed and never run; the same script prints
  `undecided N of M` on every run and files a merged PR's decode-safety
  ones with its survivors, because an untested line on main is debt in the
  same sense (#947).
- The whole crate is swept monthly by `mutants-sweep`, sharded, the score
  to the trust ledger and the decode-safety survivors in one refreshed
  issue (#455): the backstop for every line a PR gate never mutates again.

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
- Old-version frames stay decodable once a release has written them
  (CLAUDE.md rule 5, ADR-0050): from then on every historical
  `FORMAT_VERSION`'s golden pair is kept, never replaced, so this is a
  running test rather than a claim in a doc comment. Before the first
  release no version outside the current one is read, and a bump deletes
  its predecessor's pairs in the same PR (ADR-0063).
- An encoder-only change (no decode difference) is not a `FORMAT_VERSION`
  bump; it moves the current-version pair to `tests/golden/superseded/`
  (decode-only from then on) and regenerates the pair in `tests/golden/`,
  declared in the PR body with the measured justification, the
  `bench/baseline.json` pattern (issue #290's ruling; mechanics in
  `tests/golden.rs`'s module doc).
- `tests/parse_digest.rs` pins one digest of `parse_optimal`'s and one of
  `parse_greedy`'s token stream over generated inputs and every golden
  plaintext, because golden pins whole frames on ten plaintexts and a
  parse change confined to long rep runs or the carry cache passes it.
  A refactor claiming no behaviour change cites it green; an encoder-only
  change regenerates its constants under the same declaration rule as the
  golden pair (#1524).

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
- The same `baseline_gate check` invocation also fails when a gate case
  compresses to different bytes than the digest `bench/baseline.json`
  commits (issue #1182), whatever the size: it is the CI pin on
  `compress()`'s output over the fixed inputs, so a refactor's
  "bitstream unchanged" is a green check and an output change is declared
  by `baseline_gate -- write` in the same PR, the rule an encoder-only
  change already follows for `tests/golden.rs`. One of those inputs is
  longer than the rest, a repeat past the standard match reach
  (`baseline::reach_case`, issue #1434): the others sit inside it and
  cannot feel a reach-class change.
- The held-out finals reports (`docs/benchmarks/canterbury.md`,
  `docs/benchmarks/silesia.md`) are not checked by `baseline_gate`: they
  fetch real corpora over the network, too slow and non-hermetic for every
  PR, and their one writer is the publish PR cut from the pass on main
  (ADR-0076, issue #1437).

## 8. Allocation torture (landed, #453)

- The curl/SQLite mechanism: count a passing decode's allocations,
  re-run failing the k-th allocation for every k, assert graceful `Err`,
  never a panic or abort. A test-only `#[global_allocator]` in its own
  integration-test binary, `tests/torture.rs`. Every abort the sweep finds
  is a place decode grows memory before validating input, which is hard
  rule 2's audit.

## 9. Coverage map (scheduled: `coverage-check`, weekly)

- Weekly cargo-llvm-cov region coverage, published to the trust ledger
  and status page with trend. Never a gate: a coverage target
  manufactures assertions. It feeds triage: a stalled number sends the
  worst-covered module to the heartbeat as one scoped issue.

## 10. UB insurance (landed, #456)

- Weekly `cargo miri test` lane, the `miri` job in
  `.github/workflows/monster.yml`. The crate forbids unsafe,
  so Miri is insurance that the claim stays true transitively, priced
  by test selection (`cfg(miri)` excludes the slow storms).

## 11. FFI / C ABI end-to-end (planned, #41)

- Gated on the Rust API stabilizing (1.0); not scheduled work today.
  When scheduled: a C test harness links the `cdylib` crate's stable C
  ABI (`mothergod.h`) and runs round-trip, adversarial-input, and
  allocation-bound suites through the C surface itself, in CI —
  proving no panic crosses the FFI boundary, not just that the Rust
  API behaves.

## 12. Genre conformance (CLI #981, library #982)

- The FIT matrix (ADR-0064): one row per behavior the compressor genre
  shares (gzip, bzip2, xz, zstd, brotli, lz4 for the CLI; flate2, zstd,
  xz2, brotli, lz4_flex for the library), each row carrying its citation
  and a recorded state, `holds` or `gap`. A `holds` row that fails is a
  regression; a `gap` row that passes fails until the record flips; a
  declined row keeps its place in the denominator with its reason.
- The CLI half is `tests/fit/cli.txt` (the rows) and `tests/fit_cli.rs`
  (one probe per row, spawning the binary). The file is the committed
  count: `status-data.py` counts its `holds` rows over all rows, so no
  second number exists to drift. A probe returns `Seen::Unmeasured` on a lane
  that cannot run it (`script(1)` for the terminal row is Linux only and
  must pass its own pty control, the permission-bit row is Unix only) and the row is then unmeasured there.
  Citations are read from the runner's man pages, never from memory.
  The suite spawns the binary, so it runs on no Miri lane and carries the
  Android guard stated under "Current automated cadence".
- The library half is `tests/fit/lib.txt` and `tests/fit_lib.rs`, the
  same format and the same two checks (`tests/fit/matrix.rs`). A row
  with a file in `tests/fit/lib/` is probed by compiling that file with
  `rustc` against the `libmothergod` rlib beside the test binary, the
  idiom as a user writes it; a control snippet must link first, or the
  suite fails instead of reading every refusal as a gap. The other
  rows run the behavior. Citations are read from the genre crates'
  registry source in a scratch crate outside the checkout, never
  fetched. The suite spawns `rustc`, so it runs on no Miri lane and
  carries the Android guard stated under "Current automated cadence".
