// Fixtures for docs-watch, the Claude docs watcher behind the operator's
// 2026-09-17 directive.
//
// The property that matters: a quiet week posts nothing, and a real change
// is never quiet. Both failure directions are expensive. A watcher that
// cries every week gets ignored, and an ignored watcher is worse than none
// because it looks like coverage. A watcher that stays silent through a
// change is the bookmark this replaced. build() is pure so these can run
// without touching the network.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
loader = importlib.machinery.SourceFileLoader("docs_watch", sys.argv[1] + "/docs-watch.py")
spec = importlib.util.spec_from_loader("docs_watch", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
call = json.loads(sys.stdin.read())
body, changed = mod.build(
    call["prior"], call["index"], call["pages"], call["today"], "https://example.test/run/1"
)
print(json.dumps({"body": body, "changed": changed}))
`;

const run = (prior, index, pages, today = "2026-09-20") => {
  const res = spawnSync("python3", ["-c", driver, scriptsDir], {
    input: JSON.stringify({ prior, index, pages, today }),
    encoding: "utf-8",
  });
  if (res.status !== 0) return { status: res.status, err: res.stderr };
  return { status: 0, ...JSON.parse(res.stdout) };
};

// Every slug the script watches must appear, or build() throws on the
// missing key. Keeping the list here rather than importing it is
// deliberate: a page silently dropped from the watchlist should break a
// fixture, not pass quietly.
const SLUGS = [
  "env-vars",
  "settings",
  "cli-reference",
  "github-actions",
  "hooks",
  "skills",
];

const pagesWith = (overrides = {}) => Object.fromEntries(SLUGS.map((s) => [s, overrides[s] ?? `# ${s}\nbody\n`]));

const indexWith = (urls) =>
  ["# Claude Code Docs", ""]
    .concat(urls.map((u) => `- [${u.split("/").pop()}](${u}): description`))
    .join("\n");

const BASE_URLS = [
  "https://code.claude.com/docs/en/env-vars.md",
  "https://code.claude.com/docs/en/settings.md",
];

const stateOf = (body) => {
  const m = body.match(/```json\n([\s\S]*?)\n```/);
  assert.ok(m, "body carries a json state block");
  return JSON.parse(m[1]);
};

test("first run reports the watch starting, exactly once", () => {
  const first = run("", indexWith(BASE_URLS), pagesWith());
  assert.equal(first.changed, true);
  assert.match(first.body, /watch started/);
  assert.match(first.body, /now watched/);

  // Fed its own output back with identical docs, the second run is quiet.
  const second = run(first.body, indexWith(BASE_URLS), pagesWith());
  assert.equal(second.changed, false, "an unchanged week must post nothing");
});

test("a quiet week keeps the prior rounds and changes no state", () => {
  const first = run("", indexWith(BASE_URLS), pagesWith());
  const quiet = run(first.body, indexWith(BASE_URLS), pagesWith(), "2026-09-27");
  assert.equal(quiet.changed, false);
  assert.deepEqual(
    stateOf(quiet.body).pages,
    stateOf(first.body).pages,
    "digests must survive a quiet week",
  );
  assert.deepEqual(
    stateOf(quiet.body).rounds,
    stateOf(first.body).rounds,
    "a quiet week must not open a dated round",
  );
});

test("a page whose body moves is reported by name", () => {
  const first = run("", indexWith(BASE_URLS), pagesWith());
  const moved = run(
    first.body,
    indexWith(BASE_URLS),
    pagesWith({ "env-vars": "# env-vars\nbody\nBASH_MAX_TIMEOUT_MS added\n" }),
    "2026-09-27",
  );
  assert.equal(moved.changed, true);
  assert.match(moved.body, /\*\*changed\*\* \[`env-vars`\]/);
  assert.doesNotMatch(moved.body, /\*\*changed\*\* \[`hooks`\]/, "only the mover reports");
});

test("index deltas name pages that appeared and vanished", () => {
  const first = run("", indexWith(BASE_URLS), pagesWith());
  const next = run(
    first.body,
    indexWith([BASE_URLS[0], "https://code.claude.com/docs/en/brand-new.md"]),
    pagesWith(),
    "2026-09-27",
  );
  assert.equal(next.changed, true);
  assert.match(next.body, /\*\*new page\*\*.*brand-new/);
  assert.match(next.body, /\*\*page gone\*\*.*settings/);
});

test("dated rounds accumulate newest first and stay bounded", () => {
  let body = run("", indexWith(BASE_URLS), pagesWith()).body;
  for (let i = 0; i < 15; i++) {
    body = run(
      body,
      indexWith(BASE_URLS),
      pagesWith({ "env-vars": `# env-vars\nrevision ${i}\n` }),
      `2026-10-${String(i + 1).padStart(2, "0")}`,
    ).body;
  }
  const rounds = stateOf(body).rounds;
  assert.equal(rounds.length, 12, "the ledger keeps KEEP_ROUNDS rounds");
  assert.equal(rounds[0].date, "2026-10-15", "newest first");
  assert.ok(
    body.indexOf("### 2026-10-15") < body.indexOf("### 2026-10-14"),
    "rendered newest first too",
  );
});

test("an unreadable prior body restarts the watch instead of crashing", () => {
  const salvaged = run("a human retyped this issue and broke the block", indexWith(BASE_URLS), pagesWith());
  assert.equal(salvaged.status, 0);
  assert.equal(salvaged.changed, true);
  assert.match(salvaged.body, /watch started/);
});

test("an index that parses to nothing fails the run rather than reporting calm", () => {
  const broken = run("", "<html>the index is HTML now</html>", pagesWith());
  assert.notEqual(broken.status, 0, "a format change must go red, not quiet");
  assert.match(broken.err, /index format changed/);
});

test("the ledger body carries no em dash", () => {
  // CLAUDE.md voice rule, and this body is posted project surface.
  const first = run("", indexWith(BASE_URLS), pagesWith());
  assert.doesNotMatch(first.body, /—/);
});
