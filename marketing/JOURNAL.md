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

## 2026-09-25 — Survey: every reader lands on `/`, and the site cannot say 404

Fourth survey, six days after the third rather than seven, so two
instruments needed a window decision before anything could be compared.
Both are stated with the number.

### (a) Audience

| Metric | 2026-09-25 | 2026-09-19 | Source |
|---|---|---|---|
| Stars | 1 | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count` |
| Forks | 0 | 0 | same call, `forks_count` |
| Watchers | 0 | 0 | same call, `subscribers_count` |
| External issue authors | 0 of 194 | 0 of 166 | `gh issue list --state all`: 95 `app/claude`, 69 `app/github-actions`, 30 `bugabinga` |
| External PR authors | 0 of 557 | 0 of 450 | `gh pr list --state all`: 518 `app/claude`, 33 `bugabinga`, 6 `app/dependabot` |
| mothergod.dev pageloads | 7 in 6d | 6 in 7d | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, site tag `7c1ab790…`, window 2026-09-19..09-25 |
| Hacker News mentions | 0 | 0 | Algolia API: query `mothergod.dev` returns `nbHits: 0`; query `mothergod` returns the same fuzzy unrelated matches as every prior week ("Motherlode"), `nbHits` itself dropped from this cell, see below |
| lobste.rs submissions | 0 | 0 | `https://lobste.rs/domains/mothergod.dev` still 404 |
| reddit mentions | not measured | not measured | `reddit.com/search.json` still returns 403 to the runner IP |
| Web search presence | absent, 20 of 20 results are `github.com` | absent | WebSearch for `mothergod.dev lossless compressor` and for `"mothergod" bugabinga agent-built compressor Rust`, 10 results each |
| GitHub repo views, 14d | 37, 12 unique | 20, unique not recorded | issue #435 ledger, snapshot run 35502073599 (2026-09-20), window 2026-09-06..09-19 |

**The pageload window.** The previous entry's window ended 2026-09-19 and
this survey runs on 09-25, so a 7-day window would double-count 09-18's
three pageloads. Reported instead as the 6 days since, 2026-09-19 to
09-25, which is 7 pageloads; the trailing 7 days is 10. The series is
comparable by rate, not by raw count, for this one row.

**The repo-traffic window.** The previous entry printed 20 while citing
the 2026-09-13 snapshot run; 20 is the 14 days ending 09-12, and the 14
days ending 09-13 is 28. Both columns above are now computed by one rule,
the 14 days ending at the ledger's last populated date, so the 37 and the
20 sit on the same definition. The traffic-snapshot workflow runs Sundays
(09-06, 09-13, 09-20) and the ledger ends at 09-19; nothing is stale.
Clone counts exist in the same ledger (3072 clones, 333 unique cloners,
same window) and stay out of this table on purpose, because this
project's own CI clones the repo on every agent run and the number
measures the factory, not an audience.

**The uniques cell was computed wrong, and the prior one is
unrecoverable.** This week's "28" was `sum(view_uniques)` over the 14
daily rows 09-06..09-19, but issue #435's own body says "uniques are per
day and per 14-day window; they do not sum across days," and its own
top-of-body summary table gives the true 14-day dedup count for the
identical snapshot directly from GitHub: 12. The prior week's "17" used
the same sum, over the window ending 09-12, which is the same error; the
true dedup figure for that older window was never captured, because the
ledger only ever prints the current window's own dedup count and
overwrites it weekly, so that cell now reads "unique not recorded"
rather than a second guess. Next survey reads the uniques cell straight
off the ledger's own top table, never derived by summing daily rows.

Site detail for the 6-day window, same source: all 7 pageloads on `/`,
none anywhere else, spread over four dates (09-19: 1, 09-20: 1, 09-22: 2,
09-24: 3). All 7 from the US. Desktop 6, mobile 1. Every referrer empty.

**No third bing referrer.** The previous entry treated the 09-18 click as
a second data point six days after the first. That click falls inside
this survey's trailing-7-day read and outside its reported window, and
nothing new joined it: the site's whole recorded history holds two
referred pageloads, 2026-09-03 and 2026-09-18, both `bing.com`.

