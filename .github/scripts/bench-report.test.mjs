// Fixtures for bench-report, the detector the benchmark pass never had.
//
// The first assertion is the one the script exists for: reports older than
// the report binaries that print them never read `fresh`. Four slices of the
// competitor frontier (#1356, #1367, #1373, #1376) landed on main with the
// committed reports untouched and nothing to say so; two maintainer runs
// named the dispatch a blocker for want of a token the seat held.
//
// The second is the reviewer's sequence on #1378: a publish PR for pass P
// merges after a later report-affecting commit B lands, so the reports are
// newer than B by date while B's pass is unpublished. The generator a
// publish commit names in its subject, not the commit's date, decides.
//
// `decide`, `generator` and `render` are pure by construction (no network:
// `read` hands them facts) so these can exist. The end-to-end test puts a
// stub `gh` on PATH that answers the reads from a table and writes the
// artifact's five files for `run download`, the seam state.test.mjs uses,
// so the gh argument shapes and the copy into the tree are exercised, not
// only the renderer.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = join(scriptsDir, "bench-report");

const driver = `
import importlib.machinery, importlib.util, io, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("bench_report", sys.argv[1] + "/bench-report")
spec = importlib.util.spec_from_loader("bench_report", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
if fn == "render":
    buf = io.StringIO()
    state = mod.render(raw["facts"], raw["error"], out=buf)
    print(json.dumps([state, buf.getvalue()]))
elif fn == "generator":
    print(json.dumps(mod.generator(raw)))
else:
    print(json.dumps(list(mod.decide(**raw))))
`;

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const AFFECTING = { sha: "a23a7b0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", date: "2026-10-19T03:38:22Z", path: "bench/src" };
const REPORTS = {
  sha: "c71bd43bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  date: "2026-10-18T12:32:03Z",
  path: "docs/benchmarks/canterbury.md",
  subject: "bench: publish the finals reports timed one file at a time (#1349)",
};
const STALE = { sha: REPORTS.sha, date: REPORTS.date };
const FRESH = { sha: "f0e0d0cccccccccccccccccccccccccccccccccc", date: "2026-10-19T09:00:00Z" };

const pass = (over = {}) => ({
  databaseId: 42,
  status: "completed",
  conclusion: "success",
  headSha: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
  createdAt: "2026-10-19T04:00:00Z",
  updatedAt: "2026-10-19T05:30:00Z",
  url: "https://github.com/o/r/actions/runs/42",
  ...over,
});

const decision = (over = {}) => ({
  affecting: AFFECTING,
  generated: STALE,
  newest: null,
  head: null,
  live: null,
  ...over,
});
const facts = (over = {}) => ({ reports: REPORTS, ...decision(over) });

test("stale reports with no pass on main answer dispatch, never fresh", () => {
  const [state, reason] = call("decide", decision());
  assert.equal(state, "dispatch");
  assert.match(reason, /no pass on main at or after it/);
});

test("reports generated after the report-affecting commit are fresh", () => {
  assert.deepEqual(call("decide", decision({ generated: FRESH })), ["fresh", null]);
});

test("one commit touching both the binaries and the reports is fresh", () => {
  assert.deepEqual(call("decide", decision({ generated: { sha: AFFECTING.sha, date: AFFECTING.date } })), [
    "fresh",
    null,
  ]);
});

test("the reviewer's sequence: a publish merged after a later bench commit is not fresh", () => {
  // Pass P at 01:00Z generated the reports; B landed under bench/src at
  // 02:00Z; P's publish PR merged at 03:00Z. By the reports commit's date the
  // reports are newer than B; by their generator they predate it.
  const B = { ...AFFECTING, date: "2026-10-19T02:00:00Z" };
  const P = { sha: "0ddba11000000000000000000000000000000000", date: "2026-10-19T01:00:00Z" };
  assert.deepEqual(call("decide", decision({ affecting: B, generated: P })), [
    "dispatch",
    "no pass on main at or after it",
  ]);
  const running = pass({ status: "in_progress", conclusion: null, createdAt: "2026-10-19T02:01:00Z" });
  assert.deepEqual(call("decide", decision({ affecting: B, generated: P, newest: running, head: B.date })), [
    "running",
    null,
  ]);
});

test("generator reads the pass head from the publish subject this script dictates, and nothing else", () => {
  assert.equal(call("generator", "bench: publish the reports regenerated at deadbee"), "deadbee");
  assert.equal(
    call("generator", "bench: publish the reports regenerated at deadbeefdeadbeefdeadbeefdeadbeefdeadbeef (#1400)"),
    "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
  );
  assert.equal(call("generator", REPORTS.subject), null);
  assert.equal(call("generator", "lz: regenerated at a glance, the parse"), null);
  assert.equal(call("generator", ""), null);
});

test("a pass whose head predates the affecting commit does not cover it", () => {
  const [state, reason] = call("decide", decision({ newest: pass(), head: "2026-10-18T04:00:00Z", live: true }));
  assert.equal(state, "dispatch");
  assert.match(reason, /no pass on main at or after it/);
});

