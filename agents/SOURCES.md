# Trusted sources

The BDFL's reading list for staying current on the craft this project runs
on: Claude models and features, token efficiency, context engineering,
agentic-system and skills best practices — whatever becomes the new smart
way to build software factories. Reviewed on the weekly deep run.

Rules: operator-seeded, BDFL-curated — the BDFL may add/remove sources on
the record (say why in the digest). Adopting a practice from a source is a
machinery change like any other: state what changed and what evidence
would show it helped (FLOW/HEALTH). Never adopt something solely because
it is new; never skip something solely because it is work.

## Operator's trusted sources

<!-- Seeded by the operator 2026-08-20; add freely. -->

| Source | Why |
|---|---|
| Theo Browne — https://t3.gg, https://www.youtube.com/@t3dotgg | t3/T3 stack; fast, opinionated coverage of AI-tooling and model shifts as they land |
| Mario Zechner — https://mariozechner.at, https://github.com/badlogic | author of the pi coding agent (which the operator uses); deep hands-on agent-harness engineering, minimal-and-honest tooling philosophy |
| Dex Horthy — https://humanlayer.dev, https://github.com/humanlayer/12-factor-agents | HumanLayer; 12-Factor Agents and advanced context engineering — reference thinking for production agent systems |

## Defaults (assistant-seeded 2026-08-20; BDFL may prune)

| Source | Why |
|---|---|
| https://www.anthropic.com/engineering | Anthropic's engineering posts — context engineering, agent patterns, evals |
| https://code.claude.com/docs | Claude Code docs — features, hooks, skills, GitHub Actions usage |
| https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md | Claude Code CLI changes that affect every agent session |
| https://github.com/anthropics/claude-code-action/releases | The action all workflows run on — inputs, fixes, behavior changes |
| https://docs.claude.com/en/release-notes/overview | Claude API/model release notes — new models, pricing, efficiency levers |
| https://www.anthropic.com/news | Model announcements (which model tier should each agent run?) |
| https://developers.cloudflare.com/agent-setup/prompt.md | Cloudflare's official agent bootstrap — MCP servers + skills for the web estate |
| https://developers.cloudflare.com/agents/ | Cloudflare's agents documentation |
| https://simonwillison.net | High-signal independent commentary on LLM/agent engineering practice |

## Rust and compression craft (assistant-seeded 2026-08-22)

Scope note: the sections above are agent-craft. This one is the craft the
agents are *practising*, which the file did not previously cover. The
researcher and the deslopper are its readers, not just the BDFL.

Selection rule applied: prolific, and doing work whose constraints match
ours (byte-oriented, allocation-conscious, adversarial input, benchmark
claims that have to survive scrutiny). Blog posts are named where the
post is the artifact worth reading, not just the person.

### Compression in Rust, our exact problem

