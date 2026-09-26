// merge-pr's PR_NUMBER scope guard only (issue #771, closing PR #772). The
// merge and escalation logic (SHA pinning, 403/405/409 handling, the admin-PAT
// retry) has no coverage here or anywhere else yet; the stub `gh` below always
// fails loudly, so a passing test proves only that the guard did or did not
// let the call through, never that a merge succeeded.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = join(new URL(".", import.meta.url).pathname, "merge-pr");

function run(args, { env = {} } = {}) {
  const stub = mkdtempSync(join(tmpdir(), "merge-pr-"));
  const log = join(stub, "calls.jsonl");
  const recorder = join(stub, "record.mjs");
  writeFileSync(
    recorder,
    `import { appendFileSync } from "node:fs";
appendFileSync(${JSON.stringify(log)}, JSON.stringify(process.argv.slice(2)) + "\\n");
console.error("HTTP 500 stub: gh not configured for this call");
process.exit(1);
`,
  );
  writeFileSync(join(stub, "gh"), `#!/bin/sh\nexec node ${recorder} "$@"\n`);
  chmodSync(join(stub, "gh"), 0o755);
  const proc = spawnSync("python3", [script, ...args], {
    encoding: "utf8",
    env: {
      PATH: `${stub}:${process.env.PATH}`,
      GITHUB_REPOSITORY: "o/r",
      ...env,
    },
  });
  const calls = existsSync(log)
    ? readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map((l) => JSON.parse(l))
    : [];
  return { ...proc, calls };
}

// Issue #771: a review scoped to #768 commented on and merged #770 instead.
// A review session's environment carries PR_NUMBER; nothing outside that
// scope may reach gh, no matter how the wrong number was resolved.
test("PR_NUMBER in the environment refuses a merge of any other number", () => {
  const { status, calls, stderr } = run(["770"], { env: { PR_NUMBER: "768" } });
  assert.equal(status, 1);
  assert.deepEqual(calls, []);
  assert.match(stderr, /does not match PR_NUMBER=768/);
});

test("PR_NUMBER matching the target clears the guard", () => {
  const { calls } = run(["768"], { env: { PR_NUMBER: "768" } });
  assert.ok(calls.length > 0, "merge-pr should have reached gh once the guard passed");
});

test("no PR_NUMBER in the environment (every non-review seat) is unaffected", () => {
  const { calls } = run(["770"], {});
  assert.ok(calls.length > 0, "merge-pr should have reached gh with no scope set");
});
