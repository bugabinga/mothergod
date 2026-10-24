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
- **SPEED**: encode and decode MB/s and peak RSS on the finals, mothergod's
  row beside every level of pinned `gzip`, `zstd` and `xz` on the same
  machine (ADR-0074). Success is position on the field's frontier, read as
  two neighbours per corpus: the fastest competitor level that compresses
  at least as well, and the best-compressing level that is at least as
  fast; a win moves a neighbour. Decode holds a ratchet against unexplained
  slowdown (`baseline_gate check`, M5). Published: the frontier tables in
  `docs/benchmarks/`, one machine, indicative only, and the neighbours with
  the date they last moved on /status, both #1340's deliverable.
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
  digest. Published: line counts, and the public API item count with
  its change since the previous reading, on /status.
- **FIT**: the share of the genre's behaviors the CLI and the library
  hold (ADR-0064): one row per behavior gzip, bzip2, xz, zstd, brotli
  and lz4 share (CLI) or flate2, zstd, xz2, brotli and lz4_flex share
  (library), each a test recording `holds` or `gap`, ratcheted both
  ways; a declined row stays in the denominator with its reason. A row
  holds only where it holds on every runtime lane that can run the
  suite. Published: holds over total with trend on /status, from a
  committed count the gate keeps honest, like the API surface.
  Both halves are measured: the CLI's in `tests/fit/cli.txt` (#981), the
  library's in `tests/fit/lib.txt` (#982).

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
below, and an item is done when its issue closes (ADR-0047): M5, M6, M7
and M8 today. A **program** is continuous work with a target but no finish
line, so it has no tracker milestone and nothing to count: M3 (ratio).
Milestones delivered before the tracker held items (M0, M1, M2, M4) keep
one paragraph each naming what shipped, and no retroactive issues. The numbering is creation order, not a dependency chain, and a
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

The journal is the record (`research/JOURNAL.md`): what was tried, what it
measured and how it failed live there once, under their ids, and this
section names only the live thread, because a recital copied here drifts
(on 2026-10-18 it still read "no standing lead is open" after three filter
wirings had shipped under #1123 and #1222 in one week). Resolved, by id:
SSE (S1-P1), per-column modeling (S1-P5), the logit-domain mixer (S1-P8,
S2-A102), the learned baseline (S2-A104, S2-A105), the length split
(S2-A109, S2-A110), the offset split (S2-A111, S2-A112), the low-bit
length models (S2-A114, S2-A115), the declared reach to the offset code's
ceiling (S2-A127 to S2-A129, `FORMAT_VERSION` 17). Exhausted, each under
its own id, pending a genuinely new mechanism and not a variant:
btultra2-class parse (S1-P2), PPM escape (S1-P3), per-expert rate gating
(S2-R33), the previous-token flag state (S2-R34).

Live thread: large windows (S1-P4), the one `LEAD` header left among
S1-P1 to S1-P8. The reach to the offset code's ceiling shipped as a
per-frame trial and narrowed six of the seven Silesia losses to zstd -19
(`xml` unchanged) while flipping none (S2-A128); a reach past that
ceiling, a wider offset code, is the remainder the journal lists
unmeasured. The filter bank thread (selector kinds 4 to 6,
`FORMAT_VERSION` 14 to 16, it193 to it195) ended with the per-field
byte-order rejection (it196). Past S1-P4 the next candidate is a
literature idea or a cheap wild swing per the `compression-experiment`
skill, or a charter issue labeled research + product (ADR-0062). Target:
beat zstd -19
per-file on all of Silesia/Canterbury with real bitstreams; then xz -9e.

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

## M5: Parse engine (ADR-0074)

Numbered by creation order, done when the last closes (ADR-0047); the
last closed 2026-10-20. The Speed tiers program until
2026-10-18, when the operator's question (when to tackle speed, ratio on
large files and memory) got one answer because the three are one
mechanism: the optimal parse holds state per input byte (21 bytes, 41
before #1341; `src/lib.rs`, `BLOCK_LEN`'s doc; JOURNAL S2-A116, S2-A123).
That sets the 16 MiB block and its 370 MB; the block bounds the parallel
encode, four live being about 1.5 GB against the 1 GiB `memory-check`
pins, so two blocks run at once, S2-A125, and S2-A122's four-way 2x stays
unwired. The 1 MiB `lz::WINDOW` was #1342's hypothesis for the Silesia
losses: a blanket bump (JOURNAL S2-R7) and an adaptive window (S2-R11)
were rejected, and what shipped is a trial, a frame declaring a 1 MiB or
2 MiB reach with the wide parse kept only when strictly smaller
(`FORMAT_VERSION` 17, ADR-0075, S2-A127). Read once on the finals, 11 of
12 Silesia files shrink, 0.3% to 2.4% (`xml` unchanged), encode costs
1.24x to 1.52x, peak RSS stays under 1 GiB, and all seven losses to
zstd -19 remain (S2-A128): the window was part of the correlation, not
all of it. What remains open rides the journal's leads, a reach past the
offset code's ceiling and the wide trial beside the narrow ones on idle
cores.

Items, in [milestone M5](https://github.com/bugabinga/mothergod/milestone/4),
in order: the field measured at every level, gzip 1 to 9, zstd 1 to 22,
xz 0 to 9e, ratio with encode and decode MB/s and peak RSS on the finals,
mothergod one row per level it offers, published and tracked on /status
(#1340), and those reports retimed one file at a time (#1324);
the parse state budget and its cut (#1341); the headroom spent, threads
over blocks and trials under 1 GiB (#1322), then the window or block where
a file has long-range structure, read once on the finals (#1342).
Measurement ranks first because a win nobody can place against the field
is a number without a comparison (CLAUDE.md rule 4).

Out of M5, with its trigger written down: decode speed. S1-P6 is
exhausted at every slice the format allows (bit-decomposed coding shipped
under the floor, tANS loses on bits/byte, S2-A88 and S2-A90, SIMD closed
on decode by ADR-0053), so a faster decode is a second codec path and a
format bump, which waits for a user who asks for it on an issue they open.
Until then the decode floor is a ratchet.

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

## M7: Trust engineering (ADR-0043)

Numbered by creation order, placed here by priority: correctness debt
compounds worse than a late release, so the heartbeat works these ahead
of M6's open items. M3 stays the journal's program, worked by the
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
compounds, and ahead of M6 because its finish line is nearer: the genre
is finite. The scorecard's FIT line is what it moves.

Items, in [milestone M8](https://github.com/bugabinga/mothergod/milestone/3):
the CLI genre matrix with its conformance suite (#981) and the library
idiom matrix (#982) landed, and every gap measured before a matrix existed
(#976 to #979, #983) closed through them. What remains is the BSD lane
(#980), without which "fits BSD" is a claim no run has made. A new gap is
a row the suite flips in `tests/fit/*.txt`, then an issue; the count is on
/status.

## M6: Release 0.1

Items, in [milestone M6](https://github.com/bugabinga/mothergod/milestone/1):
the `mothergod` CLI with file arguments and the `.mgdc` suffix (#359); the
library surface reviewed for 0.1 (#360); the release workflow that builds
binaries and drafts the notes from `CHANGELOG.md` (#445); the first GitHub
release, operator-dispatched (#537, `blocked-on-human`); and the crates.io
publish, on the operator's token (#644, `blocked-on-human`).
