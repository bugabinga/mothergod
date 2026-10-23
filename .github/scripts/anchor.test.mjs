// Fixtures for anchor.previous_run_start, the instant operator-sweep and
// retrospect both measure from. The first fixture is 2026-09-17T04:12Z as it
// happened: thirteen guard-skipped BDFL runs, each concluded `success` in
// seconds with no session, stacked in front of the last real run on the
// 15th. Anchoring on the newest success covered two hours; the operator's
// window was two days (#525, and nine days after #517). The session row
// every run's audit step writes (ADR-0060) is what tells the two apart, the
// audit artifact (ADR-0023) when the table is unreadable: the stub D1 here
// answers the membership query from the fixture's session set, and the stub
// `gh` answers `run list` with the stack and `api .../artifacts` per run,
// logging every call so a test can say which provider was asked (#1530).
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import sys
sys.path.insert(0, sys.argv[1])
import anchor
now = anchor.stamp_to_dt(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else None
stamp, run_id = anchor.previous_run_start("o/r", sys.argv[2] or None, now=now)
print(stamp, run_id)
`;

// A stub gh: \`run list\` prints the fixture's runs newest first; \`api\`
// on a run's artifacts prints that run's artifact count. Anything else is
// a call the anchor has no business making, and fails loudly. Every call
// lands in \`calls\`, one line each.
function stubGh(runs, artifacts) {
  const dir = mkdtempSync(join(tmpdir(), "anchor-"));
  const log = join(dir, "calls");
  writeFileSync(log, "");
  writeFileSync(
    join(dir, "gh"),
    `#!/usr/bin/env python3
import json, re, sys
runs = ${JSON.stringify(runs)}
artifacts = ${JSON.stringify(artifacts)}
args = sys.argv[1:]
with open(${JSON.stringify(log)}, "a") as calls:
    calls.write(" ".join(args) + "\\n")
if args[:2] == ["run", "list"]:
    print(json.dumps(runs))
elif args[0] == "api":
    m = re.fullmatch(r"repos/o/r/actions/runs/(\\d+)/artifacts", args[1])
    print(artifacts[m.group(1)])
else:
    sys.exit("stub gh: unexpected call " + " ".join(args))
`,
  );
  chmodSync(join(dir, "gh"), 0o755);
  return { dir, log };
}

// A stub D1: answers the membership query with a row per bound id that is
// in `sessions`, or 500 when `sessions` is "down". Records what was asked.
async function withTable(sessions, fn) {
  const asked = [];
  const server = createServer((req, res) => {
    let body = "";
    req.on("data", (chunk) => (body += chunk));
    req.on("end", () => {
      const { sql, params } = JSON.parse(body);
      asked.push({ sql, params });
      if (sessions === "down") {
        res.writeHead(500, { "content-type": "application/json" });
        res.end(JSON.stringify({ success: false, errors: [{ message: "D1 is down" }] }));
        return;
      }
      const results = params.filter((id) => sessions.includes(id)).map((run_id) => ({ run_id }));
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ success: true, result: [{ success: true, results }] }));
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    return await fn(`http://127.0.0.1:${server.address().port}`, asked);
  } finally {
    server.close();
  }
}

function spawnDriver(args, env) {
  return new Promise((resolve) => {
    const child = spawn("python3", ["-c", driver, ...args], { env });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("close", (status) => resolve({ status, stdout, stderr }));
  });
}

// The session set an artifacts map implies: the table and the artifact mark
// the same runs, so a fixture written for one reads for the other.
const held = (artifacts) =>
  Object.entries(artifacts)
    .filter(([, count]) => count > 0)
    .map(([id]) => Number(id));

// `now` defaults to the fixture's own morning so the staleness line below
// stays out of every test that is not about it. The result carries how many
// `/artifacts` reads the walk made and what it asked the table.
async function anchor(
  runs,
  artifacts,
  { thisRun = "", now = "2026-09-17T04:12:00Z", sessions = held(artifacts) } = {},
) {
  const gh = stubGh(runs, artifacts);
  return withTable(sessions, async (base, asked) => {
    const proc = await spawnDriver([scriptsDir, thisRun, now], {
      ...process.env,
      PATH: `${gh.dir}:${process.env.PATH}`,
      CLOUDFLARE_API_TOKEN: "test-token",
      D1_API_BASE: base,
      D1_DATABASE: "db",
    });
    const artifactReads = readFileSync(gh.log, "utf8")
      .split("\n")
      .filter((line) => line.includes("/artifacts")).length;
    return { ...proc, asked, artifactReads };
  });
}

