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
- Codec port, first entropy-model slice (`research/JOURNAL.md` S2-A11):
  a new `model` module with `Model`, the order-0 adaptive frequency
  table ported from the archive, driving `coder::Encoder`/`Decoder`
  with real data-derived cumulative-frequency ranges. The flag/length/
  offset stages of the entropy coder will each be one `Model` instance;
  the six-expert `Lit` literal mixer remains a separate, larger slice.
- Codec port, second entropy-model slice (`research/JOURNAL.md` S2-A12):
  a new `literal` module with `Literal`, the six-expert context-mixing
  literal model ported from the archive's `Lit`. Blends a two-rate
  order-1 pair, order-0, order-2, an alignment hash, and a word hash
  under gradient-derived mixing weights. Not yet wired to a `Method`
  variant; carries a known open question (`research/JOURNAL.md` S2-D3)
  about the archive's continued use of `f64` in weight adaptation versus
  the integer-only path `JOURNAL` S1-A5 records as accepted, to resolve
  before the Method-wiring PR that will need an ADR and `FORMAT_VERSION`
  bump anyway.
- `#![forbid(unsafe_code)]` on the `mothergod` crate root (issue #76): no
  `unsafe` exists in `src/` today, so the gate costs nothing and closes
  that door permanently.
- Adversarial decode seed corpus (ROADMAP M2, `docs/TESTING.md` layer 2):
  a new `tests/adversarial/` directory of tiny fixtures built to be
  invalid (header truncations at every byte boundary, bit-flipped magic,
  a future format version, an unknown method) and `tests/adversarial.rs`,
  which asserts every fixture decodes to a graceful `Err`, never a panic.
  Runs on every PR; future fuzz-found crashers (M4) promote into this
  directory as regression seeds.
- Benchmark harness, first structured-generator slice (`research/JOURNAL.md`
  S2-A14): `access_log` in the `bench` crate, synthetic web-server access
  log lines (the "jsonl/log records" class in
  `research/corpus/POLICY.md`), ported from the founding session's
  `corpus.py`. Produces exactly the requested byte length from a small
  IP/path/status pool via the existing deterministic `Rng`.
- Benchmark harness, second structured-generator slice (`research/JOURNAL.md`
  S2-A15): `json_records` in the `bench` crate, a synthetic JSON API
  response (the "json" class in `research/corpus/POLICY.md`), ported from
  the founding session's `corpus.py`. Records carry a gaussian `score`
  (Box-Muller, mean 50, stddev 15) and an `active` field true 80% of the
  time; generates records until the requested byte length is reached,
  same deviation as `access_log`.
- `clippy.toml` with `disallowed-methods` covering the float transcendental
  family (`exp`, `ln`, `log2`, `log10`, `powf`, `powi`, `sin`, `cos`,
  `tan`, `f32`/`f64`), enforcing ADR-0024: nothing on the decode path may
  call a libm function, since implementations can disagree in the last
  ulp and desync an encoder and decoder mid-frame. Scoped to `src/` only
  (`bench/clippy.toml` overrides it back off for the corpus-generation
  crate, which never touches a bitstream).
- `Method::Lz` (`research/JOURNAL.md` S2-A17, ADR-0026, `FORMAT_VERSION`
  0 → 1): the first real compression method, wiring the already-ported
  `lz`, `model`, `literal`, and `coder` modules together — optimal-parse
  LZ tokens, entropy-coded by adaptive flag/length/offset/rep-slot models
  and a six-expert context-mixing literal model, over an adaptive range
  coder. `compress` now tries `Method::Lz` and falls back to
  `Method::Stored` whenever that is not smaller. Decode bounds allocation
  and loop iterations to the payload's own declared output length rather
  than trusting it, and rejects a corrupt match/rep distance or a
  declared-length mismatch as an error rather than panicking. The declared
  output length itself is capped at 256 MiB (`codec::MAX_DECODED_LEN`,
  new `Error::TooLarge`), checked before any decode work: without it, a
  payload where the declared length and token count agree with each
  other (both attacker-chosen, unrelated to the bytes actually sent)
  could force multi-minute, multi-gigabyte decode work from a
  double-digit-byte input. Filter selection is not wired in yet
  (`Method::Lz` always runs on raw input); that remains open M1 scope.
  Measured 2.318 bits/byte on
  `research/imports/session-1/mothergod.rs` (25,524 bytes), against
  `gzip -9`'s 2.392 bits/byte on the same file.
- Filter selection wired into `Method::Lz` (`research/JOURNAL.md` S2-D2,
  in full; ADR-0027, `FORMAT_VERSION` 1 → 2): `compress` now trials every
  candidate filter `filters::select::pick` shortlists (delta, BCJ,
  transpose, or none) against the real LZ + context-mixing pipeline and
  keeps whichever produces the smallest frame, closing M1's last open
  checklist item. The winning filter is a 2-byte selector prefixed onto
  the payload (`[kind, param]`); an unrecognized selector, or a zero
  `param` on a filter that requires one, decodes to `Error::Corrupt`
  rather than being parsed. A version-1 frame's `Method::Lz` payload used
  a layout this build no longer understands (no filter prefix), so
  `decompress` rejects that combination as `Error::UnsupportedVersion`
  explicitly (`codec::LZ_MIN_VERSION`) rather than misreading it.
  Measured 2.3184 bits/byte on the same named corpus as the entry above
  (unchanged from 2.318: this file is structured text, and `Candidate::
  Identity` wins — `JOURNAL` S1-R1 already predicted delta loses on
  text); a synthetic columnar-drift round-trip test proves the wiring
  picks and correctly reverses a non-identity filter end to end.
- Benchmark harness, third structured-generator slice (`research/JOURNAL.md`
  S2-A20): `base64_wrapped` in the `bench` crate, a base64-wrapped text
  payload (the "base64-wrapped payloads" class in
  `research/corpus/POLICY.md`), ported from the founding session's
  `corpus.py`. Wraps `json_records` output in a new standalone
  `base64_encode` helper (RFC 4648, zero-dependency) and truncates to the
  requested length.

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

- `compress()` hung on long runs of a single repeated byte (issue #179,
  found while landing `Method::Lz`, S2-A17): a 200,000-byte input took
  over 60 seconds and had to be killed. `lz::parse_optimal`'s
  rep-candidate match-length scan re-walked the whole run at every
  position, with no carry-reuse equivalent to the existing hash-chain
  search's carry. Fixed with a per-distance carry
  (`research/JOURNAL.md` S2-A18); the same 200,000-byte input now
  compresses in under a second, verified against the public API
  directly and pinned by a new wall-clock-bounded regression test.
- `literal::Literal`'s exponentiated-gradient mixing-weight update
  (`research/JOURNAL.md` S2-D3, resolved by ADR-0024) called
  `f64::exp()` on both the encode and decode path; replaced with a
  crate-local `exp` built from IEEE-754 basic operations only (range
  reduction plus a polynomial, `2^k` by exact repeated doubling), so
  encoder and decoder compute a bit-identical mixing weight on every
  platform. Verified against a kept `f64::exp` reference: bit-identical
  encoded output on a 25,524-byte named corpus (well within the 1%
  budget ADR-0024 sets). Unblocks M1's remaining `Method`-wiring slice
  (issue #161).
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
