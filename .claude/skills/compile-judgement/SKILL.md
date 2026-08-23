---
name: compile-judgement
description: Decide whether a piece of recurring work should stop being a decision and become a mechanism (script, CI check, test, lint, schema, guard), and where that mechanism should live. Four tests: is it hot, who has to be wrong for it to break, how hard is the substrate, and what is its liveness signal. Use when the same manual step, rescue, or question has come up more than twice. Rationale and history are in ADR-0022; this is the procedure only.
user-invocable: true
---

# Compile judgement

You have a candidate: work that keeps being re-decided. Run four tests in
order and stop at the first failure.

## 1. Is it hot

Three occurrences, or a rule you can state without hedging. Two is a
coincidence. Compiling a cold path costs more than it saves and leaves a
mechanism nobody remembers exists.

If it is not hot yet, write the observation down and stop.

## 2. Who has to be wrong for this to break

| Who | Verdict |
|---|---|
| Us, because we changed something | Compile freely. We control it and the break is loud |
| The world, because someone else changed something | Do not compile it yet. Go to test 3 |

## 3. How hard is the substrate

You may only compile against something that holds still, or moves slower
than you will maintain the encoding. Ranked, hardest first:

- Our own repo state, file layout, and conventions
- A structured field with a contract behind it, like an HTTP status or a
  typed payload key
- A documented API shape
- A third party's prose: error messages, log text, UI copy

The bottom rung is not a substrate. If your candidate keys on someone
else's wording, find the structured signal underneath it or leave the
work interpreted. Rewriting the same rule against a harder surface is the
usual fix, not better code against the soft one.

If nothing harder exists, stop. Interpreted is the correct answer for
some work, permanently.

## 4. What is its liveness signal

**Required. A mechanism you cannot observe working does not ship.**

The signal answers: if this silently stopped doing its job, what would
tell someone? Options, cheapest first:

- Fail loud instead of exiting 0 on the path that means "did nothing"
- Emit a count or a resolved value into the run log, so a zero is visible
- Write what it decided into the artifact or step summary
- Post only on delta, but make the no-delta case say so somewhere

"It will show up in the output eventually" is not a signal. Neither is a
comment. If you cannot name the observation that would reveal the
failure, you have not finished designing it.

## Where it goes

Cheapest carrier that does the job, because standing cost is paid on
every run forever:

| Carrier | Standing cost | Use for |
|---|---|---|
| Evidence arriving as an issue | none until it fires | triggers |
| ADR | none until read | the why |
| Skill | a description | depth needed occasionally |
| Data file the machinery reads | none | configuration and thresholds |
| Prompt prose | full weight, every run | almost nothing |

Prompt prose is the last resort, not the first. If a rule can be a lint,
a guard output, or a schema, it should not be a sentence an agent has to
remember.

## Do not compile

- A judgement whose inputs you cannot enumerate.
- Anything where being wrong is expensive and being slow is cheap.
- A transitional state. It will outlive the transition and nobody will
  delete it.
- Something already compiled elsewhere. Two mechanisms answering one
  question drift apart the first time either is touched.
