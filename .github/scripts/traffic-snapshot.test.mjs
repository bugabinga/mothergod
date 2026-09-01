// Fixtures for traffic-snapshot, the ledger merge behind issue #417.
//
// The property that matters: the ledger never loses a day it has seen.
// GitHub forgets after 14 days, so a merge bug here is permanent data
// loss with no second source to recover from. build() is pure so these
// can exist at all.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
loader = importlib.machinery.SourceFileLoader("traffic_snapshot", sys.argv[1] + "/traffic-snapshot.py")
spec = importlib.util.spec_from_loader("traffic_snapshot", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
call = json.loads(sys.stdin.read())
print(mod.build(call["prior"], call["views"], call["clones"], "https://example.test/run/1"))
`;

const run = (prior, views, clones) => {
  const res = spawnSync("python3", ["-c", driver, scriptsDir], {
    input: JSON.stringify({ prior, views, clones }),
    encoding: "utf-8",
  });
  return { status: res.status, body: res.stdout, err: res.stderr };
};

const daysOf = (body) => {
  const m = body.match(/```json\n([\s\S]*?)\n```/);
  assert.ok(m, "body carries a json block");
  return JSON.parse(m[1]);
};

const views = (buckets, count, uniques) => ({
  count,
  uniques,
  views: buckets.map(([timestamp, c, u]) => ({ timestamp, count: c, uniques: u })),
});
const clones = (buckets, count, uniques) => ({
  count,
  uniques,
  clones: buckets.map(([timestamp, c, u]) => ({ timestamp, count: c, uniques: u })),
});

const T = (d) => `${d}T00:00:00Z`;

test("first run: empty prior starts the series", () => {
  const r = run(
    "",
    views([[T("2026-08-20"), 5, 2]], 5, 2),
    clones([[T("2026-08-20"), 7, 3]], 7, 3),
  );
  assert.equal(r.status, 0, r.err);
  assert.deepEqual(daysOf(r.body).days, {
    "2026-08-20": { views: 5, view_uniques: 2, clones: 7, clone_uniques: 3 },
  });
});

test("overlap: newest snapshot wins per day, absent days survive", () => {
  // 08-20 was partial (5 views) last week and complete (9) now; 08-13
  // aged out of the fresh window and must survive on stored data alone.
  const first = run(
    "",
    views([[T("2026-08-13"), 4, 1], [T("2026-08-20"), 5, 2]], 9, 3),
    clones([[T("2026-08-13"), 1, 1]], 1, 1),
  );
  const second = run(
    first.body,
    views([[T("2026-08-20"), 9, 4]], 9, 4),
    clones([[T("2026-08-21"), 2, 2]], 2, 2),
  );
  assert.equal(second.status, 0, second.err);
  assert.deepEqual(daysOf(second.body).days, {
    "2026-08-13": { views: 4, view_uniques: 1, clones: 1, clone_uniques: 1 },
    "2026-08-20": { views: 9, view_uniques: 4 },
    "2026-08-21": { clones: 2, clone_uniques: 2 },
  });
});

test("metrics merge independently on the same day", () => {
  const first = run("", views([[T("2026-08-20"), 5, 2]], 5, 2), clones([], 0, 0));
  const second = run(first.body, views([], 0, 0), clones([[T("2026-08-20"), 7, 3]], 7, 3));
  assert.equal(second.status, 0, second.err);
  assert.deepEqual(daysOf(second.body).days["2026-08-20"], {
    views: 5,
    view_uniques: 2,
    clones: 7,
    clone_uniques: 3,
  });
});

test("non-empty prior without a json block fails instead of dropping the series", () => {
  const r = run("a hand-edited body", views([], 0, 0), clones([], 0, 0));
  assert.equal(r.status, 1);
  assert.match(r.err, /refusing to drop the series/);
});

test("unexpected payload shape fails loudly", () => {
  const r = run("", { count: 1 }, clones([], 0, 0));
  assert.equal(r.status, 1);
  assert.match(r.err, /payload shape unexpected/);
});

test("body over budget trims oldest days and records the horizon", () => {
  // ~1200 stored days overflow the 60000-byte budget by construction.
  const buckets = [];
  for (let i = 0; i < 1200; i++) {
    const d = new Date(Date.UTC(2020, 0, 1) + i * 86400000).toISOString();
    buckets.push([d, 100 + i, 10]);
  }
  const r = run("", views(buckets, 1, 1), clones(buckets, 1, 1));
  assert.equal(r.status, 0, r.err);
  assert.ok(r.body.length <= 60000, `body is ${r.body.length} bytes`);
  const state = daysOf(r.body);
  const kept = Object.keys(state.days).sort();
  assert.equal(state.trimmed_before, kept[0]);
  // Newest data survives the trim; only the oldest end pays.
  assert.ok(kept[kept.length - 1].startsWith("2023-04"));
});
