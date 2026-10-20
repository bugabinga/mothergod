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
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/sessions`;
const schemaSql = readFileSync(join(scriptsDir, "..", "..", "infra", "sessions", "schema.sql"), "utf8");

const d1 = (rows) => ({ payload: { success: true, result: [{ success: true, results: rows }] } });
const TABLES = "SELECT name FROM sqlite_master WHERE type = 'table'";
const present = {
  payload: { success: false, errors: [{ message: "D1_ERROR: duplicate column name: writes: SQLITE_ERROR" }] },
};
// A database that holds the whole schema: a CREATE IF NOT EXISTS answers
// applied, the ALTER answers the column present.
const whole = (sql) => (sql.startsWith("ALTER ") ? present : d1([]));

// Serve programmed D1 answers, one per request in order with the last
// repeating, record what the script sent, run `fn`. The schema's own
// traffic, which every backfill sends first (#1355), is answered apart so
// the ordered answers stay the walk's: a CREATE or ALTER by `ddl`, a
// function of the statement (`whole` by default) or answers in order, the
// table list before and after from `tables` (nothing new, by default),
// both recorded in `schema`. Every record carries `n`, its place in the
// one sequence.
async function withStub(answer, fn, { ddl = whole, tables = [[]] } = {}) {
  const answers = Array.isArray(answer) ? answer : [answer];
  const seen = [];
  const schema = [];
  let n = 0;
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      // Strict, as Cloudflare's parser is: a body that is not JSON (Python's
      // `Infinity`) is a 400 and a null body in `seen`, never a lenient read.
      let parsed = null;
      try {
        parsed = JSON.parse(body || "{}");
      } catch {
        seen.push({ url: req.url, auth: req.headers.authorization, body: null, n: n++ });
        res.writeHead(400, { "content-type": "application/json" });
        res.end("{}");
        return;
      }
      const record = { url: req.url, auth: req.headers.authorization, body: parsed, n: n++ };
      let a;
      if (/^(CREATE|ALTER) /.test(parsed.sql)) {
        a = typeof ddl === "function"
          ? ddl(parsed.sql)
          : ddl[Math.min(schema.filter((q) => q.body.sql !== TABLES).length, ddl.length - 1)];
        schema.push(record);
      } else if (parsed.sql === TABLES) {
        const nth = schema.filter((q) => q.body.sql === TABLES).length;
        a = d1(tables[Math.min(nth, tables.length - 1)].map((name) => ({ name })));
        schema.push(record);
      } else {
        a = answers[Math.min(seen.length, answers.length - 1)];
        seen.push(record);
      }
      res.writeHead(a.status ?? 200, { "content-type": "application/json" });
      res.end(JSON.stringify(a.payload ?? {}));
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    return await fn(`http://127.0.0.1:${server.address().port}`, seen, schema);
  } finally {
    server.close();
  }
}

// The rows the writes the stub saw produce, by the parser D1 runs: every
// INSERT script executed against the real schema in an in-memory SQLite,
// then `rows("sessions")`, in insertion order. A literal rendered wrong
// fails here, where a regex over the script's text would read it as fine.
function applied(seen) {
  const db = new DatabaseSync(":memory:");
  db.exec(schemaSql);
  for (const { body } of seen) if (body?.sql.startsWith("INSERT")) db.exec(body.sql);
  return (table) => db.prepare(`SELECT * FROM ${table} ORDER BY rowid`).all().map((r) => ({ ...r }));
}

