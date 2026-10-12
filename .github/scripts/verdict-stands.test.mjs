// Fixtures for verdict-stands, the guard that skips a review run whose diff,
// title and body the reviewer already returned (#953, #1050).
//
// decide() is pure and pinned per route. digest() runs against a real git
// repository in a temp dir, because the claim that matters is which pushes
// change the id: a merge of main that touches no PR file keeps it, an
// indentation-only fix changes it, a body edit changes it.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = join(scriptsDir, "verdict-stands");

const driver = `
import importlib.machinery, importlib.util, json, sys
loader = importlib.machinery.SourceFileLoader("verdict_stands", sys.argv[1] + "/verdict-stands")
spec = importlib.util.spec_from_loader("verdict_stands", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
call = json.loads(sys.argv[2])
if call["fn"] == "decide":
    print(json.dumps(mod.decide(call["labels"], call["comments"], call["id"])))
else:
    print(json.dumps(mod.digest(call["diff"], call["title"], call["body"])))
`;

function call(payload) {
  const res = spawnSync("python3", ["-I", "-c", driver, scriptsDir, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(res.status, 0, res.stderr);
  return JSON.parse(res.stdout);
}

const decide = (labels, comments, id) => call({ fn: "decide", labels, comments, id });
const ID = "a".repeat(64);
const OTHER = "b".repeat(64);
const reviewer = (body, createdAt, stamp) => ({
  createdAt,
  body: `${body}\n\n${
    stamp ? `<!-- verdict ${stamp[0]} ${stamp[1]} -->\n\n` : ""
  }---\n_reviewer · [Claude Code](https://claude.ai/code)_`,
});
const maintainer = (body, createdAt) => ({
  createdAt,
  body: `${body}\n\n---\n_maintainer · [Claude Code](https://claude.ai/code)_`,
});

test("a standing fail on the same id with changes-requested stands", () => {
  const [stands, why] = decide(
    ["changes-requested"],
    [reviewer("one [must]", "2026-10-12T00:45:37Z", ["fail", ID])],
    ID,
  );
  assert.equal(stands, true);
  assert.match(why, /unchanged/);
});

test("a different id reviews: the author changed what the reviewer reads", () => {
  const [stands] = decide(["changes-requested"], [reviewer("one [must]", "2026-10-12T00:45:37Z", ["fail", OTHER])], ID);
  assert.equal(stands, false);
});

test("no changes-requested label reviews, whatever the stamp says", () => {
  const [stands, why] = decide(["agent-approved"], [reviewer("holds", "2026-10-12T00:45:37Z", ["pass", ID])], ID);
  assert.equal(stands, false);
  assert.match(why, /no changes-requested/);
});

test("a pass stamp under changes-requested reviews: the two disagree", () => {
  const [stands, why] = decide(["changes-requested"], [reviewer("holds", "2026-10-12T00:45:37Z", ["pass", ID])], ID);
  assert.equal(stands, false);
  assert.match(why, /pass stamp/);
});

test("an unstamped newest reviewer comment reviews, even over a stamped older one", () => {
  const comments = [
    reviewer("one [must]", "2026-10-12T00:45:37Z", ["fail", ID]),
    reviewer("merge-pr failed, the BDFL sweep rescues", "2026-10-12T00:50:00Z", null),
  ];
  const [stands, why] = decide(["changes-requested"], comments, ID);
  assert.equal(stands, false);
  assert.match(why, /no stamp/);
});

test("the newest reviewer comment is found by footer and time, not by position", () => {
  const comments = [
    maintainer("pushed the fix", "2026-10-12T01:00:00Z"),
    reviewer("one [must]", "2026-10-12T00:45:37Z", ["fail", ID]),
    reviewer("round 1", "2026-10-11T00:00:00Z", ["fail", OTHER]),
  ];
  const [stands] = decide(["changes-requested"], comments, ID);
  assert.equal(stands, true);
});

test("a footer quoted mid-body is not the reviewer's", () => {
  const quoted = {
    createdAt: "2026-10-12T02:00:00Z",
    body:
      `the reviewer wrote:\n> _reviewer · [Claude Code](https://claude.ai/code)_\n<!-- verdict fail ${ID} -->\nso I fixed it\n\n---\n_maintainer · [Claude Code](https://claude.ai/code)_`,
  };
  const [stands, why] = decide(["changes-requested"], [quoted], ID);
  assert.equal(stands, false);
  assert.match(why, /no reviewer comment/);
});

test("an empty id never stands: an unreadable diff reviews", () => {
  const [stands, why] = decide(["changes-requested"], [reviewer("x", "2026-10-12T00:45:37Z", ["fail", ""])], "");
  assert.equal(stands, false);
  assert.match(why, /no diff id/);
});

test("digest: the body and title are part of what a verdict covers", () => {
  const base = call({ fn: "digest", diff: "d", title: "t", body: "b" });
  assert.equal(base.length, 64);
  assert.notEqual(call({ fn: "digest", diff: "d", title: "t", body: "b2" }), base);
  assert.notEqual(call({ fn: "digest", diff: "d", title: "t2", body: "b" }), base);
  assert.equal(call({ fn: "digest", diff: "d", title: "t", body: "b" }), base);
});

// The script end to end, with a stub gh and a real repository: which pushes
// keep the id and which change it.
function repo() {
  const dir = mkdtempSync(join(tmpdir(), "verdict-stands-"));
  const git = (...args) =>
    execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", ...args], {
      cwd: dir,
      encoding: "utf8",
    }).trim();
  git("init", "-q", "-b", "main");
  writeFileSync(join(dir, "a.py"), "def f():\n    return 1\n");
  writeFileSync(join(dir, "other.md"), "one\n");
  git("add", ".");
  git("commit", "-q", "-m", "base");
  return { dir, git };
}

