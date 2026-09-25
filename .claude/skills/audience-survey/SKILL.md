---
name: audience-survey
description: The mothergod herald's weekly USERS survey, one dated journal entry in three parts, measure the audience against ROADMAP.md's USERS proxies, study one successful open-source project's marketing, decide what changes on the surface. Use when marketing/JOURNAL.md has no survey entry dated within the last 6 days. A run that finds one skips this and ships one surface improvement instead.
user-invocable: true
---

# Audience survey

The weekly duty, about one herald wake in fourteen, which is why it lives
here and not in every wake's context (ADR-0025). The trigger is
self-healing: a survey the run never reached is retried by the next wake,
so a missed week costs latency and nothing else.

The survey is the run's one unit of work: one entry dated today in
`marketing/JOURNAL.md`, headed `Survey`, three parts, shipped as the run's
PR. The journal's header defines the entry kind. The previous survey is
the template: every number sits next to that entry's, and its Source
column names each instrument exactly enough to re-run it.

## (a) Measure the audience

The proxies are the ones `ROADMAP.md`'s Scorecard defines under USERS;
that definition is the list. Two instruments the previous entry cannot
tell you: mothergod.dev analytics come from the Cloudflare Web Analytics
API on `CLOUDFLARE_API_TOKEN`, in the environment and never printed
(CLAUDE.md rule 10); repo traffic comes from the ledger issue labeled
`traffic-data`, because the GitHub traffic API is closed to the session's
tokens.

Every read is read-only. External platforms are instruments here, never
channels: querying them is the whole of the survey's contact with them
(MISSION.md).

Two readings the instruments punish (#755 spent four review rounds on
them). Repo uniques are the ledger's own 14-day figure, never a sum of
its daily rows, because uniques do not add. A mention search yields
mentions you read, never a hit total: Algolia's `nbHits` for one query
answered three different numbers in one afternoon. A number you cannot
reproduce on a second fetch is not a number; it is a caveat, written as
one.

A metric you cannot measure is itself a finding: journal it with the
mechanism of failure, and file `blocked-on-human` when only the operator
can unlock it, naming the exact permission or secret.

## (b) Study one project

One successful open-source project's marketing per survey, read at the
source and dated. Extract principles; record what you adopt and what you
reject, each with its reason.

## (c) Decide what changes

State what the surface changes because of (a) and (b). Each concrete
piece becomes an issue labeled `marketing` and `product` through
`gh-comment --new`, which is the queue steward runs pick from. A week in
which nothing crosses that bar is recorded as that.
