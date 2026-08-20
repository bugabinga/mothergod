# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[SemVer](https://semver.org/) once released.

## [Unreleased]

### Added

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
- Single-language policy (ADR-0006): Rust only going forward; the archived
  Python research loop is a frozen read-only oracle, and its proxy-speed
  experimentation is to be recovered via an ideal-cost accounting mode in
  the Rust models.
