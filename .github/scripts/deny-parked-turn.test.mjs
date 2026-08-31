// The deny/allow boundary of the Stop-hook guard. The scan reads the
// real process table, so the block cases plant a real process whose
// comm is `cargo` (a renamed copy of sleep); ci.yml runs these tests
// in a node-only job where no genuine toolchain process can be alive.
// A dev running `node --test` beside a live local cargo build will see
// the no-process case fail: accepted cost of refusing a test seam in
// the script itself.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { chmodSync, copyFileSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-parked-turn", import.meta.url).pathname;

const ciEnv = { PATH: process.env.PATH, GITHUB_ACTIONS: "true" };

function run(call, env = ciEnv) {
  return spawnSync(script, [], {
    input: typeof call === "string" ? call : JSON.stringify(call),
    env,
    encoding: "utf8",
  });
}

function plantCargo() {
  const dir = mkdtempSync(join(tmpdir(), "parked-"));
  const fake = join(dir, "cargo");
  copyFileSync("/usr/bin/sleep", fake);
  chmodSync(fake, 0o755);
  const child = spawn(fake, ["60"], { stdio: "ignore" });
  return child;
}

test("blocks a stop while a cargo process is alive", async () => {
  const child = plantCargo();
  try {
    const res = run({ stop_hook_active: false });
    assert.equal(res.status, 0);
    const out = JSON.parse(res.stdout);
    assert.equal(out.decision, "block");
    assert.match(out.reason, new RegExp(`cargo\\(${child.pid}\\)`));
    assert.match(out.reason, /foreground/);
  } finally {
    child.kill();
    await once(child, "exit");
  }
});

test("a block writes its own summary line", async () => {
  const child = plantCargo();
  const summary = join(mkdtempSync(join(tmpdir(), "parked-")), "summary.md");
  try {
    const res = run({ stop_hook_active: false }, { ...ciEnv, GITHUB_STEP_SUMMARY: summary });
    assert.equal(JSON.parse(res.stdout).decision, "block");
    assert.match(
      readFileSync(summary, "utf8"),
      /^deny-parked-turn: blocked stop; live .*cargo\(\d+\)/m,
    );
  } finally {
    child.kill();
    await once(child, "exit");
  }
});

test("allows the second stop, recording what walked out", async () => {
  // One warning only: re-blocking forever would hold a dead session
  // open, so stop_hook_active allows and the summary carries the loss.
  const child = plantCargo();
  const summary = join(mkdtempSync(join(tmpdir(), "parked-")), "summary.md");
  try {
    const res = run({ stop_hook_active: true }, { ...ciEnv, GITHUB_STEP_SUMMARY: summary });
    assert.equal(res.status, 0);
    assert.equal(res.stdout, "");
    assert.match(
      readFileSync(summary, "utf8"),
      /^deny-parked-turn: allowed second stop with live .*cargo\(\d+\)/m,
    );
  } finally {
    child.kill();
    await once(child, "exit");
  }
});

test("allows outside GitHub Actions even with a cargo process alive", async () => {
  const child = plantCargo();
  try {
    const res = run({ stop_hook_active: false }, { PATH: process.env.PATH });
    assert.equal(res.status, 0);
    assert.equal(res.stdout, "");
  } finally {
    child.kill();
    await once(child, "exit");
  }
});

test("allows when no toolchain process is alive", async () => {
  const res = run({ stop_hook_active: false });
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
});

test("malformed JSON and non-object input allow", () => {
  assert.equal(run("not json").status, 0);
  assert.equal(run("not json").stdout, "");
  assert.equal(run(JSON.stringify("a string")).status, 0);
});
