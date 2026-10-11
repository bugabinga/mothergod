// Fixtures for retrospect's trend block (#896): the thresholds in trend.py
// are rules the director reads every wake, so each one is pinned here
// against rows a stub table would hold, and the block's three shapes
// (rows past a threshold, one quiet line, UNREADABLE) each have a case.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const linesDriver = `
import json, sys
sys.path.insert(0, sys.argv[1])
import trend
recent, prior, gates = (json.loads(a) for a in sys.argv[2:5])
print("\\n".join(trend.lines(recent, prior, gates)))
`;

function block(recent, prior, gates = []) {
  const r = spawnSync(
    "python3",
    ["-c", linesDriver, scriptsDir, JSON.stringify(recent), JSON.stringify(prior), JSON.stringify(gates)],
    { encoding: "utf8" },
  );
  assert.equal(r.status, 0, r.stderr || r.stdout);
  return r.stdout;
}

const run = (role, turns, { denials = 0, error = 0 } = {}) => ({
  role,
  at: "2026-10-10T00:00:00Z",
  measured: 1,
  turns,
  out_tokens: 10 * turns,
  denials,
  error,
  cost_usd: 1,
});
const runs = (role, n, turns, extra) => Array.from({ length: n }, () => run(role, turns, extra));

test("a seat whose median turns climbed a quarter prints with every measure; a steady one is named quiet", () => {
  const out = block(
    [...runs("reviewer", 10, 14), ...runs("maintainer", 10, 18)],
    [...runs("reviewer", 10, 9), ...runs("maintainer", 10, 18)],
  );
  assert.match(out, /^trend, 7d vs the prior 7d, per role, the rows past a threshold in trend\.py:$/m);
  assert.match(
    out,
    /^ {4}reviewer {4}runs 10 vs 10 {3}turns p50 14\.0 vs 9\.0 {3}denials 0 vs 0 {3}errors 0 vs 0 {3}skipped 0 of 10 {3}moved: turns_p50$/m,
  );
  assert.match(out, /^ {4}quiet: maintainer$/m);
  assert.doesNotMatch(out, /^ {4}maintainer {4}runs/m);
});

test("denials halved from a real count print; a doubling from one does not", () => {
  const out = block(
    [...runs("reviewer", 10, 12, { denials: 6 }), ...runs("herald", 4, 20), run("herald", 20, { denials: 2 })],
    [...runs("reviewer", 10, 12, { denials: 14 }), ...runs("herald", 4, 20), run("herald", 20, { denials: 1 })],
  );
  assert.match(out, /^ {4}reviewer .*denials 60 vs 140 .*moved: denials$/m);
  assert.match(out, /^ {4}quiet: herald$/m);
});

test("a quiet fortnight is one line, with the role count", () => {
  const same = [...runs("reviewer", 10, 12), ...runs("bdfl", 4, 50)];
  const out = block(same, same);
  assert.equal(out.trim(), "trend: quiet, 2 roles within thresholds, 7d vs the prior 7d (trend.py)");
});

test("a new seat is news: runs against a prior of none, median against a dash", () => {
  const out = block([...runs("researcher", 4, 51), ...runs("bdfl", 4, 50)], runs("bdfl", 4, 50));
  assert.match(out, /^ {4}researcher {2}runs 4 vs 0 {3}turns p50 51\.0 vs - .*moved: runs$/m);
  assert.match(out, /^ {4}quiet: bdfl$/m);
});

test("three runs against none stay under the floor", () => {
  const out = block([...runs("researcher", 3, 51), ...runs("bdfl", 4, 50)], runs("bdfl", 4, 50));
  assert.match(out, /^trend: quiet, 2 roles/m);
});

test("the guard holding back a quarter of a seat's wakes prints, this week's gate rows alone", () => {
  const gates = [
    ...Array.from({ length: 6 }, () => ({ role: "bdfl", decision: "run" })),
    { role: "bdfl", decision: "skip" },
    { role: "bdfl", decision: "exhausted" },
    { role: "reviewer", decision: "skip" },
  ];
  const out = block([...runs("bdfl", 6, 50), ...runs("reviewer", 10, 12)], [
    ...runs("bdfl", 6, 50),
    ...runs("reviewer", 10, 12),
  ], gates);
  assert.match(out, /^ {4}bdfl {8}runs 6 vs 6 .*skipped 2 of 8 {3}moved: skipped$/m);
  assert.match(out, /^ {4}quiet: reviewer$/m);
});

test("an error appearing prints, and so does one vanishing", () => {
  const out = block(
    [...runs("maintainer", 9, 18), run("maintainer", 18, { error: 1 })],
    runs("maintainer", 10, 18),
  );
  assert.match(out, /^ {4}maintainer .*errors 1 vs 0 .*moved: errors$/m);
  const back = block(runs("maintainer", 10, 18), [...runs("maintainer", 9, 18), run("maintainer", 18, { error: 1 })]);
  assert.match(back, /errors 0 vs 1 .*moved: errors$/m);
});

const reportDriver = `
import json, sys
from datetime import datetime, timezone
sys.path.insert(0, sys.argv[1])
import d1, trend
mode = sys.argv[2]
calls = []
def query(sql, params=()):
    calls.append([sql, list(params)])
    if mode == "unreadable":
        raise d1.Unreadable("HTTP 500: edge")
    if "FROM guard" in sql:
        return []
    row = lambda at: {"role": "bdfl", "at": at, "measured": 1, "turns": 10, "out_tokens": 1, "denials": 0, "error": 0, "cost_usd": 1}
    return [row("2026-10-04T12:00:00Z")] * 4 + [row("2026-10-04T11:59:59Z")]
trend.report(now=datetime(2026, 10, 11, 12, 0, tzinfo=timezone.utc), query=query)
print(json.dumps(calls))
`;

function reportWith(mode) {
  const r = spawnSync("python3", ["-c", reportDriver, scriptsDir, mode], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr || r.stdout);
  return r.stdout;
}

test("report asks two weeks of sessions and one of gates, and splits at the week boundary inclusive", () => {
  const out = reportWith("rows");
  const calls = JSON.parse(out.trim().split("\n").pop());
  assert.equal(calls.length, 2);
  assert.match(calls[0][0], /FROM sessions WHERE at >= \?/);
  assert.deepEqual(calls[0][1], ["2026-09-27T12:00:00Z"]);
  assert.match(calls[1][0], /FROM guard WHERE at >= \?/);
  assert.deepEqual(calls[1][1], ["2026-10-04T12:00:00Z"]);
  // Four rows at the boundary instant are this week's, the one before it is prior's.
  assert.match(out, /^ {4}bdfl {8}runs 4 vs 1 .*moved: runs$/m);
});

test("an unreadable table prints UNREADABLE and the retrospect goes on", () => {
  const out = reportWith("unreadable");
  assert.match(
    out,
    /^trend: UNREADABLE \(HTTP 500: edge\); the session table could not be read, so no week is compared\.$/m,
  );
  assert.doesNotMatch(out, /quiet/);
});
