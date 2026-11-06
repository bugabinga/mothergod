// land-standing-pass: the step that executes a PASS the review run which
// posted it never merged (#1871). The script is copied beside a stub
// `merge-pr` that records its argv and exits as told, and a stub `gh`
// on PATH answers `pr view` from the fixture; the claims pinned are which
// states reach merge-pr and how its exit maps to the step's.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const source = join(new URL(".", import.meta.url).pathname, "land-standing-pass");

function run(
  { labels = ["agent-approved"], files = ["src/lib.rs"], token = "app-token", mergeStatus = 0, ghStatus = 0 } = {},
) {
  const stub = mkdtempSync(join(tmpdir(), "land-standing-"));
  const calls = join(stub, "merge-pr.calls");
  const script = join(stub, "land-standing-pass");
  copyFileSync(source, script);
  chmodSync(script, 0o755);
  writeFileSync(
    join(stub, "merge-pr"),
    `#!/bin/sh\nprintf '%s\\n' "$*" >> ${JSON.stringify(calls)}\nexit ${mergeStatus}\n`,
  );
  chmodSync(join(stub, "merge-pr"), 0o755);
  const view = JSON.stringify({ labels: labels.map((name) => ({ name })), files: files.map((path) => ({ path })) });
  writeFileSync(
    join(stub, "gh"),
    `#!/bin/sh\nif [ ${ghStatus} -ne 0 ]; then echo 'HTTP 500 stub' >&2; exit ${ghStatus}; fi\nprintf '%s' ${
      JSON.stringify(view)
    }\n`,
  );
  chmodSync(join(stub, "gh"), 0o755);
  const env = { PATH: `${stub}:${process.env.PATH}`, GITHUB_REPOSITORY: "o/r" };
  if (token !== null) {
    const file = join(stub, "token");
    writeFileSync(file, token);
    env.GH_TOKEN_FILE = file;
  }
  const proc = spawnSync("python3", [script, "7", "abc123"], { encoding: "utf8", env });
  const merged = existsSync(calls) ? readFileSync(calls, "utf8").trim().split("\n") : [];
  return { status: proc.status, stdout: proc.stdout, stderr: proc.stderr, merged };
}

test("a standing PASS on a plain PR runs merge-pr on the head; merged and armed both exit 0", () => {
  for (const mergeStatus of [0, 2]) {
    const res = run({ mergeStatus });
    assert.equal(res.status, 0, res.stderr);
    assert.deepEqual(res.merged, ["7 --sha abc123"]);
  }
});

test("merge-pr's other exits are the step's, red for the sweep", () => {
  const res = run({ mergeStatus: 3 });
  assert.equal(res.status, 3);
  assert.match(res.stderr, /merge-pr exited 3/);
});

test("a standing FAIL lands nothing", () => {
  const res = run({ labels: ["changes-requested"] });
  assert.equal(res.status, 0);
  assert.deepEqual(res.merged, []);
  assert.match(res.stdout, /not a PASS/);
});

test("a PR under .github/workflows/ is the BDFL's, not merged here", () => {
  const res = run({ files: ["src/lib.rs", ".github/workflows/ci.yml"] });
  assert.equal(res.status, 0);
  assert.deepEqual(res.merged, []);
  assert.match(res.stdout, /BDFL/);
});

test("no minted token: the merge stays the sweep's, never on github.token", () => {
  for (const token of [null, ""]) {
    const res = run({ token });
    assert.equal(res.status, 0);
    assert.deepEqual(res.merged, []);
    assert.match(res.stdout, /sweep/);
  }
});

test("an unreadable PR is loud and merges nothing", () => {
  const res = run({ ghStatus: 1 });
  assert.equal(res.status, 1);
  assert.deepEqual(res.merged, []);
  assert.match(res.stderr, /did not read/);
});
