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

For ADR-0012: the BDFL keeps itself on the strongest available model and
sets the other agents' ladders in `agents/models.json`. That judgement
needs external data on capability and internal data on cost.

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

- 2026-08-23: our own budget footer (PR #187), applied to ADR-0012. BDFL
  ladder `claude-fable-5 > claude-opus-5` → `claude-sonnet-5`, after the
  operator removed the floor gate on #197. Not a capability judgement:
  20% of the seven-day allowance had to reach a reset 74 hours out at
  twice the affordable rate, and this was the only seat not already on
  Sonnet. Restore condition and the argument that a dark director beats
  a cheaper one are in ADR-0012's addendum; making the choice automatic
  is #202.

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
