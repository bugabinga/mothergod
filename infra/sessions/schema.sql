-- The D1 database (ADR-0060): one table per standing decision, #891
-- names them. Identifiers and API-authored numbers only; a run's prose
-- lives in its audit artifact for 90 days. Each table's comment names
-- its writer and its reader: delete the reader, drop the table in the
-- same PR.
--
-- Applied with `.github/scripts/sessions migrate`, idempotent. A new
-- column is an ALTER TABLE line appended below its table and a new
-- table a CREATE appended at the end, never an edit above, so the file
-- replays on an existing database.

-- sessions: one row per agent run. Written by .github/actions/agent-audit
-- through session-row.py as the run ends and by `sessions backfill` from
-- the archive; read by .github/scripts/sessions.
CREATE TABLE IF NOT EXISTS sessions (
  run_id         INTEGER NOT NULL,   -- GitHub Actions run id
  attempt        INTEGER NOT NULL,   -- run attempt, 1 unless re-run
  at             TEXT    NOT NULL,   -- ISO-8601 UTC, when the run ended; a backfilled row's is its artifact's creation
  role           TEXT    NOT NULL,   -- bdfl, maintainer, reviewer, ...
  event          TEXT,               -- github.event_name
  actor          TEXT,               -- github.actor
  number         INTEGER,            -- issue or PR the run served, if any
  commit_sha     TEXT,               -- github.sha the run checked out
  measured       INTEGER NOT NULL,   -- 1 when a result entry was found
  model          TEXT,               -- the model that did the work
  out_tokens     INTEGER,
  think_pct      REAL,               -- share of output spent thinking
  cost_usd       REAL,               -- projected API list cost, not a bill
  turns          INTEGER,
  duration_ms    INTEGER,
  denials        INTEGER,            -- permission denials
  error          INTEGER,            -- is_error from the result entry
  stop_reason    TEXT,
  persona_sha    TEXT,               -- sha256 of the persona file carried
  prompt_bytes   INTEGER,
  response_bytes INTEGER,
  PRIMARY KEY (run_id, attempt)
);
CREATE INDEX IF NOT EXISTS sessions_role_at ON sessions (role, at);
CREATE INDEX IF NOT EXISTS sessions_at ON sessions (at);

-- allowance: the last valid reading of each allowance window a run
-- reported (#891 slice 2). Same writers as sessions, from the same
-- metadata; `at` and `role` live in sessions once, joined on
-- (run_id, attempt). Read by `sessions sql`; the worker's /budget and
-- retrospect's budget footer move onto it in slice 2b, and the allowance
-- suffix on artifact names dies with that move.
CREATE TABLE IF NOT EXISTS allowance (
  run_id      INTEGER NOT NULL,
  attempt     INTEGER NOT NULL,
  window      TEXT    NOT NULL,   -- five_hour, seven_day, seven_day_overage_included: the API's own names
  utilization REAL    NOT NULL,   -- fraction of the window used, in [0, 1]
  resets_at   INTEGER,            -- epoch seconds the window turns over; NULL when the reading carried none valid
  overage     INTEGER,            -- 1 when the reading said isUsingOverage, 0 when it said not, NULL when silent
  PRIMARY KEY (run_id, attempt, window)
);
