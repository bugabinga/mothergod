// Fixtures for stalled-prs, the detector that replaced three prose signatures
// in the BDFL prompt. The first fixture is PR #377's real rollup shape at
// 2026-08-30T12:11Z: every required gate green, the `review` check CANCELLED
// by a runner shutdown, no verdict label. That state was invisible to all
// three remembered signatures, and it is the reason this script exists, so it
// is the first thing the suite asserts.
//
// classify() is pure by construction (no network, no clock) precisely so these
// can exist. A detector nobody can make fire is decoration.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
from datetime import datetime
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("stalled_prs", sys.argv[1] + "/stalled-prs")
spec = importlib.util.spec_from_loader("stalled_prs", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
now = datetime.fromisoformat(sys.argv[2].replace("Z", "+00:00"))
print(json.dumps(getattr(mod, sys.argv[4])(json.loads(sys.argv[3]), now)))
`;

const NOW = "2026-08-30T12:11:00Z";

function call(fn, subject, now) {
  const run = spawnSync(
    "python3",
    ["-c", driver, scriptsDir, now, JSON.stringify(subject), fn],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

function classify(pr, now = NOW) {
  return call("classify", pr, now);
}

function classifyBranch(branch, now = NOW) {
  return call("classify_branch", branch, now);
}

function check(name, conclusion, extra = {}) {
  return {
    __typename: "CheckRun",
    name,
    workflowName: "ci",
    status: "COMPLETED",
    conclusion,
    startedAt: "2026-08-30T10:50:00Z",
    completedAt: "2026-08-30T10:53:00Z",
    ...extra,
  };
}

const GREEN_GATES = ["fmt", "clippy", "test", "doc", "ratio"].map((n) => check(n, "SUCCESS"));

function review(conclusion, extra = {}) {
  return check("review", conclusion, {
    workflowName: "agent-review",
    completedAt: "2026-08-30T10:54:26Z",
    ...extra,
  });
}

function pr(overrides = {}) {
  return {
    number: 377,
    title: "codec: add decompress_bounded",
    labels: [],
    mergeStateStatus: "UNSTABLE",
    headRefName: "claude/decompress-bounded-decode",
    createdAt: "2026-08-30T10:49:32Z",
    isDraft: false,
    isCrossRepository: false,
    statusCheckRollup: [...GREEN_GATES],
    ...overrides,
  };
}

test("PR #377: gates green, review cancelled by a runner shutdown, no verdict", () => {
  const found = classify(
    pr({ statusCheckRollup: [...GREEN_GATES, review("CANCELLED")] }),
  );
  assert.equal(found.kind, "reviewer-died");
  assert.match(found.detail, /CANCELLED at 2026-08-30T10:54:26Z/);
  assert.match(found.rescue, /gh pr reopen 377/);
});

test("a replacement review already in flight supersedes the dead one", () => {
  // The rollup keeps both entries after a rescue. Reading the first match
  // would report a stall that is actively being fixed.
  const found = classify(
    pr({
      statusCheckRollup: [
        ...GREEN_GATES,
        review("CANCELLED"),
        review(null, {
          status: "IN_PROGRESS",
          startedAt: "2026-08-30T12:14:50Z",
          completedAt: null,
        }),
      ],
    }),
  );
  assert.equal(found, null);
});

test("a review that succeeded and applied no verdict label is a stall too", () => {
  const found = classify(
    pr({ statusCheckRollup: [...GREEN_GATES, review("SUCCESS")] }),
  );
  assert.equal(found.kind, "verdict-missing");
});

test("approved with green gates and still open is the unsigned-tip stall", () => {
  const found = classify(
    pr({
      labels: [{ name: "agent-approved" }],
      statusCheckRollup: [...GREEN_GATES, review("SUCCESS")],
    }),
  );
  assert.equal(found.kind, "approved-not-landing");
  assert.match(found.rescue, /merge-pr 377/);
});

test("changes-requested is a verdict, not a stall: the author owns the move", () => {
  const found = classify(
    pr({
      labels: [{ name: "changes-requested" }],
      statusCheckRollup: [...GREEN_GATES, review("SUCCESS")],
    }),
  );
  assert.equal(found, null);
});

test("a dirty merge state outranks whatever the reviewer did", () => {
  const found = classify(
    pr({
      mergeStateStatus: "DIRTY",
      statusCheckRollup: [...GREEN_GATES, review("CANCELLED")],
    }),
  );
  assert.equal(found.kind, "dirty");
});

test("no checks at all, past the grace, is conflicted at birth", () => {
  const found = classify(pr({ statusCheckRollup: [] }));
  assert.equal(found.kind, "never-fired");
  assert.match(found.detail, /no merge ref/);
});

test("no checks yet, inside the grace, is a PR that was just opened", () => {
  const found = classify(pr({ statusCheckRollup: [] }), "2026-08-30T10:55:00Z");
  assert.equal(found, null);
});

test("a required gate missing by name names it, and doubts itself first", () => {
  const found = classify(
    pr({ statusCheckRollup: GREEN_GATES.filter((c) => c.name !== "ratio") }),
  );
  assert.equal(found.kind, "never-fired");
  assert.match(found.detail, /ratio/);
  assert.match(found.rescue, /renamed in branch protection/);
});

test("a red gate is not a stall: its author owns the next move", () => {
  const found = classify(
    pr({
      statusCheckRollup: [
        ...GREEN_GATES.filter((c) => c.name !== "test"),
        check("test", "FAILURE"),
        review("CANCELLED"),
      ],
    }),
  );
  assert.equal(found, null);
});

test("a gate still running is not a stall", () => {
  const found = classify(
    pr({
      statusCheckRollup: [
        ...GREEN_GATES.filter((c) => c.name !== "test"),
        check("test", null, { status: "IN_PROGRESS", completedAt: null }),
      ],
    }),
  );
  assert.equal(found, null);
});

for (
  const [why, overrides] of [
    ["blocked-on-human is parked on purpose", { labels: [{ name: "blocked-on-human" }] }],
    ["a draft is a human's work-in-progress signal", { isDraft: true }],
    ["a fork PR belongs to the heartbeat", { isCrossRepository: true }],
  ]
) {
  test(`suppressed: ${why}`, () => {
    const found = classify(
      pr({ statusCheckRollup: [...GREEN_GATES, review("CANCELLED")], ...overrides }),
    );
    assert.equal(found, null);
  });
}

// branch-orphaned: the signature with no PR to hang on (issue #489). Run
// 33677765718 pushed `claude/bdfl-miri-lane` at 22:20:48Z after two hours of
// Miri measurement, then died on `gh pr create` with an expired app token.
// Nothing on GitHub said so; the next session found it only because the dead
// one had written a prose handoff.
test("a branch pushed and never PR'd, past grace, is stalled work", () => {
  const found = classifyBranch(
    { name: "claude/bdfl-miri-lane", pushed: "2026-09-02T22:20:48Z" },
    "2026-09-02T23:30:00Z",
  );
  assert.equal(found.kind, "branch-orphaned");
  assert.match(found.detail, /2026-09-02T22:20:48Z/);
  assert.match(found.rescue, /gh pr create --head claude\/bdfl-miri-lane/);
});

test("a branch pushed minutes ago is a live session, not a stall", () => {
  // push-branch and `gh pr create` are seconds apart, but the session between
  // them can be doing anything. Reporting that is a false line every wake.
  assert.equal(
    classifyBranch(
      { name: "claude/bdfl-miri-lane", pushed: "2026-09-02T22:20:48Z" },
      "2026-09-02T22:35:00Z",
    ),
    null,
  );
});

test("a branch older than the activity window reports rather than hides", () => {
  // Absent from the feed can only mean older than it, and a detector that
  // stays quiet on missing data is the failure this whole script exists for.
  const found = classifyBranch({ name: "claude/ancient", pushed: null });
  assert.equal(found.kind, "branch-orphaned");
  assert.match(found.detail, /before the activity window/);
});

// orphans() is the one part with network in it, and the reviewer of PR #490
// found it crashing on the second page: `gh api --paginate --jq` prints one
// document per page, so json.loads raises "Extra data" the day this repo
// passes 30 branches. Dormant then, not dormant later, and it would have taken
// all six signatures down with it. The stub returns the shape
// `--paginate --slurp` actually returns: a list of pages.
const orphansDriver = `
import importlib.machinery, importlib.util, json, sys
from datetime import datetime
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("stalled_prs", sys.argv[1] + "/stalled-prs")
spec = importlib.util.spec_from_loader("stalled_prs", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)

def fake_gh(*args):
    if args[0] == "api" and "branches" in args[1]:
        return json.dumps([[{"name": "main"}, {"name": "claude/orphan"}], [{"name": "claude/had-a-pr"}]])
    if args[0] == "api":
        return json.dumps([{"ref": "refs/heads/claude/orphan", "timestamp": "2026-09-02T22:20:48Z"}])
    if args[0] == "pr":
        return json.dumps([{"number": 7}] if "claude/had-a-pr" in args else [])
    raise AssertionError(args)

mod.gh = fake_gh
rows, branches = mod.orphans(set(), datetime.fromisoformat(sys.argv[2].replace("Z", "+00:00")))
print(json.dumps({"branches": branches, "found": [[b["name"], f["kind"]] for b, f in rows]}))
`;

test("orphans reads every page, and skips the branch that had its PR", () => {
  const run = spawnSync(
    "python3",
    ["-c", orphansDriver, scriptsDir, "2026-09-02T23:30:00Z"],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  const out = JSON.parse(run.stdout);
  assert.equal(out.branches, 3);
  assert.deepEqual(out.found, [["claude/orphan", "branch-orphaned"]]);
});
