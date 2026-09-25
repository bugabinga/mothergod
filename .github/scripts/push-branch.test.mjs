// The hour-mark fallback (issue #734): the app token dies about an hour into
// a session (issue #81) and answers 401 to every call after. `api` retries
// once on the admin PAT when the environment holds one, and only on 401: a
// 403 is a permission answer and is never routed around. The tests drive the
// module's own `api` against a fake `gh`, so no network and no real token.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, os, subprocess, sys

loader = importlib.machinery.SourceFileLoader("push_branch", os.path.join(sys.argv[1], "push-branch"))
spec = importlib.util.spec_from_loader("push_branch", loader)
pb = importlib.util.module_from_spec(spec)
loader.exec_module(pb)

scenario = sys.argv[2]
calls = []
answers = {
    "dead": (1, "", "gh: Bad credentials (HTTP 401)"),
    "forbidden": (1, "", "gh: Resource not accessible by integration (HTTP 403)"),
}

def fake_gh(args, token=None, stdin=None):
    calls.append(token)
    code, out, err = answers.get(token, (0, '{"sha": "abc"}', ""))
    return subprocess.CompletedProcess(args, code, out, err)

pb.gh = fake_gh
if scenario == "no-admin":
    os.environ.pop("GH_ADMIN_TOKEN", None)
else:
    os.environ["GH_ADMIN_TOKEN"] = "admin"
cred = pb.Credential("forbidden" if scenario == "forbidden" else "dead", "app token")
try:
    result, died = pb.api("GET", "git/ref/heads/x", cred), False
except SystemExit:
    result, died = None, True
print(json.dumps({"result": result, "died": died, "identity": cred.identity, "calls": calls}))
`;

function run(scenario) {
  const proc = spawnSync("python3", ["-c", driver, scriptsDir, scenario], {
    encoding: "utf8",
    env: { ...process.env, GITHUB_REPOSITORY: "o/r" },
  });
  assert.equal(proc.status, 0, proc.stderr);
  return { ...JSON.parse(proc.stdout), stderr: proc.stderr };
}

test("a 401 from the app token retries once on the admin PAT and says so", () => {
  const r = run("admin");
  assert.equal(r.died, false);
  assert.deepEqual(r.result, { sha: "abc" });
  assert.deepEqual(r.calls, ["dead", "admin"]);
  assert.equal(r.identity, "admin PAT", "later calls must ride the PAT too");
  assert.match(r.stderr, /#81/);
  assert.match(r.stderr, /operator-attributed/);
});

test("a 401 with no admin PAT dies naming the expiry and the write that still works", () => {
  const r = run("no-admin");
  assert.equal(r.died, true);
  assert.deepEqual(r.calls, ["dead"], "nothing to retry on, so no retry");
  assert.equal(r.identity, "app token");
  assert.match(r.stderr, /#81/);
  assert.match(r.stderr, /nothing was pushed/);
  assert.match(r.stderr, /gh-comment/);
});

test("a 403 is never routed around, admin PAT present or not", () => {
  const r = run("forbidden");
  assert.equal(r.died, true);
  assert.deepEqual(r.calls, ["forbidden"]);
  assert.equal(r.identity, "app token");
  assert.doesNotMatch(r.stderr, /retrying/);
});
