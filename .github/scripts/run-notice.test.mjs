// run-notice.py composes the only message most runs ever send the operator,
// and it reads audit data written by a process that may itself have crashed.
// Two promises in its docstring are what these pin: the result leads and the
// provenance trails (the operator read past a uniform `<label>: green, 12
// turns` opener to reach the one line that differed, 2026-09-13), and it
// never exits non-zero, because observability does not get to break the
// thing it observes. Twelve sibling scripts carried tests while this one,
// rewritten in #548, carried none.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("run-notice.py", import.meta.url).pathname;
const RUN_URL = "https://github.com/bugabinga/mothergod/actions/runs/99";

// Each case gets its own audit dir; `metadata` and `response` are written
// only when given, so a case can model a run that died before either landed.
function notice({ metadata, response, label = "heartbeat", dir } = {}) {
  const audit = dir ?? mkdtempSync(join(tmpdir(), "run-notice-"));
  if (metadata !== undefined) {
    writeFileSync(join(audit, "metadata.json"), metadata);
  }
  if (response !== undefined) {
    writeFileSync(join(audit, "output-response.md"), response);
  }
  const run = spawnSync("python3", [script, audit, label], {
    encoding: "utf-8",
    env: {
      ...process.env,
      GITHUB_SERVER_URL: "https://github.com",
      GITHUB_REPOSITORY: "bugabinga/mothergod",
      GITHUB_RUN_ID: "99",
    },
  });
  return { status: run.status, out: run.stdout, err: run.stderr };
}

const ok = (extra = {}) =>
  JSON.stringify({ is_error: false, telemetry: { num_turns: 12, duration_ms: 480000 }, ...extra });

test("the response leads and the provenance trails", () => {
  const { status, out } = notice({ metadata: ok(), response: "Merged #545." });
  assert.equal(status, 0);
  // The whole point of #548: the run's own words are the first thing read,
  // and the label/turns/duration line is last and italic.
  assert.match(out, /^Merged #545\./);
  assert.match(out.trimEnd(), /\n_heartbeat, 12 turns, 8m_$/);
});

test("a short complete summary offers no run link", () => {
  // The link is offered exactly when the message is not the whole story;
  // an unconditional link trains the operator to ignore it.
  const { out } = notice({ metadata: ok(), response: "No-op." });
  assert.ok(!out.includes(RUN_URL), `unexpected link in: ${out}`);
  assert.ok(!out.includes("…"), `unexpected ellipsis in: ${out}`);
});

test("a failed run leads with the failure, not with the summary", () => {
  // Inverted deliberately: when a run failed, that it failed IS the news.
  const { status, out } = notice({
    metadata: ok({ is_error: true }),
    response: "Got partway through the sweep.",
  });
  assert.equal(status, 0);
  assert.match(out, /^\*\*heartbeat failed\.\*\*/);
  assert.ok(out.includes(RUN_URL), "a failed run must link its record");
});

test("a clipped response says so and links the full record", () => {
  // A cut the reader cannot see or recover from reads as withholding.
  const long = `${"x".repeat(900)}\n\nthe conclusion that matters${"y".repeat(600)}`;
  const { out } = notice({ metadata: ok(), response: long });
  assert.ok(out.includes("…"), "a clipped notice must show it was clipped");
  assert.ok(out.includes(RUN_URL), "a clipped notice must link the rest");
  // Cut at the paragraph break the writer already made, not mid-word.
  assert.ok(out.includes(`${"x".repeat(900)}…`), `cut at the wrong boundary: ${out.slice(880, 940)}`);
});

test("an empty or placeholder response is absence, not content", () => {
  // agent-audit writes `(...)` when the execution file carried no result.
  for (const response of ["", "(no final response recorded)"]) {
    const { status, out } = notice({ metadata: ok(), response });
    assert.equal(status, 0);
    assert.match(out, /left no summary behind/);
    assert.ok(out.includes(RUN_URL), "no summary means the link is all there is");
  }
});

test("unreadable audit data still produces a notice, never an exit code", () => {
  // Every one of these is a real way a crashed run leaves its directory.
  const cases = [
    { name: "no metadata at all", metadata: undefined },
    { name: "metadata is not JSON", metadata: "{" },
    { name: "metadata parses to null", metadata: "null" },
    { name: "metadata parses to a list", metadata: "[]" },
  ];
  for (const { name, metadata } of cases) {
    const { status, out } = notice({ metadata, response: "ignored" });
    assert.equal(status, 0, `${name}: exited ${status}`);
    assert.ok(out.includes(RUN_URL), `${name}: must link the run log`);
    assert.ok(out.trim().length > 0, `${name}: printed nothing`);
  }
});

test("missing telemetry drops the fact it cannot state", () => {
  // A notice claiming "0 turns, 0m" would be inventing a measurement.
  const { out } = notice({ metadata: JSON.stringify({ is_error: false }), response: "Done." });
  assert.match(out.trimEnd(), /\n_heartbeat_$/);
});

test("wrong arguments are a caller bug and do exit non-zero", () => {
  // The no-nonzero-exit promise covers bad audit DATA, not a bad invocation:
  // a typo in the workflow must fail loudly rather than post a notice.
  const run = spawnSync("python3", [script, "only-one-arg"], { encoding: "utf-8" });
  assert.equal(run.status, 2);
  assert.match(run.stderr, /usage:/);
});
