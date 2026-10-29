import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

// The module the seven deny-* hooks write through and audit-extract.py
// reads (#990). The hooks' own tests pin what each writes; this pins the
// reader's edges, which no hook exercises: no runner, no record yet, and a
// record a hook's crash left half-written.
const scripts = new URL(".", import.meta.url).pathname;

function read(env) {
  const r = spawnSync("python3", ["-c", "import json, walls; print(json.dumps(walls.read()))"], {
    cwd: scripts,
    encoding: "utf8",
    env: { PATH: process.env.PATH, ...env },
  });
  assert.equal(r.status, 0, r.stderr);
  return JSON.parse(r.stdout);
}

test("read is empty outside the runner and before the first wall", () => {
  assert.deepEqual(read({}), []);
  assert.deepEqual(read({ RUNNER_TEMP: mkdtempSync(join(tmpdir(), "walls-")) }), []);
});

test("read keeps every well-formed wall in order and skips the rest", () => {
  const temp = mkdtempSync(join(tmpdir(), "walls-"));
  const wall = (reason) => JSON.stringify({ tool: "Bash", type: "hook", reason });
  writeFileSync(
    join(temp, "walls.jsonl"),
    [
      wall("deny-em-dash: denied"),
      "{truncated",
      JSON.stringify({ tool: "Bash", type: "hook" }),
      JSON.stringify({ tool: "Bash", type: "hook", reason: 7 }),
      JSON.stringify({ tool: "Bash", type: "hook", reason: "x", extra: "y" }),
      wall("deny-ci-background: denied Task in background"),
      wall("deny-em-dash: denied"),
      "",
    ].join("\n"),
  );
  assert.deepEqual(read({ RUNNER_TEMP: temp }), [
    { tool: "Bash", type: "hook", reason: "deny-em-dash: denied" },
    { tool: "Bash", type: "hook", reason: "deny-ci-background: denied Task in background" },
    { tool: "Bash", type: "hook", reason: "deny-em-dash: denied" },
  ]);
});

test("denied writes the summary line and the record, note the line alone", () => {
  const temp = mkdtempSync(join(tmpdir(), "walls-"));
  const summary = join(temp, "summary.md");
  const r = spawnSync(
    "python3",
    ["-c", "import walls; walls.note(\"h\", \"allowed\"); walls.denied(\"h\", \"Stop\", \"blocked stop\")"],
    {
      cwd: scripts,
      encoding: "utf8",
      env: { PATH: process.env.PATH, RUNNER_TEMP: temp, GITHUB_STEP_SUMMARY: summary },
    },
  );
  assert.equal(r.status, 0, r.stderr);
  assert.equal(readFileSync(summary, "utf8"), "h: allowed\nh: blocked stop\n");
  assert.deepEqual(JSON.parse(readFileSync(join(temp, "walls.jsonl"), "utf8")), {
    tool: "Stop",
    type: "hook",
    reason: "h: blocked stop",
  });
});

// #1793: a hook's class for the hit rides the line as a bracketed suffix,
// the one shape audit_facts.wall_of lifts into walls.detail; both readers
// see the same line.
test("denied with a detail suffixes the line and the record alike", () => {
  const temp = mkdtempSync(join(tmpdir(), "walls-"));
  const summary = join(temp, "summary.md");
  const r = spawnSync(
    "python3",
    ["-c", "import walls; walls.denied(\"h\", \"Bash\", \"denied Bash via x\", \"quoted\")"],
    {
      cwd: scripts,
      encoding: "utf8",
      env: { PATH: process.env.PATH, RUNNER_TEMP: temp, GITHUB_STEP_SUMMARY: summary },
    },
  );
  assert.equal(r.status, 0, r.stderr);
  assert.equal(readFileSync(summary, "utf8"), "h: denied Bash via x [quoted]\n");
  assert.deepEqual(JSON.parse(readFileSync(join(temp, "walls.jsonl"), "utf8")), {
    tool: "Bash",
    type: "hook",
    reason: "h: denied Bash via x [quoted]",
  });
});
