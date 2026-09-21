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

## 2026-09-21 — Editorial: a screenshot is evidence only if it names how it was served

Took #636, the operator's charter of 2026-09-20: "when herald makes changes
to the site, i want to see screenshots of old and new or videos." Designed
the mechanism, verified it on the runner, filed the build as #658 because
every artifact it needs is agent-system. The plumbing is in #658. What
belongs here is what verifying it taught me about the evidence this journal
has been citing.

**First, the numbers, because they kill the charter's own objection.**
Headless Chrome is already on `ubuntu-latest`; there is nothing to install.
A complete before/after pair at 375x812 and 1440x900 costs 4.6 seconds,
measured with `time` this wake. The charter weighed "a full headless browser
is heavy per-run cost" against shipping stills first. It is not heavy. It is
four and a half seconds, and it was sitting on the image the whole time.

**The finding: two ways to render a local page, and one of them lies.** My
first capture came back with the logo as its alt text. Under `file://`, all
sixteen root-relative `href`/`src` references across the three pages resolve
against the filesystem root, so the logo is simply absent; `status.html` and
`agents.html` are worse still, because both `fetch('/…json')` with absolute
paths and under `file://` render nothing but their own error states. Served
over HTTP from `site/`, the deployed root, everything renders.

**So the record here is thinner than it reads.** Two 2026-09-20 entries
("the Speed numbers stopped wearing prose" and "the verdict now sits above
the argument") each cite a "local Chromium headless screenshot, not
committed" as the evidence for a layout claim, and neither says how the page
was served. I cannot now tell whether either render showed the page a
visitor sees. The defect is not that they were wrong, and I have no reason
to think they were. It is that the record cannot say, which leaves them
assertions wearing the costume of measurement. That is the same failure I
enforce against on the public surface, committed privately in my own
institutional memory.

**Standing rule, adopted: a render cited as evidence names how it was
served.** "Served from `site/` over HTTP at 375x812" is a measurement.
"Rendered locally" is a sentence. The distinction costs four words and is
the difference between the two outcomes above.

**Why no `web-ui` checklist item shipped with this.** The obvious move is to
write the bar into the skill's completion step now and let the scripts
follow. I did not, because a checklist item demanding a mechanism that does
not exist is prose pretending to be a gate, which is precisely the defect
#636 opened against the current checklist. The bar rides in #658's PR
alongside the thing that makes it meetable, or it ships as more of what it
was filed to replace.

**One defect found by looking rather than by reading.** At 375x812 the
verdict strip from #630 wraps two-up-one-down, and the "Wins" caption breaks
across three ragged lines while "Loses" takes two. Nothing is broken and I
did not fix it here, one idea per PR. Recorded as a standing lead because of
how it was found: it is the first defect on this surface that no amount of
reading the source would have surfaced, which is the whole argument for the
mechanism the operator asked for.

**The territory contradiction, raised on #636 rather than resolved here.**
The charter says the design is mine, and the triage on it says the herald
ships the mechanism and folds the bar into `web-ui`. The herald prompt says
the opposite in as many words: not `agents/` or `.github/`, propose
machinery changes as agent-system issues instead. All three artifacts are
agent-system. Design was mine and is done; deciding my own envelope is
ADR-0008's call, not mine, so it goes to the BDFL with both exits named.

## 2026-09-21 — Editorial: the last source-identifier on the front page

Closed the remaining slice of #411's "zero internal vocabulary" half.
The curator's last two grooms on that issue both named the same live
defect: `site/index.html`'s status box showed a first-time visitor the
literal Rust constant name, `<code>FORMAT_VERSION</code> 4`, inside
"The container format (`FORMAT_VERSION` 4) carries `Stored` and
`Lz`...". A source-code identifier is not a fact a stranger needs; the
fact is "the on-disk format is versioned, frozen, and this build is on
version 4." Replaced the tag with plain prose: "The container format
(version 4) carries...". Nothing else in the sentence changed: the
frozen-format guarantee, the link to `docs/format/SPEC.md`, and the
"do not use this for data you care about yet" caveat all stay exactly
as they read before.

**Scope: `site/index.html` only, not `README.md`.** `README.md:55`
carries the identical `` `FORMAT_VERSION` `` restatement, plus journal
entry ids (`S2-D2`/`S2-D3`) in the same sentence, which is the same
class of internal vocabulary #411's original filing named. Left alone
here because the curator's grooms on #411 pointed at the site line
specifically, not the README, and because the two surfaces already
serve different readers by construction: `mothergod.dev` is the
polished landing page for a stranger with no context, while the GitHub
README is read by someone already looking at source control, a
self-selecting more technical audience where a Rust constant name and
a journal entry id cost less. That distinction is a judgment call, not
yet written down anywhere else; recording it here so the next slice
does not have to re-derive it, and flagging it as debatable: if a
future survey finds visitors arriving at the README first rather than
the site, this split stops making sense and the two surfaces should
match.

**The guard moved with the claim, matching the pattern from
2026-09-20's Speed-table entry.** `tests/claims.rs`'s
`format_version_is_current_everywhere_it_is_restated` anchored the
site half of its check on the literal substring
`<code>FORMAT_VERSION</code>`, now absent from the page it was reading.
Re-anchored it on `container format (version`, the phrase that
replaced it, unique in the file and still adjacent to the digit the
test extracts and compares against `mothergod::FORMAT_VERSION`. The
guard still fails naming both values if a future codec change bumps
the constant and nobody updates the sentence; only the anchor string
changed, not what the test verifies.

Not touched: #411's other open half, "one page per audience" (the
broader site restructure), and the remainder of #443 (ADR rendering).
Both are larger design work than this slice; #411 stays open as the
tracking umbrella per the curator's last groom.

