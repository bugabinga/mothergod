// envelope-diff's contract: a permission envelope (a `permissions` scope,
// or one of the claude_args FLAGS) moved between two refs is red unless the
// branch is the BDFL's or the author the operator, and a workflow edit that
// moves no envelope key prints nothing (ADR-0008, #1042). Each case spawns
// the real script inside a throwaway git repository with a base and a head
// commit, the way ci.yml's worker job runs it on HEAD^1 and HEAD.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";

const script = new URL("envelope-diff", import.meta.url).pathname;
const repoRoot = new URL("../..", import.meta.url).pathname;

function git(cwd, ...args) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  return r.stdout.trim();
}

// A repository whose base commit holds `base` and whose head commit holds
// `head`, both maps of path to content; a path absent from `head` is deleted.
function repo(base, head) {
  const dir = mkdtempSync(join(tmpdir(), "envelope-diff-"));
  git(dir, "init", "-q", "-b", "main");
  git(dir, "config", "user.email", "test@example.invalid");
  git(dir, "config", "user.name", "test");
  const commit = (files, message) => {
    for (const path of Object.keys(files)) {
      mkdirSync(join(dir, dirname(path)), { recursive: true });
      writeFileSync(join(dir, path), files[path]);
    }
    git(dir, "add", "-A");
    git(dir, "commit", "-q", "--allow-empty", "-m", message);
    return git(dir, "rev-parse", "HEAD");
  };
  const baseSha = commit(base, "base");
  for (const path of Object.keys(base)) if (!(path in head)) rmSync(join(dir, path));
  const headSha = commit(head, "head");
  return { dir, base: baseSha, head: headSha };
}

function run({ dir, base, head }, ...args) {
  return spawnSync(script, [base, head, ...args], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, GITHUB_REPOSITORY: "owner/repo" },
  });
}

const REVIEW = ".github/workflows/agent-review.yml";

function workflow({ contents = "read", maxTurns = 300, tools = "Bash", prompt = "Review it." } = {}) {
  return [
    "name: agent-review",
    "on: pull_request",
    "permissions: {}",
    "jobs:",
    "  review:",
    "    runs-on: ubuntu-24.04",
    "    permissions:",
    `      contents: ${contents}`,
    "      pull-requests: write",
    "    steps:",
    "      - id: claude",
    "        uses: anthropics/claude-code-action@v1",
    "        with:",
    "          prompt: |",
    `            ${prompt}`,
    "          claude_args: |",
    "            --append-system-prompt-file agents/personas/reviewer.md",
    "            ${{ steps.guard.outputs.model_flag }}",
    `            --max-turns ${maxTurns}`,
    `            --tools "${tools}"`,
    "            --allowedTools \"Bash\"",
    "",
  ].join("\n");
}

test("a job's permissions scope widened on a non-bdfl branch is red, naming file, scope and both sides", () => {
  const r = run(
    repo({ [REVIEW]: workflow() }, { [REVIEW]: workflow({ contents: "write" }) }),
    "--branch",
    "claude/deslop-x",
    "--author",
    "app/claude",
  );
  assert.equal(r.status, 1, r.stdout);
  const lines = r.stdout.trim().split("\n");
  assert.deepEqual(lines.slice(0, 1), [`${REVIEW}: jobs.review.permissions.contents: read -> write`]);
  assert.match(
    lines.at(-1),
    /^envelope moved: 1, on branch claude\/deslop-x by app\/claude\. A permission envelope .* is the BDFL's \(ADR-0008\)/,
  );
});

test("--tools and --max-turns moved inside claude_args are red; the persona flag is not a key", () => {
  const r = run(
    repo({ [REVIEW]: workflow() }, { [REVIEW]: workflow({ maxTurns: 500, tools: "Bash,Edit" }) }),
    "--branch",
    "claude/maint-x",
    "--author",
    "app/claude",
  );
  assert.equal(r.status, 1, r.stdout);
  assert.deepEqual(r.stdout.trim().split("\n").slice(0, 2), [
    `${REVIEW}: jobs.review.steps.claude.with.claude_args.--max-turns: 300 -> 500`,
    `${REVIEW}: jobs.review.steps.claude.with.claude_args.--tools: Bash -> Bash,Edit`,
  ]);
  assert.doesNotMatch(r.stdout, /append-system-prompt-file/);
});

