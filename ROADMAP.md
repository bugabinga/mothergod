# Roadmap

Where mothergod is going, ranked, and how it is judged. The mission this
serves is [`MISSION.md`](MISSION.md), the operator's to amend and nobody
else's (ADR-0011). Deliverable state is in the GitHub Milestones each
section links (ADR-0047). A measured number lives where it is generated
and never here (ADR-0048); each metric below names that home.

## Scorecard

Outcome metrics (the product):

- **RATIO**: aggregate bits/byte on the held-out finals (whole-file
  Silesia + Canterbury, real bitstreams) vs pinned `gzip -9`, `zstd -19`,
  `xz -9e`. Success ladder: (1) aggregate below zstd -19 on each corpus,
  the founding v0.6 standing; (2) win or tie every file vs zstd -19;
  (3) aggregate below xz -9e; (4) hold all of it as the corpus grows
  adversarially. Published: per file in `docs/benchmarks/`; the aggregate
  gap to the field, and when it last moved, on
  [/status](https://mothergod.dev/status).
- **TRUST**: zero known round-trip violations and zero decoder
  panics/overallocations, ever; adversarial suite green; cumulative clean
  fuzz CPU-hours growing week over week, whole-crate mutation score, and
  region coverage, each with trend (ADR-0043). Published: the trust ledger
  on /status, written by the test workflows themselves (#449).
- **SPEED**: tracked, not yet optimized: encode/decode MB/s on the finals
  each benchmark run; floor of ≥1 MB/s decode single-thread until M5 makes
  speed a first-class target. Published: the throughput columns of
  `docs/benchmarks/`, one machine, indicative only.
- **USERS**: evidence real humans use and like it: GitHub stars, forks and
  watchers with trend, external (non-agent, non-operator) issue and PR
  authors, crates.io downloads once published, and mentions found in the
  wild (Hacker News, lobste.rs, reddit, blogs), queried read-only as
  success proxies. Outcomes to earn, never to manufacture: the system never
  posts on those platforms, and gaming the numbers in any form is a HONESTY
  incident. Published: `marketing/JOURNAL.md`, weekly, from the traffic
  ledger (#435).
- **SIMPLICITY**: total `src/` SLOC and public API surface, with weekly
  delta; growth must be justified by wins elsewhere on this scorecard.
  Dependency count stays zero (ADR-0002). Deletions are celebrated in the
  digest. Published: line counts and the public API item count, each
  with trend, on /status.
- **FIT**: the share of the genre's behaviors the CLI and the library
  hold (ADR-0064): one row per behavior gzip, bzip2, xz, zstd, brotli
  and lz4 share (CLI) or flate2, zstd, xz2, brotli and lz4_flex share
  (library), each a test recording `holds` or `gap`, ratcheted both
  ways; a declined row stays in the denominator with its reason. A row
  holds only where it holds on every runtime lane that can run the
  suite. Published: holds over total with trend on /status, from a
  committed count the gate keeps honest, like the API surface.
  Unmeasured until M8's first slice lands (#981, #982).

Process metrics (the team, the BDFL's machinery gauge):

- **FLOW**: ≥1 merged PR and ≥1 recorded experiment (accept or reject) per
  week; median PR open to merge under 7 days; no PR red or stalled over
  14 days. Published: merged commits and experiment count on /status; PR
  ages in the weekly survey digest on the ops log.
- **HEALTH**: <20% of agent sessions in a week wasted (failed, stalled, or
  produced no artifact); pause downtime reported; new issues triaged within
  48 h. Published: per-run outcomes and cost on
  [/agents](https://mothergod.dev/agents); the wasted share in the
  weekly survey digest.
- **HONESTY**: every published number names corpus and version; sealed-set
  discipline unbroken (no experiment tuned against validation or finals).
  Any violation is an incident: journal entry plus process fix, same week.
  Published: incident entries in `research/JOURNAL.md`; the survey reports
  their count, target zero.

## Milestones

Two kinds of entry live here, and conflating them is how the status page came
to call this project's most-worked area "pending" (operator report,
2026-09-18).

A **milestone** is a deliverable with a definition of done. Its items are
issues in a GitHub Milestone of the same name, linked from its section
below, and an item is done when its issue closes (ADR-0047): M6, M7 and M8
today. A **program** is continuous work with a target but no finish line,
so it has no tracker milestone and nothing to count: M3 (ratio) and M5
(speed). Milestones delivered before the tracker held items (M0, M1, M2,
M4) keep one paragraph each naming what shipped, and no retroactive
issues. The numbering is creation order, not a dependency chain, and a
program gates nothing; M7 already says this of itself. Section order is
rank, and the status page renders it top to bottom.

Release 0.1 (M6) therefore gates on its own items alone. It promises what
M0/M1/M2/M4 delivered: lossless, adversarially tested, deterministic across
platforms, a versioned format, a CLI and a library. It does not promise any
rung of the RATIO ladder, whose standing is on /status. Waiting on a program
is waiting forever, because a program has no state in which it is finished.

The daily heartbeat picks the top unblocked item and ships the smallest useful
slice of it. Items marked `blocked-on-human` need the operator.
Research-flavored items defer to `research/JOURNAL.md` leads for their
ordering.

**Product shape**: mothergod ships in the zstd/xz/gzip genre, because
that is the shape a human choosing a compressor compares against. That
means one CLI that both compresses and decompresses, and a first-class
library crate; both are table stakes, not stretch goals. Beyond that shape, innovation is open: anything
goes as long as it is useful to humans.

## M0: Scaffolding ✅

Crate skeleton, v0 frame format (Stored), quality-gate CI, governance and
agent processes. Done 2026-08-20.

## M1: Port the founding-session codec ✅

The founding artifacts imported to `research/imports/session-1/` and
import-verified lossless; the archive codec ported into `src/` as
reviewable modules (filters, parse, models, coder) behind the frame
format, under the crate's rules the archive predates (decoder never
panics, docs, strict lints, invariants written down, JOURNAL S1-A*); the
Python harness verified against the it31 champion's sealed validation,
then retired to git history (commit `1a3b1c8`, ADR-0006). Done 2026-08-28.

## M2: Honest benchmarking (JOURNAL S1-D2) ✅

The Rust `bench/` harness (ADR-0006) pinning Silesia and Canterbury by
URL and SHA-256 with the train/sealed/finals split of
`research/corpus/POLICY.md`; the adversarial decode seed corpus and suite
(`tests/adversarial/`, `docs/TESTING.md` layer 2); ideal-cost accounting
inside the codec of record; the required `ratio` check against
`bench/baseline.json` (layer 7); and the per-file reports in
`docs/benchmarks/`, which no scheduled job regenerates yet. Done
2026-08-28.

## M3: Close the gaps (research program)

A program, not a milestone: no checklist, because there is no state in which
beating the field is finished. Progress reads off the Scorecard's RATIO ladder
and `research/progress.jsonl`, never a checkbox.

Work the journal's standing leads. Resolved: SSE (S1-P1), per-column
modeling (S1-P5), more experts via a logit-domain mixer (S1-P8, JOURNAL
S2-A102, ADR-0052, `FORMAT_VERSION` 5). Exhausted under every mechanism
tried so far, pending a genuinely new one, not a variant (JOURNAL S2-L1):
btultra2-class parse (S1-P2), PPM escape (S1-P3), large windows (S1-P4 —
every pricing/gating signal since S2-R11 has failed, JOURNAL S2-R14's own
remaining-scope note; what is left is a deliberate large-file mode, M5
territory, not a parse heuristic). S2-A102's own remaining-scope note had
two threads: a rate schedule keyed on something other than a step count,
and a from-scratch GLN gating architecture. The first tried an error-EMA
schedule and REJECTED it (JOURNAL S2-R26), which conflated a context's
stable residual uncertainty with genuine drift; the fix S2-R26's own
diagnosis named was tried next and ACCEPTED (JOURNAL S2-A104: a learned
baseline instead of one shared constant), then wired into a real
bitstream (JOURNAL S2-A105, ADR-0054, `FORMAT_VERSION` 6), closing that
thread. The other thread, the from-scratch GLN gating architecture, was
promoted next and tried at its smallest testable slice: per-expert
learning-rate gating, each of the six experts' own weight stepping at a
rate read from that expert's own prediction error instead of one shared
per-key rate. REJECTED (JOURNAL S2-R33): train regressed by a hair even
though both sealed cases improved, the same improving-sealed/
regressing-train split S2-R29 and S2-R30 already hit. No standing lead
is open: the next candidate is a literature idea or a cheap wild swing
per the `compression-experiment` skill; what remains of the gating
thread is deeper than a rate tweak (real gating on which expert's
signal counts, not merely how fast one shared weight set moves) and is
not itself a standing lead until a genuinely new mechanism for it
exists. Picked next: a different stage than the literal mixer every
S2-R2x/S2-R3x slice above worked. LZMA's own length coding splits match
lengths and repeated-offset lengths into two independent tables; this
project's shared, unconditioned length model never had that split.
ACCEPTED (JOURNAL S2-A109): train mean -0.002514 b/B, 7 of 11 cases
improved (`sqlite_like_records` -0.015901 the largest move), both
sealed cases cleared the bar. Wired next (JOURNAL S2-A110, ADR-0057,
`FORMAT_VERSION` 8): `Models::length` split into independent
`length_match`/`length_rep` for real, closing that thread; the real
bitstream moved less than the ideal-cost pairing predicted (train mean
-0.000727 b/B, 5 of 11 improved), S2-A110's own diagnosis being that
the ideal-cost pass walked the raw byte stream while the real encoder
parses the filtered one. Picked next: the same literature source's
other split, on the offset model instead of the length model. LZMA
also conditions a match's distance-slot coding on a coarse bucket of
that match's own length (`len_to_pos_state`); this project's offset
model has never been conditioned on anything. ACCEPTED (JOURNAL
S2-A111): train mean -0.003526 b/B, 10 of 11 cases improved
(`x86_dense_code` -0.016795 the largest move) and the eleventh flat at
exactly 0.0, both sealed cases improved too (`access_log` -0.009795).
Wired next (JOURNAL S2-A112, ADR-0058, `FORMAT_VERSION` 9):
`Models::offset` split into four length-state-keyed models for real,
closing that thread; unlike the length split, the real bitstream tracked
the ideal-cost prediction fairly closely (train mean -0.002836 b/B,
every one of the 11 cases improved or flat, none regressed;
`x86_dense_code` -0.011200 against its own predicted -0.016795, same
direction, about two-thirds the magnitude). The same specification's
coarsest `state` axis came next: the flag symbol keyed on the previous
token's kind (literal/match/rep-last) instead of today's two-way
literal/copy-last split. REJECTED (JOURNAL S2-R34): train mean
-0.000876 b/B, carried by `sqlite_like_records` alone while 9 of the
other 10 cases and sealed `access_log` regressed, because a post-rep
position only differs from a post-match one where reps are dense.
A different stage again: LZMA models the bits below a length's or
distance's bucket instead of sending them raw. ACCEPTED (JOURNAL
S2-A114), then wired for lengths only (JOURNAL S2-A115, ADR-0061,
`FORMAT_VERSION` 10): the distance trees lost on train and stay raw.
No standing lead is open: the next candidate is a literature idea or a
cheap wild swing per the `compression-experiment` skill. Target: beat zstd -19 per-file on all of
Silesia/Canterbury with real bitstreams; then xz -9e.

## M4: Production hardening ✅

Fuzzing and mutation testing in CI (`fuzz-check` weekly, `mutants-check`
on a PR's own changed lines; `docs/TESTING.md` layers 3 and 4);
cross-platform determinism with golden frames per `FORMAT_VERSION`
(`tests/golden/`, the weekly `monster` matrix across 10 hosted lanes plus
Android; layer 5); bounded-memory streaming decode (`decompress_bounded`,
`decompress_to_writer`, `decodes_incrementally`; `Transpose` keeps a
whole-buffer decode by design, JOURNAL S2-D4, and encode still takes a
whole buffer); and the format spec made normative by ADR-0041 (its
decode-forever start moved to 1.0 by ADR-0050). Done 2026-09-01.

## M7: Trust engineering (ADR-0043)

Numbered by creation order, placed here by priority: correctness debt
compounds worse than a late release, so the heartbeat works these ahead
of M5 and M6's open items. M3 stays the journal's program, worked by the
heartbeat (ADR-0062), unaffected. Strategy in `docs/TESTING.md`;
mechanisms in the issues.

Items, in [milestone M7](https://github.com/bugabinga/mothergod/milestone/2):
the trust ledger and the status page's TRUST panel (#449), which the
others append to; fuzzing that compounds (#450); the `frame_gen`
valid-frame generator and structure-aware fuzzing (#451); proptest
shrinking properties and decode-API differential agreement (#452); the
allocation torture sweep (#453); weekly region coverage, published never
gated (#454); the monthly whole-crate mutation score (#455); the Miri lane
in monster (#456); and the test-craft skill for the agents (#457).
Exploratory, a journal slice's option and no item: Kani proof harnesses on
the coder's renormalization/update math, adopted only if it proves an
invariant fuzzing cannot, journal rules apply.

## M8: Genre fit (ADR-0064)

Numbered by creation order, placed after M7 because correctness debt
compounds, and ahead of the programs because it has a finish line: the
genre is finite. The scorecard's FIT line is what it moves.

Items, in [milestone M8](https://github.com/bugabinga/mothergod/milestone/3):
the CLI genre matrix, its conformance suite and the FIT publisher (#981);
the library idiom matrix (#982); and the gaps known before any matrix
existed, measured on 2026-10-09: a broken pipe reported as an error
(#976), no `--version` (#977), concatenated frames dropped in silence and
a whole-input encode (#978), no `Read`/`Write` adapter (#979), the genre's
flag grammar absent (#983), and no BSD lane to measure the BSD third of
"unix, bsd, win32" (#980). The matrix comes first: a gap closed before its
row exists is a FIT number nobody can reproduce. The matrix files the
rest, one issue per gap cluster.

## M5: Speed tiers

A program, not a milestone, on the same terms as M3: it is the Scorecard's
SPEED line, worked continuously.

S1-P6 is exhausted at every small-slice avenue that respects the crate's
own constraints: bit-decomposed coding shipped without reaching the SPEED
floor, tANS-as-fast-path loses on bits/byte (S2-A88/S2-A90), and explicit
SIMD is closed on the decode path permanently and elsewhere pending a
candidate with a measured win (ADR-0053, re-checked S2-A103). Reopening
it needs a genuinely new idea, not a variant. The one open thread: S1-A6's
block-parallel encode/decode is correct but its scaling is unmeasured
beyond CI's 1-core container.

The floor is enforced as a ratchet, not as 1 MB/s on Silesia. Same-CPU
Silesia decode snapshots (`docs/benchmarks/silesia.md`, CPU on each) fell
from 1.879 to 1.338 MB/s on EPYC 9V74 and from 1.331 to 1.215 on EPYC
7763, over `FORMAT_VERSION` bumps 6 through 9 plus other changes; the
runner pool's CPU differs between snapshots, so raw MB/s across them is
not a series. `baseline_gate check` fails when decode cost on the hermetic
gate cases, normalized by a same-process calibration kernel, exceeds the
committed constant by its margin (#907, `bench/src/speed.rs`). No hermetic
case maps to Silesia's 1 MB/s (its files span 0.25 to 6.6), so
what is held is "no unexplained slowdown", and the aggregate on the finals
reports is still read at each regeneration.

## M6: Release 0.1

Items, in [milestone M6](https://github.com/bugabinga/mothergod/milestone/1):
the `mothergod` CLI with file arguments and the `.mgdc` suffix (#359); the
library surface reviewed for 0.1 (#360); the release workflow that builds
binaries and drafts the notes from `CHANGELOG.md` (#445); the first GitHub
release, operator-dispatched (#537, `blocked-on-human`); and the crates.io
publish, on the operator's token (#644, `blocked-on-human`).
