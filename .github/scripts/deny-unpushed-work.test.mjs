// The deny/allow boundary of the unpushed-work PreToolUse hook. Every case
// is a payload, an environment and a working tree: a temp git repo the
// test dirties or leaves clean, a stamp the test backdates to age the
// session, and the refs file push-branch would have written.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-unpushed-work", import.meta.url).pathname;
const MIN = 60;

function repo() {
  const dir = mkdtempSync(join(tmpdir(), "unpushed-repo-"));
  const git = (...args) => execFileSync("git", args, { cwd: dir, stdio: "pipe" });
  git("init", "-q");
  git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base");
  return {
    dir,
    dirty: () => writeFileSync(join(dir, "src.rs"), "edit\n"),
    pushed: () => writeFileSync(join(dir, ".git", "push-branch-refs.json"), "{}"),
  };
}

// A session whose first stamped call was `ageMinutes` ago; undefined means
// no stamp yet, the shape of a session's first call.
function session(ageMinutes) {
  const tmp = mkdtempSync(join(tmpdir(), "unpushed-tmp-"));
  const stamp = join(tmp, "deny-unpushed-work-s1.json");
  if (ageMinutes !== undefined) {
    const start = Date.now() / 1000 - ageMinutes * MIN;
    writeFileSync(stamp, JSON.stringify({ start, fired: false }));
  }
  const summary = join(tmp, "summary.md");
  const env = { PATH: process.env.PATH, GITHUB_ACTIONS: "true", TMPDIR: tmp, GITHUB_STEP_SUMMARY: summary };
  return { stamp, summary, env };
}

function bash(cwd, command = "cargo test") {
  return { tool_name: "Bash", session_id: "s1", cwd, tool_input: { command } };
}

function run(env, call) {
  return spawnSync(script, [], {
    input: typeof call === "string" ? call : JSON.stringify(call),
    env,
    encoding: "utf8",
  });
}

test("outside GitHub Actions everything is allowed and no stamp is written", () => {
  const { dir, dirty } = repo();
  dirty();
  const { stamp, env } = session(40);
  const res = run({ ...env, GITHUB_ACTIONS: "" }, bash(dir));
  assert.equal(res.status, 0);
  assert.equal(JSON.parse(readFileSync(stamp, "utf8")).fired, false);
});

test("the first call stamps the session start and allows", () => {
  const { dir, dirty } = repo();
  dirty();
  const { stamp, env } = session();
  assert.equal(existsSync(stamp), false);
  const res = run(env, bash(dir));
  assert.equal(res.status, 0);
  const written = JSON.parse(readFileSync(stamp, "utf8"));
  assert.ok(Math.abs(written.start - Date.now() / 1000) < 5);
  assert.equal(written.fired, false);
});

test("a young session with edits is allowed", () => {
  const { dir, dirty } = repo();
  dirty();
  const { env } = session(24);
  assert.equal(run(env, bash(dir)).status, 0);
});

test("an aged session with a clean tree is allowed", () => {
  const { dir } = repo();
  const { env, summary } = session(40);
  assert.equal(run(env, bash(dir)).status, 0);
  assert.equal(existsSync(summary), false);
});

test("an aged session with edits and no push is denied once, told the path and the command", () => {
  const { dir, dirty } = repo();
  dirty();
  const { stamp, summary, env } = session(40);
  const first = run(env, bash(dir));
  assert.equal(first.status, 2);
  assert.match(first.stderr, /40 min old, 1 edited path unpushed: src\.rs\./);
  assert.match(first.stderr, /\.github\/scripts\/push-branch claude\/<slug> <paths>/);
  assert.match(first.stderr, /#81/);
  assert.doesNotMatch(first.stderr, /—/);
  assert.equal(readFileSync(summary, "utf8"), "deny-unpushed-work: denied at 40 min, 1 paths unpushed\n");
  assert.equal(JSON.parse(readFileSync(stamp, "utf8")).fired, true);
  // The writer may judge the edits not ready; the second call goes through.
  const second = run(env, bash(dir));
  assert.equal(second.status, 0);
  assert.equal(second.stderr, "");
  assert.equal(readFileSync(summary, "utf8").split("\n").length, 2);
});

test("a session that already pushed is never nagged", () => {
  const { dir, dirty, pushed } = repo();
  dirty();
  pushed();
  const { env } = session(40);
  assert.equal(run(env, bash(dir)).status, 0);
});

test("the push itself is exempt", () => {
  const { dir, dirty } = repo();
  dirty();
  const { env, stamp } = session(40);
  const res = run(env, bash(dir, "printf 'msg' | .github/scripts/push-branch claude/x src.rs"));
  assert.equal(res.status, 0);
  assert.equal(JSON.parse(readFileSync(stamp, "utf8")).fired, false);
});

test("the threshold follows BASH_MAX_TIMEOUT_MS: 60 min of token, minus the longest call, minus 5", () => {
  const { dir, dirty } = repo();
  dirty();
  // Default 30 min ceiling: threshold 25, so 30 min old is past it.
  assert.equal(run(session(30).env, bash(dir)).status, 2);
  // A 10 min ceiling moves the threshold to 45, and 30 min old is young.
  const ten = session(30);
  assert.equal(run({ ...ten.env, BASH_MAX_TIMEOUT_MS: "600000" }, bash(dir)).status, 0);
  assert.equal(run({ ...session(46).env, BASH_MAX_TIMEOUT_MS: "600000" }, bash(dir)).status, 2);
});

test("many paths are listed six deep and counted in full", () => {
  const { dir } = repo();
  for (let i = 0; i < 8; i += 1) writeFileSync(join(dir, `f${i}.rs`), "x\n");
  const res = run(session(40).env, bash(dir));
  assert.equal(res.status, 2);
  assert.match(res.stderr, /8 edited paths unpushed: f0\.rs, f1\.rs, f2\.rs, f3\.rs, f4\.rs, f5\.rs, \.\.\.\./);
});

test("non-Bash tools, malformed JSON and a null tool_input are allowed", () => {
  const { dir, dirty } = repo();
  dirty();
  const { env } = session(40);
  assert.equal(run(env, { ...bash(dir), tool_name: "Write" }).status, 0);
  assert.equal(run(env, "{not json").status, 0);
  assert.equal(run(env, { ...bash(dir), tool_input: null }).status, 0);
});
