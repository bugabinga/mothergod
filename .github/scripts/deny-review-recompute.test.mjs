// Table tests for the recompute guard: the deny/allow boundary is a
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

const script = new URL("deny-review-recompute", import.meta.url).pathname;

const reviewEnv = {
  PATH: process.env.PATH,
  GITHUB_ACTIONS: "true",
  GITHUB_EVENT_NAME: "pull_request",
};

function run(stdin, env = reviewEnv) {
  return spawnSync(script, [], { input: stdin, env, encoding: "utf8" });
}

const bash = (command) => JSON.stringify({ tool_name: "Bash", tool_input: { command } });

const denied = [
  ["cargo x test", "the observed recompute, run 33229032665"],
  ["cargo x check", "the whole gate"],
  ["cargo x doc", "unscoped constituent"],
  [".github/scripts/probe cargo x test", "probe prefix changes nothing"],
  ["cd /tmp/w && cargo x check", "second shell segment"],
  ["cargo test --workspace", "the required check unwrapped"],
  ["timeout 600 cargo x test 2>&1", "wrapper and redirection"],
  ["cargo x test --", "bare -- passes no filter, full suite (#333 review)"],
  ["cargo test --", "same bypass on the unwrapped form"],
  ["cargo test -- ''", "an empty quoted filter scopes nothing"],
  ["cargo x \"check\"", "quoting one token is a shell no-op (#333 round 3)"],
  ["cargo \"x\" check", "same, middle token"],
  ["\"cargo\" x check", "same, first token"],
  ["cargo x t\"e\"st", "quote glued inside a keyword"],
  ["cargo x \"test\"", "quoted constituent, no scope"],
  ["cargo \"test\"", "quoted unwrapped form, no scope"],
  ["cargo run --manifest-path x/Cargo.toml -- test", "the alias expanded"],
  ["cargo run --quiet --manifest-path x/Cargo.toml -- check", "with --quiet"],
  ["cargo run --manifest-path ./x/Cargo.toml -- test", "./ prefix (#333 r5)"],
  [
    "cargo run --manifest-path /home/runner/work/mothergod/mothergod/x/Cargo.toml -- check",
    "absolute path, same manifest",
  ],
  [
    "cargo run --quiet --manifest-path ../mothergod/x/Cargo.toml -- test",
    "../ spelling, same manifest",
  ],
  ["cargo run --manifest-path=x/Cargo.toml -- lint", "= joins flag to path"],
  ["cargo +stable x check", "toolchain selector is transparent (#333 r4)"],
  ["cargo +stable test", "same on the unwrapped form"],
  ["cargo +nightly x lint", "same, unscoped lint"],
  ["cargo x \\\n  check", "line continuation splices out (#333 r6)"],
  ["cargo \\\ntest", "same on the unwrapped form"],
  ["cargo x che\\\nck", "mid-word splice is still one word"],
  ["cargo x test \\\n--", "continued bare -- still passes no filter"],
];

const allowed = [
  ["cargo x test -- src/lz", "path-scoped constituent"],
  ["cargo x fmt --check -- src", "scoped fmt"],
  ["cargo test -p mothergod-bench", "targeted crate test"],
  ["cargo test -p mothergod -- roundtrip", "scoped with filter"],
  ["cargo clippy --features corpus-fetch", "feature build CI skips"],
  ["cargo x test --help", "usage lookup"],
  ["gh pr checks 332", "reading CI instead of re-running"],
  ["gh pr diff 333 | grep 'cargo x check'", "mentioning is not running"],
  ["git commit -m \"run cargo x check first\"", "quoted prose"],
  ["cargo x test -- \"src/lz\"", "a quoted scope still scopes (#333 round 2)"],
  ["cargo test -p mothergod -- 'roundtrip stored'", "quoted filter"],
  ["cargo +stable test -p mothergod-bench", "toolchain plus scope"],
  [
    "cargo run --manifest-path x/Cargo.toml -- test -- src/lz",
    "expanded alias, scoped",
  ],
  [
    "cargo run --manifest-path ./x/Cargo.toml -- test -- src/lz",
    "prefixed path, still scoped",
  ],
  [
    "cargo run --manifest-path tools/unix/Cargo.toml -- test",
    "component ending in x is not the x crate",
  ],
  ["cargo x test -- \\\nsrc/lz", "a continued scope argument still scopes"],
  ["grep 'cargo x \\\ncheck' notes.md", "continuation inside quoted prose"],
];

for (const [command, why] of denied) {
  test(`denies: ${command} (${why})`, () => {
    const r = run(bash(command));
    assert.equal(r.status, 2);
    assert.match(r.stderr, /adds no signal/);
  });
}

for (const [command, why] of allowed) {
  test(`allows: ${command} (${why})`, () => {
    assert.equal(run(bash(command)).status, 0);
  });
}

test("allows everything outside a pull_request event", () => {
  const env = { ...reviewEnv, GITHUB_EVENT_NAME: "schedule" };
  assert.equal(run(bash("cargo x test"), env).status, 0);
});

test("allows everything outside GitHub Actions", () => {
  const env = { PATH: process.env.PATH, GITHUB_EVENT_NAME: "pull_request" };
  assert.equal(run(bash("cargo x test"), env).status, 0);
});

test("allows non-Bash tools, null tool_input, malformed JSON", () => {
  const read = JSON.stringify({
    tool_name: "Read",
    tool_input: { file_path: "cargo x test" },
  });
  const nullInput = JSON.stringify({ tool_name: "Bash", tool_input: null });
  for (const stdin of [read, nullInput, "not json"]) {
    assert.equal(run(stdin).status, 0);
  }
});

test("a deny writes its class to GITHUB_STEP_SUMMARY", () => {
  const summary = join(mkdtempSync(join(tmpdir(), "deny-")), "summary.md");
  const env = { ...reviewEnv, GITHUB_STEP_SUMMARY: summary };
  assert.equal(run(bash("cargo x test"), env).status, 2);
  assert.match(
    readFileSync(summary, "utf8"),
    /^deny-review-recompute: denied unscoped cargo x test\/lint\/doc\/fmt$/m,
  );
});
