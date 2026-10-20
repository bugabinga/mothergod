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

test("a `<<'EOF'` inside a quoted argument is no operator; the lines after it are live", () => {
  for (
    const command of [
      "gh-comment 1 \"title <<'EOF' idiom\"\nsleep 100 &",
      "gh-comment 1 'it''s <<\"EOF\"'\nsleep 100 &",
      "gh-comment --new \"don't <<'EOF' it\"\nsleep 100 &",
    ]
  ) {
    assert.equal(strip(command), command, command);
  }
});

test("a quote that closes before the operator leaves the receiver in charge", () => {
  const command = "gh-comment --new \"don't\" <<'EOF'\nfish & chips\nEOF";
  assert.equal(strip(command), "gh-comment --new \"don't\" <<'EOF'\nEOF");
});

test("a posting name inside an argument is not the receiver", () => {
  const command = "sh -c \"cat | sh; gh-comment 1 \" <<'EOF'\ncargo test &\nEOF";
  assert.equal(strip(command), command);
});

test("a delimiter word is read with quote removal: partly escaped or concatenated", () => {
  for (
    const operator of ["<<\\E\\OF", "<<E\\OF", "<<\"E\"OF", "<<E'O'F", "<<'E'\"O\"F"]
  ) {
    const command = `gh-comment 1 ${operator}\nprose &\nEOF\nsleep 100 &`;
    assert.equal(strip(command), `gh-comment 1 ${operator}\nEOF\nsleep 100 &`, operator);
  }
});

test("an unclosed quote in the delimiter keeps every line after it", () => {
  const command = "gh-comment 1 <<'EOF\nprose &\nEOF\nsleep 100 &";
  assert.equal(strip(command), command);
});

test("a shell inside a backtick substitution is its own receiver", () => {
  for (const body of ["cargo x check", "sleep 1 &"]) {
    const command = `gh-comment 1 \`bash <<'EOF'\n${body}\nEOF\n\``;
    assert.equal(strip(command), command, body);
  }
});

test("the & of a redirection is no control operator", () => {
  for (const redirect of ["2>&1", ">&2", "&>out", "<&0"]) {
    const command = `gh-comment 1 ${redirect} <<'EOF'\nfish & chips\nEOF`;
    assert.equal(strip(command), `gh-comment 1 ${redirect} <<'EOF'\nEOF`, redirect);
  }
});

test("a detach before the receiver still ends its segment", () => {
  const command = "sleep 1 & gh-comment 1 <<'EOF'\nfish & chips\nEOF";
  assert.equal(strip(command), "sleep 1 & gh-comment 1 <<'EOF'\nEOF");
});

test("a delimiter with `$` before a quote or a trailing backslash is not parsed; every line stays", () => {
  for (const operator of ["<<$'EOF'", "<<$\"EOF\"", "<<'E'$'OF'", "<<EOF\\"]) {
    const command = `gh-comment 1 ${operator}\nprose &\nEOF\nsleep 100 &`;
    assert.equal(strip(command), command, operator);
  }
});

test("a quote opened on an earlier line makes the operator an argument", () => {
  const command = "echo \"a\ngh-comment 1 <<'EOF'\nx\" ; sleep 100 &\nEOF";
  assert.equal(strip(command), command);
});

test("a quote that closes on an earlier line leaves the receiver in charge", () => {
  const command = "echo \"a\nb\"\ngh-comment 1 <<'EOF'\nfish & chips\nEOF";
  assert.equal(strip(command), "echo \"a\nb\"\ngh-comment 1 <<'EOF'\nEOF");
});

test("a comment hides the operator; a comment on an earlier line does not", () => {
  const live = "true # note; gh-comment 1 <<'EOF'\nsleep 100 &\nEOF";
  assert.equal(strip(live), live);
  const redirect = "echo hi >#x; gh-comment 1 <<'EOF'\nsleep 100 &\nEOF";
  assert.equal(strip(redirect), redirect);
  const earlier = "# don't\ngh-comment 1 <<'EOF'\nfish & chips\nEOF";
  assert.equal(strip(earlier), "# don't\ngh-comment 1 <<'EOF'\nEOF");
});

test("a `$'` before the operator keeps the body, its escapes are not modelled", () => {
  const command = "echo $'a\\'b\ngh-comment 1 <<'EOF'\nx' ; sleep 100 &\nEOF";
  assert.equal(strip(command), command);
});

test("a `#` inside a word is no comment", () => {
  const command = "gh-comment 1 a#b <<'EOF'\nfish & chips\nEOF";
  assert.equal(strip(command), "gh-comment 1 a#b <<'EOF'\nEOF");
});

test("no heredoc, no change", () => {
  for (const command of ["cargo test", "echo 'a << b'", ""]) {
    assert.equal(strip(command), command, command);
  }
});
