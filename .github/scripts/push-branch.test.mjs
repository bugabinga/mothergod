// The hour-mark fallback (issue #734): the app token dies about an hour into
// a session (issue #81) and answers 401 to every call after. `api` retries
// once on the admin PAT when the environment holds one, and only on 401: a
// 403 is a permission answer and is never routed around. The tests drive the
// module's own `api` against a fake `gh`, so no network and no real token.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
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

// guard_pr_scope (issue #811): merge-pr and gh-comment both gained this
// refusal in 59e7530 (#782); push-branch was named in #771's original
// defect list and left out.
const scopeDriver = `
import importlib.machinery, importlib.util, os, sys

loader = importlib.machinery.SourceFileLoader("push_branch", os.path.join(sys.argv[1], "push-branch"))
spec = importlib.util.spec_from_loader("push_branch", loader)
pb = importlib.util.module_from_spec(spec)
loader.exec_module(pb)

try:
    pb.guard_pr_scope(sys.argv[2])
    died = False
except SystemExit:
    died = True
print(died)
`;

function runScope(target, env = {}) {
  const proc = spawnSync("python3", ["-c", scopeDriver, scriptsDir, target], {
    encoding: "utf8",
    env: { PATH: process.env.PATH, GITHUB_REPOSITORY: "o/r", ...env },
  });
  return { died: proc.stdout.trim() === "True", stderr: proc.stderr };
}

test("PR_NUMBER in the environment refuses a push to any other PR number", () => {
  const r = runScope("770", { PR_NUMBER: "768" });
  assert.equal(r.died, true);
  assert.match(r.stderr, /does not match PR_NUMBER=768/);
});

test("PR_NUMBER matching the target clears the guard", () => {
  const r = runScope("768", { PR_NUMBER: "768" });
  assert.equal(r.died, false);
});

test("no PR_NUMBER in the environment (every non-review seat) is unaffected", () => {
  const r = runScope("770", {});
  assert.equal(r.died, false);
});

test("a bare branch name is never scoped, PR_NUMBER or not", () => {
  const r = runScope("claude/some-branch", { PR_NUMBER: "768" });
  assert.equal(r.died, false);
});

// The empty-commit refusal (PR #848): a `<path>...` push whose named files
// match the branch head byte for byte builds the head's own tree, and the
// commit on it has no diff. `guard_not_empty` is pure (tree shas in, die or
// return) so both directions pin without a network.
const emptyDriver = `
import importlib.machinery, importlib.util, json, os, sys
loader = importlib.machinery.SourceFileLoader("push_branch", os.path.join(sys.argv[1], "push-branch"))
spec = importlib.util.spec_from_loader("push_branch", loader)
pb = importlib.util.module_from_spec(spec)
loader.exec_module(pb)
tree_sha, already_landed = sys.argv[2], sys.argv[3] == "landed"
try:
    pb.guard_not_empty(
        "claude/x's head", "base-tree", tree_sha,
        ["research/JOURNAL.md", "research/progress.jsonl"], already_landed,
    )
    died = False
except SystemExit:
    died = True
print(json.dumps({"died": died}))
`;

function guardEmpty(treeSha, alreadyLanded = false) {
  const proc = spawnSync(
    "python3",
    ["-c", emptyDriver, scriptsDir, treeSha, alreadyLanded ? "landed" : "fresh"],
    { encoding: "utf8" },
  );
  assert.equal(proc.status, 0, proc.stderr);
  return { ...JSON.parse(proc.stdout), stderr: proc.stderr };
}

