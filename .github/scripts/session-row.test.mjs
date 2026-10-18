// Fixtures for session-row.py, the audit action's write into the session
// table (ADR-0060). Two invariants: the row is the whitelist and nothing
// else, and nothing here can fail the run it observes.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/session-row.py`;
const schemaSql = readFileSync(join(scriptsDir, "..", "..", "infra", "sessions", "schema.sql"), "utf8");

// The rows the write the stub saw produces, by the parser D1 runs: the
// script executed against the real schema in an in-memory SQLite, then
// `rows("sessions")`, in insertion order. A literal rendered wrong fails
// here, where a regex over the script's text would read it as fine.
function applied(seen) {
  const db = new DatabaseSync(":memory:");
  db.exec(schemaSql);
  for (const body of seen) if (body.sql.startsWith("INSERT")) db.exec(body.sql);
  return (table) => db.prepare(`SELECT * FROM ${table} ORDER BY rowid`).all().map((r) => ({ ...r }));
}

// The statements of the script, each cut at its column list.
const heads = (sql) => sql.split(";\n").map((s) => s.replace(/ \(.*$/s, ""));

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
    tools: { offered: ["Bash", "Edit", "Write"], used: { Bash: 20, Edit: 3 }, writes: 3 },
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

test("writes the whitelist as one INSERT OR REPLACE, in column order, one call for the run", async () => {
  const dir = auditDir(metadata);
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(r.out, /^session-row: wrote run 37148994173 attempt 2 for reviewer, tools 3$/m);
      assert.equal(seen.length, 1); // one call (#1355): the tools rows, then the session row, last (#1102)
      assert.deepEqual(heads(seen[0].sql), ["INSERT OR REPLACE INTO tools", "INSERT OR REPLACE INTO sessions"]);
      assert.deepEqual(seen[0].params, []); // literals inlined: the endpoint binds none across statements
      const [row] = applied(seen)("sessions");
      assert.equal(Object.keys(row).length, 22);
      assert.equal(row.run_id, 37148994173);
      assert.equal(row.attempt, 2);
      assert.match(row.at, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
      assert.deepEqual(
        [row.role, row.event, row.actor, row.number, row.commit_sha],
        ["reviewer", "pull_request", "claude[bot]", 893, metadata.commit],
      );
      assert.equal(row.measured, 1);
      assert.equal(row.model, "claude-sonnet-5-5"); // the model that did the work, not the 17-token one
      assert.equal(row.out_tokens, 5000); // summed over modelUsage
      assert.equal(row.think_pct, 20);
      assert.equal(row.cost_usd, 0.43); // list-basis entries summed
      assert.deepEqual(
        [row.turns, row.duration_ms, row.denials, row.error, row.stop_reason, row.persona_sha],
        [23, 420000, 2, 0, "end_turn", "abc123"],
      );
      assert.deepEqual([row.prompt_bytes, row.response_bytes, row.writes], [1234, 567, 3]); // sizes, then writes (#896 item 5)
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// An artifact from before audit-extract.py counted writes carries no key:
// its row says null, the visible unknown, never 0, which would read as a
// run that changed nothing.
test("an artifact without a writes count writes null, not zero", async () => {
  const { tools, ...telemetry } = metadata.telemetry;
  const dir = auditDir({ ...metadata, telemetry });
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.equal(applied(seen)("sessions")[0].writes, null);
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

test("the last valid reading per allowance window precedes the session row, one statement", async () => {
  const dir = auditDir({ ...metadata, rate_limit_events: readings });
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(
        r.out,
        /^session-row: wrote run 37148994173 attempt 2 for reviewer, allowance five_hour seven_day seven_day_overage_included, tools 3$/m,
      );
      assert.equal(seen.length, 1);
      assert.deepEqual(heads(seen[0].sql), [
        "INSERT OR REPLACE INTO allowance",
        "INSERT OR REPLACE INTO tools",
        "INSERT OR REPLACE INTO sessions", // last, after both derived tables
      ]);
      assert.deepEqual(applied(seen)("allowance"), [
        // the nested reading replaced the flat one
        { run_id: 37148994173, attempt: 2, window: "five_hour", utilization: 0.07, resets_at: null, overage: null },
        { run_id: 37148994173, attempt: 2, window: "seven_day", utilization: 0.28, resets_at: 1792029600, overage: 0 },
        {
          run_id: 37148994173,
          attempt: 2,
          window: "seven_day_overage_included",
          utilization: 0.36,
          resets_at: 1792029600,
          overage: 1,
        },
      ]);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// #1102: the session row is the script's last statement, so a call that
// fails midway leaves it unwritten and `sessions backfill`, which walks the
// runs absent from `sessions`, writes all four tables from the artifact on
// Sunday. Written first, the session row hid the gap for good.
test("a failed write leaves the session row unwritten for the backfill to heal, exit 0", async () => {
  const dir = auditDir({ ...metadata, rate_limit_events: readings });
  try {
    await withStub(
      [{ status: 500, payload: { success: false, errors: [{ message: "D1 is down" }] } }],
      async (base, seen) => {
        const r = await run([dir], { D1_API_BASE: base });
        assert.equal(r.code, 0, r.err);
        assert.equal(seen.length, 1); // the one write, which failed
        assert.equal(heads(seen[0].sql).at(-1), "INSERT OR REPLACE INTO sessions");
        assert.match(r.out, /^::warning::session-row: not written: Unreadable: HTTP 500/m);
        assert.doesNotMatch(r.out, /wrote run/);
      },
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// #891 slice 5: one row per tool the run called or was refused, counts
// where the artifact has them. The fixture names Edit and Write refused
// without a per-tool count (an artifact between #946 and that count), so
// their denials are null, the visible unknown; Bash, never refused, is 0.
test("one row per tool called or refused precedes the session row", async () => {
  const dir = auditDir(metadata);
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.deepEqual(heads(seen[0].sql), ["INSERT OR REPLACE INTO tools", "INSERT OR REPLACE INTO sessions"]);
      assert.deepEqual(applied(seen)("tools"), [
        { run_id: 37148994173, attempt: 2, tool: "Bash", uses: 20, denials: 0 },
        { run_id: 37148994173, attempt: 2, tool: "Edit", uses: 3, denials: null },
        { run_id: 37148994173, attempt: 2, tool: "Write", uses: 0, denials: null },
      ]);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a per-tool denial count fills the column where the artifact carries one", async () => {
  const telemetry = {
    ...metadata.telemetry,
    permission_denials: { count: 2, tools: ["Edit", "Write"], by_tool: { Edit: 1, Write: 1 } },
  };
  const dir = auditDir({ ...metadata, telemetry });
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.deepEqual(applied(seen)("tools"), [
        { run_id: 37148994173, attempt: 2, tool: "Bash", uses: 20, denials: 0 },
        { run_id: 37148994173, attempt: 2, tool: "Edit", uses: 3, denials: 1 },
        { run_id: 37148994173, attempt: 2, tool: "Write", uses: 0, denials: 1 },
      ]);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// A run's tools are capped at 100 names, the first by name, in one
// statement: a real run names a dozen, and a hostile artifact costs at most
// a hundred rows.
test("tools rows are capped at 100 names, one statement", async () => {
  const used = Object.fromEntries(Array.from({ length: 150 }, (_, i) => [`t${String(i).padStart(3, "0")}`, i]));
  const telemetry = {
    ...metadata.telemetry,
    permission_denials: { count: 0, tools: [] },
    tools: { offered: null, used, writes: 0 },
  };
  const dir = auditDir({ ...metadata, telemetry });
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(r.out, /for reviewer, tools 100$/m);
      assert.equal(heads(seen[0].sql).filter((h) => h === "INSERT OR REPLACE INTO tools").length, 1);
      const rows = applied(seen)("tools");
      assert.equal(rows.length, 100);
      assert.equal(rows[0].tool, "t000");
      assert.equal(rows[99].tool, "t099"); // the first 100 by name
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("a use count that is not a count is a warning, the session row still written", async () => {
  const telemetry = { ...metadata.telemetry, tools: { offered: null, used: { Bash: "lots" }, writes: 0 } };
  const dir = auditDir({ ...metadata, telemetry });
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.equal(seen.length, 1);
      assert.match(
        r.out,
        /^::warning::session-row: tools not written: ValueError: tools.used\['Bash'\] is not a count$/m,
      );
      assert.match(r.out, /^session-row: wrote run 37148994173 attempt 2 for reviewer$/m);
    });
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
      const [row] = applied(seen)("sessions");
      assert.equal(row.measured, 0);
      assert.deepEqual(
        [row.model, row.out_tokens, row.think_pct, row.cost_usd, row.turns, row.duration_ms, row.denials],
        [null, null, null, null, null, null, 0],
      );
      assert.deepEqual([row.prompt_bytes, row.response_bytes, row.writes], [null, null, null]); // placeholder texts are not sizes; no count, no writes
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
      assert.match(r.out, /^::warning::session-row: not written: Unreadable: HTTP 503/m);
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
    assert.match(r.out, /not written: ValueError: metadata.json is not an object/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// #1002: the live writer called row() bare, so a field the whitelist cannot
// turn into a number escaped as a traceback and exit 1 through the audit
// action, whose comment promises the step cannot redden a run.
test("a field row() cannot price is a warning and exit 0, no write attempted", async () => {
  const dir = mkdtempSync(join(tmpdir(), "audit-"));
  const meta = {
    ...metadata,
    telemetry: {
      ...metadata.telemetry,
      usage: { output_tokens: 5, output_tokens_details: { thinking_tokens: "BIG" } },
    },
  };
  // JSON.stringify has no 10**400; splice the literal in after serialising.
  writeFileSync(join(dir, "metadata.json"), JSON.stringify(meta).replace("\"BIG\"", "1" + "0".repeat(400)));
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(r.out, /^::warning::session-row: not written: OverflowError: /m);
      assert.doesNotMatch(r.out + r.err, /Traceback/);
      assert.equal(seen.length, 0);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// #1241: a wall the run hit is a row, named from the reasons the audit
// kept (#1110): the hook before the colon for a hook-path denial, the
// harness's decision type for one it decided itself. An entry of another
// shape is skipped, never a row of unknowns; two hits of one wall are one row.
test("one row per wall hit precedes the session row, named from the reasons", async () => {
  const meta = {
    ...metadata,
    telemetry: {
      ...metadata.telemetry,
      permission_denials: {
        count: 5,
        tools: ["Bash", "Edit", "Write"],
        by_tool: { Bash: 3, Edit: 1, Write: 1 },
        reasons: [
          { tool: "Bash", type: "hook", reason: "deny-unpushed-work: denied gh pr create with 2 files unpushed" },
          { tool: "Bash", type: "hook", reason: "deny-em-dash: denied" },
          { tool: "Edit", type: "rule", reason: "protected path .cargo/" },
          { tool: "Bash", type: "hook", reason: "deny-em-dash: denied again" },
          { tool: "Bash", type: "hook" },
          { tool: "Bash", type: "hook", reason: "" },
          { tool: 7, type: "mode", reason: "plan" },
        ],
      },
    },
  };
  const dir = auditDir(meta);
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(r.out, /^session-row: wrote run 37148994173 attempt 2 for reviewer, tools 3, walls 3$/m);
      assert.equal(seen.length, 1);
      assert.deepEqual(heads(seen[0].sql), [
        "INSERT OR REPLACE INTO tools",
        "INSERT OR REPLACE INTO walls",
        "INSERT OR REPLACE INTO sessions", // last
      ]);
      assert.deepEqual(applied(seen)("walls"), [
        { run_id: 37148994173, attempt: 2, tool: "Bash", wall: "deny-em-dash" },
        { run_id: 37148994173, attempt: 2, tool: "Bash", wall: "deny-unpushed-work" },
        { run_id: 37148994173, attempt: 2, tool: "Edit", wall: "rule" },
      ]);
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// #1355: the literals ride inline, and SQLite's string literal has one
// escape, the doubled quote. A `;` and a newline inside the text are text,
// not a statement boundary; a NUL is cut, because the parser reads it as
// the end of the whole script.
test("a text with a quote, a semicolon, a newline and a NUL is written as typed, the NUL cut", async () => {
  const telemetry = { ...metadata.telemetry, stop_reason: "it's; done\nnow\u0000" };
  const dir = auditDir({ ...metadata, role: "re'viewer", telemetry });
  try {
    await withStub({}, async (base, seen) => {
      const r = await run([dir], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.err);
      assert.match(r.out, /^session-row: wrote run 37148994173 attempt 2 for re'viewer, tools 3$/m);
      assert.equal(seen.length, 1);
      const [row] = applied(seen)("sessions");
      assert.equal(row.role, "re'viewer");
      assert.equal(row.stop_reason, "it's; done\nnow");
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
