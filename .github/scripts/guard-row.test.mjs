// Fixtures for guard-row.py, the run gate's write into the guard table
// (#891 slice 3). Session-row's two invariants: the row is the whitelist
// and nothing else, and nothing here can fail the run it observes.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/guard-row.py`;

// A second-gear skip of a clock wake, the row the table exists for: no
// session row will follow it.
const skipped = {
  GITHUB_RUN_ID: "38123955522",
  GITHUB_RUN_ATTEMPT: "2",
  GITHUB_EVENT_NAME: "workflow_dispatch",
  ROLE: "herald",
  DISCRETIONARY: "true",
  GUARD_DECISION: "skip",
  GUARD_REASON:
    "SKIP: discretionary wake and projected exhaustion 2026-10-14 12:00 UTC misses reset 2026-10-15 02:00 UTC at week-average 0.80%/h; keeping the first 25% of each day and this wake is at 2026-10-11 08:11 UTC",
  GUARD_RUNG: "",
  GUARD_EFFORT: "",
  GUARD_PROJECTION: "0.800",
};

const INSERT =
  "INSERT OR REPLACE INTO guard (run_id, attempt, at, role, event, discretionary, decision, reason, rung, effort, projection) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

// Serve one programmed D1 answer and record what the script sent.
async function withStub(answer, fn) {
  const seen = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      seen.push(JSON.parse(body));
      res.writeHead(answer.status ?? 200, { "content-type": "application/json" });
      res.end(JSON.stringify(answer.payload ?? { success: true, result: [{ success: true, results: [] }] }));
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    return await fn(`http://127.0.0.1:${server.address().port}`, seen);
  } finally {
    server.close();
  }
}

function run(env) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script], {
      env: { ...process.env, CLOUDFLARE_API_TOKEN: "test-token", D1_DATABASE: "db", ...skipped, ...env },
    });
    let out = "";
    let err = "";
    child.stdout.on("data", (c) => (out += c));
    child.stderr.on("data", (c) => (err += c));
    child.on("close", (code) => resolve({ code, out, err }));
  });
}

test("a skipped wake writes the whitelist row: discretionary as 1, empty rung and effort as NULL", async () => {
  await withStub({}, async (base, seen) => {
    const r = await run({ D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 1);
    assert.equal(seen[0].sql, INSERT);
    const p = seen[0].params;
    assert.equal(p.length, 11);
    assert.equal(p[0], 38123955522);
    assert.equal(p[1], 2);
    assert.match(p[2], /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
    assert.deepEqual(p.slice(3, 7), ["herald", "workflow_dispatch", 1, "skip"]);
    assert.match(p[7], /^SKIP: discretionary wake/);
    assert.deepEqual(p.slice(8), [null, null, 0.8]);
    assert.match(r.out, /^guard-row: wrote run 38123955522 attempt 2 for herald: skip$/m);
  });
});

test("a wake that runs carries its rung and effort; no projection is NULL, an event wake is 0", async () => {
  await withStub({}, async (base, seen) => {
    const r = await run({
      D1_API_BASE: base,
      DISCRETIONARY: "false",
      GUARD_DECISION: "run",
      GUARD_REASON: "claude-sonnet-5-5 (ladder: claude-sonnet-5-5 > claude-haiku-5-5)",
      GUARD_RUNG: "claude-sonnet-5-5",
      GUARD_EFFORT: "medium",
      GUARD_PROJECTION: "",
    });
    assert.equal(r.code, 0, r.err);
    const p = seen[0].params;
    assert.equal(p[5], 0);
    assert.equal(p[6], "run");
    assert.deepEqual(p.slice(8), ["claude-sonnet-5-5", "medium", null]);
  });
});

test("no token: one line, no request, exit 0", async () => {
  await withStub({}, async (base, seen) => {
    const r = await run({ D1_API_BASE: base, CLOUDFLARE_API_TOKEN: "" });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 0);
    assert.equal(r.out.trim(), "guard-row: skipped, no CLOUDFLARE_API_TOKEN");
  });
});

test("a database that refuses the write is a warning, token-free, exit 0", async () => {
  const payload = { success: false, errors: [{ message: "no such table: guard" }] };
  await withStub({ status: 200, payload }, async (base) => {
    const r = await run({ D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.match(r.out, /^::warning::guard-row: not written: Unreadable: no such table: guard$/m);
    assert.doesNotMatch(r.out + r.err, /test-token/);
  });
});

test("a decision outside the schema's five is a warning before any request, exit 0", async () => {
  await withStub({}, async (base, seen) => {
    const r = await run({ D1_API_BASE: base, GUARD_DECISION: "maybe" });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 0);
    assert.match(
      r.out,
      /::warning::guard-row: not written: ValueError: GUARD_DECISION 'maybe' is none of run, skip, exhausted, paused, stale/,
    );
  });
});

test("a missing run id is a warning, not a traceback", async () => {
  await withStub({}, async (base, seen) => {
    const r = await run({ D1_API_BASE: base, GITHUB_RUN_ID: "" });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 0);
    assert.match(r.out, /::warning::guard-row: not written: ValueError/);
    assert.equal(r.err, "");
  });
});