// The statements of one call's script, each cut at its column list.
const heads = (sql) => sql.split(";\n").map((s) => s.replace(/ \(.*$/s, ""));

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

// 2026-10-20: two director wakes in a row typed a name the schema lacks and read
// UNREADABLE, the word for a store that is down. The refusal is the caller's
// and carries the tables, so the next statement is typed from the screen.
test("a statement the store refuses prints the tables and exits 2, never UNREADABLE", async () => {
  await withStub(
    {
      status: 400,
      payload: { success: false, errors: [{ code: 7500, message: "no such table: runs: SQLITE_ERROR" }] },
    },
    async (base) => {
      const r = await run(["sql", "SELECT turns FROM runs"], { D1_API_BASE: base });
      assert.equal(r.code, 2, r.out);
      assert.match(r.out, /statement refused: .*no such table: runs/);
      assert.doesNotMatch(r.out, /UNREADABLE/);
      assert.match(r.out, /tables, from infra\/sessions\/schema\.sql/);
      assert.match(r.out, /^    sessions\(run_id, attempt, at, role, event, .*, response_bytes, writes\)$/m);
      assert.match(
        r.out,
        /^    guard\(run_id, attempt, at, role, event, discretionary, decision, reason, rung, effort, projection\)$/m,
      );
      assert.match(r.out, /^    walls\(run_id, attempt, tool, wall\)$/m);
      assert.doesNotMatch(r.out, /sessions_role_at/);
    },
  );
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
    { role: "bdfl", measured: 1, turns: 10, out_tokens: 100, denials: 0, error: 0, cost_usd: 1, writes: 0 },
    { role: "bdfl", measured: 1, turns: 30, out_tokens: 300, denials: 2, error: 1, cost_usd: 2, writes: 4 },
    { role: "bdfl", measured: 0, turns: null, out_tokens: null, denials: 0, error: 0, cost_usd: null, writes: null },
    { role: "reviewer", measured: 1, turns: 5, out_tokens: 50, denials: 0, error: 0, cost_usd: null, writes: null },
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
    // role runs measured turns_p50 turns_max out_tokens denials errors noops recorded cost_usd
    assert.deepEqual(cells, ["bdfl", "3", "2", "20.00", "30", "400", "2", "1", "1", "2", "3.00"]);
    const reviewer = r.out.split("\n").find((l) => l.startsWith("reviewer"));
    assert.deepEqual(reviewer.trim().split(/\s+/), ["reviewer", "1", "1", "5.00", "5", "50", "0", "0", "0", "0"]);
  });
});

test("roles rejects a window it cannot parse", async () => {
  const r = await run(["roles", "--since", "fortnight"], { D1_API_BASE: "http://127.0.0.1:9" });
  assert.notEqual(r.code, 0);
  assert.match(r.err, /--since takes Nd or an ISO date/);
});

// 99999999999d overflows timedelta's C int; 1000000d constructs, then the
// subtraction lands before year 1. Both are the usage line, never a traceback (#1003).
test("roles rejects a window past the calendar with the usage line, not a traceback", async () => {
  for (const spec of ["99999999999d", "1000000d"]) {
    const r = await run(["roles", "--since", spec], { D1_API_BASE: "http://127.0.0.1:9" });
    assert.notEqual(r.code, 0, spec);
    assert.match(r.err, /--since takes Nd or an ISO date/, spec);
    assert.doesNotMatch(r.err, /Traceback|OverflowError/, spec);
  }
});

// Year 1 and year 9999 parse with a zone, and the shift to UTC then leaves the
// calendar: the third overflow in since_of, the usage line again (#1029).
test("roles rejects an ISO date whose UTC shift leaves the calendar with the usage line, not a traceback", async () => {
  for (const spec of ["0001-01-01T00:00:00+05:00", "9999-12-31T23:59:59-05:00"]) {
    const r = await run(["roles", "--since", spec], { D1_API_BASE: "http://127.0.0.1:9" });
    assert.notEqual(r.code, 0, spec);
    assert.match(r.err, /--since takes Nd or an ISO date/, spec);
    assert.doesNotMatch(r.err, /Traceback|OverflowError/, spec);
  }
});

