# ADR-0027: The seven-day allowance is a cadence constraint

Status: accepted · Date: 2026-08-23 · Prompted by the first budget reading (PR #187)

## Context

Every agent in this project draws on one subscription, and that
subscription has a seven-day allowance shared by all five seats. Until
PR #187 nothing read the number. The pause machinery (ADR-0018) keys on
a `rejected` rate-limit event, which arrives when the allowance is
already gone: the factory stops, for up to a week, with no prior
signal. That is a backstop, not a governor. A backstop that fires is a
design that failed earlier.

The first reading, 2026-08-23 19:30 UTC:

```
seven_day: 78% used, 22% left, resets 2026-08-26T22:00:00Z (74.6h)
```

Sustaining to the reset allows 0.295%/h. The window's average to that
point was 0.834%/h, and the hour measured was running at 2.51%/h. Two
independent methods put exhaustion the same distance out: the
utilization projection said ~9 hours, and `run-telemetry.py` measured
160 agent runs and 3,973,559 output tokens in the preceding day, which
spends the remaining 22% in about the same time. The factory was
roughly one working evening from a three-day silence.

Where it goes, from the same telemetry:

| role | runs/day | median out tok | scheduled? |
|---|---:|---:|---|
| reviewer | 82 | 15,924 | no, per PR push |
| bdfl | 53 | 17,365 | yes, 3/h |
| maintainer | 23 | 30,002 | yes, 2/h |
| deslopper | 2 | 20,698 | yes, 2/day |

The reviewer is the largest count and is not a candidate: it is
demand-driven, and independent review is how the mission's
"trustworthy" survives the rest of us moving fast. What is left is the
scheduled wakes, and they cost the same whether or not the wake had
anything to do. A BDFL delta wake that finds nothing still runs a
sweep, a drain, a state check and a retrospect: 17k output tokens,
7.5 minutes, three times an hour, forever.

## Decision

Scheduled cadence is set against the allowance, not against impatience.

- `agent-bdfl`: `11,31,51 * * * *` → `11 * * * *`. Three wakes an hour
  to one, which ADR-0015 already names as this seat's normal.
- `agent-heartbeat`: `22,52 * * * *` → `22 */2 * * *`. Two an hour to
  one every two hours, which is the revert target #60 itself names.

Neither number is invented. Both seats return to the cadence their own
records call normal; the sprint's exit criterion is what changes.

This ends the stabilization sprint's cadence (issue #60, operator
directive, Telegram 2026-08-22) before its stated exit criterion. That
criterion is 24 consecutive hours with every scheduled agent run green
and no manual intervention. It has not held: `agent-heartbeat` run
`32660844700` died on 2026-08-23 at 19:19, 26 seconds after the feature
that dispatched it merged, and PR #190 is the fix. Waiting for the criterion is not a way to preserve the
sprint's cadence, it is a way to spend the allowance before the
criterion can ever hold. The sprint's goal survives; only its poll rate
changes.

The same read found #60's cadence section already stale: it records the
heartbeat at `52 * * * *` while the workflow ran `22,52 * * * *`. Two
places holding one number, drifted, exactly as the house value predicts.
ADR-0015 already says the cron line is the single source of truth for
how often a seat runs, so #60's copy is deleted rather than corrected.

Cadence is now a quantity with a budget behind it, and the retrospect's
budget footer is where that budget is read. When it reports slack, the
cadence goes back up; when it says SLOW DOWN, this is the first lever.
ADR-0015 still holds: the cron line is the single source of truth for
how often a seat runs. This ADR is why the line reads what it reads.

## Consequences

**Operator responsiveness is unaffected.** This is the load-bearing
point and it is easy to get backwards. The cron does not carry operator
input. Any personal operator action on the repository wakes the BDFL
within seconds through its event triggers, and a Telegram message wakes
it through the worker's `workflow_dispatch`. The predicate in
`agent-bdfl.yml` admits both regardless of schedule. What the cron buys
is *autonomous* work: the queue, stall detection, the digest. One wake
an hour is 24 autonomous work items a day against a 21-issue queue.
The constraint was never how often the factory was allowed to start.

**The maintainer is now demand-woken too.** PR #186 wakes that seat
when a review verdict lands, which is when it has something to do.
Quartering its cron removes idle wakes without touching the responsive
path, and that path is why the cron was raised to every 30 minutes on
2026-08-23: the operator saw issues piling up faster than one seat
could clear them. What actually cleared that pile was the realm split
between the two seats, not a higher poll rate.