test("a tree identical to the branch head's dies naming #848 and the two paths", () => {
  const r = guardEmpty("base-tree");
  assert.equal(r.died, true);
  assert.match(r.stderr, /2 path\(s\) named would leave claude\/x's head's tree unchanged/);
  assert.match(r.stderr, /#848/);
  assert.match(r.stderr, /git status/);
});

test("a tree that differs from the branch head's passes silently", () => {
  const r = guardEmpty("new-tree");
  assert.equal(r.died, false);
  assert.equal(r.stderr, "");
});

// Round 3 (#851): a retry after this run's own write landed but its read-back
// lagged (issue #193) rebuilds the identical tree against its own prior
// commit. That match is confirmation, not the #848 mistake, so a base that is
// this run's own recorded push exempts the guard even though the tree matches.
test("a tree identical to the base passes when the base is this run's own recorded push", () => {
  const r = guardEmpty("base-tree", true);
  assert.equal(r.died, false);
  assert.equal(r.stderr, "");
});

// End-to-end through the real record_push/own_pushes round trip (same style
// as settle-push.test.mjs), not just a hand-passed boolean: a P1 that wrote
// S1 and recorded it before its read-back lagged and died must exempt a P2
// retry that rebuilds the identical tree against S1, and must never exempt
// some other branch's base by accident. `tried` computes already_landed the
// same way main() does (push-branch:477-483), not the round-3 shortcut.
const wiringDriver = `
import importlib.machinery, importlib.util, json, os, sys

loader = importlib.machinery.SourceFileLoader("push_branch", os.path.join(sys.argv[1], "push-branch"))
spec = importlib.util.spec_from_loader("push_branch", loader)
pb = importlib.util.module_from_spec(spec)
loader.exec_module(pb)

os.chdir(sys.argv[2])
pb.record_push("claude/x", "s1")  # P1's write landed, read-back died after

def tried(branch, base_sha):
    own = pb.own_pushes().get(branch, {})
    try:
        pb.guard_not_empty(
            f"{branch}'s head", "t1", "t1", ["research/JOURNAL.md"],
            own.get("sha") == base_sha and not own.get("confirmed"),
        )
        return False
    except SystemExit:
        return True

print(json.dumps({
    "retry_on_own_base": tried("claude/x", "s1"),
    "other_branch_same_sha": tried("claude/y", "s1"),
    "same_branch_other_sha": tried("claude/x", "s0"),
}))
`;

test("a P2 retry against this run's own recorded push exempts the guard, nothing else does", () => {
  const repo = mkdtempSync(join(tmpdir(), "push-branch-test-"));
  try {
    const init = spawnSync("git", ["init", "-q", repo], { encoding: "utf8" });
    assert.equal(init.status, 0, init.stderr);
    const run = spawnSync("python3", ["-c", wiringDriver, scriptsDir, repo], { encoding: "utf8" });
    assert.equal(run.status, 0, run.stderr);
    const out = JSON.parse(run.stdout);
    assert.equal(out.retry_on_own_base, false, "P2 on P1's own landed base must not die");
    assert.equal(out.other_branch_same_sha, true, "a different branch at the same sha is not this run's push");
    assert.equal(out.same_branch_other_sha, true, "the same branch at a base we never recorded is not exempt");
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

// Round 4 (#851 review, "must"): own_pushes recorded only the last sha a
// checkout wrote, with no bit for whether the read-back ever confirmed it.
// That let the round-3 exemption cover every later same-tree push to a
// branch this checkout had ever landed on, not just the one lagging retry it
// was built for; a forgotten edit on push N would have been waved through
// the same way the original #848 bug was. confirm_push flips that bit once
// the read-back loop actually proves the ref moved, and guard_not_empty's
// exemption must stop applying the moment it does: a confirmed base is a
// push that already fully landed, so a later same-tree push against it is a
// forgotten edit, not a retry, and must still die.
const confirmDriver = `
import importlib.machinery, importlib.util, json, os, sys

loader = importlib.machinery.SourceFileLoader("push_branch", os.path.join(sys.argv[1], "push-branch"))
spec = importlib.util.spec_from_loader("push_branch", loader)
pb = importlib.util.module_from_spec(spec)
loader.exec_module(pb)

os.chdir(sys.argv[2])
pb.record_push("claude/x", "s1")

def tried():
    own = pb.own_pushes().get("claude/x", {})
    try:
        pb.guard_not_empty(
            "claude/x's head", "t1", "t1", ["research/JOURNAL.md"],
            own.get("sha") == "s1" and not own.get("confirmed"),
        )
        return False
    except SystemExit:
        return True

before_confirm = tried()
pb.confirm_push("claude/x", "s1")
after_confirm = tried()
print(json.dumps({"before_confirm": before_confirm, "after_confirm": after_confirm}))
`;

test("confirm_push ends the already_landed exemption; an unconfirmed push still gets it", () => {
  const repo = mkdtempSync(join(tmpdir(), "push-branch-test-"));
  try {
    const init = spawnSync("git", ["init", "-q", repo], { encoding: "utf8" });
    assert.equal(init.status, 0, init.stderr);
    const run = spawnSync("python3", ["-c", confirmDriver, scriptsDir, repo], { encoding: "utf8" });
    assert.equal(run.status, 0, run.stderr);
    const out = JSON.parse(run.stdout);
    assert.equal(out.before_confirm, false, "unconfirmed: a same-tree retry must not die");
    assert.equal(out.after_confirm, true, "confirmed: a later same-tree push must die, a forgotten edit");
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

// own_pushes must also hand settle-push and site-shots a plain sha, not the
// {"sha", "confirmed"} shape push-branch stores it in: both read the same
// file for a different purpose (which commit landed, not whether a retry is
// safe) and a dict leaking through would break their sha comparisons.
const shapeDriver = `
import importlib.machinery, importlib.util, json, os, sys

loader = importlib.machinery.SourceFileLoader("push_branch", os.path.join(sys.argv[1], "push-branch"))
spec = importlib.util.spec_from_loader("push_branch", loader)
pb = importlib.util.module_from_spec(spec)
loader.exec_module(pb)

os.chdir(sys.argv[2])
pb.record_push("claude/x", "s1")
entry = pb.own_pushes()["claude/x"]
print(json.dumps({"sha": entry.get("sha"), "confirmed": entry.get("confirmed")}))
`;

test("own_pushes stores sha and a confirmed bit, not a bare sha", () => {
  const repo = mkdtempSync(join(tmpdir(), "push-branch-test-"));
  try {
    const init = spawnSync("git", ["init", "-q", repo], { encoding: "utf8" });
    assert.equal(init.status, 0, init.stderr);
    const run = spawnSync("python3", ["-c", shapeDriver, scriptsDir, repo], { encoding: "utf8" });
    assert.equal(run.status, 0, run.stderr);
    const out = JSON.parse(run.stdout);
    assert.equal(out.sha, "s1");
    assert.equal(out.confirmed, false, "record_push must leave a fresh write unconfirmed");
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

// The format refusal (issue #938): six red `fmt` jobs on bdfl branches in
// sixteen days, each a maintainer run and a reviewer re-run, because the
// seat typing the push skipped `cargo x check`. `guard_formatted` asks x per
// path and dies on a finding with x's fix line. A stub `cargo` on PATH plays
// x's three answers (0 clean, 1 finding, 2 unsupported) by file name and
// logs every call, so the test also sees which paths were never asked.
const fmtDriver = `
import importlib.machinery, importlib.util, json, os, sys

loader = importlib.machinery.SourceFileLoader("push_branch", os.path.join(sys.argv[1], "push-branch"))
spec = importlib.util.spec_from_loader("push_branch", loader)
pb = importlib.util.module_from_spec(spec)
loader.exec_module(pb)

os.environ["PATH"] = sys.argv[2]
try:
    pb.guard_formatted(sys.argv[3:])
    died = False
except SystemExit:
    died = True
print(json.dumps({"died": died}))
`;

// x's refusals are two lines, the cause then a `help:` line, so the stub
// prints both: a note that quotes the last line quotes the help (#997 round 1).
const stubCargo = `#!/bin/sh
echo "$*" >> "$STUB_LOG"
help='help: run \`cargo x help\` or \`cargo x help <COMMAND>\`'
case "$5" in
  *unformatted*) echo "$5: needs formatting" >&2; echo "  fix: cargo x fmt -- $5" >&2; echo "fmt: 1 finding(s)" >&2; exit 1 ;;
  *.xyz) echo "error: $5: this file type is not supported by x" >&2; echo "$help" >&2; exit 2 ;;
  *link*) echo "error: $5: symbolic links are not rewritten by x" >&2; echo "$help" >&2; exit 2 ;;
  *broken*) echo "error: could not compile x" >&2; exit 101 ;;
  *) echo "fmt: 1 files checked"; exit 0 ;;
esac
`;

function guardFormatted(paths, { cargo = true } = {}) {
  const bin = mkdtempSync(join(tmpdir(), "push-branch-cargo-"));
  const log = join(bin, "calls.log");
  try {
    if (cargo) {
      writeFileSync(join(bin, "cargo"), stubCargo, { mode: 0o755 });
    }
    // git must stay reachable for rev-parse, so the stub dir goes first, PATH after.
    const path = `${bin}:${process.env.PATH}`;
    const proc = spawnSync("python3", ["-c", fmtDriver, scriptsDir, cargo ? path : bin, ...paths], {
      encoding: "utf8",
      env: { ...process.env, STUB_LOG: log },
    });
    assert.equal(proc.status, 0, proc.stderr);
    const asked = existsSync(log) ? readFileSync(log, "utf8").trim().split("\n") : [];
    return { ...JSON.parse(proc.stdout), stderr: proc.stderr, asked };
  } finally {
    rmSync(bin, { recursive: true, force: true });
  }
}

test("an unformatted file dies with x's fix line, naming #938, and nothing is pushed", () => {
  const r = guardFormatted(["bench/unformatted.json", "src/ok.rs"]);
  assert.equal(r.died, true);
  assert.match(r.stderr, /#938/);
  assert.match(r.stderr, /fix: cargo x fmt -- bench\/unformatted\.json/);
  assert.match(r.stderr, /nothing was pushed/);
  assert.doesNotMatch(r.stderr, /finding\(s\)/, "x's summary line is noise beside its fix line");
  assert.deepEqual(r.asked, ["x fmt --check -- bench/unformatted.json", "x fmt --check -- src/ok.rs"]);
});

test("Markdown and extensionless paths never ask x, so a docs push never builds it", () => {
  const r = guardFormatted(["ROADMAP.md", ".github/scripts/sessions", "docs/adr/0001-x.md"]);
  assert.equal(r.died, false);
  assert.equal(r.stderr, "");
  assert.deepEqual(r.asked, []);
});

test("a kind x refuses is skipped and the path beside it is still checked", () => {
  const r = guardFormatted(["assets/exotic.xyz", "bench/unformatted.json"]);
  assert.equal(r.died, true);
  assert.match(r.stderr, /bench\/unformatted\.json: needs formatting/);
  assert.deepEqual(r.asked, ["x fmt --check -- assets/exotic.xyz", "x fmt --check -- bench/unformatted.json"]);
});

test("formatted files pass silently", () => {
  const r = guardFormatted(["src/ok.rs", "bench/ok.json"]);
  assert.equal(r.died, false);
  assert.equal(r.stderr, "");
  assert.equal(r.asked.length, 2);
});

test("no cargo on PATH says so once and lets the push proceed, CI's gate decides", () => {
  const r = guardFormatted(["bench/unformatted.json"], { cargo: false });
  assert.equal(r.died, false);
  assert.match(r.stderr, /cargo is not on PATH/);
  assert.match(r.stderr, /CI's fmt job is the gate/);
  assert.deepEqual(r.asked, []);
});

test("an x that cannot run says so once, stops asking, and lets the push proceed", () => {
  const r = guardFormatted(["x/broken.rs", "bench/unformatted.json"]);
  assert.equal(r.died, false);
  assert.match(r.stderr, /could not run \(error: could not compile x\)/);
  assert.match(r.stderr, /CI's fmt job is the gate/);
  assert.deepEqual(r.asked, ["x fmt --check -- x/broken.rs"], "one note, then stop asking");
});

// #997 round 1, the reviewer's repro: a finding already held when x stops
// answering was dropped and the push proceeded, the silent loss this guard
// exists to remove.
test("a finding found before x stopped answering still refuses the push", () => {
  const r = guardFormatted(["bench/unformatted.json", "x/broken.rs"]);
  assert.equal(r.died, true);
  assert.match(r.stderr, /could not run \(error: could not compile x\)/);
  assert.match(r.stderr, /fix: cargo x fmt -- bench\/unformatted\.json/);
  assert.match(r.stderr, /nothing was pushed/);
});

test("a path x refuses on its own account is noted with x's cause, and its neighbours still answer", () => {
  const r = guardFormatted(["assets/link.svg", "bench/unformatted.json"]);
  assert.equal(r.died, true);
  assert.match(
    r.stderr,
    /assets\/link\.svg is not format-checked here \(error: assets\/link\.svg: symbolic links are not rewritten by x\)/,
  );
  assert.doesNotMatch(r.stderr, /help: run/, "the cause is x's error line, never its help line");
  assert.match(r.stderr, /bench\/unformatted\.json: needs formatting/);
  assert.deepEqual(r.asked, ["x fmt --check -- assets/link.svg", "x fmt --check -- bench/unformatted.json"]);
});
