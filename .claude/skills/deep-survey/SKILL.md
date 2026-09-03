---
name: deep-survey
description: The mothergod BDFL's weekly deep-run duties, in order, once the delta core is clean. Full state read, process health, scorecard, sources review, workflow speed hunt, model ladders, roster charters, lifecycle verification, and the digest that records the survey ran. Use on a SURVEY wake, meaning a scheduled Sunday wake whose Sunday has no deep-survey digest on the ops-log issue yet. Not for chat wakes or routine delta wakes, which skip these duties entirely.
user-invocable: true
---

# Deep survey

The weekly duties. They fire on roughly one BDFL wake in seven, which is
why they live here instead of in every wake's context.

Run the delta core first, in full. The survey is what a clean delta core
earns, never a reason to skip a stalled PR or an unread operator comment.

The mode condition is self-healing: it keys on a deep-survey digest
dated that Sunday on the ops-log issue. A survey the run never reached
is retried by the next scheduled wake, so a missed Sunday costs latency
and nothing else. That only holds if the digest carries the date, which
is this skill's completion gate.

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
judgment. A metric you cannot measure is itself a top gap: schedule the
work that makes it measurable before the work it would measure.

## 4. Stay current

Review `agents/SOURCES.md`: new Claude models and features,
token-efficiency levers, context engineering, agentic and skills best
practices, whatever becomes the new smart way to run software factories.

Adopt what measurably improves the machinery, prune what rotted, log
every adoption and every deliberate rejection in the SOURCES.md adoption
log and the digest.

Includes your own substrate: action and CLI versions, model per role, and
Cloudflare tooling. Their agent bootstrap (MCP servers and skills) is at
https://developers.cloudflare.com/agent-setup/prompt.md; when real
Cloudflare work starts, fetch it and wire what fits by PR.

## 5. Stay fast

Operator directive, Telegram, 2026-08-22. A workflow minute is operator
subscription and contributor wait, both budget.

Read recent Actions timings and hunt three smells: a workflow that got
slower, a cache that stopped hitting, a run a concurrency group or path
filter would have skipped entirely. Fix by PR like any defect.

## 6. Model ladders

ADR-0031 and ADR-0018. Track model releases against `agents/models.json`:
when a stronger model ships, prepend it by PR and say why.

Set the other seats deliberately, from published news and lived
experience of where each role struggles: judgment-heavy roles reward
strength, mechanical roles may not. Unpinned means drifting with a
default nobody chose. Log each change in the SOURCES.md adoption log with
the reason.

## 7. Roster charters

Operator directive, Telegram, 2026-09-01. A seat's charter is
provisional, tuned to the project's current phase, and re-evaluated here
alongside the model ladders: trim duties that stopped earning their
tokens, extend or create seats when a new phase or subgoal opens.

Charter changes follow the roster rules in the prompt. The reason goes
in the digest.

## 8. Lifecycle verification

Operator directive, Telegram, 2026-08-23. Verify the recorded lifecycle
matches what actually happened: labels, milestones, and whatever else was
supposed to make state legible from the repo alone.

File fresh ideas for features, workflows, or comm channels that serve the
mission as issues, each with its case.

## 9. Context audit, one seat per week

Issue #486, item 4. Audit one agent's context against the checklist in
that issue's research comment. Smallest effective context is the target
metric. Name the seat you audited in the digest so the rotation is
visible and does not silently stall on the same seat.

## Completion gate

- The digest is posted on the ops-log issue and **carries the survey's
  Sunday date**, because the mode condition reads that date to decide
  whether the week's survey already ran.
- It carries the full scorecard: each metric, value or "unmeasurable
  yet", trend, one-line judgment.
- Every adoption, rejection, model change, and charter change made this
  run is named in it, with its reason.
- The Telegram status line rides the run's last message, as on any wake.
