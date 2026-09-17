// Fixtures for anchor.previous_run_start, the instant operator-sweep and
// retrospect both measure from. The first fixture is 2026-09-17T04:12Z as it
// happened: thirteen guard-skipped BDFL runs, each concluded `success` in
// seconds with no session, stacked in front of the last real run on the
// 15th. Anchoring on the newest success covered two hours; the operator's
// window was two days (#525, and nine days after #517). The artifact every
// session uploads (ADR-0023) is what tells the two apart, so the stub `gh`
// here answers `run list` with the stack and `api .../artifacts` per run.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import sys
sys.path.insert(0, sys.argv[1])
import anchor
stamp, run_id = anchor.previous_run_start("o/r", sys.argv[2] or None)
print(stamp, run_id)
`;

// A stub gh: \`run list\` prints the fixture's runs newest first; \`api\`
// on a run's artifacts prints that run's artifact count. Anything else is
// a call the anchor has no business making, and fails loudly.
function stubGh(runs, artifacts) {
  const dir = mkdtempSync(join(tmpdir(), "anchor-"));
  writeFileSync(
    join(dir, "gh"),
    `#!/usr/bin/env python3
import json, re, sys
runs = ${JSON.stringify(runs)}
artifacts = ${JSON.stringify(artifacts)}
args = sys.argv[1:]
if args[:2] == ["run", "list"]:
    print(json.dumps(runs))
elif args[0] == "api":
    m = re.fullmatch(r"repos/o/r/actions/runs/(\\d+)/artifacts", args[1])
    print(artifacts[m.group(1)])
else:
    sys.exit("stub gh: unexpected call " + " ".join(args))
`,
  );
  chmodSync(join(dir, "gh"), 0o755);
  return dir;
}

function anchor(runs, artifacts, thisRun = "") {
  const proc = spawnSync("python3", ["-c", driver, scriptsDir, thisRun], {
    encoding: "utf8",
    env: { ...process.env, PATH: `${stubGh(runs, artifacts)}:${process.env.PATH}` },
  });
  return proc;
}

const run = (id, startedAt) => ({ databaseId: id, startedAt, createdAt: startedAt });

// 2026-09-17T04:12Z, abridged to the newest three hollow runs and the real one.
const STACK = [
  run(35165527435, "2026-09-17T00:11:33Z"),
  run(35145089313, "2026-09-16T20:11:52Z"),
  run(35120333073, "2026-09-16T16:11:52Z"),
  run(34927890526, "2026-09-15T04:11:52Z"),
];
const HOLLOW_TOP = { 35165527435: 0, 35145089313: 0, 35120333073: 0, 34927890526: 1 };

test("skips guard-skipped successes and anchors on the last run that held a session", () => {
  const proc = anchor(STACK, HOLLOW_TOP);
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-15T04:11:52Z 34927890526");
});

test("the ordinary wake anchors on the newest success when it has an artifact", () => {
  const proc = anchor(STACK, { ...HOLLOW_TOP, 35165527435: 1 });
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-17T00:11:33Z 35165527435");
});

test("this run is excluded even when it already has an artifact", () => {
  const proc = anchor(STACK, { ...HOLLOW_TOP, 35165527435: 1 }, "35165527435");
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal(proc.stdout.trim(), "2026-09-15T04:11:52Z 34927890526");
});

test("a stack with no session in it dies naming --since, never anchors on a hollow run", () => {
  const proc = anchor(STACK, { ...HOLLOW_TOP, 34927890526: 0 });
  assert.notEqual(proc.status, 0);
  assert.match(proc.stderr, /--since/);
  assert.match(proc.stderr, /audit artifact/);
});
