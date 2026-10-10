// Fixtures for session-row.py, the audit action's write into the session
// table (ADR-0060). Two invariants: the row is the whitelist and nothing
// else, and nothing here can fail the run it observes.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/session-row.py`;

const metadata = {
  role: "reviewer",
  trigger: { event: "pull_request", actor: "claude[bot]", number: "893" },
  commit: "1ef993a0000000000000000000000000000000000",
  ref: "refs/pull/893/merge",
  run_id: "37148994173",
  run_attempt: "2",
  is_error: false,
  telemetry: {
    num_turns: 23,
    duration_ms: 420000,
    stop_reason: "end_turn",
    usage: { output_tokens: 5000, output_tokens_details: { thinking_tokens: 1000 } },
    modelUsage: {
      "claude-sonnet-5-5": { outputTokens: 4983, costUSD: 0.42, costBasis: "list" },
      "claude-haiku-4-5-20251001": { outputTokens: 17, costUSD: 0.01, costBasis: "list" },
    },
    permission_denials: { count: 2, tools: ["Edit", "Write"] },
  },
  persona: { path: "agents/personas/reviewer.md", exists: true, bytes: 2000, sha256: "abc123" },
  prompt_extracted: true,
  response_extracted: true,
};

function auditDir(meta, files = {}) {
  const dir = mkdtempSync(join(tmpdir(), "audit-"));
  writeFileSync(join(dir, "metadata.json"), JSON.stringify(meta));
  writeFileSync(join(dir, "input-prompt.md"), files.prompt ?? "p".repeat(1234));
  writeFileSync(join(dir, "output-response.md"), files.response ?? "r".repeat(567));
  return dir;
}

// Serve programmed D1 answers, one per request in order with the last
// repeating, and record what the script sent.
async function withStub(answer, fn) {
  const answers = Array.isArray(answer) ? answer : [answer];
  const seen = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const a = answers[Math.min(seen.length, answers.length - 1)];
      seen.push(JSON.parse(body));
      res.writeHead(a.status ?? 200, { "content-type": "application/json" });
      res.end(JSON.stringify(a.payload ?? { success: true, result: [{ success: true, results: [] }] }));
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    return await fn(`http://127.0.0.1:${server.address().port}`, seen);
  } finally {
    server.close();
  }
}

function run(args, env) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script, ...args], {
      env: { ...process.env, CLOUDFLARE_API_TOKEN: "test-token", ...env },
    });
    let out = "";
    let err = "";
    child.stdout.on("data", (c) => (out += c));
    child.stderr.on("data", (c) => (err += c));
    child.on("close", (code) => resolve({ code, out, err }));
  });
}

