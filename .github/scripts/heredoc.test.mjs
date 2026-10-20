// The parse the two shell-idiom guards share (#1377): a heredoc body
// fed to a posting script leaves the command before the scan, every
// other body stays. The guards' own tests pin the deny/allow effect; this
// pins the parse, whose edges (quoted delimiters, `<<-`, here-strings, a
// missing terminator, two bodies on one line) no guard case names.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scripts = new URL(".", import.meta.url).pathname;

function strip(command) {
  const r = spawnSync(
    "python3",
    ["-c", "import sys, heredoc; sys.stdout.write(heredoc.without_posted_bodies(sys.stdin.read()))"],
    { cwd: scripts, input: command, encoding: "utf8", env: { PATH: process.env.PATH } },
  );
  assert.equal(r.status, 0, r.stderr);
  return r.stdout;
}

test("a posting script's body leaves; the operator line and the terminator stay", () => {
  const command = ".github/scripts/gh-comment 1377 <<'EOF'\nThe old chain was fmt & clippy.\nEOF";
  assert.equal(strip(command), ".github/scripts/gh-comment 1377 <<'EOF'\nEOF");
});

test("what follows the terminator stays in the scan", () => {
  const command = "gh-comment 1 <<'EOF'\nprose &\nEOF\nsleep 100 &";
  assert.equal(strip(command), "gh-comment 1 <<'EOF'\nEOF\nsleep 100 &");
});

test("a shell's body stays, wrapped or not", () => {
  for (const receiver of ["bash", "sh", "timeout 600 bash", "env sh", "python3"]) {
    const command = `${receiver} <<EOF\ncargo test &\nEOF`;
    assert.equal(strip(command), command, receiver);
  }
});

test("a timeout or an assignment in front of a posting script is transparent", () => {
  for (
    const receiver of [
      "timeout 60 .github/scripts/gh-pr claude/x \"title\"",
      "timeout -k 5 2m tg-send",
      "GH_TOKEN=x push-branch claude/x src/lz.rs",
      "FOO=1 BAR=2 post-verdict 9 pass",
    ]
  ) {
    assert.equal(strip(`${receiver} <<'EOF'\nfish & chips\nEOF`), `${receiver} <<'EOF'\nEOF`, receiver);
  }
});

test("an unquoted delimiter keeps its body, which bash expands before the poster reads it", () => {
  for (
    const command of [
      "gh-comment 1 <<EOF\nthe `cargo x check` gate\nEOF",
      "gh-comment 1 <<EOF\n$(touch PWNED)\nEOF",
      "gh-comment 1 << END\nx & y\nEND",
      "gh-comment 1 <<-END\n\tx & y\n\tEND",
    ]
  ) {
    assert.equal(strip(command), command, command);
  }
});

test("an empty quoted delimiter ends at the first blank line", () => {
  assert.equal(strip("gh-comment 1 <<''\nx & y\n\ntail"), "gh-comment 1 <<''\n\ntail");
});

test("a body piped onward is read by the pipe's first command, not the poster", () => {
  const command = "cat <<EOF | gh-comment 1\nfish & chips\nEOF";
  assert.equal(strip(command), command);
});

test("delimiter spellings: quoted, double-quoted, escaped, spaced, tab-stripping", () => {
  const cases = [
    ["gh-comment 1 <<'END'\nx & y\nEND", "gh-comment 1 <<'END'\nEND"],
    ["gh-comment 1 <<\"END\"\nx & y\nEND", "gh-comment 1 <<\"END\"\nEND"],
    ["gh-comment 1 <<\\END\nx & y\nEND", "gh-comment 1 <<\\END\nEND"],
    ["gh-comment 1 << 'END'\nx & y\nEND", "gh-comment 1 << 'END'\nEND"],
    ["gh-comment 1 <<-'END'\n\tx & y\n\tEND", "gh-comment 1 <<-'END'\n\tEND"],
    ["gh-comment 1 <<'END' >out\nx & y\nEND", "gh-comment 1 <<'END' >out\nEND"],
  ];
  for (const [command, expected] of cases) {
    assert.equal(strip(command), expected, command);
  }
});

test("a terminator must be the whole line; a body with none runs to the end", () => {
  assert.equal(strip("gh-comment 1 <<'EOF'\nx & y\nEOF \nz &\nEOF"), "gh-comment 1 <<'EOF'\nEOF");
  assert.equal(strip("gh-comment 1 <<'EOF'\nx & y\nmore"), "gh-comment 1 <<'EOF'");
});

test("a here-string is a word, not a body", () => {
  const command = "gh-comment 1 <<<'fish & chips'\necho done &";
  assert.equal(strip(command), command);
});

test("two heredocs on one line take their bodies in order", () => {
  const command = "gh-comment 1 <<'A' <<'B'\nfirst &\nA\nsecond &\nB\ntail";
  assert.equal(strip(command), "gh-comment 1 <<'A' <<'B'\nA\nB\ntail");
});

test("a posting command behind a control operator still owns its body", () => {
  const command = "cd /tmp && .github/scripts/gh-comment 1 --close <<'EOF'\nfmt & clippy\nEOF";
  assert.equal(strip(command), "cd /tmp && .github/scripts/gh-comment 1 --close <<'EOF'\nEOF");
});

test("no heredoc, no change", () => {
  for (const command of ["cargo test", "echo 'a << b'", ""]) {
    assert.equal(strip(command), command, command);
  }
});
