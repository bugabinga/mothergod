// gh-comment assembles two things a human keeps getting wrong by hand: the
// role footer the self-trigger guard matches on (issue #138, #206) and the
// credential the write must ride (issue #50, #81). Issue #489 added a third
// verb, `--new`, because the app token expires an hour into a session and a
// long run could not file what it learned. All three are argv-shaped, so a
// stub `gh` on PATH is the whole harness: it records what it was called with.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = join(new URL(".", import.meta.url).pathname, "gh-comment");

function run(args, { body = "hello", env = {} } = {}) {
  const stub = mkdtempSync(join(tmpdir(), "gh-comment-"));
  const log = join(stub, "argv.json");
  const recorder = join(stub, "record.mjs");
  writeFileSync(
    recorder,
    `import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(log)}, JSON.stringify({
  argv: process.argv.slice(2),
  token: process.env.GH_TOKEN ?? null,
  admin: process.env.GH_ADMIN_TOKEN ?? null,
}));
console.log("https://github.com/o/r/issues/1");
`,
  );
  writeFileSync(join(stub, "gh"), `#!/bin/sh\nexec node ${recorder} "$@"\n`);
  chmodSync(join(stub, "gh"), 0o755);
  const proc = spawnSync("python3", [script, ...args], {
    encoding: "utf8",
    input: body,
    env: {
      PATH: `${stub}:${process.env.PATH}`,
      ROLE: "bdfl",
      GH_WORKFLOW_TOKEN: "workflow-token",
      GH_ADMIN_TOKEN: "admin-token",
      ...env,
    },
  });
  let called = null;
  try {
    called = JSON.parse(readFileSync(log, "utf8"));
  } catch {
    // gh was never reached; the refusal tests assert exactly that.
  }
  return { ...proc, called };
}

test("--new creates an issue with the labels, the footer, and the workflow token", () => {
  const { status, called } = run(["--new", "Late writes die", "--label", "bug", "--label", "agent-system"]);
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 4), ["issue", "create", "--title", "Late writes die"]);
  assert.match(called.argv[5], /^hello\n\n---\n_bdfl · \[Claude Code\]/);
  assert.deepEqual(called.argv.slice(6), ["--label", "bug", "--label", "agent-system"]);
  assert.equal(called.token, "workflow-token");
  // The admin PAT is stripped, not merely unused: an ambient one is the bug.
  assert.equal(called.admin, null);
});

test("a completion-announcement opener is trimmed before the body is posted", () => {
  const { status, called, stderr } = run(["489"], {
    body: "Ops-log posted. PR #641 passed review and merged.",
  });
  assert.equal(status, 0);
  assert.match(called.argv[4], /^PR #641 passed review and merged\.\n\n---/);
  assert.match(stderr, /trimmed 1 completion-announcement opener/);
});

test("a body with no opener reaches gh unchanged", () => {
  const { called, stderr } = run(["489"], { body: "PR #641 passed review and merged." });
  assert.match(called.argv[4], /^PR #641 passed review and merged\.\n\n---/);
  assert.equal(stderr, "");
});

test("commenting and closing keep their shapes", () => {
  assert.deepEqual(run(["489"]).called.argv.slice(0, 2), ["issue", "comment"]);
  assert.deepEqual(run(["489", "--close"]).called.argv.slice(0, 2), ["issue", "close"]);
});

test("--new refuses a number or a close, which would mean two verbs at once", () => {
  const { status, stderr, called } = run(["--new", "t", "489", "--close"]);
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /takes no number/);
});

test("--label without --new is refused rather than silently dropped", () => {
  const { status, stderr } = run(["489", "--label", "bug"]);
  assert.equal(status, 1);
  assert.match(stderr, /gh issue edit/);
});

test("no verb reaches gh without GH_WORKFLOW_TOKEN", () => {
  const { status, called } = run(["--new", "t"], { env: { GH_WORKFLOW_TOKEN: "" } });
  assert.equal(status, 1);
  assert.equal(called, null);
});

test("an empty --new is a typo, and says so instead of asking for a number", () => {
  const { status, stderr, called } = run(["--new", ""]);
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /needs a title/);
});

// The two env vars above are one capability, not two knobs: GH_WORKFLOW_TOKEN
// is the credential the post rides and ROLE is the seat it is attributed to,
// and gh-comment refuses without either. Four seats exported the token and not
// the role (issue #505), so every one of their posts died on the second check
// after the agent had already done the work. The roster is the authority for
// the footer's name, so the seat list is read from agents/personas/ rather
// than restated here. Scope: presence per workflow file, which is the mistake
// that happened; two agent steps in one file with mismatched env blocks would
// slip through, and no workflow has ever had two.
test("every workflow that can post also says which seat is posting", () => {
  const root = join(new URL("../..", import.meta.url).pathname);
  const seats = readdirSync(join(root, "agents/personas"))
    .filter((f) => f.endsWith(".md") && f !== "README.md")
    .map((f) => f.slice(0, -3));
  const workflows = readdirSync(join(root, ".github/workflows")).filter((f) => f.endsWith(".yml"));
  for (const file of workflows) {
    const text = readFileSync(join(root, ".github/workflows", file), "utf8");
    if (!text.includes("GH_WORKFLOW_TOKEN:")) continue;
    const role = text.match(/^\s*ROLE:\s*(\S+)\s*$/m);
    assert.ok(role, `${file} exports GH_WORKFLOW_TOKEN but no ROLE; gh-comment dies late (issue #505)`);
    assert.ok(
      seats.includes(role[1]),
      `${file} claims ROLE: ${role[1]}, which is no seat in agents/personas/ (${seats.join(", ")})`,
    );
  }
});

// Issue #590: the footer is assembled here, so a body that arrives carrying
// one, in any of the census shapes, leaves with exactly one, the role's.
test("a typed footer is stripped before the role footer goes on", () => {
  const body = "PR #641 passed review.\n\n---\n_Generated by [Claude Code](https://claude.ai/code)_\n\n"
    + "🤖 Generated with [Claude Code](https://claude.com/claude-code)";
  const { status, stderr, called } = run(["683"], { body });
  assert.equal(status, 0);
  assert.equal(called.argv[4].split("[Claude Code]").length - 1, 1);
  assert.match(called.argv[4], /^PR #641 passed review\.\n\n---\n_bdfl · \[Claude Code\]/);
  assert.match(stderr, /stripped 2 typed attribution line/);
});

test("a PR number is a comment target like any issue: gh takes it (#589, #683)", () => {
  assert.deepEqual(run(["683"]).called.argv.slice(0, 3), ["issue", "comment", "683"]);
});

// Issue #771: a review scoped to #768 commented on and merged #770 instead.
// A review session's environment carries PR_NUMBER; nothing outside that
// scope may reach gh, no matter how the wrong number was resolved.
test("PR_NUMBER in the environment refuses a write to any other number", () => {
  const { status, called, stderr } = run(["770"], { env: { PR_NUMBER: "768" } });
  assert.equal(status, 1);
  assert.equal(called, null);
  assert.match(stderr, /does not match PR_NUMBER=768/);
});

test("PR_NUMBER matching the target is unaffected", () => {
  const { status, called } = run(["768"], { env: { PR_NUMBER: "768" } });
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 3), ["issue", "comment", "768"]);
});

test("no PR_NUMBER in the environment (every non-review seat) is unaffected", () => {
  const { status, called } = run(["770"]);
  assert.equal(status, 0);
  assert.deepEqual(called.argv.slice(0, 3), ["issue", "comment", "770"]);
});
