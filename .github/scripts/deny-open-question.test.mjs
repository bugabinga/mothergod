// The deny/allow boundary of the closing-question Stop hook, and the
// liveness lines it writes. The predicate reads one field of the Stop
// payload, so every case is a payload and an environment; no process
// table, no filesystem beyond the summary file.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-open-question", import.meta.url).pathname;

const ciEnv = { PATH: process.env.PATH, GITHUB_ACTIONS: "true" };

function run(call, env = ciEnv) {
  return spawnSync(script, [], {
    input: typeof call === "string" ? call : JSON.stringify(call),
    env,
    encoding: "utf8",
  });
}

function withSummary(env = ciEnv) {
  const summary = join(mkdtempSync(join(tmpdir(), "open-question-")), "summary.md");
  return { env: { ...env, GITHUB_STEP_SUMMARY: summary }, summary };
}

// Run 35842553330's last words, verbatim in shape.
const ASKING = "My plan is to add an exclude_re entry. OK to proceed?";

test("blocks a stop whose final response ends on a question", () => {
  const res = run({ last_assistant_message: ASKING, stop_hook_active: false });
  assert.equal(res.status, 0);
  const out = JSON.parse(res.stdout);
  assert.equal(out.decision, "block");
  assert.match(out.reason, /rule 12/);
  assert.match(out.reason, /blocked-on-human/);
});

test("trailing markdown emphasis does not hide the question", () => {
  const res = run({ last_assistant_message: "**Shall I proceed?**  \n" });
  assert.equal(JSON.parse(res.stdout).decision, "block");
});

test("a quoted question is a report, not a question", () => {
  const res = run({
    last_assistant_message: "The maintainer ended its run with \"OK to proceed?\"",
  });
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
});

test("a question followed by a decision passes", () => {
  const res = run({
    last_assistant_message: "Should the file move? Yes: moved it to tests/, PR opened.",
  });
  assert.equal(res.stdout, "");
});

test("a statement passes", () => {
  assert.equal(run({ last_assistant_message: "Merged #698." }).stdout, "");
});

test("allows outside CI, where a human can answer", () => {
  const res = run({ last_assistant_message: ASKING }, { PATH: process.env.PATH });
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
});

test("malformed or shapeless stdin allows", () => {
  assert.equal(run("not json").stdout, "");
  assert.equal(run("[1, 2]").stdout, "");
});

test("a block writes its own summary line", () => {
  const { env, summary } = withSummary();
  const res = run({ last_assistant_message: ASKING }, env);
  assert.equal(JSON.parse(res.stdout).decision, "block");
  assert.match(readFileSync(summary, "utf8"), /^deny-open-question: blocked stop; final response ends on a question$/m);
});

test("allows the second stop, recording the question that walked out", () => {
  // One warning only: re-blocking forever would hold a dead session open.
  const { env, summary } = withSummary();
  const res = run({ last_assistant_message: ASKING, stop_hook_active: true }, env);
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
  assert.match(
    readFileSync(summary, "utf8"),
    /^deny-open-question: allowed second stop; final response still ends on a question$/m,
  );
});

test("a payload without the field allows and says which field", () => {
  // The predicate's substrate is a documented hook field; a rename upstream
  // must leave a trace, not a guard that silently never fires again.
  const { env, summary } = withSummary();
  const res = run({ stop_hook_active: false }, env);
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
  assert.match(readFileSync(summary, "utf8"), /payload carries no last_assistant_message/);
});

test("a fired decision writes no summary when the runner offers none", () => {
  const res = run({ last_assistant_message: ASKING });
  assert.equal(res.status, 0);
  assert.equal(JSON.parse(res.stdout).decision, "block");
});
