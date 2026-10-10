-- The session table (ADR-0060): one row per agent run, written by
-- .github/actions/agent-audit through session-row.py, read by
-- .github/scripts/sessions. Identifiers and API-authored numbers only;
-- the run's prose lives in its audit artifact for 90 days.
--
-- Applied with `.github/scripts/sessions migrate`, idempotent. A new
-- column is an ALTER TABLE line appended below, never an edit above it,
-- so the file replays on an existing database.
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
