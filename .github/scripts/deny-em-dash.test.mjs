// Table tests for the em dash guard: the boundary is one code point and a
// posting-verb regex, and a matcher's contract only exists as its cases.
// Each case spawns the real script with the real PreToolUse stdin shape,
// because the plumbing failures (#324's null tool_input, malformed JSON)
// live outside the regex.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-em-dash", import.meta.url).pathname;
const ciEnv = { PATH: process.env.PATH, GITHUB_ACTIONS: "true" };

function run(stdin, env = ciEnv) {
  return spawnSync(script, [], { input: stdin, env, encoding: "utf8" });
}
const bash = (command) => JSON.stringify({ tool_name: "Bash", tool_input: { command } });

const denied = [
  [
    "gh pr comment 5 --body-file - <<'EOF'\nPASS — verified independently\nEOF",
    "the reviewer's PR comment, heredoc body",
  ],
  ["gh pr create --title t --body 'one idea — one PR'", "a PR body"],
  ["gh pr review 5 --approve --body 'fine — ship'", "a review body"],
  [".github/scripts/gh-comment 3 <<'EOF'\nDigest — nothing moved\nEOF", "an issue comment"],
  ["printf 'no-op — quiet' | .github/scripts/tg-send", "the Telegram status line"],
  ["git commit -m 'lz: prune — dead code'", "a commit message"],
  ["gh api repos/o/r/issues/1/comments -f body='x — y'", "the raw API"],
  ["gh issue close 4 --comment 'done — thanks'", "a closing comment"],
];
const allowed = [
  ["grep -rn '—' --include=*.md .", "a search for the character"],
  ["sed -i 's/—/, /g' research/JOURNAL.md", "a fix that removes it"],
  ["gh pr comment 5 --body 'PASS: verified, 58/58'", "a post without one"],
  ["gh pr comment 5 --body 'a 3–6 line summary'", "an en dash, which the rule does not name"],
  ["echo '—'", "a print that posts nothing"],
];

for (const [command, why] of denied) {
  test(`denies ${why}`, () => {
    const r = run(bash(command));
    assert.equal(r.status, 2, r.stderr);
    assert.match(r.stderr, /em dash \(U\+2014\)/);
  });
}
for (const [command, why] of allowed) {
  test(`allows ${why}`, () => {
    const r = run(bash(command));
    assert.equal(r.status, 0, r.stderr);
    assert.equal(r.stderr, "");
  });
}

test("outside GitHub Actions everything is allowed", () => {
  const r = run(bash(denied[0][0]), { PATH: process.env.PATH });
  assert.equal(r.status, 0);
});

test("another tool, malformed JSON and a null tool_input are allowed", () => {
  assert.equal(run(JSON.stringify({ tool_name: "Write", tool_input: { content: "—" } })).status, 0);
  assert.equal(run("{not json").status, 0);
  assert.equal(run(JSON.stringify({ tool_name: "Bash", tool_input: null })).status, 0);
});

test("a deny writes its fixed-vocabulary line to the step summary", () => {
  const summary = join(mkdtempSync(join(tmpdir(), "em-dash-")), "summary.md");
  const r = run(bash(denied[0][0]), { ...ciEnv, GITHUB_STEP_SUMMARY: summary });
  assert.equal(r.status, 2);
  assert.equal(readFileSync(summary, "utf8"), "deny-em-dash: denied, 1 em dash(es)\n");
});
