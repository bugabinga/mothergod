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

// A block is its `# mutants:` line, omitted when count is null, then its
// entries; blocks are split by a blank line, as in tests/mutants.toml.
const block = (count, ...entries) => [
  ...(count === null ? [] : [`  # mutants: ${count}`]),
  ...entries.map((e) => `  ${JSON.stringify(e)},`),
];
const config = (...blocks) => `# a comment\nexclude_re = [\n${blocks.map((b) => b.join("\n")).join("\n\n")}\n]\n`;
// One block whose proof covers whatever matches, for the per-entry cases.
const toml = (...entries) => config(block("any", ...entries));

test("every entry matching exits 0 with its count beside it", () => {
  const r = run(toml("replace -= with \\+= in Encoder::encode$", "^src/coder\\.rs:260:34: replace << with >>"));
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /^ {4}2 {2}replace -= with \\\+= in Encoder::encode\$$/m);
  assert.match(r.stdout, /^ {4}1 {2}\^src\/coder\\\.rs:260:34: replace << with >>$/m);
  assert.match(
    r.stdout,
    /mutants-exclusions: entries 2 \| live 2 \| dead 0 \| unreadable 0 \| blocks 1 \| miscounted 0 \| uncounted 0 \| mutants 7$/m,
  );
  assert.doesNotMatch(r.stdout, /::error::/);
});

test("a column pin survives a line shift above it, and its block stays counted", () => {
  // #1813: the same mutant before and after 20 lines land above it.
  const pin = "^src/coder\\.rs:\\d+:34: replace << with >> in Decoder<'a>::decode$";
  for (const line of [260, 280]) {
    const r = run(config(block(1, pin)), list.replace("260:34", `${line}:34`));
    assert.equal(r.status, 0, r.stdout + r.stderr);
    assert.match(r.stdout, /blocks 1 \| miscounted 0 \| uncounted 0/);
  }
});

test("a block matching more than its count is red: the over-match a column pin risks", () => {
  // The cm_path shape: a catchable twin at the same column, operator and
  // function on another line, which only the line tells apart.
  const twin = `${list}\nsrc/coder.rs:290:40: replace & with | in Decoder<'a>::decode`;
  const r = run(config(block(1, "^src/coder\\.rs:\\d+:40: replace & with \\| in Decoder<'a>::decode$")), twin);
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(
    r.stdout,
    /::error::miscounted exclusion block at \S+mutants\.toml:3: its entries match 2 mutants where its `# mutants: 1` line claims 1\./,
  );
  assert.match(r.stdout, /dead 0 \| unreadable 0 \| blocks 1 \| miscounted 1 \| uncounted 0/);
});

test("a block matching fewer than its count is red even with no entry dead", () => {
  const r = run(config(block(3, "replace -= with \\+= in Encoder::encode$")));
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /its entries match 2 mutants where its `# mutants: 3` line claims 3\./);
  assert.match(r.stdout, /dead 0 \| unreadable 0 \| blocks 1 \| miscounted 1/);
});

test("a block counts each mutant once, however many of its entries match it", () => {
  const r = run(config(block(1, "^src/coder\\.rs:14:23:", "in HALF$")));
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /blocks 1 \| miscounted 0/);
});

test("each block is counted on its own, and the finding names the one off by its line", () => {
  const r = run(
    config(
      block(2, "replace -= with \\+= in Encoder::encode$"),
      block(2, "^src/coder\\.rs:\\d+:34: replace << with >> in Decoder<'a>::decode$"),
    ),
  );
  assert.equal(r.status, 1, r.stdout + r.stderr);
  assert.match(r.stdout, /miscounted exclusion block at \S+mutants\.toml:6: its entries match 1 mutants/);
  assert.doesNotMatch(r.stdout, /mutants\.toml:3:/);
  assert.match(r.stdout, /blocks 2 \| miscounted 1/);
});

test("a block declaring no count, or two, is red: an over-match in it would be silent", () => {
  for (const b of [block(null, "in HALF$"), ["  # mutants: 1", ...block(1, "in HALF$")]]) {
    const r = run(config(b));
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(
      r.stdout,
      /::error::uncounted exclusion block at \S+mutants\.toml:3: it declares no single `# mutants: N` line/,
    );
    assert.match(r.stdout, /miscounted 0 \| uncounted 1/);
  }
});

test("an array whose blocks do not reproduce its entries is UNREADABLE, exit 2", () => {
  // An entry on the opening line is parsed by TOML but sits in no block.
  const r = run("exclude_re = [\"in HALF$\",\n  # mutants: 1\n  \"^src/coder\\\\.rs:14:23:\",\n]\n");
  assert.equal(r.status, 2, r.stdout + r.stderr);
  assert.match(r.stderr, /UNREADABLE \(\S+ exclude_re blocks do not reproduce its entries/);
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
  assert.match(
    r.stdout,
    /Rescope it to the mutant's current name, or delete it if the mutant left, in tests\/mutants\.toml\./,
  );
  assert.match(
    r.stdout,
    /entries 2 \| live 1 \| dead 1 \| unreadable 0 \| blocks 1 \| miscounted 0 \| uncounted 0 \| mutants 7$/m,
  );
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
  assert.match(
    r.stdout,
    /entries 0 \| live 0 \| dead 0 \| unreadable 0 \| blocks 0 \| miscounted 0 \| uncounted 0 \| mutants 7/,
  );
});

test("the repository's own config is readable in this dialect and counted per block", () => {
  // Every entry on main compiles as a Python re and sits in a block with
  // a count, so a PR adding one this check cannot read or count goes red
  // here before it goes red in mutants-check.
  const repoRoot = new URL("../..", import.meta.url).pathname;
  const probe = `
import importlib.machinery, importlib.util, sys
loader = importlib.machinery.SourceFileLoader("mx", sys.argv[1])
spec = importlib.util.spec_from_loader("mx", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
with open(sys.argv[2], encoding="utf-8") as f:
    text = f.read()
rows = mod.check(mod.entries(text), ["nothing"])
bad = [e for e, state, _ in rows if state == "unreadable"]
blocks = mod.blocks(text)
in_blocks = [e for _, _, entries in blocks for e in entries]
uncounted = [line for line, declared, _ in blocks if declared is None]
print(len(rows), len(bad), int(in_blocks == mod.entries(text)), len(uncounted))
`;
  const r = spawnSync("python3", ["-c", probe, script, join(repoRoot, "tests/mutants.toml")], {
    encoding: "utf8",
  });
  assert.equal(r.status, 0, r.stderr);
  const [entries, unreadable, reproduced, uncounted] = r.stdout.trim().split(" ").map(Number);
  assert.ok(entries > 0, "tests/mutants.toml carries entries");
  assert.equal(unreadable, 0, `unreadable entries: ${unreadable}`);
  assert.equal(reproduced, 1, "tests/mutants.toml's blocks reproduce its entries");
  assert.equal(uncounted, 0, `blocks without a # mutants: line: ${uncounted}`);
});
