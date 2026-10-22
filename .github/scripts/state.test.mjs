// Fixtures for state, the BDFL's state read that replaced eight gh calls by hand.
//
// The first assertion is the one the script exists for: a fact that cannot be
// read prints UNREADABLE and never contributes a zero to the footer. A false
// empty tells a wake there is nothing to do, and the prompt answers nothing
// by stopping; that is how a red check sits unread for a week (#1058).
//
// render() and the row builders are pure by construction (no network, no
// clock: `now` is an argument) so these can exist. The end-to-end tests put a
// stub `gh` on PATH that answers from a table keyed by a substring of argv,
// the same seam gh-comment.test.mjs uses, so the per-workflow read of the
// checks is exercised as a whole and not only its renderer.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = join(scriptsDir, "state");

const driver = `
import importlib.machinery, importlib.util, io, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("state", sys.argv[1] + "/state")
spec = importlib.util.spec_from_loader("state", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
if fn == "render":
    buf = io.StringIO()
    mod.render(raw, out=buf)
    print(json.dumps(buf.getvalue()))
else:
    print(json.dumps(getattr(mod, fn)(*raw)))
`;

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const check = (over = {}) => ({
  name: "monster",
  conclusion: "success",
  age: 1,
  created: "2026-10-10",
  event: "schedule",
  url: "https://github.com/o/r/actions/runs/1",
  cited: null,
  error: null,
  ...over,
});

const facts = (over = {}) => ({
  checks: { error: null, rows: [check()], unfiled: 0 },
  prs: { error: null, rows: [] },
  issues: { error: null, open: 0, counts: { product: 0, "agent-system": 0 }, unrouted: [] },
  ops_log: { error: null, total: 0, rows: [] },
  bench: { error: null, line: "fresh: the reports are at or after the newest report-affecting commit." },
  commits: { error: null, since: "2026-10-15T20:11:13Z", run: 1, truncated: false, rows: [] },
  ...over,
});

test("an unreadable fact prints UNREADABLE and never a zero in the footer", () => {
  const out = call("render", facts({ checks: { error: "HTTP 403", rows: [], unfiled: 0 } }));
  assert.match(out, /^checks: UNREADABLE \(HTTP 403\)/m);
  assert.match(out, /state: checks unreadable \|/);
  assert.doesNotMatch(out, /checks 0/);
});

test("every fact can be unreadable at once, and the footer says so six times", () => {
  const out = call(
    "render",
    facts({
      checks: { error: "x", rows: [], unfiled: 0 },
      prs: { error: "x", rows: [] },
      issues: { error: "x", open: 0, counts: {}, unrouted: [] },
      ops_log: { error: "x", total: 0, rows: [] },
      commits: { error: "no anchor", since: null, run: null, rows: [] },
    }),
  );
  assert.equal((out.match(/unreadable/g) || []).length, 5, out);
  assert.match(out, /commits unreadable since no anchor/);
});

test("a red nobody filed says unfiled and the footer counts it", () => {
  const out = call(
    "render",
    facts({
      checks: {
        error: null,
        rows: [check({ conclusion: "failure", age: 6, cited: [] })],
        unfiled: 1,
      },
    }),
  );
  assert.match(out, /monster  failure  6d \(2026-10-10, schedule\)  unfiled: a bug issue this run/);
  assert.match(out, /https:\/\/github.com\/o\/r\/actions\/runs\/1/);
  assert.match(out, /checks 1 \(red 1, unfiled 1, unreadable 0\)/);
});

