---
name: deep-survey
description: The mothergod BDFL's weekly deep-run duties, in order, once the delta core is clean. Full state read, process health, scorecard, sources review, workflow speed hunt, model ladders, roster charters, lifecycle verification, and the digest that records the survey ran. Use on a SURVEY wake, meaning a wake where `.github/scripts/survey-due` answers `due`. Not for chat wakes or routine delta wakes, which skip these duties entirely.
user-invocable: true
---

# Deep survey

The weekly duties. They fire on roughly one BDFL wake in seven, which is
why they live here instead of in every wake's context.

Run the delta core first, in full. The survey is what a clean delta core
earns, never a reason to skip a stalled PR or an unread operator comment.

The mode condition is compiled: `.github/scripts/survey-due` keys on
the marker this skill's completion gate emits, through two clauses. A
Sunday with no marker dated that Sunday is due, and so is any scheduled
wake whose newest marker is seven or more days old. The second clause
heals a shed Sunday on Monday; it exists because the week of 2026-09-21
was never surveyed when both 2026-09-27 wakes shed it under SLOW DOWN
and every later wake read not-due by the clock (#910). Emitting the
marker is the first line of the gate below, because a digest without
it reports a survey as not run.

## 1. Read the state

The one run where the expensive reads are worth their tokens (ADR-0025).

- `ROADMAP.md` against what actually merged: `git log`,
  `gh pr list --state merged`. Where the roadmap and the log disagree,
  the log wins and the roadmap gets fixed.
- Open PRs and their ages, open issues, the ops-log issue.
- `research/JOURNAL.md` and `research/progress.jsonl`. Enter a long file
  through `grep -n '^#'` on it rather than reading it whole.

## 2. Process health

`gh run list`: which agent sessions succeeded, failed, stalled, or wasted
turns? Read a failed run's story before judging it.

## 3. Scorecard

Every metric in `ROADMAP.md`'s Scorecard section, which defines them,
computed or estimated from repo evidence. USERS comes from
`marketing/JOURNAL.md`.

Each metric gets a value or "unmeasurable yet", a trend, and a one-line
judgment.

## 4. Stay current

Start with the ledger issue labeled `docs-intel`, which
`.github/workflows/docs-watch.yml` refreshes weekly, ahead of this wake,
with what moved in the Claude Code docs: pages added or removed from the
index, and content changes in the reference pages that govern this
fleet's own substrate. Read the rounds newer than the SOURCES.md
adoption log's top entry date. That date is the review watermark, and it
is the only bookkeeping: everything above it is unconsidered.

An empty or missing ledger is a finding, not a pass. It means the watch
has never fired or has been failing; check the workflow before believing
the docs stood still for a week.

Then review `agents/SOURCES.md` itself, which the ledger does not cover:
the non-Anthropic sources, new models and features, token-efficiency
levers, context engineering, agentic and skills best practices, whatever
becomes the new smart way to run software factories.

Adopt what measurably improves the machinery, prune what rotted, log
every adoption and every deliberate rejection in the SOURCES.md adoption
log and the digest. Logging the rejection is what advances the watermark,
so a round considered and dismissed is never re-read.

Includes your own substrate: action and CLI versions, model per role, and
Cloudflare tooling. Their agent bootstrap (MCP servers and skills) is at
https://developers.cloudflare.com/agent-setup/prompt.md; when real
Cloudflare work starts, fetch it and wire what fits by PR.

## 5. Stay fast

Read recent Actions timings and hunt three smells: a workflow that got
slower, a cache that stopped hitting, a run a concurrency group or path
filter would have skipped entirely. Fix by PR like any defect.

## 6. Model ladders

ADR-0031, ADR-0018, ADR-0059. Rungs in `agents/models.json` are family
aliases, so a generation launch needs no edit; the CLI adopts it. What
needs you: a new family (the model-intel issue or the cli-reference page
in #578 names it), a seat whose retrospect says its tier or effort is
wrong, or an alias 404 in the model-limits ledger.

Set each seat deliberately, from published news and lived experience of
where the role struggles: judgment-heavy roles reward strength,
mechanical roles may not, and effort is explicit on every seat because
an empty one drifts with a default nobody chose. Log each change in the
SOURCES.md adoption log with the reason.

## 7. Roster charters

A seat's charter is provisional, tuned to the project's current phase,
and re-evaluated here alongside the model ladders: trim duties that
stopped earning their tokens, extend or create seats when a new phase or
subgoal opens.

Charter changes follow the roster rules in the prompt. The reason goes
in the digest.

## 8. Lifecycle verification

Verify the recorded lifecycle matches what actually happened: labels,
milestones, and whatever else was supposed to make state legible from
the repo alone (ADR-0047).

File fresh ideas for features, workflows, or comm channels that serve the
mission as issues, each with its case.

## 9. Context audit, one seat per week

Issue #486, item 4. Audit one agent's context against the checklist in
that issue's research comment. Smallest effective context is the target
metric. Name the seat you audited in the digest so the rotation is
visible and does not silently stall on the same seat.

## Completion gate

- The digest is posted on the ops-log issue and **carries the marker**,
  on its own line, with the survey's own UTC date, the Sunday as a
  rule, or the weekday a shed Sunday healed on (#910):

      <!-- deep-survey 2026-09-13 -->

  `survey-due` reads it to decide whether the week's survey already ran,
  and reads nothing else: a prose heading gets rephrased by whoever
  writes next week's digest, and a run that could not tell reported a
  survey it had already done as skipped (#558). A missing marker costs a
  duplicated survey, so write it before the prose, not after.
- It carries the full scorecard: each metric, value or "unmeasurable
  yet", trend, one-line judgment.
- Every adoption, rejection, model change, and charter change made this
  run is named in it, with its reason.
- The Telegram status line rides the run's last message, as on any wake.
