// The deny/allow boundary of the off-voice Stop hook, and the liveness
// lines it writes. The predicates read one field of the Stop payload, so
// every case is a payload and an environment; no process table, no
// filesystem beyond the summary file.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("deny-off-voice", import.meta.url).pathname;

const ciEnv = { PATH: process.env.PATH, GITHUB_ACTIONS: "true" };

function run(call, env = ciEnv) {
  return spawnSync(script, [], {
    input: typeof call === "string" ? call : JSON.stringify(call),
    env,
    encoding: "utf8",
  });
}

function withSummary(env = ciEnv) {
  const summary = join(mkdtempSync(join(tmpdir(), "off-voice-")), "summary.md");
  return { env: { ...env, GITHUB_STEP_SUMMARY: summary }, summary };
}

const DASH = "\u2014";
// Run 35881083464's first words, verbatim in shape.
const ANNOUNCING = "Heartbeat done. No open PRs needed attention, so I took #695.";
// Run 35861671444's shape: two issues, each introduced with the dash.
const DASHED = `Two issues filed:\n\n- #702 ${DASH} nothing compares the exclusions.\n- #703 ${DASH} the status box hand-types two facts.`;

test("blocks a stop whose final response opens on an announcement", () => {
  const res = run({ last_assistant_message: ANNOUNCING, stop_hook_active: false });
  assert.equal(res.status, 0);
  const out = JSON.parse(res.stdout);
  assert.equal(out.decision, "block");
  assert.match(out.reason, /"Heartbeat done\."/);
  assert.match(out.reason, /run-notice\.py/);
});

test("the bare announcement blocks too, where trim would have kept it", () => {
  // opener.trim leaves "Heartbeat complete for this cycle." because deleting
  // it empties the notice; here the writer is present to write the content.
  const res = run({ last_assistant_message: "Heartbeat complete for this cycle." });
  assert.equal(JSON.parse(res.stdout).decision, "block");
});

test("blocks a final response carrying em dashes, counted", () => {
  const res = run({ last_assistant_message: DASHED });
  const out = JSON.parse(res.stdout);
  assert.equal(out.decision, "block");
  assert.match(out.reason, /2 em dashes \(U\+2014\)/);
  assert.match(out.reason, /comma, colon, semicolon or period/);
});

test("both findings ride one reason", () => {
  const res = run({ last_assistant_message: `Run complete.\n\n${DASHED}` });
  const out = JSON.parse(res.stdout);
  assert.match(out.reason, /completion announcement/);
  assert.match(out.reason, /em dashes/);
});

test("an opener with content passes", () => {
  assert.equal(run({ last_assistant_message: "Merged #698." }).stdout, "");
  assert.equal(
    run({ last_assistant_message: "PR #706 is open: two function-scoped exclusions." }).stdout,
    "",
  );
  // Keyword, but past twelve words: the sentence says something.
  assert.equal(
    run({
      last_assistant_message:
        "Nothing changed because the sweep, the drain, the stall check and the queue were all empty when read.",
    }).stdout,
    "",
  );
});

test("an en dash and a hyphen pass; the rule names the em dash", () => {
  assert.equal(run({ last_assistant_message: "Bits 0–1 mix with bit 2; disjoint-bit, equivalent." }).stdout, "");
});

test("allows outside CI, where a reader can say so", () => {
  const res = run({ last_assistant_message: ANNOUNCING }, { PATH: process.env.PATH });
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
});

test("malformed or shapeless stdin allows", () => {
  assert.equal(run("not json").stdout, "");
  assert.equal(run("[1, 2]").stdout, "");
});

test("a block writes one summary line per finding", () => {
  const { env, summary } = withSummary();
  const res = run({ last_assistant_message: `Run complete.\n\n${DASHED}` }, env);
  assert.equal(JSON.parse(res.stdout).decision, "block");
  const written = readFileSync(summary, "utf8");
  assert.match(written, /^deny-off-voice: blocked stop; final response opens on an announcement$/m);
  assert.match(written, /^deny-off-voice: blocked stop; final response carries 2 em dashes$/m);
});

test("allows the second stop, recording what walked out", () => {
  // One warning only: re-blocking forever would hold a dead session open.
  const { env, summary } = withSummary();
  const res = run({ last_assistant_message: ANNOUNCING, stop_hook_active: true }, env);
  assert.equal(res.status, 0);
  assert.equal(res.stdout, "");
  assert.match(
    readFileSync(summary, "utf8"),
    /^deny-off-voice: allowed second stop; final response still opens on an announcement$/m,
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
  const res = run({ last_assistant_message: ANNOUNCING });
  assert.equal(res.status, 0);
  assert.equal(JSON.parse(res.stdout).decision, "block");
});
