// Fixtures for trust-telemetry.py, the trust ledger's read of the trust
// table (#449; #891 slice 4). The contract is run-telemetry.py's: a read
// that failed writes a JSON saying what it saw and exits 0, never zeros the
// status page would render as measurements.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/trust-telemetry.py`;

const row = (run_id, at, role, fields) => ({
  run_id,
  at,
  role,
  fuzz_cpu_s: null,
  crashers_new: null,
  mutation_score: null,
  coverage_region_pct: null,
  ...fields,
});

// Two writers interleaved, in `at` order as the SELECT returns them; the
// first fuzz row predates measured crashers, so its count is NULL.
const rows = [
  row(100, "2026-08-29T02:40:00Z", "fuzz", { fuzz_cpu_s: 1800 }),
  row(200, "2026-09-07T05:00:00Z", "coverage", { coverage_region_pct: 96.5 }),
  row(300, "2026-09-20T02:40:00Z", "fuzz", { fuzz_cpu_s: 1986, crashers_new: 1 }),
  row(400, "2026-10-05T05:00:00Z", "coverage", { coverage_region_pct: 97.38 }),
  row(500, "2026-10-11T02:47:10Z", "fuzz", { fuzz_cpu_s: 2008, crashers_new: 0 }),
];

async function withD1(answer, fn) {
  const seen = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      seen.push(JSON.parse(body));
      res.writeHead(answer.status ?? 200, { "content-type": "application/json" });
      res.end(
        JSON.stringify(answer.payload ?? { success: true, result: [{ success: true, results: answer.rows ?? [] }] }),
      );
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
  const out = join(mkdtempSync(join(tmpdir(), "trust-telemetry-")), "trust-data.json");
  return new Promise((resolve) => {
    const child = spawn("python3", [script, out], {
      env: { ...process.env, CLOUDFLARE_API_TOKEN: "test-token", D1_DATABASE: "db", ...env },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (c) => (stdout += c));
    child.stderr.on("data", (c) => (stderr += c));
    child.on("close", (code) => resolve({ code, stdout, stderr, json: JSON.parse(readFileSync(out, "utf8")) }));
  });
}

test("the table aggregates: hours summed over every row, trend from the last two that measured, since the first row", async () => {
  await withD1({ rows }, async (base, seen) => {
    const r = await run({ D1_API_BASE: base });
    assert.equal(r.code, 0, r.stderr);
    assert.equal(seen.length, 1);
    assert.match(seen[0].sql, /^SELECT .* FROM trust ORDER BY at, run_id, attempt$/);
    const j = r.json;
    assert.equal(j.count, 5);
    assert.equal(j.since, "2026-08-29T02:40:00Z");
    assert.equal(j.cumulative_fuzz_cpu_hours, (1800 + 1986 + 2008) / 3600);
    assert.equal(j.crashers_total, 1);
    assert.deepEqual(j.trend, {
      fuzz_cpu_s: { latest: 2008, previous: 1986 },
      crashers_new: { latest: 0, previous: 1 },
      mutation_score: { latest: null, previous: null },
      coverage_region_pct: { latest: 97.38, previous: 96.5 },
    });
    assert.equal(j.entries.length, 5);
    // The entry shape the page and the writers share: run_id a string.
    assert.deepEqual(j.entries[0], {
      date: "2026-08-29T02:40:00Z",
      run_id: "100",
      role: "fuzz",
      fuzz_cpu_s: 1800,
      crashers_new: null,
      mutation_score: null,
      coverage_region_pct: null,
    });
    assert.equal(j.error, undefined);
    assert.match(r.stdout, /^trust-telemetry: 5 entries since 2026-08-29T02:40:00Z$/m);
  });
});

test("the entries tail is the newest 25; the count and the sums cover every row", async () => {
  const many = Array.from(
    { length: 30 },
    (_, i) =>
      row(1000 + i, `2026-09-${String(1 + (i % 28)).padStart(2, "0")}T0${i % 10}:00:00Z`, "fuzz", { fuzz_cpu_s: 3600 }),
  );
  await withD1({ rows: many }, async (base) => {
    const r = await run({ D1_API_BASE: base });
    assert.equal(r.code, 0, r.stderr);
    assert.equal(r.json.count, 30);
    assert.equal(r.json.entries.length, 25);
    assert.equal(r.json.cumulative_fuzz_cpu_hours, 30);
  });
});

test("an empty table is named, never a ledger of zeros", async () => {
  await withD1({ rows: [] }, async (base) => {
    const r = await run({ D1_API_BASE: base });
    assert.equal(r.code, 0, r.stderr);
    assert.match(r.json.error, /^The trust table is empty: no scheduled test workflow has written a row/);
    assert.equal(r.json.count, 0);
    assert.deepEqual(r.json.entries, []);
    assert.equal(r.json.cumulative_fuzz_cpu_hours, undefined);
  });
});

test("no token, and a database that refuses the read: the error names it, token-free, exit 0", async () => {
  await withD1({ rows }, async (base) => {
    const r = await run({ D1_API_BASE: base, CLOUDFLARE_API_TOKEN: "" });
    assert.equal(r.code, 0, r.stderr);
    assert.equal(r.json.error, "Could not read the trust table (CLOUDFLARE_API_TOKEN is unset).");
    assert.equal(r.json.count, 0);
  });
  const payload = { success: false, errors: [{ message: "no such table: trust" }] };
  await withD1({ status: 200, payload }, async (base) => {
    const r = await run({ D1_API_BASE: base });
    assert.equal(r.code, 0, r.stderr);
    assert.equal(r.json.error, "Could not read the trust table (no such table: trust).");
    assert.doesNotMatch(JSON.stringify(r.json) + r.stdout + r.stderr, /test-token/);
  });
});
