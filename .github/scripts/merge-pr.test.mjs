// merge-pr's PR_NUMBER scope guard (issue #771, closing PR #772) and its
// `--wait` path (issue #842). The stub `gh` below answers from a script of
// responses keyed by a regex over the argv it was called with, each key a
// sequence consumed call by call with the last answer repeating, so a poll
// that must see "in progress, then concluded" has something to see. With no
// script every call fails loudly, so the guard tests prove only that the
// guard did or did not let the call through. The unscripted merge paths
// (403 escalation, a first-attempt 409) still have no coverage here; 405 and
// the arm refusal it can meet are covered below. The timeline read in
// verdict.py is covered through the wait's stale-approval and unread-timeline
// tests only.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = join(new URL(".", import.meta.url).pathname, "merge-pr");

function run(args, { env = {}, responses = [] } = {}) {
  const stub = mkdtempSync(join(tmpdir(), "merge-pr-"));
  const log = join(stub, "calls.jsonl");
  const counts = join(stub, "counts.json");
  const recorder = join(stub, "record.mjs");
  writeFileSync(join(stub, "responses.json"), JSON.stringify(responses));
  writeFileSync(
    recorder,
    `import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
const argv = process.argv.slice(2);
appendFileSync(${JSON.stringify(log)}, JSON.stringify(argv) + "\\n");
const responses = JSON.parse(readFileSync(${JSON.stringify(join(stub, "responses.json"))}, "utf8"));
const counts = existsSync(${JSON.stringify(counts)}) ? JSON.parse(readFileSync(${JSON.stringify(counts)}, "utf8")) : {};
const line = argv.join(" ");
const hit = responses.findIndex((r) => new RegExp(r.match).test(line));
if (hit < 0) {
  console.error("HTTP 500 stub: gh not configured for this call");
  process.exit(1);
}
const seq = responses[hit].responses;
const n = counts[hit] ?? 0;
counts[hit] = n + 1;
writeFileSync(${JSON.stringify(counts)}, JSON.stringify(counts));
const answer = seq[Math.min(n, seq.length - 1)];
if (answer.stdout !== undefined) process.stdout.write(typeof answer.stdout === "string" ? answer.stdout : JSON.stringify(answer.stdout));
if (answer.stderr) process.stderr.write(answer.stderr);
process.exit(answer.status ?? 0);
`,
  );
  writeFileSync(join(stub, "gh"), `#!/bin/sh\nexec node ${recorder} "$@"\n`);
  chmodSync(join(stub, "gh"), 0o755);
  const proc = spawnSync("python3", [script, ...args], {
    encoding: "utf8",
    env: {
      PATH: `${stub}:${process.env.PATH}`,
      GITHUB_REPOSITORY: "o/r",
      ...env,
    },
  });
  const calls = existsSync(log)
    ? readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map((l) => JSON.parse(l))
    : [];
  return { ...proc, calls, merges: calls.filter((c) => c.includes("PUT")) };
}

// Issue #771: a review scoped to #768 commented on and merged #770 instead.
// A review session's environment carries PR_NUMBER; nothing outside that
// scope may reach gh, no matter how the wrong number was resolved.
test("PR_NUMBER in the environment refuses a merge of any other number", () => {
  const { status, calls, stderr } = run(["770"], { env: { PR_NUMBER: "768" } });
  assert.equal(status, 1);
  assert.deepEqual(calls, []);
  assert.match(stderr, /does not match PR_NUMBER=768/);
});

test("PR_NUMBER matching the target clears the guard", () => {
  const { calls } = run(["768"], { env: { PR_NUMBER: "768" } });
  assert.ok(calls.length > 0, "merge-pr should have reached gh once the guard passed");
});

test("no PR_NUMBER in the environment (every non-review seat) is unaffected", () => {
  const { calls } = run(["770"], {});
  assert.ok(calls.length > 0, "merge-pr should have reached gh with no scope set");
});

