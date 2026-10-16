// Fixtures for recheck-decide, the verdict that re-runs a red scheduled
// check after a push to main (#1069).
//
// Two things it can get wrong are pinned here: which workflows it may
// dispatch on the PAT (never a model seat, #50), and when a red earns a
// retry (once per RETRY_HOURS, so a persistent red has a cost ceiling).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = join(scriptsDir, "recheck-decide.py");

const driver = `
import importlib.machinery, importlib.util, json, pathlib, sys
loader = importlib.machinery.SourceFileLoader("recheck_decide", sys.argv[1] + "/recheck-decide.py")
spec = importlib.util.spec_from_loader("recheck_decide", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
call = json.loads(sys.argv[2])
if call["fn"] == "decide":
    print(json.dumps(mod.decide(call["runs"], call["now"])))
else:
    files, error = mod.candidates(pathlib.Path(call["dir"]))
    print(json.dumps([files, error]))
`;

function call(payload) {
  const res = spawnSync("python3", ["-c", driver, scriptsDir, JSON.stringify(payload)], { encoding: "utf8" });
  assert.equal(res.status, 0, res.stderr);
  return JSON.parse(res.stdout);
}

const decide = (runs) => call({ fn: "decide", runs, now: NOW });
const candidates = (dir) => call({ fn: "candidates", dir });

// 2026-10-16T16:00:00Z.
const NOW = 1_792_166_400;
const stamp = (hoursAgo) => new Date((NOW - hoursAgo * 3600) * 1000).toISOString().replace(/\.\d{3}Z$/, "Z");
const done = (id, hoursAgo, conclusion, event = "schedule") => ({
  databaseId: id,
  status: "completed",
  conclusion,
  createdAt: stamp(hoursAgo),
  event,
});
const live = (id, hoursAgo, status = "in_progress") => ({
  databaseId: id,
  status,
  conclusion: null,
  createdAt: stamp(hoursAgo),
  event: "push",
});
const verdictOf = (runs) => decide({ "x.yml": runs })[0].slice(0, 2);

test("a red with no retry dispatches, and the red itself is never a retry", () => {
  assert.deepEqual(verdictOf([done(1, 30, "failure")]), ["x.yml", "dispatch"]);
  // A scheduled red minutes old still earns its first retry at the first push.
  assert.deepEqual(verdictOf([done(1, 0.1, "failure")]), ["x.yml", "dispatch"]);
  // So does one left by a hand dispatch older than the bound.
  assert.deepEqual(verdictOf([done(1, 7, "failure", "workflow_dispatch")]), ["x.yml", "dispatch"]);
});

test("a green newest run is green, whatever came before it", () => {
  assert.deepEqual(verdictOf([done(2, 1, "success"), done(1, 30, "failure")]), ["x.yml", "green"]);
  assert.deepEqual(verdictOf([]), ["x.yml", "green"]);
  assert.deepEqual(decide({}), []);
});

test("a retry within RETRY_HOURS holds; one past it dispatches again", () => {
  const red = done(1, 30, "failure");
  assert.deepEqual(verdictOf([done(2, 2, "failure", "workflow_dispatch"), red]), ["x.yml", "held"]);
  assert.deepEqual(verdictOf([done(2, 6, "failure", "workflow_dispatch"), red]), ["x.yml", "held"]);
  assert.deepEqual(verdictOf([done(2, 6.01, "failure", "workflow_dispatch"), red]), ["x.yml", "dispatch"]);
});

test("a queued or running run holds, however old the red", () => {
  const red = done(1, 30, "failure");
  assert.deepEqual(verdictOf([live(2, 0.1), red]), ["x.yml", "held"]);
  assert.deepEqual(verdictOf([live(2, 0.1, "queued"), red]), ["x.yml", "held"]);
  // Still held when the live run is the push's own, as deploy-site's is.
  const [, verdict, why] = decide({ "x.yml": [red, live(2, 0.1)] })[0];
  assert.equal(verdict, "held");
  assert.match(why, /run 2 is in_progress/);
});

test("skipped and cancelled are routine, not conclusions (ADR-0014)", () => {
  const red = done(1, 30, "failure");
  assert.deepEqual(verdictOf([done(3, 0.5, "cancelled"), done(2, 1, "skipped"), red]), ["x.yml", "dispatch"]);
  assert.deepEqual(verdictOf([done(2, 1, "cancelled"), done(1, 30, "success")]), ["x.yml", "green"]);
  assert.deepEqual(verdictOf([done(2, 1, "cancelled")]), ["x.yml", "green"]);
});

test("timed_out and startup_failure are red, like agent-alarm's", () => {
  assert.deepEqual(verdictOf([done(1, 30, "timed_out")]), ["x.yml", "dispatch"]);
  assert.deepEqual(verdictOf([done(1, 30, "startup_failure")]), ["x.yml", "dispatch"]);
  assert.deepEqual(verdictOf([done(1, 30, "neutral")]), ["x.yml", "green"]);
});

test("newest is sorted here, not trusted (#902)", () => {
  // Oldest first, the green in the middle: still the red is newest.
  assert.deepEqual(verdictOf([done(1, 30, "failure"), done(2, 20, "success"), done(3, 10, "failure")]), [
    "x.yml",
    "dispatch",
  ]);
  assert.deepEqual(verdictOf([done(1, 30, "failure"), done(3, 10, "success"), done(2, 20, "failure")]), [
    "x.yml",
    "green",
  ]);
});

