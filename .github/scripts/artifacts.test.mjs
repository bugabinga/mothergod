// Fixtures for the per-artifact lens (issue #485): the kind a post sorts
// into names the rule set the judge cites, the role comes off the footer
// line and nowhere else, and a ledger thread is skipped unless it is the
// ops log. Rendering is pinned on one artifact, because the section is
// what the BDFL reads every wake and a lens that prints the wrong label
// has the judge citing the wrong rule.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import io, json, sys
sys.path.insert(0, sys.argv[1])
import artifacts
fixture = json.loads(sys.argv[2])
found = artifacts.classify(fixture["threads"], fixture["comments"], fixture["anchor"])
out = io.StringIO()
count = artifacts.render(found, out)
print(json.dumps({
    "labels": [f"{a['at']} {a['role']} {artifacts.label(a)}" for a in found],
    "count": count,
    "rendered": out.getvalue(),
}))
`;

const run = (fixture) => {
  const proc = spawnSync("python3", ["-I", "-c", driver, scriptsDir, JSON.stringify(fixture)], {
    encoding: "utf8",
  });
  assert.equal(proc.status, 0, proc.stderr);
  return JSON.parse(proc.stdout);
};

const footer = (role) => `\n\n---\n_${role} · [Claude Code](https://claude.ai/code)_`;

const thread = (number, created, { pr = false, role = null, labels = [], body = "body" } = {}) => ({
  number,
  title: `title ${number}`,
  body: role ? body + footer(role) : body,
  created_at: created,
  updated_at: created,
  html_url: `https://github.com/o/r/${pr ? "pull" : "issues"}/${number}`,
  labels: labels.map((name) => ({ name })),
  ...(pr ? { pull_request: { url: "x" } } : {}),
});

const comment = (id, number, created, role, body = "comment") => ({
  id,
  body: role ? body + footer(role) : body,
  created_at: created,
  updated_at: created,
  html_url: `https://github.com/o/r/issues/${number}#issuecomment-${id}`,
  issue_url: `https://api.github.com/repos/o/r/issues/${number}`,
});

const ANCHOR = "2026-10-08T04:00:00Z";

test("each kind names its rule set, oldest first", () => {
  const result = run({
    anchor: ANCHOR,
    threads: [
      thread(926, "2026-10-08T07:00:46Z", { role: "herald" }),
      thread(925, "2026-10-08T06:58:00Z", { pr: true, role: "herald" }),
      thread(3, "2026-08-20T19:00:00Z", { labels: ["ops-log", "ledger"] }),
    ],
    comments: [
      comment(1, 925, "2026-10-08T07:03:04Z", "reviewer"),
      comment(2, 925, "2026-10-08T07:10:00Z", "maintainer"),
      comment(3, 3, "2026-10-08T06:23:46Z", "maintainer"),
      comment(4, 926, "2026-10-08T07:30:00Z", "curator"),
    ],
  });
  assert.deepEqual(result.labels, [
    "2026-10-08T06:23:46Z maintainer digest on #3 [S]",
    "2026-10-08T06:58:00Z herald PR #925 [T,P]",
    "2026-10-08T07:00:46Z herald issue #926 [I]",
    "2026-10-08T07:03:04Z reviewer review on #925 [R]",
    "2026-10-08T07:10:00Z maintainer reply on #925 [Q]",
    "2026-10-08T07:30:00Z curator reply on #926 [Q]",
  ]);
  assert.equal(result.count, 6);
});

test("no footer, no seat: the operator's issue and dependabot's PR are not judged", () => {
  const result = run({
    anchor: ANCHOR,
    threads: [
      thread(930, "2026-10-08T07:00:00Z"),
      thread(931, "2026-10-08T07:00:00Z", { pr: true }),
    ],
    comments: [comment(1, 930, "2026-10-08T07:05:00Z", null)],
  });
  assert.deepEqual(result.labels, []);
  assert.match(result.rendered, /no seat opened a PR or issue/);
});

test("a footer quoted mid-body is not the footer line", () => {
  const body = "see `_reviewer · [Claude Code](https://claude.ai/code)_` in the script\n\nmore prose";
  const result = run({
    anchor: ANCHOR,
    threads: [thread(940, "2026-10-08T07:00:00Z", { body })],
    comments: [],
  });
  assert.deepEqual(result.labels, []);
});

test("created before the anchor is out, however recently updated", () => {
  const result = run({
    anchor: ANCHOR,
    threads: [thread(900, "2026-10-01T00:00:00Z", { role: "bdfl" })],
    comments: [comment(1, 900, "2026-10-01T00:10:00Z", "curator")],
  });
  assert.deepEqual(result.labels, []);
});

test("a ledger thread's comments are skipped; the ops log's are the digests", () => {
  const result = run({
    anchor: ANCHOR,
    threads: [
      thread(309, "2026-09-01T00:00:00Z", { labels: ["ledger", "agent-system"] }),
      thread(3, "2026-08-20T19:00:00Z", { labels: ["ops-log", "ledger"] }),
    ],
    comments: [
      comment(1, 309, "2026-10-08T07:03:00Z", "bdfl"),
      comment(2, 3, "2026-10-08T07:04:00Z", "bdfl"),
    ],
  });
  assert.deepEqual(result.labels, ["2026-10-08T07:04:00Z bdfl digest on #3 [S]"]);
});

test("a comment whose thread the page did not return is a reply on an issue", () => {
  const result = run({
    anchor: ANCHOR,
    threads: [],
    comments: [comment(1, 950, "2026-10-08T07:03:00Z", "reviewer")],
  });
  assert.deepEqual(result.labels, ["2026-10-08T07:03:00Z reviewer reply on #950 [Q]"]);
});

test("rendering: header, url, body without its footer, cut at the cap", () => {
  const long = "x".repeat(2000);
  const result = run({
    anchor: ANCHOR,
    threads: [thread(926, "2026-10-08T07:00:46Z", { role: "herald", body: long })],
    comments: [],
  });
  const lines = result.rendered.split("\n");
  assert.match(result.rendered, /rule ids in CLAUDE.md "Voice"/);
  assert.ok(lines.includes("2026-10-08T07:00:46Z  herald  issue #926 [I]  title 926"), lines.join("|"));
  assert.ok(lines.includes("    https://github.com/o/r/issues/926"));
  assert.ok(!result.rendered.includes("Claude Code](https://claude.ai/code)_"), "footer must come off");
  assert.ok(lines.includes("    [cut, 200 more characters]"), lines.slice(-3).join("|"));
});
