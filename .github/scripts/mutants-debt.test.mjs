// Fixtures for mutants-debt: a red that lands after the merge must become an
// issue, and every other state must not. `decide`, `render` and
// `in_territory` are pure so both failure directions pin without spawning
// gh: filing on an open PR duplicates the annotations the review already
// reads; not filing a decode-safety survivor on a merged one is the silent
// loss the script exists to end (#676, #677, #690); filing a scaffold
// survivor is the debt ADR-0056 retired.
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
    verdict, detail = mod.decide(raw["missed"], raw.get("outside", 0), raw["merged"], raw["filed"])
    print(json.dumps({"verdict": verdict, "detail": detail}))
elif raw["fn"] == "territory":
    print(json.dumps([mod.in_territory(line) for line in raw["lines"]]))
else:
    title, body = mod.render(raw["pr"], raw["missed"], raw.get("outside", 0), raw["generated"], raw["decided"], raw["run_url"])
    print(json.dumps({"title": title, "body": body}))
`;

function call(payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

// Decode-safety territory: filed after a merge (ADR-0056).
const survivors = [
  "src/codec.rs:555:5: replace ensure_room -> Result<(), Error> with Ok(())",
  "src/coder.rs:303:26: replace & with | in Decoder<'a>::decode",
];
// Scaffold territory: a map reading, never filed.
const scaffold = [
  "src/literal.rs:952:55: replace + with - in Literal::mix_ppm",
  "src/codec.rs:608:9: replace <impl TokenSink for PpmExpertCostSink<'_>>::flag with ()",
];

test("territory is read off the function the line names, cheap direction is under-filing", () => {
  const lines = [
    ...survivors,
    "src/lib.rs:332:1: replace check_stored_bound -> Result<(), Error> with Ok(())",
    "src/codec.rs:418:1: replace undo_filter -> Result<Vec<u8>, Error> with Ok(vec![])",
    "src/codec.rs:897:1: replace read_u32_le -> Result<u32, Error> with Ok(0)",
    ...scaffold,
    "src/lz.rs:1200:9: replace < with <= in dp_round",
    // Proven override (#854 review): bare substring match on "bound" would
    // otherwise file SSE's logit-clamp constant as decode-safety.
    "src/sse.rs:315:1: replace stretch_bound -> f64 with 1.0",
  ];
  const out = call({ fn: "territory", lines });
  assert.deepEqual(out, [true, true, true, true, true, false, false, false, false]);
});

test("scaffold-only survivors are not filed, whatever the PR state", () => {
  for (const merged of [true, false, null]) {
    const out = call({ fn: "decide", missed: [], outside: scaffold.length, merged, filed: null });
    assert.equal(out.verdict, "scaffold");
    assert.match(out.detail, /2 survivor/);
  }
});

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
  assert.equal(full.title, "mutants: 2 survivors outlived PR #690 (src/codec.rs, src/coder.rs)");
  for (const line of survivors) assert.ok(full.body.includes(line), line);
  assert.ok(full.body.includes("https://x/run/1"));
  assert.ok(!full.body.includes("never ran"), "a complete run must not claim undecided mutants");
  assert.ok(!full.body.includes("outside decode-safety"), "no scaffold survivors, no scaffold line");

  const mixed = call({
    fn: "render",
    pr: 690,
    missed: survivors,
    outside: scaffold.length,
    generated: 104,
    decided: 104,
    run_url: "u",
  });
  assert.equal(mixed.title, "mutants: 2 survivors outlived PR #690 (src/codec.rs, src/coder.rs)");
  assert.ok(mixed.body.includes("2 more survivors sit outside decode-safety territory"));
  for (const line of scaffold) assert.ok(!mixed.body.includes(line), "scaffold lines stay off the issue");

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
  assert.equal(one.title, "mutants: 1 survivor outlived PR #5 (src/codec.rs)");
  assert.ok(!one.body.includes("never ran"), "unknown totals must not invent a shortfall");
});
