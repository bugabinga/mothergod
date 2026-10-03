# ADR-0060: A session table as the long-term run store

Status: accepted · Date: 2026-10-03 · Amends ADR-0023 (the report as store) · Prompted by issue #891

## Context

Every agent run publishes an audit artifact: persona, prompt, response, and a `metadata.json` of API-authored numbers read under one whitelist (ADR-0023, `.github/actions/agent-audit`).
GitHub keeps an artifact 90 days; the oldest, from 2026-08-22, expires 2026-11-20.
ADR-0023 made the weekly model-intel report's edit history the long-term store.
It keeps two windows of aggregates per week and answers the questions asked in 2026-08.
It cannot answer one it did not anticipate: the reviewer's turns per PR since its persona changed, every run denied a tool this month, the director's turns per wake over a quarter.
Two readers walk the archive today, `retrospect` since the last wake and `run-telemetry.py` over two windows, each for its fixed question; a new question is a third walk or one hand-assembled in a session shell.
The operator's steer (Telegram, 2026-10-03, inbox messages 857 and 861): visibility over sessions is a standing instrument of the director's role, not an answer produced on request, and storage beyond KV is available if the design needs it.
The Cloudflare account already serves the project (ADR-0009): one KV namespace holds the operator conversation, and the token is granted D1 and R2 on the free tier (`agents/OPERATIONS.md`, the admin token section).

## Decision

Every agent run appends one row to a table `sessions` in a Cloudflare D1 database, and any question about sessions is a SQL query against it.

- The audit action writes the row (`.github/scripts/session-row.py`) once the artifact is extracted, through `audit_facts.py`, the whitelist `run-telemetry.py` reads, so the parse exists once.
  Identifiers and API-authored numbers only: run, attempt, instant, role, trigger, commit, model, tokens, turns, duration, denials, error, stop reason, persona hash, prompt and response byte counts.
  No prose reaches the table (ADR-0019); the artifact keeps the text for its 90 days, linked by run id.
- A write that fails is a warning annotation on the run and nothing more: observability does not break the thing it observes (ADR-0023).
  The artifact remains the record of a run the table missed.
- `.github/scripts/sessions` is the one reader: `sql` for any statement, `roles` and `show` as the two canned reports, `migrate` for the schema.
  It prints UNREADABLE and exits non-zero when the database cannot be read, never an empty table, the contract `inbox` and `security-alerts` already carry.
- The schema is `infra/sessions/schema.sql`; the database id is named once in `d1.py`, as the KV namespace is in `kv.py`.
- The table is the long-term store.
  ADR-0023's weekly report keeps its role as notification channel and operator-readable digest; its edit history is no longer asked to be the archive.

## Consequences

A question about sessions costs a query, not a script, so the set of questions grows and shrinks without the code changing.
The director's standing reads, retrospect's footer and the deep survey's process health, can draw trends from the table once two windows of rows exist; wiring them is chartered in #891, not decided here.
Rows exist from this decision forward.
The archive still holds every session since 2026-08-22, so a backfill from artifacts, run before 2026-11-20, makes the table complete; after that date each expiring artifact takes its session with it.
A fourth store, at the cost of one: a schema file to keep true, a table nobody hand-edits, and a token already in every seat's environment making one more call per run.
The free tier's limits exceed the project's volume by three orders of magnitude; the table is exportable in SQL if the tier ever changes.
The six ledger issues that use an issue body or comment stream as a time series are the same shape as this table.
Each moves to it when it needs a query; none moves for tidiness.

## Rejected alternatives

**KV rows under the existing namespace.** Same token, no new resource, and every question is a Python pass over all rows: a flag per question, the growth this decision avoids. KV is a map; the questions are aggregates.

**A JSONL file in the repository.** A commit per run on a repository whose ruleset refuses direct pushes, and run history interleaved with code history.

**Keeping ADR-0023's report as the store.** A store that holds only the answers to anticipated questions is a report. The report stays; the storage claim goes.
