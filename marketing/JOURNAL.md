# Marketing journal

The herald's institutional memory (ADR-0040): how mothergod meets
humans, what was measured, what was learned, what was rejected.
Public on purpose — this project's marketing is evidence, arranged
well, and the arranging is auditable.

Entries are dated, newest first, two kinds:

- **Survey** (weekly): the USERS numbers with sources, next to last
  week's; one OSS marketing study with the principles extracted; the
  changes decided because of both, each linked to its issue.
- **Editorial**: an audience-model change, a positioning decision,
  or a rejected approach worth not retrying, with the reason.

Rules: every number names its source, every claim its evidence.
A rejected approach is recorded with the mechanism of failure, same
as research/JOURNAL.md. The audience model lives here, in one
place, and pages cite it rather than restating it.

## 2026-08-31 — Survey: the baseline row

First survey. Every number below is a first observation, so nothing
has a prior week to sit next to yet; the "prev" column starts filling
on 2026-09-07.

### (a) Audience

| Metric | 2026-08-31 | Source |
|---|---|---|
| Stars | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count`; `/stargazers` names the one account, `bugabinga`, the operator |
| Forks | 0 | same call, `forks_count` |
| Watchers | 0 | same call, `subscribers_count` |
| External issue authors | 0 of 95 issues | `gh issue list --state all`: 63 `app/claude`, 26 `bugabinga`, 6 `app/github-actions` |
| External PR authors | 0 of 318 PRs | `gh pr list --state all`: 282 `app/claude`, 33 `bugabinga`, 3 `app/dependabot` |
| mothergod.dev pageloads, 7d | 24 | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, site tag `7c1ab790…`, window 2026-08-24..2026-08-31 |
| Hacker News mentions | 0 | Algolia API: query `mothergod.dev` returns `nbHits: 0`; query `mothergod` returns only fuzzy unrelated matches ("motherlode", "MotherCoders") |
| lobste.rs submissions | 0 | `https://lobste.rs/domains/mothergod.dev` returns 404, meaning no story was ever submitted from the domain |
| reddit mentions | not measured | `reddit.com/search.json` returns 403 to the runner IP; a WebSearch for the domain and for `bugabinga mothergod` surfaced nothing |
| GitHub repo views/clones | not measurable | `/repos/…/traffic/views` returns 403 `Resource not accessible by integration` on both the claude app token and `GH_WORKFLOW_TOKEN` |

Site detail for the 7-day window, same source: all 24 pageloads on `/`,
all with an empty referrer host, none on `/status.html` or `/agents.html`.
By country: US 18, CN 2, CA 2, TH 1, KR 1. By device: desktop 19, mobile 5.

Three caveats, because the raw numbers overstate what is known.

- **24 pageloads is not 24 readers.** Every referrer is empty and no
  traffic came from Germany, where the operator is, so this is neither
  attributable to a referral nor to the operator. Scanners that execute
  JavaScript reach the beacon too. Treat it as an upper bound on humans.
- **The Cloudflare window must stay fixed at 7 days.** The same query over
  30 days returned 20 pageloads across only two populated dates, while the
  7-day query returned 24 across seven. `rumPageloadEventsAdaptiveGroups`
  selects a coarser sampled rollup for longer ranges, so two ranges are not
  comparable. A survey that varies its window is not a series.
- **Zero on the sub-pages assumes uniform beacon injection.** The site is
  `auto_install: true`, so Cloudflare injects the beacon into proxied HTML;
  that it does so identically on every page is assumed, not verified.

Issue #412 asked for analytics to be wired. It already was: the account
carries a Web Analytics site for `mothergod.dev` with `auto_install: true`,
so the setup half of that issue was already satisfied and this row is the
baseline half. `/user/tokens/verify` rejects our Cloudflare token while
`/zones` and the GraphQL analytics endpoint accept it; the token is scoped,
not invalid, and `verify` is not the way to test it.

### (b) Study: zstd's README

Chosen because it is the incumbent our evaluating engineer will compare us
to, and because its README solves the problem issue #411 opened against
ours: many audiences, one document.

What it actually does, read on 2026-08-31 from `facebook/zstd@dev`:

1. **The first sentence is a category claim, not a mission statement.**
   "a fast lossless compression algorithm, targeting real-time compression
   scenarios at zlib-level and better compression ratios." Category, axis,
   and a comparison anchor the reader already has a feel for, in one line.
   Who built it and how appears nowhere on the first screen.
2. **The benchmark table carries its conditions inline.** CPU model and
   clock, OS and kernel build, the harness (`lzbench`), the compiler
   version, and the corpus, in the sentence directly above the table. The
   reader never has to leave to find out what the numbers mean.
3. **It publishes the rows it loses.** lz4 decompresses at 3850 MB/s
   against zstd's best 2050. Printing the loss is what makes the wins
   readable as measurement rather than as advertising.
4. **Bold marks only its own rows**, so the eye is directed rather than
   decorated. Same rule as our house `**bold**` rule, applied to a table.
5. **Order is decision-support first, reference second.** What it is,
   proof, how to get it, then eleven build-system sections nobody reads
   before deciding. Even the CI badges sit below the intro, in their own
   "Development branch status" section, not in the header.

The principle worth stealing is (2) plus (3): *a benchmark is credible in
proportion to how easy you make it to attack.* That is the same instinct as
our own corpus policy, which scores additions by regret, and it is already
the house rule that a claim names its corpus.

The principle we **cannot** apply yet is the table itself. There is no
Silesia or Canterbury number for the Rust build; that is ROADMAP M2. Copying
zstd's shape today would mean printing a table we cannot fill, and
`research/corpus/POLICY.md` forbids quoting `bench/baseline.json` in its
place, because those are train and sealed numbers on synthetic generators,
not the held-out finals.

So the adaptation, and this is the editorial decision: **when you have no
ratio, sell the property you can prove, and name the date the ratio
arrives.** What is provable today, each with an artifact a stranger can
open: two fuzz targets including `decode_arbitrary`, an adversarial test
suite, golden fixtures, and 81 recorded experiments in
`research/progress.jsonl` dated 2026-08-22 to 2026-08-31, of which 10 are
rejections, each carrying its hypothesis, mechanism, corpus, and train and
validation bits-per-byte deltas. A published falsification record is
something no incumbent compressor offers, it costs us nothing to show
because we keep it anyway, and it is the honest lead while M2 is open.

### (c) What changes because of (a) and (b)

- The one page that exists is `/`, and 24 of 24 pageloads landed there.
  Sub-page work is not worth a cycle until `/` earns a second click, so
  the site overhaul in #411 starts and stays on `index.html`.
- `index.html` says the compressor was "validated on the Silesia and
  Canterbury corpora" two paragraphs below a status box saying no such
  number exists for this build. One of them is false to any reader who does
  not know that "the prototype" means a Python program that lives only in
  git history. Filed as the first slice of #411, with the exact replacement
  text: issue #415.
- Both the tagline and the README's first line lead with who builds this
  before what it does, which is (b)(1) backwards. Filed: issue #416.
- Repo traffic is unmeasurable on any token an agent holds. Filed
  `blocked-on-human`: issue #417.
- The repository's homepage field is empty, so the highest-traffic link
  slot GitHub gives us does not point at mothergod.dev. Filed: issue #418.

Rejected this week: buying reach by posting anywhere. Zero mentions across
HN, lobste.rs, and reddit is a real number and it stays a real number.
External platforms are read-only instruments for this project (ADR-0040),
and a number we manufactured would measure nothing.
