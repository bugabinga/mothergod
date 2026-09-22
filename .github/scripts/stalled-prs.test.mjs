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

test("PR #516: a dying review's half-applied label does not mask its death", () => {
  // The 401ing review session applied changes-requested five seconds before
  // its run failed, posting no items. A label from a dead round vouches for
  // nothing; the stall must still report.
  const found = classify(
    pr({
      labels: [{ name: "changes-requested" }],
      statusCheckRollup: [...GREEN_GATES, review("FAILURE")],
    }),
  );
  assert.equal(found.kind, "reviewer-died");
});

test("a stale approval does not authorize a head whose review died", () => {
  // agent-approved from an earlier round plus a dead re-review means the
  // current head is unreviewed; the rescue is a fresh review, not a merge.
  const found = classify(
    pr({
      labels: [{ name: "agent-approved" }],
      statusCheckRollup: [...GREEN_GATES, review("CANCELLED")],
    }),
  );
  assert.equal(found.kind, "reviewer-died");
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
  // #594: the same signature covers a duplicate, whose rescue is not a merge.
  assert.match(found.rescue, /read the conflict first/);
  assert.match(found.rescue, /raced/);
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

// --rescue (issue #566): the three single-command rescues stop being commands
// a BDFL wake types after reading this report. plan() is pure so the holds can
// be asserted without a PR to break; apply() is the one line of subprocess.
const planDriver = `
import importlib.machinery, importlib.util, json, sys
from datetime import datetime
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("stalled_prs", sys.argv[1] + "/stalled-prs")
spec = importlib.util.spec_from_loader("stalled_prs", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
now = datetime.fromisoformat(sys.argv[2].replace("Z", "+00:00"))
case = json.loads(sys.argv[3])
found = mod.classify(case["pr"], now)
print(json.dumps({"found": found, "plan": mod.plan(case["pr"], found, now, case.get("files"), case.get("approved_at"))}))
`;

function planFor(pr, extra = {}, now = NOW) {
  const run = spawnSync(
    "python3",
    ["-c", planDriver, scriptsDir, now, JSON.stringify({ pr, ...extra })],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const HEAD = "811b32a5545ed62ea7278b844801ba351fe4015a";
const approved = (extra = {}) =>
  pr({
    headRefOid: HEAD,
    labels: [{ name: "agent-approved" }],
    statusCheckRollup: [...GREEN_GATES, review("SUCCESS", { startedAt: "2026-08-30T10:53:30Z" })],
    ...extra,
  });

test("a dead review past the grace is refired: close, then reopen", () => {
  const { found, plan } = planFor(
    pr({ statusCheckRollup: [...GREEN_GATES, review("CANCELLED")] }),
  );
  assert.equal(found.review.completedAt, "2026-08-30T10:54:26Z");
  assert.deepEqual(plan, {
    argv: [["gh", "pr", "close", "377"], ["gh", "pr", "reopen", "377"]],
  });
});

test("a review that died minutes ago is held: it chases the wall it died against", () => {
  const { plan } = planFor(
    pr({ statusCheckRollup: [...GREEN_GATES, review("CANCELLED")] }),
    {},
    "2026-08-30T11:02:00Z",
  );
  assert.match(plan.hold, /7m ago, inside the 15m grace/);
});

test("a verdict-less review gets the same refire", () => {
  const { found, plan } = planFor(
    pr({ statusCheckRollup: [...GREEN_GATES, review("SUCCESS")] }),
  );
  assert.equal(found.kind, "verdict-missing");
  assert.deepEqual(plan.argv, [["gh", "pr", "close", "377"], ["gh", "pr", "reopen", "377"]]);
});

test("PR #571: approved after the head's review round, no workflow file, lands on that head", () => {
  const { found, plan } = planFor(approved(), {
    files: ["src/column.rs"],
    approved_at: "2026-08-30T10:56:50Z",
  });
  assert.equal(found.kind, "approved-not-landing");
  assert.equal(plan.argv.length, 1);
  assert.match(plan.argv[0][0], /merge-pr$/);
  assert.deepEqual(plan.argv[0].slice(1), ["377", "--sha", HEAD]);
});

test("a workflow file makes the merge the BDFL's, so it is held", () => {
  const { plan } = planFor(approved(), {
    files: ["src/column.rs", ".github/workflows/agent-review.yml"],
    approved_at: "2026-08-30T10:56:50Z",
  });
  assert.match(plan.hold, /BDFL's to land/);
});

test("an approval older than the head's review round is held, not trusted", () => {
  // The label survived a push. It vouches for a head that no longer exists.
  const { plan } = planFor(approved(), {
    files: ["src/column.rs"],
    approved_at: "2026-08-30T10:40:00Z",
  });
  assert.match(plan.hold, /predates the review round/);
  assert.match(plan.hold, new RegExp(`merge-pr 377 --sha ${HEAD}`));
});

test("a file list that did not read holds the merge; a vacuous read is not proof", () => {
  const { plan } = planFor(approved(), { approved_at: "2026-08-30T10:56:50Z" });
  assert.match(plan.hold, /did not read/);
});

test("a dirty PR has no mechanical rescue", () => {
  const { found, plan } = planFor(
    pr({ mergeStateStatus: "DIRTY", statusCheckRollup: [...GREEN_GATES, review("CANCELLED")] }),
  );
  assert.equal(found.kind, "dirty");
  assert.equal(plan, null);
});

const applyDriver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("stalled_prs", sys.argv[1] + "/stalled-prs")
spec = importlib.util.spec_from_loader("stalled_prs", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
print(json.dumps(mod.apply(json.loads(sys.argv[2]))))
`;

function applySteps(steps) {
  const run = spawnSync("python3", ["-c", applyDriver, scriptsDir, JSON.stringify(steps)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  const lines = run.stdout.trimEnd().split("\n");
  return { ok: JSON.parse(lines.pop()), printed: lines };
}

test("apply prints each command with the tool's last line, and stops at the first failure", () => {
  const clean = applySteps([["sh", "-c", "echo merge-pr: merged 377"]]);
  assert.equal(clean.ok, true);
  assert.deepEqual(clean.printed, ["        applied: sh -c echo merge-pr: merged 377 -> merge-pr: merged 377"]);

  const broken = applySteps([["sh", "-c", "echo nope >&2; exit 3"], ["sh", "-c", "echo never"]]);
  assert.equal(broken.ok, false);
  assert.deepEqual(broken.printed, ["        applied: sh -c echo nope >&2; exit 3 -> FAILED exit 3 -> nope"]);
});

// The two reads the merge arm depends on, and rescue() around them, against a
// stubbed `gh` on PATH. The reviewer of PR #572 found approved_at() dying the
// whole sweep on one failed timeline read while the docstring promised the
// sweep goes on; these pin the soft failure and the hold it becomes.
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const readsDriver = `
import importlib.machinery, importlib.util, json, sys
from datetime import datetime
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("stalled_prs", sys.argv[1] + "/stalled-prs")
spec = importlib.util.spec_from_loader("stalled_prs", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
now = datetime.fromisoformat(sys.argv[2].replace("Z", "+00:00"))
case = json.loads(sys.argv[3])
out = {"approved_at": mod.approved_at(571), "changed_files": mod.changed_files(571)}
if "pr" in case:
    found = mod.classify(case["pr"], now)
    out["rescue"] = mod.rescue(case["pr"], found, now)
print(json.dumps(out))
`;

// A gh that answers `pr view` and `api` from the script given, and fails
// everything else loudly; the stub's exit code is the failure under test.
function withGh(script, extra = {}, now = NOW) {
  const dir = mkdtempSync(join(tmpdir(), "stalled-prs-gh-"));
  writeFileSync(join(dir, "gh"), `#!/bin/sh\n${script}\n`);
  chmodSync(join(dir, "gh"), 0o755);
  const run = spawnSync(
    "python3",
    ["-c", readsDriver, scriptsDir, now, JSON.stringify(extra)],
    { encoding: "utf8", env: { ...process.env, PATH: `${dir}:${process.env.PATH}` } },
  );
  assert.equal(run.status, 0, run.stderr);
  const lines = run.stdout.trimEnd().split("\n");
  return { out: JSON.parse(lines.pop()), printed: lines };
}

const TIMELINE = JSON.stringify([
  [
    { event: "labeled", label: { name: "changes-requested" }, created_at: "2026-09-17T09:29:59Z" },
    { event: "committed", sha: "811b32a" },
  ],
  [
    { event: "labeled", label: { name: "agent-approved" }, created_at: "2026-09-17T09:36:50Z" },
    { event: "unlabeled", label: { name: "changes-requested" }, created_at: "2026-09-17T09:36:51Z" },
  ],
]);

test("approved_at reads the latest agent-approved event across pages; changed_files reads paths", () => {
  const { out } = withGh(
    `case "$1 $2" in
       "pr view") echo '{"files":[{"path":"src/column.rs"}]}' ;;
       "api repos/{owner}/{repo}/issues/571/timeline?per_page=100") echo '${TIMELINE}' ;;
       *) echo "unexpected: $*" >&2; exit 9 ;;
     esac`,
  );
  assert.equal(out.approved_at, "2026-09-17T09:36:50Z");
  assert.deepEqual(out.changed_files, ["src/column.rs"]);
});

test("a gh that fails makes both reads None instead of ending the sweep", () => {
  const { out } = withGh(`echo "HTTP 502" >&2; exit 1`);
  assert.equal(out.approved_at, null);
  assert.equal(out.changed_files, null);
});

test("an unparseable timeline is None too", () => {
  const { out } = withGh(`echo 'not json'`);
  assert.equal(out.approved_at, null);
});

test("rescue() on a timeline that did not read holds, prints why, and returns", () => {
  const { out, printed } = withGh(
    `case "$1 $2" in
       "pr view") echo '{"files":[{"path":"src/column.rs"}]}' ;;
       *) echo "HTTP 502" >&2; exit 1 ;;
     esac`,
    { pr: approved() },
  );
  assert.equal(out.rescue, "held");
  assert.equal(printed.length, 1);
  assert.match(printed[0], /^ {8}held: the agent-approved label event did not read/);
  assert.match(printed[0], new RegExp(`merge-pr 377 --sha ${HEAD}`));
});