test("a covering pass still in progress is running", () => {
  const facts_ = decision({ newest: pass({ status: "in_progress", conclusion: null }), head: "2026-10-19T04:00:00Z" });
  assert.deepEqual(call("decide", facts_), ["running", null]);
});

test("a covering pass that succeeded with a live artifact is publish", () => {
  assert.deepEqual(call("decide", decision({ newest: pass(), head: "2026-10-19T04:00:00Z", live: true })), [
    "publish",
    null,
  ]);
});

test("a covering pass whose artifact expired is dispatch, naming the expiry", () => {
  const [state, reason] = call("decide", decision({ newest: pass(), head: "2026-10-19T04:00:00Z", live: false }));
  assert.equal(state, "dispatch");
  assert.match(reason, /42, succeeded but its artifact expired/);
});

test("a covering pass that failed or was cancelled is dispatch, naming the conclusion", () => {
  for (const conclusion of ["failure", "cancelled", "timed_out"]) {
    const [state, reason] = call("decide", decision({ newest: pass({ conclusion }), head: "2026-10-19T04:00:00Z" }));
    assert.equal(state, "dispatch", conclusion);
    assert.match(reason, new RegExp(`concluded ${conclusion}`));
  }
});

test("render leads with the state word, dictates the publish title, and ends with the three facts", () => {
  const [state, out] = call("render", {
    facts: facts({ newest: pass(), head: "2026-10-19T04:00:00Z", live: true }),
    error: null,
  });
  assert.equal(state, "publish");
  const lines = out.trim().split("\n");
  assert.match(lines[0], /^publish 42: /);
  assert.match(out, /bench-report --publish/);
  // The quoters ride in the same diff: a publish PR of the five files alone
  // fails `test`, because tests/claims.rs pins README.md and site/index.html
  // to the reports' timing columns (#1384, PR #1383's first round).
  assert.match(out, /`cargo x check`: tests\/claims\.rs pins README\.md and site\/index\.html/);
  assert.match(out, /"bench: publish the reports regenerated at deadbee"/);
  assert.match(
    lines.at(-1),
    /^bench-report: publish \| affecting a23a7b0 .* under bench\/src \| reports c71bd43 generated at c71bd43 .* \| pass 42 \| gh readable$/,
  );
});

test("render's dispatch line carries the one command on the workflow token", () => {
  const [state, out] = call("render", { facts: facts(), error: null });
  assert.equal(state, "dispatch");
  assert.match(out.split("\n")[0], /^dispatch: the reports \(c71bd43, generated at c71bd43 .*\) predate a23a7b0 /);
  assert.match(out, /GH_TOKEN="\$GH_WORKFLOW_TOKEN" gh workflow run bench-report\.yml --ref main/);
});

test("an unreadable gh prints UNREADABLE and no state word", () => {
  const [state, out] = call("render", { facts: null, error: "runs of bench-report.yml on main: HTTP 403" });
  assert.equal(state, "UNREADABLE");
  assert.match(out, /^bench-report: UNREADABLE \(runs of bench-report\.yml on main: HTTP 403\)/);
  assert.doesNotMatch(out, /^fresh|^dispatch|^publish|^running/m);
});

// End to end: a stub gh answers the reads from a table keyed by a substring
// of argv and, for `run download`, writes the five files under the -D
// directory. The tree is a scratch git repository so `--publish` has a
// toplevel to write into and a `git status` to report.
function withGh(answers, args, cwd) {
  const dir = mkdtempSync(join(tmpdir(), "bench-report-"));
  const table = join(dir, "answers.json");
  writeFileSync(table, JSON.stringify(answers));
  const stub = join(dir, "stub.mjs");
  writeFileSync(
    stub,
    `import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
const answers = JSON.parse(readFileSync(${JSON.stringify(table)}, "utf8"));
const argv = process.argv.slice(2);
const joined = argv.join(" ");
if (argv[0] === "run" && argv[1] === "download") {
  const out = argv[argv.indexOf("-D") + 1];
  for (const [rel, body] of Object.entries(answers["download"])) {
    mkdirSync(dirname(join(out, rel)), { recursive: true });
    writeFileSync(join(out, rel), body);
  }
  process.exit(0);
}
const hit = Object.entries(answers).find(([needle]) => needle !== "download" && joined.includes(needle));
if (!hit) { console.error("stub gh: no answer for " + joined); process.exit(1); }
console.log(JSON.stringify(hit[1]));
`,
  );
  writeFileSync(join(dir, "gh"), `#!/bin/sh\nexec node ${stub} "$@"\n`);
  chmodSync(join(dir, "gh"), 0o755);
  return spawnSync(script, ["--repo", "o/r", ...args], {
    cwd,
    encoding: "utf8",
    env: { ...process.env, PATH: `${dir}:${process.env.PATH}` },
  });
}