## 2026-09-20 — Editorial: the Speed numbers stopped wearing prose

Shipped another slice of issue #604: item 2, "prose where structure
belongs." The Speed section's four rate figures sat embedded inside a
sentence ("mothergod encoded Canterbury at 0.133 MB/s and decoded it at
4.422 MB/s; it encoded Silesia at 0.059 MB/s and decoded it at 1.879
MB/s"), the same shape the charter named as the defect: "the facts here
are wearing prose."

**What shipped.** A two-column table, styled identically to the Measured
table two sections up (same `.bench` class, same right-aligned monospace
cells), corpus by encode/decode MB/s. The surrounding sentences stay
prose, because they are argument, not fact: the machine caveat, the "roughly
thirty times slower" comparison, and the two disclosed uncertainties
(single-machine, aggregate-not-typical-file) are reasoning a table cannot
carry. Only the four numbers that were doing a table's job moved into one.
Rendered at 375×812 and 1440×900, no horizontal scroll needed at either
width (local Chromium headless screenshots, not committed).

**The guard moved with the claim, not around it.** `tests/claims.rs`
previously read both README.md's and site/index.html's throughput claims
with the same prose-scanning function, anchored on "encoded Canterbury
at ... MB/s ... decoded it at ... MB/s" appearing in both files verbatim.
Restructuring only the site half of that shared text would have desynced
the two without breaking either extractor silently: README still says the
old sentence, but the site sentence is gone, and a scanner still hunting
for it there would read stale cached values or panic opaquely depending on
what text happened to remain. Added `throughput_from_site`, scoped to the
table's own `id="speed"` so it cannot collide with the Measured table's
identical `<th scope="row">Canterbury</th>` markup, mirroring the existing
`aggregate_from_site` html-cell reader rather than inventing a second
parsing strategy. README's extractor is untouched: its numbers are still
prose and stay guarded the prose way.

**Not touched.** Item 3 (status.html card ranking) turned out already
substantially addressed: PRs #601, #603, #605, #609, #612, none of them
mine and none citing #604, landed 2026-09-18 through 09-19 on the
milestone tracker and the ratio panel, and between them the ratio card
now sits above the synthetic gate table with the reasoning recorded
inline, and the milestone list carries its own "ranked, not chronological"
comment. Coincidence of timing with the charter, not response to it;
checked by reading each PR's own title and body rather than assuming from
the numbering. Item 4 (a shared frame across the three pages) and the
remaining prose-heavy sections ("What this is," "Try it") are unattempted;
"Try it" is a recipe, which is the one place prose is the right shape for
a command sequence. Left for the next slice.

## 2026-09-20 — Editorial: the verdict now sits above the argument

Shipped a slice of issue #604 (the operator's charter, 2026-09-18): item 1,
"no headline". Before this change, `/`'s first screen was the logo, the
tagline, and a pre-alpha status box; the two facts a stranger actually
wants, "is it good" and "can I use it yet", sat three sections and one
scroll further down, inside running prose ("Read it honestly: mothergod
beats both `zstd -19` and `xz -9e` in aggregate on Canterbury, and loses to
both in aggregate on Silesia").

**What shipped.** A three-item verdict strip in the header, between the
tagline and the status box: "Wins" (Canterbury, vs both references),
"Loses" (Silesia, vs the same two), "Not yet" (pre-alpha, no release), each
with a one-line caption, plus a `#measured` anchor link down to the full
table and its conditions. Rendered at 375×812, all three items and the
status box's opening line clear the first screen with no scroll (evidence:
local Chromium headless screenshot, not committed). This is structure
where the charter's item 2 also asks for it: three words and a caption
beat a sentence for a reader scanning rather than reading.

**Why the strip carries no new numbers.** The obvious version prints the
bits/byte figures (1.374 vs 1.470/1.403, etc.) in the header too. I did not:
`tests/claims.rs` guards exactly one restatement of each aggregate number,
the `<th scope="row">Canterbury</th>` row in the Measured table, matched
against `docs/benchmarks/*.md`. A second restatement in the header would be
an unguarded duplicate, the failure mode issue #431 exists to prevent, and
adding a guard for a second location multiplies the maintenance surface for
a page that already has one. The strip states the qualitative verdict
instead (wins/loses/not yet), which is the same sentence already proven
true in the Measured section's own prose, not a new claim needing its own
source.

**What #604 still needs**, not attempted here because the charter is four
independent items and this is one slice: item 3 (`/status.html`'s four
same-volume cards need ranking) and item 4 (a shared frame across the three
pages) are separate, larger changes. The 2026-09-19 survey entry recorded a
"why not (yet)" heading, peer to a "why" heading, as a second concrete way
to satisfy item 1's done-bar; not done here, left as the next slice, for
the same single-source-of-truth reason that entry gave for not filing it
separately.

## 2026-09-19 — Survey: a short, self-healing pause, and one heading worth stealing

Third survey, seven days after the second, on schedule for the first time.
Frame for the numbers below: issue #560 paused every agent workflow for
about 24 hours (2026-09-14T00:14:27Z to 2026-09-15T00:23:00Z), an operator
pause with a `RESUME-AT` line that closed itself on schedule, unlike #517's
nine-day dark stretch with no resume time. This week's numbers are close to
a normal week's, not a recovery from an outage.

### (a) Audience

| Metric | 2026-09-19 | 2026-09-12 | Source |
|---|---|---|---|
| Stars | 1 | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count` |
| Forks | 0 | 0 | same call, `forks_count` |
| Watchers | 0 | 0 | same call, `subscribers_count` |
| External issue authors | 0 of 166 | 0 of 134 | `gh issue list --state all`: 95 `app/claude`, 41 `app/github-actions`, 30 `bugabinga` |
| External PR authors | 0 of 450 | 0 of 386 | `gh pr list --state all`: 411 `app/claude`, 33 `bugabinga`, 6 `app/dependabot` |
| mothergod.dev pageloads, 7d | 6 | 7 | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, site tag `7c1ab790…`, window 2026-09-12..09-19 |
| Hacker News mentions | 0 | 0 | Algolia API: query `mothergod.dev` returns `nbHits: 0`; query `mothergod` returns the same fuzzy unrelated matches as every prior week ("Motherlode", "MotherCoders") |
| lobste.rs submissions | 0 | 0 | `https://lobste.rs/domains/mothergod.dev` still 404 |
| reddit mentions | not measured | not measured | `reddit.com/search.json` still returns 403 to the runner IP |
| Web search presence | absent (results are our own repo, PRs and issues) | absent | WebSearch for `mothergod.dev lossless compressor` and for `"mothergod" bugabinga agent-built compressor Rust`; every result link is `github.com/bugabinga/mothergod` or a page under it |
| GitHub repo views, 14d | 20 | 11, stale to 2026-08-31 | issue #435 ledger, snapshot run 34749845655 (2026-09-13); fresh this time, one day after the last survey rather than stale through a pause |

Site detail for the 7-day window, same source: all 6 pageloads on `/`, none
on `/status.html` or `/agents.html`, spread over three dates (09-12: 2,
09-16: 1, 09-18: 3). Country: US 5, KR 1. Device: desktop 5, mobile 1.

**Second data point on the only positive signal this project has:** one of
the three 09-18 pageloads carries `refererHost: bing.com`. Last week's entry
called the first such referrer "not a trend and I am not going to pretend
otherwise." A second one, six days later, from the same search engine, is
still not a trend at two points, but it is no longer a single unrepeated
event either. Every other pageload in this window and all prior ones carries
an empty referrer.

Housekeeping found while reading the queue for this table: issue #522 (this
project's own ask, "the boldest claim on `/` has its evidence on a page
nobody visits") shipped in PR #595 on 2026-09-18, but the PR body referenced
the issue in prose rather than a closing keyword, so it never auto-closed.
Closed it directly on inspection rather than leaving a stale open issue for
the next queue read to trip over.

Issue #526 (the pause guard stops measurement jobs; repo traffic ages out in
14 days), filed last survey, is still open and unclaimed. This week's pause
(#560) fell entirely between two traffic-snapshot runs (2026-09-13 and the
next expected around 2026-09-20), so it did not exercise the failure mode
#526 describes; the fix remains unverified by a live pause either way.

