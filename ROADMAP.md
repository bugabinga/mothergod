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

The BDFL owns this mission; it runs on the agent clock (ADR-0035), judges the
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
  panics/overallocations, ever; adversarial suite green; and, once the
  trust ledger lands (#449), cumulative clean fuzz CPU-hours growing
  week over week, whole-crate mutation score, and region coverage,
  each reported with trend (ADR-0043).
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

Two kinds of entry live here, and conflating them is how the status page came
to call this project's most-worked area "pending" (operator report,
2026-09-18).

A **milestone** is a deliverable with a definition of done. Its items are
issues in a GitHub Milestone of the same name, linked from its section
below, and an item is done when its issue closes (ADR-0047): M6 and M7
today. A **program** is continuous work with a target but no finish line,
so it has no tracker milestone and nothing to count: M3 (ratio) and M5
(speed). Milestones delivered before the tracker held items (M0, M1, M2,
M4) keep one paragraph each naming what shipped, and no retroactive
issues. The numbering is creation order, not a dependency chain, and a
program gates nothing; M7 already says this of itself. Section order is
rank, and the status page renders it top to bottom.

Release 0.1 (M6) therefore gates on its own items alone. It promises what
M0/M1/M2/M4 delivered: lossless, adversarially tested, deterministic across
platforms, a frozen format, a CLI and a library. It does not promise the
RATIO ladder, whose first rung is open (Silesia aggregate 2.061 b/B against
zstd -19's 1.997, `docs/benchmarks/`). Waiting on a program is waiting
forever, because a program has no state in which it is finished.

The daily heartbeat picks the top unblocked item and ships the smallest useful
slice of it. Items marked `blocked-on-human` need the operator.
Research-flavored items defer to `research/JOURNAL.md` leads for their
ordering.

**Product shape** (operator directive, 2026-08-22): mothergod ships in
the zstd/xz/gzip genre. That means one CLI that both compresses and
decompresses, and a first-class library crate; both are table stakes,
not stretch goals. Beyond that shape, innovation is open: anything
goes as long as it is useful to humans.

## M0 — Scaffolding ✅

Crate skeleton, v0 frame format (Stored), quality-gate CI, governance and
agent processes. Done 2026-08-20.

## M1 — Port the founding-session codec ✅

The founding artifacts imported to `research/imports/session-1/` and
import-verified lossless; the archive codec ported into `src/` as
reviewable modules (filters, parse, models, coder) behind the frame
format, under the crate's rules the archive predates (decoder never
panics, docs, strict lints, invariants written down, JOURNAL S1-A*); the
Python harness verified against the it31 champion's sealed validation,
then retired to git history (commit `1a3b1c8`, ADR-0006). Done 2026-08-28.

## M2 — Honest benchmarking (JOURNAL S1-D2) ✅

The Rust `bench/` harness (ADR-0006) pinning Silesia and Canterbury by
URL and SHA-256 with the train/sealed/finals split of
`research/corpus/POLICY.md`; the adversarial decode seed corpus and suite
(`tests/adversarial/`, `docs/TESTING.md` layer 2); ideal-cost accounting
inside the codec of record; the required `ratio` check against
`bench/baseline.json` (layer 7); and the per-file reports in
`docs/benchmarks/`, which no scheduled job regenerates yet. Done
2026-08-28.

## M3 — Close the gaps (research program)

A program, not a milestone: no checklist, because there is no state in which
beating the field is finished. Progress reads off the Scorecard's RATIO ladder
and `research/progress.jsonl`, never a checkbox.

Work the journal's standing leads in order: SSE (S1-P1), btultra2-class parse
(S1-P2), PPM escape (S1-P3), large windows (S1-P4), per-column modeling
(S1-P5). Target: beat zstd -19 per-file on all of Silesia/Canterbury with
real bitstreams; then xz -9e.

## M4 — Production hardening ✅

Fuzzing and mutation testing in CI (`fuzz-check` weekly, `mutants-check`
on a PR's own changed lines; `docs/TESTING.md` layers 3 and 4);
cross-platform determinism with golden frames per `FORMAT_VERSION`
(`tests/golden/`, the weekly `monster` matrix across 10 hosted lanes plus
Android; layer 5); bounded-memory streaming decode (`decompress_bounded`,
`decompress_to_writer`, `decodes_incrementally`; `Transpose` keeps a
whole-buffer decode by design, JOURNAL S2-D4, and encode still takes a
whole buffer); and the format spec frozen by ADR-0041, versions 2 and
later decoding forever. Done 2026-09-01.

## M7 — Trust engineering (ADR-0043)

Numbered by creation order, placed here by priority: correctness debt
compounds worse than a late release (operator directive, 2026-09-01),
so the heartbeat works these ahead of M5 and M6's open items. M3 stays
the researcher's program, unaffected. Strategy in `docs/TESTING.md`;
mechanisms in the issues.

Items, in [milestone M7](https://github.com/bugabinga/mothergod/milestone/2):
the trust ledger and the status page's TRUST panel (#449), which the
others append to; fuzzing that compounds (#450); the `frame_gen`
valid-frame generator and structure-aware fuzzing (#451); proptest
shrinking properties and decode-API differential agreement (#452); the
allocation torture sweep (#453); weekly region coverage, published never
gated (#454); the monthly whole-crate mutation score (#455); the Miri lane
in monster (#456); and the test-craft skill for the agents (#457).
Exploratory, the researcher's option and no item: Kani proof harnesses on
the coder's renormalization/update math, adopted only if it proves an
invariant fuzzing cannot, journal rules apply.

## M5 — Speed tiers

A program, not a milestone, on the same terms as M3: it is the Scorecard's
SPEED line, worked continuously.

Bit-decomposed fast models, tANS fast path (level -1 mode), explicit SIMD
blend, measured multi-core scaling (S1-P6).

## M6 — Release 0.1

Items, in [milestone M6](https://github.com/bugabinga/mothergod/milestone/1):
the `mothergod` CLI with file arguments and the `.mgdc` suffix (#359); the
library surface reviewed for 0.1 (#360); the release workflow that builds
binaries and drafts the notes from `CHANGELOG.md` (#445); the first GitHub
release, operator-dispatched (#537, `blocked-on-human`); and the crates.io
publish, on the operator's token (#644, `blocked-on-human`).