// --wait fixtures: PR #42 at head `abc`, ruleset requiring fmt and test.
const HEAD = "abc1234567890abcdef";
const pr = (labels, extra = {}) => ({
  stdout: {
    state: "open",
    merged: false,
    html_url: "https://example/pull/42",
    head: { sha: HEAD },
    node_id: "PR_node42",
    labels: labels.map((name) => ({ name })),
    ...extra,
  },
});
const ruleset = {
  match: "^api repos/o/r/rules/branches/main$",
  responses: [
    {
      stdout: [
        {
          type: "required_status_checks",
          parameters: { required_status_checks: [{ context: "fmt" }, { context: "test" }] },
        },
      ],
    },
  ],
};
const runs = (...entries) => ({
  stdout: {
    check_runs: entries.map(([name, status, conclusion, started_at = "2026-10-09T08:00:00Z"]) => ({
      name,
      status,
      conclusion,
      started_at,
    })),
  },
});
// The review run in every fixture started at 08:00:00Z; the label event
// either postdates it (a verdict on this head) or predates it (a label that
// survived a push, PR #966's review).
// The read is on GITHUB_REPOSITORY, never gh's `{owner}/{repo}` placeholder,
// which expands only inside a checkout (#968).
const labeled = (at) => ({
  match: "^api repos/o/r/issues/42/timeline",
  responses: [{ stdout: [[{ event: "labeled", label: { name: "agent-approved" }, created_at: at }]] }],
});
const APPROVED_NOW = labeled("2026-10-09T08:01:30Z");
const APPROVED_EARLIER = labeled("2026-10-09T07:30:00Z");
const GREEN_GATES = [["fmt", "completed", "success"], ["test", "completed", "success"]];
const merge = { match: "PUT repos/o/r/pulls/42/merge", responses: [{ stdout: { sha: "squash1" } }] };
const WAIT = ["42", "--wait", "--interval", "0", "--timeout", "60"];

test("--wait: an approving verdict on the pinned head with green gates merges", () => {
  const { status, stdout, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([]), pr(["agent-approved"])] },
      ruleset,
      {
        match: "check-runs",
        responses: [
          runs(["review", "in_progress", null], ...GREEN_GATES),
          runs(["review", "completed", "success"], ...GREEN_GATES),
        ],
      },
      APPROVED_NOW,
      merge,
    ],
  });
  assert.equal(status, 0, stdout);
  assert.match(
    stdout,
    /approved at abc123456789 \(2026-10-09T08:01:30Z, after the review started 2026-10-09T08:00:00Z\), every required gate green/,
  );
  assert.match(stdout, /merged #42 at abc123456789 as squash1/);
  assert.equal(merges.length, 1);
  assert.match(merges[0].join(" "), /PUT repos\/o\/r\/pulls\/42\/merge/);
});

test("--wait: a stale agent-approved is not read until this head's review concludes", () => {
  // Round 1 approved, the author pushed, round 2 is running: the label is on
  // the PR the whole time and must not land the unreviewed head.
  const { status, stderr, merges } = run(["42", "--wait", "--interval", "0", "--timeout", "0"], {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "in_progress", null], ...GREEN_GATES)] },
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(stderr, /waited 0.*review in_progress, verdict labels agent-approved/);
  assert.deepEqual(merges, []);
});

test("--wait: an agent-approved applied before this head's review run started is held", () => {
  // Round 1 approved, the author pushed, the guard was paused, round 2 ended
  // green having run nothing: the label is on the PR, the run is success, and
  // the head is unreviewed. The label event, not the label, is the evidence.
  const { status, stderr, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      APPROVED_EARLIER,
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(
    stderr,
    /applied at 2026-10-09T07:30:00Z, before this head's review round started at 2026-10-09T08:00:00Z/,
  );
  assert.deepEqual(merges, []);
});

test("--wait: a second review run on the same head after the PASS is the same round, so it lands", () => {
  // PR #1243 (#1301): round 1 posted its PASS at 08:00 and the label event
  // followed at 08:01:30; an `edited` event raised a second run on the same
  // head at 08:02 that skipped the model (#1246) and concluded success. The
  // round began when its first run did, so the label postdates it. Reading
  // the newest run held this head at exit 4 with every gate green.
  const { status, stdout, stderr, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      {
        match: "check-runs",
        responses: [
          runs(
            ["review", "completed", "success", "2026-10-09T08:00:00Z"],
            ["review", "completed", "success", "2026-10-09T08:02:00Z"],
            ...GREEN_GATES,
          ),
        ],
      },
      APPROVED_NOW,
      merge,
    ],
  });
  assert.equal(status, 0, stdout + stderr);
  assert.match(stdout, /after the review started 2026-10-09T08:00:00Z/);
  assert.equal(merges.length, 1);
});