### (b) Study: ripgrep's README

Chosen per the house instinct to study projects that argue with benchmarks
rather than adjectives, and because it is a Rust CLI tool built by one
person that reached default-tool status in its category, a distance this
project has not covered on any axis yet.

Read 2026-09-19 from `github.com/BurntSushi/ripgrep`, `README.md`:

1. **Five benchmark tables, each with its own scenario, not one aggregate.**
   Word-boundary search on the Linux kernel tree, the same search ignoring
   gitignore, a single 13GB file in memory, a pattern that triggers a
   "performance cliff," and a high-match-count query that "smooths out the
   differences between tools." Ripgrep wins every table, but the margin
   runs from 35.94x down to 1.28x depending on scenario, and the shrinking
   margin sits in the README next to the blowout numbers, in the same table
   format. This is the same instinct already adopted here (2026-09-02,
   per-file range disclosed next to the aggregate): a benchmark is credible
   in proportion to how many of its own bad days it shows.
2. **A named, one-line epistemic hedge sits directly above the first
   number:** "Please remember that a single benchmark is never enough!"
   with a link to a longer blog post for the reader who wants the full
   analysis. Same shape as our own "the reader learns the worst
   decision-relevant fact from us, in the same screen" rule (2026-09-02).
   Already adopted, confirmed by a second source rather than new.
3. **"Why should I use ripgrep?" and "Why shouldn't I use ripgrep?" are two
   headings of equal weight, back to back.** The second is not a caveat
   folded into the pitch; it is its own section, naming three concrete
   reasons to reach for something else (POSIX portability, an unlisted
   missing feature, an unspecified performance edge case), addressed to a
   reader actively deciding, not one already sold.

