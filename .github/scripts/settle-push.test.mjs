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