test("--wait: a queued re-read with no start stamp beside a concluded round does not hold it", () => {
  // The null stamp belongs to a run that has not started; the round's start
  // is its first run's, and only a head with no stamped run at all is held.
  const { status, stdout, stderr, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      {
        match: "check-runs",
        responses: [
          runs(
            ["review", "completed", "success", "2026-10-09T08:00:00Z"],
            ["review", "queued", null, null],
            ...GREEN_GATES,
          ),
        ],
      },
      APPROVED_NOW,
      merge,
    ],
  });
  assert.equal(status, 0, stdout + stderr);
  assert.equal(merges.length, 1);
});

test("--wait: a concluded review run with a null started_at is held, not merged", () => {
  // The label event reads fine; the run's own stamp is missing. `labeled < ""`
  // is False for every stamp, so the old read merged on no timing evidence
  // (#967). Polling cannot grow a stamp a finished run never got: exit 4.
  const { status, stderr, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success", null], ...GREEN_GATES)] },
      APPROVED_NOW,
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(stderr, /review round on abc123456789 concluded with no started_at stamp/);
  assert.deepEqual(merges, []);
});

test("--wait: one failed read inside the deadline is retried, not fatal", () => {
  const { status, stdout, merges } = run(WAIT, {
    responses: [
      {
        match: "^api repos/o/r/pulls/42$",
        responses: [
          { status: 1, stderr: "gh: Server Error (HTTP 502)", stdout: { message: "Server Error" } },
          pr(["agent-approved"]),
        ],
      },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      APPROVED_NOW,
      merge,
    ],
  });
  assert.equal(status, 0, stdout);
  assert.match(stdout, /read failed \(Server Error\); retrying at the next poll/);
  assert.equal(merges.length, 1);
});

test("--wait: changes-requested exits 3 and merges nothing", () => {
  const { status, stderr, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["changes-requested"])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      merge,
    ],
  });
  assert.equal(status, 3);
  assert.match(stderr, /changes requested on #42 at abc123456789: read https:\/\/example\/pull\/42/);
  assert.deepEqual(merges, []);
});

test("--wait: a red required gate exits 2 at once instead of waiting on it", () => {
  const { status, stderr, merges, calls } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      {
        match: "check-runs",
        responses: [
          runs(["review", "completed", "success"], ["fmt", "completed", "failure"], ["test", "in_progress", null]),
        ],
      },
      merge,
    ],
  });
  assert.equal(status, 2);
  assert.match(stderr, /required gates red on abc123456789: fmt/);
  assert.deepEqual(merges, []);
  assert.equal(calls.filter((c) => c.join(" ").includes("check-runs")).length, 1, "one poll, no second");
});

