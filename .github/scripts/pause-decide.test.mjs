// Fixtures for pause-decide, the verdict that ledgers a rung or pauses the
// fleet after a failed run.
//
// The suite exists because the verdict lived in a YAML heredoc for six weeks
// and three edits, provable only by regex-extracting it in a shell (#894).
// It can idle every seat at once, so each route it can take is pinned here.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
loader = importlib.machinery.SourceFileLoader("pause_decide", sys.argv[1] + "/pause-decide.py")
spec = importlib.util.spec_from_loader("pause_decide", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
call = json.loads(sys.argv[2])
entries = mod.entries_of(call["raw"]) if "raw" in call else call["entries"]
print(json.dumps(mod.decide(entries, call.get("rung", ""), call["now"])))
`;

// 2026-10-12T10:00:00Z, so a "resets 3:30pm (UTC)" phrase lands today and a
// "resets 9:00am (UTC)" one lands tomorrow.
const NOW = 1_791_799_200;
const RUNG = "claude-sonnet-5";
const RESOLVED = "claude-sonnet-5-5-20260301";

function decide(call) {
  const res = spawnSync("python3", ["-I", "-c", driver, scriptsDir, JSON.stringify({ now: NOW, ...call })], {
    encoding: "utf8",
  });
  assert.equal(res.status, 0, res.stderr);
  return JSON.parse(res.stdout);
}

const init = { type: "system", subtype: "init", model: RESOLVED };
const ok = { type: "result", is_error: false, result: "done" };
const failed = (extra) => ({ type: "result", is_error: true, ...extra });
const rejected = (resetsAt) => ({ type: "rate_limit_event", rate_limit_info: { status: "rejected", resetsAt } });

test("a success routes none", () => {
  assert.deepEqual(decide({ entries: [init, ok], rung: RUNG }), ["none", "", "", ""]);
});

test("transcript prose never pauses (#11): a turn about limits is not a limit", () => {
  const prose = { type: "assistant", message: "You've reached your weekly limit, resets 3:30pm (UTC)" };
  assert.deepEqual(decide({ entries: [init, prose, ok], rung: RUNG }), ["none", "", "", ""]);
  // Even inside the failure, only the failure's own text is read.
  const unrelated = failed({ result: "git push rejected" });
  assert.deepEqual(decide({ entries: [init, prose, unrelated], rung: RUNG }), ["none", "", "", ""]);
});

test("a success whose text names a limit still routes none (#11)", () => {
  const limitText = { type: "result", is_error: false, result: "You've reached your weekly limit" };
  assert.deepEqual(decide({ entries: [init, limitText], rung: RUNG }), ["none", "", "", ""]);
});

test("a 429 keys the ledger by the alias rung, the resolved id in the note", () => {
  const entries = [init, rejected(NOW + 1800), rejected(NOW + 5400), failed({ api_error_status: 429 })];
  assert.deepEqual(decide({ entries, rung: RUNG }), [
    "ledger",
    RUNG,
    String(NOW + 5400),
    `rate-limited, ran as ${RESOLVED}`,
  ]);
});

test("a rejection with no advertised reset blocks the rung for an hour", () => {
  const entries = [init, { type: "rate_limit_event", rate_limit_info: { status: "rejected" } }, failed({})];
  assert.deepEqual(decide({ entries, rung: RUNG }), [
    "ledger",
    RUNG,
    String(NOW + 3600),
    `rate-limited, ran as ${RESOLVED}`,
  ]);
});

test("a 404 keys the ledger by the alias rung for seven days", () => {
  const entries = [init, failed({ api_error_status: 404 })];
  assert.deepEqual(decide({ entries, rung: RUNG }), [
    "ledger",
    RUNG,
    String(NOW + 7 * 86400),
    `not served, ran as ${RESOLVED}`,
  ]);
});

test("a seat without a rung keys by the resolved id, and the note names no alias", () => {
  const entries = [init, failed({ api_error_status: 429 })];
  assert.deepEqual(decide({ entries, rung: "" }), ["ledger", RESOLVED, String(NOW + 3600), "rate-limited"]);
});

test("a rung that resolved to itself carries no 'ran as'", () => {
  const entries = [{ ...init, model: RUNG }, failed({ api_error_status: 404 })];
  assert.deepEqual(decide({ entries, rung: RUNG }), ["ledger", RUNG, String(NOW + 7 * 86400), "not served"]);
});

test("401 and 403 route to auth, whatever the rung", () => {
  for (const status of [401, 403]) {
    const entries = [init, rejected(NOW + 60), failed({ api_error_status: status })];
    assert.deepEqual(decide({ entries, rung: RUNG }), ["auth", "", String(status), ""]);
  }
});

test("the text-marker fallback pauses on a limit with no model to pin it on", () => {
  const weekly = failed({ result: "You've reached your weekly limit. Resets 3:30pm (UTC)." });
  assert.deepEqual(decide({ entries: [weekly] }), [
    "pause",
    "weekly",
    "reached your weekly limit",
    "2026-10-12T15:30:00Z",
  ]);
  const rolling = failed({ result: "Claude usage limit hit, resets 9:00am (UTC)" });
  assert.deepEqual(decide({ entries: [rolling] }), ["pause", "rolling", "usage limit", "2026-10-13T09:00:00Z"]);
  const noReset = failed({ result: "out of extra usage" });
  assert.deepEqual(decide({ entries: [noReset] }), ["pause", "rolling", "out of extra usage", ""]);
});

test("a 429 with no rung and no resolved model falls through to the marker and pauses", () => {
  const entry = failed({ api_error_status: 429, result: "Claude usage limit hit" });
  assert.deepEqual(decide({ entries: [entry] }), ["pause", "rolling", "usage limit", ""]);
});

test("a reset named for the current minute means tomorrow, a later minute today", () => {
  const now = failed({ result: "usage limit, resets 10:00am (UTC)" });
  assert.equal(decide({ entries: [now] })[3], "2026-10-13T10:00:00Z");
  const later = failed({ result: "usage limit, resets 10:01am (UTC)" });
  assert.equal(decide({ entries: [later] })[3], "2026-10-12T10:01:00Z");
});

test("the marker never carries the field separator", () => {
  const entry = failed({ result: "reached your a|b limit" });
  assert.deepEqual(decide({ entries: [entry] }), ["pause", "rolling", "reached your a b limit", ""]);
});

test("entries_of reads one JSON array or one object per line, skipping torn lines", () => {
  const array = JSON.stringify([init, failed({ api_error_status: 404 })]);
  assert.equal(decide({ raw: array, rung: RUNG })[0], "ledger");
  const lines = [JSON.stringify(init), "{torn", JSON.stringify(failed({ api_error_status: 404 }))].join("\n");
  assert.equal(decide({ raw: lines, rung: RUNG })[0], "ledger");
  assert.deepEqual(decide({ raw: "", rung: RUNG }), ["none", "", "", ""]);
});