**Chat latency should improve slightly.** Real wakes share one
concurrency lane, at most one running plus one pending. On 2026-08-23 a
Telegram dispatch at 18:55:53 was displaced because a scheduled run
held the lane and another was queued; the operator waited 24 minutes
for a reply. Fewer scheduled wakes means the lane is free more often.
It does not fix the case where a long run holds it: a 36-minute session
still blocks whatever arrives during it. That is a separate problem and
this ADR does not claim to solve it.

**Reviewer volume falls without being touched.** Review runs are
downstream of how many PRs the factory opens, and fewer autonomous
wakes open fewer PRs. The largest line in the table shrinks by leaving
it alone, which is the only way to shrink it without cutting into
verification.

**The cadence already exceeded the service rate, so most of those wakes
died anyway.** A BDFL run that does real work takes 30 to 90 minutes;
the previous one ran 36. At one wake every 20 minutes into a lane that
holds one running plus one pending, the third wake is cancelled before
it reads anything. Two were cancelled during the run that wrote this
ADR, at 19:29:52 and earlier at 18:55:53. The old cadence was not
buying three chances an hour to notice something. It was buying one
long run and two funerals.

**This does not by itself bring the rate under the line, and saying so
is the point.** Scheduled wakes were 76 of the day's 160 agent runs;
this removes about 46 of them, and the reviewer runs they no longer
generate remove more. Call it a third off the window's 0.834%/h
average, landing near 0.5%/h against the 0.28%/h that reaches the
reset. That is a longer fuse, not a defused one: the allowance still
runs out around Tuesday. It is the first lever because it is the only
one that costs nothing real, removing wakes that had nothing to do and
nothing else. The honest position is that the project may still hit the
pause, and the pause machinery exists precisely because the operator
accepted that outcome when they chose subscription-only auth.

**Throughput drops, deliberately.** Roughly a third of the previous
autonomous BDFL capacity and half the maintainer's. On 2026-08-23 the
project merged seven PRs in two hours; the constraint on this project
is not how fast it can open work, and a factory that spends its week by
Monday evening ships nothing Tuesday.

## Revisiting

The budget footer on every BDFL wake. If it reports the allowance
running slack against the reset for a full window, raise the cadence
and record the new reading here.

It will keep saying SLOW DOWN at this cadence, because the arithmetic
above says so. The order of the next levers is set here so that a
future run under pressure does not reach for the cheapest one:

1. per-role effort (ADR-0021), starting with this seat, which spent
   40,956 of its previous run's 75,807 output tokens thinking;
2. model tier (ADR-0012), and only downward for seats whose work is
   mechanical;
3. nothing else.

The reviewer is not on the list at any position. It is the seat that
checks the others, and a factory that saves its allowance by reviewing
less has spent the mission to buy a week.

### 2026-08-23 20:05: lever 1 pulled, and the list runs out

One hour and one BDFL wake after this ADR merged:

```
seven_day: 80% used, 20% left, resets 2026-08-26T22:00:00Z (74.0h)
    burned 3.0% over the 55 min audited (3.26%/h); 0.27%/h reaches the reset.
```

The cadence cut had not had a full window to show in that rate, so the
reading neither confirms nor refutes the 0.5%/h estimate above. It does
not need to: the estimate itself was already twice the line.

So lever 1, `bdfl.effort` `xhigh` → `high`. Thinking was 54% of this
seat's output tokens at `xhigh`, and this is the only seat on Opus.

**It does not close the gap, and lever 2 is empty.** Halving the
thinking of the seat that is 31% of the day's raw output tokens is worth
somewhere between a twelfth and a fifth of the total rate; call the
result 0.4%/h, which is 50 hours of a 74-hour wait. Lever 2 was model
tier, downward, for mechanical seats. There is no such move left:
`agents/models.json` gives the other four seats a one-rung ladder at
`claude-sonnet-5`, ceiling and floor the same, so none of them has a
rung to drop to. The only tier left is this seat's Opus floor, and
ADR-0012 makes proposing a lower floor the operator's, not mine.

**The measurement gap this exposes matters more than the number.** The
audit artifact carries one global `utilization` fraction. Per-role
output tokens are known; the weight the allowance puts on Opus against
Sonnet is not. So this project can see that it is overspending and
cannot see where, which is why every projection here carries a range
instead of a value, and why "cut the seat on the expensive model" is an
argument from direction rather than from measurement. Issue #63 already
proposes reading a typed rate-limit event; per-role attribution belongs
with it.

Escalated to the operator as #197 with three options: accept the
blackout, raise the allowance, or lower this seat's floor. Default if
nothing is said is the blackout, with the remaining allowance pointed at
operator-triggered wakes so the chat channel outlives the autonomous
work. The blackout is now a prediction with a date on it rather than a
Monday morning discovery, which is the whole of what the budget footer
bought.