test("--wait: approved with a gate still pending keeps waiting, then names it at the deadline", () => {
  const { status, stderr, merges } = run(["42", "--wait", "--interval", "0", "--timeout", "0"], {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      {
        match: "check-runs",
        responses: [
          runs(["review", "completed", "success"], ["fmt", "completed", "success"], ["test", "queued", null]),
        ],
      },
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(stderr, /review success, verdict labels agent-approved, gates pending test/);
  assert.deepEqual(merges, []);
});

test("--wait: a timeline that never reads is named at the deadline, not 'gates pending none'", () => {
  // #968: run from outside a checkout, the placeholder read failed every poll
  // and the deadline exit named nothing a reader could act on.
  const { status, stderr, stdout, merges, calls } = run(["42", "--wait", "--interval", "0", "--timeout", "0"], {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      {
        match: "^api repos/o/r/issues/42/timeline",
        responses: [{ status: 1, stderr: "unable to expand placeholder in path" }],
      },
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(stdout, /the agent-approved label event did not read; retrying/);
  assert.match(
    stderr,
    /gates pending none; the agent-approved event never read, `gh api repos\/o\/r\/issues\/42\/timeline` says why/,
  );
  assert.deepEqual(merges, []);
  assert.equal(calls.filter((c) => c.join(" ").includes("timeline")).length, 1);
});

test("--wait: a failed review run exits 4, nobody will post a verdict", () => {
  const { status, stderr, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "failure"], ...GREEN_GATES)] },
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(stderr, /review failure on abc123456789: no verdict will come/);
  assert.deepEqual(merges, []);
});

// --gates-only (#975): the one PR no review can start on, agent-review.yml's
// own edit, lands on its gates alone. The flag is the caller's typed
// discretion; the script checks its precondition from the file list and
// infers nothing from the diff otherwise.
const files = (...paths) => ({
  match: "^api repos/o/r/pulls/42/files",
  responses: [{ stdout: paths.map((filename) => ({ filename })) }],
});
const GATES_ONLY = [...WAIT, "--gates-only"];

// The workflow carve-out (GOVERNANCE "Merging"): on 2026-10-18 the reviewer
// seat merged #1363, a workflow-touching PR, against its own verdict, the
// platform 403 that once stopped it long gone. The seat is ROLE, which every
// agent workflow exports; a shell without one is a human's.
test("ROLE other than bdfl refuses a PR touching .github/workflows/ before any merge", () => {
  const { status, stderr, merges, calls } = run(["42"], {
    env: { ROLE: "reviewer" },
    responses: [files(".github/workflows/agent-model-intel.yml", ".github/scripts/sessions")],
  });
  assert.equal(status, 1);
  assert.match(stderr, /#42 touches \.github\/workflows\/, the BDFL's to merge and not the reviewer seat's/);
  assert.deepEqual(merges, []);
  assert.equal(calls.length, 1); // the file list, nothing after it
});

test("ROLE other than bdfl on a PR touching no workflow file goes on to the merge", () => {
  const { status, stdout, merges } = run(["42"], {
    env: { ROLE: "reviewer" },
    responses: [files(".github/scripts/merge-pr"), { match: "^api repos/o/r/pulls/42$", responses: [pr([])] }, merge],
  });
  assert.equal(status, 0, stdout);
  assert.equal(merges.length, 1);
});

test("ROLE bdfl, and a shell with no ROLE, merge a workflow-touching PR", () => {
  for (const env of [{ ROLE: "bdfl" }, {}]) {
    const { status, stdout, merges } = run(["42"], {
      env,
      responses: [
        files(".github/workflows/agent-bdfl.yml"),
        { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
        merge,
      ],
    });
    assert.equal(status, 0, JSON.stringify(env) + stdout);
    assert.equal(merges.length, 1);
  }
});

test("--wait --gates-only: a failed review on a PR editing agent-review.yml lands on green gates", () => {
  const { status, stdout, merges, calls } = run(GATES_ONLY, {
    responses: [
      files(".github/workflows/agent-review.yml", ".github/scripts/prompt-bytes"),
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "failure"], ...GREEN_GATES)] },
      merge,
    ],
  });
  assert.equal(status, 0, stdout);
  assert.match(
    stdout,
    /no verdict can come on a PR editing \.github\/workflows\/agent-review\.yml; landing by discretion \(--gates-only\)/,
  );
  assert.match(stdout, /merged #42 at abc123456789 as squash1/);
  assert.equal(merges.length, 1);
  assert.ok(!calls.some((c) => c.join(" ").includes("timeline")), "no verdict read under --gates-only");
});

test("--wait --gates-only: a PR that does not edit agent-review.yml is refused before any wait", () => {
  const { status, stderr, merges, calls } = run(GATES_ONLY, {
    responses: [
      files(".github/workflows/agent-bdfl.yml", "agents/GOVERNANCE.md"),
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "failure"], ...GREEN_GATES)] },
      merge,
    ],
  });
  assert.equal(status, 1);
  assert.match(stderr, /--gates-only refused: #42 does not edit \.github\/workflows\/agent-review\.yml/);
  assert.deepEqual(merges, []);
  assert.ok(!calls.some((c) => c.join(" ").includes("check-runs")), "refused before the first poll");
});