function stands(dir, number, base, head, pr) {
  const stub = mkdtempSync(join(tmpdir(), "verdict-stands-gh-"));
  writeFileSync(join(stub, "gh"), `#!/bin/sh\ncat <<'JSON'\n${JSON.stringify(pr)}\nJSON\n`);
  chmodSync(join(stub, "gh"), 0o755);
  const res = spawnSync("python3", [script, number, base, head], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, PATH: `${stub}:${process.env.PATH}` },
  });
  assert.equal(res.status, 0, res.stderr);
  const out = Object.fromEntries(res.stdout.trim().split("\n").map((line) => line.split("=", 2)));
  return { ...out, stderr: res.stderr };
}

const failedOn = (id) => ({
  title: "a: b",
  body: "why",
  labels: [{ name: "changes-requested" }],
  comments: [reviewer("one [must]", "2026-10-12T00:45:37Z", ["fail", id])],
});

test("end to end: a merge of main that touches no PR file keeps the id and stands", () => {
  const { dir, git } = repo();
  const base1 = git("rev-parse", "HEAD");
  git("checkout", "-q", "-b", "pr");
  writeFileSync(join(dir, "a.py"), "def f():\n    return 2\n");
  git("commit", "-qam", "pr change");
  const head1 = git("rev-parse", "HEAD");
  const first = stands(dir, "1", base1, head1, failedOn(""));
  assert.equal(first.stands, "false");
  assert.match(first.diff_id, /^[0-9a-f]{64}$/);

  // main moves on another file; propagate-review merges it into the PR.
  git("checkout", "-q", "main");
  writeFileSync(join(dir, "other.md"), "one\ntwo\n");
  git("commit", "-qam", "main moves");
  const base2 = git("rev-parse", "HEAD");
  git("checkout", "-q", "pr");
  git("merge", "-q", "--no-edit", "main");
  const head2 = git("rev-parse", "HEAD");
  const second = stands(dir, "1", base2, head2, failedOn(first.diff_id));
  assert.equal(second.diff_id, first.diff_id);
  assert.equal(second.stands, "true");
  assert.match(second.stderr, /unchanged/);
});

test("end to end: an indentation-only fix changes the id, so a whitespace [must] gets its round", () => {
  const { dir, git } = repo();
  const base = git("rev-parse", "HEAD");
  git("checkout", "-q", "-b", "pr");
  writeFileSync(join(dir, "a.py"), "def f():\n    if True:\n    return 2\n");
  git("commit", "-qam", "pr change");
  const first = stands(dir, "1", base, git("rev-parse", "HEAD"), failedOn(""));
  writeFileSync(join(dir, "a.py"), "def f():\n    if True:\n        return 2\n");
  git("commit", "-qam", "indent");
  const second = stands(dir, "1", base, git("rev-parse", "HEAD"), failedOn(first.diff_id));
  assert.notEqual(second.diff_id, first.diff_id);
  assert.equal(second.stands, "false");
});

test("end to end: a body edit changes the id, so an answer in the body gets its round", () => {
  const { dir, git } = repo();
  const base = git("rev-parse", "HEAD");
  git("checkout", "-q", "-b", "pr");
  writeFileSync(join(dir, "a.py"), "def f():\n    return 2\n");
  git("commit", "-qam", "pr change");
  const head = git("rev-parse", "HEAD");
  const first = stands(dir, "1", base, head, failedOn(""));
  const edited = { ...failedOn(first.diff_id), body: "why, and the measurement the [must] asked for" };
  const second = stands(dir, "1", base, head, edited);
  assert.notEqual(second.diff_id, first.diff_id);
  assert.equal(second.stands, "false");
});

test("end to end: an unreadable PR or diff prints an empty id and reviews", () => {
  const { dir, git } = repo();
  const base = git("rev-parse", "HEAD");
  const unknown = "0".repeat(40);
  const out = stands(dir, "1", base, unknown, failedOn(""));
  assert.equal(out.diff_id, "");
  assert.equal(out.stands, "false");
  assert.match(out.stderr, /merge-base .* failed/);
});

test("a usage error exits nonzero and prints nothing for GITHUB_OUTPUT", () => {
  const res = spawnSync("python3", [script, "1"], { encoding: "utf8" });
  assert.equal(res.status, 2);
  assert.equal(res.stdout, "");
  assert.match(res.stderr, /usage/);
});
