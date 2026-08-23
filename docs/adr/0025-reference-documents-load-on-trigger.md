# ADR-0025: Reference documents load on trigger, and a skill holds a procedure

Status: accepted · Date: 2026-08-23 · Prompted by issue #172 (operator)

## Context

Every agent prompt opens with an ordered read of a fixed set of files.
The order is unconditional, so the cost is paid on every wake whether or
not the run touches the subject.

| File | chars | ordered by |
|---|---:|---|
| `research/JOURNAL.md` | 45,437 | heartbeat, reviewer, researcher |
| `agents/GOVERNANCE.md` | 18,576 | deslopper, heartbeat, reviewer |
| `CLAUDE.md` | 10,270 | deslopper, heartbeat, reviewer, researcher |
| `ROADMAP.md` | 8,571 | heartbeat, researcher |

`JOURNAL.md` is the one with no ceiling. CLAUDE.md rule 6 requires an
entry per experiment, accepted or rejected, so it grows monotonically
and will never shrink. At 45KB it is already ~11,400 tokens paid by
three agents on every run.

One correction to the numbers in #172, made by the rule that issue
argues for: the BDFL is not ordered to read `JOURNAL.md` or
`SOURCES.md` on a routine wake. Both live in the SURVEY block and the
weekly deep-run duty. On a delta wake it reads none of the four. The
~24,000-token figure is the Sunday cost, not the hourly one, and the
per-run problem belongs to the other three seats.

## Decision

Three moves, all in the prompts, no new artifact.

**1. `CLAUDE.md` is injected, so stop ordering it read.** The harness
loads it into every session as project instructions. Evidence rather
than inference: the BDFL prompt has never contained a line telling it
to read `CLAUDE.md`, and BDFL sessions carry the file's full text
regardless. All five agents run `anthropics/claude-code-action@v1` in
the same repository with no `settings` override, so the injection is
identical for all of them. Four prompts additionally order a second
copy: about 2,600 tokens and one turn, per agent, per run, for text
already in front of the model. Deleted, and `CLAUDE.md` now says so
about itself, in one line, so nobody helpfully re-adds it.

**2. Every remaining ordered read states its condition.** The reviewer
needs the journal on a diff that touches `src/` or `research/`; on the
workflow and prompt PRs that have dominated this week it needs nothing
from it. Same shape for the heartbeat. The researcher's read stays
unconditional, because the journal is its instrument and not its
background.

**3. A long file is entered through its own headings.** `grep -n '^#'`
on the target is one cheap call, and its output is generated from the
file at read time.

## Rejected: a skill per reference document

#172 proposes wrapping each document as a skill, since a skill costs
only its `description` until something invokes it, and a good
description routes. The mechanism is real and the diagnosis is right.
It is rejected on the design constraint #172 itself names: index, do
not copy.

An index of a file's sections is a copy of that file's structure.
Rename a heading and the index drifts, and a drifted index is worse
than no index because it is consulted with confidence. That is exactly
the failure item 1 of the same issue documents: a sentence that was
accurate when written, believed after it went stale. Buying context
savings with a fresh drift surface across five agents pays for tokens
with truth, and truth is the more expensive currency here.

The map is also already free. `grep -n '^#' research/JOURNAL.md`
cannot be stale, costs a few hundred tokens, and needs no maintenance
by anyone, ever.

So, stated once so it does not get relitigated: **a skill holds a
procedure — steps somebody follows, rarely, written nowhere else.**
`compile-judgement`, `rust-craft` and `deslop` are all that shape. A
reference document is not a procedure, it is a place to look, and
routing to a place to look costs one clause in a prompt. A clause has
no structure to drift.

## Consequences

Measurable, on the instrument #172 names and ADR-0022 already relies
on: the audit trail records input and `cache_read_input_tokens` per
run. If this worked, a routine reviewer or heartbeat run gets cheaper
while a run on a codec PR does not. Both flat means nothing moved.
Read it in the 2026-08-30 survey.

The risk is real and named: an agent free to skip the journal may
re-run a falsified experiment, which is the premise of rule 6. It is
held on two sides. The researcher, the seat that actually runs
experiments, keeps the unconditional read. The reviewer and heartbeat
get a condition that names the case where it matters, and the reviewer
is the second pair of eyes on anything the heartbeat ships.

`agents/PERSONALITY.md` records that a pointer to a file demonstrably
did not shape behaviour, which is why personas are interpolated rather
than referenced. That finding is about binding text, which is obeyed
and therefore must be present. This decision moves only lookup
material, which is fetched because not having it blocks the task. If
the distinction turns out to be wrong — a PR touching `src/` reviewed
without the journal ever being opened — the answer is not to index the
whole file. It is to split the journal, whose two halves are already
braided: durable laws and standing leads, which bind and are small,
and the append-only log of entries, which is lookup and is 45KB. That
split is the next move, on evidence, not now.