test("verdicts keep the candidates' order, one per file", () => {
  const verdicts = decide({ "a.yml": [done(1, 30, "failure")], "b.yml": [done(2, 1, "success")] });
  assert.deepEqual(
    verdicts.map(([file, verdict]) => [file, verdict]),
    [
      ["a.yml", "dispatch"],
      ["b.yml", "green"],
    ],
  );
});

const cron = `on:\n  schedule:\n    - cron: "1 2 * * *"\n`;
const dispatch = `  workflow_dispatch:\n`;
const push = `  push:\n    branches: [main]\n`;
const job = (uses) => `jobs:\n  go:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: ${uses}\n`;
const CHECKOUT = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7";
const SEAT = "anthropics/claude-code-action@0123456789abcdef0123456789abcdef01234567 # v1";

function fixtureDir(files) {
  const dir = mkdtempSync(join(tmpdir(), "recheck-"));
  for (const [name, text] of Object.entries(files)) {
    writeFileSync(join(dir, name), text);
  }
  return dir;
}

test("a candidate carries a cron and a dispatch trigger and runs no model (#50)", () => {
  const dir = fixtureDir({
    "check.yml": `name: check\n${cron}${dispatch}${job(CHECKOUT)}`,
    "pushed.yml": `name: pushed\n${cron}${push}${dispatch}${job(CHECKOUT)}`,
    "seat.yml": `name: seat\n${cron}${dispatch}${job(SEAT)}`,
    "dispatch-only.yml": `name: d\non:\n${dispatch}${job(CHECKOUT)}`,
    "cron-only.yml": `name: c\n${cron}${job(CHECKOUT)}`,
    "list-on.yml": `name: l\non: [push]\n${job(CHECKOUT)}`,
    "notes.md": "not a workflow\n",
  });
  assert.deepEqual(candidates(dir), [["check.yml", "pushed.yml"], null]);
});

test("a workflow that cannot be parsed is UNREADABLE, never a shorter list", () => {
  const dir = fixtureDir({
    "check.yml": `name: check\n${cron}${dispatch}${job(CHECKOUT)}`,
    "broken.yml": "on: [\n",
  });
  const [files, error] = candidates(dir);
  assert.equal(files, null);
  assert.match(error, /^broken\.yml: /);
});

test("in this tree every cron workflow is a candidate and no seat is", () => {
  const [files, error] = candidates(join(scriptsDir, "..", "workflows"));
  assert.equal(error, null);
  for (const name of ["monster.yml", "coverage-check.yml", "corpus-fetch-check.yml", "fuzz-check.yml"]) {
    assert.ok(files.includes(name), `${name} is a scheduled check`);
  }
  for (const name of ["agent-bdfl.yml", "agent-heartbeat.yml", "agent-review.yml", "ci.yml", "agent-alarm.yml"]) {
    assert.ok(!files.includes(name), `${name} is never dispatched on the PAT`);
  }
});

// The stdout contract recheck.yml pipes into `xargs gh workflow run`: one
// file per line, nothing else, and nothing at all when a read failed.
function run(table, dir) {
  const bin = mkdtempSync(join(tmpdir(), "recheck-gh-"));
  const tablePath = join(bin, "table.json");
  writeFileSync(tablePath, JSON.stringify(table));
  const stub = join(bin, "stub.mjs");
  writeFileSync(
    stub,
    `import { readFileSync } from "node:fs";
const args = process.argv.slice(2);
const wf = args[args.indexOf("--workflow") + 1];
const table = JSON.parse(readFileSync(process.env.RECHECK_TABLE, "utf8"));
if (!(wf in table)) { process.stderr.write("HTTP 500: stub has no " + wf + "\\n"); process.exit(1); }
process.stdout.write(JSON.stringify(table[wf]));
`,
  );
  writeFileSync(join(bin, "gh"), `#!/bin/sh\nexec node ${stub} "$@"\n`);
  chmodSync(join(bin, "gh"), 0o755);
  return spawnSync("python3", [script, dir], {
    encoding: "utf8",
    env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, RECHECK_TABLE: tablePath },
  });
}

test("stdout is the dispatch list alone; stderr says why and counts", () => {
  const dir = fixtureDir({
    "red.yml": `name: red\n${cron}${dispatch}${job(CHECKOUT)}`,
    "green.yml": `name: green\n${cron}${dispatch}${job(CHECKOUT)}`,
    "held.yml": `name: held\n${cron}${dispatch}${job(CHECKOUT)}`,
  });
  const res = run(
    {
      "red.yml": [done(1, 30, "failure")],
      "green.yml": [done(2, 1, "success")],
      "held.yml": [done(4, 1, "failure", "workflow_dispatch"), done(3, 30, "failure")],
    },
    dir,
  );
  assert.equal(res.status, 0, res.stderr);
  assert.equal(res.stdout, "red.yml\n");
  assert.match(res.stderr, /^recheck: held\.yml: held, run 4 concluded failure/m);
  assert.match(res.stderr, /^recheck: red\.yml: dispatch, run 1 concluded failure/m);
  assert.doesNotMatch(res.stderr, /green\.yml/);
  assert.match(res.stderr, /^recheck: candidates 3 \| red 2 \| dispatched 1 \| held 1$/m);
});

test("a run list that cannot be read exits 1 with nothing on stdout", () => {
  const dir = fixtureDir({
    "red.yml": `name: red\n${cron}${dispatch}${job(CHECKOUT)}`,
    "unread.yml": `name: unread\n${cron}${dispatch}${job(CHECKOUT)}`,
  });
  const res = run({ "red.yml": [done(1, 30, "failure")] }, dir);
  assert.equal(res.status, 1);
  assert.equal(res.stdout, "");
  assert.match(res.stderr, /^recheck: UNREADABLE unread\.yml: HTTP 500/m);
});
