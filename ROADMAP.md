# Roadmap

## Mission

Build the best general-purpose lossless compressor — "mother god of all
general purpose compressors" — as a real open-source project that **real
human users** choose, trust, and enjoy. Three non-negotiables define "best":

1. **Trustworthy**: lossless always, decoder safe on any input, deterministic
   across platforms. A ratio win that costs trust is a loss.
2. **Honest**: every claim measured on named corpora with real bitstreams,
   every design decision traceable to a recorded experiment. We beat the
   incumbents on their benchmarks, not ours. Honesty extends to marketing:
   no astroturfing, no manufactured engagement, ever.
3. **Wanted**: the target audience is people, not benchmarks. Ease of
   building, integrating, and understanding the project are first-class
   outcomes; the more happy users, the better. A technically superior
   compressor nobody adopts has failed.

Guiding principles:

- **The less code, the better.** Simplicity is a feature; every line is a
  liability some future session must understand and maintain. Prefer
  deleting to adding; quality and performance come from design, not
  accretion.
- **Beat the competition, and learn from it shamelessly.** Study how zstd,
  lz4, brotli, xz — and great OSS beyond compression (ripgrep, SQLite,
  curl) and OSS history at large — do engineering, docs, releases, and
  community. Write down what was learned and applied.
- **Every aspect of open source is in scope**, not just code: README first
  impressions, docs, release notes, the blog, positioning, community tone.
  The BDFL steers all of it — and **publishes only on channels mothergod
  owns** (this repo, its blog, its releases). External platforms — Hacker
  News, lobste.rs, reddit, socials — are queried as success proxies, never
  posted to by the system; any thread there is organic or the operator's
  own doing.

The BDFL owns this mission; it runs hourly (ADR-0015), judges the
project against the scorecard below — in full on its weekly deep run — and
reports in the ops-log digest. Its default is to solve problems by improving
the agent system itself, not by one-off work. A metric that cannot yet be
measured is itself a top gap — the BDFL schedules the work that makes it
measurable before the work it would measure.

**Amendment clause (ADR-0011).** This Mission section — the mission
statement, the three non-negotiables, and the guiding principles above — is
the one thing in this repository agents do not change. Amendments are the
operator's alone; the BDFL proposes them via `blocked-on-human`. Everything
below this section, and everything else in the project — name, logo,
architecture, code, roadmap, processes — is the BDFL's to change (ADR-0011).

## Scorecard

Outcome metrics (the product):

- **RATIO** — aggregate bits/byte on the held-out finals (whole-file
  Silesia + Canterbury, real bitstreams) vs pinned `gzip -9`, `zstd -19`,
  `xz -9e`. Success ladder: (1) reclaim the founding v0.6 standing —
  aggregate below zstd -19. Whole-file numbers are established in
  `docs/benchmarks/` (2026-08-29): Canterbury aggregate beats both zstd -19
  and xz -9e; Silesia aggregate trails zstd -19 (2.061 vs 1.997 b/B), so
  rung 1 is Silesia's to close. (2) win or tie every file vs zstd -19;
  (3) aggregate below xz -9e; (4) hold all of it as the corpus grows
  adversarially.
- **TRUST** — zero known round-trip violations and zero decoder
  panics/overallocations, ever; adversarial suite green; cumulative clean
  fuzzing hours growing week over week once M4 lands.
- **SPEED** — tracked, not yet optimized: report encode/decode MB/s on the
  finals each benchmark run; floor of ≥1 MB/s decode single-thread until M5
  makes speed a first-class target.
- **USERS** — evidence real humans use and like it: GitHub stars/forks/
  watchers and their trend, external (non-agent, non-operator) issue and PR
  authors, crates.io downloads once published, and mentions found in the
  wild — Hacker News, lobste.rs, reddit, blogs — queried read-only as
  success proxies. Report weekly. These are outcomes to earn, never to
  manufacture — the system never posts on those platforms, and gaming the
  numbers in any form is a HONESTY incident.
- **SIMPLICITY** — total `src/` SLOC and public API surface, reported with
  weekly delta; growth must be justified by wins elsewhere on this
  scorecard. Dependency count stays zero (ADR-0002). Deletions are
  celebrated in the digest.