test("--wait --gates-only: an unreadable file list is refused, naming it", () => {
  const { status, stderr, merges } = run(GATES_ONLY, {
    responses: [
      {
        match: "^api repos/o/r/pulls/42/files",
        responses: [{ status: 1, stderr: "gh: Server Error (HTTP 502)", stdout: { message: "Server Error" } }],
      },
      merge,
    ],
  });
  assert.equal(status, 1);
  assert.match(stderr, /cannot read #42's file list \(Server Error\); refusing to wait on the gates alone/);
  assert.deepEqual(merges, []);
});

test("--wait --gates-only: a gate still pending keeps waiting, then names it at the deadline", () => {
  const { status, stderr, merges } = run(["42", "--wait", "--gates-only", "--interval", "0", "--timeout", "0"], {
    responses: [
      files(".github/workflows/agent-review.yml"),
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      ruleset,
      {
        match: "check-runs",
        responses: [
          runs(["review", "completed", "failure"], ["fmt", "completed", "success"], ["test", "queued", null]),
        ],
      },
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(stderr, /review failure, verdict labels none, gates pending test/);
  assert.deepEqual(merges, []);
});

test("--wait --gates-only: a red required gate exits 2 at once", () => {
  const { status, stderr, merges } = run(GATES_ONLY, {
    responses: [
      files(".github/workflows/agent-review.yml"),
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      ruleset,
      {
        match: "check-runs",
        responses: [
          runs(["review", "completed", "failure"], ["fmt", "completed", "failure"], ["test", "queued", null]),
        ],
      },
      merge,
    ],
  });
  assert.equal(status, 2);
  assert.match(stderr, /required gates red on abc123456789: fmt/);
  assert.deepEqual(merges, []);
});

test("--gates-only without --wait is a usage error with exit 1", () => {
  const { status, stderr, calls } = run(["42", "--gates-only"]);
  assert.equal(status, 1);
  assert.match(stderr, /usage: merge-pr/);
  assert.deepEqual(calls, []);
});

test("--wait: a review that concluded green without a verdict label exits 4", () => {
  const { status, stderr, merges } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      merge,
    ],
  });
  assert.equal(status, 4);
  assert.match(stderr, /verdict labels none/);
  assert.deepEqual(merges, []);
});

// A read whose head differs from the pin asks the compare API which is the
// ancestor: `behind` is a push past the pin (the pin is the older commit),
// `ahead` is the PR view lagging the push the pin came from (#1429).
const MOVED = "def4567890abcdef1234";
const compare = (base, head, status) => ({
  match: `^api repos/o/r/compare/${base}\\.\\.\\.${head}$`,
  responses: [{ stdout: { status } }],
});

test("--wait: a head that moves between polls exits 1 before any merge", () => {
  const { status, stderr, merges, calls } = run(WAIT, {
    responses: [
      {
        match: "^api repos/o/r/pulls/42$",
        responses: [pr([]), pr([], { head: { sha: MOVED } })],
      },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "in_progress", null], ...GREEN_GATES)] },
      compare(MOVED, HEAD, "behind"),
      merge,
    ],
  });
  assert.equal(status, 1);
  assert.match(stderr, /head moved past the pinned abc123456789 to def4567890ab/);
  assert.deepEqual(merges, []);
  assert.equal(
    calls.filter((c) => c.join(" ").includes("check-runs")).length,
    1,
    "the first poll saw the old head; the second never read its runs",
  );
});

// Issue #1429: `merge-pr --wait --sha <new>` typed within a second of
// push-branch returning <new> read the PR's previous head and died as if
// the head had moved past the pin, when the pin was the newer commit.
const STALE = "0ld0ld0ld0ld0ld0ld0l";
const PINNED = [...WAIT, "--sha", HEAD];

