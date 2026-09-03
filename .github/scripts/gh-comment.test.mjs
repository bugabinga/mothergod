// gh-comment assembles two things a human keeps getting wrong by hand: the
// role footer the self-trigger guard matches on (issue #138, #206) and the
// credential the write must ride (issue #50, #81). Issue #489 added a third
// verb, `--new`, because the app token expires an hour into a session and a
// long run could not file what it learned. All three are argv-shaped, so a
// stub `gh` on PATH is the whole harness: it records what it was called with.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = join(new URL(".", import.meta.url).pathname, "gh-comment");

function run(args, { body = "hello", env = {} } = {}) {
  const stub = mkdtempSync(join(tmpdir(), "gh-comment-"));
  const log = join(stub, "argv.json");
  const recorder = join(stub, "record.mjs");
  writeFileSync(
    recorder,
    `import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(log)}, JSON.stringify({
  argv: process.argv.slice(2),
  token: process.env.GH_TOKEN ?? null,
  admin: process.env.GH_ADMIN_TOKEN ?? null,
}));
console.log("https://github.com/o/r/issues/1");
`,
  );
  writeFileSync(join(stub, "gh"), `#!/bin/sh\nexec node ${recorder} "$@"\n`);
  chmodSync(join(stub, "gh"), 0o755);
  const proc = spawnSync("python3", [script, ...args], {
    encoding: "utf8",
    input: body,
    env: {
      PATH: `${stub}:${process.env.PATH}`,
      ROLE: "bdfl",
      GH_WORKFLOW_TOKEN: "workflow-token",
      GH_ADMIN_TOKEN: "admin-token",
      ...env,
    },
  });
  let called = null;
  try {
    called = JSON.parse(readFileSync(log, "utf8"));
  } catch {
    // gh was never reached; the refusal tests assert exactly that.
  }
  return { ...proc, called };
}

test("--new creates an issue with the labels, the footer, and the workflow token", () => {
  const { status, called } = run(["--new", "Late writes die", "--label", "bug", "--label", "agent-system"]);
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 4), ["issue", "create", "--title", "Late writes die"]);
  assert.match(called.argv[5], /^hello\n\n---\n_bdfl · \[Claude Code\]/);
  assert.deepEqual(called.argv.slice(6), ["--label", "bug", "--label", "agent-system"]);
  assert.equal(called.token, "workflow-token");
  // The admin PAT is stripped, not merely unused: an ambient one is the bug.
  assert.equal(called.admin, null);
});

test("commenting and closing keep their shapes", () => {
  assert.deepEqual(run(["489"]).called.argv.slice(0, 2), ["issue", "comment"]);
  assert.deepEqual(run(["489", "--close"]).called.argv.slice(0, 2), ["issue", "close"]);
});

test("--new refuses a number or a close, which would mean two verbs at once", () => {
  const { status, stderr, called } = run(["--new", "t", "489", "--close"]);
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /takes no number/);
});

test("--label without --new is refused rather than silently dropped", () => {
  const { status, stderr } = run(["489", "--label", "bug"]);
  assert.equal(status, 1);
  assert.match(stderr, /gh issue edit/);
});

test("no verb reaches gh without GH_WORKFLOW_TOKEN", () => {
  const { status, called } = run(["--new", "t"], { env: { GH_WORKFLOW_TOKEN: "" } });
  assert.equal(status, 1);
  assert.equal(called, null);
});