Process metrics (the team — BDFL's machinery gauge):

- **FLOW** — ≥1 merged PR and ≥1 recorded experiment (accept or reject) per
  week; median PR open→merge under 7 days; no PR red or stalled >14 days.
- **HEALTH** — <20% of agent sessions in a week wasted (failed, stalled, or
  produced no artifact); pause downtime reported; new issues triaged <48 h.
- **HONESTY** — every published number names corpus+version; sealed-set
  discipline unbroken (no experiment tuned against validation or finals).
  Any violation is an incident: journal entry plus process fix, same week.

## Milestones

Ordered milestones. The daily heartbeat picks the top unblocked item and ships
the smallest useful slice of it. Items marked `blocked-on-human` need the
operator. Research-flavored items defer to `research/JOURNAL.md` leads for
their ordering.

**Product shape** (operator directive, 2026-08-22): mothergod ships in
the zstd/xz/gzip genre. That means one CLI that both compresses and
decompresses, and a first-class library crate; both are table stakes,
not stretch goals. Beyond that shape, innovation is open: anything
goes as long as it is useful to humans.

## M0 — Scaffolding ✅

Crate skeleton, v0 frame format (Stored), quality-gate CI, governance and
agent processes. Done 2026-08-20.

## M1 — Port the founding-session codec ✅

- [x] Founding artifacts imported to `research/imports/session-1/` and the
      codec import-verified lossless (2026-08-20).
- [x] Port `research/imports/session-1/mothergod.rs` into `src/` as
      reviewable modules (filters, parse, models, coder) behind the frame
      format, one PR per module, tests per module, invariants written down
      (JOURNAL S1-A*). The archive file stays untouched; the port must meet
      the crate's rules the archive predates: decoder never panics on
      adversarial input (the archive uses assert/unwrap), docs, strict lints.
- [x] Python harness verified (reproduces the it31 champion's sealed
      validation exactly), then moved to git history (commit `1a3b1c8`) to
      keep the tree single-language (ADR-0006).

## M2 — Honest benchmarking (JOURNAL S1-D2)

- [x] `bench/` harness, in Rust (ADR-0006): `bench/corpus.toml` manifest
      pinning Silesia + Canterbury by URL + SHA-256 (fetch-and-cache, never
      committed), deterministic in-repo generators (entropy ladder,
      markov-H8/2, structured classes), three-tier train/sealed/finals split
      per `research/corpus/POLICY.md`.
- [x] Adversarial decode seed corpus + suite (`tests/adversarial/`,
      `docs/TESTING.md` layer 2).
- [x] Ideal-cost accounting mode in the Rust models (sum −log₂(p) without
      emitting bits) — recovers the archive's proxy-speed experiment loop
      inside the codec of record.
- [x] CI benchmark gate: PR fails on regression vs `bench/baseline.json`
      (required `ratio` check, docs/TESTING.md layer 7).
- [x] Report: bits/byte vs gzip/zstd/xz, per-dataset, in `docs/benchmarks/`
      (`canterbury.md`, `silesia.md`). By hand, not nightly/weekly yet —
      no scheduled job regenerates either file on a cadence.

## M3 — Close the gaps (research program)

Work the journal's standing leads in order: SSE (S1-P1), btultra2-class parse
(S1-P2), PPM escape (S1-P3), large windows (S1-P4), per-column modeling
(S1-P5). Target: beat zstd -19 per-file on all of Silesia/Canterbury with
real bitstreams; then xz -9e.

## M4 — Production hardening

- [ ] cargo-fuzz targets and cargo-mutants in scheduled CI
      (`docs/TESTING.md` layers 3–4); surviving mutants become issues.
- [ ] Cross-platform determinism CI + golden frames per `FORMAT_VERSION`
      (layer 5).
- [ ] Streaming/block API, bounded-memory decode guarantees.
- [ ] Frozen format spec v1 (`docs/format/SPEC.md`) + `FORMAT_VERSION` 1.

## M5 — Speed tiers

Bit-decomposed fast models, tANS fast path (level -1 mode), explicit SIMD
blend, measured multi-core scaling (S1-P6).

## M6 — Release 0.1

- [x] CLI binary (`mothergod` compress/decompress), first slice: stdin-in,
      stdout-out subcommands (`src/bin/mothergod.rs`), zero dependencies.
      File arguments and the `.mgdc` suffix landed in #359; remaining scope
      is streaming I/O, shared with M4's streaming/block API item.
- [x] Library surface reviewed for 0.1 (#360): internals `#[doc(hidden)]`,
      documented API is `MAGIC`, `FORMAT_VERSION`, `Method`, `Error`,
      `compress`, `decompress`, `filters`, with a crate-root doctest.
- [ ] GitHub release with binaries, agent-drafted changelog.
- [ ] `blocked-on-human` crates.io publish (operator holds the token).