test("--wait --sha: a PR read behind the pin is re-read once, then the wait proceeds", () => {
  const { status, stdout, merges, calls } = run(PINNED, {
    responses: [
      {
        match: "^api repos/o/r/pulls/42$",
        responses: [pr([], { head: { sha: STALE } }), pr(["agent-approved"])],
      },
      ruleset,
      compare(STALE, HEAD, "ahead"),
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      APPROVED_NOW,
      merge,
    ],
  });
  assert.equal(status, 0, stdout);
  assert.match(stdout, /the PR reads 0ld0ld0ld0ld, behind the pinned abc123456789: the read lags the push/);
  assert.match(stdout, /merged #42 at abc123456789 as squash1/);
  assert.equal(merges.length, 1);
  assert.equal(
    calls.filter((c) => c.join(" ").includes("check-runs")).length,
    1,
    "the lagging poll judged nothing; only the caught-up read checked the runs",
  );
});

test("--wait --sha: a read still behind the pin after the retry names the lag, not a move", () => {
  const { status, stderr, merges } = run(PINNED, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([], { head: { sha: STALE } })] },
      ruleset,
      compare(STALE, HEAD, "ahead"),
      merge,
    ],
  });
  assert.equal(status, 1);
  assert.match(stderr, /still reads 0ld0ld0ld0ld, behind the pinned abc123456789/);
  assert.doesNotMatch(stderr, /moved past/);
  assert.deepEqual(merges, []);
});

test("--wait --sha: a read the pin diverged from is a move, dead at once", () => {
  const { status, stderr, merges } = run(PINNED, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([], { head: { sha: STALE } })] },
      ruleset,
      compare(STALE, HEAD, "diverged"),
      merge,
    ],
  });
  assert.equal(status, 1);
  assert.match(stderr, /head moved past the pinned abc123456789 to 0ld0ld0ld0ld/);
  assert.deepEqual(merges, []);
});

test("--wait: a PR merged by the reviewer during the wait is success", () => {
  const { status, stdout, merges } = run(WAIT, {
    responses: [
      {
        match: "^api repos/o/r/pulls/42$",
        responses: [pr([]), pr([], { state: "closed", merged: true, merge_commit_sha: "squash1" })],
      },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "in_progress", null], ...GREEN_GATES)] },
      merge,
    ],
  });
  assert.equal(status, 0);
  assert.match(stdout, /already merged as squash1/);
  assert.deepEqual(merges, []);
});

test("--timeout without --wait is a usage error with exit 1, never exit 2", () => {
  const { status, stderr, calls } = run(["42", "--timeout", "5"]);
  assert.equal(status, 1);
  assert.match(stderr, /usage: merge-pr/);
  assert.deepEqual(calls, []);
});

// Exit 2 arms the pinned auto-merge itself (#1026): thirteen of fourteen
// landings on 2026-10-10 typed `gh pr merge --auto` by hand after it, and
// the porcelain pins no head. The PUT's stderr carries the status, the way
// gh reports it; the stub cannot speak HTTP.
const unmet = {
  match: "PUT repos/o/r/pulls/42/merge",
  responses: [
    {
      status: 1,
      stdout: { message: "Required status check \"test\" is expected." },
      stderr: "gh: Required status check (HTTP 405)",
    },
  ],
};
const armed = {
  match: "^api graphql",
  responses: [
    {
      stdout: {
        data: {
          enablePullRequestAutoMerge: { pullRequest: { autoMergeRequest: { enabledAt: "2026-10-11T00:00:00Z" } } },
        },
      },
    },
  ],
};

test("a 405 arms auto-merge on the pinned head and exits 2", () => {
  const { status, calls, stderr } = run(["42", "--sha", HEAD], {
    responses: [{ match: "^api repos/o/r/pulls/42$", responses: [pr([])] }, unmet, armed],
  });
  assert.equal(status, 2, stderr);
  const arm = calls.find((argv) => argv[1] === "graphql");
  assert.ok(arm, "the mutation was sent");
  assert.ok(arm.includes(`oid=${HEAD}`), "pinned to the reviewed head");
  assert.ok(arm.includes("id=PR_node42"));
  assert.match(stderr, /auto-merge armed on abc123456789 alone/);
});

// Issue #1417: GitHub arms nothing on a mergeable PR, and the last gate can
// go green in the seconds between the attempt and the arm; #1413 sat approved
// for four hours on that race. A refusal is answered with one more attempt on
// the same pin, whatever the refusal's words, because the pin keeps a moved
// head out (409) and unmet gates come back as a 405 the caller can read.
const refused = {
  match: "^api graphql",
  responses: [
    {
      status: 1,
      stdout: { errors: [{ message: "Pull request is in clean status" }] },
      stderr: "gh: Pull request is in clean status (enablePullRequestAutoMerge)",
    },
  ],
};
const merged = { stdout: { sha: "squash0123456789", merged: true, message: "Pull Request successfully merged" } };

