// Fixtures for vitals-sidecar.py, the row-a-minute writer a heartbeat job
// starts beside its session (#1665). Two invariants, guard-row's: the row is
// the schema's columns and nothing else, and nothing here can fail or hold
// the run it observes. The sampler reads a fixture /proc so the numbers are
// pinned; the disk field reads a real path, so only its sign is.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/vitals-sidecar.py`;

const INSERT =
  "INSERT OR REPLACE INTO vitals (run_id, attempt, at, mem_avail_mb, swap_free_mb, disk_avail_mb, load1, top_rss_mb, top_name) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

// A /proc as the 03:23Z heartbeat's would have read mid-kill: rustc the
// heaviest process, claude beside it, `self` and a vanished pid to skip.
function fixtureProc() {
  const proc = mkdtempSync(join(tmpdir(), "vitals-proc-"));
  writeFileSync(
    join(proc, "meminfo"),
    "MemTotal:       16373232 kB\nMemFree:          319488 kB\nMemAvailable:     409600 kB\nSwapTotal:       3145724 kB\nSwapFree:          12288 kB\n",
  );
  writeFileSync(join(proc, "loadavg"), "9.12 4.03 1.57 3/412 13404\n");
  mkdirSync(join(proc, "100"));
  writeFileSync(join(proc, "100", "status"), "Name:\trustc\nState:\tR (running)\nVmRSS:\t 6553600 kB\n");
  mkdirSync(join(proc, "101"));
  writeFileSync(join(proc, "101", "status"), "Name:\tclaude\nVmRSS:\t 1048576 kB\n");
  mkdirSync(join(proc, "102")); // a pid that exited before its status was read
  mkdirSync(join(proc, "self"));
  writeFileSync(join(proc, "self", "status"), "Name:\tpython3\nVmRSS:\t 99999999 kB\n");
  return proc;
}

// Serve one programmed D1 answer and record what the script sent, the
// bearer beside the body: the token reaches the request from stdin alone.
async function withStub(answer, fn) {
  const seen = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      seen.push({ ...JSON.parse(body), auth: req.headers.authorization });
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

// The token on stdin, one line, as the workflow step feeds it; `stdin`
// empty is a step that had none to give.
function run(args, env, stdin = "test-token\n") {
  return new Promise((resolve) => {
    const child = spawn("python3", [script, ...args], {
      env: {
        ...process.env,
        D1_DATABASE: "db",
        GITHUB_RUN_ID: "40263802968",
        GITHUB_RUN_ATTEMPT: "1",
        ...env,
      },
    });
    let out = "";
    let err = "";
    child.stdout.on("data", (c) => (out += c));
    child.stderr.on("data", (c) => (err += c));
    child.on("close", (code) => resolve({ code, out, err }));
    child.stdin.end(stdin);
  });
}

test("--once writes the schema's row from the fixture /proc: MB from kB, the heaviest pid by RSS, non-pids skipped", async () => {
  const proc = fixtureProc();
  await withStub({}, async (base, seen) => {
    const r = await run(["--once", "--proc", proc, "--root", proc], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 1);
    assert.equal(seen[0].sql, INSERT);
    assert.equal(seen[0].auth, "Bearer test-token");
    const p = seen[0].params;
    assert.equal(p.length, 9);
    assert.equal(p[0], 40263802968);
    assert.equal(p[1], 1);
    assert.match(p[2], /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
    assert.deepEqual(p.slice(3, 5), [400, 12]);
    assert.ok(Number.isInteger(p[5]) && p[5] > 0, `disk avail ${p[5]}`);
    assert.equal(p[6], 9.12);
    assert.deepEqual(p.slice(7), [6400, "rustc"]);
    assert.match(
      r.out,
      /^vitals-sidecar: wrote row 1 at \S+: mem avail 400 MB, disk avail \d+ MB, load 9\.12, heaviest rustc 6400 MB$/m,
    );
    assert.match(r.out, /^vitals-sidecar: done, 1 rows$/m);
  });
});

test("a /proc field it cannot read is NULL in its column, the row still written", async () => {
  const proc = mkdtempSync(join(tmpdir(), "vitals-proc-"));
  writeFileSync(join(proc, "loadavg"), "0.50 0.40 0.30 1/100 42\n");
  await withStub({}, async (base, seen) => {
    const r = await run(["--once", "--proc", proc, "--root", proc], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 1);
    const p = seen[0].params;
    assert.deepEqual([p[3], p[4], p[7], p[8]], [null, null, null, null]);
    assert.equal(p[6], 0.5);
    assert.match(r.out, /^vitals-sidecar: meminfo unread: FileNotFoundError/m);
  });
});

test("no token on stdin: one line, no request, exit 0; no run id the same", async () => {
  await withStub({}, async (base, seen) => {
    const r = await run(["--once"], { D1_API_BASE: base }, "");
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 0);
    assert.equal(r.out.trim(), "vitals-sidecar: skipped, no token on stdin");
    const env = { ...process.env, D1_API_BASE: base, D1_DATABASE: "db" };
    delete env.GITHUB_RUN_ID;
    const child = spawn("python3", [script, "--once"], { env });
    let out = "";
    child.stdout.on("data", (c) => (out += c));
    child.stdin.end("test-token\n");
    const code = await new Promise((resolve) => child.on("close", resolve));
    assert.equal(code, 0);
    assert.equal(out.trim(), "vitals-sidecar: skipped, no GITHUB_RUN_ID");
    assert.equal(seen.length, 0);
  });
});

test("the token is read from stdin alone: one in the environment is not used", async () => {
  const proc = fixtureProc();
  await withStub({}, async (base, seen) => {
    const r = await run(
      ["--once", "--proc", proc, "--root", proc],
      { D1_API_BASE: base, CLOUDFLARE_API_TOKEN: "env-token" },
      "stdin-token\n",
    );
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 1);
    assert.equal(seen[0].auth, "Bearer stdin-token");
  });
});

test("a database that refuses the write is one token-free line, the loop goes on, exit 0", async () => {
  const proc = fixtureProc();
  const payload = { success: false, errors: [{ message: "no such table: vitals" }] };
  await withStub({ status: 200, payload }, async (base, seen) => {
    const r = await run(["--once", "--proc", proc, "--root", proc], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 1);
    assert.match(r.out, /^vitals-sidecar: not written: Unreadable: no such table: vitals$/m);
    assert.match(r.out, /^vitals-sidecar: done, 0 rows$/m);
    assert.doesNotMatch(r.out + r.err, /test-token/);
  });
});

// The end is the assertion, never a row count: under a 60 ms budget a loaded
// runner wrote one row where two were pinned, 4 of 24 runs at four
// concurrent (the reviewer's repro on #1666), and `node --test` runs files
// in parallel.
test("--max-minutes ends the loop where --once is absent", async () => {
  const proc = fixtureProc();
  await withStub({}, async (base, seen) => {
    const r = await run(["--interval", "0.05", "--max-minutes", "0.01", "--proc", proc, "--root", proc], {
      D1_API_BASE: base,
    });
    assert.equal(r.code, 0, r.err);
    assert.ok(seen.length >= 1, `rows ${seen.length}`);
    assert.match(r.out, /^vitals-sidecar: done, \d+ rows$/m);
  });
});
