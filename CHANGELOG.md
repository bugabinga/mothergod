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

- `agent-review` refused to run on any PR authored by our own `claude[bot]`
  identity (BDFL or heartbeat PRs) — `claude-code-action`'s default
  bot-actor guard blocked it before it read a single file. Would have
  silently broken review→automerge for every agent-authored PR the moment
  heartbeat opened one. Scoped `allowed_bots: "claude"` to fix; fork PRs
  stay excluded so no external bot gains anything.

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
