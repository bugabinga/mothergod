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
# An event is a rate_limit_info dict, or {"when", "info"} when the fixture
# needs the sessions spaced in time (the #533 burst is a delta over minutes).
samples = [
    (e["when"], e["info"]) if "info" in e else ("2026-08-28T12:00:00Z", e)
    for e in json.loads(sys.argv[2])
]
mod.budget(samples, None)
`;

// Issue #533's minute, verbatim: the seven-day window opened 2026-09-10 02:00
// UTC and resets 2026-09-17 02:00 UTC; the five-hour resets at midnight.
const SEVEN_DAY_RESET = 1_789_610_400;
const FIVE_HOUR_RESET = 1_789_257_600;
const windows = (fiveHour, sevenDay) => ({
  rateLimitType: "five_hour",
  unifiedWindows: {
    five_hour: { utilization: fiveHour, resetsAt: FIVE_HOUR_RESET },
    seven_day: { utilization: sevenDay, resetsAt: SEVEN_DAY_RESET },
  },
});

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
  {
    // #533: a post-pause catch-up burst. Sixteen audited minutes burn 11% of
    // the five-hour and 1% of the seven-day, which the old two-reading delta
    // extrapolated to exhaustion in 18 hours. The governor, on the same
    // reading, projects 82% for the window and throttles nothing.
    name: "a burst in the audited minutes is an observation, not a verdict (#533)",
    events: [
      { when: "2026-09-12T19:10:00Z", info: windows(0.08, 0.32) },
      { when: "2026-09-12T19:26:00Z", info: windows(0.19, 0.33) },
    ],
    expect: [
      /five_hour: 19% used, 81% left, resets 2026-09-13T00:00:00Z \(4\.6h\); burned 11\.0% over the 16 min audited/,
      /seven_day: 33% used, 67% left, resets 2026-09-17T02:00:00Z \(102\.6h\); burned 1\.0% over the 16 min audited\n\s+governor: week-average 0\.50%\/h, 0\.65%\/h reaches the reset; projection reaches the reset, every seat in its normal tier\./,
    ],
    reject: [/SLOW DOWN/, /five_hour[^\n]*\n\s+governor/, /exhausts in/],
  },
  {
    // The governor's own miss: 90% spent with half the window elapsed, so
    // the footer says what the guard is doing about it, and then SLOW DOWN.
    name: "a week-average that misses the reset carries the governor's verdict",
    events: [
      {
        when: "2026-09-13T14:00:00Z",
        info: windows(0.05, 0.9),
      },
    ],
    expect: [
      /seven_day: 90% used, 10% left/,
      /governor: week-average 1\.07%\/h, 0\.12%\/h reaches the reset; projected exhaustion 2026-09-13T23:2\d:\d\dZ misses it by 74\.\dh\. Thrift tiers on, discretionary wakes being skipped\./,
      /SLOW DOWN: cut discretionary work/,
    ],
    reject: [/five_hour[^\n]*\n\s+governor/],
  },
  {
    // The five-hour window has no governor. It exhausts and resets in hours;
    // the imperative is reserved for the window whose exhaustion is a week.
    name: "the five-hour window never shouts, however hot the burst",
    events: [
      { when: "2026-09-12T19:10:00Z", info: windows(0.08, 0.1) },
      { when: "2026-09-12T19:26:00Z", info: windows(0.6, 0.1) },
    ],
    expect: [/five_hour: 60% used, 40% left[^\n]*; burned 52\.0% over the 16 min audited/],
    reject: [/SLOW DOWN/],
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
