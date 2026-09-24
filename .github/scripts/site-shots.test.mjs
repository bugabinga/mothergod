// site-shots decides which pages get captured and from which tree, and a
// wrong answer there is evidence for a page nobody will see (#658). A stub
// browser fetches the URL it is handed and writes the served HTML where the
// PNG would go, so these assert which tree served which page at which
// viewport, without Chrome. The real capture is exercised by the herald's
// post-run step on every site PR.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("site-shots", import.meta.url).pathname;

// The stub: `--screenshot=<out>` and the URL are the two arguments that
// matter; the served body stands in for pixels.
const stub = join(mkdtempSync(join(tmpdir(), "site-shots-stub-")), "browser");
writeFileSync(
  stub,
  `#!/usr/bin/env python3
import sys, urllib.request
out = next(a for a in sys.argv if a.startswith("--screenshot=")).split("=", 1)[1]
size = next(a for a in sys.argv if a.startswith("--window-size=")).split("=", 1)[1]
body = urllib.request.urlopen(sys.argv[-1], timeout=10).read().decode()
open(out, "w").write(size + " " + body)
`,
  { mode: 0o755 },
);

function repo() {
  const dir = mkdtempSync(join(tmpdir(), "site-shots-repo-"));
  const git = (...args) =>
    execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", ...args], { cwd: dir }).toString().trim();
  git("init", "-q", "-b", "main");
  mkdirSync(join(dir, "site"));
  writeFileSync(join(dir, "site/index.html"), "<p>old index</p>");
  writeFileSync(join(dir, "site/agents.html"), "<p>old agents</p>");
  writeFileSync(join(dir, "site/logo.svg"), "<svg/>");
  writeFileSync(join(dir, "README.md"), "readme");
  git("add", "-A");
  git("commit", "-q", "-m", "base");
  const base = git("rev-parse", "HEAD");
  return { dir, git, base };
}

function run(dir, out, args, env = {}) {
  return spawnSync("python3", [script, out, ...args], {
    cwd: dir,
    env: { PATH: process.env.PATH, SITE_SHOTS_BROWSER: stub, ...env },
    encoding: "utf8",
  });
}

const manifest = (out) => JSON.parse(readFileSync(join(out, "manifest.json"), "utf8")).shots;

test("a changed page is captured from both trees at both viewports", () => {
  const { dir, git, base } = repo();
  writeFileSync(join(dir, "site/index.html"), "<p>new index</p>");
  git("commit", "-q", "-am", "change index");
  const head = git("rev-parse", "HEAD");
  const out = join(dir, "shots");
  const r = run(dir, out, ["--base", base, "--branch", `claude/x=${head}`]);
  assert.equal(r.status, 0, r.stderr);
  const rows = manifest(out);
  assert.deepEqual(rows.map((row) => [row.page, row.viewport]), [
    ["site/index.html", "375x812"],
    ["site/index.html", "1440x900"],
  ]);
  assert.equal(readFileSync(join(out, rows[0].before), "utf8"), "375,812 <p>old index</p>");
  assert.equal(readFileSync(join(out, rows[0].after), "utf8"), "375,812 <p>new index</p>");
  assert.equal(readFileSync(join(out, rows[1].after), "utf8"), "1440,900 <p>new index</p>");
  assert.equal(rows[0].base, base);
  assert.equal(rows[0].sha, head);
  // agents.html did not change, so it was not captured.
  assert.ok(!readdirSync(out).some((f) => f.includes("agents")));
  // The worktrees are gone: only the main checkout remains.
  assert.equal(git("worktree", "list").split("\n").length, 1);
});

test("a changed asset captures every page, and a new page has no before", () => {
  const { dir, git, base } = repo();
  writeFileSync(join(dir, "site/logo.svg"), "<svg>new</svg>");
  writeFileSync(join(dir, "site/status.html"), "<p>status</p>");
  git("add", "-A");
  git("commit", "-q", "-m", "asset and new page");
  const head = git("rev-parse", "HEAD");
  const out = join(dir, "shots");
  const r = run(dir, out, ["--base", base, "--branch", `claude/x=${head}`]);
  assert.equal(r.status, 0, r.stderr);
  const rows = manifest(out);
  assert.deepEqual([...new Set(rows.map((row) => row.page))], [
    "site/agents.html",
    "site/index.html",
    "site/status.html",
  ]);
  const status = rows.find((row) => row.page === "site/status.html");
  assert.equal(status.before, null);
  assert.equal(readFileSync(join(out, status.after), "utf8"), "375,812 <p>status</p>");
});

test("the push record is the default input, and a branch outside site/ costs nothing", () => {
  const { dir, git, base } = repo();
  writeFileSync(join(dir, "README.md"), "changed");
  git("commit", "-q", "-am", "readme only");
  const head = git("rev-parse", "HEAD");
  writeFileSync(join(dir, ".git/push-branch-refs.json"), JSON.stringify({ "claude/docs": head }));
  const out = join(dir, "shots");
  const r = run(dir, out, ["--base", base]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /claude\/docs: no change under site\//);
  assert.deepEqual(manifest(out), []);
});

test("no record and no --branch is a one-line no-op", () => {
  const { dir, base } = repo();
  const r = run(dir, join(dir, "shots"), ["--base", base]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /pushed no branch/);
});

test("one page's failure keeps every other page's rows and still exits non-zero", () => {
  // The review's reproduction (#732): two changed pages, a browser that
  // fails on one. The other page's rows must survive in the manifest, and
  // the exit must still say something went wrong.
  const flaky = join(mkdtempSync(join(tmpdir(), "site-shots-flaky-")), "browser");
  writeFileSync(
    flaky,
    `#!/usr/bin/env python3
import sys
if sys.argv[-1].endswith("/index.html"):
    sys.exit(3)
exec(open(${JSON.stringify(stub)}).read())
`,
    { mode: 0o755 },
  );
  const { dir, git, base } = repo();
  writeFileSync(join(dir, "site/index.html"), "<p>new index</p>");
  writeFileSync(join(dir, "site/agents.html"), "<p>new agents</p>");
  git("commit", "-q", "-am", "two pages");
  const head = git("rev-parse", "HEAD");
  const out = join(dir, "shots");
  const r = run(dir, out, ["--base", base, "--branch", `claude/x=${head}`], { SITE_SHOTS_BROWSER: flaky });
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /index\.html/);
  assert.match(r.stdout, /4 shot\(s\) failed/);
  const rows = manifest(out);
  const agents = rows.filter((row) => row.page === "site/agents.html");
  assert.equal(agents.length, 2);
  assert.equal(readFileSync(join(out, agents[0].after), "utf8"), "375,812 <p>new agents</p>");
  // index.html's rows exist with nothing captured, which tg-photo skips.
  assert.deepEqual(rows.filter((row) => row.page === "site/index.html").map((row) => [row.before, row.after]), [
    [null, null],
    [null, null],
  ]);
});

test("a missing browser fails loudly rather than skipping", () => {
  const { dir, git, base } = repo();
  writeFileSync(join(dir, "site/index.html"), "<p>new</p>");
  git("commit", "-q", "-am", "change");
  const head = git("rev-parse", "HEAD");
  const r = run(dir, join(dir, "shots"), ["--base", base, "--branch", `claude/x=${head}`], {
    SITE_SHOTS_BROWSER: "/nonexistent/browser",
  });
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /browser/);
});
