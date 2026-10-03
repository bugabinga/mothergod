// Fixtures for `sessions`, the SQL reader of the session table (ADR-0060).
//
// The first assertion is the one the script exists for: a database that
// cannot be read prints UNREADABLE and exits non-zero, never a table. The
// stub stands in for Cloudflare's D1 endpoint through D1_API_BASE, the same
// seam inbox.test.mjs uses for KV; nothing in production sets it.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/sessions`;

// Serve one programmed D1 answer, record what the script sent, run `fn`.
async function withStub(answer, fn) {
  const seen = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      seen.push({ url: req.url, auth: req.headers.authorization, body: JSON.parse(body || "{}") });
      res.writeHead(answer.status ?? 200, { "content-type": "application/json" });
      res.end(JSON.stringify(answer.payload ?? {}));
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
    const child = spawn(script, args, {
      env: { ...process.env, CLOUDFLARE_API_TOKEN: "test-token", ...env },
    });
    let out = "";
    let err = "";
    child.stdout.on("data", (c) => (out += c));
    child.stderr.on("data", (c) => (err += c));
    child.on("close", (code) => resolve({ code, out, err }));
  });
}

const d1 = (rows) => ({ payload: { success: true, result: [{ success: true, results: rows }] } });

test("an unreadable database prints UNREADABLE and exits non-zero, never a table", async () => {
  await withStub({ status: 500, payload: { success: false, errors: [{ message: "edge down" }] } }, async (base, seen) => {
    const r = await run(["sql", "SELECT 1"], { D1_API_BASE: base });
    assert.notEqual(r.code, 0);
    assert.match(r.out, /UNREADABLE/);
    assert.match(r.out, /HTTP 500/);
    assert.doesNotMatch(r.out, /no rows/);
    assert.doesNotMatch(r.out + r.err, /test-token/);
    assert.equal(seen.length, 1);
  });
});

test("success:false from the API is unreadable too, with its message", async () => {
  await withStub({ payload: { success: false, errors: [{ message: "no such table: sessions" }] } }, async (base) => {
    const r = await run(["sql", "SELECT 1"], { D1_API_BASE: base });
    assert.notEqual(r.code, 0);
    assert.match(r.out, /UNREADABLE: no such table/);
  });
});

test("no token is unreadable, not empty", async () => {
  const r = await run(["sql", "SELECT 1"], { CLOUDFLARE_API_TOKEN: "" });
  assert.notEqual(r.code, 0);
  assert.match(r.out, /UNREADABLE: CLOUDFLARE_API_TOKEN is unset/);
});

test("sql sends the statement with its params and renders the rows aligned", async () => {
  const rows = [
    { role: "bdfl", runs: 12, cost_usd: 1.5 },
    { role: "reviewer", runs: 7, cost_usd: null },
  ];
  await withStub(d1(rows), async (base, seen) => {
    const r = await run(["sql", "SELECT role, COUNT(*) runs FROM sessions WHERE at >= ?", "2026-10-01"], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.deepEqual(seen[0].body, { sql: "SELECT role, COUNT(*) runs FROM sessions WHERE at >= ?", params: ["2026-10-01"] });
    assert.equal(seen[0].auth, "Bearer test-token");
    assert.match(seen[0].url, /\/d1\/database\/[0-9a-f-]+\/query$/);
    const lines = r.out.trimEnd().split("\n");
    assert.equal(lines[0], "role      runs  cost_usd");
    assert.equal(lines[2], "bdfl      12    1.50");
    assert.equal(lines[3], "reviewer  7");
  });
});

test("an empty result is a finding, printed as such", async () => {
  await withStub(d1([]), async (base) => {
    const r = await run(["sql", "SELECT * FROM sessions WHERE denials > 0"], { D1_API_BASE: base });
    assert.equal(r.code, 0);
    assert.equal(r.out.trim(), "(no rows)");
  });
});

test("roles aggregates per role with medians over measured runs only", async () => {
  const rows = [
    { role: "bdfl", measured: 1, turns: 10, out_tokens: 100, denials: 0, error: 0, cost_usd: 1 },
    { role: "bdfl", measured: 1, turns: 30, out_tokens: 300, denials: 2, error: 1, cost_usd: 2 },
    { role: "bdfl", measured: 0, turns: null, out_tokens: null, denials: 0, error: 0, cost_usd: null },
    { role: "reviewer", measured: 1, turns: 5, out_tokens: 50, denials: 0, error: 0, cost_usd: null },
  ];
  await withStub(d1(rows), async (base, seen) => {
    const r = await run(["roles", "--since", "14d"], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.match(seen[0].body.sql, /WHERE at >= \?/);
    assert.match(seen[0].body.params[0], /^\d{4}-\d{2}-\d{2}T/);
    assert.match(r.out, /^sessions since \S+: 4$/m);
    const bdfl = r.out.split("\n").find((l) => l.startsWith("bdfl"));
    assert.ok(bdfl, r.out);
    const cells = bdfl.trim().split(/\s+/);
    // role runs measured turns_p50 turns_max out_tokens denials errors cost_usd
    assert.deepEqual(cells, ["bdfl", "3", "2", "20.00", "30", "400", "2", "1", "3.00"]);
    const reviewer = r.out.split("\n").find((l) => l.startsWith("reviewer"));
    assert.deepEqual(reviewer.trim().split(/\s+/), ["reviewer", "1", "1", "5.00", "5", "50", "0", "0"]);
  });
});

test("roles rejects a window it cannot parse", async () => {
  const r = await run(["roles", "--since", "fortnight"], { D1_API_BASE: "http://127.0.0.1:9" });
  assert.notEqual(r.code, 0);
  assert.match(r.err, /--since takes Nd or an ISO date/);
});

test("show prints one run vertically, or says which run is missing", async () => {
  await withStub(d1([{ run_id: 42, attempt: 1, role: "herald", turns: 9 }]), async (base, seen) => {
    const r = await run(["show", "42"], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.deepEqual(seen[0].body.params, [42]);
    assert.match(r.out, /^run_id   42$/m);
    assert.match(r.out, /^role     herald$/m);
  });
  await withStub(d1([]), async (base) => {
    const r = await run(["show", "43"], { D1_API_BASE: base });
    assert.equal(r.code, 0);
    assert.equal(r.out.trim(), "no session with run id 43");
  });
});

test("migrate posts the schema file as one batch", async () => {
  await withStub(d1([]), async (base, seen) => {
    const r = await run(["migrate"], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.match(seen[0].body.sql, /CREATE TABLE IF NOT EXISTS sessions/);
    assert.match(seen[0].body.sql, /CREATE INDEX IF NOT EXISTS sessions_role_at/);
    assert.deepEqual(seen[0].body.params, []);
    assert.match(r.out, /schema applied/);
  });
});

test("an unknown verb prints the usage line and exits 2", async () => {
  const r = await run(["list"], {});
  assert.equal(r.code, 2);
  assert.match(r.out, /^Usage: sessions sql/);
});
