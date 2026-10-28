// The deny/allow boundary of the Stop-hook guard. The scan reads the
// process table from DENY_PARKED_TURN_PROC, so each case stages its own:
// a directory of `<pid>/status` files in /proc's shape. The first version
// refused that seam and planted a real process named `cargo` instead;
// ci.yml's worker job runs every test file in one parallel `node --test`,
// and a sibling's cargo spawn turned the no-process case red on a PR that
// touched neither the hook nor its test (#1736).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-parked-turn", import.meta.url).pathname;

// A process table: `<root>/<pid>/status` per entry, `Name:` and `State:`
// in the kernel's tab-separated shape, with a neighbour line on each side
// so the parse is proven to pick its keys rather than line positions.
function table(entries) {
  const root = mkdtempSync(join(tmpdir(), "parked-proc-"));
  for (const [pid, name, state] of entries) {
    mkdirSync(join(root, String(pid)));
    writeFileSync(
      join(root, String(pid), "status"),
      `Name:\t${name}\nUmask:\t0022\nState:\t${state}\nTgid:\t${pid}\nPid:\t${pid}\n`,
    );
  }
  return root;
}

const QUIET = [
  [1, "systemd", "S (sleeping)"],
  [4242, "node", "R (running)"],
];
const BUSY = [...QUIET, [777, "cargo", "S (sleeping)"], [778, "rustc", "R (running)"], [
  779,
  "cargo-clippy",
  "D (disk sleep)",
]];

function ciEnv(root, extra = {}) {
  return { PATH: process.env.PATH, GITHUB_ACTIONS: "true", DENY_PARKED_TURN_PROC: root, ...extra };
}

function run(call, env) {
  return spawnSync(script, [], {
    input: typeof call === "string" ? call : JSON.stringify(call),
    env,
    encoding: "utf8",
  });
}

test("blocks a stop while a cargo process is alive, naming every toolchain pid", () => {
  const res = run({ stop_hook_active: false }, ciEnv(table(BUSY)));
  assert.equal(res.status, 0);
  const out = JSON.parse(res.stdout);
  assert.equal(out.decision, "block");
  assert.match(out.reason, /cargo\(777\)/);
  assert.match(out.reason, /rustc\(778\)/);
  assert.match(out.reason, /cargo-clippy\(779\)/, "a cargo subcommand binary counts by prefix");
  assert.doesNotMatch(out.reason, /node\(4242\)/);
  assert.match(out.reason, /foreground/);
});

test("a block writes its own summary line and the audit's record", () => {
  const temp = mkdtempSync(join(tmpdir(), "parked-"));
  const summary = join(temp, "summary.md");
  const res = run({ stop_hook_active: false }, ciEnv(table(BUSY), { GITHUB_STEP_SUMMARY: summary, RUNNER_TEMP: temp }));
  assert.equal(JSON.parse(res.stdout).decision, "block");
  assert.match(readFileSync(summary, "utf8"), /^deny-parked-turn: blocked stop; live .*cargo\(777\)/m);
  const wall = JSON.parse(readFileSync(join(temp, "walls.jsonl"), "utf8"));
  assert.equal(wall.tool, "Stop");
  assert.equal(wall.type, "hook");
  assert.match(wall.reason, /^deny-parked-turn: blocked stop; live .*cargo\(777\)/);
});

test("allows the second stop, recording what walked out", () => {
  // One warning only: re-blocking forever would hold a dead session
  // open, so stop_hook_active allows and the summary carries the loss.
  const temp = mkdtempSync(join(tmpdir(), "parked-"));
  const summary = join(temp, "summary.md");
  const res = run({ stop_hook_active: true }, ciEnv(table(BUSY), { GITHUB_STEP_SUMMARY: summary, RUNNER_TEMP: temp }));
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
  assert.match(readFileSync(summary, "utf8"), /^deny-parked-turn: allowed second stop with live .*cargo\(777\)/m);
  assert.equal(existsSync(join(temp, "walls.jsonl")), false, "an allow is a summary line, not a wall");
});

test("allows outside GitHub Actions even with a cargo process alive", () => {
  const res = run({ stop_hook_active: false }, { PATH: process.env.PATH, DENY_PARKED_TURN_PROC: table(BUSY) });
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
});

test("allows when no toolchain process is alive", () => {
  const res = run({ stop_hook_active: false }, ciEnv(table(QUIET)));
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
});

test("a zombie cargo is dead work, not live work", () => {
  const res = run({ stop_hook_active: false }, ciEnv(table([...QUIET, [900, "cargo", "Z (zombie)"]])));
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
});

test("a table it cannot read is no evidence: allows", () => {
  // No root at all, and a root whose one entry lost its status file
  // between the listing and the read.
  const missing = run({ stop_hook_active: false }, ciEnv(join(tmpdir(), "parked-no-such-root")));
  assert.equal(missing.status, 0);
  assert.equal(missing.stdout, "");
  const root = table(QUIET);
  mkdirSync(join(root, "555"));
  const vanished = run({ stop_hook_active: false }, ciEnv(root));
  assert.equal(vanished.status, 0);
  assert.equal(vanished.stdout, "");
});

test("the default table is the host's /proc", () => {
  // Unset seam, real table: the hook must read it without error. Whether
  // it allows or blocks depends on what the host runs at this moment, so
  // the assertion is the exit and the shape of what it printed.
  const res = run({ stop_hook_active: false }, { PATH: process.env.PATH, GITHUB_ACTIONS: "true" });
  assert.equal(res.status, 0);
  assert.equal(res.stderr, "");
  if (res.stdout !== "") assert.equal(JSON.parse(res.stdout).decision, "block");
});

test("malformed JSON and non-object input allow", () => {
  const env = ciEnv(table(BUSY));
  assert.equal(run("not json", env).status, 0);
  assert.equal(run("not json", env).stdout, "");
  assert.equal(run(JSON.stringify("a string"), env).status, 0);
  assert.equal(run(JSON.stringify("a string"), env).stdout, "");
});