The principle worth stealing is (3), and it is genuinely new here, not a
confirmation of something already adopted. Our own honest content
("Pre-alpha... Do not use this for data you care about yet",
`README.md`:54) is real and true, but it is one clause inside the "Where
it stands" section, a paragraph the reader has to already be reading
closely to reach. Giving the negative case its own heading, at the same
level as the positive one, is a structural change, not a wording one: it
tells a skimming reader that the negative case is a first-class answer to
"is this for me," not a footnote to the positive one.

This maps directly onto the operator's #604 charter (2026-09-18, open,
unclaimed): "the page opens with architecture" and the reader has to work
past an argument to find the verdict. A peer-level "why not (yet)" heading
on `/` is one concrete way to satisfy that charter's "done means" bar
without inventing a new claim; the material already exists (pre-alpha, no
release, slow, loses Silesia). **Not filed as a separate issue**: #604
already owns this redesign end to end, and a second issue naming the same
page would fragment one decision into two tickets, the opposite of single
source of truth. Recorded here as guidance for whoever executes #604.

**Explicitly rejected: leading with a screenshot**, ripgrep's actual first
visual element after the intro. It sells the tool by showing highlighted
matches in a terminal, a real product to look at. A compressor's output is
bytes getting smaller; there is nothing to show a screenshot of that isn't
already a number in our table. The transferable part of "show, don't just
assert" for this project is the existing `cmp FILE FILE.out` recipe
(2026-09-03 entry): a command the reader runs, not a picture they look at.

### (c) What changes because of (a) and (b)

- Closed #522, shipped and unclosed (housekeeping, not a new ask).
- The "why not (yet)" heading finding is recorded above as guidance for
  #604, not filed separately, for the single-source-of-truth reason
  stated there.
- Nothing else in (a) crossed the bar for a new issue this week: the
  numbers are flat and unremarkable, which is itself the finding after two
  weeks of pause-distorted data. #526 stays open and unclaimed; #523 and
  #524 stay closed from last week; #604, #443 and #411 remain the open
  queue for whoever picks up site work next.

Rejected again, same reason as every prior week: buying reach by posting
anywhere. Zero mentions across Hacker News, lobste.rs and reddit is a real
number and it stays one.

## 2026-09-18 — Editorial: the trust page's evidence, minus the numbers that would lie by tomorrow

Shipped issue #522. `/`'s Principles section stated four claims as bare
assertions; the evidence for all four already existed, machine-generated,
on `/status.html`, which has recorded zero pageloads in every window
measured so far (2026-08-31, 2026-09-12). Each principle now links the
artifact that backs it, inline, with at least one number a reader can go
count: the panic claim links the four fuzz targets (`decode_arbitrary`
named) and the adversarial and torture suites; the corpus claim links
`bench/corpus.toml`'s 13 pinned archives; the experiment claim links
`research/progress.jsonl`; the independent-verification claim cites
`CLAUDE.md`'s hard rules 3 and 8 by number.

**Deviation from the issue as filed, and why.** #522 asked the panic
claim to carry the fuzz CPU-hours and crasher count from the trust ledger.
I did not put either number on `/`. Both live only in
`site/trust-data.json`, generated at deploy time from a 90-day rolling
window of GitHub Actions artifacts (`.github/scripts/trust-telemetry.py`)
with no committed source in the repository at all: nothing for
`tests/claims.rs` to diff a static claim against, and the number changes
daily regardless. The issue's own cited evidence for the crasher count,
26, is proof of the failure mode: `trust-telemetry.py`'s current source
records that every entry before 2026-09-13T06:00Z counted libFuzzer's
`slow-unit-*` files as crashers, measuring nothing, and the true count
today is 0. A number with no guard and a known history of being wrong
does not belong baked into JavaScript-free, unguarded HTML. `/` instead
tells the reader the cumulative fuzz time and crash count run live on
`/status.html` and links it, which both answers the claim and gives that
page its first reason for a click.

**Second deviation: a floor, not an exact count, for the experiment
claim.** `research/progress.jsonl` gets a line on every accepted or
rejected experiment (`CLAUDE.md` rule 6), from the researcher's weekly
runs and from codec slices alike; three entries landed on 2026-09-18
alone. An exact-equality guard comparing the site's number against the
file's current length would fail CI on the next unrelated PR that
appends a line, in a realm (`src/`, `research/`) the herald does not
own. `tests/claims.rs` asserts `true_count >= claimed_count` instead: the
site says "at least 90 entries, at least 10 rejected" against a true
92/12 as of this entry, true today and remaining true as the log grows,
and only failing if the log were ever truncated. Same principle as the
FORMAT_VERSION and ratio guards, aimed at the specific failure mode
(single source of truth, drift across duplicates) rather than copying
their exact-match mechanism onto a counter that does not hold still.

Not touched: #443 (render ADRs on the site) and the remainder of #411
(full audience-model overhaul), both larger design work than a single
slice.

## 2026-09-12 — Survey: nine dark days, and the instruments went dark with them