test("the same move on a claude/bdfl-* branch, or by the operator, is allowed and exits 0", () => {
  const same = () => repo({ [REVIEW]: workflow() }, { [REVIEW]: workflow({ contents: "write" }) });
  const bdfl = run(same(), "--branch", "claude/bdfl-x", "--author", "app/claude");
  assert.equal(bdfl.status, 0, bdfl.stdout);
  assert.equal(bdfl.stdout.trim().split("\n").at(-1), "envelope moved: 1, allowed: branch claude/bdfl-x is the BDFL's");
  const operator = run(same(), "--branch", "feature/x", "--author", "owner");
  assert.equal(operator.status, 0, operator.stdout);
  assert.equal(operator.stdout.trim().split("\n").at(-1), "envelope moved: 1, allowed: author owner is the operator");
  const named = run(same(), "--branch", "feature/x", "--author", "someone", "--operator", "someone");
  assert.equal(named.status, 0, named.stdout);
});

test("a prompt edit, a non-YAML file and a file outside the roots print nothing and exit 0", () => {
  const r = run(
    repo(
      { [REVIEW]: workflow(), ".github/scripts/x": "a\n", "src/lib.rs": "a\n" },
      { [REVIEW]: workflow({ prompt: "Review it twice." }), ".github/scripts/x": "b\n", "src/lib.rs": "b\n" },
    ),
    "--branch",
    "claude/deslop-x",
    "--author",
    "app/claude",
  );
  assert.equal(r.status, 0, r.stdout);
  assert.equal(r.stdout, "");
});

test("a new workflow's grants and a deleted workflow's are moves from and to absent; an empty map is its own value", () => {
  const added = run(repo({}, { [REVIEW]: workflow() }), "--branch", "claude/x", "--author", "app/claude");
  assert.equal(added.status, 1, added.stdout);
  assert.ok(added.stdout.includes(`${REVIEW}: permissions: absent -> {}`), added.stdout);
  assert.ok(added.stdout.includes(`${REVIEW}: jobs.review.permissions.contents: absent -> read`), added.stdout);
  const deleted = run(repo({ [REVIEW]: workflow() }, {}), "--branch", "claude/x", "--author", "app/claude");
  assert.equal(deleted.status, 1, deleted.stdout);
  assert.ok(deleted.stdout.includes(`${REVIEW}: jobs.review.permissions.contents: read -> absent`), deleted.stdout);
});

test("a composite action's claude_args is read under its steps too", () => {
  const action = (turns) =>
    [
      "name: x",
      "runs:",
      "  using: composite",
      "  steps:",
      "    - uses: anthropics/claude-code-action@v1",
      "      with:",
      "        claude_args: |",
      `          --max-turns ${turns}`,
      "",
    ].join("\n");
  const r = run(
    repo({ ".github/actions/seat/action.yml": action(10) }, { ".github/actions/seat/action.yml": action(20) }),
    "--branch",
    "claude/x",
    "--author",
    "app/claude",
  );
  assert.equal(r.status, 1, r.stdout);
  assert.equal(
    r.stdout.trim().split("\n")[0],
    ".github/actions/seat/action.yml: runs.steps[0].with.claude_args.--max-turns: 10 -> 20",
  );
});

// Every flag any real claude_args block carries is either an envelope key
// or named as not one, so a new flag reddens here until it is classified.
test("the script classifies every flag the repository's claude_args blocks use", () => {
  const source = readFileSync(script, "utf8");
  const listed = (name) =>
    [...source.match(new RegExp(`^${name} = \\(([^)]*)\\)`, "m"))[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  const known = new Set([...listed("FLAGS"), ...listed("IGNORED")]);
  const dir = join(repoRoot, ".github/workflows");
  const unclassified = new Set();
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".yml"))) {
    const text = readFileSync(join(dir, file), "utf8");
    for (const block of text.matchAll(/claude_args: \|\n((?:[ \t]+\S[^\n]*\n)+)/g)) {
      for (const flag of block[1].matchAll(/^\s*(--[\w-]+)/gm)) {
        if (!known.has(flag[1])) unclassified.add(`${file}: ${flag[1]}`);
      }
    }
  }
  assert.deepEqual([...unclassified], [], "name each flag in envelope-diff's FLAGS or IGNORED");
});

test("the real tree from HEAD to HEAD moves nothing", () => {
  const r = spawnSync(script, ["HEAD", "HEAD"], { cwd: repoRoot, encoding: "utf8" });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.equal(r.stdout, "");
});
