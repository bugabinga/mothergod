// Covers the behavior this guard gained after shipping: the
// GITHUB_STEP_SUMMARY liveness line (a deny used to leave no durable
// trace, because the audit artifact excludes transcripts by design).
// The deny/allow boundary itself was exercised live before this file
// existed (#323, #324); the boundary cases here pin it against
// regression now that a test file finally exists to hold them.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-ci-background", import.meta.url).pathname;

const ciEnv = { PATH: process.env.PATH, GITHUB_ACTIONS: "true" };

function run(call, env = ciEnv) {
  return spawnSync(script, [], {
    input: JSON.stringify(call),
    env,
    encoding: "utf8",
  });
}

test("denies backgrounded Bash, allows foreground", () => {
  const bg = {
    tool_name: "Bash",
    tool_input: { command: "cargo test", run_in_background: true },
  };
  const fg = { tool_name: "Bash", tool_input: { command: "cargo test" } };
  assert.equal(run(bg).status, 2);
  assert.equal(run(fg).status, 0);
});

test("denies default-background tools unless explicitly foreground", () => {
  const agent = { tool_name: "Agent", tool_input: { prompt: "x" } };
  const fgAgent = {
    tool_name: "Agent",
    tool_input: { prompt: "x", run_in_background: false },
  };
  assert.equal(run(agent).status, 2);
  assert.equal(run(fgAgent).status, 0);
});

test("null tool_input on a default-background tool still denies", () => {
  assert.equal(run({ tool_name: "Workflow", tool_input: null }).status, 2);
});

test("denies a shell-& detach in the command string", () => {
  // Run 33269388206 bypassed the structured field entirely; the
  // command string is part of the surface (PR #354).
  const detached = [
    "cargo x check > /tmp/x.log 2>&1 &",
    "nohup cargo test & echo started",
    "(cargo test &)",
  ];
  for (const command of detached) {
    const call = { tool_name: "Bash", tool_input: { command } };
    assert.equal(run(call).status, 2, command);
  }
});

test("denies a setsid fork, allows setsid used as a waiter", () => {
  for (
    const command of [
      "setsid -f cargo test > /tmp/o 2>&1 < /dev/null",
      "setsid --fork cargo test",
      "setsid -cf cargo test",
      "cd /tmp && setsid -f cargo test",
    ]
  ) {
    const call = { tool_name: "Bash", tool_input: { command } };
    assert.equal(run(call).status, 2, command);
  }
  const waiting = { tool_name: "Bash", tool_input: { command: "setsid -w cargo test" } };
  assert.equal(run(waiting).status, 0);
});

test("denies a coprocess, allows word idioms as mid-sentence prose", () => {
  for (
    const command of ["coproc cargo test", "cd /tmp && coproc watch { cargo test; }"]
  ) {
    const call = { tool_name: "Bash", tool_input: { command } };
    assert.equal(run(call).status, 2, command);
  }
  // Command-position anchoring: mentioning an idiom is not using it.
  for (
    const command of [
      "echo coprocessor ready",
      "git commit -m 'guard: mention the setsid -f trap in prose'",
    ]
  ) {
    const call = { tool_name: "Bash", tool_input: { command } };
    assert.equal(run(call).status, 0, command);
  }
});

test("allows &&, fd duplication, |&, and embedded ampersands", () => {
  const foreground = [
    "cargo build && cargo test",
    "cargo test > log 2>&1",
    "curl \"https://x.test/?a=1&b=2\"",
    "cargo test |& tee log",
  ];
  for (const command of foreground) {
    const call = { tool_name: "Bash", tool_input: { command } };
    assert.equal(run(call).status, 0, command);
  }
});

test("quoted prose ampersand denies: the accepted cost, pinned", () => {
  const call = {
    tool_name: "Bash",
    tool_input: { command: "echo \"fish & chips\"" },
  };
  assert.equal(run(call).status, 2);
});

test("a shell-& deny writes its own summary line", () => {
  const summary = join(mkdtempSync(join(tmpdir(), "deny-")), "summary.md");
  const env = { ...ciEnv, GITHUB_STEP_SUMMARY: summary };
  const call = {
    tool_name: "Bash",
    tool_input: { command: "cargo test &" },
  };
  assert.equal(run(call, env).status, 2);
  assert.match(
    readFileSync(summary, "utf8"),
    /^deny-ci-background: denied Bash via shell &$/m,
  );
});

test("a deny writes the tool name to GITHUB_STEP_SUMMARY", () => {
  const summary = join(mkdtempSync(join(tmpdir(), "deny-")), "summary.md");
  const env = { ...ciEnv, GITHUB_STEP_SUMMARY: summary };
  const bg = {
    tool_name: "Bash",
    tool_input: { command: "cargo test", run_in_background: true },
  };
  assert.equal(run(bg, env).status, 2);
  assert.match(
    readFileSync(summary, "utf8"),
    /^deny-ci-background: denied Bash in background$/m,
  );
});