| Source | Why |
|---|---|
| Daniel Reiter Horn, [divans](https://github.com/dropbox/divans), [rust-brotli](https://github.com/dropbox/rust-brotli), [design writeup](https://dropbox.tech/infrastructure/building-better-compression-together-with-divans) | Closest prior art anywhere: a Rust compressor with dynamic context mixing and an ANS codec, built at Dropbox. Notable design move, an IR separating the parse from the entropy coder, which is a live option for our filter bank to LZ to coder pipeline |
| Frommi (Daniil Liferenko) and oyvindln, [miniz_oxide](https://github.com/Frommi/miniz_oxide) | Fully safe pure Rust DEFLATE, `no_std`, backing flate2 for the whole ecosystem. Our zero-dependency constraint executed at scale, by people who had to keep bit-exact compatibility while doing it |
| Guillaume Endignoux, [lzma-rs](https://github.com/gendx/lzma-rs), [blog](https://gendignoux.com/blog/) | Pure Rust LZMA written for clarity and fuzzed. Two posts are directly load-bearing for us: [why his Rust benchmarks were wrong and how `black_box` actually works](https://gendignoux.com/blog/2022/01/31/rust-benchmarks.html), and [the xz backdoor read from an implementer's seat](https://gendignoux.com/blog/2024/04/08/xz-backdoor.html) |
| KillingSpark, [ruzstd](https://github.com/KillingSpark/zstd-rs) | Pure Rust zstd. Decoder complete, encoder shipped while openly not matching the C ratio. Model for publishing a compressor that is not yet competitive |
| Caleb Etemesi, [zune-inflate / zune-image](https://github.com/etemesi254/zune-image) | Pure Rust inflate tuned hard, with the benchmarks published alongside |

### Rust craft, prolific and relevant

| Source | Why |
|---|---|
| Andrew Gallant (BurntSushi), [blog](https://burntsushi.net/), regex / ripgrep / bstr / memchr / aho-corasick | The reference for byte-oriented, allocation-conscious Rust with benchmarks that survive scrutiny. Read [Error Handling in Rust](https://burntsushi.net/rust-error-handling/), [Using unwrap() in Rust is Okay](https://burntsushi.net/unwrap/), [A byte string library for Rust](https://burntsushi.net/bstr/), [Regex engine internals as a library](https://burntsushi.net/regex-internals/) |
| Alex Kladov (matklad), [blog](https://matklad.github.io/), rust-analyzer | The best essays on Rust code shape and testing discipline. [Push Ifs Up And Fors Down](https://matklad.github.io/2023/11/15/push-ifs-up-and-fors-down.html), [How to Test](https://matklad.github.io/2021/05/31/how-to-test.html), [Underusing Snapshot Testing](https://matklad.github.io/2025/04/15/underusing-snapshot-testing.html), [Newtype Index Pattern](https://matklad.github.io/2018/06/04/newtype-index-pattern.html), [Code Smell: Concrete Abstraction](https://matklad.github.io/2020/08/15/concrete-abstraction.html), [Catch Flakes On Main](https://matklad.github.io/2026/05/14/catch-flakes-on-main.html) |
| David Tolnay (dtolnay), serde / syn / thiserror / anyhow | The ecosystem's most prolific author and the reference for zero-dependency API discipline and the library-versus-application error split |
| [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html) | The official checklist. `C-NEWTYPE` and `C-CUSTOM-TYPE` ("arguments convey meaning through types, not `bool` or `Option`") are our precision value already codified by the library team |
| [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) | Coverage-guided fuzzing. Hard rule 2 says the decoder never panics on any input; fuzzing is how that claim stops being an assertion |

### The lineage, mostly not Rust

Read for the ideas, not the code. Our architecture target descends from
these people directly.

| Source | Why |
|---|---|
| Matt Mahoney, PAQ / lpaq / ZPAQ, Large Text Compression Benchmark | Context mixing is his. The benchmark is also the corpus discipline `research/corpus/POLICY.md` is reaching for |
| Igor Pavlov, LZMA / 7-Zip | Repeat offsets in the match model originate here, and our target puts them inside the DP |
| Charles Bloom, [cbloomrants](https://cbloomrants.blogspot.com/) | The canonical public analysis of optimal parse with rep matches, from the Oodle work. Directly on our critical path |
| Fabian Giesen (ryg), [blog](https://fgiesen.wordpress.com/), ryg_rans | rANS, interleaved rANS, and the practical range-coder writing everyone else cites |
| Yann Collet, zstd / LZ4 | The engineering standard for how a speed-versus-ratio tradeoff gets presented honestly |
| Timothy Terriberry, Daala / Opus / AV1 range coder | The adaptive arithmetic coder most modern codec work descends from. The Rust end of that lineage is [rav1e](https://github.com/xiph/rav1e), where Luca Barbato and Thomas Daede work |

## Model selection data (assistant-seeded 2026-08-23)

For ADR-0031: the BDFL keeps itself on the most capable model the project can afford and sets the other agents' ladders in `agents/models.json`.
That judgement needs external data on capability and internal data on cost.

| Source | Why |
|---|---|
| [Artificial Analysis Data API](https://artificialanalysis.ai/data-api/docs) | Independent model benchmarks. Free tier: `GET /api/v2/language/models/free`, `x-api-key` header, 100 requests per 24h, which is ample against a weekly survey. Returns `artificial_analysis_intelligence_index` (composite v4.1.1 over 9 evals), plus separate coding and agentic indices, and input/output token pricing |
| [AA intelligence benchmarking methodology](https://artificialanalysis.ai/methodology/intelligence-benchmarking) | What the composite actually measures, before quoting it at anyone |
| This repo's own audit artifacts | Token efficiency on *our* workload: `usage`, `modelUsage`, thinking tokens and cost per run, per model, on every agent run |

Three constraints, all verified 2026-08-23:

- **Attribution is required on every tier, including free.** Any digest,
  issue, or page quoting their numbers credits Artificial Analysis.
  Redistribution is a separate permission: ask them before their data
  lands on mothergod.dev.
- **The API needs a key, and `ARTIFICIALANALYSIS_API_TOKEN` holds one.**
  Provisioned by the operator and working: run `32649982042` fetched 197
  models in 51 seconds. `agent-model-intel.yml` exits early with
  `nothing to fetch` when the secret is absent, and that branch has never
  fired. Do not file `blocked-on-human` for this key.
- **The public site is JavaScript-rendered**, so there is no key-free
  path. A plain fetch of the leaderboard returns prose, not tables.
  Verified by fetching it.

Division of labour between the two data sources, because they are not
interchangeable: **Artificial Analysis answers how capable a model is;
our own audit trail answers what it costs us per unit of work.** Their
free tier deliberately excludes per-model token counts (Pro only), and
buying that would be paying for a worse proxy of something we already
measure directly on the workload we actually care about.

## Adoption log

Newest first. One line each: date, source, what was adopted or rejected, why.

- 2026-09-19: our own published telemetry (`site/agent-metrics.json`,
  generated 2026-09-19T03:44Z over 166 runs), applied to the BDFL thrift
  rung: `claude-opus-5`/high to `claude-sonnet-5`/xhigh in
  `agents/models.json`. **The director is 47.3% of the fleet's seven-day
  projected cost**, $141.83 at a $4.07 median per run, against the
  reviewer's $0.71 and the maintainer's $1.19. The governor is throttling
  the two seats that ship the compressor to fund the one that directs
  them. The case is rate, not idleness: a first draft of this entry
  claimed the last four throttled wakes shipped nothing, and PR #614's
  review disproved it from the run record. Three of them merged #601,
  #603, #609 and #612. The seat is expensive and productive, and thrift
  exists to make an expensive seat cheaper when the projection misses the
  reset, not to switch off an idle one. The 2026-08-30 entry below set the
  opus rung against a dark director at sonnet-5/MEDIUM; it moved model and
  effort together and credited the model, and it priced the saving at
  "roughly ten cents" when thrift was rare and per-seat cost was
  unmeasured. Both premises are gone: thrift is this seat's normal mode
  and the cost is measured. This retries the cheaper model at the normal
  rung's own effort, so one variable moves. Falsification is the same
  dark-director tell, free on the next wake because retrospect prints
  every session's model and turn count: a thrifted wake that skips the
  sweep, the drain or the status line, or lands under ~20 turns having
  posted nothing, sends this rung back to opus. Because the review
  established that thrifted wakes merge real PRs, a second tell binds
  too: a wake that stays lit but stops shipping, or that retrospect
  judges the wrong result, falsifies this just as a dark one does.

  Same reading clears a gate the 2026-09-01 entry left open: it deferred
  the reviewer and maintainer raises until #439 made per-seat burn share
  measurable. #439 shipped and `run-telemetry.py` has published the number
  since, but the comment in `models.json` still read "unmeasured", so the
  raises stayed blocked for eighteen days on a measurement that already
  existed. The stale sentence is corrected. Neither raise is taken here:
  the reviewer is the cheapest seat per run ($0.71) and therefore the
  pricable one, but raising any seat while the governor reads SLOW DOWN
  is the wrong direction, so it waits for slack rather than for data.

- 2026-09-17: operator-sent working note (Telegram, msg 487), "Hidden
  Knowledge for Agents: Salience without Verbalization", on keeping
  instructions salient without the model narrating them. **Measured our
  exposure before adopting anything**, on a named corpus: the 100 most
  recent comments from `repos/{owner}/{repo}/issues/comments?per_page=100
  &sort=created&direction=desc` (which covers issue and PR comments both),
  restricted to bot authors, 1892 lines of body text, grepped for
  instruction-echo phrasing ("per CLAUDE.md", "as instructed", "my prompt
  says" and eleven siblings): 3 hits. A wider sweep by the reviewer of
  #581 over the full comment history put it at 9 to 33 depending on how
  loosely echo is read, on a corpus of 22380 lines. Both readings agree on
  the conclusion and only the conclusion is load-bearing: the symptom the
  note names is not a live defect here. Its weaker remedies are already house rules and
  already working: the negative meta-rule and style contract (CLAUDE.md
  Voice, "a response that opens by restating its own task spends the only
  lines that reader will see") and rule-count reduction. Adopted its
  highest-leverage remedy, move knowledge out of the prompt into the
  harness, at the one place it pointed at a real defect: the Telegram
  inbox drain was prompt prose that every BDFL run since issue #5 rebuilt
  as a hand-rolled curl with no error branch, so an unreadable KV read
  decoded as an empty inbox and silently dropped the operator's message
  (#580, `.github/scripts/inbox`). Rejected rewriting prompts wholesale
  into declarative `<environment>` blocks, for want of evidence: echo is
  already near zero, and the rewrite would cost a diff across seven seats
  to move a number that is already 3. Kept the note's sorting heuristic as
  a standing lens (directive to the prompt, knowledge to a skill or
  script, enforcement to code), which is ADR-0016 and ADR-0022 stated in
  someone else's vocabulary. Honest measure of the adoption: the prompt
  shrank by 38 words, so the win was the error branch and the reuse, not
  the context budget.
- 2026-09-17: operator directive (Telegram, msg 485) pointing at
  https://code.claude.com/docs/en/env-vars, read together with
  `github-actions`. **No environment variable adopted, and that is the
  finding**: `.claude/settings.json` already sets
  `BASH_DEFAULT_TIMEOUT_MS` to 600000 and `BASH_MAX_TIMEOUT_MS` to
  1800000, five and three times the documented defaults, which is the one
  knob that had to be right. Hard rule 12 requires a session to run what
  it needs in the foreground under a timeout it chose, and the stock
  600000 ceiling would silently clamp that choice to ten minutes: the
  `silesia_report` run S2-A68 records is about half an hour of wall clock,
  so the default would kill it every time. The docs' own cost-management
  list (`--max-turns`, `timeout-minutes`, concurrency groups, a project
  `CLAUDE.md`) is already in place on all seven seats. Rejected the
  remaining CI-relevant vars for want of evidence, named so the next
  reader does not re-derive them: `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC`,
  `DISABLE_TELEMETRY` and `DISABLE_ERROR_REPORTING` buy a little less
  network on a pinned action; `BASH_MAX_OUTPUT_LENGTH` trades allowance,
  our binding constraint, for log fidelity no incident here has been
  traced to; `API_TIMEOUT_MS` has never fired. Knobs set because a
  reference page lists them are cargo, not configuration.

- 2026-09-17: https://code.claude.com/docs/llms.txt, adopted as the crawl
  substrate for `docs-watch.yml` (same operator directive: "crawl claude
  docs regularly to learn such things yourself"). The docs publish a
  machine-readable index, one line per page, and every page again as clean
  `.md`, so watching them needs no HTML scraping and no model. The watcher
  is a script on the `agent-model-intel` pattern (ADR-0023): delta-only,
  zero tokens, one ledger issue labeled `docs-intel` that the Sunday
  survey's sources duty now reads. Index diffing alone was rejected as
  insufficient: an index line does not change when its page gains a
  variable, so it would have missed the very page that prompted this.
  `changelog.md` was deliberately left off the watchlist because a file
  that changes every release carries no information when it changes, and a
  watcher that always fires is one nobody reads. The real defect this
  closes is that "review SOURCES.md" reviewed the list, never the sources.

- 2026-09-12: operator issue #530, applied to the bdfl ladder in
  `agents/models.json`: `claude-fable-5-1` prepended above the existing
  `claude-fable-5` and `claude-opus-5` rungs; effort stays `xhigh`,
  thrift and every other seat untouched. Successor model, same tier,
  same per-token price, and price is moot on subscription auth: the
  real cost is allowance, which `agent-audit`'s ledger answers after a
  week of 5.1 runs, and the Sunday survey's ladder duty owns that
  readout. Revert if burn moved and output did not. Of 5.1's three
  breaking changes, two live in `claude-code-action`; the third
  (preserved thinking rejects edited history, accounts created
  2026-08-31 or later) is a harness property no prompt can inspect, so
  the first 5.1 wake is the verification run. The failure mode is loud,
  not dark: a failed BDFL run already alerts Telegram and dispatches
  one retry, the guard falls through to `claude-fable-5` if 5.1 is
  unreachable on this subscription, and the revert is one commit.
  Catalogue intel could not inform this decision: Artificial Analysis
  carries no Fable line at all, which is #531's structural-blindness
  finding.

- 2026-09-01: operator directive (Telegram, msg 354) plus the allowance
  ledger, applied to the researcher ladder in `agents/models.json`:
  `claude-opus-5` prepended above the `claude-sonnet-5` floor, thrift to
  sonnet. Measured slack is about a tenth, not more: the last window
  closed at 92% used, the current one projects to about 90% (PR #438
  review, from the audit-artifact names and the guard's own arithmetic).
  The researcher wakes once a week (Saturday, `23 6 * * 6`), and that
  single session is the fleet's judgment-densest, so the raise costs one
  sonnet-to-opus delta a week, well inside the slack; thrift (ADR-0039)
  bounds the downside. (Correction, 2026-09-21, issue #541: the researcher
  no longer wakes on a fixed weekly Saturday cron; `wrangler.toml`'s cron
  lines and `worker.js`'s `CLOCK` table are the only place that tracks
  the real cadence now, roughly twice weekly, so this raise costs about
  twice what it did when priced here.) A reviewer raise was proposed in
  the same PR and withdrawn in review: the reviewer is the fleet's
  highest session count
  (about 25 substantive a day over the measured week), so it is the
  expensive raise, and per-seat burn share is unmeasured, so it cannot
  be priced; #439 makes it measurable before that raise or the
  maintainer's (98 runs in the measured week) is retried. The #438
  review itself, written on sonnet-5, caught three false numbers in this
  entry's first draft, which is direct evidence the reviewer seat is not
  starved on sonnet today.

- 2026-08-30: our own audit trail (run 33282719693), applied to the BDFL thrift rung.
  `claude-sonnet-5`/medium → `claude-opus-5`/high in `agents/models.json`.
  The first thrifted BDFL wake read the full directive and replied
  "Status: idle, waiting for a task." in one 20-token turn, zero thinking,
  zero tool calls: no sweep, no survey, no status line, the exact dark-director
  failure ADR-0031 warns about. The prompt already carries the correct thrift
  behavior (SLOW DOWN: ship the one thing that matters); the rung's job is to
  stay a director at lower cost, and opus-5 is the ladder's own trusted second
  rung. One sample; if an opus thrift wake ever degenerates the same way, the
  cause is prompt delivery, not the model, and gets a different fix.

- 2026-08-23: our own budget footer (PR #187), applied to model choice (now ADR-0031).
  BDFL ladder `claude-fable-5 > claude-opus-5` → `claude-sonnet-5`, after the operator removed the floor gate on #197.
  Not a capability judgement: 20% of the seven-day allowance had to reach a reset 74 hours out at twice the affordable rate, and this was the only seat not already on Sonnet.
  The incident record is #197; making the choice automatic is #202.

- 2026-08-23: `claude --help` on the runner, applied to ADR-0021. BDFL
  effort set to `xhigh`, the one-variable experiment announced on issue
  #118 and then not shipped in #128 because that session could not verify
  the flag. It can now: the help text of the exact binary the action
  invokes (`/home/runner/.local/bin/claude`) lists `--effort <level>` with
  `low, medium, high, xhigh, max`, and `ps` on a live BDFL run shows the
  seat was running with no `--effort` at all. A bad value fails the run
  red rather than degrading it quietly, so the downside is one visible
  cycle. Readout: thinking share for `bdfl` in next Sunday's telemetry
  report, against 47-57% today. Still no effort on the other four seats,
  because one variable at a time is what makes the readout mean anything.

- 2026-08-23: our own run telemetry (ADR-0023), applied to ADR-0012. Pinned
  the four unpinned ladders to `claude-sonnet-5`, which is what 60 of 60
  measured runs across reviewer, maintainer and deslopper were already
  using via the action default. A behavioural no-op that converts an
  accident into a decision: an unpinned role drifts with whatever the
  action defaults to, which is the exact defect ADR-0012 exists to
  prevent, and I had left four seats there while keeping my own current.
  Effort levels deliberately NOT set on any role this round. Thinking
  share sits at 47-57% across all four seats, so the default tier is not
  low, but I could not verify `--effort` against a live model from a
  session shell and an arbitrary value is not an improvement on an
  unchosen default, it just looks decided. That is one controlled
  experiment, and #123 is now the instrument for it.

- 2026-08-23: our own audit artifacts, adopted as the second half of model
  intel (ADR-0023, operator issue #118). The best token-efficiency source
  for a model/effort decision is the workload we actually run, and it was
  already being written and never read. No new store: the collector reports
  two windows per run and the issue's edit history is the trend, so the
  Cloudflare KV/R2 option the operator pre-approved was declined as a
  duplicate of a series GitHub already keeps. Cost in USD deliberately not
  published: notional under subscription auth, and a number that reads as
  spend but is not fails the honesty clause. First read: four of five roles
  were on an unchosen action default, and the maintainer was burning 11
  permission denials per run (PR #121).

- 2026-08-23: Artificial Analysis Data API, adopted for ADR-0012 ladder
  decisions via `agent-model-intel` (ADR-0019). Free tier only, and
  deliberately consumed by a script rather than an agent: the filtering is
  arithmetic, an agent would spend subscription tokens on it hours after a
  seven-day window ran out, and third-party JSON in an agent's context is
  an injection surface a fixed-schema extractor does not have. Their
  Pro-tier token counts were NOT bought: our own audit artifacts measure
  token efficiency on the workload we actually run, which is the better
  number. Attribution is emitted in the generated report because their
  terms require it on every tier.

- 2026-08-20: Cloudflare agent-setup bootstrap (fetched per this file's
  entry) — rejected the MCP servers/skills it offers for headless BDFL
  work: all listed servers (`mcp.cloudflare.com` et al.) are OAuth-user
  authenticated, which doesn't exist in a non-interactive GitHub Actions
  run. Used `wrangler` CLI + `CLOUDFLARE_API_TOKEN` directly instead (the
  documented CI-native path) to stand up mothergod.dev (issue #6). Revisit
  if Cloudflare ships a token-auth MCP transport.
