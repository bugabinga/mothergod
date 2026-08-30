// Fixtures for guard-decide, the run decision behind every agent seat.
//
// The suite exists because of the second gear (issue #375): a decider that can
// cancel a scheduled run is one nobody should have to reason about by reading
// it. Three of these assert the gear never touches the responsive path, which
// is the property that makes the whole mechanism safe to leave running.
//
// decide() is pure by construction so these can exist at all.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
loader = importlib.machinery.SourceFileLoader("guard_decide", sys.argv[1] + "/guard-decide.py")
spec = importlib.util.spec_from_loader("guard_decide", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
call = json.loads(sys.argv[2])
status, model, effort, note = mod.decide(**call)
print(json.dumps({"status": status, "model": model, "effort": effort, "note": note}))
`;

const ROLES = {
  bdfl: {
    ladder: ["claude-opus-5"],
    effort: "high",
    thrift: { ladder: ["claude-sonnet-5"], effort: "medium" },
  },
  reviewer: { ladder: ["claude-sonnet-5"], effort: "medium" },
};

// A reading whose week-average burn overshoots what reaches the reset by 2x:
// half the window elapsed, 90% of the allowance spent, so 10% has to cover the
// remaining half. Sustainable/actual = 1/9, floored to 25% by KEEP_FLOOR.
const RESETS = 1_700_000_000;
const missing = (used = 0.9, elapsedFraction = 0.5) => ({
  observedAt: RESETS - 604800 * (1 - elapsedFraction),
  resetsAt: RESETS,
  utilization: used,
});

const fence = (obj) => "prose above\n\n```json\n" + JSON.stringify(obj) + "\n```\n\nprose below";

function decide(call) {
  const args = { role: "bdfl", roles: ROLES, ledger: "", allowance: "", ...call };
  const run = spawnSync("python3", ["-c", driver, scriptsDir, JSON.stringify(args)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

test("second gear: a discretionary wake under a missing projection (#375)", async (t) => {
  const allowance = fence(missing());

  await t.test("skips the wakes decimation drops, and says the numbers", () => {
    // 25% kept: runs 4, 8, 12 survive, the three between each pair do not.
    const skipped = decide({ allowance, run_number: 7, discretionary: true });
    assert.equal(skipped.status, "skip");
    assert.match(skipped.note, /^SKIP: discretionary wake/);
    assert.match(skipped.note, /keeping 25% of these wakes/);
    assert.match(skipped.note, /run 7 is not one of them/);
    assert.match(skipped.note, /week-average \d+\.\d\d%\/h/);
    assert.match(skipped.note, /misses reset 2023-11-14 22:13 UTC/);
  });

  await t.test("is a throttle, not a stop: a stated share still runs", () => {
    const kept = [1, 2, 3, 4, 5, 6, 7, 8]
      .map((n) => decide({ allowance, run_number: n, discretionary: true }))
      .filter((d) => d.status !== "skip");
    assert.equal(kept.length, 2, "one wake in four runs");
    // Those that survive still get the thrift tier, both gears at once.
    assert.equal(kept[0].model, "claude-sonnet-5");
    assert.match(kept[0].note, /THRIFT/);
  });

  await t.test("decimates evenly rather than in bursts", () => {
    // A random draw at 25% skips eight in a row often enough to matter; the
    // gap between kept wakes is what the stall sweep and inbox drain feel.
    const kept = Array.from({ length: 40 }, (_, i) => i + 1).filter(
      (n) => decide({ allowance, run_number: n, discretionary: true }).status !== "skip",
    );
    const gaps = kept.slice(1).map((n, i) => n - kept[i]);
    assert.ok(Math.max(...gaps) <= 4, `gaps were ${gaps}`);
  });

  await t.test("throttles proportionally, not as an on/off switch", () => {
    // A 1% overshoot must not cost half the fleet's wakes. 50.5% spent at the
    // halfway mark keeps ~98% of them.
    const gentle = fence(missing(0.505));
    const kept = Array.from({ length: 40 }, (_, i) => i + 1).filter(
      (n) => decide({ allowance: gentle, run_number: n, discretionary: true }).status !== "skip",
    );
    assert.ok(kept.length >= 38, `kept ${kept.length} of 40`);
  });
});

test("second gear never touches the responsive path (ADR-0039)", async (t) => {
  const allowance = fence(missing());
  // Same reading, same run number, the one that IS skipped when discretionary.
  const run_number = 7;

  await t.test("an operator dispatch or event wake runs", () => {
    const d = decide({ allowance, run_number, discretionary: false });
    assert.equal(d.status, "ok");
    assert.match(d.note, /THRIFT/, "thrift still applies; only the skip does not");
  });

  await t.test("a reviewer run runs", () => {
    const d = decide({ role: "reviewer", allowance, run_number, discretionary: false });
    assert.equal(d.status, "ok");
    assert.equal(d.model, "claude-sonnet-5");
  });

  await t.test("a discretionary wake under a healthy allowance runs", () => {
    // 10% spent at the halfway mark reaches the reset with room to spare.
    const healthy = fence(missing(0.1));
    const d = decide({ allowance: healthy, run_number, discretionary: true });
    assert.equal(d.status, "ok");
    assert.equal(d.model, "claude-opus-5", "no thrift either");
    assert.doesNotMatch(d.note, /THRIFT|SKIP/);
  });
});

test("a projection nobody can trust never costs a wake", async (t) => {
  const run_number = 7;
  const cases = {
    "no allowance ledger at all": "",
    "no json fence in the body": "the ledger issue, prose only",
    "unparseable json": "```json\n{not json,}\n```",
    "missing fields": fence({ utilization: 0.9 }),
    "a lapsed window": fence({ ...missing(), observedAt: RESETS + 60 }),
    "zero burn": fence(missing(0)),
    "a string where a number belongs": fence({ ...missing(), utilization: "high" }),
  };
  for (const [name, allowance] of Object.entries(cases)) {
    await t.test(name, () => {
      const d = decide({ allowance, run_number, discretionary: true });
      assert.equal(d.status, "ok");
      assert.equal(d.model, "claude-opus-5");
      assert.doesNotMatch(d.note, /THRIFT|SKIP/);
    });
  }
});

test("first gear and ladder resolution, unchanged by the extraction", async (t) => {
  await t.test("the normal tier resolves with no ledgers", () => {
    assert.deepEqual(decide({}), {
      status: "ok",
      model: "claude-opus-5",
      effort: "high",
      note: "claude-opus-5 (ladder: claude-opus-5)",
    });
  });

  await t.test("thrift swaps ladder and effort together", () => {
    const d = decide({ allowance: fence(missing()) });
    assert.equal(d.model, "claude-sonnet-5");
    assert.equal(d.effort, "medium");
    assert.match(d.note, /\[THRIFT \(projected exhaustion .* at week-average .*%\/h\)\]$/);
  });

  await t.test("a limited rung falls through to the next", () => {
    const roles = { bdfl: { ladder: ["claude-opus-5", "claude-sonnet-5"] } };
    const ledger = fence({ "claude-opus-5": Math.floor(Date.now() / 1000) + 3600 });
    const d = decide({ roles, ledger });
    assert.equal(d.model, "claude-sonnet-5");
    assert.match(d.note, /limited: claude-opus-5/);
  });

  await t.test("an expired limit does not block its rung", () => {
    const roles = { bdfl: { ladder: ["claude-opus-5"] } };
    const d = decide({ roles, ledger: fence({ "claude-opus-5": 1 }) });
    assert.equal(d.model, "claude-opus-5");
  });

  await t.test("every rung limited exhausts the seat", () => {
    const roles = { bdfl: { ladder: ["claude-opus-5"] } };
    const resets = Math.floor(Date.now() / 1000) + 3600;
    const d = decide({ roles, ledger: fence({ "claude-opus-5": resets }) });
    assert.equal(d.status, "exhausted");
    assert.match(d.note, new RegExp(`earliest reset ${resets}`));
  });

  await t.test("an unknown role gets the action defaults", () => {
    const d = decide({ role: "nobody" });
    assert.equal(d.status, "ok");
    assert.equal(d.model, "");
    assert.match(d.note, /no ladder for this role/);
  });

  await t.test("an unrecognized effort is dropped, not fatal", () => {
    const roles = { bdfl: { ladder: ["claude-opus-5"], effort: "ultracode" } };
    const d = decide({ roles });
    assert.equal(d.status, "ok");
    assert.equal(d.effort, "");
  });

  await t.test("a corrupt model ledger fails open rather than idling the seat", () => {
    const d = decide({ ledger: "```json\n[1, 2, 3]\n```" });
    assert.equal(d.model, "claude-opus-5");
  });
});
