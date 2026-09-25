// Table tests for the off-PR write guard: the deny/allow boundary is a
// regex matcher, and a matcher's contract only exists as its cases.
// Each case spawns the real script with the real PreToolUse stdin
// shape, because the hook's failure modes (#324's null tool_input,
// malformed JSON) live in the plumbing, not the regex.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-other-pr", import.meta.url).pathname;
const workflow = new URL("../workflows/agent-review.yml", import.meta.url).pathname;

const reviewEnv = { PATH: process.env.PATH, PR_NUMBER: "768" };

function run(stdin, env = reviewEnv) {
  return spawnSync(script, [], { input: stdin, env, encoding: "utf8" });
}

const bash = (command) => JSON.stringify({ tool_name: "Bash", tool_input: { command } });

const denied = [
  [".github/scripts/merge-pr 770 --sha 3c115a9", "the observed merge, run 36178330703"],
  ["merge-pr 770", "bare script name"],
  [".github/scripts/gh-comment 770 < /tmp/verdict.md", "the observed verdict comment"],
  ["gh-comment 770 --close", "close is a write"],
  ["gh-comment --close 770", "flag before the number"],
  ["gh pr edit 770 --add-label agent-approved", "the observed label flip"],
  ["gh pr edit --add-label agent-approved --remove-label changes-requested 770", "number after flags"],
  ["gh pr merge 770 --squash --auto", "the exit-2 arming aimed wrong"],
  ["gh pr close 770", "close"],
  ["gh pr reopen 770", "reopen"],
  ["gh pr ready 770", "ready"],
  ["gh pr review 770 --approve", "review"],
  ["gh pr comment 770 --body q", "comment"],
  ["gh issue edit 770 --add-label agent-approved", "a PR is an issue node"],
  ["gh issue close 770", "issue close"],
  ["gh issue comment 771 --body-file /tmp/x.md", "an issue is off-PR too"],
  ["gh api -X POST repos/o/r/issues/770/comments -f body=q", "explicit POST"],
  ["gh api --method PATCH repos/o/r/pulls/770 -f title=q", "explicit PATCH"],
  ["gh api repos/o/r/issues/770/labels -f 'labels[]=agent-approved'", "a field flag makes gh POST"],
  ["gh api repos/o/r/issues/770/labels --input /tmp/labels.json", "input body makes gh POST"],
  ["gh api -X DELETE repos/o/r/issues/770/labels/changes-requested", "DELETE"],
  ["cd /tmp/w && .github/scripts/merge-pr 770", "second shell segment"],
  ["gh pr edit 768 --title q; gh pr edit 770 --title q", "right PR then wrong PR"],
  ["merge-pr \"770\"", "a quoted number runs like a bare one"],
  ["merge-pr '770' --sha abc", "single quotes, same"],
  [".github/scripts/push-branch 770 --merge abc", "a merge push to another PR"],
  ["timeout 300 .github/scripts/merge-pr 770 2>&1", "wrapper and redirection"],
  ["gh pr comment 768 --body \"ok\"\ngh pr merge 770 --squash --auto", "newline-separated, review round one"],
  ["merge-pr 768\nmerge-pr 770", "same verb twice, right then wrong"],
  [
    "gh api -X POST repos/o/r/issues/768/comments -f body=q\ngh api -X POST repos/o/r/issues/770/comments -f body=q",
    "two api writes, right then wrong",
  ],
  ["gh pr edit 768 --add-label agent-approved\r\n.github/scripts/merge-pr 770", "CRLF"],
];

