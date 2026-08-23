# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/) once released.

## [Unreleased]

### Added

- Benchmark harness, first slice (`research/JOURNAL.md` S2-A1): a new
  `bench/` workspace crate with the two mandatory corpus generators from
  `research/corpus/POLICY.md` ported to Rust — `entropy_ladder` (iid bytes
  at a chosen order-0 entropy) and `markov_h8_2_trap` (uniform histogram,
  low conditional entropy). Core `mothergod` crate stays zero-dependency.
- Codec port, first slice (`research/JOURNAL.md` S2-A2): `filters` module
  with a fixed-stride delta filter, ported from the founding session's
  archived codec. Not yet wired to a compression `Method` — the LZ, model,
  and coder modules it will sit behind are still to come.
- Codec port, second filter slice (`research/JOURNAL.md` S2-A3): a
  row-major-to-column-major transpose filter, ported from the founding
  session's archived codec. `filters` is now submodules (`delta`,
  `transpose`) to keep each filter's `encode`/`decode` pair namespaced.
- Codec port, third filter slice (`research/JOURNAL.md` S2-A4): the x86
  call/jmp (BCJ) filter, ported from the founding session's archived
  codec as a `bcj` submodule of `filters`.
- Codec port, fourth filter slice (`research/JOURNAL.md` S2-A5): the
  base64-unwrap filter, ported from the founding session's archived
  codec as a `base64_unwrap` submodule of `filters`. Unlike the earlier
  filters, unwrapping is a data-dependent decision rather than a
  caller-supplied parameter, so `encode` prefixes a one-byte flag that
  `decode` reads back; supported by a new zero-dependency standard
  base64 encode/strict-decode pair (no base64 crate, per ADR-0002).
- Codec port, fifth filter slice (`research/JOURNAL.md` S2-A6): a
  byte-order reversal filter, ported from the founding session's
  archived codec as a `reverse` submodule of `filters`. Self-inverse
  (`encode` and `decode` are the same operation), covering M1's
  filter-bank checklist in full: `pick_filters`, the LZ, model, and
  coder modules remain.
- Codec port, trial-selection slice (`research/JOURNAL.md` S2-A7): a
  `filters::select` submodule with a `pick` function that shortlists
  which filters (delta, BCJ, transpose) are worth a full trial encode
  against a given input, using an order-1 entropy proxy on a bounded
  probe. Ported from the archive's `pick_filters`. Not yet called by
  anything — the LZ, model, and coder modules it will feed remain.
- Deslopper agent (ADR-0016): a fifth agent seat that removes slop from
  `src/` twice daily without changing observable behaviour, one scope per
  PR, approved by the reviewer like any other agent PR. Its taxonomy and
  scope rule ship as a Claude Code skill at `.claude/skills/deslop/`.
- Real-time operator wake (issue #5): Telegram messages hit a Cloudflare
  Worker at `bot.mothergod.dev` that stores them in KV and dispatches
  the BDFL within seconds, replacing the per-run `getUpdates` poll.
- Codec port, LZ slice one (`research/JOURNAL.md` S2-A8): a new `lz`
  module with the greedy/lazy parser (`Token`, `parse_greedy`), ported
  from the archive's `lz` function, plus `replay`, the token-stream
  inverse that proves it losslessly reversible ahead of the entropy
  coder that will eventually consume it. The archive's DP-priced
  optimal parse (`lz_opt`) is a follow-up slice; it runs this parser
  internally as its price-seeding first pass.
- Codec port, LZ slice two (`research/JOURNAL.md` S2-A9): `lz::parse_optimal`,
  the archive's DP-priced optimal parse, seeded by `parse_greedy` and
  costed against a lightweight frequency-table price model (no real
  entropy coder exists yet to price against). One deliberate correctness
  fix over the archive: this parse's internal repeat-offset-cache
  bookkeeping always matches `replay`'s, closing a round-trip hazard
  present in the archive's own DP (see the journal entry for the
  mechanism).
