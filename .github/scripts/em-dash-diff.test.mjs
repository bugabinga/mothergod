// Fixture diffs for em-dash-diff: the rule is one code point on an added
// line of a prose file, and its contract is the hunk arithmetic (the line
// number the message names must be the line the author opens) plus the two
// directions: a removed or context line, or a code file, is never a finding.
// Each case spawns the real script on stdin, the way ci.yml's job does.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const script = new URL("em-dash-diff", import.meta.url).pathname;

function run(diff) {
  return spawnSync(script, [], { input: diff, encoding: "utf8" });
}

// One file's diff, from the hunks given; `from` is `/dev/null` for a new file.
function file(path, hunks, { from = `a/${path}`, to = `b/${path}` } = {}) {
  return [`diff --git a/${path} b/${path}`, "index 1111111..2222222 100644", `--- ${from}`, `+++ ${to}`, ...hunks].join(
    "\n",
  ) + "\n";
}

const DASH = "—";

test("an added markdown line carrying the dash is reported at its new-side line", () => {
  const r = run(file("research/JOURNAL.md", ["@@ -40,0 +41,2 @@ ## Laws", "+first added", `+second ${DASH} added`]));
  assert.equal(r.status, 1);
  assert.equal(r.stdout.split("\n")[0], `research/JOURNAL.md:42: second ${DASH} added`);
  assert.match(r.stdout, /added lines carrying an em dash \(U\+2014\): 1\. CLAUDE\.md Voice/);
});

test("line numbers follow each hunk header, with context and removed lines counted right", () => {
  const diff = file("docs/TESTING.md", [
    "@@ -3,2 +3,2 @@",
    " context",
    "-removed",
    "+kept, clean",
    "@@ -10,1 +10,3 @@",
    `-old ${DASH} gone`,
    "+ten",
    "+eleven",
    `+twelve ${DASH} here`,
  ]);
  const r = run(diff);
  assert.equal(r.status, 1);
  assert.deepEqual(r.stdout.split("\n").slice(0, 1), [`docs/TESTING.md:12: twelve ${DASH} here`]);
});

test("a removed line, a context line and an en dash are not findings", () => {
  const diff = file("README.md", [
    "@@ -1,2 +1,2 @@",
    ` kept ${DASH} context`,
    `-gone ${DASH} removed`,
    "+a 3–6 line summary",
  ]);
  const r = run(diff);
  assert.equal(r.status, 0, r.stdout);
  assert.equal(r.stdout, "");
});

test("a new prose file is read from line 1; a deleted one is not read", () => {
  const added = file("docs/adr/0099-x.md", ["@@ -0,0 +1,2 @@", `+# ADR ${DASH} title`, "+body"], { from: "/dev/null" });
  const r = run(added);
  assert.equal(r.status, 1);
  assert.equal(r.stdout.split("\n")[0], `docs/adr/0099-x.md:1: # ADR ${DASH} title`);
  const deleted = file("docs/old.md", ["@@ -1,1 +0,0 @@", `-was ${DASH} here`], { to: "/dev/null" });
  assert.equal(run(deleted).status, 0);
});

test("an added HTML line is a finding: the site is posted text (#1653)", () => {
  const r = run(file("site/index.html", ["@@ -5,0 +6,1 @@", `+<title>mothergod ${DASH} compression</title>`]));
  assert.equal(r.status, 1);
  assert.equal(r.stdout.split("\n")[0], `site/index.html:6: <title>mothergod ${DASH} compression</title>`);
});

test("code files carry the character as data and are outside the rule", () => {
  const diff = file(".github/scripts/deny-em-dash.test.mjs", ["@@ -1,0 +2,1 @@", `+const s = "${DASH}";`])
    + file("src/lz.rs", ["@@ -1,0 +2,1 @@", `+// a ${DASH} b`])
    + file(".github/workflows/agent-bdfl.yml", ["@@ -1,0 +2,1 @@", `+            prose ${DASH} prompt`]);
  const r = run(diff);
  assert.equal(r.status, 0, r.stdout);
});

test("the JSON escape counts in a prose file, because JSON lines and TOML decode it", () => {
  const diff = file("research/progress.jsonl", ["@@ -9,0 +10,1 @@", "+{\"id\":\"S9\",\"note\":\"x \\u2014 y\"}"])
    + file("bench/corpus.toml", ["@@ -9,0 +10,1 @@", "+description = \"a \\u2014 b\""]);
  const r = run(diff);
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^research\/progress\.jsonl:10: /m);
  assert.match(r.stdout, /^bench\/corpus\.toml:10: /m);
  assert.match(r.stdout, /\(U\+2014\): 2\./);
});

test("content that looks like a header inside a hunk does not close the file", () => {
  // A removed line `-- x` renders as `--- x` and an added `++ y` as `+++ y`.
  const diff = file("docs/format/SPEC.md", ["@@ -5,1 +5,2 @@", "--- x", "+++ y", `+then ${DASH} this`]);
  const r = run(diff);
  assert.equal(r.status, 1);
  assert.equal(r.stdout.split("\n")[0], `docs/format/SPEC.md:6: then ${DASH} this`);
});

test("a path that is not under the b/ prefix, and an empty diff, find nothing", () => {
  assert.equal(run("").status, 0);
  const odd = ["diff --git x.md x.md", "--- x.md", "+++ x.md", "@@ -1,0 +2,1 @@", `+a ${DASH} b`].join("\n");
  assert.equal(run(odd).status, 0);
});
