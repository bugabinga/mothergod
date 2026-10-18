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
recent, prior, gates, walls = (json.loads(a) for a in sys.argv[2:6])
print("\\n".join(trend.lines(recent, prior, gates, walls)))
`;

function block(recent, prior, gates = [], walls = []) {
  const r = spawnSync(
    "python3",
    ["-c", linesDriver, scriptsDir, ...[recent, prior, gates, walls].map((a) => JSON.stringify(a))],
    { encoding: "utf8" },
  );
  assert.equal(r.status, 0, r.stderr || r.stdout);
  return r.stdout;
}

// `writes` absent is a row from before the column (#896 item 5), null in the table.
const run = (role, turns, { denials = 0, error = 0, writes } = {}) => ({
  role,
  at: "2026-10-10T00:00:00Z",
  measured: 1,
  turns,
  out_tokens: 10 * turns,
  denials,
  error,
  cost_usd: 1,
  ...(writes === undefined ? {} : { writes }),
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
    /^ {4}reviewer {4}runs 10 vs 10 {3}turns p50 14\.0 vs 9\.0 {3}denials 0 vs 0 {3}errors 0 vs 0 {3}no-op 0 of 0 {3}skipped 0 of 10 {3}moved: turns_p50$/m,
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

test("a median that doubled from 2 to 4 stays quiet under the floor of 3 turns", () => {
  const out = block(runs("reviewer", 10, 4), runs("reviewer", 10, 2));
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("three turns on a median of 23 is not a quarter, so it stays quiet", () => {
  const out = block(runs("reviewer", 10, 23), runs("reviewer", 10, 20));
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("denials 15 against 10 are past the floor and under the factor, so they stay quiet", () => {
  const out = block(
    [...runs("reviewer", 5, 12, { denials: 3 })],
    [...runs("reviewer", 5, 12, { denials: 2 })],
  );
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("runs 14 against 10 are under the factor, so they stay quiet", () => {
  const out = block(runs("reviewer", 14, 12), runs("reviewer", 10, 12));
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("runs at exactly 1.5x print: 6 against 4", () => {
  const out = block(runs("reviewer", 6, 12), runs("reviewer", 4, 12));
  assert.match(out, /^ {4}reviewer {4}runs 6 vs 4 .*moved: runs$/m);
});

test("a median moved by exactly a quarter prints: 20 against 15", () => {
  const out = block(runs("reviewer", 10, 20), runs("reviewer", 10, 15));
  assert.match(out, /turns p50 20\.0 vs 15\.0 .*moved: turns_p50$/m);
});

test("a median moved by exactly the floor prints: 12 against 9", () => {
  const out = block(runs("reviewer", 10, 12), runs("reviewer", 10, 9));
  assert.match(out, /turns p50 12\.0 vs 9\.0 .*moved: turns_p50$/m);
});

test("22 turns on a median of 100 is under a quarter, so it stays quiet", () => {
  const out = block(runs("reviewer", 10, 100), runs("reviewer", 10, 78));
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("denials at exactly the floor print: 3 against none", () => {
  const out = block([...runs("reviewer", 9, 12), run("reviewer", 12, { denials: 3 })], runs("reviewer", 10, 12));
  assert.match(out, /denials 3 vs 0 .*moved: denials$/m);
});

test("denials at exactly the factor print: 20 against 10", () => {
  const out = block(runs("reviewer", 10, 12, { denials: 2 }), runs("reviewer", 10, 12, { denials: 1 }));
  assert.match(out, /denials 20 vs 10 .*moved: denials$/m);
});

test("the guard holding back one wake in five stays quiet", () => {
  const gates = [...Array.from({ length: 4 }, () => ({ role: "bdfl", decision: "run" })), {
    role: "bdfl",
    decision: "skip",
  }];
  const out = block(runs("bdfl", 4, 50), runs("bdfl", 4, 50), gates);
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("a paused wake is held back: one in four prints", () => {
  const gates = [...Array.from({ length: 3 }, () => ({ role: "bdfl", decision: "run" })), {
    role: "bdfl",
    decision: "paused",
  }];
  const out = block(runs("bdfl", 3, 50), runs("bdfl", 3, 50), gates);
  assert.match(out, /^ {4}bdfl {8}runs 3 vs 3 .*skipped 1 of 4 {3}moved: skipped$/m);
});

test("an empty read is a finding, never a quiet one", () => {
  const out = block([], []);
  assert.equal(
    out.trim(),
    "trend: no sessions in the last 14d and no gate rows in the last 7d; the table answered and held nothing",
  );
  assert.doesNotMatch(out, /quiet/);
});

test("three runs against none stay under the floor", () => {
  const out = block([...runs("researcher", 3, 51), ...runs("bdfl", 4, 50)], runs("bdfl", 4, 50));
  assert.match(out, /^trend: quiet, 2 roles/m);
});

test("the guard holding back a quarter of a seat's wakes prints; a stale wake is not held back", () => {
  const gates = [
    ...Array.from({ length: 6 }, () => ({ role: "bdfl", decision: "run" })),
    { role: "bdfl", decision: "skip" },
    { role: "bdfl", decision: "exhausted" },
    { role: "bdfl", decision: "stale" },
    { role: "reviewer", decision: "skip" },
  ];
  const out = block([...runs("bdfl", 6, 50), ...runs("reviewer", 10, 12)], [
    ...runs("bdfl", 6, 50),
    ...runs("reviewer", 10, 12),
  ], gates);
  assert.match(out, /^ {4}bdfl {8}runs 6 vs 6 .*skipped 2 of 8 {3}moved: skipped$/m);
  assert.match(out, /^ {4}quiet: reviewer$/m);
});

// #896 item 5, operator steer of 2026-10-03: a seat that wakes and changes
// nothing half the time is a deletion candidate named by name. The share
// is over runs with writes recorded; rows from before the column are null
// and count on neither side, so the column's first week prints on what it
// has, never on a history of zeros.
test("a seat that changed nothing in half its recorded runs prints: 4 of 8", () => {
  const out = block(
    [...runs("curator", 4, 20, { writes: 0 }), ...runs("curator", 4, 20, { writes: 2 })],
    runs("curator", 8, 20, { writes: 1 }),
  );
  assert.match(out, /^ {4}curator {5}runs 8 vs 8 .*errors 0 vs 0 {3}no-op 4 of 8 {3}skipped 0 of 8 {3}moved: no-op$/m);
});

test("3 no-ops of 8 recorded is under half, so it stays quiet", () => {
  const out = block(
    [...runs("curator", 3, 20, { writes: 0 }), ...runs("curator", 5, 20, { writes: 1 })],
    runs("curator", 8, 20, { writes: 1 }),
  );
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("2 no-ops of 3 recorded is under the floor of 4, so it stays quiet", () => {
  const out = block(
    [...runs("curator", 2, 20, { writes: 0 }), run("curator", 20, { writes: 1 }), ...runs("curator", 5, 20)],
    runs("curator", 8, 20),
  );
  assert.match(out, /^trend: quiet, 1 roles/m);
});

test("rows without the column are neither no-ops nor recorded: 4 zeros among 10 runs print as 4 of 4", () => {
  const out = block(
    [...runs("curator", 4, 20, { writes: 0 }), ...runs("curator", 6, 20)],
    runs("curator", 10, 20),
  );
  assert.match(out, /^ {4}curator {5}runs 10 vs 10 .*no-op 4 of 4 .*moved: no-op$/m);
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

// #1241: a denials row that moved names the walls under it, so the reader
// goes to the guard instead of a shrug. The unit is runs that hit each
// wall, because a run's reasons are distinct triples (walls.py); a seat
// whose artifacts predate the reasons record (#1110) prints the absence,
// and a seat whose denials held still prints no walls line at all.
test("a denials row that moved names its walls, most runs first, its own role's only", () => {
  const out = block(
    runs("reviewer", 10, 12, { denials: 6 }),
    runs("reviewer", 10, 12, { denials: 1 }),
    [],
    [
      { role: "reviewer", wall: "deny-unpushed-work", runs: 3 },
      { role: "bdfl", wall: "rule", runs: 1 },
      { role: "reviewer", wall: "deny-em-dash", runs: 9 },
    ],
  );
  assert.match(
    out,
    /^ {4}reviewer .*moved: denials\n {16}walls, runs that hit each: deny-em-dash 9, deny-unpushed-work 3$/m,
  );
  assert.doesNotMatch(out, /rule/);
});

test("a denials row that moved with no wall recorded says so; a steady one prints no walls line", () => {
  const moved = block(runs("reviewer", 10, 12, { denials: 6 }), runs("reviewer", 10, 12, { denials: 1 }));
  assert.match(moved, /^ {4}reviewer .*moved: denials\n {16}walls: none recorded/m);
  const steady = block(runs("reviewer", 10, 14, { denials: 2 }), runs("reviewer", 10, 9, { denials: 2 }));
  assert.match(steady, /moved: turns_p50$/m);
  assert.doesNotMatch(steady, /walls/);
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
    if "FROM guard" in sql or "FROM walls" in sql:
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

test("report asks two weeks of sessions and one of gates and walls, and splits at the week boundary inclusive", () => {
  const out = reportWith("rows");
  const calls = JSON.parse(out.trim().split("\n").pop());
  assert.equal(calls.length, 3);
  assert.match(calls[0][0], /FROM sessions WHERE at >= \?/);
  assert.deepEqual(calls[0][1], ["2026-09-27T12:00:00Z"]);
  assert.match(calls[1][0], /FROM guard WHERE at >= \?/);
  assert.deepEqual(calls[1][1], ["2026-10-04T12:00:00Z"]);
  assert.match(calls[2][0], /FROM walls w JOIN sessions s ON .* WHERE s\.at >= \? GROUP BY s\.role, w\.wall$/);
  assert.deepEqual(calls[2][1], ["2026-10-04T12:00:00Z"]);
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