- Codec port, first coder slice (`research/JOURNAL.md` S2-A10): a new
  `coder` module with `Encoder`/`Decoder`, the adaptive range coder
  ported from the archive's `Enc`/`Dec`. Driven directly by
  caller-supplied cumulative-frequency ranges; the adaptive frequency
  tables (the archive's `Model` and the six-expert `Lit` mixer) that
  will supply those ranges are the next slice.

### Removed

- The interactive `@claude` mention agent (operator directive). Mentions
  no longer trigger anything; open an issue instead, the heartbeat
  triages and answers daily.

### Changed

- CI gate (operator directive): the four cargo jobs (fmt, clippy, test,
  doc) skip on pull requests that change no cargo input, decided by one
  `changes` job filtering on tool-input file types rather than tree
  paths; pushes to `main` always run everything.
- Pause detector (ADR-0004, amended): the usage-limit marker list gains
  the "session limit" dialect after run 32588022230 slipped through
  unpaused, and RESUME-AT now honors a UTC reset time advertised in the
  error message, falling back to the blanket +6h/+24h rule when none is
  present.
- Two-realm repository layout (ADR-0010): agent-system files moved to
  `agents/` (governance, operations, personas, sources, identities),
  strictly separated from the classical project tree.

### Fixed

- The reviewer agent approved PRs but sometimes skipped merging them
  (issue #21), leaving the operator to merge by hand (PRs #15, #19). Root
  cause: the PASS procedure told the reviewer to run
  `gh pr checks <n> --watch --fail-fast` before merging, but that check
  list includes the reviewer's own still-running job, which cannot
  complete while being watched from inside itself; confirmed against the
  repo's ruleset, which requires only `test`/`doc`/`clippy`/`fmt`, not
  `review`, so watching the reviewer's own check was never necessary.
  The reviewer now merges unconditionally as its last action
  (`gh pr merge <n> --squash --auto`, falling back to a plain squash
  merge) instead of watching first.

- The reviewer wrongly labeled PR #22 `blocked-on-human`, believing an
  unsigned branch commit would fail the `required_signatures` ruleset on
  `main` (issue #24). It does not: squash merge creates a new commit
  server-side, signed by GitHub, independent of the source branch's own
  signature status. The fact now lives in `agents/GOVERNANCE.md`
  ("Merging"), which every reviewing agent reads, and the reviewer's merge
  step spells out the specific belief to reject. Follow-up, found while
  landing this fix on PR #25: `gh pr merge` runs its own client-side
  mergeable-state check and can refuse a squash the REST API accepts
  immediately; the reviewer's merge step now falls back to `gh api -X PUT
  .../pulls/<n>/merge` when `gh pr merge` refuses citing branch policy.

- The usage-limit pause detector false-positived on the system's own
  documentation (issue #11): it grepped the whole session transcript, and
  this repo's prompts and docs legitimately contain phrases like "usage
  limit" and "weekly limit" because they describe the pause machinery
  itself. A max-turns failure of the BDFL's first run was thereby
  misclassified as a weekly usage limit, pausing all agents for 24 hours.
  The detector now inspects only structured error result objects, so a
  successful run can never pause the system and only a genuine limit error
  triggers. Turn and time budgets raised generously across all agents
  (BDFL 120→500 turns, heartbeat 130→400, researcher 150→500, reviewer
  100→300, interactive 60→200) per the operator's directive: tight limits
  poison good runs, smart limits need experience first.
- `agent-review` refused to run on any PR authored by our own `claude[bot]`
  identity (BDFL or heartbeat PRs) — `claude-code-action`'s default
  bot-actor guard blocked it before it read a single file. Would have
  silently broken review→automerge for every agent-authored PR the moment
  heartbeat opened one. Scoped `allowed_bots: "claude"` to fix; fork PRs
  stay excluded so no external bot gains anything.
- `agent-review` also could not merge what it just verified when the PR
  author is `claude[bot]`: GitHub refuses self-approval at the platform
  level (`gh pr review --approve` → "Can not approve your own pull
  request"), independent of branch protection. Since this repo's ruleset
  requires zero approving reviews to merge, an approval was never actually
  load-bearing — the reviewer's prompt now posts its verification as a
  plain comment instead when self-approval fails, then proceeds to label
  and merge as normal.
- The `main` branch ruleset carried four rules (`copilot_code_review`,
  `code_coverage`, `code_quality`, `code_scanning`) for tools this repo
  never runs, plus `require_extra_approval_for_unattributed_changes`,
  which the reviewer can never satisfy for its own agent-authored PRs
  (GitHub refuses self-approval). Both fail silently as
  `mergeStateStatus: BLOCKED` with no indication which rule is at fault.
  Every merge to `main` since the repo's creation had therefore fallen
  back to the operator merging by hand via admin bypass — the autonomous
  reviewer/heartbeat merge pipeline (ADR-0003) had never actually run
  end to end. Removed the unsatisfiable rules and disabled the
  extra-approval flag; verified by merging PR #2 (a routine dependabot
  bump) with a plain agent token — first fully autonomous merge to
  `main`.

### Added

- Project website at [mothergod.dev](https://mothergod.dev): a minimal,
  honest landing page (`site/`) stating pre-alpha status, linking the
  roadmap, research journal, and governance docs, deployed to Cloudflare
  Pages by `deploy-site.yml` on every push touching `site/`.
- Project mark (`assets/logo.svg`, halo variant) in README and rustdoc;
  brand sheet at `assets/mark.html` (anatomy, palette, scale test, source).
- Telegram status bot integration: automatic pause alerts from every agent
  workflow, dire escalations and weekly digest from the BDFL, and an
  operator inbox read at each BDFL wake-up; chat id self-bootstraps on the
  operator's first message to the bot.

- Crate skeleton with v0 framed container format (magic, version, method
  byte); `Stored` method only.
- Quality-gate CI (fmt, clippy, tests, docs).
- Agent-run development system: daily maintainer heartbeat, adversarial PR
  reviewer with autonomous merge, weekly researcher, interactive `@claude`,
  usage-limit pause mechanism.
- Governance, contributing, security, roadmap, and research-journal
  documentation; journal seeded with the founding session's findings.
- Founding-session artifacts archived verbatim in
  `research/imports/session-1/` (codec import-verified lossless), and a
  weekly BDFL driver agent that directs the project and evolves the
  non-code processes without ceremony (ADR-0005).
- BDFL core mandate and success scorecard codified in ROADMAP.md (mission:
  trustworthy, honest, wanted; metrics: RATIO/TRUST/SPEED/USERS/SIMPLICITY,
  FLOW/HEALTH/HONESTY) and wired into the weekly digest; BDFL steers all
  non-code OSS aspects (docs, blog, launches — external posting
  operator-gated). BDFL cadence raised to every three hours with run-economy
  rules, and an explicit bias to solve problems by improving the agent
  system itself (ADR-0007).
- Single-language policy (ADR-0006): Rust only — the founding Python
  harness was verified, then moved to git history; its proxy-speed
  experimentation is to be recovered via an ideal-cost accounting mode in
  the Rust models. Corpus sourcing plan and test-suite strategy codified
  (`research/corpus/POLICY.md`, `docs/TESTING.md`).
