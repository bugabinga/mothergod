// Fixtures for trust-row.py, the trust ledger's write into the trust table
// (#891 slice 4). The live write carries guard-row's two invariants: the
// row is the whitelist and nothing else, and nothing can fail the run it
// observes. backfill carries `sessions backfill`'s: the census is the last
// line, a gap exits 1, one artifact costs one line and never the walk.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/trust-row.py`;

const INSERT =
  "INSERT OR REPLACE INTO trust (run_id, attempt, at, role, fuzz_cpu_s, crashers_new, mutation_score, coverage_region_pct) VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

// A fuzz-check entry as fuzz-check.yml prints it: run_id a string, the
// fields it does not own null.
const fuzzEntry = {
  date: "2026-10-11T02:47:10Z",
  run_id: "38112345678",
  role: "fuzz",
  fuzz_cpu_s: 2008,
  crashers_new: 0,
  mutation_score: null,
  coverage_region_pct: null,
};

// A D1 stub with one table's worth of memory: a SELECT answers with `rows`
// plus every key an INSERT wrote, anything else with an empty result, and
// every statement sent is recorded. `answer` overrides the whole reply.
async function withD1(rows, fn, answer = null) {
  const seen = [];
  const inserted = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const sent = JSON.parse(body);
      seen.push(sent);
      if (sent.sql.startsWith("INSERT")) inserted.push({ run_id: sent.params[0], attempt: sent.params[1] });
      const results = sent.sql.startsWith("SELECT") ? [...rows, ...inserted] : [];
      res.writeHead(answer?.status ?? 200, { "content-type": "application/json" });
      res.end(JSON.stringify(answer?.payload ?? { success: true, result: [{ success: true, results }] }));
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    return await fn(`http://127.0.0.1:${server.address().port}`, seen);
  } finally {
    server.close();
  }
}

function entryFile(entry) {
  const path = join(mkdtempSync(join(tmpdir(), "trust-row-")), "entry.json");
  writeFileSync(path, JSON.stringify(entry));
  return path;
}

function run(args, env) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script, ...args], {
      env: {
        ...process.env,
        CLOUDFLARE_API_TOKEN: "test-token",
        D1_DATABASE: "db",
        GITHUB_RUN_ATTEMPT: "1",
        GITHUB_REPOSITORY: "",
        ...env,
      },
    });
    let out = "";
    let err = "";
    child.stdout.on("data", (c) => (out += c));
    child.stderr.on("data", (c) => (err += c));
    child.on("close", (code) => resolve({ code, out, err }));
  });
}

test("a fuzz entry writes the whitelist row: run_id an integer, the attempt from the run, unowned fields NULL", async () => {
  await withD1([], async (base, seen) => {
    const r = await run([entryFile(fuzzEntry)], { D1_API_BASE: base, GITHUB_RUN_ATTEMPT: "2" });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 1);
    assert.equal(seen[0].sql, INSERT);
    assert.deepEqual(seen[0].params, [38112345678, 2, "2026-10-11T02:47:10Z", "fuzz", 2008, 0, null, null]);
    assert.match(r.out, /^trust-row: wrote run 38112345678 attempt 2 for fuzz$/m);
  });
});

