// merge-pr's PR_NUMBER scope guard (issue #771, closing PR #772) and its
// `--wait` path (issue #842). The stub `gh` below answers from a script of
// responses keyed by a regex over the argv it was called with, each key a
// sequence consumed call by call with the last answer repeating, so a poll
// that must see "in progress, then concluded" has something to see. With no
// script every call fails loudly, so the guard tests prove only that the
// guard did or did not let the call through. The unscripted merge paths
// (403 escalation, 405, 409) still have no coverage here.
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
    check_runs: entries.map(([name, status, conclusion]) => ({
      name,
      status,
      conclusion,
      started_at: "2026-10-09T08:00:00Z",
    })),
  },
});
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
      merge,
    ],
  });
  assert.equal(status, 0, stdout);
  assert.match(stdout, /approved at abc123456789, every required gate green/);
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

test("--wait: the head moving past the pinned SHA exits 1 before any merge", () => {
  const { status, stderr, merges } = run([...WAIT, "--sha", "0000000000000000"], {
    responses: [
      { match: "^api repos/o/r/pulls/42$", responses: [pr([])] },
      ruleset,
      merge,
    ],
  });
  assert.equal(status, 1);
  assert.match(stderr, /head moved past the pinned 000000000000 to abc123456789/);
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
