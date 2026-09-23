// Fixtures for mutants-debt: a red that lands after the merge must become an
// issue, and every other state must not. `decide` and `render` are pure so
// both failure directions pin without spawning gh: filing on an open PR
// duplicates the annotations the review already reads; not filing on a
// merged one is the silent loss the script exists to end (#676, #677, #690).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
loader = importlib.machinery.SourceFileLoader("mutants_debt", sys.argv[1] + "/mutants-debt")
spec = importlib.util.spec_from_loader("mutants_debt", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
raw = json.loads(sys.argv[2])
if raw["fn"] == "decide":
    verdict, detail = mod.decide(raw["missed"], raw["merged"], raw["filed"])
    print(json.dumps({"verdict": verdict, "detail": detail}))
else:
    title, body = mod.render(raw["pr"], raw["missed"], raw["generated"], raw["decided"], raw["run_url"])
    print(json.dumps({"title": title, "body": body}))
`;

function call(payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const survivors = [
  "src/literal.rs:952:55: replace + with - in Literal::mix_ppm",
  "src/codec.rs:608:9: replace <impl TokenSink for PpmExpertCostSink<'_>>::flag with ()",
];

test("no survivors files nothing, whatever the PR state", () => {
  for (const merged of [true, false, null]) {
    assert.equal(call({ fn: "decide", missed: [], merged, filed: null }).verdict, "no survivors");
  }
});

test("survivors on an open PR stay on the diff", () => {
  const out = call({ fn: "decide", missed: survivors, merged: false, filed: null });
  assert.equal(out.verdict, "open");
  assert.match(out.detail, /2 survivor/);
});

test("survivors on a merged PR are filed once", () => {
  assert.equal(call({ fn: "decide", missed: survivors, merged: true, filed: null }).verdict, "file");
  const dup = call({ fn: "decide", missed: survivors, merged: true, filed: 700 });
  assert.equal(dup.verdict, "filed already");
  assert.match(dup.detail, /#700/);
});

test("an unreadable PR state is its own verdict, never silently open", () => {
  assert.equal(call({ fn: "decide", missed: survivors, merged: null, filed: null }).verdict, "unreadable");
});

test("render names the PR, the files, and every survivor; partial runs say so", () => {
  const full = call({
    fn: "render",
    pr: 690,
    missed: survivors,
    generated: 104,
    decided: 104,
    run_url: "https://x/run/1",
  });
  assert.equal(full.title, "mutants: 2 survivors outlived PR #690 (src/codec.rs, src/literal.rs)");
  for (const line of survivors) assert.ok(full.body.includes(line), line);
  assert.ok(full.body.includes("https://x/run/1"));
  assert.ok(!full.body.includes("never ran"), "a complete run must not claim undecided mutants");

  const partial = call({
    fn: "render",
    pr: 690,
    missed: survivors,
    generated: 104,
    decided: 84,
    run_url: "u",
  });
  assert.ok(partial.body.includes("20 of 104 mutants never ran"));

  const one = call({ fn: "render", pr: 5, missed: [survivors[0]], generated: null, decided: null, run_url: "u" });
  assert.equal(one.title, "mutants: 1 survivor outlived PR #5 (src/literal.rs)");
  assert.ok(!one.body.includes("never ran"), "unknown totals must not invent a shortfall");
});
