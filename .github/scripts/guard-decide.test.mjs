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
import { readFileSync } from "node:fs";
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

// Wake times are what the second gear reads, so fixtures name hours, not
// counts. Hour 0 through 5 fall inside the 25% keep window, 6 through 23 do not.
const at = (hour, minute = 11) => 1_700_000_000 - (1_700_000_000 % 86400) + hour * 3600 + minute * 60;

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

  await t.test("skips a wake outside the keep window, and says the numbers", () => {
    const skipped = decide({ allowance, now: at(9), discretionary: true });
    assert.equal(skipped.status, "skip");
    assert.match(skipped.note, /^SKIP: discretionary wake/);
    assert.match(skipped.note, /keeping the first 25% of each day/);
    assert.match(skipped.note, /this wake is at 2023-11-14 09:11 UTC/);
    assert.match(skipped.note, /week-average \d+\.\d\d%\/h/);
    assert.match(skipped.note, /misses reset 2023-11-14 22:13 UTC/);
  });

  await t.test("is a throttle, not a stop: the keep window still runs", () => {
    const kept = decide({ allowance, now: at(4), discretionary: true });
    assert.notEqual(kept.status, "skip");
    // What survives still gets the thrift tier: both gears at once.
    assert.equal(kept.model, "claude-sonnet-5");
    assert.match(kept.note, /THRIFT/);
  });

  await t.test("the wake stream's shape cannot alias the decimation", () => {
    // The bug this replaced, found in review of this PR. Decimation used to
    // count runs, and `github.run_number` advances on EVERY wake of a
    // workflow, not just the discretionary ones. One interleaved operator
    // wake per tick put the whole cron on odd run numbers, where a
    // keep-every-fourth rule kept none of them: total starvation wearing the
    // label of a 25% floor. A time-domain rule cannot have this bug because
    // it never reads the stream, so the assertion is that identical clocks
    // decide identically however many other wakes happened in between.
    for (const hour of [0, 3, 5, 6, 12, 23]) {
      const first = decide({ allowance, now: at(hour), discretionary: true });
      const second = decide({ allowance, now: at(hour), discretionary: true });
      assert.equal(first.status, second.status, `hour ${hour} must not depend on history`);
    }
  });

  await t.test("no governed seat can go a day without a wake", () => {
    // The keep floor is a bound on the GAP, and that bound only holds while
    // the seats tick faster than the keep window is wide. The cadence lives
    // in wrangler.toml and moves with the allowance lever, so the invariant
    // is asserted against the real crons rather than against a fixture: a
    // lever pull that would starve a seat fails here instead of in silence.
    const wrangler = readFileSync(
      new URL("../../infra/telegram-worker/wrangler.toml", import.meta.url),
      "utf8",
    );
    const crons = wrangler.match(/^crons\s*=\s*\[(.*)\]/m)[1]
      .split(",")
      .map((entry) => entry.trim().replace(/^["']|["']$/g, ""));
    // "<minute> */<n> * * *" is the only shape the clock uses; a governed seat
    // written any other way needs this test taught how to read it.
    const governed = ["11 */4 * * *", "22 */3 * * *"];
    for (const cron of governed) {
      assert.ok(crons.includes(cron), `${cron} must still be a live trigger`);
      const [minute, hours] = cron.split(" ");
      const step = Number(hours.replace("*/", ""));
      const ticks = [];
      for (let hour = 0; hour < 24; hour += step) ticks.push(at(hour, Number(minute)));
      const kept = ticks.filter((now) => decide({ allowance, now, discretionary: true }).status !== "skip");
      assert.ok(kept.length >= 1, `${cron} keeps ${kept.length} wakes a day at the floor`);
    }
  });

  await t.test("throttles proportionally, not as an on/off switch", () => {
    // A 1% overshoot must not cost a quarter of the day. 50.5% spent at the
    // halfway mark keeps ~98% of it, so only a wake in the last half hour goes.
    const gentle = fence(missing(0.505));
    const kept = Array.from(
      { length: 24 },
      (_, hour) => decide({ allowance: gentle, now: at(hour), discretionary: true }),
    ).filter((d) => d.status !== "skip");
    assert.ok(kept.length >= 23, `kept ${kept.length} of 24 hours`);
  });
});

test("second gear never touches the responsive path (ADR-0039)", async (t) => {
  const allowance = fence(missing());
  // Same reading, same clock: an hour that IS skipped when discretionary.
  const now = at(9);

  await t.test("an operator dispatch or event wake runs", () => {
    const d = decide({ allowance, now, discretionary: false });
    assert.equal(d.status, "ok");
    assert.match(d.note, /THRIFT/, "thrift still applies; only the skip does not");
  });

  await t.test("a reviewer run runs", () => {
    const d = decide({ role: "reviewer", allowance, now, discretionary: false });
    assert.equal(d.status, "ok");
    assert.equal(d.model, "claude-sonnet-5");
  });

  await t.test("a discretionary wake under a healthy allowance runs", () => {
    // 10% spent at the halfway mark reaches the reset with room to spare.
    const healthy = fence(missing(0.1));
    const d = decide({ allowance: healthy, now, discretionary: true });
    assert.equal(d.status, "ok");
    assert.equal(d.model, "claude-opus-5", "no thrift either");
    assert.doesNotMatch(d.note, /THRIFT|SKIP/);
  });
});

test("a projection nobody can trust never costs a wake", async (t) => {
  // The hour that IS skipped on a usable reading, so a fixture that runs here
  // proves the reading was rejected rather than that the clock saved it.
  const now = at(9);
  const cases = {
    "no allowance ledger at all": "",
    "no json fence in the body": "the ledger issue, prose only",
    "unparseable json": "```json\n{not json,}\n```",
    "missing fields": fence({ utilization: 0.9 }),
    "a lapsed window": fence({ ...missing(), observedAt: RESETS + 60 }),
    // Both boundary readings divide by zero without the `elapsed <= 0 or
    // remaining <= 0` guard, and review of this PR showed removing that guard
    // failed nothing: it was load-bearing and untested, which is the same
    // shape of hole as an untested rescue.
    "a reading taken at the window's first instant": fence({
      observedAt: RESETS - 604800,
      resetsAt: RESETS,
      utilization: 0.9,
    }),
    "a reading taken at the reset with the allowance overspent": fence({
      observedAt: RESETS,
      resetsAt: RESETS,
      utilization: 1.1,
    }),
    "zero burn": fence(missing(0)),
    "a string where a number belongs": fence({ ...missing(), utilization: "high" }),
  };
  for (const [name, allowance] of Object.entries(cases)) {
    await t.test(name, () => {
      const d = decide({ allowance, now, discretionary: true });
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