function scratchRepo() {
  const root = mkdtempSync(join(tmpdir(), "bench-tree-"));
  spawnSync("git", ["init", "-q", root]);
  for (
    const rel of [
      "bench/baseline.json",
      "docs/benchmarks/baseline.md",
      "docs/benchmarks/baseline.svg",
      "docs/benchmarks/canterbury.md",
      "docs/benchmarks/silesia.md",
    ]
  ) {
    mkdirSync(join(root, rel, ".."), { recursive: true });
    writeFileSync(join(root, rel), "old\n");
  }
  spawnSync("git", ["-C", root, "add", "."]);
  spawnSync("git", ["-C", root, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "seed"]);
  return root;
}

const commit = (sha, date, message = "x") => [{ sha, commit: { committer: { date }, message } }];
const workflow = (paths) => ({
  content: Buffer.from(
    `name: bench-report\non:\n  push:\n    branches: [main]\n    paths:\n${
      paths.map((p) => `      - ${p}\n`).join("")
    }  workflow_dispatch:\n`,
  ).toString("base64"),
});

const PUBLISH_SUBJECT = "bench: publish the reports regenerated at 0ddba11 (#1390)\n\nbody";

// The reports commit is a publish merged after the bench commit (03:38Z), so
// by date it is newer; its subject names pass head 0ddba11, generated at
// 01:00Z, which is what decides.
const answers = {
  "contents/.github/workflows/bench-report.yml": workflow(["bench/src/**", ".github/workflows/bench-report.yml"]),
  "path=bench/src": commit(AFFECTING.sha, AFFECTING.date),
  "path=.github/workflows/bench-report.yml": commit("1111111111111111111111111111111111111111", "2026-10-17T00:00:00Z"),
  "path=docs/benchmarks/canterbury.md": commit(REPORTS.sha, "2026-10-19T04:30:00Z", PUBLISH_SUBJECT),
  "path=docs/benchmarks/silesia.md": commit(REPORTS.sha, "2026-10-19T04:30:00Z", PUBLISH_SUBJECT),
  "commits/0ddba11": {
    sha: "0ddba11000000000000000000000000000000000",
    commit: { committer: { date: "2026-10-19T01:00:00Z" } },
  },
  "run list": [pass()],
  "commits/deadbeef": {
    sha: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
    commit: { committer: { date: "2026-10-19T04:00:00Z" } },
  },
  "actions/runs/42/artifacts": { artifacts: [{ name: "bench-report", expired: false }] },
  download: {
    "bench/baseline.json": "new\n",
    "docs/benchmarks/baseline.md": "new\n",
    "docs/benchmarks/baseline.svg": "new\n",
    "docs/benchmarks/canterbury.md": "new\n",
    "docs/benchmarks/silesia.md": "new\n",
  },
};

test("end to end: the generator named in the publish subject outranks the reports commit's date, and --publish writes the files", () => {
  const root = scratchRepo();
  const read = withGh(answers, [], root);
  assert.equal(read.status, 0, read.stderr);
  assert.match(
    read.stdout,
    /^publish 42: .*the reports \(c71bd43, generated at 0ddba11 2026-10-19T01:00:00Z\) predate a23a7b0/,
  );

  const written = withGh(answers, ["--publish"], root);
  assert.equal(written.status, 0, written.stderr);
  assert.match(written.stdout, /published run 42: 5 files written into the tree/);
  assert.match(written.stdout, / M docs\/benchmarks\/silesia\.md/);
  assert.equal(readFileSync(join(root, "docs/benchmarks/canterbury.md"), "utf8"), "new\n");
  assert.equal(readFileSync(join(root, "bench/baseline.json"), "utf8"), "new\n");
});

test("end to end: a reports commit naming no generator is its own, and a later one reads fresh", () => {
  const root = scratchRepo();
  const subject = "lz: widen the window and regenerate the reports (#1401)";
  const own = {
    ...answers,
    "path=docs/benchmarks/canterbury.md": commit(FRESH.sha, FRESH.date, subject),
    "path=docs/benchmarks/silesia.md": commit(FRESH.sha, FRESH.date, subject),
  };
  const run = withGh(own, ["--publish"], root);
  assert.equal(run.status, 1);
  assert.match(
    run.stdout,
    /^fresh: the reports \(f0e0d0c, generated at f0e0d0c 2026-10-19T09:00:00Z\) are at or after/,
  );
  assert.match(run.stderr, /--publish needs state publish, not fresh/);
  assert.equal(readFileSync(join(root, "docs/benchmarks/canterbury.md"), "utf8"), "old\n");
});

test("end to end: the trigger paths come from main's workflow, and a workflow listing none is UNREADABLE", () => {
  const root = scratchRepo();
  const none = { ...answers, "contents/.github/workflows/bench-report.yml": workflow([]) };
  const run = withGh(none, [], root);
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /^bench-report: UNREADABLE \(bench-report\.yml on main lists no push paths\)/);
});

test("end to end: a gh that cannot read answers UNREADABLE, exit 0 without --publish", () => {
  const root = scratchRepo();
  const run = withGh({}, [], root);
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /^bench-report: UNREADABLE \(bench-report\.yml on main: stub gh: no answer for /);
});
