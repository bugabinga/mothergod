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
-- writes (#896 item 5): the run's tool calls that changed the project's
-- shared state, a post through gh-comment, a PR through gh-pr or
-- push-branch, a merge through merge-pr, a label, milestone or title edit
-- through gh issue edit or gh pr edit, a github_file_ops commit or delete,
-- each counted when its result was not an error; audit-extract.py
-- names the set. 0 is a run that changed nothing. NULL is an artifact
-- written before the column, which the backfill leaves as the visible
-- unknown. Read by trend.py as a seat's no-op share.
ALTER TABLE sessions ADD COLUMN writes INTEGER;

-- allowance: the last valid reading of each allowance window a run
-- reported (#891 slice 2). Same writers as sessions, from the same
-- metadata; `at` and `role` live in sessions once, joined on
-- (run_id, attempt). Read by `sessions sql` and by the Telegram worker's
-- /budget through the SESSIONS binding in infra/telegram-worker/wrangler.toml
-- (slice 2b). Retrospect's budget footer reads the same readings from the
-- artifacts it already downloads to judge the sessions, so it stays there.
CREATE TABLE IF NOT EXISTS allowance (
  run_id      INTEGER NOT NULL,
  attempt     INTEGER NOT NULL,
  window      TEXT    NOT NULL,   -- five_hour, seven_day, seven_day_overage_included: the API's own names
  utilization REAL    NOT NULL,   -- fraction of the window used, in [0, 1]
  resets_at   INTEGER,            -- epoch seconds the window turns over; NULL when the reading carried none valid
  overage     INTEGER,            -- 1 when the reading said isUsingOverage, 0 when it said not, NULL when silent
  PRIMARY KEY (run_id, attempt, window)
);

-- guard: one row per run gate evaluation (#891 slice 3). Written by
-- .github/actions/agent-guard through guard-row.py as the gate decides,
-- fail-open; read by `sessions sql`, where retrospect's "skipped N" and
-- the survey's HEALTH line ask which wakes did not run and why (#896).
-- A wake the gate stops never reaches agent-audit, so it has no sessions
-- row to join: `at` and `role` live here too.
CREATE TABLE IF NOT EXISTS guard (
  run_id        INTEGER NOT NULL,
  attempt       INTEGER NOT NULL,
  at            TEXT    NOT NULL,   -- ISO-8601 UTC, when the gate decided
  role          TEXT    NOT NULL,   -- the seat; '' when the caller named none
  event         TEXT,               -- github.event_name
  discretionary INTEGER NOT NULL,   -- 1 when only the clock asked for the wake (ADR-0039)
  decision      TEXT    NOT NULL,   -- run, skip (second gear), exhausted (no rung left), paused, stale
  reason        TEXT,               -- the gate's own log line for the decision, numbers included
  rung          TEXT,               -- the model chosen; NULL when the wake does not run or the action default serves
  effort        TEXT,               -- the --effort level chosen; NULL for the session default
  projection    REAL,               -- week-average seven-day burn in %/h when it misses the reset; NULL when it reaches it or there is no reading
  PRIMARY KEY (run_id, attempt)
);
CREATE INDEX IF NOT EXISTS guard_role_at ON guard (role, at);

-- trust: one row per trust-ledger entry a scheduled test workflow wrote
-- (#891 slice 4; the ledger is #449, ADR-0043). Written by fuzz-check.yml
-- and coverage-check.yml through trust-row.py beside the entry.json
-- artifact each uploads, and by `trust-row.py backfill` from the
-- unexpired artifacts every Sunday (agent-model-intel.yml); read by
-- trust-telemetry.py at site deploy, where "cumulative" means since the
-- first row and no longer the artifact retention window. A writer leaves
-- the fields it does not measure NULL, never 0.
CREATE TABLE IF NOT EXISTS trust (
  run_id              INTEGER NOT NULL,
  attempt             INTEGER NOT NULL,
  at                  TEXT    NOT NULL,   -- ISO-8601 UTC, the entry's own date
  role                TEXT    NOT NULL,   -- the writer: fuzz, coverage; mutation when #455 lands
  fuzz_cpu_s          REAL,               -- seconds the fuzz targets ran, timed around the run
  crashers_new        INTEGER,            -- crash-, oom-, timeout-, leak- files libFuzzer left; NULL before 2026-09-13T06:00:00Z, when slow units counted too
  mutation_score      REAL,
  coverage_region_pct REAL,               -- cargo-llvm-cov region coverage of the crate
  PRIMARY KEY (run_id, attempt)
);
CREATE INDEX IF NOT EXISTS trust_at ON trust (at);
