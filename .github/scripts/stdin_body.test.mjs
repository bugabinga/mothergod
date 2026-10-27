// Issue #1682: a posting script called without a piped body blocked on
// stdin until the session died, because the Bash tool's stdin is a pipe
// that never closes. `stdin_body.read` gives a writer WAIT_S to produce a
// byte and dies naming the fix. These pin the three stdin shapes (a silent
// open pipe, a closed empty pipe, a writer that starts late) and that every
// script in `heredoc.POSTING` reads through it, so the list stays the one
// place that names a posting script.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const scripts = new URL(".", import.meta.url).pathname;
const env = { PATH: process.env.PATH };

// A child whose stdin stays open and silent unless `write` does something
// with it. spawn, never spawnSync: spawnSync closes stdin at once, which is
// the one shape the bug never saw.
function read(what, waitS, { write } = {}) {
  return new Promise((resolve) => {
    const code = `import stdin_body; print(repr(stdin_body.read(${JSON.stringify(what)}, ${waitS})))`;
    const child = spawn("python3", ["-c", code], { cwd: scripts, env });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk) => (stdout += chunk));
    child.stderr.setEncoding("utf8").on("data", (chunk) => (stderr += chunk));
    child.on("exit", () => child.stdin.destroy());
    child.on("close", (status) => resolve({ status, stdout, stderr }));
    if (write) write(child.stdin);
  });
}

test("a silent open pipe dies after the wait, naming what was missing and the fix", async () => {
  const started = Date.now();
  const r = await read("commit message", 0.3);
  assert.equal(r.status, 1);
  assert.equal(r.stdout, "");
  assert.match(r.stderr, /the commit message comes from stdin; nothing arrived in 0\.3 s: pipe it/);
  assert.match(r.stderr, /<<'EOF'/);
  assert.ok(Date.now() - started < 5000, "the wait ended it, not a harness timeout");
});

test("a closed empty pipe returns an empty body, so the caller's own message fires", async () => {
  const r = await read("body", 0.3, { write: (stdin) => stdin.end() });
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout.trim(), "''");
});

test("a writer that starts inside the wait is read whole and stripped", async () => {
  const r = await read("body", 2, { write: (stdin) => setTimeout(() => stdin.end("  late\nbody  \n"), 100) });
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout.trim(), "'late\\nbody'");
});

test("every posting script reads its body through stdin_body, never sys.stdin", () => {
  const list = spawnSync("python3", ["-c", "import heredoc; print('\\n'.join(sorted(heredoc.POSTING)))"], {
    cwd: scripts,
    env,
    encoding: "utf8",
  });
  assert.equal(list.status, 0, list.stderr);
  const posting = list.stdout.trim().split("\n");
  assert.ok(posting.length >= 5, `POSTING names ${posting.length} scripts`);
  for (const name of posting) {
    const source = readFileSync(join(scripts, name), "utf8");
    assert.match(source, /stdin_body\.read\(/, `${name} reads its body through stdin_body`);
    assert.doesNotMatch(source, /sys\.stdin\.read\(/, `${name} reads stdin only through stdin_body`);
  }
});