Second survey, twelve days after the first rather than seven. The gap is
the finding that frames everything below: issue #517 paused every agent
workflow from 2026-09-03T16:29Z to 2026-09-12T18:34Z, nine days, on a
Claude authentication failure with no RESUME-AT line. `main`'s last
commit before the gap is `3fefa98` at 2026-09-03T16:14Z, fourteen
minutes earlier. Nothing landed until today.

### (a) Audience

| Metric | 2026-09-12 | 2026-08-31 | Source |
|---|---|---|---|
| Stars | 1 | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count`; `/stargazers` still names one account, `bugabinga`, the operator |
| Forks | 0 | 0 | same call, `forks_count` |
| Watchers | 0 | 0 | same call, `subscribers_count` |
| External issue authors | 0 of 134 | 0 of 95 | `gh issue list --state all`: 95 `app/claude`, 26 `bugabinga`, 13 `app/github-actions` |
| External PR authors | 0 of 386 | 0 of 318 | `gh pr list --state all`: 347 `app/claude`, 33 `bugabinga`, 6 `app/dependabot` |
| mothergod.dev pageloads, 7d | 7 | 24 | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, site tag `7c1ab790…`, window 2026-09-05..09-12 |
| Hacker News mentions | 0 | 0 | Algolia API: query `mothergod.dev` returns `nbHits: 0`; query `mothergod` returns the same fuzzy unrelated matches as last time |
| lobste.rs submissions | 0 | 0 | `https://lobste.rs/domains/mothergod.dev` still 404, meaning no story was ever submitted from the domain |
| reddit mentions | not measured | not measured | `reddit.com/search.json` still returns 403 to the runner IP |
| Web search presence | absent | not measured | WebSearch for `mothergod.dev lossless compressor` and for `"mothergod" bugabinga agent-built compressor Rust` returns nothing belonging to this project |
| GitHub repo views, 14d | 11, stale to 2026-08-31 | 11 | issue #435 ledger; the 2026-09-06 snapshot skipped on the pause guard and wrote nothing |

Site detail for the 7-day window, same source: all 7 pageloads on `/`,
none on `/status.html` or `/agents.html`, spread over three dates
(09-05: 3, 09-09: 3, 09-11: 1). All 7 from the US. Desktop 4, mobile 3.
Browsers: Chrome 3, ChromeMobile 2, MobileSafari 1, Edge 1.

**One new signal, and it is the only positive number on this page: the
first non-empty referrer the site has ever recorded.** One of the seven
pageloads came from `bing.com`. Every pageload in every previous window
had an empty referrer host. One search click is not a trend and I am not
going to pretend otherwise, but it is the first evidence that the link
exists anywhere outside this repository.

Reading the traffic drop honestly: 24 to 7 over a week in which the
project was frozen for nine days, published nothing, and merged nothing.
The number is small enough that three pageloads either way is noise, so
this is not a measurement of the pause's cost. It is a measurement of a
project nobody has heard of, taken twice.

**Correction to the baseline row's third caveat.** That entry explained
a range discrepancy by saying `rumPageloadEventsAdaptiveGroups` selects
a coarser sampled rollup for longer ranges, and concluded the window
must stay fixed at 7 days for the series to be comparable. The fixed
window is not sufficient, because the degradation tracks data age, not
range length. The identical query over the identical
2026-08-24..08-31 window returned 24 pageloads across seven dates on
2026-08-31; re-run today it returns 10, all attributed to a single date.
I re-ran the two intervening windows as well and both came back as a
single-date lump (10 on 2026-08-25, 10 on 2026-09-03), which is the same
artifact.

**Standing rule, adopted: the analytics number is captured on the day or
it is lost.** Cloudflare is not an archive, this journal is. A survey
that slips a week does not get to backfill: the number it would recover
is a decayed rollup, not the number, and quoting it next to a fresh one
would silently compare two different measurements. So the missing
2026-09-05 row stays missing, and the three lumps above are recorded as
evidence of the decay rather than as data.

Both of this survey's instruments failed the same way this week, which
is why #526 exists: the repo-traffic snapshot skipped on the pause guard
while GitHub's traffic API forgets daily views after 14 days, and the
RUM series decays on its own clock. Nine days cost nothing recoverable;
fifteen would have deleted a week of the repo series permanently, and
nobody would have been able to tell afterwards.

### (b) Study: sqlite.org/testing.html

Chosen because trust is the one axis where this project can beat its
incumbents today. We cannot beat `xz -9e` on Silesia yet. We can show a
published falsification record, and no incumbent compressor offers one.

Read on 2026-09-12, `https://www.sqlite.org/testing.html`:

1. **It is a marketing page that never once sounds like one.** The voice
   is reference documentation throughout. The persuasion is carried
   entirely by counts and mechanisms, and the closing sentence finally
   states the goal out loud: "with the hope of inspiring confidence that
   SQLite is suitable for use in mission-critical applications."
2. **The headline number is a ratio, and ratios need no context to land.**
   "590 times as much test code" is what a reader repeats to a colleague.
   92 MSLOC is not. The absolute figure is present, directly beside it,
   for the reader who wants to check.
3. **Every number is versioned and dated in place**: "As of version
   3.42.0 (2023-05-16)". The page ages visibly rather than silently.
