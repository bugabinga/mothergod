# ADR-0022: This project compiles judgement

Status: accepted · Date: 2026-08-23

## Context

Over a project's life, a good engineer working with an agent keeps moving
work from "decide this again each time" to "a mechanism decides it".
Scripts, CI checks, tests, lints, schemas, guards. The options differ, the
move is the same, and nobody has a name for it as a unit. Operator's
framing, 2026-08-23: calcification of intelligence.

The mechanics are a tracing JIT. An interpreted path is re-decided on
every invocation and costs full price each time; a compiled one is decided
once and then runs for free. A JIT does not compile everything: it watches
for hot paths and compiles those, leaving cold paths interpreted because
compiling them would cost more than it saves. Recurring work gets
compiled, novel work stays interpreted.

The economics bind harder on agents than on people. A human's attention
recovers overnight; a context window does not. Every token spent
re-deriving a settled answer is a token not spent on the open question,
and unlike a human the agent cannot notice it is doing this.

One day of evidence, all of it from 2026-08-23:

- `agent-pause` matched vendor error text against five phrases. That is
  compiled against prose Anthropic controls, so it broke the moment the
  message read "You've reached your Fable 5 limit", and it broke
  silently. Its replacement keys on `rate_limit_events[].status` and
  `resetsAt`: not better code, just a harder substrate.
- The model ladder (ADR-0018) moved "the agent must remember ADR-0012 and
  notice it is rate-limited" into a resolution the guard performs. Run
  32635414455 came up on `claude-opus-5` and worked for 21 minutes; earlier
  that day, runs 32634179687 and 32634021913 died in under a second to a
  429, before the ladder existed to reroute around it.
- ADR-0019 declined to make model intake an agent, on the grounds that
  filtering an API response is arithmetic.

And the failure mode, three times in one day: the pause not firing, a
review check going green in 26 seconds having reviewed nothing, and
model-intel bailing to "no report" forever after a key rename. **Every one
looked like success.** Intelligence fails loudly, saying it does not know.
Mechanisms fail silently and everyone assumes the job got done.

## Decision

Name it **compiling judgement**, and treat it as a standing duty rather
than a happy accident.

The operating tests live in the `compile-judgement` skill. This ADR does
not restate them: it records that the practice is deliberate, and why.

Three commitments.

**Every compiled step ships with a liveness signal.** Not optional, and
checkable in review rather than remembered. A mechanism that cannot be
observed working is worse than the manual step it replaced, because the
manual step at least had a person noticing its absence. All three of this
day's silent failures would have been caught by this clause alone.

**Where a rule lives is decided by what it costs on every run, not by
what is convenient to write.** The BDFL prompt is the most expensive text
in this project, so it is the last resort for a new rule rather than the
first. The skill ranks the carriers.

**No detector yet.** The tempting next step is a job that counts recurring
failures and files an issue, exactly like `agent-model-intel`. Right shape
eventually, wrong time now: this project has about a week of history, and
encoding "what recurs" from a week is the same mistake as the phrase-list
regex, which was also compiled from a handful of observations against a
surface that moved. Build it once the retrospection question has produced
three or four real answers, so it is compiled against something observed
rather than imagined.

## Consequences

The BDFL's retrospection step gains one question. That is the whole
run-time cost of this ADR.

**How we would know it is working.** The audit trail records `num_turns`,
`total_cost_usd`, and thinking-token share per run, and issue #118 is
building the aggregation. If this practice is real, the cost of a
**routine** run trends down while a **novel** run stays expensive, because
routine decisions stop needing a mind. Both flat means no compiling is
happening. Both falling means judgement is being frozen that should not
be, and the thing to look for is a mechanism deciding something it has no
business deciding.

**What this costs.** Compiled steps accrete and nobody deletes them, so
transitional encodings become permanent; a two-shape compatibility
shim written this morning had to be reverted on the operator's catch that
it would outlive its reason. And a script records what without why, so
when the why changes nobody knows the script should. That second cost is
why the ADR series is load-bearing rather than ceremonial, and why this
ADR exists at all: it is the layer that keeps compilation reversible.
Without it the endpoint is an expert system.

**The risk being accepted.** Premature compilation is the specific way
this goes wrong, and it is indistinguishable from success until the
substrate moves. The mitigation is the hot-path threshold and the
substrate test in the skill, both of which are judgement calls, so this
ADR reduces the failure rate rather than eliminating it.