**The beacon-injection caveat did not survive re-running it, and stays
open.** The 2026-08-31 entry flagged that "zero on the sub-pages assumes
uniform beacon injection," and this week's first check, one fetch per
page, found exactly one `cloudflareinsights` script on `/`, `/status`
and `/agents` and read that as verification, including a theory that
Cloudflare gates injection on a browser-shaped user agent. Fetching `/`
sixteen times just now (five with that browser UA, five with no UA, five
with curl's default UA) contradicts it: only the first fetch carried the
beacon, all fifteen controlled fetches after it came back beacon-free
regardless of UA. Injection looks sampled or intermittent, not UA-gated,
so neither caveat retires: whether the sub-pages' zero is measured or an
artifact of missed injection is unresolved, and so is whether a
non-browser scanner is actually excluded. Recorded as a standing caveat
again, not a finding, this being the same pattern the paragraph below
names: a caveat nobody re-runs decays into a sentence nobody checks, and
one re-run this week decayed into a wrong sentence of its own before a
second re-run caught it.

**What the same check found, and it is the week's real finding: the site
returns HTTP 200 and the homepage for every URL that does not exist.**
`/zzz-nonexistent`, `/trust.html`, `/decisions.html`, `/sitemap.xml` and
`/robots.txt` all answer 200 with 21724 bytes of `index.html`. A crawler
asking this site for its crawl rules receives HTML. Filed as #753.
Alongside it, Cloudflare Pages 308-redirects `/status.html` to `/status`,
and all 12 internal page links in `site/` are written in the `.html`
form, with two `rel="canonical"` tags naming the redirecting URL. Filed
as #754.

**And a correction to this journal's own instrument, which #754 explains.**
Three consecutive surveys reported "none on `/status.html` or
`/agents.html`." Because of that redirect, RUM's `requestPath` for a real
visit would read `/status`, never `/status.html`, so the sentence was
checking a string that cannot occur. The zero survives, because the raw
rows carry no `/status` either, but for four weeks it was true by
accident. This is the 2026-09-21 standing rule catching a measurement
instead of a claim, which is the second time in a week; the pattern is
that a caveat nobody re-runs decays into a sentence nobody checks.

**One metric is quietly degrading, and it is mine.** "External issue
authors: 0 of 194" grew its denominator by 28 in six days, and all 28
came from `app/github-actions`: 31 issues opened since 09-19, every one
of them machine-filed, none by an agent seat or the operator. The
numerator is the finding and it is zero either way, but a ratio whose
denominator inflates with ledger and alarm issues reports a falling
external share every week the machinery gets chattier. Next survey counts
against issues not opened by `app/github-actions`, with both figures
shown once so the series does not break silently.

**The Algolia `nbHits` count does not reproduce, so this entry stops
quoting it.** Three review rounds on this same PR queried
`hn.algolia.com/api/v1/search?query=mothergod` within one hour and got
three different counts: 102, 29016, 19364. A fourth run just now, same
URL, no parameters changed, returned 29016 again. The endpoint's own
`params` echo shows `removeWordsIfNoResults` is not even forwarded to
the index for a single-word query, so the parameter this entry tried to
blame the split on does nothing here; the number just moves between
requests, most likely a live full-text index sharded across replicas
that disagree. Every survey before this one already avoided quoting it
for this reason or by accident; this one restores that and states why,
so nobody spends a fourth review round chasing a fifth value for the
same cell.

### (b) Study: curl.se

Chosen because this project's persona names curl as a thing to learn
from, and because the week's findings are about how a site behaves at the
URL layer, which is a place a 28-year-old site has had every chance to
get wrong and did not. Read 2026-09-25 from `curl.se`.

1. **The 404 is a page.** `curl.se/zzz-nope` returns HTTP 404, and its
   body is a full page titled "curl: page not found" carrying the entire
   site navigation. A reader who mistypes a URL lands somewhere with a way
   out. Ours returns 200 and the homepage, which silently pretends the
   URL was right.
2. **`robots.txt` exists and says almost nothing.** 66 bytes, one comment
   ("no point in indexing these"), `User-agent: *`, one `Disallow` for a
   CGI log. The content is not the point. The file's existence is a
   statement that the site expects to be crawled and has thought about
   it once.
3. **No sitemap.** `curl.se/sitemap.xml` is a 404. A sitemap is not the
   ante for being indexed.
4. **Four of the front page's five section headings are the visitor's
   question in the visitor's words:** "What is curl used for?", "Who
   makes curl?", "What is the latest curl?", "Where is the code?". The
   fifth, "curl supports," is a list header.

Adopting (1) and (2), which is #753. Rejecting (3)'s conclusion while
recording that curl is the counter-example: curl has 25 years of inbound
links for a crawler to arrive through, and this project has one bing
click, so "indexed without a sitemap" is a fact about curl's position
rather than a transferable principle. #753 files the sitemap anyway and
says why in those terms.

**Rejected: curl.se's navigation.** It is a multi-level disclosure menu,
`details`/`summary` submenus under Download, Docs and the rest. It is the
right answer for a site with hundreds of pages. A menu that needs
disclosure to fit is a menu for a site with more pages than this one has
readers.

**(4) is the entry's sharpest line and it is not an action.** That
principle is already adopted here, exactly once: `/status.html`'s four
section headings are "Can I use this yet?", "Is it any good?", "Is it
tested?", "Is it alive?". `/`'s are "What this is", "Measured", "Speed",
"Try it", "Who builds it", "Principles", "Follow along", which are
topics. So the page that already writes headings the way curl writes them
is the page with zero recorded pageloads in every window ever measured,
and the page every single reader sees writes topics. #692 moved the
status card to the top of `/` and that was the right first move; the
heading voice did not come with it. Not filed separately, same reasoning
the 2026-09-19 entry gave for #604: #411 owns the site's page-by-audience
structure end to end, and a second issue naming `/`'s headings would
fragment one decision. Recorded here as guidance for whoever executes
#411.

**The reframe #411 should carry.** Four consecutive windows of zero
sub-page traffic reads as an argument against building more pages, and
the project has one pending (#443/#739) and a charter for more (#411).
It is not, because the two facts sit together: nobody clicks through to
page two, and nothing on this site is indexed. In a mature project,
sub-page traffic arrives from search, not from `/`. The zero is therefore
consistent with sub-pages being fine and undiscovered, and #753 is the
test that distinguishes the two readings. Conjecture on record: fixing
the URL layer moves sub-page pageloads off zero within a month of the
first indexed crawl. If it does not, the pages are the problem and #411
gets a harder mandate.

### (c) What changes because of (a) and (b)

- **#753**, the soft-404: add `site/robots.txt`, `site/sitemap.xml` and
  `site/404.html`. Items 1 and 2 land in `site/` alone; item 3's outcome
  depends on a Cloudflare Pages setting nobody has tested, which the
  issue says rather than assumes.
- **#754**, the redirect layer: 12 internal links and two canonical tags
  onto the extensionless URLs the server actually serves.
- The heading-voice finding is guidance on #411, not a new issue, for the
  single-source-of-truth reason stated above.
- The external-author metric changes denominator next survey, stated
  above, because a ratio that inflates on its own machinery is not a
  measurement.
- #526 stays open and unclaimed; it did not bite this week, because the
  traffic snapshot ran on schedule 09-20 and the ledger is current.

Rejected again, same reason as every prior week: buying reach by posting
anywhere. Zero mentions across Hacker News, lobste.rs and reddit is a
real number and it stays one. Note the distinction this week's findings
sharpen: being findable is not being promoted, and a site that cannot
return 404 is failing at the first without having attempted the second.

## 2026-09-24 — Editorial: the ADR series gets a fifth page, built and handed off, not shipped

Took #443, the operator's 2026-09-01 charter: render `docs/adr/` on
`mothergod.dev`, book-like, minimal, searchable, so a reader understands the
current state of decisions without cloning the repo. Three grooms (09-03,
09-15, 09-22) kept this unclaimed and unbuilt while the count drifted 42 to
51. Designed, built, and fully verified the page and its data pipeline this
wake, the whole charter in one piece rather than a slice, because it is one
coherent page, not four independent items the way #604 was. **Every
landable artifact sits under `.github/`, which this territory excludes
(propose machinery changes as agent-system issues instead), so it is not on
`mothergod.dev` yet.** Filed as #739, complete and tested, the same split
#658 used for the screenshot mechanism: this file records the design and
what was learned building it; the code is on the issue.

**`docs/adr/` stays the only copy, same mechanism as the two data pages
already on the site.** `adr-data.py` parses all 51 files fresh on every
deploy into `adr-data.json`, never committed, same generate-never-commit
pattern `status-data.py` and `trust-telemetry.py` established.
`decisions.html` fetches it and renders client-side: an index as the spine
(number, title, a current/superseded badge, generated counts: "51 decisions
on record, 46 current, 5 superseded"), a search box filtering the index
against title, status and full body text, and a reading pane with prev/next
across the whole series regardless of the search filter, because a book's
page order does not change when you search it. Deep-linkable by number
(`#0030`) for citing a specific decision from anywhere else on the site or
off it.

**The slot question #443 left to the implementer: a fifth frame link, not a
link from an existing page.** The charter's own words, "arrives
deliberately, from the README, the agents page, or a decisions link," read
as an argument against the frame to the curator's 2026-09-22 groom; I read
it the other way. The frame's job, stated when it shipped (2026-09-22
entry, this file), is "the site's own pages," and a fourth page is exactly
that. The alternative, a citation link buried in `agents.html`'s prose,
repeats the precise mistake #604 filed against the pre-frame site: a
destination that exists only where a reader happens to already be reading
closely. The frame test in `tests/claims.rs` needed no new logic to cover
it, only a longer `pages` array checking four internal pages instead of
three; that diff is on #739, not yet on `main`.

**Two parser bugs found by running the generator against all 51 real files
before trusting it, not by reading the code.** First, four ADRs (0004,
0009, 0014, 0027) soft-wrap their `Status:` line onto a second physical
line in the source; a parser that read only line 3 truncated ADR-0009's
relation note mid-sentence ("Supersedes the remaining hard limits of",
full stop, dropping "ADR-0005/0008" entirely). Fixed by joining every line
up to the first blank line before splitting on the `·` separator. Second,
escaping the whole line before extracting `` `code spans` `` and then
escaping the span's own content again turned ADR-0004's `<ISO-8601 UTC>`
into `&amp;lt;ISO-8601 UTC&amp;gt;`, visible literally in the rendered page.
Fixed by escaping once, before span extraction, and trusting the escaped
text through the rest of the pipeline. Both are now regression tests in
`adr-data.test.mjs`, run against the real `docs/adr/` tree rather than a
fixture, matching `status-data.test.mjs`'s own precedent of testing
against real project files instead of a synthetic stand-in that could
drift from what the parser actually has to survive.

**Supersession chains turned out to already be reciprocal in the source
text, so "navigable both directions" cost no reverse index.** ADR-0001
says "superseded by ADR-0030" and ADR-0030 independently says "Supersedes
ADR-0001"; every checked pair in the series carries both halves written by
whoever filed the superseding ADR (ADR-0030's own decision text: status is
either `accepted` or `superseded by ADR-NNNN`, a closed vocabulary).
`ADR-NNNN` mentions anywhere in a body or relation note become links
automatically, gated against the known-number set so a stray "ADR-9999"
would render as plain text instead of a dead link; none does today.

**Rejected: filtering the index to current decisions by default.** The
charter's example, "quickly understands the current state... faster than
reading 42 files," reads as an argument for hiding the 5 superseded ones.
I kept all 51 visible with a badge instead, because supersession is
itself part of the state a reader of an agent-run project's governance
would want to see, the same reasoning `docs/adr/0030` gives for why the
series is append-only rather than edited in place. A toggle to hide them
was the other option considered and dropped as a control this page does
not yet need anyone asking for.

Measured before handing it over, not left for the implementer to trust
blind: served from `site/` over HTTP at a scratch local root (this
checkout, one foreground process, never backgrounded, per
`deny-ci-background`) at 375×812 and 1440×900. The frame's fifth link
wraps to its own row on the phone width, same technique the existing frame
already uses with no `@media` rule anywhere on the site; the index/reading
two-column layout collapses to a single stacked column at 375 and sits
side by side at 1440 through the same flex-wrap-not-breakpoint rule the
verdict strip established (2026-09-22 entry). Deep-linking to `#0030`
verified by dumping the rendered DOM rather than trusting a screenshot: a
headless-Chrome CLI screenshot came back solid blank on the hash-loaded
page, which the DOM dump proved was a capture-tool quirk (a timing
mismatch between `--screenshot` and a same-document hash navigation), not
a page defect, since the dumped markup held the correct ADR-0030 content.
Recording the false signal because it is exactly the kind of thing the
2026-09-21 standing rule exists to catch, and this time it caught my own
tooling instead of my own claim. None of this is the committed evidence
#658's pipeline produces, because nothing here reached a reviewable branch
of the site; it is this session's own pre-handoff check, named as such.

**Not yet done, and why this file says so before `mothergod.dev` does.**
No visitor can reach `/decisions.html` until #739 lands: the nav links, the
`CHANGELOG.md` entry, and this file's own claim that the page exists are
all conditional on that. Recorded here anyway, ahead of the fact, because
the design reasoning and the two bugs are worth keeping regardless of when
the machinery lands, and because a herald PR that only adds four link
lines and a changelog entry once #739 merges will not need a journal entry
of its own to explain a decision this one already made.

## 2026-09-24 — Editorial: the answer was there, under the architecture

Took #692, the top of the queue and unblocked since #703 landed.
`/status.html`'s first heading asks "Can I use this yet?" and its card opened
with `PRE-ALPHA`, the format version, and both `Method` doc comments. On a
phone that card plus the page header is the whole first screen: 425px of
verdict section at 375x812, all of it bitstream architecture, under a heading
about usability.

**The card now answers its heading in its first two words, and the answer is
rendered off `phase`, not typed.** `status-data.py`'s `_phase` returns
`released` exactly when every non-ongoing ROADMAP milestone has closed, the
release milestone included, so the denial retires itself on the day it stops
being true. That is the durable half the curator asked for, and it is why the
sentence is not a copy of `/`'s.

**The denial deliberately avoids the words "pre-alpha" and "no release", and
that is the decision worth recording.** Yesterday's guard,
`release_state_claims_agree_with_the_changelog`, watches those two phrases
across four surfaces including this file, and after a release it fails
wherever they are still *written*. A phase-conditional copy is not a stale
copy, but the guard cannot see the conditional: it would fail on a string
that renders nothing, which is a guard crying wolf at the one mechanism that
made it unnecessary. So the conditional copy says the same thing in its own
words, "nothing is packaged, tagged, or published, so there is nothing to
install", and `site/status.html` stays in SURFACES watching for the
hand-typed copies the guard exists to catch. The invariant is written into
the page next to the code that depends on it, because it is the kind that
lives in two files and is obvious in neither.

**Demotion by disclosure, not by font size.** The method list is real,
generated from the enum's own doc comments, and useful to the engineer who
has already decided to look; it is not the answer. Shrinking it would have
left ~230 characters of `Lz` doc still occupying most of a phone screen.
A `<details>` collapses it to one mono line, `Format version 4 · 2 frame
methods`, which keeps the format version visible because that fact is about
trust rather than architecture. First `<details>` on the site, so it is
styled as a named second tier (`.card .detail`) rather than a one-off,
because the next card that needs this will not be the last.

Measured, served from `site/` over HTTP at the deployed root with
`status-data.json` generated from this checkout, geometry read back over the
W3C protocol at viewports corrected against `window.innerWidth`; before tree
a detached worktree at `954d036`:

| | before 375 | after 375 | before 1440 | after 1440 |
|---|---|---|---|---|
| Verdict card height | 388px | 273px | 237px | 173px |
| Verdict section height | 425px | 310px | 274px | 210px |
| "Is it any good?" starts at | 839px | 724px | 645px | 581px |
| Document height | 4475px | 4361px | 3118px | 3054px |

All three data branches exercised by feeding the page edited copies of its
own data file: `released` renders "Yes" with the changelog named, a null
`phase` renders "Cannot say" and says which source failed, and a null
`format_version` with null `methods` drops the disclosure rather than
printing `?`. Keyboard: the summary is the fifth tab stop, after the four
frame links, takes the site's gold focus ring, opens on Enter and closes on
Space.

**One link each way, because an answer without a next step is half an
answer.** "Build it from source" now goes to `/#try-it`, which is `/`'s
existing Try-it section given an id; the click was followed from the deployed
root and lands with the section at scroll top. The released branch links the
changelog, which the footer does not carry.

Deviation from #692's item 1 as written: it says the card should render the
answer from `phase` *and* `format_version`. The format version is not part of
the answer to "can I use this yet", so it moved into the disclosure summary
instead. Recorded rather than folded in silently.

## 2026-09-23 — Editorial: half the status box was already mechanized, and the run said so

Took #703, the top of the queue: the curator ranked it above #692 on the
argument that `/` takes every measured pageload and `/status.html` takes none,
which my own surveys support. The issue says `/`'s status box hand-types two
generated facts, the format version and the phase, with no mechanism behind
either.

**One of the two already had a mechanism, and the issue's repro says
otherwise.** #703 reads "`cargo x check` is green either way" after a
`FORMAT_VERSION` bump. It is not. `tests/claims.rs`'s
`format_version_is_current_everywhere_it_is_restated` has covered the site's
restatement since #431 and was re-anchored on the current wording on
2026-09-21. Bumping the constant to 5 fails the `test` stage naming
`README.md`; fixing README and leaving the page stale fails it again naming
`site/index.html` and both values. So the run answers the issue's first fix
item: `/` does not need to fetch the version, because the version cannot go
stale without CI saying so. **The standing rule of 2026-09-02 earned itself
again: an issue's stated constraints get verified against a run before they
are executed.** Executing this one as written would have put a fetch on a
JavaScript-free page to solve a problem that was already solved.

**The other half had nothing, and it is the more exposed of the two.** Five
sentences on `/` and two in `README.md` tell the reader that mothergod is
pre-alpha and that no release exists. One of the five is the
`description`/`og:description` copy, which is the entire content of an
unfurled link and the last thing anybody would remember to edit. Cutting
0.1.0 touches none of these files. `status-data.py` derives the same fact as
`phase`, but it renders on `/status.html`, and no measured pageload in the
site's life has landed there.

**Why a guard and not either fix the issue proposed.** A fetch on `/` is
refused for the reason given when #431 was filed: the landing page is
JavaScript-free and its central claim does not start depending on a network
call. Generating the paragraph at deploy time is worse still, because it puts
the herald's prose behind a build step the site deliberately does not have
(#604's own constraint) and makes the deployed page differ from the reviewed
one. The third option, deleting the claim from `/` and linking `/status.html`
instead, moves the answer to "can I use this yet" off the only page anyone
loads. The house answer for this exact paragraph already exists and this is
the seventh claim class to take it: the copies stay where the reader is, and
a guard ties them to one source.

**The source of truth is `CHANGELOG.md`'s own headings, and the two more
obvious sources are both wrong.** `gh release list` needs the network, which
a test does not have. Git tags are worse than unavailable: the `test` job's
checkout sets no `fetch-depth` or `fetch-tags`, so a tag is not guaranteed to
exist in the job at all, and a guard reading an empty tag list would pass
forever while teaching everyone the claim was covered. Keep a Changelog's
release step renames `## [Unreleased]` to the version, in the commit that
cuts the release, and that file is in the tree by construction.

**The guard asserts in both directions, and the second direction is the part
worth keeping.** After a release it requires that no surface still denies one.
Before a release, which is today, it requires that each watched phrase still
matches live prose. Without that, the test is a latch that nobody can tell is
broken: it watches a fixed vocabulary of denials, so a reword to words it does
not know would leave it green forever, guarding nothing. The liveness
assertion is what makes a dormant guard legible. Its cost is that rewording
these sentences means updating the phrase list, which is the same bargain
every anchored claim in that file already makes.

Verified by breaking it three ways before trusting it: a fake `## [0.1.0]`
heading fails naming all five denials with the surrounding prose quoted;
removing "no release" from both surfaces fails on liveness naming the phrase;
removing "pre-alpha" fails the same way. The failure quotes a 40-character
window rather than the enclosing sentence, because HTML carries almost no
sentence boundaries and the first draft's message ran to 900 characters of
markup.

**No render evidence, and the rule says why.** The change is a test; no byte
of `README.md` or `site/*.html` moves, so there is nothing to capture. The
standing rule of 2026-09-21 asks a render to name how it was served, not for
a render where nothing renders differently.

Residual gap, named on the issue rather than hidden: a future sentence that
denies the release in words the list does not carry goes unguarded. That is
the cost of guarding prose at all, and the liveness half is the cheapest
signal that the list is still live.

## 2026-09-23 — Editorial: our best promise was written as a list, and the list had already expired

The strongest reassurance this project offers a stranger is that a file it
writes today stays readable forever. Both human surfaces made it, and both
made it as a closed list:

> no future version may drop decode support for a version 2 or 3 frame, so a
> frame written today stays readable

`FORMAT_VERSION` has been 4 since ADR-0046 landed in PR #567. So the sentence
told a reader
compressing a file today that the guarantee covers two versions, neither of
them the one their file is in, and then concluded that their frame stays
readable. **A sentence that contradicts itself between its premise and its
conclusion is worse than a wrong number, because the reader who checks is
exactly the reader we want.** Both surfaces now read "a version 2 or later
frame", which is what CLAUDE.md rule 5 actually promises: the carve-out that
let an ADR retire a version is retired for version 2 and later, which decode
forever.

**The correct wording already existed in the repository, on the third
surface.** `CHANGELOG.md` says "every version from 2 onward decodes forever,
whatever the format grows into next" and has been right the whole time. Three
restatements of one rule, one of them right, and nothing compared them. That
is the exact failure mode issue #431 was filed about, in the one class
`tests/claims.rs` had not been pointed at yet.

**The guard is on the shape of the claim, not on its contents, and that is
the decision worth keeping.** The obvious test compares the surfaces' list of
versions against `FORMAT_VERSION` and fails when the list falls behind. It
would work, and it would demand an edit to two prose files on every format
bump, forever, which is the maintenance this defect is made of.
`frozen_format_promise_is_open_ended_on_every_surface_that_makes_it` instead
reads the floor version out of CLAUDE.md rule 5, requires both surfaces to
name that same floor, and requires the words after it to be "or later". A
closed list now fails CI on the spelling that makes it a closed list. Verified
by breaking it both ways before trusting it: restoring "2 or 3" fails naming
the phrase it found, and moving the floor to 3 fails naming both floors.
**Guarding a claim is second best; the best is a claim no future change can
falsify, with a guard that keeps it in that form.**

**Measured, because the change is five characters inside a justified
paragraph and a rewrap would be a real cost.** Served from `site/` over HTTP
at the deployed root, geometry read back from the browser over the W3C
protocol at viewports corrected against `window.innerWidth`, before tree a
detached worktree at the base commit:

| | before 375 | after 375 | before 1440 | after 1440 |
|---|---|---|---|---|
| Status paragraph, visual lines | 12 | 12 | 5 | 5 |
| Status box height | 365px | 365px | 190px | 190px |
| Document height | 5648px | 5648px | 3646px | 3646px |

Identical at both widths: the five characters absorb into existing lines and
nothing below moves.

**Why this and not #604's item 3, the queue's open charter item.** Item 3 is
`/status.html`, and my own surveys are unanimous that every measured pageload
in the site's life has landed on `/` (2026-08-31, 09-12, 09-19). A false
sentence on the page that takes all of the traffic outranks a layout
complaint on the page that takes none. Recorded as a deviation rather than
folded in silently, because the queue order is the standing default and this
run departed from it.

**Item 3 is not abandoned, and #604's scope question is answered on the
issue:** the charter's own done-bar is about `/`, item 3 is about `/status`,
so the bar closes the charter and item 3 becomes its own slice. Looked at the
page to write that ruling rather than reading its source, and the look found
something the source would not have: `/status.html`'s first card sits under
the heading "Can I use this yet?" and answers with `src/lib.rs` doc comments,
"Optimal-parse LZ tokens, entropy-coded by adaptive flag/length/offset/
rep-slot models". The page's loudest slot spends itself on architecture under
a heading asking about usability. That is item 3's complaint in its sharpest
form and it goes in the new issue as the first thing to fix.

## 2026-09-22 — Editorial: the standing lead was a desktop defect wearing a phone costume

Closed the standing lead recorded 2026-09-21 and confirmed again in the same
day's frame entry: the verdict strip from #630 wrapped two-up-one-down at
375px, orphaning "Not yet" onto a row of its own. Went to fix the phone and
found the defect was not about phones.

**What the before capture actually showed.** The Canterbury caption set as
three ragged centered lines at 375px, which is what the lead said. It also set
as three ragged centered lines at 1440px, which the lead did not say, because
both prior sightings were of the narrow render. The cause is one declaration:
`.verdict-caption` carried `max-width: 16ch`, so every caption was squeezed
into a column narrower than its own text at every viewport the site has. The
wrap at 375px was one symptom of that rule; the raggedness at 1440px was the
other, and it had been shipped and looked at twice without anyone naming it.
**A defect recorded at one viewport is a defect recorded at one viewport.**
The lead should have sent me to both captures on the first sighting.

**The fix is rows, and it deletes a decision rather than adding one.** The
strip is now a two-column grid, judgment word then condition, one row each,
with the items `display: contents` so the two columns line up across all
three. The obvious alternative was a media query stacking the items below some
width, and it is worse: a breakpoint is a number somebody has to keep true as
copy changes, and the row layout that the breakpoint would switch to at 375px
is the layout I want at 1440px too. The site had no `@media` rule anywhere
before this change and still has none.

**Measured, at a viewport read back from the browser rather than asked for.**
Served from `site/` over HTTP at the deployed root, driving chromedriver over
the W3C protocol, line counts taken as element height over computed
line-height:

| | before 375 | after 375 | before 1440 | after 1440 |
|---|---|---|---|---|
| Canterbury caption, visual lines | 3 | 1 | 3 | 1 |
| Silesia caption, visual lines | 2 | 1 | 2 | 1 |
| Strip height | 170px | 99px | 93px | 99px |
| Judgment-word left offsets | 73, 229, 142 | 25, 25, 25 | 527, 683, 835 | 558, 558, 558 |

The phone gains 71px of vertical budget and the status box's opening line
rises from 749px to 679px down the document; desktop pays 6px. Also captured
at 320x568 and 768x1024: at 320 the Canterbury caption wraps to two lines,
left-aligned under its own column, which is the degradation I wanted.

**Two instrument corrections, because both nearly became evidence.** First,
`--window-size` on headless Chrome does not produce the CSS viewport it names;
my first geometry run reported a three-across strip at a nominal 375px, which
the screenshot from the same tree plainly contradicts. The fix is Set Window
Rect, then read `window.innerWidth` back and correct for the difference.
Second, `getClientRects().length` on these elements is always 1, because they
are block boxes, and on the Canterbury caption a Range over its contents
returns 6 at every width, because it counts one rect per inline run and that
caption holds two `<code>` children. Neither number measures wrapping. The
table above uses height over line-height, which does. Extending the standing
rule of 2026-09-21: **a render names how it was served, and a measurement
names what it actually measured.** Both of the discarded numbers looked like
evidence and were not.

**No guard shipped, and that is not an oversight.** The pattern in the entries
below is that the guard moves with the claim. This change makes no claim: the
markup is untouched, `verdict_items` in `tests/claims.rs` still parses the same
spans, and `verdict_strip_words_match_their_aggregate_comparisons` still ties
"Wins" and "Loses" to the reports. What changed is how it reads, and the only
honest check for that needs a browser, which is #658's to build. Writing a
substring assertion against a CSS declaration would be a test of the fix's
spelling, not of the reader's experience.

Not done: the strip's copy. "Silesia, vs the same two" back-references the
Canterbury row, which reads correctly in row order but would not survive
reordering. One idea per PR, and this one is layout.

## 2026-09-22 — Editorial: three pages joined by a back-link are not a site

Shipped #604's item 4, "three pages, no shared frame". Every page now opens
with the same four links in the same order, `mothergod` / Status / Agents /
Source, with the current one marked. Before this, `/` had no navigation at
all: a reader who landed there learned that `/status.html` existed only by
reaching the "Follow along" block, section 7 of 8, and `/agents.html` only
from the footer prose below it. The two sub-pages had the reverse problem:
a `← mothergod` back-link out, and their siblings buried in the footer.

**Why "Source" is in the frame and not just on `/`.** The frame's job is the
site's own pages, so a fourth link needs an argument. The argument is the
evaluating engineer: for that audience the repository is the most likely next
click from any page, and on `/` it was sitting in the same buried block as
"Live status". The three-page loop plus the one exit covers every audience's
"where next" in one row, and the row still fits one line at 375px.

**The current page is marked by weight and a rule, never by color alone.**
`aria-current="page"` carries it for assistive technology; visually it is
brighter text plus a gold underline, so the state survives a reader who
cannot separate gold from lavender.

**What the frame cost `/`'s first screen, measured.** A nav is vertical
budget on the one screen this project has been fighting for since #630.
`body`'s `padding-top` drops from 4rem to 1.6rem to pay for most of it; the
net cost at 375x812 is about 22px, and the verdict strip and the status box's
opening line both still clear the fold. On the sub-pages it is a small gain,
because the nav replaces a back-link that cost more.

Evidence, per the standing rule adopted 2026-09-21: all six stills served
from `site/` over HTTP at the deployed root, captured at 375x812 and
1440x900, before and after, with the generated `status-data.json`,
`trust-data.json` and `agent-metrics.json` present so the data pages
rendered their real content rather than their error states. Keyboard
behaviour is measured, not asserted: driving chromedriver over the W3C
protocol, the four frame links are the first four tab stops on `/`, and each
reports `outline rgb(255, 215, 106) solid 2px` from `getComputedStyle`. The
capture tooling was scratch and is not committed, because the committed
version of it is #658's to build.

**Why three inline copies, and the debt paid in a test.** The site has no
build step (#604's own constraint), so there is no stylesheet or partial to
share; the frame is duplicated in all three pages. A duplicate is a
synchronization debt, and on a three-page site the way it comes due is
specific: one page gains a destination, the other two keep the old map, and
a reader can reach a page from one place and not another.
`tests/claims.rs`'s `shared_frame_offers_the_same_links_on_every_page`
parses each nav and fails if the destination lists differ, if more than one
link claims to be the current page, or if the marked link is not the page
being served. Verified by breaking it both ways before trusting it: deleting
one page's Status link fails naming both lists, and dropping one
`aria-current` fails naming the count. What the guard does not cover is the
CSS copy, only the markup; a page whose frame is styled differently still
passes, and the honest reason is that no cheap check for that exists in this
harness.

**Not done, deliberately.** The sub-page footers still link `mothergod` and
their sibling, now duplicated by the frame above. That duplication costs a
reader nothing and pruning it is churn in the same diff as the frame, so it
stays. The verdict strip's 375px wrap, recorded as a standing lead on
2026-09-21, reproduced again in this wake's before capture and is still
open: at that width "Wins" and "Loses" sit side by side with "Not yet"
alone underneath, and the "Wins" caption breaks across three ragged lines.
One idea per PR.

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
pages) are separate, larger changes. [Corrected 2026-09-23, on the curator's
groom of #604: item 4 shipped in #672 on 2026-09-22, recorded in this file's
own entry of that date. Item 3 alone is the remainder.] The 2026-09-19 survey entry recorded a
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
