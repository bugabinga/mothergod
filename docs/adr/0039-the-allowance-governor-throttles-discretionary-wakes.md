# ADR-0039: The allowance governor throttles discretionary wakes

Status: accepted · Date: 2026-08-30 · Extends ADR-0027 · Prompted by issue #375

## Context

ADR-0027 made scheduled cadence a quantity with a budget behind it, and named
the cron line as the first lever to pull when the retrospect's budget footer
says SLOW DOWN. A BDFL pulls it by hand.

Since then the guard grew an allowance projection (issues #202, #369). On every
wake it reads the `allowance-state` ledger, projects the week-average seven-day
burn against the next reset, and when the projection misses, swaps the role to
its cheaper `thrift` ladder. That is a discount, not a brake. It changes what a
wake costs and not how many wakes there are, and the number of wakes is what
costs. Only two of five seats have a `thrift` block at all, and the maintainer,
the most expensive seat per run, is not one of them.

On 2026-08-30 the manual lever was pulled twice in three and a half hours: PR
#368 took the BDFL from hourly to two-hourly at 06:41, PR #374 from two-hourly
to four-hourly at 10:15. Between those pulls the thrift fix (#372) also landed.
The readings either side:

| time | spent | week-average | reaches the reset |
|---|---:|---:|---:|
| 06:20 | 53% | 0.66%/h | 0.53%/h |
| 09:49 | 56% | 0.74%/h | 0.52%/h |

Burn rose across both interventions. Each pull cost a BDFL run, a PR, a review
and a worker deploy, to change one integer the guard already had the arithmetic
to choose for itself. ADR-0022's threshold for compiling a decision into a
mechanism is twice.

The cron is also the wrong instrument for the job it was given. It is one
number serving two purposes that pull opposite ways: how responsive the factory
is when there is work, and how much the factory spends when there is not. Tuned
for solvency it is too slow on a busy day; tuned for responsiveness it is
insolvent on a quiet week. It is wrong in both directions most of the time.

## Decision

The guard skips a computed share of discretionary wakes while its projection
misses the reset. This is the second gear of the mechanism that already selects
thrift, and it joins the guard's `paused` output as a fourth reason not to run.

A wake is **discretionary** when nothing but the clock asked for it. The
calling workflow declares this, because only it knows what woke it: today the
BDFL clock tick and the maintainer clock tick. Everything else declares false.
Operator event wakes, Telegram dispatches, `/run`, alarm wakes (ADR-0034) and
every reviewer run are never skipped, because operator responsiveness and
independent review are not budget levers.

The share kept is the ratio of the sustainable rate to the observed rate,
floored at a quarter, and applied in the time domain: a wake runs if it lands
in the first `share` of the current UTC day, and does not otherwise.
Proportional, so a 1% overshoot costs the last 15 minutes of the day rather
than half the wakes.

Decimating on time rather than on a count is the load-bearing choice, and the
first draft of this decision got it backwards. It counted wakes, using the
workflow's run number, on the reasoning that a counter cannot alias against a
cron the way an hour-derived index can. But that counter advances on every wake
of a workflow, not just the discretionary ones, so a single interleaved
operator wake per tick puts the whole cron on odd run numbers, where a
keep-every-fourth rule keeps none of them. Review of PR #383 ran it: total
starvation of the seat, presented as a 25% floor. A time-domain rule cannot
have that failure because it never reads the wake stream. It asks one question
of one wake, and a run that never happened changes nothing.

What the floor guarantees is therefore a bound on the gap, not on the count: at
a quarter the keep window is six hours wide, so any seat ticking faster than
six-hourly gets at least one wake a day. That precondition is not left to
prose. The test suite reads the live crons out of `wrangler.toml` and asserts
it for each governed seat, so a future pull of the cadence lever that would
starve one fails a check instead of going quiet.

Both gears are stateless and self-restoring. Every wake re-projects from the
latest reading, so full cadence returns on its own once the allowance shows
slack, with no PR either direction and no run remembering what the last one did.
Every input that could be missing or malformed fails open: an unusable reading
costs one thin cycle at worst, never a skipped one.

This makes the guard the operator of the lever ADR-0027 handed to the BDFL. The
cron becomes the responsive ceiling, tuned for how fast the factory should
react; the governor is the throttle underneath it, tuned continuously for what
the factory can afford.

## Consequences

**The class of manual cadence PRs is deleted.** With the throttle automatic,
the cron can go back up rather than down, because raising it no longer raises
the bill: it raises the ceiling the governor is allowed to fill. That move is
not part of this decision, which changes no cron line. It is what this decision
makes safe.

**Availability drops before the model does.** Where thrift degraded quality
quietly, the second gear degrades presence visibly: a skipped wake leaves a run
log saying so, with the numbers. The keep floor is the guarantee underneath it,
because the stall sweep, the inbox drain and the operator sweep only happen on a
wake that runs. A governor that starved them for days would have traded a budget
problem for a liveness problem.

**Autonomous work concentrates in the early UTC day** while the governor is
engaged, that being where the keep window sits. This is a side effect of the
window having to start somewhere, not a schedule anyone designed. It costs
nothing on the responsive path, which is not windowed at all.

**The seat with no thrift block is now governed anyway.** The maintainer's runs
are the most expensive the factory makes and the first gear does nothing for
them. The second gear does not ask.

**The deslopper stays ungoverned**, twice a day being a rounding error against
an allowance measured in percent per hour. It carries no `source` input and so
declares nothing.

**So does the maintainer wake chained off a landed verdict** (#186), which on a
busy day outnumbers that seat's clock ticks. The argument for leaving it is that
it fires only when a PR just merged, so throttling it throttles the factory
exactly when the factory is working, and ADR-0027 already observed that
downstream volume shrinks on its own when the clock is throttled upstream. The
argument against is that it is the larger share. Neither is measured. If a
reading after this lands still misses the reset, that is the evidence, and the
chain is the next flag to flip.

**A workflow can now lie about why it woke.** The `discretionary` flag is an
input a caller sets, so a mistake there silently removes a seat from the
governor or, worse, throttles a responsive path. That risk is why the decider
moved out of the guard's YAML heredoc into `.github/scripts/guard-decide.py`,
where three of its tests assert only that the responsive path is never touched.
The starvation bug above is the argument in one example: it was invisible to
reading and obvious to running, and it lived in the file for as long as the
file could not be run.

**The budget footer keeps its job.** The governor reacts to the projection; the
footer is still how a human, and a BDFL, learn the number. An automatic
throttle that nobody watches is how a factory gets quietly slower without anyone
asking why.
