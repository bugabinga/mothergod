// Fixtures for `sessions`, the SQL reader of the session table (ADR-0060).
//
// The first assertion is the one the script exists for: a database that
// cannot be read prints UNREADABLE and exits non-zero, never a table. The
// stub stands in for Cloudflare's D1 endpoint through D1_API_BASE, the same
// seam inbox.test.mjs uses for KV; nothing in production sets it.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/sessions`;

// Serve programmed D1 answers, one per request in order with the last
// repeating, record what the script sent, run `fn`.
async function withStub(answer, fn) {
  const answers = Array.isArray(answer) ? answer : [answer];
  const seen = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const a = answers[Math.min(seen.length, answers.length - 1)];
      seen.push({ url: req.url, auth: req.headers.authorization, body: JSON.parse(body || "{}") });
      res.writeHead(a.status ?? 200, { "content-type": "application/json" });
      res.end(JSON.stringify(a.payload ?? {}));
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
  await withStub(
    { status: 500, payload: { success: false, errors: [{ message: "edge down" }] } },
    async (base, seen) => {
      const r = await run(["sql", "SELECT 1"], { D1_API_BASE: base });
      assert.notEqual(r.code, 0);
      assert.match(r.out, /UNREADABLE/);
      assert.match(r.out, /HTTP 500/);
      assert.doesNotMatch(r.out, /no rows/);
      assert.doesNotMatch(r.out + r.err, /test-token/);
      assert.equal(seen.length, 1);
    },
  );
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
    const r = await run(["sql", "SELECT role, COUNT(*) runs FROM sessions WHERE at >= ?", "2026-10-01"], {
      D1_API_BASE: base,
    });
    assert.equal(r.code, 0, r.err);
    assert.deepEqual(seen[0].body, {
      sql: "SELECT role, COUNT(*) runs FROM sessions WHERE at >= ?",
      params: ["2026-10-01"],
    });
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

// The archive, for `backfill`: a stub gh answers the listing with `artifacts`
// and a zip download with the files `zips` holds for that id, or junk bytes
// for an id mapped to null. Every call lands in calls.log, so a test can say
// which artifacts were downloaded and which were skipped on their name.
function stubGh(artifacts, zips) {
  const dir = mkdtempSync(join(tmpdir(), "sessions-"));
  writeFileSync(
    join(dir, "gh"),
    `#!/usr/bin/env python3
import io, json, sys, zipfile
artifacts = json.loads(${JSON.stringify(JSON.stringify(artifacts))})
zips = json.loads(${JSON.stringify(JSON.stringify(zips))})
args = sys.argv[1:]
with open(${JSON.stringify(join(dir, "calls.log"))}, "a") as fh:
    fh.write(" ".join(args) + "\\n")
if args[:3] == ["api", "--paginate", "--slurp"] and args[3].startswith("repos/o/r/actions/artifacts?"):
    print(json.dumps([{"artifacts": artifacts}]))
elif args[0] == "api" and args[1].startswith("repos/o/r/actions/artifacts/") and args[1].endswith("/zip"):
    files = zips.get(args[1].split("/")[-2])
    if files is None:
        sys.stdout.buffer.write(b"not a zip")
        sys.exit(0)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as zf:
        for name, text in files.items():
            zf.writestr(name, text)
    sys.stdout.buffer.write(buf.getvalue())
else:
    sys.exit("stub gh: unexpected call " + " ".join(args))
`,
  );
  chmodSync(join(dir, "gh"), 0o755);
  return dir;
}

function downloads(dir) {
  let log = "";
  try {
    log = readFileSync(join(dir, "calls.log"), "utf8");
  } catch {
    return [];
  }
  return log
    .split("\n")
    .filter((l) => l.endsWith("/zip"))
    .map((l) => l.split("/").at(-2));
}

const metadata = (run_id, role) => ({
  role,
  trigger: { event: "pull_request", actor: "claude[bot]", number: "893" },
  commit: "1ef993a0000000000000000000000000000000000",
  run_id: String(run_id),
  run_attempt: "1",
  is_error: false,
  telemetry: {
    num_turns: 23,
    duration_ms: 420000,
    stop_reason: "end_turn",
    usage: { output_tokens: 5000, output_tokens_details: { thinking_tokens: 1000 } },
    modelUsage: { "claude-sonnet-5-5": { outputTokens: 5000, costUSD: 0.42, costBasis: "list" } },
    permission_denials: { count: 2, tools: ["Edit", "Write"] },
  },
  persona: { sha256: "abc123" },
  prompt_extracted: true,
  response_extracted: true,
});

const artifact = (id, name, created_at, expired = false) => ({ id, name, created_at, expired });

const listing = [
  artifact(1, "audit-bdfl-100-1", "2026-08-22T08:14:32Z"),
  artifact(2, "audit-reviewer-200-1-u2400-r1792029600", "2026-09-01T00:00:00Z"),
  artifact(3, "audit-herald-300-2", "2026-09-02T00:00:00Z"),
  artifact(4, "audit-curator-50-1", "2026-07-01T00:00:00Z", true),
  artifact(5, "site-shots-1", "2026-09-03T00:00:00Z"),
];
const zips = {
  2: {
    "metadata.json": JSON.stringify(metadata(200, "reviewer")),
    "input-prompt.md": "p".repeat(1234),
    "output-response.md": "r".repeat(567),
  },
  3: null,
};
const keys = (pairs) => d1(pairs.map(([run_id, attempt]) => ({ run_id, attempt })));

test("backfill writes the rows the archive has and the table lacks, and names the gap", async () => {
  const gh = stubGh(listing, zips);
  await withStub([keys([[100, 1]]), d1([]), keys([[100, 1], [200, 1]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 1, r.out + r.err);
    assert.equal(seen[0].body.sql, "SELECT run_id, attempt FROM sessions");
    assert.match(seen[1].body.sql, /^INSERT OR REPLACE INTO sessions \(run_id, attempt, at, role, /);
    const p = seen[1].body.params;
    assert.equal(p.length, 21);
    assert.deepEqual(p.slice(0, 4), [200, 1, "2026-09-01T00:00:00Z", "reviewer"]); // at is the artifact's creation
    assert.deepEqual(p.slice(19), [1234, 567]); // sizes read from the zip
    assert.equal(seen[2].body.sql, "SELECT run_id, attempt FROM sessions");
    assert.equal(seen.length, 3);
    // Sorted: eight workers download concurrently, so the stub's log holds no order.
    assert.deepEqual(downloads(gh).sort(), ["2", "3"]); // the present row cost no download; the expired and the non-audit never listed
    assert.match(r.out, /^  unreadable audit-herald-300-2: /m);
    assert.match(
      r.out,
      /^sessions backfill: artifacts 3 since 2026-08-22T08:14:32Z \| present 1 \| written 1 \| unreadable 1 \| missing 1$/m,
    );
  });
});

test("backfill on a table equal to the archive downloads nothing and exits 0", async () => {
  const gh = stubGh(listing.slice(0, 1), {});
  await withStub([keys([[100, 1]]), keys([[100, 1]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 0, r.out + r.err);
    assert.equal(seen.length, 2);
    assert.deepEqual(downloads(gh), []);
    assert.match(
      r.out,
      /artifacts 1 since 2026-08-22T08:14:32Z \| present 1 \| written 0 \| unreadable 0 \| missing 0$/m,
    );
  });
});

test("backfill --since keeps the artifacts created from that instant", async () => {
  const gh = stubGh(listing.slice(0, 2), zips);
  await withStub([keys([]), d1([]), keys([[200, 1]])], async (base, seen) => {
    const r = await run(["backfill", "--since", "2026-09-01", "--repo", "o/r"], {
      D1_API_BASE: base,
      PATH: `${gh}:${process.env.PATH}`,
      TZ: "America/New_York", // a date-only --since is UTC midnight, never the machine's
    });
    assert.equal(r.code, 0, r.out + r.err);
    assert.deepEqual(downloads(gh), ["2"]);
    assert.equal(seen[1].body.params[0], 200);
    assert.match(
      r.out,
      /artifacts 1 since 2026-09-01T00:00:00Z \| present 0 \| written 1 \| unreadable 0 \| missing 0$/m,
    );
  });
});

test("backfill on an unreadable table stops before any download", async () => {
  const gh = stubGh(listing, zips);
  await withStub({ status: 500, payload: { success: false, errors: [{ message: "edge down" }] } }, async (base) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.notEqual(r.code, 0);
    assert.match(r.out, /UNREADABLE: HTTP 500/);
    assert.deepEqual(downloads(gh), []);
  });
});

test("backfill without a repository says what it needs and exits 2", async () => {
  const r = await run(["backfill"], { GITHUB_REPOSITORY: "", D1_API_BASE: "http://127.0.0.1:9" });
  assert.equal(r.code, 2);
  assert.match(r.out, /backfill needs GITHUB_REPOSITORY or --repo/);
});

// One bad artifact must cost one `unreadable`, not the walk: oldest first, a
// poison artifact that aborted the verb would abort every rerun at the same
// place and starve the rows after it.
test("backfill counts a malformed or hostile artifact unreadable and writes the rest", async () => {
  const gh = stubGh(listing.slice(0, 3), {
    1: { "metadata.json": JSON.stringify({ ...metadata(100, "bdfl"), telemetry: "x" }) },
    2: zips[2],
    3: { "metadata.json": "[".repeat(100000) },
  });
  await withStub([keys([]), d1([]), keys([[200, 1]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 1, r.out + r.err);
    assert.equal(seen[1].body.params[0], 200);
    assert.match(r.out, /^  unreadable audit-bdfl-100-1: metadata has a malformed field/m);
    assert.match(r.out, /^  unreadable audit-herald-300-2: /m);
    assert.match(
      r.out,
      /artifacts 3 since 2026-08-22T08:14:32Z \| present 0 \| written 1 \| unreadable 2 \| missing 2$/m,
    );
  });
});

test("a flag without its value prints the usage line and exits 2", async () => {
  for (const args of [["backfill", "--since"], ["backfill", "--repo"], ["roles", "--since"]]) {
    const r = await run(args, { GITHUB_REPOSITORY: "o/r", D1_API_BASE: "http://127.0.0.1:9" });
    assert.equal(r.code, 2, `${args}: ${r.out}${r.err}`);
    assert.match(r.out, new RegExp(`${args.at(-1)} takes a value`));
    assert.match(r.out, /^Usage: sessions /m);
  }
});
