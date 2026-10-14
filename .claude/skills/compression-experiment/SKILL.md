---
name: compression-experiment
description: "Use when choosing, running, or recording one compression experiment, or a session of cheap tries on one idea (ADR-0070), including a prerequisite codec or benchmark capability patch needed to measure it."
user-invocable: true
---

# Compression experiment

`research/JOURNAL.md` owns experimental memory and standing leads.
`research/README.md` owns the progress schema.
`research/corpus/POLICY.md` owns corpus separation, acceptance, and quoted
number provenance.
ADR-0070 owns the loop's contract.
This skill coordinates one run without copying those rules.

## Preflight

1. Read `research/JOURNAL.md` in full, then `research/README.md`,
   `research/corpus/POLICY.md`, and `ROADMAP.md`.
2. Inspect open PRs and `claude/*` branches.
   Do not duplicate work already in flight.
3. Confirm the codec and benchmark harness can measure the next candidate.
4. Note the clock: the loop's end is measured from here.

## Capability route

When measurement is not yet possible:

1. Select the smallest unclaimed codec or benchmark capability patch needed
   for the next experiment.
2. Implement it and prove the capability with focused tests.
3. Run the project quality gates.
4. Record `kind="patch"` through the journal and progress schema.
   Use null deltas only under `research/README.md`'s capability-patch rule.
5. Stop after the capability verdict is recorded.

## Experiment route

When measurement is possible:

1. Select exactly one idea, in order:
   - the journal's top standing lead;
   - one implementable literature idea with its source;
   - one cheap wild swing.
2. State one falsifiable hypothesis naming the expected bits-per-byte effect
   and target data class.
3. Route the idea by what a try changes (ADR-0070).
   A cheap try leaves `decode` unchanged: an encoder-only choice measured
   on the real bitstream, or an unwired model variant priced by the
   pairing driver.
   An idea made of cheap tries runs the loop below.
   A try that changes what `decode` does is one candidate, steps 4 to 8
   once, with `decode_cost_delta`.
4. Implement the candidate without changing its benchmark, corpus, or
   acceptance guard.
5. Apply the required round-trip and adversarial guards, then measure through
   the corpus policy's train and sealed-validation procedure.
   An unwired literal candidate is priced by the bench crate's pairing
   driver, `mothergod_bench::literal_pairing::train_and_sealed_delta_bpb`
   (issue #828), never by a scratch binary of your own: S2-A104, S2-R27
   and S2-A106 each rewrote that driver by hand and the third divided by
   the wrong byte count (PR #823).
6. Apply the corpus policy's verdict and number-provenance rules unchanged.
   A candidate that changes what `decode` does also records
   `decode_cost_delta` (`research/README.md`), measured with
   `baseline_gate -- speed` before and after.
   Past the speed gate's margin the slice either moves
   `speed::BASELINE_DECODE_COST` with the trade stated in the PR body, or
   is rejected on that trade; the corpus verdict stays the policy's (#907).
7. Record the mechanism and verdict in both research records, a slice in
   the shape the journal's header gives it: its own entry, one pointer
   sentence in the lead's bullet.
   A rejected candidate is deleted in the same PR, whole: code, tests, and
   the apparatus that measured it; its journal entry is the only trace.
   (PR #752's review read "leaves" as "stays" and 731 lines of rejected
   code landed on main.)
   An accepted candidate stays on main until its wiring slice, with the
   apparatus that reaches it (pairing function, cost sink, experiment entry
   point) and focused tests on all of it, because the code the wiring slice
   ships must be the code that produced the recorded number, and a private
   candidate with no caller is dead code the lint gate refuses.
   The wiring slice deletes that apparatus in the PR that wires
   (PR #600's shape); until then it is code on main like any other, and a
   mutant surviving in it is a test owed, not a deletion (issue #715).
   The scratch driver is deleted with the verdict either way: once the
   numbers are recorded, nothing calls it.
   Numbers live in the research records only.
   An entry names no test and counts none: the tests are in the tree or
   gone with the candidate, and PR #765 cited one that never existed, then
   spent two rounds on counts.
   A module doc cites the journal id and never restates a figure;
   PR #690 carried one count in four places and needed two review rounds to
   correct them all.
8. Run the project quality gates and leave exactly the recorded PR scope.

## Loop

One idea, many tries, one PR (ADR-0070).
The champion is main at the session's start; a kept try is the champion
for the next.
Per try:

1. State the change and its expected train effect in one sentence.
2. Apply it and measure its train delta against the champion through the
   bench crate, in the foreground: `literal_pairing::case_delta_bpb` over
   `baseline::cases()` for a model variant, `baseline::measure_all` on
   champion and try in the same session for an encoder-only change.
   No try generates a sealed case.
3. Keep the try when the train mean falls; otherwise revert it whole.
4. Append its `research/progress.jsonl` line now: `train_delta_bpb`
   against the champion it was applied to, `val_delta_bpb` null, verdict
   `rejected` when reverted, the mechanism in one sentence.

No try starts past 60% of the heartbeat's `timeout-minutes` from the
preflight clock; that mark or the idea's last try ends the loop, and the
session lands:

5. Measure the keeper, main plus every kept try, once on the sealed set:
   `literal_pairing::train_and_sealed_delta_bpb` for a model variant, the
   two sealed kinds at `sealed_seed(CASE_SEED)` on real bitstreams for an
   encoder-only change.
   The corpus policy's verdict applies to the keeper whole: a sealed
   regression reverts every kept try and turns its line `rejected`;
   otherwise every kept line turns `accepted`; both carry the keeper's
   `val_delta_bpb`.
6. Record one journal entry for the idea: the keeper's mechanism and
   sealed verdict, then its tries by progress id with their train deltas;
   the lead's bullet gets its one pointer sentence.
   The route's record step governs what of the keeper stays on main.
7. Open the session's one PR: the keeper with both research records, or
   the records alone when nothing was kept, under the route's gates.

## Environmental failure

Return the exact blocker without inventing a verdict or measurement.
The calling seat's prompt owns where that operational result is posted.
