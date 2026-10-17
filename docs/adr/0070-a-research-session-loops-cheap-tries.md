# ADR-0070: A research session loops cheap tries and lands one PR

Status: accepted · Date: 2026-10-14 · Prompted by issue #962

## Context

ADR-0003 gave the researcher one experiment per session.
ADR-0062 retired that seat and left the journal to the heartbeat's duty 4c through `compression-experiment`, which kept the one-candidate shape: every try is implemented, measured, journaled and shipped through its own PR and review, rejections included.
`research/progress.jsonl` by ISO week: W38 17, W39 28, W40 16 (#962; the fleet was paused 2026-10-05 to 10-08).
The founding session ran about 41 tries of the same loop in one sitting (`research/JOURNAL.md`'s header): a frozen harness, keep or revert on one number, one row per try, landings rare.
A try's measurement costs about a minute once the crate is built (`ratio` ran 36 s on run 37896355589, #962); the PR and review around it cost a session each.
Review is where trust lives, and it is owed per landing, not per try.
The operator pointed at karpathy/autoresearch (Telegram, 2026-10-09): the same loop contract, with the division of labor this project lost on its way to a product.

## Decision

A research session on an idea made of cheap tries runs a loop and lands one PR.

- A cheap try leaves `decode` unchanged: an encoder-only choice measured on the real bitstream, or an unwired model variant priced by the bench crate's pairing driver.
  A try that changes what `decode` does is one candidate on the existing route, with `decode_cost_delta`.
- Selection is on the train cases; a kept try is the champion for the next.
  The sealed set is read once per session, for the keeper, and the corpus policy's verdict applies to the keeper whole.
  Keep-or-revert on validation is the Goodhart loop S1-L5 and S1-L7 forbid, and autoresearch runs it because it has one set.
- Every try is a `research/progress.jsonl` line, a reverted one with `val_delta_bpb` null: the one null the schema admits beyond a capability patch.
  The journal gets one entry per idea with its tries by id, and the lead's bullet one pointer sentence.
- No try starts past 60% of the heartbeat's `timeout-minutes` from the job's start, the moment that timeout counts from; that mark or the idea's last try ends the loop, and the session lands with the rest.
  Every try runs in the foreground to completion (CLAUDE.md hard rule 12).
- One PR per session: the keeper's code with both research records, or the records alone when nothing was kept.
  The reviewer re-measures the keeper, as it re-measures a candidate now, never the tries.

The procedure is the `compression-experiment` skill's loop section; the null rule is `research/README.md`'s.

## Consequences

The measure is tries per week: `research/progress.jsonl` lines per ISO week, W38 to W40 the before, the four weeks from the first loop session the after.
The accept rate is counted per session, because per try it falls by about 1/N by construction.
Under 2x, a superseding ADR reverts the loop with the mechanism journaled.
FLOW's experiment count on /status carries the number; the weekly survey reads it.

`progress.jsonl` grows faster, and an id is a try, not a landing; `kind` and the rest of the schema stand.
Many tries on the fixed train cases raise the memorization risk S1-L7 names; the sealed read at the landing is the check, and the policy's rotating train windows (`train_window`) are the lever if keepers start failing it.
A loop session runs on the heartbeat's rung; a thinner loop is a ladder question (ADR-0031), not a seat.

## Rejected alternatives

One PR per try, the shape until now: the ceremony is the cost, and it buys no trust, because the reviewer can only re-measure a landing.

Keep-or-revert on the sealed set, as autoresearch does on its one set: S1-L5, S1-L7.

A research seat looping on its own cadence: ADR-0062 retired it on the same evidence, one mandate in two prompts.
