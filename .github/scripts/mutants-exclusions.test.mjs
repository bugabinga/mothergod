// Table tests for the dead-exclusion check: each case spawns the real
// script on a config and a list written to a temp dir, because the
// contract is the exit code and the footer the workflow reads, not a
// function. The dead case is the done-criterion #702's triage named: the
// check fires on a deliberately dead pin.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("mutants-exclusions", import.meta.url).pathname;

// The list shape `cargo mutants --list --no-config --colors=never` prints:
// path:line:col, the mutation, the function.
const list = [
  "src/coder.rs:260:34: replace << with >> in Decoder<'a>::decode",
  "src/coder.rs:260:40: replace & with | in Decoder<'a>::decode",
  "src/coder.rs:263:28: replace << with >> in Decoder<'a>::decode",
  "src/coder.rs:14:23: replace >> with << in HALF",
  "src/coder.rs:131:17: replace -= with += in Encoder::encode",
  "src/coder.rs:140:17: replace -= with += in Encoder::encode",
  "src/filters.rs:40:30: replace | with ^ in base64_unwrap::try_decode",
].join("\n");

function run(config, listText = list) {
  const dir = mkdtempSync(join(tmpdir(), "mutants-exclusions-"));
  const configPath = join(dir, "mutants.toml");
  const listPath = join(dir, "mutants.list");
  writeFileSync(configPath, config);
  writeFileSync(listPath, listText);
  return spawnSync(script, [configPath, listPath], { encoding: "utf8" });
}

const toml = (...entries) =>
  `# a comment\nexclude_re = [\n${entries.map((e) => `  ${JSON.stringify(e)},\n`).join("")}]\n`;

test("every entry matching exits 0 with its count beside it", () => {
  const r = run(toml("replace -= with \\+= in Encoder::encode$", "^src/coder\\.rs:260:34: replace << with >>"));
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /^ {4}2 {2}replace -= with \\\+= in Encoder::encode\$$/m);
  assert.match(r.stdout, /^ {4}1 {2}\^src\/coder\\\.rs:260:34: replace << with >>$/m);
  assert.match(r.stdout, /mutants-exclusions: entries 2 \| live 2 \| dead 0 \| unreadable 0 \| mutants 7$/m);
  assert.doesNotMatch(r.stdout, /::error::/);
});

test("a pin on a line that moved is dead: exit 1, one error naming the entry", () => {
  // The #1172 shape: the shift moved from 288 to 260, the pin stayed.
  const r = run(
    toml(
      "^src/coder\\.rs:288:34: replace << with >>",
      "replace \\| with \\^ in base64_unwrap::(try_decode|b64_encode)$",
    ),
  );
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /^ {4}0 {2}\^src\/coder\\\.rs:288:34/m);
  assert.match(
    r.stdout,
    /::error::dead exclusion: '\^src\/coder\\\\\.rs:288:34: replace << with >>' matches no mutant/,
  );
  assert.match(r.stdout, /Repin it to the current line, or delete it if the mutant left, in tests\/mutants\.toml\./);
  assert.match(r.stdout, /entries 2 \| live 1 \| dead 1 \| unreadable 0 \| mutants 7$/m);
});

test("an entry is unanchored unless it says otherwise, as cargo-mutants reads it", () => {
  // `in HALF` is a substring of one name; `^in HALF` is a substring of none.
  const r = run(toml("in HALF", "^in HALF"));
  assert.equal(r.status, 1);
  assert.match(r.stdout, /^ {4}1 {2}in HALF$/m);
  assert.match(r.stdout, /^ {4}0 {2}\^in HALF$/m);
});

test("an entry Python re cannot compile is a finding, not a skip", () => {
  const r = run(toml("replace \\p{Lu} with"));
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /::error::unreadable exclusion: 'replace \\\\p\{Lu\} with' does not compile/);
  assert.match(r.stdout, /\(no \\p\{\} classes\)\./);
  assert.match(r.stdout, /entries 1 \| live 0 \| dead 0 \| unreadable 1/);
});

test("a list with no names is UNREADABLE, exit 2, never zero dead", () => {
  const r = run(toml("in HALF"), "\n\n");
  assert.equal(r.status, 2);
  assert.match(r.stderr, /UNREADABLE/);
  assert.doesNotMatch(r.stdout, /dead 0/);
});

test("a config that is not TOML is UNREADABLE, exit 2", () => {
  const r = run("exclude_re = [\n  \"unterminated\n");
  assert.equal(r.status, 2);
  assert.match(r.stderr, /UNREADABLE/);
});

test("a config without exclude_re checks nothing and exits 0", () => {
  const r = run("timeout_multiplier = 10\n");
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /entries 0 \| live 0 \| dead 0 \| unreadable 0 \| mutants 7/);
});

test("the repository's own config is readable in this dialect", () => {
  // Every entry on main compiles as a Python re, so a PR adding one this
  // check cannot read goes red here before it goes red in mutants-check.
  const repoRoot = new URL("../..", import.meta.url).pathname;
  const probe = `
import importlib.machinery, importlib.util, sys
loader = importlib.machinery.SourceFileLoader("mx", sys.argv[1])
spec = importlib.util.spec_from_loader("mx", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
with open(sys.argv[2], encoding="utf-8") as f:
    rows = mod.check(mod.entries(f.read()), ["nothing"])
bad = [e for e, state, _ in rows if state == "unreadable"]
print(len(rows), len(bad))
`;
  const r = spawnSync("python3", ["-c", probe, script, join(repoRoot, "tests/mutants.toml")], {
    encoding: "utf8",
  });
  assert.equal(r.status, 0, r.stderr);
  const [entries, unreadable] = r.stdout.trim().split(" ").map(Number);
  assert.ok(entries > 0, "tests/mutants.toml carries entries");
  assert.equal(unreadable, 0, `unreadable entries: ${unreadable}`);
});