test("a red whose only citing issue is closed waits on the next cron", () => {
  const out = call(
    "render",
    facts({
      checks: {
        error: null,
        rows: [check({ conclusion: "failure", cited: ["#1013 closed 2026-10-10"] })],
        unfiled: 0,
      },
    }),
  );
  assert.match(out, /#1013 closed 2026-10-10: fix landed, the next cron tells/);
  assert.match(out, /checks 1 \(red 1, unfiled 0, unreadable 0\)/);
});

test("a red with an open citing issue is filed, nothing more to say", () => {
  const out = call(
    "render",
    facts({
      checks: {
        error: null,
        rows: [check({ conclusion: "failure", cited: ["#1058 open", "#1013 closed 2026-10-10"] })],
        unfiled: 0,
      },
    }),
  );
  assert.match(out, /#1058 open, #1013 closed 2026-10-10\n/);
  assert.doesNotMatch(out, /next cron tells/);
});

test("a green board is one line, oldest newest-run first", () => {
  const out = call(
    "render",
    facts({
      checks: {
        error: null,
        rows: [
          check({ name: "ci", age: 0 }),
          check({ name: "monster", age: 6 }),
          check({ name: "fuzz-check", age: 1 }),
        ],
        unfiled: 0,
      },
    }),
  );
  assert.match(out, /checks: 3 watched, 3 green on main\n    green  monster 6d, fuzz-check 1d, ci 0d\n/);
});

test("a check with no concluded run and one that cannot be read each get their own line", () => {
  const out = call(
    "render",
    facts({
      checks: {
        error: null,
        rows: [
          check({ conclusion: null, age: null }),
          check({ name: "gone", conclusion: null, error: "no workflow of that name" }),
        ],
        unfiled: 0,
      },
    }),
  );
  assert.match(out, /monster  no concluded run among the newest 100 on main/);
  assert.match(out, /gone  UNREADABLE \(no workflow of that name\)/);
  assert.match(out, /checks 2 \(red 0, unfiled 0, unreadable 1\)/);
});

test("a red whose citations cannot be read is counted unreadable, not filed", () => {
  const out = call(
    "render",
    facts({
      checks: { error: null, rows: [check({ conclusion: "failure", cited: null })], unfiled: 0 },
    }),
  );
  assert.match(out, /citations UNREADABLE/);
  assert.match(out, /checks 1 \(red 1, unfiled 0, unreadable 1\)/);
});

test("a full commits page prints 100+, never a count that looks whole", () => {
  const out = call(
    "render",
    facts({
      commits: {
        error: null,
        since: "2026-10-12T00:00:00Z",
        run: null,
        truncated: true,
        rows: Array.from({ length: 100 }, (_, i) => ({ sha: `c${i}`, subject: "s" })),
      },
    }),
  );
  assert.match(out, /commits: 100\+ on main/);
  assert.match(out, /\| commits 100\+ since/);
});

test("PRs print verdict, draft and age, oldest first", () => {
  const pr = (over) => ({
    number: 1,
    age: 0,
    verdict: "no-verdict",
    draft: false,
    author: "bot",
    branch: "claude/x",
    title: "x: y",
    ...over,
  });
  const out = call(
    "render",
    facts({
      prs: {
        error: null,
        rows: [pr({ number: 2, verdict: "agent-approved" }), pr({ number: 3, age: 4, draft: true })],
      },
    }),
  );
  assert.match(
    out,
    /prs: 2 open\n    #3  4d  no-verdict  DRAFT  bot  claude\/x :: x: y\n    #2  0d  agent-approved  bot/,
  );
  assert.match(out, /\| prs 2 \|/);
});

test("an unrouted issue prints before every count", () => {
  const out = call(
    "render",
    facts({
      issues: {
        error: null,
        open: 3,
        counts: { product: 1, "agent-system": 1, "ready-to-ship": 1 },
        unrouted: [{ number: 9, title: "nobody's" }],
      },
    }),
  );
  assert.match(
    out,
    /issues: 3 open \| unrouted 1 \| product 1 \| agent-system 1 \| ready-to-ship 1\n    unrouted  #9  nobody's/,
  );
  assert.match(out, /issues 3 \(unrouted 1\)/);
});

test("the ops log prints seat and first line, the commits sha and subject", () => {
  const out = call(
    "render",
    facts({
      ops_log: {
        error: null,
        total: 764,
        rows: [{ created: "2026-10-15T22:18:37Z", seat: "maintainer", line: "PR #1221 pushed" }],
      },
      commits: {
        error: null,
        since: "2026-10-15T20:11:13Z",
        run: 42,
        truncated: false,
        rows: [{ sha: "dd06127", subject: "filters: wire reverse" }],
      },
    }),
  );
  assert.match(out, /ops-log: #3, last 1 of 764\n    2026-10-15T22:18:37Z  maintainer  PR #1221 pushed/);
  assert.match(out, /commits: 1 on main since 2026-10-15T20:11:13Z \(run 42\)\n    dd06127  filters: wire reverse/);
});

test("newest_run judges main only, skips skipped and cancelled, and sorts rather than trusting the page", () => {
  const run = (created, conclusion, head_branch = "main") => ({ created_at: created, conclusion, head_branch });
  const got = call("newest_run", [[
    run("2026-10-01T00:00:00Z", "success"),
    run("2026-10-11T00:00:00Z", "failure", "claude/x"),
    run("2026-10-10T00:00:00Z", "skipped"),
    run("2026-10-09T00:00:00Z", "cancelled"),
    run("2026-10-08T00:00:00Z", "failure"),
  ]]);
  assert.equal(got.created_at, "2026-10-08T00:00:00Z");
  assert.equal(call("newest_run", [[run("2026-10-10T00:00:00Z", "skipped")]]), null);
  assert.equal(call("newest_run", [[run("2026-10-10T00:00:00Z", "failure", "claude/x")]]), null);
});

test("cite lists open issues before closed ones, newest first within each", () => {
  const got = call("cite", [[
    { number: 1013, state: "closed", closed_at: "2026-10-10T12:23:33Z" },
    { number: 980, state: "open" },
    { number: 1069, state: "open" },
  ]]);
  assert.deepEqual(got, ["#1069 open", "#980 open", "#1013 closed 2026-10-10"]);
});

// The title monster-report.mjs files is the one the filter must keep, and
// GitHub's search folds case, so the filter does (#1464).
test("names_workflow wants the name whole, in any case: deny-ci-background names no ci", () => {
  assert.equal(call("names_workflow", ["🐛 Monster CI failure", "monster"]), true);
  assert.equal(call("names_workflow", ["CI red on main", "ci"]), true);
  assert.equal(call("names_workflow", ["CodeQL analysis red", "codeql"]), true);
  assert.equal(call("names_workflow", ["ci: worker red on main", "ci"]), true);
  assert.equal(call("names_workflow", ["`ci` red at 7c97e01", "ci"]), true);
  assert.equal(call("names_workflow", ["deny-ci-background and deny-review-recompute match prose", "ci"]), false);
  assert.equal(call("names_workflow", ["agent-ci-x red", "ci"]), false);
  assert.equal(call("names_workflow", ["cia", "ci"]), false);
  assert.equal(call("names_workflow", [null, "ci"]), false);
});

test("seat_of reads the role footer and falls back to the login", () => {
  const footer = "Shipped it.\n\n---\n_maintainer · [Claude Code](https://claude.ai/code)_";
  assert.equal(call("seat_of", [{ body: footer, user: { login: "github-actions" } }]), "maintainer");
  assert.equal(call("seat_of", [{ body: "plain", user: { login: "bugabinga" } }]), "bugabinga");
});

// A stub gh answering from a table: the first needle contained in the joined
// argv wins; no match exits 1 with the argv on stderr, which the script must
// render as UNREADABLE for that fact and nothing else.
function withGh(answers, args, env = {}) {
  const dir = mkdtempSync(join(tmpdir(), "state-"));
  const table = join(dir, "answers.json");
  writeFileSync(table, JSON.stringify(answers));
  const stub = join(dir, "stub.mjs");
  writeFileSync(
    stub,
    `import { readFileSync } from "node:fs";
const answers = JSON.parse(readFileSync(${JSON.stringify(table)}, "utf8"));
const argv = process.argv.slice(2).join(" ");
const hit = Object.entries(answers).find(([needle]) => argv.includes(needle));
if (!hit) { console.error("stub gh: no answer for " + argv); process.exit(1); }
console.log(JSON.stringify(hit[1]));
`,
  );
  writeFileSync(join(dir, "gh"), `#!/bin/sh\nexec node ${stub} "$@"\n`);
  chmodSync(join(dir, "gh"), 0o755);
  const alarm = join(dir, "alarm.yml");
  writeFileSync(
    alarm,
    "name: agent-alarm\non:\n  workflow_run:\n    workflows:\n      - monster\n      - ci\n    types: [completed]\n",
  );
  return spawnSync(script, ["--alarm", alarm, "--repo", "o/r", ...args], {
    encoding: "utf8",
    env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, ...env },
  });
}

const ISSUE = { number: 1, title: "t", labels: [{ name: "product" }] };
const GREEN = {
  id: 5,
  created_at: "2026-10-15T22:00:00Z",
  conclusion: "success",
  event: "schedule",
  head_branch: "main",
  html_url: "https://github.com/o/r/actions/runs/5",
};

test("end to end: the checks are read per workflow, a red is cited, the rest counted", () => {
  const run = (id, created, conclusion, event = "schedule") => ({
    id,
    created_at: created,
    conclusion,
    event,
    head_branch: "main",
    html_url: `https://github.com/o/r/actions/runs/${id}`,
  });
  const proc = withGh(
    {
      "actions/workflows?": { workflows: [{ name: "monster", id: 11 }, { name: "ci", id: 12 }] },
      "workflows/11/runs": {
        workflow_runs: [run(1, "2026-10-10T09:46:00Z", "failure"), run(0, "2026-10-03T09:00:00Z", "success")],
      },
      "workflows/12/runs": {
        workflow_runs: [
          run(3, "2026-10-15T22:26:11Z", "skipped", "push"),
          run(2, "2026-10-15T22:00:00Z", "success", "push"),
        ],
      },
      "search/issues": {
        items: [{ number: 1013, title: "🐛 Monster CI failure", state: "closed", closed_at: "2026-10-10T12:23:33Z" }],
      },
      "pr list": [{
        number: 7,
        title: "a: b",
        labels: [{ name: "agent-approved" }],
        createdAt: "2026-10-15T00:00:00Z",
        isDraft: false,
        headRefName: "claude/a",
        author: { login: "bot" },
      }],
      "issue list": [ISSUE, { number: 2, title: "nobody's", labels: [] }],
      "issues/3/comments?per_page=100&page=1": [{
        created_at: "2026-10-15T20:00:00Z",
        body: "old\n\n---\n_bdfl · [Claude Code](https://claude.ai/code)_",
        user: { login: "github-actions" },
      }],
      "issues/3/comments?per_page=100&page=2": [{
        created_at: "2026-10-15T22:00:00Z",
        body: "newest line\nmore",
        user: { login: "bugabinga" },
      }],
      "issues/3": { comments: 101 },
      "commits?sha=main&since=2026-10-15T20:11:13Z": [{
        sha: "dd06127abc",
        commit: { message: "filters: wire reverse\n\nbody" },
      }],
    },
    ["--since", "2026-10-15T20:11:13Z"],
  );
  assert.equal(proc.status, 0, proc.stderr);
  const out = proc.stdout;
  assert.match(
    out,
    /checks: 2 watched, 1 green on main\n    green  ci \d+d\n    monster  failure  \d+d \(2026-10-10, schedule\)  #1013 closed 2026-10-10: fix landed, the next cron tells/,
  );
  assert.match(out, /prs: 1 open\n    #7  \d+d  agent-approved  bot  claude\/a :: a: b/);
  assert.match(
    out,
    /issues: 2 open \| unrouted 1 \| product 1 \| agent-system 0 \| marketing 0 \| ready-to-ship 0 \| blocked-on-human 0 \| ledger 0 \| bug 0\n    unrouted  #2  nobody's/,
  );
  // Two pages, sorted by time: the bdfl footer names the seat, the bare comment its login.
  assert.match(
    out,
    /ops-log: #3, last 2 of 101\n    2026-10-15T20:00:00Z  bdfl  old\n    2026-10-15T22:00:00Z  bugabinga  newest line/,
  );
  assert.match(out, /commits: 1 on main since 2026-10-15T20:11:13Z\n    dd06127  filters: wire reverse/);
  assert.match(
    out,
    /state: checks 2 \(red 1, unfiled 0, unreadable 0\) \| prs 1 \| issues 2 \(unrouted 1\) \| ops-log #3 \| commits 1 since 2026-10-15T20:11:13Z/,
  );
});

test("end to end: a gh that answers nothing leaves every fact UNREADABLE and exits 0", () => {
  const proc = withGh({}, ["--since", "2026-10-15T20:11:13Z"]);
  assert.equal(proc.status, 0, proc.stderr);
  assert.equal((proc.stdout.match(/UNREADABLE/g) || []).length, 6, proc.stdout);
  assert.match(
    proc.stdout,
    /state: checks unreadable \| prs unreadable \| issues unreadable \| ops-log unreadable \| commits unreadable[^|]*\| bench-report unreadable/,
  );
});

test("end to end: one workflow's runs unreadable does not take the board down", () => {
  const proc = withGh(
    {
      "actions/workflows?": { workflows: [{ name: "monster", id: 11 }, { name: "ci", id: 12 }] },
      "workflows/12/runs": {
        workflow_runs: [{
          id: 2,
          created_at: "2026-10-15T22:00:00Z",
          conclusion: "success",
          event: "push",
          head_branch: "main",
          html_url: "u",
        }],
      },
      "pr list": [],
      "issue list": [ISSUE],
      "issues/3/comments": [],
      "issues/3": { comments: 0 },
      "commits?": [],
    },
    ["--since", "2026-10-15T20:11:13Z"],
  );
  assert.equal(proc.status, 0, proc.stderr);
  assert.match(
    proc.stdout,
    /checks: 2 watched, 1 green on main\n    green  ci \d+d\n    monster  UNREADABLE \(stub gh: no answer for/,
  );
  assert.match(
    proc.stdout,
    /state: checks 2 \(red 0, unfiled 0, unreadable 1\) | prs 0 \| issues 1 \(unrouted 0\) \| ops-log #3 \| commits 0/,
  );
});

// The stub answers the first needle contained in argv, so order is the test:
// the `label:bug` row is the only one a bug-scoped query reaches, and the
// bare `search/issues` row is what a title-only query would have returned.
test("end to end: a feature issue titled with the workflow's name does not file its red", () => {
  const run = {
    id: 1,
    created_at: "2026-10-10T09:46:00Z",
    conclusion: "failure",
    event: "schedule",
    head_branch: "main",
    html_url: "https://github.com/o/r/actions/runs/1",
  };
  const proc = withGh(
    {
      "actions/workflows?": { workflows: [{ name: "monster", id: 11 }, { name: "ci", id: 12 }] },
      "workflows/11/runs": { workflow_runs: [run] },
      "workflows/12/runs": { workflow_runs: [GREEN] },
      "label:bug": { items: [] },
      "search/issues": { items: [{ number: 980, state: "open" }] },
      "pr list": [],
      "issue list": [ISSUE],
      "issues/3/comments": [],
      "issues/3": { comments: 0 },
      "commits?": [],
    },
    ["--since", "2026-10-15T20:11:13Z"],
  );
  assert.equal(proc.status, 0, proc.stderr);
  assert.match(proc.stdout, /monster  failure  \d+d \(2026-10-10, schedule\)  unfiled: a bug issue this run/);
  assert.doesNotMatch(proc.stdout, /#980/);
  assert.match(proc.stdout, /checks 2 \(red 1, unfiled 1, unreadable 0\)/);
});

// GitHub tokenises `deny-ci-background` into a hit for `in:title ci`; the
// red read as filed by an issue about a hook, and the footer said unfiled 0
// on a red nobody had filed (#1457). The filter is on the title, in Python.
test("end to end: a bug titled with a hyphenated word holding the name does not file the red", () => {
  const run = {
    id: 1,
    created_at: "2026-10-20T09:46:00Z",
    conclusion: "failure",
    event: "push",
    head_branch: "main",
    html_url: "https://github.com/o/r/actions/runs/1",
  };
  const hook = { number: 1377, title: "deny-ci-background and deny-review-recompute match prose", state: "open" };
  const red = { number: 1454, title: "worker red on main: ci fails at 7c97e01", state: "open" };
  const board = (items) =>
    withGh(
      {
        "actions/workflows?": { workflows: [{ name: "monster", id: 11 }, { name: "ci", id: 12 }] },
        "workflows/11/runs": { workflow_runs: [GREEN] },
        "workflows/12/runs": { workflow_runs: [run] },
        "label:bug": { items },
        "pr list": [],
        "issue list": [ISSUE],
        "issues/3/comments": [],
        "issues/3": { comments: 0 },
        "commits?": [],
      },
      ["--since", "2026-10-15T20:11:13Z"],
    );
  const only = board([hook]);
  assert.equal(only.status, 0, only.stderr);
  assert.match(only.stdout, /ci  failure  \d+d \(2026-10-20, push\)  unfiled: a bug issue this run/);
  assert.doesNotMatch(only.stdout, /#1377/);
  assert.match(only.stdout, /checks 2 \(red 1, unfiled 1, unreadable 0\)/);
  const both = board([hook, red]);
  assert.equal(both.status, 0, both.stderr);
  assert.match(both.stdout, /ci  failure  \d+d \(2026-10-20, push\)  #1454 open/);
  assert.doesNotMatch(both.stdout, /#1377/);
  assert.match(both.stdout, /checks 2 \(red 1, unfiled 0, unreadable 0\)/);
});

test("end to end: a PR-branch failure is never read for a workflow with no run on main", () => {
  const failure = {
    id: 9,
    created_at: "2026-10-15T20:37:50Z",
    conclusion: "failure",
    event: "pull_request",
    html_url: "https://github.com/o/r/actions/runs/9",
    head_branch: "claude/x",
  };
  const proc = withGh(
    {
      "actions/workflows?": { workflows: [{ name: "monster", id: 11 }, { name: "ci", id: 12 }] },
      "workflows/11/runs": { workflow_runs: [failure] },
      "workflows/12/runs": { workflow_runs: [failure] },
      "pr list": [],
      "issue list": [ISSUE],
      "issues/3/comments": [],
      "issues/3": { comments: 0 },
      "commits?": [],
    },
    ["--since", "2026-10-15T20:11:13Z"],
  );
  assert.equal(proc.status, 0, proc.stderr);
  assert.match(proc.stdout, /monster  no concluded run among the newest 100 on main/);
  assert.doesNotMatch(proc.stdout, /unfiled:|claude\/x/);
  assert.match(proc.stdout, /checks 2 \(red 0, unfiled 0, unreadable 0\)/);
});

// While Actions was degraded on 2026-10-22 the per-workflow page under
// `branch=main` came back `total_count 0`, empty, for seventeen of eighteen
// watched workflows, and the board read red 0 (run 39811812824). A watched
// workflow has run, so the page is read across branches and an empty one is
// a read that failed.
test("end to end: an empty runs page is UNREADABLE and counted, never a no-run", () => {
  const proc = withGh(
    {
      "actions/workflows?": { workflows: [{ name: "monster", id: 11 }, { name: "ci", id: 12 }] },
      "workflows/11/runs": { total_count: 0, workflow_runs: [] },
      "workflows/12/runs": { workflow_runs: [GREEN] },
      "pr list": [],
      "issue list": [ISSUE],
      "issues/3/comments": [],
      "issues/3": { comments: 0 },
      "commits?": [],
    },
    ["--since", "2026-10-15T20:11:13Z"],
  );
  assert.equal(proc.status, 0, proc.stderr);
  assert.match(proc.stdout, /monster  UNREADABLE \(no runs on any branch: a page the Actions API served empty\)/);
  assert.doesNotMatch(proc.stdout, /no concluded run/);
  assert.match(proc.stdout, /checks 2 \(red 0, unfiled 0, unreadable 1\)/);
});
