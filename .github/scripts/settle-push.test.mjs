// The contract between push-branch and settle-push: push-branch records what
// it pushed per branch (record_push), and settle-push's no---sha default reads
// that record (pushed_sha, issue #421). The filename is the interface, named
// independently in each script; a silent rename on either side would demote
// every uncertified settle to "unverified" forever, with no failure anywhere.
// This pins the pair with a real round trip through a real git dir.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, os, sys

def load(name, filename):
    loader = importlib.machinery.SourceFileLoader(name, os.path.join(sys.argv[1], filename))
    spec = importlib.util.spec_from_loader(name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod

os.chdir(sys.argv[2])
push_branch = load("push_branch", "push-branch")
settle_push = load("settle_push", "settle-push")
assert settle_push.OWN_PUSHES == push_branch.OWN_PUSHES, (
    "the scripts disagree on the record filename")
before = settle_push.pushed_sha("claude/some-branch")
push_branch.record_push("claude/some-branch", "abc123def456")
print(json.dumps({
    "before": before,
    "recorded": settle_push.pushed_sha("claude/some-branch"),
    "unrecorded": settle_push.pushed_sha("claude/other-branch"),
}))
`;

test("settle-push's no---sha default is what push-branch recorded", () => {
  const repo = mkdtempSync(join(tmpdir(), "settle-push-test-"));
  try {
    const init = spawnSync("git", ["init", "-q", repo], { encoding: "utf8" });
    assert.equal(init.status, 0, init.stderr);
    const run = spawnSync("python3", ["-c", driver, scriptsDir, repo], {
      encoding: "utf8",
    });
    assert.equal(run.status, 0, run.stderr);
    const out = JSON.parse(run.stdout);
    assert.equal(out.before, null, "an unrecorded branch must read as absent");
    assert.equal(out.recorded, "abc123def456");
    assert.equal(out.unrecorded, null);
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

// The hour-mark fallback (issue #734): a read that meets the app token's 401
// (issue #81) retries once on GH_WORKFLOW_TOKEN; close and reopen never do,
// because the actor they carry is their whole point.
const fallbackDriver = `
import importlib.machinery, importlib.util, json, os, subprocess, sys

loader = importlib.machinery.SourceFileLoader("settle_push", os.path.join(sys.argv[1], "settle-push"))
spec = importlib.util.spec_from_loader("settle_push", loader)
sp = importlib.util.module_from_spec(spec)
loader.exec_module(sp)

calls = []
def fake_run(cmd, capture_output=True, text=True, env=None):
    token = (env or os.environ).get("GH_TOKEN")
    calls.append([list(cmd[1:3]), token])
    if token != "workflow":
        return subprocess.CompletedProcess(cmd, 1, "", "gh: Bad credentials (HTTP 401)")
    return subprocess.CompletedProcess(cmd, 0, "{}", "")

sp.subprocess.run = fake_run
os.environ.pop("GH_TOKEN", None)
os.environ["GH_WORKFLOW_TOKEN"] = "workflow"
read = sp.gh("api", "repos/o/r/actions/runs")
try:
    sp.gh("pr", "close", "1")
    closed = True
except SystemExit:
    closed = False
print(json.dumps({"read": read, "closed": closed, "calls": calls}))
`;

test("settle-push reads retry on the workflow token past the hour, close and reopen do not", () => {
  const run = spawnSync("python3", ["-c", fallbackDriver, scriptsDir], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  const out = JSON.parse(run.stdout);
  assert.equal(out.read, "{}");
  assert.equal(out.closed, false, "an attributed write must die on the session token");
  assert.deepEqual(out.calls, [
    [["api", "repos/o/r/actions/runs"], null],
    [["api", "repos/o/r/actions/runs"], "workflow"],
    [["pr", "close"], null],
  ]);
  assert.match(run.stderr, /#81/);
});