test("a 405, a refused arm, then a 200: the race lands on the second attempt", () => {
  const { status, merges, stdout, stderr } = run(["42", "--sha", HEAD], {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      { match: "PUT repos/o/r/pulls/42/merge", responses: [unmet.responses[0], merged] },
      refused,
    ],
  });
  assert.equal(status, 0, stdout + stderr);
  assert.equal(merges.length, 2, "one attempt before the arm, one after the refusal");
  assert.ok(merges.every((argv) => argv.includes("--input")), "both attempts carry the pinned body");
  assert.match(stdout, /refused to arm auto-merge on abc123456789 \(.*Pull request is in clean status/);
  assert.match(stdout, /merged #42 at abc123456789 as squash0123456789 on the session token, second attempt/);
});

test("a 405 whose arming GitHub refuses, then a 405 again, exits 1 with both messages and nothing armed", () => {
  const { status, merges, stderr } = run(["42", "--sha", HEAD], {
    responses: [{ match: "^api repos/o/r/pulls/42$", responses: [pr([])] }, unmet, refused],
  });
  assert.equal(status, 1, stderr);
  assert.equal(merges.length, 2, "the refusal buys exactly one more attempt");
  assert.match(stderr, /nothing is armed/);
  assert.match(stderr, /Pull request is in clean status/);
  assert.match(stderr, /Required status check "test" is expected/);
});

test("a 405, a refused arm, then a 409: the moved head exits 1 and nothing lands", () => {
  const moved = {
    status: 1,
    stdout: { message: "Head branch was modified. Review and try the merge again." },
    stderr: "gh: Head branch was modified (HTTP 409)",
  };
  const { status, merges, stderr } = run(["42", "--sha", HEAD], {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      { match: "PUT repos/o/r/pulls/42/merge", responses: [unmet.responses[0], moved] },
      refused,
    ],
  });
  assert.equal(status, 1, stderr);
  assert.equal(merges.length, 2);
  assert.match(stderr, /head moved past the pinned abc123456789/);
});

// 2026-10-24: `--sha a67fe4b7dc56`, the twelve characters every state and
// retrospect line prints, died on #1573 as "head moved past the pinned
// a67fe4b7dc56 to a67fe4b7dc56". A prefix of the head is the head.
test("--sha a prefix of the head pins the full head: the arm carries the whole oid", () => {
  const { status, calls, stderr } = run(["42", "--sha", HEAD.slice(0, 12)], {
    responses: [{ match: "^api repos/o/r/pulls/42$", responses: [pr([])] }, unmet, armed],
  });
  assert.equal(status, 2, stderr);
  const arm = calls.find((argv) => argv[1] === "graphql");
  assert.ok(arm, "the mutation was sent");
  assert.ok(arm.includes(`oid=${HEAD}`), "pinned to the full head, not the typed prefix");
});

test("--wait --sha a prefix of the head is not a move; the wait merges at it", () => {
  const { status, stdout, stderr, merges } = run([...WAIT, "--sha", HEAD.slice(0, 12)], {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      APPROVED_NOW,
      merge,
    ],
  });
  assert.equal(status, 0, stdout + stderr);
  assert.doesNotMatch(stderr, /moved past/);
  assert.match(stdout, /merged #42 at abc123456789 as squash1/);
  assert.equal(merges.length, 1);
});

test("--sha that is not a commit sha is refused before any read", () => {
  const { status, calls, stderr } = run(["42", "--sha", "abc"]);
  assert.equal(status, 1);
  assert.deepEqual(calls, []);
  assert.match(stderr, /--sha 'abc' is not a commit sha/);
});

test("--wait never arms: a 405 after green gates exits 2 with nothing armed", () => {
  const { status, calls, stderr } = run(WAIT, {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr(["agent-approved"])] },
      ruleset,
      { match: "check-runs", responses: [runs(["review", "completed", "success"], ...GREEN_GATES)] },
      APPROVED_NOW,
      unmet,
      armed,
    ],
  });
  assert.equal(status, 2, stderr);
  assert.ok(!calls.some((argv) => argv[1] === "graphql"), "no mutation under --wait");
  assert.match(stderr, /nothing armed/);
});
