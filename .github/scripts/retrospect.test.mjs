// Fixtures for retrospect's budget footer, the display half of the
// allowance-sensing chain. The shape parse is shared with the allowance
// index (.github/scripts/allowance.py, issue #310), whose consumer has
// its own fixtures in agent-audit's action.test.mjs; these cover what
// only the footer does with the readings, because a footer the BDFL
// reads every wake must degrade loudly, never lie or die (PR #308).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("retrospect", sys.argv[1] + "/retrospect")
spec = importlib.util.spec_from_loader("retrospect", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
samples = [("2026-08-28T12:00:00Z", info) for info in json.loads(sys.argv[2])]
mod.budget(samples, None)
`;

const ALARM = /no readable window utilization/;

const fixtures = [
  {
    name: "reads both windows from the nested unifiedWindows shape",
    events: [
      {
        rateLimitType: "five_hour",
        resetsAt: 1_787_933_400,
        unifiedWindows: {
          five_hour: { utilization: 0.17, resetsAt: 1_787_933_400 },
          seven_day: { utilization: 0.22, resetsAt: 1_788_386_400 },
        },
      },
    ],
    expect: [/five_hour: 17% used, 83% left/, /seven_day: 22% used, 78% left/],
  },
  {
    name: "old flat shape still parses",
    events: [{ rateLimitType: "seven_day", utilization: 0.5, resetsAt: 1_788_386_400 }],
    expect: [/seven_day: 50% used, 50% left/],
  },
  {
    name: "boolean utilization alarms instead of printing 100%",
    events: [
      {
        rateLimitType: "five_hour",
        unifiedWindows: { seven_day: { utilization: true, resetsAt: 1_800_000_000 } },
      },
    ],
    expect: [ALARM],
    reject: [/% used/],
  },
  {
    name: "string utilization alarms instead of crashing",
    events: [
      {
        rateLimitType: "five_hour",
        unifiedWindows: { seven_day: { utilization: "0.5", resetsAt: 1_800_000_000 } },
      },
    ],
    expect: [ALARM],
  },
  {
    name: "bad reset degrades to no-reset, the fraction still reports",
    events: [
      {
        rateLimitType: "five_hour",
        unifiedWindows: { seven_day: { utilization: 0.4, resetsAt: "soon" } },
      },
    ],
    expect: [/seven_day: 40% used, 60% left \(no reset time reported\)/],
  },
  {
    name: "a malformed window beside a healthy sibling is named, not dropped",
    events: [
      {
        rateLimitType: "five_hour",
        unifiedWindows: {
          five_hour: { utilization: 0.5, resetsAt: 1_800_000_000 },
          seven_day: { utilization: true, resetsAt: 1_800_000_000 },
        },
      },
    ],
    expect: [
      /five_hour: 50% used, 50% left/,
      /seven_day: present in 1 event\(s\), none readable/,
    ],
  },
  {
    name: "empty unifiedWindows alarms instead of going silent",
    events: [{ rateLimitType: "five_hour", unifiedWindows: {} }],
    expect: [ALARM],
  },
  {
    name: "no events at all keeps the quiet no-events message",
    events: [],
    expect: [/no rate-limit events in the window/],
  },
];

test("retrospect budget footer fixtures", async (t) => {
  for (const fixture of fixtures) {
    await t.test(fixture.name, () => {
      const result = spawnSync(
        "python3",
        ["-c", driver, scriptsDir, JSON.stringify(fixture.events)],
        { encoding: "utf8" },
      );
      assert.equal(result.status, 0, result.stderr || result.stdout);
      for (const pattern of fixture.expect) {
        assert.match(result.stdout, pattern);
      }
      for (const pattern of fixture.reject ?? []) {
        assert.doesNotMatch(result.stdout, pattern);
      }
    });
  }
});