// Year 1 in UTC sits inside the calendar, and glibc's `%Y` printed it as
// `1-01-01T...`, a string that sorts before every row in `at` (#1203). The
// stamp carries four year digits, the column's shape, so the window is whole.
test("roles stamps a year-1 window with four year digits, the at column's shape", async () => {
  const rows = [
    { role: "bdfl", measured: 1, turns: 10, out_tokens: 100, denials: 0, error: 0, cost_usd: 1, writes: 0 },
    { role: "reviewer", measured: 1, turns: 5, out_tokens: 50, denials: 0, error: 0, cost_usd: null, writes: null },
  ];
  await withStub(d1(rows), async (base, seen) => {
    const r = await run(["roles", "--since", "0001-01-01T00:00:00Z"], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.deepEqual(seen[0].body.params, ["0001-01-01T00:00:00Z"]);
    assert.match(r.out, /^sessions since 0001-01-01T00:00:00Z: 2$/m);
  });
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

// One statement per call, because `ALTER TABLE ADD COLUMN` is not
// idempotent in SQLite and a one-batch replay would stop at its "duplicate
// column name" with every statement after it unapplied. The schema file's
// header promises a replay on an existing database; this is what keeps it.
// The table list is read before and after, the one read that says what a
// CREATE IF NOT EXISTS created (#1355).
test("migrate posts the schema one statement at a time, comments stripped, between two reads of the table list", async () => {
  await withStub(d1([]), async (base, seen, schema) => {
    const r = await run(["migrate"], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 0);
    assert.equal(schema[0].body.sql, TABLES);
    assert.equal(schema.at(-1).body.sql, TABLES);
    const ddl = schema.slice(1, -1);
    assert.ok(ddl.length > 4, `${ddl.length} statements`);
    assert.match(ddl[0].body.sql, /^CREATE TABLE IF NOT EXISTS sessions \(/);
    assert.match(ddl[1].body.sql, /^CREATE INDEX IF NOT EXISTS sessions_role_at/);
    for (const { body } of ddl) {
      assert.doesNotMatch(body.sql, /;|--/, body.sql);
      assert.deepEqual(body.params, []);
    }
    assert.match(r.out, /schema applied from .*schema\.sql: new none$/m);
  });
});

test("migrate skips a column already present, applies the statements after it, and names it not new", async () => {
  // The ALTER is the fourth statement of the file: sessions, its two indexes, then the column.
  await withStub(
    d1([]),
    async (base, seen, schema) => {
      const r = await run(["migrate"], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.out + r.err);
      const ddl = schema.slice(1, -1);
      assert.match(ddl[3].body.sql, /^ALTER TABLE sessions ADD COLUMN writes INTEGER$/);
      assert.ok(ddl.length > 4, "the statements after the present column were applied");
      assert.match(r.out, /schema applied from .*schema\.sql: new none$/m);
    },
    { ddl: [d1([]), d1([]), d1([]), present, d1([])] },
  );
});

test("migrate names the table the database lacked and the column it added", async () => {
  const before = ["sessions", "allowance", "guard", "trust", "tools"];
  await withStub(
    d1([]),
    async (base) => {
      const r = await run(["migrate"], { D1_API_BASE: base });
      assert.equal(r.code, 0, r.out + r.err);
      assert.match(r.out, /schema applied from .*schema\.sql: new walls, sessions\.writes$/m);
    },
    { ddl: [d1([])], tables: [before, [...before, "walls"]] },
  );
});

test("migrate on any other refusal is unreadable, with the message", async () => {
  const refused = { payload: { success: false, errors: [{ message: "no such table: nowhere" }] } };
  await withStub(
    d1([]),
    async (base) => {
      const r = await run(["migrate"], { D1_API_BASE: base });
      assert.notEqual(r.code, 0);
      assert.match(r.out + r.err, /UNREADABLE.*no such table: nowhere/);
    },
    { ddl: [d1([]), refused] },
  );
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
            if name != "__encrypted__":
                zf.writestr(name, text)
    data = bytearray(buf.getvalue())
    if "__encrypted__" in files:
        # Bit 0 of the general-purpose flag, in every local and central
        # header: zipfile then raises RuntimeError on read, no password.
        for sig, off in ((b"PK\\x03\\x04", 6), (b"PK\\x01\\x02", 8)):
            i = data.find(sig)
            while i != -1:
                data[i + off] |= 1
                i = data.find(sig, i + 4)
    sys.stdout.buffer.write(bytes(data))
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
    assert.deepEqual(seen[1].body.params, []); // literals inlined: one call carries many statements (#1355)
    const [row] = applied(seen)("sessions");
    assert.equal(Object.keys(row).length, 22);
    assert.deepEqual([row.run_id, row.attempt, row.at, row.role], [200, 1, "2026-09-01T00:00:00Z", "reviewer"]); // at is the artifact's creation
    assert.deepEqual([row.prompt_bytes, row.response_bytes, row.writes], [1234, 567, null]); // sizes read from the zip; writes null, the artifact predates the count
    assert.equal(seen[2].body.sql, "SELECT run_id, attempt FROM sessions");
    assert.equal(seen.length, 3);
    // Sorted: eight workers download concurrently, so the stub's log holds no order.
    assert.deepEqual(downloads(gh).sort(), ["2", "3"]); // the present row cost no download; the expired and the non-audit never listed
    assert.match(r.out, /^  unreadable audit-herald-300-2: /m);
    assert.match(
      r.out,
      /^sessions backfill: schema new none \\| artifacts 3 since 2026-08-22T08:14:32Z \| present 1 \| written 1 \| unreadable 1 \| missing 1$/m,
    );
  });
});

test("backfill writes an artifact's allowance readings beside its row; --rebuild reads present rows too", async () => {
  const meta = {
    ...metadata(100, "bdfl"),
    rate_limit_events: [
      { unifiedWindows: { seven_day: { utilization: 0.24, resetsAt: 1792029600, isUsingOverage: false } } },
    ],
  };
  const gh = stubGh(listing.slice(0, 1), {
    1: { "metadata.json": JSON.stringify(meta), "input-prompt.md": "p", "output-response.md": "r" },
  });
  // The table is read under --rebuild too, so the census can count what it held (#1012).
  await withStub([keys([[100, 1]]), d1([]), keys([[100, 1]])], async (base, seen) => {
    const r = await run(["backfill", "--rebuild"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 0, r.out + r.err);
    assert.deepEqual(downloads(gh), ["1"]); // the row the table already holds, read again
    assert.equal(seen.length, 3);
    assert.equal(seen[0].body.sql, "SELECT run_id, attempt FROM sessions");
    assert.match(
      seen[1].body.sql,
      /^INSERT OR REPLACE INTO allowance \(run_id, attempt, window, utilization, resets_at, overage\) VALUES \(100, 1, 'seven_day', 0\.24, 1792029600, 0\);\nINSERT OR REPLACE INTO sessions /,
    ); // the session row last: the mark that the derived rows are in (#1102)
    assert.deepEqual(applied(seen)("allowance"), [
      { run_id: 100, attempt: 1, window: "seven_day", utilization: 0.24, resets_at: 1792029600, overage: 0 },
    ]);
    assert.equal(seen[2].body.sql, "SELECT run_id, attempt FROM sessions");
    // present 1, not 0: the row the table held before the walk counts, rewritten or not.
    assert.match(
      r.out,
      /^sessions backfill: schema new none \\| artifacts 1 since 2026-08-22T08:14:32Z \| present 1 \| written 1 \| unreadable 0 \| missing 0$/m,
    );
  });
});

// #891 slice 5: the tool counts ride the same download as the session row.
// Edit and Write are named refused with no per-tool count, so their denials
// are null; the artifacts before #946 carry no `tools` record and get no row.
test("backfill writes an artifact's tool counts beside its row, none for an artifact without the record", async () => {
  const base_meta = metadata(100, "bdfl");
  const meta = {
    ...base_meta,
    telemetry: { ...base_meta.telemetry, tools: { offered: null, used: { Bash: 7 }, writes: 1 } },
  };
  const files = (m) => ({ "metadata.json": JSON.stringify(m), "input-prompt.md": "p", "output-response.md": "r" });
  const gh = stubGh(listing.slice(0, 2), { 1: files(meta), 2: files(metadata(200, "reviewer")) });
  await withStub([keys([]), d1([]), keys([[100, 1], [200, 1]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 0, r.out + r.err);
    assert.equal(seen.length, 3); // both runs in one call
    assert.deepEqual(heads(seen[1].body.sql), [
      "INSERT OR REPLACE INTO tools",
      "INSERT OR REPLACE INTO sessions", // the bdfl row, after its tools
      "INSERT OR REPLACE INTO sessions", // the reviewer artifact: a row, no tools
    ]);
    assert.deepEqual(applied(seen)("tools"), [
      { run_id: 100, attempt: 1, tool: "Bash", uses: 7, denials: 0 },
      { run_id: 100, attempt: 1, tool: "Edit", uses: 0, denials: null },
      { run_id: 100, attempt: 1, tool: "Write", uses: 0, denials: null },
    ]);
    assert.match(r.out, /written 2 \| unreadable 0 \| missing 0$/m);
  });
});

// #1102: the session row is the last statement of a run's script, so a
// call that fails midway leaves the run absent and the next walk writes
// all four tables from the artifact. Session row first, the run read as
// present and the gap outlived the artifact.
test("a write that fails leaves the run absent for the next walk, its session row the script's last statement", async () => {
  const meta = {
    ...metadata(100, "bdfl"),
    rate_limit_events: [
      { unifiedWindows: { seven_day: { utilization: 0.24, resetsAt: 1792029600, isUsingOverage: false } } },
    ],
  };
  const gh = stubGh(listing.slice(0, 1), {
    1: { "metadata.json": JSON.stringify(meta), "input-prompt.md": "p", "output-response.md": "r" },
  });
  const down = { status: 500, payload: { success: false, errors: [{ message: "edge down" }] } };
  await withStub([keys([]), down], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 1, r.out + r.err);
    assert.match(r.out, /^sessions: UNREADABLE: HTTP 500/m);
    assert.equal(seen.length, 2); // the read, then the one write that failed
    assert.deepEqual(heads(seen[1].body.sql), ["INSERT OR REPLACE INTO allowance", "INSERT OR REPLACE INTO sessions"]);
  });
});

test("a tool count that is not a count makes its artifact unreadable, never the walk", async () => {
  const base_meta = metadata(100, "bdfl");
  const meta = {
    ...base_meta,
    telemetry: { ...base_meta.telemetry, tools: { offered: null, used: { Bash: 7.5 }, writes: 1 } },
  };
  const files = (m) => ({ "metadata.json": JSON.stringify(m), "input-prompt.md": "p", "output-response.md": "r" });
  const gh = stubGh(listing.slice(0, 2), { 1: files(meta), 2: files(metadata(200, "reviewer")) });
  await withStub([keys([]), d1([]), keys([[200, 1]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 1, r.out + r.err);
    assert.equal(seen.filter((q) => q.body.sql.startsWith("INSERT")).length, 1); // the reviewer artifact alone
    assert.match(
      r.out,
      /^  unreadable audit-bdfl-100-1: metadata has a malformed field \(ValueError: tools.used\['Bash'\] is not a count\)$/m,
    );
    assert.match(r.out, /written 1 \| unreadable 1 \| missing 1$/m);
  });
});

test("an artifact with more windows than the API reports is cut at sixteen rows, never the walk", async () => {
  const windows = Object.fromEntries(
    Array.from({ length: 20 }, (_, i) => [`w${String(i).padStart(2, "0")}`, { utilization: 0.1 }]),
  );
  const hostile = { ...metadata(100, "bdfl"), rate_limit_events: [{ unifiedWindows: windows }] };
  const files = (meta) => ({
    "metadata.json": JSON.stringify(meta),
    "input-prompt.md": "p",
    "output-response.md": "r",
  });
  const gh = stubGh(listing.slice(0, 2), { 1: files(hostile), 2: files(metadata(200, "reviewer")) });
  await withStub([keys([]), d1([]), keys([[100, 1], [200, 1]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 0, r.out + r.err);
    const rows = applied(seen);
    assert.equal(rows("allowance").length, 16); // the first sixteen by name
    assert.equal(rows("sessions").length, 2); // the next artifact was still written
    assert.match(r.out, /written 2 \| unreadable 0 \| missing 0$/m);
  });
});

test("backfill counts present by name before the walk rewrites keys from metadata", async () => {
  // Name says attempt 1, metadata says attempt 2, and the table holds (100, 2):
  // not present by name, so walked and written, and the census must say so.
  // Unreachable from a real artifact today, whose name and metadata agree (#1016).
  const gh = stubGh(listing.slice(0, 1), {
    1: {
      "metadata.json": JSON.stringify({ ...metadata(100, "bdfl"), run_attempt: "2" }),
      "input-prompt.md": "p",
      "output-response.md": "r",
    },
  });
  await withStub([keys([[100, 2]]), d1([]), keys([[100, 2]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 0, r.out + r.err);
    assert.deepEqual(downloads(gh), ["1"]);
    assert.equal(seen.length, 3);
    assert.match(seen[1].body.sql, /INTO sessions \([^)]*\) VALUES \(100, 2, /); // written under the metadata key
    assert.match(
      r.out,
      /^sessions backfill: schema new none \\| artifacts 1 since 2026-08-22T08:14:32Z \| present 0 \| written 1 \| unreadable 0 \| missing 0$/m,
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
    assert.match(seen[1].body.sql, /VALUES \(200, 1, /);
    assert.match(
      r.out,
      /artifacts 1 since 2026-09-01T00:00:00Z \| present 0 \| written 1 \| unreadable 0 \| missing 0$/m,
    );
  });
});

// Round 5 (#995): the contract is one unreadable line per artifact, whatever
// broke it, not a list of exception types. An encrypted member raises
// zipfile's RuntimeError inside fetch; `run_id: 1e999` parses to inf and
// int(inf) raises OverflowError inside row(). Both sit before the valid
// artifact, oldest first, where uncontained either ends every rerun.
test("one artifact costs one unreadable line whatever broke it, never the walk", async () => {
  const inf = JSON.stringify(metadata(800, "herald")).replace("\"run_id\":\"800\"", "\"run_id\":1e999");
  assert.match(inf, /"run_id":1e999/);
  const gh = stubGh(
    [
      artifact(8, "audit-herald-800-1", "2026-08-28T00:00:00Z"),
      artifact(9, "audit-curator-900-1", "2026-08-29T00:00:00Z"),
      listing[1],
    ],
    {
      8: { "metadata.json": inf },
      9: { "metadata.json": JSON.stringify(metadata(900, "curator")), __encrypted__: true },
      2: zips[2],
    },
  );
  await withStub([keys([]), d1([]), keys([[200, 1]])], async (base, seen) => {
    const r = await run(["backfill", "--repo", "o/r"], { D1_API_BASE: base, PATH: `${gh}:${process.env.PATH}` });
    assert.equal(r.code, 1, r.out + r.err);
    assert.doesNotMatch(r.err, /Traceback/);
    assert.match(seen[1].body.sql, /VALUES \(200, 1, /, "the valid artifact after both poisons is written");
    assert.match(r.out, /^  unreadable audit-herald-800-1: metadata carries no run_id$/m);
    assert.match(r.out, /^  unreadable audit-curator-900-1: RuntimeError: .*encrypted/m);
    assert.match(
      r.out,
      /artifacts 3 since 2026-08-28T00:00:00Z \| present 0 \| written 1 \| unreadable 2 \| missing 2$/m,
    );
  });
});

// Round 6 (#995): a field `facts` passes through raw can be a value D1 cannot
// bind. `costUSD: 1e999` is inf, which json.dumps writes as the non-JSON
// constant Infinity; the strict stub rejects it with 400 as Cloudflare's
// parser would, and one rejected INSERT used to kill the walk at that
// artifact with the valid one after it never written. A role that is an
// object, and a turn count past 64 bits, ride the same artifact.
test("a value D1 cannot bind becomes null in its row, never a dead walk", async () => {
  const poisoned = JSON.parse(JSON.stringify(metadata(800, "herald")));
  poisoned.role = { not: "text" };
  const text = JSON.stringify(poisoned)
    .replace("\"costUSD\":0.42", "\"costUSD\":1e999")
    .replace("\"num_turns\":23", "\"num_turns\":100000000000000000000000000000");
  assert.match(text, /"costUSD":1e999/);
  assert.match(text, /"num_turns":1000000000000000000000000000/);
  const gh = stubGh(
    [artifact(8, "audit-herald-800-1", "2026-08-28T00:00:00Z"), listing[1]],
    { 8: { "metadata.json": text }, 2: zips[2] },
  );
  await withStub([keys([]), d1([]), keys([[800, 1], [200, 1]])], async (base, seen) => {
    const r = await run(["backfill", "--repo", "o/r"], { D1_API_BASE: base, PATH: `${gh}:${process.env.PATH}` });
    assert.equal(r.code, 0, r.out + r.err);
    assert.equal(seen.length, 3, "both artifacts reached the one write");
    const [poisoned_row, valid] = applied(seen)("sessions");
    assert.equal(poisoned_row.run_id, 800);
    assert.equal(poisoned_row.role, "?");
    assert.equal(poisoned_row.cost_usd, null);
    assert.equal(poisoned_row.turns, null);
    assert.equal(valid.run_id, 200, "the valid artifact after it is written");
    assert.match(r.out, /written 2 \| unreadable 0 \| missing 0$/m);
  });
});

// A text column rides inline now, so a hostile one is the statement: past
// MAX_STATEMENT_BYTES, `script` raises and the artifact costs one
// unreadable line, never a call that fails with the seven runs beside it.
test("a text past the statement cap costs its artifact one unreadable line, never the walk", async () => {
  const bloated = JSON.parse(JSON.stringify(metadata(800, "herald")));
  bloated.telemetry.stop_reason = "x".repeat(150000);
  const gh = stubGh(
    [artifact(8, "audit-herald-800-1", "2026-08-28T00:00:00Z"), listing[1]],
    { 8: { "metadata.json": JSON.stringify(bloated) }, 2: zips[2] },
  );
  await withStub([keys([]), d1([]), keys([[200, 1]])], async (base, seen) => {
    const r = await run(["backfill", "--repo", "o/r"], { D1_API_BASE: base, PATH: `${gh}:${process.env.PATH}` });
    assert.equal(r.code, 1, r.out + r.err);
    assert.equal(seen.length, 3);
    assert.match(seen[1].body.sql, /VALUES \(200, 1, /, "the valid artifact after it is written");
    assert.ok(seen[1].body.sql.length < 10000, "nothing of the bloated artifact reached the call");
    assert.match(
      r.out,
      /^  unreadable audit-herald-800-1: metadata has a malformed field \(ValueError: a statement of 15\d{4} bytes, over 50000\)$/m,
    );
    assert.match(r.out, /written 1 \| unreadable 1 \| missing 1$/m);
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
    assert.match(seen[1].body.sql, /VALUES \(200, 1, /);
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

// #1241: the walls ride the same download as the session row, before it;
// an artifact whose denials carry no reasons record gets no walls row.
test("backfill writes an artifact's walls beside its row, none for an artifact without reasons", async () => {
  const base_meta = metadata(100, "bdfl");
  const meta = {
    ...base_meta,
    telemetry: {
      ...base_meta.telemetry,
      permission_denials: {
        count: 2,
        tools: ["Bash"],
        by_tool: { Bash: 2 },
        reasons: [
          { tool: "Bash", type: "hook", reason: "deny-em-dash: denied" },
          { tool: "Bash", type: "mode", reason: "plan mode" },
        ],
      },
    },
  };
  const files = (m) => ({ "metadata.json": JSON.stringify(m), "input-prompt.md": "p", "output-response.md": "r" });
  const gh = stubGh(listing.slice(0, 2), { 1: files(meta), 2: files(metadata(200, "reviewer")) });
  await withStub([keys([]), d1([]), keys([[100, 1], [200, 1]])], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 0, r.out + r.err);
    assert.equal(seen.length, 3);
    assert.deepEqual(heads(seen[1].body.sql), [
      "INSERT OR REPLACE INTO walls",
      "INSERT OR REPLACE INTO sessions", // the bdfl row, after its walls
      "INSERT OR REPLACE INTO sessions", // the reviewer artifact: a row, no walls
    ]);
    assert.deepEqual(applied(seen)("walls"), [
      { run_id: 100, attempt: 1, tool: "Bash", wall: "deny-em-dash" },
      { run_id: 100, attempt: 1, tool: "Bash", wall: "mode" },
    ]);
    assert.match(r.out, /written 2 \| unreadable 0 \| missing 0$/m);
  });
});

// #1355: the allowance, tools and walls tables each got their history by a
// hand `migrate` then a hand `backfill --rebuild`, and the third rebuild
// outran a wake. The Sunday walk applies the schema itself, before it reads
// the table, and a statement that created a table makes the walk whole.
test("backfill applies the schema before reading the table, and a created table makes the walk whole", async () => {
  const gh = stubGh(listing.slice(0, 1), {
    1: { "metadata.json": JSON.stringify(metadata(100, "bdfl")), "input-prompt.md": "p", "output-response.md": "r" },
  });
  await withStub(
    [keys([[100, 1]]), d1([]), keys([[100, 1]])],
    async (base, seen, schema) => {
      const r = await run(["backfill"], {
        D1_API_BASE: base,
        GITHUB_REPOSITORY: "o/r",
        PATH: `${gh}:${process.env.PATH}`,
      });
      assert.equal(r.code, 0, r.out + r.err);
      assert.ok(schema.at(-1).n < seen[0].n, "the schema is applied before the table is read");
      assert.deepEqual(downloads(gh), ["1"]); // the row the table already holds, read again
      assert.equal(seen.length, 3);
      assert.match(seen[1].body.sql, /^INSERT OR REPLACE INTO sessions /);
      assert.match(
        r.out,
        /^sessions backfill: schema new walls \| artifacts 1 since 2026-08-22T08:14:32Z \| present 1 \| written 1 \| unreadable 0 \| missing 0$/m,
      );
    },
    { tables: [["sessions"], ["sessions", "walls"]] },
  );
});

// Eight runs ride one call (RUNS_PER_CALL), each run's statements in
// `script`'s order with its session row last, so a whole walk stays under
// the API's calls per five minutes and a call cut short leaves every run
// in it whole or absent.
test("backfill writes eight runs a call, oldest first, each run's session row last in its slice", async () => {
  const many = Array.from(
    { length: 9 },
    (_, i) => artifact(10 + i, `audit-bdfl-${1000 + i}-1`, `2026-09-${String(10 + i).padStart(2, "0")}T00:00:00Z`),
  );
  const reading = [{ unifiedWindows: { seven_day: { utilization: 0.1 } } }];
  const gh = stubGh(
    many,
    Object.fromEntries(
      many.map((a, i) => [
        a.id,
        { "metadata.json": JSON.stringify({ ...metadata(1000 + i, "bdfl"), rate_limit_events: reading }) },
      ]),
    ),
  );
  await withStub([keys([]), d1([]), d1([]), keys(many.map((_, i) => [1000 + i, 1]))], async (base, seen) => {
    const r = await run(["backfill"], {
      D1_API_BASE: base,
      GITHUB_REPOSITORY: "o/r",
      PATH: `${gh}:${process.env.PATH}`,
    });
    assert.equal(r.code, 0, r.out + r.err);
    assert.equal(seen.length, 4); // the read, eight runs, the ninth, the read
    const slice = ["INSERT OR REPLACE INTO allowance", "INSERT OR REPLACE INTO sessions"];
    assert.deepEqual(heads(seen[1].body.sql), Array(8).fill(slice).flat());
    assert.deepEqual(heads(seen[2].body.sql), slice);
    const rows = applied(seen);
    assert.deepEqual(
      rows("sessions").map((row) => row.run_id),
      many.map((_, i) => 1000 + i),
    );
    assert.equal(rows("allowance").length, 9);
    assert.match(r.out, /written 9 \| unreadable 0 \| missing 0$/m);
  });
});
