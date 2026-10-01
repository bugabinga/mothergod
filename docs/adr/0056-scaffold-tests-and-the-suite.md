# ADR-0056: Scaffold tests and the suite are two artifacts

Status: accepted · Date: 2026-10-01 · Extends ADR-0043 · Prompted by operator question (Telegram, 2026-10-01)

## Context

ADR-0043 organised testing as a cost-tiered portfolio and left one word
doing two jobs. "Test" names both the example a session writes beside new
code to see it work, and the behavioral layers (`tests/`, `fuzz/`, the
properties, the golden pins) that carry the product's trust claim. The two
have different lifetimes, and the project has been maintaining the first
to the second's standard.

Measured on the repository at `6539dcd` (2026-10-01), 42 days of history:

- Inline `#[cfg(test)]` code is 7,504 of 17,324 lines in `src/`, 0.76 per
  product line; `literal.rs` is 3,068 lines, 1,372 of them tests, longer
  than one default agent read. 515 inline tests were added and 69
  deleted; 59 of the 69 went with the code they pinned (a rejected
  candidate the same day, PR #763; measurement apparatus within three
  weeks, #600, #665, #784; a retired version, #722), none because the
  test was wrong.
- Mutation debt: twelve `mutants: N survivors outlived PR` issues in
  eight days (#695 to #838). Four were false (#790, #815, #835, #838):
  `tests/torture.rs` already decided the mutant and `mutants-check`
  cannot run it (#791). Two were equivalent mutants answered with an
  exclusion (#695, #696). Six became tests, written after the merge by a
  later session, pinning hand-computed intermediate values (#713, #717,
  #813, #827); #713's were deleted three days later with the sink they
  tested, and #813 refactored product code so a test could reach the
  arithmetic. Bugs found by the twelve: none. Real gaps the instrument
  has found: two, both where the suite makes a claim (#390's decode
  bounds surviving deletion, from the whole-crate sweep; #827's bin
  boundary).
- The suite over the same days: `tests/claims.rs` caught four stale
  published claims (#431 twice, #469, PR #800's red main); the torture
  sweep found one abort path and three abort sites at its own creation
  (#479, #478); fuzzing found the decode amplification the day its
  targets landed (#219) and nothing in 15.4 CPU-hours since; the golden
  pin held one encoder-only change until #290 ruled. `cargo test` went
  red in CI three times in the history `gh` retains, all `claims.rs`,
  never an inline test. Zero of the reviewer's 45 FAIL verdicts since
  2026-08-20 carried a must asking for a test.
- Inline tests at write time, from `research/JOURNAL.md`: the
  `parse_optimal` hang (#179), a range-coder interval assigned to the
  wrong bit, an ideal-cost sink desynced from the real encoder by 1.33%,
  a wrong smoothing floor, each caught before the PR opened.

Inline tests earn their keep the day they are written and cost from then
on; the suite earns its keep for as long as the claim stands. One bar for
both produced work on the wrong class: pins on apparatus that lives for
days, filed as product bugs.

## Decision

Tests are two artifacts.

**Scaffold tests** are what an implementer writes beside code to see it
work: inline unit tests on private helpers, hand-computed pins of
intermediate values, examples of a function's intent. A practice, not a
deliverable. The implementer writes as many as the work needs, nobody
else writes them, and nobody maintains them: a scaffold test is deleted
with the code it pins, and no replacement is owed when a suite layer
states the function's observable claim. A surviving mutant in scaffold
territory is a map reading, not debt: no issue, no test owed.

**The suite** is the engineered artifact: the layers `docs/TESTING.md`
lists, each stating a contract (the public API, the format, a module
invariant the journal names, a published claim) and naming the failure
class it catches. A suite test is written against the contract, never
against an implementation's intermediate values, so a model change does
not rewrite it. Location is not the class: a module invariant pinned
inline is suite; a hand-computed pin in `tests/` would be scaffold.

A change owes the suite exactly the contribution its class demands, and
`docs/TESTING.md` "Two classes" lists them. The implementer makes it in
the same PR; the reviewer checks the list, not the test count. New suite
instruments (a fuzz target, a torture fixture, a strategy) are
test-engineering slices with their own M7 issue, never riders on a
feature PR.

Mutation testing measures the suite. `mutants-debt` files a merged PR's
survivors only in decode-safety territory, where hard rule 2 is the claim
and #390 proved the instrument's worth; the territory rule is executable
in the script. Survivors elsewhere stay annotations on the open PR for
the reviewer's eye.

Module tests live beside product code, not inside it:
`src/<module>/tests.rs`, declared by `#[cfg(test)] mod tests;`. A product
file then reads in one pass and test lines count by file name.

No seat owns the suite. Its design is the BDFL's through
`docs/TESTING.md` and the trust ledger; its reds already route to the
fixer by alarm (ADR-0036); its churn is low, 44 commits to `tests/` in 42
days, 19 of them `claims.rs`. A seat whose job is tests writes tests
whether or not a failure class needs one, the manufactured assertion
ADR-0043 forbids.

## Consequences

Mutation-debt filings drop to the decode path, and the heartbeat stops
spending sessions on pins in apparatus. The `try_new`/`Default`
allocation-fallibility class is excluded in `tests/mutants.toml` with its
proof, because torture decides it and `cargo test` cannot; #791 stays
open for putting torture on a schedule. The mutation score, once #455
lands, reads lower than a scaffold-inclusive number would; that is the
honest reading of the artifact.

Moving inline test modules to sibling files is one mechanical issue,
#853, routed `product`; it also changes
`.github/scripts/status-data.py`'s test-line split from brace counting to
file names.

Unchanged: hard rules 1 to 3, the required gate, ADR-0043's tiers, #290's
golden rule, and the `compression-experiment` rule that an accepted
candidate's apparatus stays on main with its tests until the wiring
slice; those tests are scaffold, written once by the implementer and
deleted with the apparatus.

## Rejected alternatives

- **Stop filing mutation debt entirely.** #390's class, an unverified
  hard-rule-2 guard, has no other post-merge reader until #455's sweep
  exists.
- **Measure mutants against the suite only**, excluding inline tests from
  the kill set. The honest number for the artifact, but it multiplies
  survivors at the moment the project has shown it answers survivors
  with pins; revisit once the migration makes the kill set a file list.
- **A test-steward seat.** A cron tick under the budget governor for a
  suite that changed 44 times in 42 days, and the mutants-debt pattern
  written as a job description.
- **Delete the existing inline tests.** They catch bugs at write time
  and cost nothing until the code they pin changes; the sibling-file
  move makes their cost visible and their deletion a file operation when
  that day comes.
