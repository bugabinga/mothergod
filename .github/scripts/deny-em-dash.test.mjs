// Table tests for the em dash guard: the boundary is one code point and a
// posting-verb regex, and a matcher's contract only exists as its cases.
// Each case spawns the real script with the real PreToolUse stdin shape,
// because the plumbing failures (#324's null tool_input, malformed JSON)
// live outside the regex.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
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
  [".github/scripts/gh-pr claude/x 'footer — assembled' <<'EOF'\nbody\nEOF", "a PR through gh-pr"],
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
  assert.equal(run(JSON.stringify({ tool_name: "Read", tool_input: { file_path: "README.md" } })).status, 0);
  assert.equal(run("{not json").status, 0);
  assert.equal(run(JSON.stringify({ tool_name: "Bash", tool_input: null })).status, 0);
});

test("a deny writes its fixed-vocabulary line to the step summary and the audit's record", () => {
  const temp = mkdtempSync(join(tmpdir(), "em-dash-"));
  const summary = join(temp, "summary.md");
  const r = run(bash(denied[0][0]), { ...ciEnv, GITHUB_STEP_SUMMARY: summary, RUNNER_TEMP: temp });
  assert.equal(r.status, 2);
  assert.equal(readFileSync(summary, "utf8"), "deny-em-dash: denied\n");
  assert.deepEqual(JSON.parse(readFileSync(join(temp, "walls.jsonl"), "utf8")), {
    tool: "Bash",
    type: "hook",
    reason: "deny-em-dash: denied",
  });
});

// A hook that dies exits 1, a non-blocking error the harness lets proceed,
// so a telemetry write that raised turned the deny into an allow (#1111).
test("a deny stands when the step summary or the record cannot be written", () => {
  const dir = mkdtempSync(join(tmpdir(), "em-dash-"));
  const missing = join(dir, "missing");
  for (
    const env of [
      { ...ciEnv, GITHUB_STEP_SUMMARY: join(missing, "summary.md"), RUNNER_TEMP: dir },
      { ...ciEnv, GITHUB_STEP_SUMMARY: join(dir, "summary.md"), RUNNER_TEMP: missing },
    ]
  ) {
    const r = run(bash(denied[0][0]), env);
    assert.equal(r.status, 2, r.stderr);
    assert.match(r.stderr, /em dash \(U\+2014\)/);
    assert.match(r.stderr, /^walls: .*missing/m);
  }
});

// The heredoc is not the only way a body travels: a file written earlier
// carries the dash where the command string cannot show it (PR #669 review).
test("a body file named by the command is read", () => {
  const dir = mkdtempSync(join(tmpdir(), "em-dash-"));
  const dashed = join(dir, "dashed.md");
  const clean = join(dir, "clean.md");
  writeFileSync(dashed, "shipped \u2014 verified\n");
  writeFileSync(clean, "shipped, verified\n");
  assert.equal(run(bash(`gh pr comment 5 --body-file ${dashed}`)).status, 2, "--body-file path");
  assert.equal(run(bash(`gh pr create --title t --body-file=${dashed}`)).status, 2, "--body-file=path");
  assert.equal(run(bash(`gh pr comment 5 -F '${dashed}'`)).status, 2, "-F path, quoted");
  assert.equal(run(bash(`git commit -F ${dashed}`)).status, 2, "git commit -F");
  assert.equal(run(bash(`gh api repos/o/r/issues/1/comments -F body=@${dashed}`)).status, 2, "gh api -F key=@path");
  assert.equal(run(bash(`gh pr comment 5 --body-file ${clean}`)).status, 0, "a clean body file");
  assert.equal(
    run(bash(`gh pr comment 5 --body-file ${join(dir, "missing.md")}`)).status,
    0,
    "a file that does not exist yet",
  );
  assert.equal(run(bash(`cat ${dashed}`)).status, 0, "reading it without posting");
});

// Prose written through Edit or Write leaves a session as a file, never as
// a command: research/JOURNAL.md grew 47 em-dash lines in three weeks of
// entries that way (#774). The same guard reads what those two tools write
// into a prose file (prose.py's suffixes).
const edit = (file_path, new_string, old_string = "") =>
  JSON.stringify({ tool_name: "Edit", tool_input: { file_path, old_string, new_string } });
const write = (file_path, content) => JSON.stringify({ tool_name: "Write", tool_input: { file_path, content } });

test("an Edit or Write that puts an em dash into a prose file is denied", () => {
  for (
    const [input, why] of [
      [edit("research/JOURNAL.md", "a law — stated"), "an Edit to markdown"],
      [
        edit("docs/adr/0001-x.md", "kept — as was", "kept — as was"),
        "an Edit that keeps one inside its span: the new text is what the diff shows",
      ],
      [write("research/progress.jsonl", "{\"note\":\"x \\u2014 y\"}"), "the escape, which JSON lines decode"],
      [write("bench/corpus.toml", "description = \"a — b\""), "a Write to TOML"],
    ]
  ) {
    const r = run(input);
    assert.equal(r.status, 2, why);
    assert.match(r.stderr, /em dash \(U\+2014\) into a prose file/, why);
  }
});

test("an Edit or Write without one, or to a file that is code, is allowed", () => {
  for (
    const [input, why] of [
      [edit("research/JOURNAL.md", "a law, stated", "a law — stated"), "a fix that removes it"],
      [edit("src/lz.rs", "// a — b"), "a Rust comment: code is outside prose.py's set"],
      [write(".github/scripts/deny-em-dash.test.mjs", "const s = '—';"), "this file's own fixtures"],
      [edit("README.md", "a 3–6 line summary"), "an en dash"],
      [JSON.stringify({ tool_name: "Write", tool_input: { content: "—" } }), "a Write with no path"],
    ]
  ) {
    const r = run(input);
    assert.equal(r.status, 0, `${why}: ${r.stderr}`);
  }
});

test("an Edit deny records the tool it refused", () => {
  const temp = mkdtempSync(join(tmpdir(), "em-dash-"));
  const r = run(edit("CHANGELOG.md", "fixed — twice"), {
    ...ciEnv,
    GITHUB_STEP_SUMMARY: join(temp, "s.md"),
    RUNNER_TEMP: temp,
  });
  assert.equal(r.status, 2);
  assert.equal(JSON.parse(readFileSync(join(temp, "walls.jsonl"), "utf8")).tool, "Edit");
});
