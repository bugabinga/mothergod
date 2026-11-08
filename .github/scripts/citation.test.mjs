// The A2 predicate is one regex behind three strips, and a matcher's contract
// only exists as its cases. These pin what counts as a bare hash (the four
// shapes the 2026-11-07/08 retrospects read by hand) and what must not: the
// cited form, a backticked pin, a URL, a fenced block, a verdict stamp, a
// run id, a hex-only word.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import json, sys
sys.path.insert(0, sys.argv[1])
import citation
print(json.dumps(citation.bare_hashes(sys.argv[2])))
`;

function bare(body) {
  const result = spawnSync("python3", ["-I", "-c", driver, scriptsDir, body], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("a bare short hash in prose is found", () => {
  assert.deepEqual(bare("746cf1e published the first reading."), ["746cf1e"]);
});

test("a bare full hash in prose is found", () => {
  const full = "d1527c8f67ef9af357290475d4af4b0fab8f12e0";
  assert.deepEqual(bare(`read at ${full} by hand`), [full]);
});

test("several bare hashes come back in order, once each", () => {
  assert.deepEqual(
    bare("Run 42383982473 at 6283612a; the version 20 pass at 3e32118 took 178 (9abbf2b), and 3e32118 again."),
    ["6283612a", "3e32118", "9abbf2b"],
  );
});

test("the cited form is not bare, with straight or curly quotes", () => {
  assert.deepEqual(bare("6ef1f88 (\"bench: frontier sections print the share (#1925)\") prints them."), []);
  assert.deepEqual(bare("3e32118 (“bench: time -4 apart”) prints them."), []);
});

test("a backticked pin is not bare", () => {
  assert.deepEqual(bare("Executed on `04b50da`, through probe."), []);
});

test("a hash inside a fenced block or an HTML comment is not bare", () => {
  assert.deepEqual(bare("```\nindex 5f0bb4b..61391a9 100644\n```\n<!-- verdict pass a1b2c3d4 -->"), []);
});

test("a URL and an action pin are not bare", () => {
  assert.deepEqual(
    bare("https://github.com/o/r/commit/5f0bb4b and actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"),
    [],
  );
});

test("a 64-hex verdict stamp, a decimal run id and a hex-only word are not bare", () => {
  const stamp = "022362c6f3a38bce38d25d9ef065f56fd1b553881e3259aeef37153bc1f0f26b";
  assert.deepEqual(bare(`stamp ${stamp}, run 42431032218, effaced and defaced`), []);
});

test("neither half of a two- or three-dot range is bare, a hash at a sentence end is", () => {
  assert.deepEqual(bare("git log 5f0bb4b..61391a9 shows it."), []);
  assert.deepEqual(bare("git diff 5f0bb4b...61391a9 shows it."), []);
  assert.deepEqual(bare("The pass that contains df405fc."), ["df405fc"]);
});

test("a single dot before a hash does not exempt it: only a range does", () => {
  assert.deepEqual(bare("Read it.df405fc carried the fix."), ["df405fc"]);
});
