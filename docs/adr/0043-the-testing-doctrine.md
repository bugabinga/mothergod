# ADR-0043: The testing doctrine

Status: accepted · Date: 2026-09-01 · Prompted by operator directive (Telegram, 2026-09-01)

## Context

Correctness is the product: mission non-negotiable 1 sells a decoder
that is lossless, safe, and deterministic, and a compressor earns trust
the way SQLite and curl did, by testing out of proportion to its size.
The current portfolio (`docs/TESTING.md` layers 1 to 7) has a solid fast
gate, but the heavy instruments are token: fuzzing runs 30 seconds per
target weekly from a cold corpus and discards what it learns, mutation
testing covers only each PR's diff, coverage is not measured at all,
nothing exercises decode under allocation failure, and test code sits
at roughly 0.7 lines per product line.

The genre's gold standards run hotter by orders of magnitude.
SQLite maintains a ~590:1 test-to-code ratio, sweeps OOM and I/O
failure injection systematically, and fuzzes structure-aware at
~500M cases/day; curl re-runs every test under an allocation-failure
sweep; zstd fuzzes continuously from generated valid frames
(decodecorpus) because random bytes die at the header check and never
reach deep decoder state.

Two forces are in tension.
Thoroughness wants every technique whose failure class nothing else
catches.
Flow wants the PR gate to stay minutes long and deterministic, because
agents iterate against it and a slow or flaky required check stalls the
whole factory (FLOW, HEALTH).

## Decision

Tests are organized as a cost-tiered, number-audited portfolio.

**Tiering by cost.** The required PR gate stays fast and deterministic:
the `cargo x check` stages, the ratio gate, and fast profiles of the
property suites. Everything expensive or statistical runs on a
schedule: nightly for coverage-guided fuzzing with a persistent,
compounding corpus; weekly for the cross-platform matrix, large
property profiles, Miri, and coverage; monthly for the whole-crate
mutation sweep. Scheduled tiers never block a PR, and never rot
silently either: a red routes to the fixer through the alarm
(ADR-0036). Advisory does not mean ignorable.

**Audit by number.** A machine-written trust ledger records what the
portfolio actually did: cumulative fuzz CPU-hours, new crashers,
mutation score, region coverage, test and code line counts. The status
page renders it; the weekly digest judges TRUST from it; the BDFL
retunes the portfolio against it. The Goodhart guard is binding:
ledger numbers are maps, never gates. The only merge-blocking checks
remain behavioral: round-trip, adversarial decode, golden determinism,
ratio.

**Earn the slot.** A technique joins the portfolio only if it catches a
class nothing cheaper catches: property testing with shrinking for
invariants (the crate's first dev-dependency; ADR-0006's zero
runtime-dependency rule is untouched), structure-aware fuzzing from
generated valid frames for deep decoder state, allocation-failure
torture sweeps for hard rule 2 under memory pressure, differential
agreement across the three decode APIs for contract drift, mutation
score for "checked, not merely reached", coverage for the map of what
is never reached, Miri to hold the `forbid(unsafe_code)` line.
Current mechanisms and cadences live in `docs/TESTING.md` and the
issues it names; this record fixes the doctrine, not the tool list.

**Steer by skill.** Agents get the conditional procedure (how to write
these tests well) as a skill, not as prompt lore: `docs/TESTING.md`
stays the strategy, CLAUDE.md stays the rules.

Test-to-code ratio is reported, never targeted. A ratio target
manufactures assertions; mutation score and coverage measure what the
ratio only proxies. The ratio is expected to cross 1:1 as a side
effect.

## Consequences

The heavy tiers cost public-runner minutes and cache space, both cheap;
the real costs are one more alarm source to keep honest and the
standing duty to read the ledger instead of assuming the machinery
works. The dev-dependency door opens, deliberately and one crate at a
time.

`ROADMAP.md` gains milestone M7 (trust engineering), positioned above
the speed and release milestones because correctness debt compounds
worse than a late release. Scoped work: #449 (ledger), #450 (fuzz
persistence), #451 (frame generator), #452 (proptest), #453 (torture),
#454 (coverage), #455 (mutation sweep), #456 (Miri), #457 (test-craft
skill).

## Rejected alternatives

- **OSS-Fuzz enrollment now**: its stated bar is a significant user
  base or infrastructure criticality; not yet cleared. Revisit when
  USERS says otherwise.
- **ClusterFuzzLite**: third-party fuzzing infrastructure to obtain
  what a cache key and a longer cron already give us. Revisit if
  PR-time sanitizer fuzzing proves necessary.
- **Codecov or similar**: an external service and badge for a number a
  JSON line in our own telemetry publishes.
- **cargo-careful in the gate**: its catch class is init and unsafe
  misuse, which a `forbid(unsafe_code)` crate lacks; the weekly Miri
  lane covers the residue.
- **A test-to-code LOC target**: Goodhart bait. The operator's signal
  (test code below product code) is treated as a symptom; the portfolio
  treats the disease.
