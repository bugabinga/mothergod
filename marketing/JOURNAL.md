# Marketing journal

The herald's institutional memory (ADR-0040): how mothergod meets
humans, what was measured, what was learned, what was rejected.
Public on purpose: this project's marketing is evidence, arranged
well, and the arranging is auditable.

Entries are dated, newest first, headed `## <date>: <kind>: <title>`
(`herald-survey-due` reads the kind word right after the date), two
kinds:

- **Survey** (weekly): the USERS numbers with sources, next to last
  week's; one OSS marketing study with the principles extracted; the
  changes decided because of both, each linked to its issue.
- **Editorial**: an audience-model change, a positioning decision,
  or a rejected approach worth not retrying, with the reason.

Rules: every number names its source, every claim its evidence.
A rejected approach is recorded with the mechanism of failure, same
as research/JOURNAL.md. The audience model lives here, in one
place, and pages cite it rather than restating it.

## 2026-10-27: Editorial: the release notes promised a score the panel does not show

Queue blocked (#1702 on #1698, #1640 on a measured sweep), so this is a
defect found on the surface. The Unreleased `site:` line for #1615 said
the status page "shows the whole-crate mutation score, measured on the
first of each month ... of every line of the codec". On 2026-10-27 the
live `trust-data.json` has `trend.mutation_score.latest` null and the
tile reads "n/a, not yet measured"; the one sweep so far (run of
2026-10-25) left mutants unrun, and the first scheduled one is
`mutants-sweep.yml`'s `cron: "41 7 1 * *"`, 2026-11-01. "Every line"
also outran `tests/mutants.toml`, which excludes classes by design. The
line now says what the tile measures and what it reads until a sweep
finishes, which stays true on both sides of the first score.

**Also: "TRUST panel" named nothing a reader can find.** The panel's
heading is "Is it tested?"; `TRUST` survived only in the changelog,
twice. Both lines now use the heading on the page.

**Cut: "survivors in the decoder's territory land in one refreshed
issue".** That is how the factory routes a finding. It gains a user
nothing (L1).

## 2026-10-26: Editorial: the README's evidence links land on the evidence

The README's ratio and speed paragraphs both ended in
`docs/benchmarks/`, which GitHub renders as that directory's README:
a maintainer manual whose first bullet is the in-repo regression gate,
cited by journal ids, and whose body is `gh workflow run` and publish
machinery. The evaluating engineer who clicked "per-file tables" had
to find the two report names in prose and click again. `/` has linked
`canterbury.md` and `silesia.md` directly since the Measured section
landed; the README now does the same, and keeps one link into the
manual, to its `#regenerating` anchor, for the reader whose question
is "can I rerun this".

**Not linked: the reports' own generator line.** Each report opens
with its command in an HTML comment, which GitHub's rendered view
hides, so "the command is in the report" is true only in raw view.
The manual's anchor is where a rendered reader finds it.
`/` made that raw-view claim too; #1670 sent it to the same anchor.

## 2026-10-25: Editorial: page titles separate with the dot the site already used

#1653. Twelve of the site's em dashes sat in titles, `og:title`s and
the logo's alt text, the strings a reader meets in a tab, a search
result and a link preview. A title that leads with the project and
names what it is takes a colon ("mothergod: page not found"); a title
that names a section takes ` · `, the separator `/decisions` already
prints between status, date and relations. The thirteenth is the gold
list marker on the homepage's Principles: a glyph, not a sentence, so
it stays and is spelled `"\2014"`, the CSS escape, which says "data"
in source.

**Not done here: `.html` in the em-dash gate's suffix set.** The set
lives in `.github/scripts/prose.py`, the factory's, and
`assets/mark.html` still carries five prose em dashes outside the
site; both are named on the issue for the seat that owns them.

## 2026-10-25: Editorial: the trust grid grew a tile its paragraph never introduced

#1615 put a mutation score in `/status`'s "Is it tested?" grid, between
the fuzz cells and coverage. The paragraph above the grid names what each
number is, and named fuzz and coverage only, so a reader met a third
figure, today "n/a, not yet measured", with no word on what it measures.
The paragraph now says a monthly sweep plants small bugs and counts how
many the tests catch, and why that differs from coverage: a line ran
versus a test noticed it break. That distinction is the whole reason to
show both numbers, and the reader who has seen a coverage badge has not
necessarily met it.

**Deferred: the homepage and README.** The live `trust-data.json` holds
no score yet (`trend.mutation_score.latest` null on 2026-10-25); the first
sweep lands on the first of the month. The 2026-10-13 entry carried the
CRC-32 from the changelog to both Principles lists because a trust fact
belongs where readers judge; this one follows once there is a score to
cite, because "we measure it" with no number answers no question.

## 2026-10-24: Editorial: the fuzz totals stopped being a 90-day window on 2026-10-11

`/status`'s "Is it tested?" paragraph said fuzz time and crashes are
totalled "over the last 90 days, which is as long as their records are
kept". #1077 made each run a permanent table row and the total
cumulative, and its changelog line said so; the page prose kept the old
window for thirteen days. The live `trust-data.json` read 57 entries
since 2026-09-02, 28.46 fuzz CPU-hours, 0 crashers. The sentence now
says every recorded night, and points at the date line the page already
renders under the grid, so the start date lives in the data and not in
prose.

**Rejected: typing the start date into the paragraph.** The asof line
already prints it from `since`; a second copy is one more sentence to
rot.

## 2026-10-24: Editorial: the site names its wins too, after the list held through a codec change

#1566, closing the 2026-10-22 entry's deferral. The homepage's Measured
paragraph now carries the README's two sentences, the eight wins and the
four widest losses, names in `<code>`. The condition was met: #1556
republished both reports and the README list still matched, and #1586
closed the empty-list hole the loss half had (#1535) before a second
surface could inherit it. `named_wins_and_widest_losses_match_their_generated_reports`
now reads both surfaces, one quoting rule each.

**Kept: the losses as a prefix of the ranking.** Dropping the last name
from either surface still passes, because a top three is as true as a
top four. The guard checks truth, not completeness; completeness is
review's.

## 2026-10-23: Editorial: the experiment floor said 10 rejections; the log holds 45

The Principles list on `/` said `research/progress.jsonl` holds "at
least 90 entries, at least 10 of them rejected". Both floors date from
#595 (2026-09-18), set against a true 92 and 12. Today the log holds 166
entries, 45 rejected, every one with a hypothesis and a mechanism. The
claim stayed true and stopped persuading: the line exists to show that
rejections get recorded, and it understated them more than four times.
Now "at least 160, at least 40", and "mechanism of failure" became "why
it worked or failed", because 121 of the entries are accepts.

**Kept: the floor.** The reasoning in the 2026-09-18 entry stands; an
exact count would fail CI on every experiment another seat records.
The cost of a floor is that it decays into an undersell, so it gets
re-read when a survey passes through the Principles list, not only when
the guard fails, because the guard never fails in that direction.

## 2026-10-23: Editorial: "will it build here" is answered before the build

#1516, from the 2026-10-22 survey's bzip3 study. Both "Try it" sections
now say the test suite runs weekly on Linux, Windows and macOS, each on
x64 and ARM64, and on Android x64, and link `docs/TESTING.md`'s lane
table. The sentence sits beside "Building it yourself is the only way",
where the reader decides to spend the build. Evidence: monster run
39014722259 (2026-10-17, scheduled), every runtime lane green on stable
and the MSRV.

**Rejected: "supported".** The sweep is advisory and alarms after the
fact; a promise would outrun it. "Runs weekly" says what happens.

**Rejected: the lane count, the ABIs, the MSRV.** A count rots (#559),
musl versus glibc answers no first-screen question, and the table holds
all three for the reader who descends. The sentence names families only,
so it moves when a whole OS or architecture leaves the matrix, which is
rare enough that review catches it; no guard parses the workflow.

## 2026-10-22: Editorial: the win count names its files, by name, not by class

#1515, adopting bzip3's "excels at text or code" in the only shape our
evidence holds. README now names the eight files mothergod wins and the
four it loses widest, with a plain-word kind for each (star catalog,
medical image), labels from the Silesia and Canterbury descriptions.
`tests/claims.rs` holds both lists to the reports: the wins exactly, the
losses as the top of the regret ranking.

**Rejected: a class sentence, "wins on small text".** Two facts break
it. The larger texts lose: `lcet10.txt` and `plrabn12.txt` by under
0.01 bits per byte, `dickens` by 0.054. And executables land on both
sides, `ooffice` a win and `sum` a top-four loss. A class claim would
outrun its evidence, and no test can check that a file is "text". File
names are checkable, and the reader can match them to their own data.
"Six smallest text and source files" stays as a description of the
list, true today by byte count; when a file changes sides the guard
fails and the sentence gets rewritten, not patched.

**Deferred: the site.** Its paragraph links the same reports, so the
sentence would be backed there too. The README is where the evaluating
engineer asks "will it win on my data", and one surface at a time keeps
the guard simple. The site follows if the README version holds through
a codec change.

## 2026-10-22: Survey: the site counter reads again, if read within the week

Eighth survey, seven days after the seventh.

### (a) Audience

| Metric | 2026-10-22 | 2026-10-15 | Source |
|---|---|---|---|
| Stars | 1 | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count` |
| Forks | 0 | 0 | same call, `forks_count` |
| Watchers | 0 | 0 | same call, `subscribers_count` |
| External issue authors, excluding machine-filed | 0 of 127 | 0 of 127 | `gh issue list --state all --limit 3000`: 96 `app/claude`, 31 `bugabinga`; 336 `app/github-actions` excluded |
| External PR authors | 0 of 1051 | 0 of 857 | `gh pr list --state all --limit 4000`: 1004 `app/claude`, 33 `bugabinga`, 14 `app/dependabot` |
| mothergod.dev pageloads | 14 in 7d | 0 recorded, not a number | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, account-level, site tag `7c1ab790…`, window 2026-10-15 06:51 UTC to 10-22 06:51 UTC, `avg { sampleInterval }` 1; a second fetch returned the same 14 |
| Hacker News mentions | 0 | 0 | Algolia API: query `mothergod.dev` returns no hits; query `mothergod` returns 50 hits, all read, none about the project |
| lobste.rs submissions | 0 | 0 | `https://lobste.rs/domains/mothergod.dev` still 404 |
| reddit mentions | not measured | not measured | `reddit.com/search.json` still 403 to the runner IP |
| Web search presence | absent | absent | WebSearch for `mothergod.dev lossless compressor` and for `"mothergod" bugabinga agent-built compressor Rust`: no result names the project |
| GitHub repo views, 14d | 35, 16 unique | 49, 28 unique | issue #435 ledger, top table, snapshot run 39113377763 (2026-10-18), window 2026-10-04..10-17 |

**The site row is a number again, and the reason is the window.** Last
week's entry blamed sampling that coarsens with age. Today's reads
narrow it. A window of 7 days or less, ending now, comes back at sample
interval 1: 14 pageloads, 13 on `/` and one on `/decisions`. Widen the
same window to 8 days and the answer becomes one sampled row worth 10,
interval 10. Events older than about a week read at 1 in 10 under any
window, even one day wide. So a full-resolution count exists only for a
7-day window read at its own end, which is the survey's window. Last
week's zero was read that way, so it now stands as a quiet week, not a
blind one. Earlier site readings stay unreproducible.

**What the 14 cannot say.** Six of them land one per hour on 10-19,
03:00 to 09:00 UTC, each a new visit. The cadence looks like one patient
reader or one scripted browser. Nothing in this repo is that browser:
`site-shots` serves pages from 127.0.0.1, where Cloudflare injects no
beacon. Bots that do not run JavaScript never reach this counter,
but a headless browser would. Read 14 as an upper bound on humans.

**The repo reader fell again.** Uniques went 22, 40, 28, now 16 over
the 14-day window. No day since 10-03 had more than 3 viewers. Two
windows down is the start of a trend, and its cause is still upstream:
arrivals, which no edit to the README buys.

### (b) Study: bzip3's README

Chosen because bzip3 is the closest thing in the wild to what mothergod
is: a young compressor, by one author, that leads with ratio, concedes
speed, and earned an audience (1523 stars, `gh api
repos/kspalaiologos/bzip3`, read 2026-10-22). Read the same day from
`raw.githubusercontent.com/kspalaiologos/bzip3/master/README.md`, 150
lines.

1. **A lineage places it in one phrase.** "A better, faster and stronger
   spiritual successor to BZip2." The reader knows the category before
   the first benchmark.
2. **One bold line names the data it wins on.** "BZip3 excels at
   compressing text or code." The benchmark that follows backs exactly
   that class.
3. **The benchmark is a story you can re-run.** It shows how the corpus
   was built (every Perl 5 release, fetched with a quoted `wget`) and
   each compressor's command line with time and memory, then the
   tables.
4. **The risk is stated in capitals, then argued.** "DO NOT COMPRESS ANY
   DATA WITH THIS PROGRAM UNLESS YOU ARE PREPARED TO ACCEPT THE
   POSSIBILITY..." follows a calm paragraph on why bugs cannot be ruled
   out.
5. **Tested architectures are a list.** It names ten, from x86 to s390x,
   right after the speed caveat.
6. **Independent benchmarks are linked.** powturbo's comparison sits
   beside its own numbers.

**Adopting (2): #1515.** The README gives a win count, 7 of 11 and 1 of
12, and leaves the reader to sort per-file rows. Those rows show a
pattern worth one sentence. mothergod wins every Canterbury text file of
152 KB or less, plus `kennedy.xls` and Silesia's `ooffice`. Its widest
losses are `sao`, `mozilla`, `mr` and `sum`. The sentence moves with the
codec, so the issue asks for a guard like the win count's.

**Adopting (5): #1516.** "Try it" asks for a build from source and never
says where that works. The weekly `monster` sweep already runs the
tests on Linux, Windows and macOS, x64 and ARM64, plus Android, and
`docs/TESTING.md` holds the table. One clause and a link, worded
"tested weekly", because the sweep is advisory and promises nothing.

**Already true: (3) and (4).** Our corpora are pinned by URL and
SHA-256, and `docs/benchmarks/` carries the regeneration command. The
pre-alpha paragraph already says not to trust it with data, and gives
the stronger reason: the next format change will not read today's file.

**Rejected: (1), a lineage.** mothergod descends from no predecessor
whose name a reader knows. Borrowing one, PAQ or xz, would claim a
kinship the code does not have. The README's "built for compression
ratio rather than speed" does the placing job without the borrowed
name.

**Not available: (6).** Nobody outside the project has benchmarked
mothergod. That link is earned, not written, and asking for one is
reach we do not buy.

### (c) What changes because of (a) and (b)

- **Name the data mothergod wins on**: #1515.
- **Name the platforms the tests run on**: #1516.
- The site row is read the same way next week: a 7-day window, at its
  own end, `sampleInterval` reported. A row whose interval is not 1 is
  a caveat, not a number.
- The sub-page conjecture's test stays the 2026-10-27 survey. This week
  the instrument can see a sub-page, once: one load of `/decisions`.

Rejected again, same reason as every prior week: buying reach by
posting anywhere. Two falling repo windows do not change that.

## 2026-10-21: Editorial: two roadmap promises outlived the roadmap

README and site called closing Silesia "the current milestone", a
sentence from #433 (2026-09-01). ROADMAP.md files it as M3, "a
program, not a milestone", first by section order, with no state in
which it is finished; M5, the parse engine work that attacked the
Silesia losses, closed 2026-10-20 with all seven losses to zstd -19
intact (S2-A128). The sentence now says closing Silesia heads the
roadmap as open-ended research with no date attached. A reader who
read "milestone" expected a date the project never set.

The site's Speed section promised speed would become "a target of its
own at the speed-tiers milestone"; ROADMAP.md folded that program into
M5 on 2026-10-18, and M5 parks a faster decoder until "a user who asks
for it on an issue they open". The roadmap's own trigger is a reader's
next step, so the sentence now ends on an issue link and asks the
reader to say so. That is an invitation to a real user, never a
prompt to manufacture demand.

**Rejected: a guard in `tests/claims.rs`.** A milestone's state lives
in GitHub Milestones (ADR-0047), which a test cannot read offline; a
test keyed on ROADMAP.md's prose would guard wording, not state. The
rule instead: the surface names the roadmap's standing ranking, never
a milestone by status word, because the status word rots on close.
Peak memory in `docs/benchmarks/` is labeled MB for MiB figures,
filed as #1502 because `bench/` is measurement code.

## 2026-10-21: Editorial: peak memory carries the unit its measurement produced

#1403. Three Unreleased `encode:` entries quoted `/usr/bin/time` peaks,
which are KiB, as MB: 877,156 KiB read "877 MB", 2.4% low against
decimal MB and labeled with neither. They now say MiB, the KiB figure
over 1024, rounded (857 MiB). The #1341 entry had the same shape and
the issue did not list it; it moved too.

**Rejected: converting to decimal MB.** The bound the reader compares
against is "1 GiB", and the #1382 entry beside these already writes
MiB; decimal MB would put 509 MiB and 521 MB for one run in adjacent
lines. The rule: a memory figure carries the binary unit its source
measured in. The 256 MiB "946 MB" entry stays, because its journal
source (S2-A116) records MB and no KiB to convert from.

## 2026-10-20: Editorial: "no internal parallelism" outlived the second core by a day

The site's Speed section opened by saying the codec has no internal
parallelism, a sentence from #483. #1394 gave `write::Encoder` two
blocks at once and #1402 gave a one-block file two filter-trial
threads; the CLI encodes through that path, and the reports published
in #1416 were measured at `7fbf7e9`, after both. So the quoted encode
rates are two-core rates under a sentence promising one. The sentence
now says encode uses up to two cores and decode one, and warns a
one-core reader to expect slower encodes (#1402 measured `dickens`
1.81 times faster with the second thread).

**Rejected: a guard in `tests/claims.rs`.** The fact lives in thread
spawns, not in a constant or a report a test can read; a test that
greps `src/` for `spawn` would guard a word, not the behavior. The
README carries the two-core fact in its memory paragraph already and
never made the false claim, so it stays as is.

## 2026-10-19: Editorial: "slow" got its comparison, from the same run

README and site said mothergod is slow and quoted its MB/s, with
nothing to measure them against: a reader had to know what `xz -9e`
does to read 0.134 as slow or fine. The finals reports gained an
every-level frontier table this morning (#1373, published in #1383),
so the competitor rates now come from the same run on the same
machine. Both surfaces quote `xz -9e` on Silesia, 2.249 MB/s encode
and 73.047 MB/s decode, and say "more than ten times faster both
ways". `tests/claims.rs` holds the rates to the report and the
multiplier to both ratios, so a faster encoder fails the test before
the sentence turns false.

**`xz -9e` on Silesia, not zstd:** it is the column the Measured table
already names, and on Silesia it also beats mothergod on ratio, so the
reader sees one competitor that wins on both axes. It is not the
harshest anchor: `zstd -19` decodes Silesia at 601.258 MB/s, about 259
times mothergod's 2.320, where `xz -9e` is about 31 times (#1386). **Rejected: an exact
multiplier ("17 times").** Every regeneration moves it, and a
multiplier the reader can recompute from two quoted rates adds a
third number to rot. **Rejected: quoting mothergod's peak memory in
this PR.** The frontier's 652.3 MB on Silesia is a reading per run;
the 2026-10-17 entry's reasoning (publish the bound, not the reading)
stands, and the new sentence points at the column instead.

## 2026-10-18: Editorial: the release notes named a format three versions old

`CHANGELOG.md`'s summary said the bitstream is `FORMAT_VERSION` 13 and
that versions below 13 no longer decode. The build writes 16 and
decodes only 16 (`src/lib.rs`), so a reader of the first release's
notes would have kept 13, 14 and 15 files expecting them to decode.
Three format bumps (#1123, #1206, #1222) each added their own bullet
and none touched the summary, because `tests/claims.rs` held README
and site to the constant and skipped the changelog.

Both numbers now read 16, and the same test holds them. **Rejected:
dropping the number from the summary.** The release notes are where
a packager looks for which files decode; the number belongs there,
guarded, rather than nowhere.

## 2026-10-17: Editorial: "one 16 MiB block at a time" read as a memory figure

#1145. The README's streaming sentence named the block size and no
peak, so an embedder sizing a container read 16 MiB; the encode peak
on 256 MiB of zeros is about 700 MB (`memory-check` footer, quoted
in the issue). The sentence now separates the two: memory follows
the block, not the input, and the number to plan around is the
bound the weekly lane enforces, under 1 GiB on that corpus.

Rejected: quoting the measured peak. It is a weekly reading that
moves whenever the encoder's tables do, and the README would drift
from the lane's log; the bound is the published claim and the lane
goes red before the sentence turns false. Not claimed: any peak on
incompressible input, which nobody has measured. The two rustdoc
sites in #1145 are the maintainer's and stay open there.

## 2026-10-17: Editorial: the 404 page counted its siblings, and the count rotted in a day

#1229 added `/decisions` to the 404's frame, and its sentence still said
"Three pages do", listing three. The frame was right because
`tests/claims.rs`'s `shared_frame_offers_the_same_links_on_every_page`
holds it equal on every page; the prose had no such guard.

The sentence now claims only what that test enforces: every page is
linked above, and the reader starts at the overview. **Rejected:
"Four pages do", with the fourth link added.** That is the same count
waiting for the fifth page. A sentence that points at a tested
structure does not need its own guard; a sentence that restates the
structure does.

## 2026-10-16: Editorial: /decisions shipped; one sentence leads to it, from where "who decides" is asked

#1231, closing out the 2026-09-24 entry's hand-over: #1229 put the page
on `mothergod.dev` this morning.

**What changed from the 2026-09-24 design.** Links are extensionless
(`/decisions`, matching `/status` and `/agents`). The
frame link sits on all four sibling pages plus the 404. Without
JavaScript the page says so instead of showing "Loading". The counts are
now 73 decisions, 67 current, 6 superseded, generated, so this file will
not restate them again.

**Verified from the deployed root, not the checkout.** `adr-data.json`
on mothergod.dev lists 73 records and an empty `problems` array, equal to
`ls docs/adr | wc -l` on `main`. The deep link `/decisions#0030`, rendered
in headless Chrome with a virtual-time budget and read as a DOM dump,
titles itself "ADR-0030: Stable architecture decision records" and shows
its Context, Decision and Consequences.

**Where the page is linked, and where it is not.** #1231 expected the
README and the agents page to mention `docs/adr/`; grepping both found no
such mention, so there was nothing to convert. The question a reader of
decisions arrives with is "who runs this, and by what rules", and both
README and homepage already answer it in one paragraph ending "How it
works: agents/GOVERNANCE.md". The link went there, as one sentence
naming what the page holds, reversals included, since supersession is the
part a skeptic cannot get from a governance document. Rejected: a link on
`/agents`. That page is run telemetry; its reader came for numbers, and
the frame already offers the way out. Rejected: a line in "Principles",
which states rules a reader should keep in mind, not places to go.

**To measure.** Whether `/decisions` gets read at all; the traffic
snapshot owns that number, and the next survey reads it.

## 2026-10-16: Editorial: the smoke test is one a reader can run twice

#1216. The quoted `mothergod -v LICENSE` wrote `LICENSE.mgdc` into the
checkout, so the second run failed with `File exists`. Both pages now
quote `mothergod -v -c LICENSE > /dev/null`, which prints the same line,
writes nothing and exits 0 every time; `tests/claims.rs` runs it with
stdout discarded, the way the page does.

**Rejected: bare `-v -c LICENSE`,** the fix the issue thread preferred.
On a terminal it prints `compressed data not written to a terminal` and
exits 1, so every reader would hit an error on the first run instead of
the second. The test missed this because its stdout was a pipe. A quoted
command gets tried in a terminal before it ships, not only in a test.

**Rejected: keep the command and add a clause about the file.** The
second run still fails; a sentence warning about a failure we can avoid
is the wrong fix.

## 2026-10-15: Editorial: "Try it" shows what a first run prints, and a test holds the number

#1196, from this morning's survey. Both "Try it" sections now quote one
transcript, `mothergod -v LICENSE` and the line it prints, between the
build and the round-trip recipe. A trier sees the tool's voice before
building it, and learns that silence is success.

**`LICENSE`, not a file of the reader's:** it ships in the clone and has
not changed since the initial commit, so the quoted line is reproducible
by anyone who followed the build. **Pinned, not regenerated:**
`tests/claims.rs` runs the built binary on it and fails naming page,
quoted line and printed line, so a codec change that moves 638 cannot
merge with the pages stale. One mechanism, beside the recipe guard it
mirrors.

**Rejected: the gzip/zstd/xz comparison on the same file.** One 1 KB file
is not a corpus; the pages say so in one clause and point at the tables.

## 2026-10-15: Survey: the repo reader fell back, and the site instrument went blind at our volume

Seventh survey, seven days after the sixth.

### (a) Audience

| Metric | 2026-10-15 | 2026-10-08 | Source |
|---|---|---|---|
| Stars | 1 | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count` |
| Forks | 0 | 0 | same call, `forks_count` |
| Watchers | 0 | 0 | same call, `subscribers_count` |
| External issue authors, excluding machine-filed | 0 of 127 | 0 of 127 | `gh issue list --state all --limit 3000`: 96 `app/claude`, 31 `bugabinga`; 211 `app/github-actions` excluded |
| External PR authors | 0 of 857 | 0 of 686 | `gh pr list --state all --limit 4000`: 812 `app/claude`, 33 `bugabinga`, 12 `app/dependabot` |
| mothergod.dev pageloads | 0 recorded, not a number | 7 in 6.5d | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, account-level, site tag `7c1ab790…`, window 2026-10-08 06:51 UTC (last read) to 10-15 06:51 UTC; read again day by day, same zero; see below |
| Hacker News mentions | 0 | 0 | Algolia API: query `mothergod.dev` returns no hits; query `mothergod` returns 20 hits, all read, none about the project |
| lobste.rs submissions | 0 | 0 | `https://lobste.rs/domains/mothergod.dev` still 404 |
| reddit mentions | not measured | not measured | `reddit.com/search.json` still 403 to the runner IP |
| Web search presence | absent | absent | WebSearch for `mothergod.dev lossless compressor` and for `"mothergod" bugabinga agent-built compressor Rust`: no result names the project |
| GitHub repo views, 14d | 49, 28 unique | 75, 40 unique | issue #435 ledger, top table, snapshot run 38133636798 (2026-10-11), window 2026-09-27..10-10 |

**The site row is a caveat this week, not a number.** Reading the new
window returned zero pageloads, both as one window and as eight one-day
windows. Before trusting that, I re-read last survey's window. It
reported 7 pageloads a week ago. Today the same query returns 20: two
rows of 10 at 2026-10-03 06:00 and 13:00, `avg { sampleInterval }` 10.
The three 10-07 pageloads are gone. A 30-day read returns only 10-03.
The mechanism: Cloudflare's adaptive sampling re-reads stored events at
a coarser sample as they age. At 1 in 10, a site with single-digit
weekly traffic keeps zero or one sampled event per week. So a zero
cannot be told apart from a quiet week, and no earlier site reading in
this journal is reproducible today. They stay what they were when they
were read: readings, not a series.

**Rejected as the replacement: zone-level `httpRequests1dGroups`.** It
is unsampled, and it reads about 30 `pageViews` a day for the zone
(13 to 435). It counts every crawler and probe that requests HTML,
and the bot score that would separate them requires a paid plan.
Swapping a blind human counter for a sighted bot counter would not
improve the row. No `blocked-on-human` issue, because no permission or
secret unlocks unsampled RUM. Next survey reads the row the same way
and says plainly what it can and cannot show.

**The repo reader fell back after three doublings.** Uniques went 12,
22, 40, now 28. The ledger's daily rows show where: 09-27 and 09-28
had 5 and 7 viewers; no day since 10-03 has had more than 3. One
window down is a reading, not a trend. README edits cannot explain
it: the drop starts 10-03, the opener rewrite (#941) landed 10-08, and
a page's text reaches only readers who have already arrived. Arrivals
are reach, and reach is not ours to buy.

### (b) Study: TigerBeetle's README

Chosen because TigerBeetle sells correctness to the same evaluating
engineer, and its README is the front door of a project whose site
holds the depth. That matches our split, where the repo has more
readers than the site. Read 2026-10-15 from
`raw.githubusercontent.com/tigerbeetle/tigerbeetle/main/README.md`,
72 lines.

1. **One italic sentence: what, for whom, two properties, a horizon.**
   "The financial transactions database designed for mission critical
   safety and performance to power the next 30 years of OLTP."
2. **The README is short on purpose.** It has a documentation link
   list, a Start section, and a security contact. Depth, including
   the engineering methodology (`TIGER_STYLE.md`), is one link away
   and never restated.
3. **Start is a transcript.** Each command is followed by what it
   actually printed, timestamps included. The reader sees the product
   working before running it.
4. **The security contact is the last line.** For a product whose
   pitch is safety, the place to report a breach of that pitch is on
   the front page.

**Adopting (3): #1196.** Our "Try it" shows commands, and on README one
comment about `cmp`. Since #1160, `-v` prints a line worth showing.
On a fresh build, `mothergod -v LICENSE` prints
`mothergod: LICENSE: 1074 -> 638 bytes (59.4%)`. `LICENSE` is in the
reader's clone and its bytes do not change, so the example will not go
stale with the next README edit. The issue asks for the line to be
pinned or regenerated, because a codec change moves it.

**Already true: (1) and (4).** The README's bold opener already does (1)
in our own voice. `SECURITY.md` exists at the root, and GitHub links
it from the repo's sidebar, which is where a repo reader looks.

**Rejected: (2), the short README.** TigerBeetle can be short because
its docs site has readers. Ours has fewer than the repo, and this
week's instrument cannot even count them. The README stays the
complete front door, including the library example. Revisit when the
site is the measured entry point.

### (c) What changes because of (a) and (b)

- **One new issue: "Try it" shows a run's output, on README and site**:
  #1196.
- The site row stays in the table with its caveat. The repo ledger is
  now the only USERS instrument that reproduces.
- The sub-page conjecture's test stays the 2026-10-27 survey. Expect
  it to fail on instrument grounds before traffic grounds, for the
  sampling reason above.

Rejected again, same reason as every prior week: buying reach by
posting anywhere. Falling uniques do not change that.

## 2026-10-14: Editorial: "Try it" became the man page, one clause per CLI PR

Queue empty, so this is a defect found on the surface. #1153, #1160 and
#1179 each appended a flag to the paragraph under "Try it", on README and
`/`, after #983 had put `-c`, `-f`, `-k` and `-` there. The paragraph
reached 134 words on README and 175 on `/`, a flag inventory between the
trier and the next section, restating `--help` with no guard:
`tests/claims.rs` checks only the recipe lines' options, not the prose.

**Both now keep what a trier needs before running it, and send the rest
to `--help`:** a file gets `<file>.mgdc` beside it, the input is never
deleted (a `gzip`-trained hand expects the opposite), and `--help` lists
the rest. 61 and 103 words, same count method as the 2026-10-09 entry.
Evidence, a release build of `cc6aaa6`: `mothergod f` left `f` and
`f.mgdc`, a second run exited 1 on the existing `f.mgdc`, `-d` restored
`f` byte for byte, and `--help` is nine lines. "Flags you know from
`gzip`": each of `-c -d -f -h -k -q -t -v -V` appears in `gzip 1.12
--help` under the same long name.

**Rejected: keeping "the entire interface so far".** It was the sentence
that invited each new flag in; "no compression levels or tuning knobs"
is the claim the reader needs, and it stays. The changelog keeps one
bullet per flag, because release notes are where a returning user looks
for what is new.

## 2026-10-14: Editorial: the streaming pair grew a third member, and the README kept two

Queue empty, so this is a defect found on the surface. #1150 added
`write::Encoder`, the `std::io::Write` side, hours after the entry below
showed the streaming pair. An embedder whose producer writes, a `tar`
builder or a serializer, read `copy_encode` and concluded they must buffer
or pipe first. Only the changelog said otherwise.

**README now names `write::Encoder` in one sentence after the streaming
example, with the one trap it carries: without `finish`, the last block
never reaches the file.** Evidence, an example built against `fa861d7`: a
636079-byte mix of Rust source and random bytes, written in 4096-byte
chunks, came out byte for byte what `copy_encode` writes, and `mothergod
-d | cmp` was silent. The same encoder dropped without `finish` left a
0-byte file.

**Rejected: a third code block.** It would repeat the example's shape with
one name changed; the type's own doc example is a `cargo doc --open` away.

## 2026-10-13: Editorial: the README's library example stopped at the slice API

Queue empty, so this is a defect found on the surface. The README's
"Library API" showed `compress` and `decompress` on a byte slice and
nothing else. A Rust user evaluating the crate for real files would
conclude it holds everything in memory, though #1106 and #1134 gave it
`copy_encode` and `read::Decoder`, the `std::io` pair the genre's crates
offer. Only the changelog named them.

**README now says the slice pair holds everything in memory, then shows
the streaming pair in four lines, and points at `cargo doc --open`** for
each call's memory bound, because no docs.rs page exists before a
release. Evidence: those four lines, built as an example against
`41255ab`, round-tripped a 1500000-byte mix of Rust source and random
bytes (`cmp` silent) at a 105 MB peak RSS.

**Rejected: stating `decompress`'s 256 MiB cap.** It covers `Lz` output
only, a `Stored` frame passes it, and the true sentence is one the
reader does not need to pick the streaming pair. **Rejected: a library
section on `/`.** The site's visitor decides whether to try the tool;
the embedder reads the repo, and `/` already sends them there.

## 2026-10-13: Editorial: the strongest new trust fact reached only the changelog

Queue empty, so this is a defect found on the surface. #1118 ended every
frame in a CRC-32, so a damaged file is an error instead of wrong bytes
with exit status 0. That answers the evaluating engineer's "can I trust
it" more directly than "never panics" does, and README and `/` said
nothing; only `CHANGELOG.md` did, the page that reader opens last.

**Both Principles lists now say it, in the reader's terms: a flipped bit
is an error, never quietly wrong output.** Evidence, a release build of
`b80f1d5`, format 13: a 13476-byte file compressed to 5737 bytes; 1014
single-bit flips (every bit 0, 3 and 7 of the first 40 bytes and of 300
random ones) each exited non-zero, 968 of them on the check value, the
rest rejected earlier by the header or extent parse. None exited 0.

**Rejected: "damaged input is always an error."** CRC-32 catches every
single-bit flip and every burst up to 32 bits; wider damage slips past
one time in 2^32. The surfaces claim the flipped bit, which is what the
evidence covers. **Rejected: naming zstd and lz4 as the half of the
genre that decodes flips silently at their defaults** (`tests/fit/lib.txt`,
`integrity-check`). True and measured, but on `/` it reads as a jab at
tools the reader likely uses, and the claim stands without it.

## 2026-10-12: Editorial: the big-file warning outlived its bug by one bullet

Queue empty, so this is a defect found on the surface. #1106 made the
CLI compress through `copy_encode` and took the entry below's warning off
README and `/`. `CHANGELOG.md` kept it as its own bullet: "an input over
256 MiB is stored ... until large inputs are split". A first-release
reader would believe their VM image comes back uncompressed. The CLI
bullet also said "Decompression streams instead of buffering the whole
file", and `src/bin/mothergod.rs` reads the compressed input whole.

**The stored-input bullet is gone; its one true remainder, that the
library's `compress` stores past 256 MiB, rides in the `copy_encode`
bullet, which already named the boundary.** The CLI bullet now says
decompression reads the compressed file whole, with #978, because the
same reader with a 10 GB tar needs that before decompressing it.
Evidence, a release build of `69750ff`, format 12: a 268435457-byte file
of zeros compressed to 6175 bytes in 412 s at a 716620 KB peak RSS, and
`-dc | cmp` was silent. "where it read its input whole before" also left
the `copy_encode` bullet: no release had the earlier behavior.

**Rejected: printing the 412 s.** It is one input class on one runner,
and in release notes it reads as a speed claim; the README's Speed
section owns speed, with its corpus named.

## 2026-10-12: Editorial: a trier with a big file got its own size back, unwarned

Queue empty, so this is a defect found on the surface. #1090 made
`compress` store any input over 256 MiB, so that every frame it writes
decodes. That is correct and costs nothing on the benchmarks, whose
largest file is under 256 MiB. But "Try it" asks the reader to round-trip
"something of yours", and a VM image or tarball is the obvious thing to
reach for. The reader would see an output a few bytes larger than the
input and conclude the compressor is broken. Only the changelog said so,
under "Fixed", for a bug no release ever shipped.

**README and `/` now say it under "Try it", in bold, with #978 as the
next step.** It also says the CLI reads each input whole into memory,
because the same reader with a big file needs that before running it.
Evidence, a release build of `21473a2`, format 12: a 268435457-byte file
of zeros came out at 268435471 bytes in 0.50 s with a 526568 KB peak RSS,
and `-d -c | cmp` was silent. A 1 MiB file of zeros came out at 53 bytes.
`src/bin/mothergod.rs`'s `read_input` reads to the end in both modes. The
256 MiB is `codec::MAX_DECODED_LEN`.

**The changelog's "Fixed" bullet is now an Added limitation**, same
reason as the entry below: a first-release reader had no earlier
behavior that was fixed.

**Rejected: printing the 14-byte overhead.** It is the Stored header,
and it can change with any format bump. "A few bytes" stays true, and
the exact count changes nothing the reader does.

## 2026-10-11: Editorial: the changelog kept the sentence README and site lost

Queue empty, so this is a defect found on the surface. The entry below
fixed "`--help` is the entire interface" on README and `/`; `CHANGELOG.md`
still said "`mothergod --help` is the whole interface", one sentence before
naming `--version`. Release notes are the surface a first-release reader
reads whole, so the CLI bullet had grown into five facts appended by four
PRs, and two more CLI facts sat under "Changed", relative to a release
that does not exist.

**The CLI is now two Added bullets: what the two subcommands do, then the
two flags and what `--version` is for.** The two "Changed" CLI bullets
folded in, because a reader of the first release's notes had no earlier
CLI for them to change. Every behavior checked against a release build of
`62cd37f`: `--help` and `-h` print the usage; `--version` and `-V` print
`mothergod 0.1.0 (format 11)`; `compress f` keeps `0640` and an mtime of
2001-09-09 on `f.mgdc`, and `decompress f.mgdc` keeps both on `f`; a
second `compress f` exits 1 on the existing `f.mgdc`; `decompress | head
-1` exits 141; `compress` under `script(1)` prints "compressed data not
written to a terminal", exit 1.

**Rejected: restating the help text's grammar** (`<compress|decompress>
[file]`). `--help` owns it, and `tests/claims.rs` already guards the
README's recipe against it; a third copy in the changelog would be
unguarded.

## 2026-10-11: Editorial: "no flags" stopped being true on 2026-10-10

Queue empty, so this is a defect found on the surface. README and `/`
both said "`mothergod --help` is the entire interface: no flags". #1035
added `--version` and `-V`, and `--help`'s own usage line does not list
them, so the sentence was false twice. Checked against a release build
of `b4f57cb`: `--help` prints one usage line, `--version` prints
`mothergod 0.1.0 (format 11)`.

**Both surfaces now name the three entries and give `--version` a job:**
put it in a bug report, because the format version is the one fact that
says which frames a build reads. The reader keeps "no compression
levels, no tuning knobs", which is the claim that matters to them.

**Rejected: listing `-V` too.** The sentence is about how small the
interface is, not its aliases; `CHANGELOG.md` names the short spelling.

## 2026-10-10: Editorial: the homepage's trust link landed on factory vocabulary

Queue empty, so this is a defect found on the surface. `/`'s panic
principle sends the reader to `/status` for the live fuzz time and crash
count (2026-09-18 entry). The card it lands on opened with an issue
number, a script path, "artifact window" and "the weekly digest", a
record no visitor can find. It answered "how is this number produced"
for the factory before "is it tested" for the reader.

**The card now says what runs and how often, in the reader's words,**
then what the numbers cover, then links `docs/TESTING.md` for the
engineer who descends. Evidence: `fuzz-check.yml` runs nightly
(`cron: "13 2 * * *"`) at `FUZZ_BUDGET_S: 1800`, and the live ledger's
fuzz entries record 1812 to 1986 CPU-seconds each, so "half an hour";
`fuzz/fuzz_targets/roundtrip.rs` asserts `decompressed == data`, so "a
round trip that comes back different"; `coverage-check.yml` is weekly
(`cron: "51 4 * * 0"`); both upload with `retention-days: 90`. The two
pending stat labels lost their issue numbers; #454 (coverage) has closed
and the live ledger shows 97.38%.

**Rejected: keeping `trust-telemetry.py` linked in the card.** Its
docstring is the factory's; `docs/TESTING.md` is the human page about
the same portfolio, and the script stays one click further for whoever
reads the strategy.

## 2026-10-10: Editorial: "can be retired" undersold a rule that now always fires

Queue empty, so this is a defect found on the surface. ADR-0063 (#949)
made it a rule: until the first release, a `FORMAT_VERSION` bump retires
its predecessor in the same PR, and `decode` reads exactly one version.
The README said "until 1.0 a version can be retired"; the homepage and
the status page said a file "carries no promise" a later build reads it.
Both true, both written for a possibility. The trier who compresses a
file today, pulls next week, and gets `UnsupportedVersion` deserved the
certainty, and versions 7 through 10 landed between 2026-10-01 and
2026-10-08 (#834, #869, #873, #930).

**All three surfaces now say it as a consequence:** the next build that
changes the format will not read a file you compress today. Then the two
later regimes, one clause each, with "until 1.0" kept because
`tests/claims.rs`'s `format_promise_claims_agree_with_the_changelog`
watches that phrase. Scope: a `Method::Stored` frame (input the codec
stored verbatim) still decodes under any version up to the current one,
per `docs/format/SPEC.md`; "a file you compress" reads as the compressed
case, and the exception does not change what a trier should do.

**Rejected: a bump rate ("about weekly").** It rots with the next quiet
week, and the certainty, not the frequency, is what changes the action.
Status box: 81 words to 92, same count method as the 2026-10-09 entry.

## 2026-10-09: Editorial: the origin story goes where the reader asks "who"

#963: the surface never said mothergod began as an autoresearch loop.
**Placed it as the first paragraph of "Who builds it", README and site,
not the opener.** The opener answers "what is this, is it any good", and
the 2026-10-08 entry just emptied it of everything else; an origin story
there would push the table down again. "Who builds it" is where the
reader asking how this was made already looks, and the agent paragraph
that follows reads as what the loop grew into.

Evidence per claim: the date and "about 40 iterations" from
`research/JOURNAL.md`'s S1 preamble ("~41 loop iterations"; the in-tree
archive holds it1 to it31, the rest are journaled only, so "about").
The loop's steps from `autoresearch2.py` at `1a3b1c8`: the train/val
scoring, the journal's `accepted` field, and the `NOT INVERTIBLE` assert,
which covers `kind == 'filter'` only (`param` has no check, `context`
checks an index range, `corpus` a regret threshold), so the sentence
says "a proposed filter", never "the change". Acceptance also takes
`dv<-0.05` ("rule v2"), so VAL steers it: "validation data", not
"held-out" in `research/corpus/POLICY.md`'s sealed sense. karpathy/autoresearch's metric (`val_bpb`) from its README, read
2026-10-09. "Still runs": `research/progress.jsonl` it190 is dated
2026-10-08.

**Rejected: the issue's counts (190 experiments, 97k stars).** Both rot
by the week, and the iteration ids already say it on the journal for
the reader who descends. **Rejected: any word implying inspiration or a
ratio comparison**: the sentence states a shared metric and nothing more,
per the issue's own constraint. Measure: the next four surveys' repo
numbers, per the issue.

## 2026-10-09: Editorial: the site's Status box explained the pipeline twice

Queue empty (no open `marketing` issue, no `claude/*` branch), so this is
a defect found on the surface. #923 took the pipeline out of the README's
opener; the site carried the same shape one screen down. Its Status box
named the method enum (`Stored`, `Lz`), then described the LZ stage and
the arithmetic coder, and the very next section, "What this is", described
both again. Then it inlined the post-release retirement procedure, a rule
for releases that do not exist yet.

**Cut the box from 158 words to 81, to what a visitor deciding whether
to try it needs:** pre-alpha, the format is versioned but not frozen, a
file compressed today carries no promise a later build reads it, nothing
is retired from 1.0 on, and the spec owns the rules in between. The
"no promise" clause stays because it is the one fact that changes a
trier's action, and it matches the status page's own wording. Counts:
`awk '/<div class="status">/,/<\/div>/' site/index.html`, tags stripped,
`wc -w`, before and after.

The README already reads this way since #849; the two surfaces now say
the same thing at the same length.

## 2026-10-08: Editorial: the README opener lost its pipeline, and gained no pointer

#923 cut "A filter bank feeds an optimal-parse LZ..." from the README's
first paragraph. What remains is the site tagline's pitch: what it is,
what it trades, how to judge it, where the evidence lives.

**Rejected: a replacement link to "the architecture".** The issue
allowed one. No page written for a visitor states the pipeline as an
overview (grep for "filter bank" over `*.md` and `*.html` outside
`research/` and `agents/`: only `CLAUDE.md`, changelog lines, and this
journal). `CLAUDE.md` is the agent contract; linking it would hand the
reader the factory's instructions. The `research/JOURNAL.md` link already
serves the engineer who wants to descend. Revisit when a design page
for humans exists.

## 2026-10-08: Survey: the repo reader doubled again, and the README greets them with the pipeline

Sixth survey, seven days after the fifth.

### (a) Audience

| Metric | 2026-10-08 | 2026-10-01 | Source |
|---|---|---|---|
| Stars | 1 | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count` |
| Forks | 0 | 0 | same call, `forks_count` |
| Watchers | 0 | 0 | same call, `subscribers_count` |
| External issue authors, excluding machine-filed | 0 of 127 | 0 of 127 | `gh issue list --state all --limit 3000`: 96 `app/claude`, 31 `bugabinga`; 109 `app/github-actions` excluded |
| External PR authors | 0 of 686 | 0 of 632 | `gh pr list --state all --limit 4000`: 641 `app/claude`, 33 `bugabinga`, 12 `app/dependabot` |
| mothergod.dev pageloads | 7 in 6.5d | 4 in 7d | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, site tag `7c1ab790…`, window 2026-10-01 18:52 UTC (last read) to 10-08 06:51 UTC, re-fetched at 06:52 with the same total |
| Hacker News mentions | 0 | 0 | Algolia API: query `mothergod.dev` returns no hits; query `mothergod` returns unrelated fuzzy matches ("Motherlode") |
| lobste.rs submissions | 0 | 0 | `https://lobste.rs/domains/mothergod.dev` still 404 |
| reddit mentions | not measured | not measured | `reddit.com/search.json` still 403 to the runner IP |
| Web search presence | absent | absent | WebSearch for `mothergod.dev lossless compressor` and for `"mothergod" bugabinga agent-built compressor Rust`: no result names the project, not even `github.com` this week |
| GitHub repo views, 14d | 75, 40 unique | 56, 22 unique | issue #435 ledger, top table, snapshot run 37196071166 (2026-10-04), window 2026-09-20..10-03 |

**The machine-filed issue row retired, as last week promised.** Machine-filed
issues still grow (89 to 109); nothing else did.

**Site detail, same source, by hour:** 10-03 06:00 UTC two Edge; 10-03
13:00 and 15:00 one Chrome each; 10-07 09:00 two Chrome; 10-07 12:00 one
Chrome. All on `/`, all desktop, all US, every referrer empty. The Edge
pair at 06:00 repeats last week's (10-01 06:00, two Edge), and both
mornings had a herald run at 06:49. Every other morning this week had
one too, with no Edge pair, so the coincidence explains nothing. It
stays a caveat: if the pair is the factory, the week's human count
is 5.

**Web search lost the repo results too.** Last week 20 of 20 results
were `github.com` pages; this week neither query returns anything about
the project. One search tool, one afternoon each, so this is a reading,
not a trend. The sub-page conjecture (clock started 2026-09-27) is
still at zero, as expected before 10-27.

**The week's finding is the same split, wider.** Repo uniques went 12,
22, 40 across three surveys; site pageloads went 7, 4, 7. The two
instruments count different things over different windows, so no ratio
between them is honest; the direction is. The README is the front
door, and the site's
first screen is the better of the two pitches: its tagline says what
the thing is and how to judge it, while the README's second sentence
is the codec's pipeline ("A filter bank feeds an optimal-parse LZ with
in-DP repeat offsets..."). The reader with the most traffic gets the
most internal vocabulary.

### (b) Study: ziglang.org

Chosen because Zig is a pre-1.0 systems project that sells to the same
evaluating engineer we do, and its front page has to be honest about
immaturity without leading with it. Read 2026-10-08 from the raw HTML
of `https://ziglang.org/`.

1. **The tagline is what and three adjectives; mechanism never reaches
   the first screen.** "A general-purpose programming language and
   toolchain for maintaining robust, optimal and reusable software."
   How comptime works sits below, as three bullets, ending in a "Full
   overview" link.
2. **Maturity is a version label, said once.** "Latest Release: 0.17.0"
   sits beside the "GET STARTED" button. No sentence on the page says
   "unstable", "breaking" or "1.0"; the `0.` carries it, and release
   notes one click away carry the detail.
3. **The code sample ends in its own run.** `index.zig` is followed by
   the shell output of `zig test` passing. The page shows the thing
   working, not only the thing.
4. **Backing is a section, not a pitch.** The foundation and its
   sponsors sit last, below community and development.

**Adopting (1), and it is where the README diverges from the site.** The
site's tagline already reads the way Zig's does. The README's opener
carries a sentence whose three key terms answer no visitor question.
That is #923.

**Already true: (3).** "Try it" ends in `cmp FILE FILE.out   # silence
means the round trip was lossless`, which is the run, stated, for the
one property that matters.

**Rejected: (2), for now.** A version label signals maturity only when a
version exists. We have no release and no tag, so prose has to say it.
Checked whether the README says it too often: "Pre-alpha: no release,
no packaged binary, no version tag" and, eight lines down, "There is no
download: no release, no tag, no package." Kept both, because each sits
where its reader is: the first answers the evaluator reading status,
the second the trier who just reached the commands. Revisit when a
`0.x` tag exists and can carry the first one.

**Rejected: the ⚡ headings and the language selector.** House voice
allows at most one emoji, and we have one language's worth of readers.

### (c) What changes because of (a) and (b)

- **One new issue: the README's opening paragraph says what a reader
  gets, not the pipeline**: #923.
- The sub-page conjecture's test stays the 2026-10-27 survey.

Rejected again, same reason as every prior week: buying reach by
posting anywhere. Zero mentions is a real number and it stays one.

## 2026-10-03: Editorial: the Measured section's win-count claim had the same unguarded shape

`#443` remains the only open `marketing` issue, still blocked on `#739`
(agent-system, unmerged). Queue empty again, so this is a second defect
found directly on the surface rather than from the queue, found while
re-checking the entry above against today's regenerated reports: the CPU
model matched again (`7763` this time, per `#882`'s regeneration), but
reading the same paragraph's neighbor sentence found a second instance
of the exact pattern that entry just closed.

**Found: README.md and site/index.html both say mothergod "wins 6 of 11
on Canterbury and 1 of 12 on Silesia," and nothing in `tests/claims.rs`
compared either number to `docs/benchmarks/{canterbury,silesia}.md`.**
Hand-verified both counts against the reports' own regret columns before
trusting them (Canterbury: `asyoulik.txt`, `cp.html`, `fields.c`,
`grammar.lsp`, `kennedy.xls`, `xargs.1` win, 6 of 11; Silesia: `ooffice`
alone, 1 of 12): both currently true, so this was a guard gap, not yet a
false claim, the same state the CPU model was in before its CI host
rotated twice.

**Fixed by adding `win_count_claims_match_their_generated_reports`,**
counting negative-regret rows per report and comparing against both
surfaces. Verified by breaking it both ways before trusting it: bumping
README's count to "7 of 11" fails naming 7 vs 6; flipping one file's
regret sign in the report fails naming 6 vs 7, both reverted after.
`cargo test -p mothergod --test claims`: 20 for 20 (was 19). `cargo x
check`: clean.

No `CHANGELOG.md` entry: a test-only change with no surface diff, same
precedent as the entry above and `72baf65` (#803).

## 2026-10-03: Editorial: the Speed section's CPU claim had no guard, and drifted

`#443` remains the only open `marketing` issue, still blocked on `#739`
(agent-system, unmerged). Queue empty, so this is a defect found on the
surface directly, not from the queue.

**Found: `site/index.html`'s Speed section named "AMD EPYC 9V74" as the
benchmark machine; both `docs/benchmarks/canterbury.md` and `silesia.md`,
regenerated today (2026-10-03), say "AMD EPYC 9V45".** The CI runner's CPU
model is not pinned, so GitHub Actions can hand the benchmark job a
different physical host between runs: `9V74` (2026-09-02, `e2ba004`,
#483) became `7763` (2026-09-27, `72baf65`, #803) became `9V45` (today),
with the site's prose caught and fixed by hand the first two times but
never guarded. `tests/claims.rs` already compares the restated ratio,
throughput, measurement date, and reference-compressor-version claims
against the same generated reports; the CPU model was the one fact in that
Speed section paragraph nothing compared, so it could go stale with
nothing failing CI, #469's exact pattern against a different fact class.

**Fixed the text to `9V45`, matching both reports, and added
`published_cpu_model_matches_its_generated_reports` to close the gap.**
Verified by mutation: reverting the site string to `9V74` fails the new
test with `site/index.html's CPU model is 9V74,
docs/benchmarks/canterbury.md (Canterbury) says 9V45`; restoring `9V45`
passes. `cargo test --test claims`: 19 for 19 (was 18). `cargo x check`:
clean.

No `CHANGELOG.md` entry: a copy edit correcting a drifted fact, not a
product change, same precedent as `72baf65` (#803) and the entry below.

## 2026-10-02: Editorial: README made readers scroll past two paragraphs to reach "Try it"

Queue: #849, unclaimed, filed by the 2026-10-01 survey below and accepted by
the curator at 172ea7a. #443 is the only other open `marketing` issue; its
build still sits in #739, agent-system territory, unmerged.

**Fixed: trimmed the speed and pre-alpha paragraphs to their verdicts,
linking the detail instead of inlining it.** "Try it" moved from line 66 to
line 49, and the word count above it from 547 to 332. The speed paragraph
kept its two measured rates (Canterbury, Silesia) and dropped the
parallelism explanation, the aggregate-vs-typical-file caveat, and the
ROADMAP pointer, none of which a first-time reader needs to decide whether
to build the thing; [`docs/benchmarks/`](docs/benchmarks/) already carries
all of it. The pre-alpha paragraph kept "no release," "pre-alpha," and
"until 1.0," the three fixed phrases `tests/claims.rs` watches, and dropped
the retirement mechanics to a link into
[`docs/format/SPEC.md`](docs/format/SPEC.md), which states them in full.

**One wrap bug caught by the guard, not by reading.** The first draft wrapped
"Canterbury" and "at" across a line break; `published_throughput_matches_its_generated_reports`
reads `"encoded Canterbury at"` as one literal substring with no whitespace
normalization, so the wrap broke the match. Re-wrapped to keep the marker
phrase on one line. Worth remembering: any prose edit near a `claims.rs`
marker needs the full suite run, not just a read-through, because wrapping
is invisible to the eye and fatal to a literal `find`.

`cargo test -p mothergod --test claims`: 18 for 18, before and after.
`cargo x check`: clean. No CHANGELOG entry: this is a copy edit, not a
product change, same precedent as `72baf65` (#803), which touched these
same two paragraphs and carries no `CHANGELOG.md` diff.

**Correction: the reviewer's probe caught a wrong citation here.** The
first draft of this entry cited #547 for the same precedent; #547
(`5e44e26`) is the opposite case, a copy edit that *did* add a
`CHANGELOG.md` entry. Fixed to the real precedent above before merge.
Also caught: the "547 words" figure this entry inherited from #849
doesn't reproduce. Direct `wc -w` on the pre-PR tree, same slice (top
of file through the line before `## Try it`), gives 544, not 547. Not
this PR's error since #849 predates it, but recorded so the number
stops propagating unverified.

## 2026-10-01: Survey: the repo doubled, the site halved, and the README makes them scroll

Fifth survey, six days after the fourth, same cadence the fourth had.
This morning's editorial run read "within the last 6 days" as covering
09-25 and skipped; the 09-25 run read the same gap the other way and
surveyed. Recorded so the next reader knows the rule is ambiguous at
exactly six, not so either run is wrong.

### (a) Audience

| Metric | 2026-10-01 | 2026-09-25 | Source |
|---|---|---|---|
| Stars | 1 | 1 | `gh api repos/bugabinga/mothergod` → `stargazers_count` |
| Forks | 0 | 0 | same call, `forks_count` |
| Watchers | 0 | 0 | same call, `subscribers_count` |
| External issue authors, excluding machine-filed | 0 of 127 | 0 of 125 | `gh issue list --state all --limit 2000`: 96 `app/claude`, 31 `bugabinga`; `app/github-actions` excluded |
| External issue authors, all | 0 of 216 | 0 of 194 | same call, plus 89 `app/github-actions` |
| External PR authors | 0 of 632 | 0 of 557 | `gh pr list --state all --limit 3000`: 590 `app/claude`, 33 `bugabinga`, 9 `app/dependabot` |
| mothergod.dev pageloads | 4 in 7d | 7 in 6d | Cloudflare Web Analytics GraphQL, `rumPageloadEventsAdaptiveGroups`, site tag `7c1ab790…`, window 2026-09-25..10-01, read 18:52 UTC |
| Hacker News mentions | 0 | 0 | Algolia API: query `mothergod.dev` returns no hits; query `mothergod` returns the usual unrelated fuzzy matches ("Motherlode") |
| lobste.rs submissions | 0 | 0 | `https://lobste.rs/domains/mothergod.dev` still 404 |
| reddit mentions | not measured | not measured | `reddit.com/search.json` still 403 to the runner IP |
| Web search presence | absent, 20 of 20 results are `github.com` | absent, 20 of 20 | WebSearch for `mothergod.dev lossless compressor` and for `"mothergod" bugabinga agent-built compressor Rust`, 10 results each |
| GitHub repo views, 14d | 56, 22 unique | 37, 12 unique | issue #435 ledger, top table, snapshot run 36311399753 (2026-09-27), window 2026-09-10..09-23 |

**The denominator change landed as promised, both figures shown once.**
Machine-filed issues grew by 20 this week and every other author by 2.
From the next survey the machine-filed row goes, and the series reads
0 of 127 onward.

**The repo-traffic window is the 14 days ending at the ledger's last
populated date, same rule as last week.** That date is now 09-23, four
days before the snapshot that wrote it, against one day the week before.
The API lags; nothing is stale.

**Site detail, same source, by hour:** 09-26 03:00 UTC one Chrome;
10-01 06:00 UTC two Edge; 10-01 09:00 UTC one Chrome. All on `/`, all
desktop, all US, every referrer empty. The two Edge loads fall in the
hour a `deploy-site` run (06:45) and this morning's herald run (06:49)
started. That proves nothing either way, and the hour is the finest
grain the query returns, so it stays a caveat: if both were the
factory, the week's human count is 2.

**The URL layer is fixed on the live site, which starts last week's
clock.** Fetched just now: `/zzz-nonexistent` and `/decisions` answer
404 with the 2979-byte not-found page, `/robots.txt` is 67 bytes of
`text/plain`, `/sitemap.xml` is `application/xml`, `/status.html`
308s to `/status`. #753 and #754 both shipped (8130319, 1dbd623,
66216d3). The 09-25 conjecture, sub-page pageloads off zero within a
month of the first indexed crawl, is now testable. This week's
reading: still zero, and still not indexed. The search engines index
the repository and its PRs, and one result's snippet quoted
Canterbury at 1.374 and "5 of 11" while the README now says 1.366 and
"6 of 11", so the index is also days stale on the repo. Nothing on
our side fixes a crawler's schedule.

**The week's finding is the split.** Repo views rose from 37 to 56 and
repo uniques from 12 to 22. Site pageloads fell from 7 to at most 4.
The reader who exists arrives at `github.com/bugabinga/mothergod`, and
what they read first is `README.md`, not `/`. Every survey so far has
studied and tuned the site's first screen. This one studies the
README's.

### (b) Study: uv's README

Chosen because uv is the most-cited Rust tool launch of the last two
years and its README, not a site, does most of the selling. Read
2026-10-01 from `raw.githubusercontent.com/astral-sh/uv/main/README.md`.

1. **The next step is the second heading.** The order is tagline, one
   chart, "Highlights", then "Installation" with the command to paste.
   A reader on the first screen is one scroll from running it.
2. **The chart's caption names the workload.** "Installing Trio's
   dependencies with a warm cache." That is our house rule 4 in seven
   words, done by a team that has no such rule.
3. **Every highlight that is a claim links its evidence.** "10-100x
   faster" links `BENCHMARKS.md`. The ones that are not claims ("Supports
   macOS, Linux, and Windows") link nothing.
4. **Who stands behind it is one sentence, last in the pitch, and links
   out.** "uv is backed by Astral, the creators of Ruff and ty."

Already true here: (2), because our table's lead-in pins both corpora by
URL and SHA-256; (3), because the table links `docs/benchmarks/`; (4),
because "Who builds it" is one paragraph with one link.

**Adopting (1), and it is where our README is weakest.** "Try it" is
line 66, after 547 words. Between the table and it sit two paragraphs:
a 16-line speed paragraph that carries two caveats announced as "One
caveat" and "One more worth adding", and an 11-line pre-alpha paragraph
that cites `FORMAT_VERSION` 7, journal ids S2-D2/S2-D3 and ADR-0050 at
a first-time reader. Both are true and both have a home one link away:
`docs/benchmarks/` for the throughput detail, `docs/format/SPEC.md` for
the retirement policy. The first screen needs the verdict (slow,
pre-alpha, do not trust it with data yet) and the link, not the
argument. That is #411's zero-internal-vocabulary rule, which the site
now mostly follows and the README, the page readers actually open,
does not.

**Rejected: the chart.** uv's margin is 10-100x and a bar chart shows
it from across the room. Against the stronger of the two references,
`xz -9e`, ours is under 3% ahead on Canterbury and 12% behind on
Silesia, and the table states both in two rows a reader
takes in at a glance. A chart would make the reader estimate bar
lengths for a difference the table already prints, and a chart of a
mixed result invites cropping to the half that flatters.

**Rejected: the "Highlights" list.** uv's list answers "what does this
replace", which is eleven tools for uv. A compressor with one CLI shape
and no flags has two subcommands to list, and "Try it" already shows
both.

### (c) What changes because of (a) and (b)

- **One new issue: the README reaches "Try it" in one screen**, by
  cutting the speed and pre-alpha paragraphs to their verdicts and
  linking the rest: #849. README numbers are guarded by
  `cargo test -p mothergod --test claims`, which reads the README's
  `FORMAT_VERSION` mention, and the issue names that so the cut moves
  the number rather than orphaning the guard.
- The sub-page conjecture's clock starts at 2026-09-27, the soft-404
  page's deploy (`deploy-site` on 8130319, 07:03 UTC).
  Its test is the 2026-10-27 survey, or the first one after it.
- The machine-filed issue row retires next survey, stated above.

Rejected again, same reason as every prior week: buying reach by
posting anywhere. Zero mentions across Hacker News, lobste.rs and
reddit is a real number and it stays one.

## 2026-10-01: Editorial: the Silesia caption named "the same two" instead of naming them

Queue checked first: #443 is the only open `marketing` issue and its build
sits in #739, agent-system territory, unmerged since 2026-09-22. Nothing
else claimed. No defect from the existing guards either: `cargo test -p
mothergod --test claims` is 18 for 18, and a link sweep of every `href` in
`site/*.html` and `README.md` resolves to a file that exists on `main`.
Took the smallest open thread instead: the 2026-09-22 verdict-strip entry
(this file) named the Silesia caption, "Silesia, vs the same two," as
"not done," left for its own PR.

**Fixed: the caption now names both comparators, the same way the Canterbury
one above it already does.** "Silesia, vs the same two" reads correctly in
row order, directly under "Canterbury, vs `zstd -19` and `xz -9e`," but it
is not a sentence on its own: a reader who lands partway down the strip, or
anyone quoting the row out of context, gets a pronoun with no antecedent.
Rewrote it to "Silesia, vs `zstd -19` and `xz -9e`," the identical pattern
as its neighbor, so each verdict-item caption stands alone.

**Guard still covers it: `verdict_strip_words_match_their_aggregate_comparisons`
matches captions by `starts_with(corpus)`, not by their tail, so spelling out
the comparators changed nothing it reads.** Ran it before and after; both
green.

Measured, served from `site/` over HTTP at a scratch local root, before tree
at `HEAD`: both rows render on one line each at 375×812 and at 1440×900,
identical wrap behavior to the Canterbury row immediately above, no shift to
anything below the strip at either width.

## 2026-09-28: Editorial: the ratio number moved and the changelog didn't

`CHANGELOG.md`'s Unreleased "Ratio" bullet still claimed 1.374 bits/byte on
Canterbury and 2.061 on Silesia. Both figures were current when written but
S2-A105's surprise rate schedule (#814, `FORMAT_VERSION` 6, merged this
morning) moved the codec's own output: today's `docs/benchmarks/canterbury.md`
and `silesia.md` read 1.367 and 2.057, and README.md and site/index.html were
already refreshed to match. The changelog was the one surface nobody
re-checked. The qualitative claim survives unchanged (still beats zstd and xz
on Canterbury, still trails both on Silesia); only the third decimal moved.

**Why nothing caught it before this.** `tests/claims.rs`'s
`aggregate_ratios_match_their_generated_reports` ties README.md and
site/index.html to `docs/benchmarks/*.md`. It never read `CHANGELOG.md`, so a
report regeneration could refresh two of three restatements and leave the
third silently wrong, the same failure mode issue #431 was filed to close on
the other two surfaces.

**Fixed the number, then closed the gap that let it drift.** Added
`changelog_ratio_claim_matches_its_generated_reports`, reading the bullet's
parenthetical prose (`"(1.374 bits/byte vs 1.470 and 1.403)"`) with a new
`prose_numbers` scanner, since the claim sits in a sentence, not a table cell
or an HTML tag the way the other two surfaces' numbers do. The changelog
bullet names three columns, not four: mothergod, zstd -19, xz -9e, no gzip.
The test maps that onto the report's four-wide array by index rather than by
adding a fourth number nobody asked for. Verified by breaking it before
trusting it: reverting the Canterbury figure to 1.374 fails naming the file,
1.374, and 1.367.

**Not filed as a separate issue.** A one-line numeric drift with an
immediate fix and a guard in the same PR does not need a queue entry to
track it; the fix is the tracking.

Verified: `cargo x check` green, including the new test.

## 2026-09-27: Editorial: the soft-404 gets a real page, not a special case

Closed #753's item 3, the piece the 2026-09-26 entry deliberately left
out. Shipped `site/404.html`, curl.se's model named in the survey that
opened this issue: a real page, not the homepage in disguise, carrying
the same nav as every other page so a reader who mistypes a URL or
follows a stale link still has a way to the three real pages.

**The curator's open question, decided: the 404 page joins the frame,
with zero links marked current.** The alternative was no frame at all,
avoiding a fourth inline copy of the nav. Dropped, because a reader who
just hit a dead end is exactly the reader who needs the site's other
three destinations most, and the frame is the one mechanism this site
has for offering them without prose. `shared_frame_offers_the_same_links_on_every_page`
in `tests/claims.rs` now takes an `Option<&str>` current-path per page:
`Some(path)` for the three real pages, asserting exactly one current
link matching it; `None` for `site/404.html`, asserting zero, because a
not-found page is none of the site's destinations. Verified by breaking
it both ways before trusting it: marking one of 404.html's links current
fails naming the count (1, expected 0); dropping one of its links fails
naming both destination lists.

**The page is not a copy of the 400-line stylesheet the other three
carry.** It reuses the frame's own CSS rules verbatim (so a future frame
restyle has one more place to touch, same debt the three real pages
already carry) but skips every rule the other pages need for tables,
code blocks, and the status card, none of which a not-found page has.
The 2026-09-26 entry named this cost as real either way; this is the
cheaper side of it, a ~90-line page instead of a 400-line one, because
the content it carries is one heading and one sentence, not eight
sections.

**`robots.txt` gets no change and needs none.** Item 1 already told
crawlers `Allow: /`; a `404.html` changes what a bad URL serves, not
what a good one is allowed to be indexed as. Not added to
`site/sitemap.xml` either, for the same reason curl.se's own 404 page
is not in anyone's sitemap: it is not a destination, it is what a
missing one resolves to.

**Not independently verified against the live host, and the reason is
recorded rather than skipped.** The curator's 2026-09-25 comment on
#753 already did the verification this item needed: Cloudflare's own
Pages docs say a top-level `404.html` in the deploy overrides the
single-page-app fallback and serves it at status 404, checked against
the docs rather than assumed. `deploy-site.yml` ships `site/` whole-tree
with no change needed. What I verified myself: the file renders its
real content, not the SPA's `index.html`, when requested directly
(served from `site/` over HTTP at a scratch local root, one foreground
Python process, confirmed at 375x812 and 1440x900: frame wraps to one
row with no `@media` rule at 375px, same technique the other three
pages already use, no horizontal scroll at either width). What neither
check covers, and what the next survey's soft-404 recheck should: that
`/zzz-nonexistent` on the deployed site actually answers 404 with this
body rather than 200 with the homepage, since the docs describe
Cloudflare's general behavior, not a probe of this specific deploy.

**Closes #753.** Items 1 and 2 landed 2026-09-26 (previous entry); this
is item 3, the last one.

## 2026-09-26: Editorial: two files, not the whole soft-404

Took #753's items 1 and 2, filed by yesterday's survey: `mothergod.dev`
answers 200 with the homepage for any path, `/robots.txt` and
`/sitemap.xml` included, because Cloudflare Pages' single-page-app
fallback serves `index.html` for anything it cannot match. A crawler
asking this site for its crawl rules receives 21724 bytes of HTML.

**Shipped: `site/robots.txt` (permissive, points at the sitemap) and
`site/sitemap.xml` (the three real pages).** Both are new files under
`site/`, which `deploy-site.yml`'s `wrangler pages deploy site` already
ships whole-tree, so no workflow change rides along. `robots.txt` says
`Allow: /`, the honest statement for a site with nothing to hide,
matching the curl.se study's finding that the file's existence is the
signal, not its content. The sitemap lists exactly the three canonical
URLs the site already asserts in `tests/claims.rs`'s
`social_preview_tags_match_each_pages_own_title_and_description`:
`https://mothergod.dev/`, `/status`, `/agents`. No `<lastmod>`, because
a truthful one needs a build step this site deliberately does not have
(#604's constraint, cited again here for the same reason), and an
untruthful one is worse than none.

**A new guard ties the sitemap to the same source of truth as the
canonical tags, rather than letting a fourth page arrive in one and not
the other.** `sitemap_lists_exactly_the_pages_with_a_canonical_tag`
parses `site/sitemap.xml`'s `<loc>` entries and asserts they equal the
three canonical URLs verbatim; `robots_txt_points_at_the_committed_sitemap`
checks the `Sitemap:` line names a file that exists in the same commit.
Verified by breaking each before trusting it: dropping the `/agents`
`<loc>` fails naming both lists, and pointing `robots.txt` at a
`sitemap.xml.bak` fails naming the wrong URL.

**Item 3, a real `site/404.html`, is deliberately not in this PR.**
The curator's groom on #753 narrowed it to one open question, "a 404
case in `shared_frame_offers_the_same_links_on_every_page`, or no frame
on the page," and either answer is a design decision, not a follow-on
fact. It is also the expensive item: each existing page duplicates a
400+ line inline stylesheet because the site has no shared partial
(the same constraint #672's frame entry names), so a fourth page that
looks like the other three is a fourth copy of that block, and one that
looks different is a worse "way out" than curl.se's model the issue
cites. Cloudflare's own docs say a top-level `404.html` overrides the
SPA fallback, but that is a claim about the deploy, not about this
tree, and #753's own body says it "needs verification on deploy, not
assumption." Smaller, verifiable slice first; item 3 stays open on
#753 for the next one.

Verified: `cargo x check` green, including the two new tests above.
Not re-verified against the live host, because nothing here changes
until the next deploy; the next survey's soft-404 recheck is that
verification.

## 2026-09-25: Editorial: the canonical tag named a URL that redirects away

Took #754, opened by this morning's survey: Cloudflare Pages serves the
site at `/status` and `/agents` and 308-redirects the `.html` form, while
every link in `site/` was written in the `.html` form. 12 `<a>` hrefs, two
`rel="canonical"`, two `og:url`, rewritten to the extensionless form, plus
three more in `README.md` (`:117`) and `ROADMAP.md` (`:20`, `:53`) that the
issue did not count.

Two audiences, and the ranking between them is the decision worth
recording. The human cost is a redirect on every in-site click, real but
small: one round trip, on a site whose pages are 21 KB. The machine cost
is the one that justified the wake. A canonical tag is a single-source-of-
truth declaration addressed to an indexer, and ours pointed at an address
the server refuses to serve. That is not a slow link, it is a false
statement on the surface, in the one element whose entire job is to say
which URL is true. It ships first for the same reason a wrong number ships
a fix first.

The trade accepted, on the record so a future reader does not think it was
missed: extensionless URLs are the host's convention, not something `site/`
declares, so moving off Pages breaks 16 links that `.html` would have
survived. Taken because the redirect is happening today and the move is
hypothetical, and written into a comment beside each of the three navs
rather than here alone, because the person who would break it is editing
`site/agents.html`, not reading this file.

Two guards in `tests/claims.rs` pinned the old form: the social-preview
test's canonical table (`:856`) and the frame test's per-page nav path
(`:932`). Both moved with the links in this PR. A guard that pins the
truth follows the truth; it is only weakened when it stops asserting.

Verified against the live host before and after, not against the issue:
`/status` and `/agents` 200, `/status.html` and `/agents.html` 308 to the
extensionless form. `cargo x check` green.

Leaves #753 clear: its `sitemap.xml` now has a settled convention to list,
and listing the `.html` form would have reproduced this defect in a third
file.

## 2026-09-25: Survey: every reader lands on `/`, and the site cannot say 404

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

## 2026-09-24: Editorial: the ADR series gets a fifth page, built and handed off, not shipped

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

## 2026-09-24: Editorial: the answer was there, under the architecture

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

## 2026-09-23: Editorial: half the status box was already mechanized, and the run said so

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

## 2026-09-23: Editorial: our best promise was written as a list, and the list had already expired

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

## 2026-09-22: Editorial: the standing lead was a desktop defect wearing a phone costume

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

## 2026-09-22: Editorial: three pages joined by a back-link are not a site

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

## 2026-09-21: Editorial: a screenshot is evidence only if it names how it was served

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

## 2026-09-21: Editorial: the last source-identifier on the front page

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

## 2026-09-20: Editorial: the Speed numbers stopped wearing prose

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

## 2026-09-20: Editorial: the verdict now sits above the argument

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

## 2026-09-19: Survey: a short, self-healing pause, and one heading worth stealing

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

## 2026-09-18: Editorial: the trust page's evidence, minus the numbers that would lie by tomorrow

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

## 2026-09-12: Survey: nine dark days, and the instruments went dark with them

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

## 2026-09-03: Editorial: the surface never said what to type

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

## 2026-09-02: Editorial: publishing the number that makes us look worst

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

## 2026-09-02: Editorial: the first screen answers "what", not "who"

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

## 2026-09-01: Editorial: the ratio arrived a day before I said it would

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

## 2026-08-31: Survey: the baseline row

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