test("writes the whitelist as one INSERT OR REPLACE, in column order", async () => {
  const dir = auditDir(metadata);
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(r.out, /^session-row: wrote run 37148994173 attempt 2 for reviewer$/m);
      assert.equal(seen.length, 1);
      assert.match(seen[0].sql, /^INSERT OR REPLACE INTO sessions \(run_id, attempt, at, role, /);
      const p = seen[0].params;
      assert.equal(p.length, 21);
      assert.equal(p[0], 37148994173);
      assert.equal(p[1], 2);
      assert.match(p[2], /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
      assert.deepEqual(p.slice(3, 8), ["reviewer", "pull_request", "claude[bot]", 893, metadata.commit]);
      assert.equal(p[8], 1); // measured
      assert.equal(p[9], "claude-sonnet-5-5"); // the model that did the work, not the 17-token one
      assert.equal(p[10], 5000); // out_tokens, summed over modelUsage
      assert.equal(p[11], 20); // think_pct
      assert.equal(p[12], 0.43); // cost_usd, list-basis entries summed
      assert.deepEqual(p.slice(13, 19), [23, 420000, 2, 0, "end_turn", "abc123"]);
      assert.deepEqual(p.slice(19), [1234, 567]);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// Both payload shapes allowance.py names, in the order the API moved through
// them: one flat window, then both windows nested. Last wins per window.
const readings = [
  { rateLimitType: "five_hour", utilization: 0.03, resetsAt: 1791990000 },
  {
    rateLimitType: "seven_day",
    unifiedWindows: {
      five_hour: { utilization: 0.07, resetsAt: "soon" }, // valid fraction, junk reset: a row with no reset
      seven_day: { utilization: 0.28, resetsAt: 1792029600, isUsingOverage: false },
      seven_day_overage_included: { utilization: 0.36, resetsAt: 1792029600, isUsingOverage: true },
    },
  },
  { unifiedWindows: { seven_day: { utilization: 1.5, resetsAt: 1792029600 } } }, // no valid fraction: not a reading
];

test("the last valid reading per allowance window follows the session row, one statement", async () => {
  const dir = auditDir({ ...metadata, rate_limit_events: readings });
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(
        r.out,
        /^session-row: wrote run 37148994173 attempt 2 for reviewer, allowance five_hour seven_day seven_day_overage_included$/m,
      );
      assert.equal(seen.length, 2);
      assert.match(seen[0].sql, /^INSERT OR REPLACE INTO sessions /);
      assert.equal(
        seen[1].sql,
        "INSERT OR REPLACE INTO allowance (run_id, attempt, window, utilization, resets_at, overage) "
          + "VALUES (?, ?, ?, ?, ?, ?), (?, ?, ?, ?, ?, ?), (?, ?, ?, ?, ?, ?)",
      );
      assert.deepEqual(seen[1].params, [
        37148994173,
        2,
        "five_hour",
        0.07,
        null,
        null, // the nested reading replaced the flat one
        37148994173,
        2,
        "seven_day",
        0.28,
        1792029600,
        0,
        37148994173,
        2,
        "seven_day_overage_included",
        0.36,
        1792029600,
        1,
      ]);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a failed allowance write is its own warning after the session row, exit 0", async () => {
  const dir = auditDir({ ...metadata, rate_limit_events: readings });
  try {
    await withStub(
      [{}, { status: 500, payload: { success: false, errors: [{ message: "D1 is down" }] } }],
      async (base, seen) => {
        const r = await run([dir], { D1_API_BASE: base });
        assert.equal(r.code, 0, r.err);
        assert.equal(seen.length, 2);
        assert.match(r.out, /^::warning::session-row: allowance not written: HTTP 500/m);
        assert.match(r.out, /^session-row: wrote run 37148994173 attempt 2 for reviewer$/m);
      },
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("an unmeasured run keeps its numbers absent, not zero", async () => {
  const meta = { ...metadata, telemetry: {}, is_error: null, prompt_extracted: false, response_extracted: false };
  const dir = auditDir(meta);
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      const p = seen[0].params;
      assert.equal(p[8], 0); // measured
      assert.deepEqual(p.slice(9, 16), [null, null, null, null, null, null, 0]);
      assert.deepEqual(p.slice(19), [null, null]); // placeholder texts are not sizes
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a failed write is a warning and exit 0, never a failed run", async () => {
  const dir = auditDir(metadata);
  try {
    await withStub({ status: 503, payload: { success: false, errors: [{ message: "unavailable" }] } }, async (base) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0);
      assert.match(r.out, /^::warning::session-row: not written: HTTP 503/m);
      assert.doesNotMatch(r.out + r.err, /test-token/);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("no token means skipped, one line, no network", async () => {
  const dir = auditDir(metadata);
  try {
    const r = await run([dir], { CLOUDFLARE_API_TOKEN: "", D1_API_BASE: "http://127.0.0.1:9" });
    assert.equal(r.code, 0);
    assert.equal(r.out.trim(), "session-row: skipped, no CLOUDFLARE_API_TOKEN");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a missing or malformed metadata.json is a warning, not a crash", async () => {
  const dir = mkdtempSync(join(tmpdir(), "audit-"));
  try {
    let r = await run([dir], { D1_API_BASE: "http://127.0.0.1:9" });
    assert.equal(r.code, 0);
    assert.match(r.out, /^::warning::session-row: not written: /m);
    writeFileSync(join(dir, "metadata.json"), "[]");
    r = await run([dir], { D1_API_BASE: "http://127.0.0.1:9" });
    assert.equal(r.code, 0);
    assert.match(r.out, /not written: metadata.json is not an object/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