test("a coverage entry carries its percentage as a number and nothing of fuzz's", async () => {
  const entry = { ...fuzzEntry, role: "coverage", fuzz_cpu_s: null, crashers_new: null, coverage_region_pct: 97.38 };
  await withD1([], async (base, seen) => {
    const r = await run([entryFile(entry)], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.deepEqual(seen[0].params.slice(3), ["coverage", null, null, null, 97.38]);
    assert.equal(seen[0].params[1], 1);
  });
});

test("an entry dated before crashers were measured writes crashers_new NULL, the seconds intact", async () => {
  const entry = { ...fuzzEntry, date: "2026-09-01T02:40:00Z", crashers_new: 26 };
  await withD1([], async (base, seen) => {
    const r = await run([entryFile(entry)], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.deepEqual(seen[0].params.slice(4, 6), [2008, null]);
  });
});

test("no token: one line, no request, exit 0", async () => {
  await withD1([], async (base, seen) => {
    const r = await run([entryFile(fuzzEntry)], { D1_API_BASE: base, CLOUDFLARE_API_TOKEN: "" });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 0);
    assert.equal(r.out.trim(), "trust-row: skipped, no CLOUDFLARE_API_TOKEN");
  });
});

test("a database that refuses the write is a warning, token-free, exit 0", async () => {
  const payload = { success: false, errors: [{ message: "no such table: trust" }] };
  await withD1([], async (base) => {
    const r = await run([entryFile(fuzzEntry)], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.match(r.out, /^::warning::trust-row: not written: Unreadable: no such table: trust$/m);
    assert.doesNotMatch(r.out + r.err, /test-token/);
  }, { status: 200, payload });
});

test("a count that is not whole, a run id that is not a number: a warning before any request", async () => {
  await withD1([], async (base, seen) => {
    let r = await run([entryFile({ ...fuzzEntry, crashers_new: 1.5 })], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.match(r.out, /::warning::trust-row: not written: ValueError: crashers_new is 1\.5, not a count/);
    r = await run([entryFile({ ...fuzzEntry, run_id: "" })], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.match(r.out, /::warning::trust-row: not written: ValueError/);
    assert.equal(seen.length, 0);
    assert.equal(r.err, "");
  });
});

test("a missing entry file is a warning, not a traceback", async () => {
  await withD1([], async (base, seen) => {
    const r = await run(["/nonexistent/entry.json"], { D1_API_BASE: base });
    assert.equal(r.code, 0, r.err);
    assert.equal(seen.length, 0);
    assert.match(r.out, /::warning::trust-row: not written: FileNotFoundError/);
    assert.equal(r.err, "");
  });
});

// The archive, for `backfill`: a stub gh answers the listing with
// `artifacts` and a zip download with the files `zips` holds for that id,
// or junk bytes for an id mapped to null. Every call lands in calls.log.
function stubGh(artifacts, zips) {
  const dir = mkdtempSync(join(tmpdir(), "trust-gh-"));
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

const artifact = (id, name, created_at, expired = false) => ({ id, name, created_at, expired });

const listing = [
  artifact(1, "trust-fuzz-100-1", "2026-08-29T02:40:00Z"),
  artifact(2, "trust-coverage-200-1", "2026-09-07T05:00:00Z"),
  artifact(3, "trust-fuzz-300-2", "2026-09-20T02:40:00Z"),
  artifact(4, "trust-fuzz-50-1", "2026-07-01T00:00:00Z", true),
  artifact(5, "audit-bdfl-400-1", "2026-09-21T00:00:00Z"),
  artifact(6, "trust-fuzz-500-1", "2026-09-22T02:40:00Z"),
];
const zips = {
  1: { "entry.json": JSON.stringify({ ...fuzzEntry, date: "2026-08-29T02:40:00Z", run_id: "100", crashers_new: 26 }) },
  3: null,
  6: { "entry.json": "{not json" },
};

test("backfill writes the rows the artifacts hold and the table lacks, and names the gap", async () => {
  const gh = stubGh(listing, zips);
  await withD1([{ run_id: 200, attempt: 1 }], async (base, seen) => {
    const r = await run(["backfill", "--repo", "o/r"], { D1_API_BASE: base, PATH: `${gh}:${process.env.PATH}` });
    assert.equal(r.code, 1, r.out + r.err);
    // The expired one and the audit artifact never download; the present
    // one costs no download; the two unreadable ones cost one line each.
    assert.deepEqual(downloads(gh), ["1", "3", "6"]);
    const inserts = seen.filter((s) => s.sql === INSERT);
    assert.equal(inserts.length, 1);
    assert.deepEqual(inserts[0].params, [100, 1, "2026-08-29T02:40:00Z", "fuzz", 2008, null, null, null]);
    assert.match(r.out, /^  unreadable trust-fuzz-300-2: BadZipFile: /m);
    assert.match(r.out, /^  unreadable trust-fuzz-500-1: JSONDecodeError: /m);
    assert.match(
      r.out,
      /^trust-row backfill: artifacts 4 since 2026-08-29T02:40:00Z \| present 1 \| written 1 \| unreadable 2 \| missing 2$/m,
    );
    assert.equal(r.out.trim().split("\n").at(-1).startsWith("trust-row backfill:"), true);
  });
});

test("backfill on a table equal to the archive downloads nothing and exits 0", async () => {
  const gh = stubGh([listing[1]], {});
  await withD1([{ run_id: 200, attempt: 1 }], async (base) => {
    const r = await run(["backfill", "--repo", "o/r"], { D1_API_BASE: base, PATH: `${gh}:${process.env.PATH}` });
    assert.equal(r.code, 0, r.out + r.err);
    assert.deepEqual(downloads(gh), []);
    assert.match(
      r.out,
      /^trust-row backfill: artifacts 1 since 2026-09-07T05:00:00Z \| present 1 \| written 0 \| unreadable 0 \| missing 0$/m,
    );
  });
});

test("backfill with no repository names the flag, exit 2; an unreadable listing is UNREADABLE, exit 1", async () => {
  const gh = stubGh([], {});
  await withD1([], async (base, seen) => {
    let r = await run(["backfill"], { D1_API_BASE: base, PATH: `${gh}:${process.env.PATH}` });
    assert.equal(r.code, 2);
    assert.equal(r.out.trim(), "trust-row: backfill needs GITHUB_REPOSITORY or --repo owner/name");
    r = await run(["backfill", "--repo", "x/y"], { D1_API_BASE: base, PATH: `${gh}:${process.env.PATH}` });
    assert.equal(r.code, 1, r.out);
    assert.match(r.out, /^trust-row: UNREADABLE: stub gh: unexpected call/m);
    assert.equal(seen.filter((s) => s.sql === INSERT).length, 0);
  });
});

test("no verb prints the usage line, exit 2", async () => {
  const r = await run([], {});
  assert.equal(r.code, 2);
  assert.match(r.out, /^Usage: trust-row\.py <entry\.json> \| backfill/);
});
