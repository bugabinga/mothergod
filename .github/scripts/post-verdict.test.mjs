// post-verdict applies the reviewer's verdict as one shape: the label pair
// flips, then the comment goes through gh-comment stamped with the id
// verdict-stands reads (#953, #1050). Both writes are argv-shaped, so a stub
// `gh` on PATH that appends every call it sees is the whole harness.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = join(new URL(".", import.meta.url).pathname, "post-verdict");
const ID = "c".repeat(64);

function run(args, { body = "one [must]: the test is missing", env = {}, failEdit = false } = {}) {
  const stub = mkdtempSync(join(tmpdir(), "post-verdict-"));
  const log = join(stub, "calls.jsonl");
  const recorder = join(stub, "record.mjs");
  writeFileSync(
    recorder,
    `import { appendFileSync } from "node:fs";
const argv = process.argv.slice(2);
appendFileSync(${JSON.stringify(log)}, JSON.stringify({ argv, token: process.env.GH_TOKEN ?? null }) + "\\n");
if (argv[1] === "edit" && ${failEdit}) { console.error("label not found"); process.exit(1); }
console.log("https://github.com/o/r/pull/1#issuecomment-9");
`,
  );
  writeFileSync(join(stub, "gh"), `#!/bin/sh\nexec node ${recorder} "$@"\n`);
  chmodSync(join(stub, "gh"), 0o755);
  const proc = spawnSync("python3", [script, ...args], {
    encoding: "utf8",
    input: body,
    env: {
      PATH: `${stub}:${process.env.PATH}`,
      ROLE: "reviewer",
      GH_TOKEN: "app-token",
      GH_WORKFLOW_TOKEN: "workflow-token",
      DIFF_ID: ID,
      ...env,
    },
  });
  let calls = [];
  try {
    calls = readFileSync(log, "utf8").trim().split("\n").map((line) => JSON.parse(line));
  } catch {
    // gh was never reached; the refusal tests assert exactly that.
  }
  return { ...proc, calls };
}

test("fail flips the pair to changes-requested, then posts the stamped items", () => {
  const { status, stdout, calls } = run(["950", "fail"]);
  assert.equal(status, 0, stdout);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0].argv, [
    "pr",
    "edit",
    "950",
    "--add-label",
    "changes-requested",
    "--remove-label",
    "agent-approved",
  ]);
  assert.equal(calls[0].token, "app-token");
  assert.deepEqual(calls[1].argv.slice(0, 3), ["issue", "comment", "950"]);
  assert.match(
    calls[1].argv[4],
    new RegExp(
      `^one \\[must\\]: the test is missing\\n\\n<!-- verdict fail ${ID} -->\\n\\n---\\n_reviewer · \\[Claude Code\\]`,
    ),
  );
  assert.equal(calls[1].token, "workflow-token");
  assert.equal(stdout.trim(), "https://github.com/o/r/pull/1#issuecomment-9");
});

test("pass flips the pair to agent-approved and stamps pass", () => {
  const { status, calls } = run(["950", "pass"], { body: "holds; ran the round-trip" });
  assert.equal(status, 0);
  assert.deepEqual(calls[0].argv, [
    "pr",
    "edit",
    "950",
    "--add-label",
    "agent-approved",
    "--remove-label",
    "changes-requested",
  ]);
  assert.match(calls[1].argv[4], new RegExp(`<!-- verdict pass ${ID} -->`));
});

test("an empty DIFF_ID posts unstamped and says so, never withholds the verdict", () => {
  const { status, stderr, calls } = run(["950", "fail"], { env: { DIFF_ID: "" } });
  assert.equal(status, 0);
  assert.equal(calls.length, 2);
  assert.doesNotMatch(calls[1].argv[4], /<!-- verdict/);
  assert.match(stderr, /DIFF_ID is empty, posting unstamped/);
});

test("a verdict other than pass or fail is a usage error before any write", () => {
  const { status, calls } = run(["950", "maybe"]);
  assert.notEqual(status, 0);
  assert.equal(calls.length, 0);
});

test("a seat other than the reviewer cannot apply the pair (#181)", () => {
  const { status, stderr, calls } = run(["950", "fail"], { env: { ROLE: "bdfl" } });
  assert.equal(status, 1);
  assert.equal(calls.length, 0);
  assert.match(stderr, /ROLE is not reviewer/);
});

test("an empty body is refused before the label moves", () => {
  const { status, stderr, calls } = run(["950", "fail"], { body: "  \n" });
  assert.equal(status, 1);
  assert.equal(calls.length, 0);
  assert.match(stderr, /empty body/);
});

test("a failed label edit stops before the comment", () => {
  const { status, stderr, calls } = run(["950", "fail"], { failEdit: true });
  assert.equal(status, 1);
  assert.equal(calls.length, 1);
  assert.match(stderr, /gh pr edit on #950 failed: label not found/);
});

test("a PR outside PR_NUMBER moves no label and posts nothing (#771)", () => {
  const { status, stderr, calls } = run(["951", "fail"], { env: { PR_NUMBER: "950" } });
  assert.equal(status, 1);
  assert.equal(calls.length, 0);
  assert.match(stderr, /does not match PR_NUMBER=950/);
});
