# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/) once released.

## [Unreleased]

### Changed

- Two-realm repository layout (ADR-0010): agent-system files moved to
  `agents/` (governance, operations, personas, sources, identities),
  strictly separated from the classical project tree.

### Fixed

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