const run = (id, startedAt) => ({ databaseId: id, startedAt, createdAt: startedAt });

// 2026-09-17T04:12Z, abridged to the newest three hollow runs and the real one.
const STACK = [
  run(35165527435, "2026-09-17T00:11:33Z"),
  run(35145089313, "2026-09-16T20:11:52Z"),
  run(35120333073, "2026-09-16T16:11:52Z"),
  run(34927890526, "2026-09-15T04:11:52Z"),
];
const HOLLOW_TOP = { 35165527435: 0, 35145089313: 0, 35120333073: 0, 34927890526: 1 };
const ALL_HOLLOW = { ...HOLLOW_TOP, 34927890526: 0 };

test("skips guard-skipped successes and anchors on the last run that held a session", async () => {
  const proc = await anchor(STACK, HOLLOW_TOP);
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-15T04:11:52Z 34927890526");
});

test("the table is asked once for the whole page and no artifacts page is read", async () => {
  const proc = await anchor(STACK, HOLLOW_TOP);
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.asked.length, 1);
  assert.match(proc.asked[0].sql, /SELECT run_id FROM sessions WHERE run_id IN \(\?, \?, \?, \?\)/);
  assert.deepEqual(proc.asked[0].params, [35165527435, 35145089313, 35120333073, 34927890526]);
  assert.equal(proc.artifactReads, 0);
});

test("the ordinary wake anchors on the newest success when it has a session", async () => {
  const proc = await anchor(STACK, { ...HOLLOW_TOP, 35165527435: 1 });
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-17T00:11:33Z 35165527435");
});

test("this run is excluded even when it already has a session", async () => {
  const proc = await anchor(STACK, { ...HOLLOW_TOP, 35165527435: 1 }, { thisRun: "35165527435" });
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-15T04:11:52Z 34927890526");
});

test("a runs page served oldest-first still anchors on the newest session (#902)", async () => {
  const proc = await anchor([...STACK].reverse(), { ...HOLLOW_TOP, 35165527435: 1 });
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-17T00:11:33Z 35165527435");
  assert.equal(proc.stderr, "");
});

test("an anchor older than the cron explains prints one loud line naming its age and run (#902)", async () => {
  // Run 37152004054 on 2026-10-03 anchored on a 2026-09-03 run: 30 days on
  // a four-hour cron, and nothing said so until the footer.
  const proc = await anchor(STACK, HOLLOW_TOP, { now: "2026-10-15T04:11:52Z" });
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-15T04:11:52Z 34927890526");
  assert.match(proc.stderr, /anchor 2026-09-15T04:11:52Z \(run 34927890526\) is 30\.0 days old/);
  assert.match(proc.stderr, /#902/);
});

test("a stack with no session in it dies naming --since, never anchors on a hollow run", async () => {
  const proc = await anchor(STACK, ALL_HOLLOW);
  assert.notEqual(proc.status, 0);
  assert.match(proc.stderr, /--since/);
  assert.match(proc.stderr, /session row/);
  assert.equal(proc.artifactReads, 0);
});

test("every artifacts page answering 0 while the table names the session anchors on it, no artifacts read (#1530)", async () => {
  // 2026-10-22T16:13Z under Actions' degraded_performance: `/artifacts`
  // answered total_count 0 for sixty successes, `run list` answered every
  // start, and the walk died. The table saw the same runs the whole time.
  const proc = await anchor(STACK, ALL_HOLLOW, { sessions: [34927890526] });
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-15T04:11:52Z 34927890526");
  assert.equal(proc.artifactReads, 0);
  assert.equal(proc.stderr, "");
});

test("an unreadable table falls back to the artifact per run and says so once (#1530)", async () => {
  const proc = await anchor(STACK, HOLLOW_TOP, { sessions: "down" });
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-15T04:11:52Z 34927890526");
  assert.equal(proc.artifactReads, 4);
  assert.equal(proc.stderr.match(/UNREADABLE/g).length, 1);
  assert.match(proc.stderr, /session table UNREADABLE \(HTTP 500/);
  assert.match(proc.stderr, /audit artifacts instead/);
});

test("the table down and every artifacts page hollow dies naming --since (#1530)", async () => {
  const proc = await anchor(STACK, ALL_HOLLOW, { sessions: "down" });
  assert.notEqual(proc.status, 0);
  assert.equal(proc.artifactReads, 4);
  assert.match(proc.stderr, /UNREADABLE/);
  assert.match(proc.stderr, /--since/);
  assert.match(proc.stderr, /audit artifact/);
});