4. **The counts are countable.** 51445 test cases, 6754 `assert()`
   statements, 1184 `testcase()` macros. These are not summary adjectives.
   A reader who doubts the page can go and count, and the page is written
   by someone who expects that.
5. **It publishes what its own methods cannot do.** "Static analysis has
   not been helpful in finding bugs in SQLite. More bugs have been
   introduced into SQLite while trying to get it to compile without
   warnings than have been found by static analysis." Also the MC/DC
   versus fuzzing tension, and the false positives in mutation testing.
   The most-deployed database in the world hedges its own evidence.
6. **Its reach page hedges harder than its testing page.** `mostdeployed.html`
   qualifies with "likely", "probably", "our best guess", and
   "Precise numbers are difficult to obtain and exact rankings are
   impossible", for a claim that is almost certainly true. Confidence is
   spent where the evidence is countable and withheld where it is not.

The principle worth stealing is (1) plus (4): *the document that earns
trust is the one written as a reference and filled with things the reader
could go count.* That is the same instinct as last study's zstd lesson,
a benchmark is credible in proportion to how easy you make it to attack,
pointed at an axis where we have more material than the incumbents do.

**Explicitly rejected: the ratio headline, principle (2).** It is the
most quotable thing on SQLite's page and it does not transfer. Our
generator counts 4603 test SLOC against 6968 code SLOC in `src/`
(`status-data.json`, 2026-09-12), about 0.66 to 1. Printing that anywhere
near SQLite's 590 argues against us, and inventing a more flattering
denominator to fix it would be the thing this project exists not to do.
We have counts and mechanisms. We do not have their ratio, so we do not
print a ratio.

The material we do have, all of it already machine-generated and live:
`cumulative_fuzz_cpu_hours` 5.27 and `crashers_total` 26 from
`trust-data.json`; `test_functions_src` 293 and 84 recorded experiments
of which 10 rejected from `status-data.json`; four fuzz targets including
`decode_arbitrary`; an adversarial suite, a torture suite, and golden
fixtures. That is a stronger trust page than most projects can write, and
it currently renders only on `/status.html`.

### (c) What changes because of (a) and (b)

- **The trust evidence is on the one page with zero recorded pageloads.**
  Every measured pageload in the site's life has landed on `/`, and `/`
  states its four principles as bare assertions with no number behind any
  of them, including the strongest claim the project makes, that the
  decoder never panics. Filed with the exact numbers, their sources, and
  the JavaScript-free constraint that says they must be baked and guarded
  rather than fetched: issue #522, a slice of #411.
- **A shared link unfurls as a bare URL.** No `og:` or `twitter:` tags on
  any of the three pages. Sending the link by hand is this project's only
  distribution channel, and this week's Bing referrer is the first
  evidence that a link ever travelled. Filed: issue #523.
- **The README promises a triage answer "usually within a day",** and that
  sentence was false for nine of the last ten days while the surface said
  nothing. A promise the system cannot keep by construction is a claim
  outrunning its evidence in our favour, which is the defect I am supposed
  to hate most. Filed: issue #524.
- **Both audience instruments went dark during the pause.** Filed as
  `agent-system`, since the fix is a workflow and the damage is to the
  factory's instruments: issue #526.

Not filed, and the reason is a finding: **the site's own staleness
handling worked and needs nothing.** `/status.html` publishes
`merged_commits_7d`, which reads 0 today, and dates every panel from the
repository. A reader arriving during the freeze would have seen a frozen
project, correctly, without anyone writing a word about it. The generated
surface told the truth through an outage that the hand-written surface
lied through. That is the argument for generating more of the surface,
and it is worth more than any sentence I could have written about the
pause.

Rejected again this week, for the same reason as last week: buying reach
by posting anywhere. Zero mentions across Hacker News, lobste.rs and
reddit is a real number and it stays one.

## 2026-09-03 — Editorial: the surface never said what to type

For two days I polished the argument and never noticed the surface had
no verb. `README.md` and `site/index.html` answered three of the
visitor's four questions well: what is this, is it any good, can I
trust it. The fourth, **what do I do next**, was answered by
`cargo test` in the README and by nothing at all on the site.

Meanwhile `src/bin/mothergod.rs` has been a working CLI since PR #359,
with `compress`/`decompress` subcommands, stdin/stdout by default, a
`.mgdc` suffix, and a refusal to clobber or delete. `ROADMAP.md` M6
ticks it. Neither human-facing surface mentioned that it exists. The
project shipped the thing and forgot to say so.

**Why I missed it, and the transferable part.** I was auditing claims,
and a missing next step is not a false claim, so it passed every check
I was running. The prior two entries built a discipline for verifying
what the page *says* against a run; nothing in it looks for what the
page *does not say*. Truth-checking is a filter, not a survey. The
survey is the audience walk: take each audience's four questions in
order and find where the page stops answering.

**The `cmp` line is the editorial decision worth keeping.** The recipe
does not end at `decompress`; it ends at
`cmp FILE FILE.out`, which prints nothing when the round trip was
lossless. That costs two lines and hands the reader the falsification
tool for our single most important claim, in the same screen as the
claim. It is the corpus rule turned into a command: we say lossless is
sacred, so the recipe's last step is the reader checking it on their
own data, on their own machine, without trusting us. Same instinct as
publishing the losing Silesia row, applied to an axis where the reader
can do the measuring.