const allowed = [
  [".github/scripts/merge-pr 768 --sha 3c115a9", "the PR under review"],
  ["merge-pr 768", "same, bare"],
  [".github/scripts/gh-comment 768 < /tmp/verdict.md", "verdict on the right PR"],
  ["gh-comment 768 --close", "closing the right PR"],
  ["gh-comment --new \"title\" --label agent-system < body", "a new issue names no number"],
  ["gh pr edit 768 --add-label agent-approved --remove-label changes-requested", "the label flip"],
  ["gh pr merge 768 --squash --auto", "arming auto on the right PR"],
  ["gh pr edit 768 --body \"supersedes #770\"", "a number in quoted prose"],
  ["gh pr comment 768 --body \"see #770 and run 36178330703\"", "prose again"],
  ["gh pr view 770", "reading another PR"],
  ["gh pr diff 770", "reading its diff"],
  ["gh pr checks 768", "reading CI"],
  ["gh pr checks 770", "reading another PR's CI"],
  ["gh pr list --state open", "listing"],
  ["gh pr checkout 770", "a local checkout writes nothing remote"],
  ["gh issue view 771", "reading an issue"],
  ["gh api repos/o/r/issues/768/events", "a GET on the right PR"],
  ["gh api repos/o/r/issues/770/events", "a GET on another PR"],
  ["gh api repos/o/r/pulls/770/files --jq .[].filename", "another GET"],
  ["gh api -X POST repos/o/r/issues/768/comments -f body=q", "a write on the right PR"],
  ["gh api repos/o/r/issues/768/labels -f 'labels[]=agent-approved'", "same, implicit POST"],
  [".github/scripts/push-branch 768 --merge abc", "a merge push to the right PR"],
  ["push-branch claude/foo README.md", "a branch name is not a number"],
  ["echo $PR_NUMBER", "resolving the number"],
  ["git log --oneline -770", "a flag value is not a target"],
  ["grep -n 770 research/JOURNAL.md", "no write verb"],
  ["gh run view 36178330703 --log", "a run id is not a PR"],
  ["gh pr edit 768 --title q\ngh pr view 770", "a read on the second line"],
  ["merge-pr 768\necho done", "a second line with no verb"],
];

for (const [command, why] of denied) {
  test(`denies: ${command} (${why})`, () => {
    const r = run(bash(command));
    assert.equal(r.status, 2);
    assert.match(r.stderr, /This run reviews #768; the command targets #77[01]\./);
  });
}

for (const [command, why] of allowed) {
  test(`allows: ${command} (${why})`, () => {
    const r = run(bash(command));
    assert.equal(r.status, 0);
    // Silence on allow is part of the contract: a warning ahead of every
    // real deny message teaches the model noise.
    assert.equal(r.stderr, "");
  });
}

test("allows everything without PR_NUMBER, which is every other seat", () => {
  const env = { PATH: process.env.PATH };
  assert.equal(run(bash("merge-pr 770"), env).status, 0);
  assert.equal(run(bash("merge-pr 770"), { ...env, PR_NUMBER: "" }).status, 0);
  assert.equal(run(bash("merge-pr 770"), { ...env, PR_NUMBER: "$PR_NUMBER" }).status, 0);
});

test("allows non-Bash tools, null tool_input, malformed JSON", () => {
  const read = JSON.stringify({ tool_name: "Read", tool_input: { file_path: "merge-pr 770" } });
  const nullInput = JSON.stringify({ tool_name: "Bash", tool_input: null });
  for (const stdin of [read, nullInput, "not json"]) {
    assert.equal(run(stdin).status, 0);
  }
});

test("a deny writes its class and target to GITHUB_STEP_SUMMARY", () => {
  const summary = join(mkdtempSync(join(tmpdir(), "deny-")), "summary.md");
  const env = { ...reviewEnv, GITHUB_STEP_SUMMARY: summary };
  assert.equal(run(bash("gh pr edit 770 --add-label agent-approved"), env).status, 2);
  assert.match(readFileSync(summary, "utf8"), /^deny-other-pr: denied gh pr write on #770$/m);
});

test("agent-review.yml still arms the guard by setting PR_NUMBER", () => {
  // The guard keys on this one variable; a rename would disarm it
  // silently, so the coupling is pinned here rather than remembered.
  assert.match(
    readFileSync(workflow, "utf8"),
    /^\s+PR_NUMBER: \$\{\{ github\.event\.pull_request\.number \}\}$/m,
  );
});