Two things deliberately not done:

- **No `cargo install`, no download link.** There is no release, no
  tag, no package: `gh release list` is empty and `git tag` prints
  nothing. So the section opens by saying so, in those words. A reader
  who expects a binary and finds a build is annoyed; a reader told
  there is no binary and handed a three-line build is informed.
- **No compression-time number in the recipe.** "Expect the encode to
  take a while, at the rates above" points at the measured figures
  rather than inventing a new one for an unnamed file on an unknown
  machine.

Guarded: `tests/claims.rs` now parses the subcommands out of the
published command lines and checks each against the
`<compress|decompress>` alternation that `mothergod --help` prints, so
renaming a subcommand fails CI instead of leaving a command a stranger
cannot run. `tests/cli.rs` already proves the binary behaves; what was
unguarded is that the page names what it answers to. Four of the five
restated fact classes on the surface now carry a guard, with the
measurement date still open as issue #469.

## 2026-09-02 — Editorial: publishing the number that makes us look worst

The 2026-09-01 entry withheld encode and decode throughput from the
surface for one stated reason: the reports carried MB/s columns without
naming the machine, and a speed number without its hardware is the same
defect as a ratio without its corpus. I filed that against the generator
as issue #432. It closed the same day; both reports now carry the CPU
model, the core count, and the threading model directly above the table.
The condition I named is gone, so the number ships.

**The numbers, and why they are the point.** Canterbury: 0.133 MB/s
encode, 4.422 MB/s decode. Silesia: 0.059 MB/s encode, 1.879 MB/s
decode. Silesia's 212 MB therefore takes about an hour to compress. The
surface previously said "speed is recorded but not yet worked on" and
linked the reports. That sentence is true and it is not the same
communication: a reader parses it as a modest caveat, and the fact
behind it is roughly an hour of CPU for one corpus. **A caveat that
understates its own magnitude is a claim, and it was outrunning its
evidence in our favour.** That is the defect I am supposed to hate most,
and it was mine.

This is the zstd study's principle 3 (2026-08-31) applied to the axis I
had only applied it to for ratio. Printing the Silesia ratio loss was
easy: we win the other row. Printing the speed has no offsetting row at
all, which is exactly why it buys more trust than it costs. An evaluating
engineer discovers this number ten minutes after cloning; the only
question our page controlled was whether they learned it from us or from
their own terminal, and learning it from us is the version where the rest
of the page stays credible.

**Editorial rule, adopted: the reader learns the worst decision-relevant
fact from us, on the page, in the same screen as the claim it qualifies.**
Not behind a link, not as an adjective. A link is where the reader goes
for detail they already know exists.

Two smaller decisions worth keeping:

- **Verbatim over rounded.** The reports print three decimals and the
  surfaces quote them character for character, even though a single run
  on a shared CI runner does not justify four significant figures. The
  caveat sentence carries the uncertainty ("single-run figures from one
  machine, not a cross-machine claim") instead of the rounding doing it
  silently. Quoting the source exactly also lets a guard compare strings.
- **The aggregate is disclosed as an aggregate.** Total bytes over total
  time is not a typical file: per-file decode rates in the reports run
  from 0.3 to 39 MB/s, and Canterbury's headline 4.422 is substantially
  carried by one highly compressible spreadsheet. The range ships next to
  the aggregate so nobody plans against the wrong number.

Guarded, not just written: `tests/claims.rs` now compares all four
restated rates against the generated reports, so the next regeneration
that moves them fails CI instead of leaving a stale number on the page.
That test is the mechanism this surface keeps earning; it now covers
three of the four restated fact classes, with the measurement date still
open as issue #469.

## 2026-09-02 — Editorial: the first screen answers "what", not "who"

Shipped issue #416. Both first screens led with who builds mothergod:
the site tagline was "General-purpose lossless compression, developed in
public by an autonomous AI development team", and the README opened on
"mothergod is two experiments in one repository" with the compressor as
item 1 of 2. New order on both: category and axis, then the evidence
state, then the team.

**Why the "two experiments" list was the wrong shape, and this is the
transferable part.** It is a true and tidy description of the project,
and it answers a question nobody arriving asks. A visitor's first
question is "what is this", and a list of two things is a refusal to
answer it: the reader has to hold both halves and decide which one the
project actually is. Worse, the framing puts the compressor and the
agent team on the same footing, so the agent team reads as half the
value proposition rather than as how the compressor gets built. It
served the project's own sense of itself, which is the standing hazard
for a surface written from inside.

The team story is not cut. It gained a heading, "Who builds it", which
makes it more findable than it was as item 2 of a list, and it now sits
where a reader who has already seen the numbers can be interested in
who produced them. Demotion in order, promotion in structure.

**Rejected: putting the win into the tagline.** The strongest available
line is that mothergod beats `zstd -19` and `xz -9e` on Canterbury, and
it fits. It does not ship, because the loss on Silesia does not fit next
to it, and a tagline that carries only the win makes the reader discover
the loss three sections later, after having been sold. The tagline names
the yardstick (`zstd -19`, `xz -9e`) and the table renders the verdict,
both rows. Naming the yardstick is the zstd principle from the
2026-08-31 study; claiming the verdict before showing it is the part
that would have been advertising.

**Finding: an issue body is a document, so the truth rule binds it too.**
#416's constraint paragraph instructed me to sell the provable property
because "there is no Silesia or Canterbury number for the Rust build".
That was already false when I opened this run: PR #433 published those
numbers on 2026-09-01, the day after #416 was filed. I caught it only
because of the standing rule adopted in yesterday's entry, which sent me
to `baseline_gate check` before writing any claim. Extending that rule:
**an issue's stated constraints get verified against a run before they
are executed, not just its ask.** A stale constraint is more dangerous
than a stale claim, because executing it writes a fresh stale claim by
instruction.

Also fixed on both surfaces: the numbers were dated "Measured
2026-08-30" while the reports that produce them say "As of
2026-09-01" (regenerated by PR #436, commit `2f86863`). The values are
unchanged; only the date drifted. `tests/claims.rs`, the guard built for
exactly this class after issue #431, compares the restated ratios and
`FORMAT_VERSION` against their sources but not the restated measurement
date, so nothing caught it. Filed as issue #469: a guard that covers
three of the four restated facts teaches everyone that the fourth is
covered.

## 2026-09-01 — Editorial: the ratio arrived a day before I said it would

Yesterday's survey concluded "when you have no ratio, sell the property
you can prove, and name the date the ratio arrives." That was wrong on
the premise, not the principle. **The ratio already existed.**
`docs/benchmarks/canterbury.md` and `silesia.md` were both generated
2026-08-30, hold whole-file numbers for this Rust build
against pinned `gzip -9`, `zstd -19` and `xz -9e`, and `ROADMAP.md`'s
scorecard already quoted them. Three human-facing surfaces still said
no such number existed: `README.md`, `site/index.html`, and
`site/status.html`'s baseline caption.

How I missed it: I read the site and the README as the statement of
where the project stands, and checked them against each other rather
than against a run. Both agreed, and both were stale, which reads
exactly like consensus. CLAUDE.md's truth value names this failure
mode in advance: a document records what was true when someone wrote
it, and when a document and a run disagree, the run wins. The run here
was `baseline_gate check`, which prints `finals reports fresh` because
the required `ratio` job fails when a committed report's
`baseline-fingerprint` no longer matches `bench/baseline.json` (issue
#327). That one command is the freshness oracle for every number I
publish, and it takes twelve seconds.

**Standing rule, adopted: before writing any claim about where the
project stands, run `baseline_gate check` and read
`docs/benchmarks/*.md`, never the previous version of my own copy.**
The surface is downstream of the evidence, and reading it as evidence
is a loop that preserves whatever was last true.

The structural cause is not my reading, though; it is that the status
paragraph is hand-copied into three places while `format_version` and
the benchmark rows already exist as generated data in
`site/status-data.json`. Every codec advance silently falsifies all
three copies, and this is the second recorded occurrence: commit
`878a3a8` (PR #243, 2026-08-25) fixed the same paragraph in the same
two files when `Method::Lz` had landed and the copy still said
`Stored` was the only method.
Twice is the threshold at which a recurring correction should stop
being a decision. Filed as a guard rather than a generator, because
`site/index.html` is JavaScript-free and its central claim should not
start depending on a fetch: issue #431.

Not published today, and the reason is itself a finding: **encode and
decode throughput.** Both reports carry MB/s columns and neither names
the machine they ran on. A speed number without its hardware is the
same defect as a ratio without its corpus, so the surface says speed is
recorded but not yet worked on, and links the reports instead of
quoting them. Filed against the generator: issue #432.

What shipped, applying yesterday's zstd study directly:

- **Conditions inline** (study principle 2): corpus, file count, total
  bytes, pinning method, measurement date, and the three reference
  compressors' exact versions sit in the sentence above the table, not
  behind a link.
- **The losing row is printed** (principle 3), and named in prose so a
  scanner cannot miss it: Canterbury wins against both references,
  Silesia loses to both, and per file it is 5 of 11 and 1 of 12 against
  whichever reference is stronger on that file. Publishing the loss is
  what makes the win readable as measurement.
- **Bold marks our column only** (principle 4), on both the site table
  and the README table, including on the row we lose.

Rejected: quoting the founding Python prototype's `zstd -19` win
anywhere on the surface. It was the strongest-sounding sentence we had
and it is now the weakest: the artifact is not in the tree, its number
is not reproducible from this repository, and it sits directly above a
table showing this build losing Silesia to that same reference. A
claim a reader cannot check, contradicted by one they can, costs more
trust than it buys attention.

Also corrected on `site/index.html`: "bitstream format unstable" is
false since ADR-0041 froze `FORMAT_VERSION` 3. What is pre-alpha is the
software, not the format, and the distinction is worth the words: the
project's rules forbid ever dropping decode support for a version 2 or
3 frame, so a frame written today stays readable. That is a real answer
to the visitor's "can I trust it", and it was being thrown away by a
sentence that overstated our own instability.

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
