// Fixtures for retrospect's budget footer, the display half of the
// allowance-sensing chain. The shape parse is `allowance.window_readings`
// (.github/scripts/allowance.py, issue #310), shared with the guard's
// allowance ledger and the allowance table, whose consumers have their own
// fixtures in agent-audit's action.test.mjs and session-row.test.mjs; these
// cover what only the footer does with the readings, because a footer the
// BDFL reads every wake must degrade loudly, never lie or die (PR #308).
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
# The footer measures every span from the present, so a fixture must pin
# the present or its expected hours move with the wall clock. argv[3] is
# that instant; absent, it is the last reading, the zero-staleness case
# the pre-#584 footer assumed unconditionally.
now = mod.stamp_to_dt(sys.argv[3]) if len(sys.argv) > 3 else (
    mod.stamp_to_dt(max(when for when, _ in samples)) if samples else None
)
mod.budget(samples, None, now)
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
      // The shed order is the operator's, and it changed once (#897): the
      // director's queue item was the first cut from 2026-09-13 and is the
      // third since 2026-10-03. The survey stays last either way.
      /Shed cheapest loss first: the retrospect deep-read, then the digest's depth, then the agent-system queue item; a due deep survey last/,
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
  {
    // The defect this file's `now` exists for. Run 35290133931 read a
    // five-hour window that had reset 52 minutes earlier and printed
    // "55% left ... (2.5h)": headroom in a window that no longer existed,
    // because time-to-reset was measured from the reading, not the clock.
    name: "a window whose reset has passed reports the closed window, not headroom",
    events: [{ when: "2026-09-12T19:26:00Z", info: windows(0.45, 0.24) }],
    now: "2026-09-13T00:52:00Z",
    expect: [
      /five_hour: reset 2026-09-13T00:00:00Z, 0\.9h ago; the last reading \(45% used, at 2026-09-12T19:26:00Z\) describes the window that closed\./,
    ],
    reject: [/five_hour: 45% used/, /55% left/, /\(2\.5h\)/],
  },
  {
    // Same staleness on a live window: the reset is real headroom, measured
    // from now, and `used` is named as the floor it is.
    name: "a stale reading on a live window measures from now and calls used a floor",
    events: [{ when: "2026-09-12T19:26:00Z", info: windows(0.45, 0.24) }],
    now: "2026-09-12T22:26:00Z",
    expect: [
      /five_hour: 45% used, 55% left, resets 2026-09-13T00:00:00Z \(1\.6h\); reading 3\.0h old, so used is a floor/,
      /seven_day: 24% used, 76% left, resets 2026-09-17T02:00:00Z \(99\.6h\); reading 3\.0h old, so used is a floor/,
    ],
    reject: [/\(4\.6h\)/],
  },
  {
    // A reading under STALE_HOURS says nothing about its own age: an
    // unconditional clause would be filler on every healthy run.
    name: "a fresh reading carries no staleness clause",
    events: [{ when: "2026-09-12T19:26:00Z", info: windows(0.45, 0.24) }],
    now: "2026-09-12T20:00:00Z",
    expect: [/five_hour: 45% used, 55% left, resets 2026-09-13T00:00:00Z \(4\.0h\)/],
    reject: [/so used is a floor/],
  },
  {
    // A SLOW DOWN from a window the guard has already rolled over would
    // throttle a seat that has its full allowance back.
    name: "the governor stays silent on a governed window that already reset",
    events: [{ when: "2026-09-16T14:00:00Z", info: windows(0.05, 0.9) }],
    now: "2026-09-17T03:00:00Z",
    expect: [/seven_day: reset 2026-09-17T02:00:00Z, 1\.0h ago/],
    reject: [/SLOW DOWN/, /governor:/, /90% used, 10% left/],
  },
];

test("retrospect budget footer fixtures", async (t) => {
  for (const fixture of fixtures) {
    await t.test(fixture.name, () => {
      const result = spawnSync(
        "python3",
        [
          "-c",
          driver,
          scriptsDir,
          JSON.stringify(fixture.events),
          ...(fixture.now ? [fixture.now] : []),
        ],
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

// classify() separates the runs that held no session, a guard-skipped run, a
// phantom GitHub never gave a job (#553), a script workflow with no Claude
// step and a cancelled run the debounce displaced (ADR-0014), from one that
// died before its audit step and from one the runner stopped under; none has
// an artifact. The shapes are real, probed on 2026-09-17: a skipped BDFL run
// (35165527435), a session that died (the reviewer on #564, 34970585258), an
// alarm run with no Claude step at all (34970888339) and a zero-job phantom
// on PR #571 (35205833235); and on 2026-10-25: four heartbeats cancelled at
// 47 to 80 minutes, Claude step `failure` on exit 143 with every step after
// it skipped (40192857205 and three more, #1661), and a review the debounce
// displaced before its session, Claude step `skipped` (40181349615). Before
// this, forty skipped runs after an outage printed as forty deaths (#525),
// the phantom printed as a death whose log to go read, when it has no log,
// and the four killed heartbeats printed as nothing at all.
const classifyDriver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("retrospect", sys.argv[1] + "/retrospect")
spec = importlib.util.spec_from_loader("retrospect", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
jobs = json.loads(sys.argv[3])
print(json.dumps(mod.classify(jobs, sys.argv[4]) if sys.argv[2] == "classify" else mod.claude_seconds(jobs)))
`;

// gh api ... --jq already applied the selector; `claude` is its answer, one
// object per Claude step, a bare string standing for a step with no stamps.
function probed(claude, { jobs = 1, stepless = 0 } = {}) {
  const steps = claude.map((c) => (typeof c === "string" ? { conclusion: c } : c));
  return JSON.stringify({ jobs, stepless, claude: steps });
}

function classify(claude, { conclusion = "failure", ...shape } = {}) {
  const proc = spawnSync(
    "python3",
    ["-c", classifyDriver, scriptsDir, "classify", probed(claude, shape), conclusion],
    { encoding: "utf8" },
  );
  assert.equal(proc.status, 0, proc.stderr);
  return JSON.parse(proc.stdout.trim());
}

function claudeSeconds(claude) {
  const proc = spawnSync(
    "python3",
    ["-c", classifyDriver, scriptsDir, "seconds", probed(claude), "cancelled"],
    { encoding: "utf8" },
  );
  assert.equal(proc.status, 0, proc.stderr);
  return JSON.parse(proc.stdout.trim());
}

const KILLED = { conclusion: "failure", started_at: "2026-10-25T21:23:21Z", completed_at: "2026-10-25T22:11:37Z" };

test("a guard-skipped run: the Claude step concluded skipped", () => {
  assert.equal(classify(["skipped"]), "skipped");
});

test("a session that died is not skipped, whatever its post step says", () => {
  assert.equal(classify(["failure", "success"]), null);
});

test("a run with jobs and no Claude step is a script workflow, not a death", () => {
  assert.equal(classify([], { jobs: 3 }), "script");
});

test("a job with no steps on a run that is not cancelled tells nothing, so it stays a death", () => {
  assert.equal(classify([], { stepless: 1 }), null);
});

test("run 35205833235: zero jobs is a phantom, not a death with a log to read", () => {
  assert.equal(classify([], { jobs: 0 }), "phantom");
});

test("run 40235230221: cancelled with the Claude step failed is a killed session", () => {
  assert.equal(classify([KILLED], { conclusion: "cancelled" }), "killed");
});

test("run 40181349615: cancelled with the Claude step skipped is the debounce, not a skip to count", () => {
  assert.equal(classify(["skipped"], { conclusion: "cancelled" }), "debounced");
});

test("run 37370936150: cancelled with a job that got no steps is the debounce, not a death", () => {
  assert.equal(classify([], { stepless: 1, conclusion: "cancelled" }), "debounced");
});

test("a cancelled run with no job at all is the debounce, not a phantom", () => {
  assert.equal(classify([], { jobs: 0, conclusion: "cancelled" }), "debounced");
});

test("a killed session's length is its Claude step's span, from the jobs API's stamps", () => {
  assert.equal(claudeSeconds([KILLED]), 48 * 60 + 16);
});

test("a Claude step without stamps adds no time", () => {
  assert.equal(claudeSeconds(["skipped", KILLED]), 48 * 60 + 16);
});

// A killed run's line ends with its newest vitals row (#1665): the sidecar
// agent-heartbeat.yml runs beside the session writes one a minute, so the
// newest is the post-mortem the runner used to take with it.
const vitalsPrelude = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("retrospect", sys.argv[1] + "/retrospect")
spec = importlib.util.spec_from_loader("retrospect", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
`;
const vitalsDriver = `${vitalsPrelude}
print(mod.vitals_line(json.loads(sys.argv[2])))
`;
// last_vitals with the query injected: argv[2] is the rows it returns, or
// "unreadable" for a database that raises d1.Unreadable.
const lastVitalsDriver = `${vitalsPrelude}
def query(sql, params):
    assert params == [40263802968], params
    if sys.argv[2] == "unreadable":
        raise mod.d1.Unreadable("D1 answered 500")
    return json.loads(sys.argv[2])
print(mod.last_vitals("40263802968", query))
`;

function vitalsLine(rows) {
  const proc = spawnSync("python3", ["-c", vitalsDriver, scriptsDir, JSON.stringify(rows)], { encoding: "utf8" });
  assert.equal(proc.status, 0, proc.stderr);
  return proc.stdout.trim();
}

test("a killed run's last minute is its newest vitals row on one line: GB past a gigabyte, MB below", () => {
  const row = {
    at: "2026-10-26T03:38:21Z",
    mem_avail_mb: 409,
    swap_free_mb: 12,
    disk_avail_mb: 81234,
    load1: 9.12,
    top_rss_mb: 6400,
    top_name: "rustc",
  };
  assert.equal(
    vitalsLine([row]),
    "2026-10-26T03:38:21Z  mem avail 409 MB, swap free 12 MB, disk avail 79.3 GB, load 9.12, heaviest rustc 6.2 GB",
  );
});

test("a field the sampler could not read prints as ?, never as 0", () => {
  const row = {
    at: "2026-10-26T03:38:21Z",
    mem_avail_mb: null,
    swap_free_mb: 12,
    disk_avail_mb: 81234,
    load1: null,
    top_rss_mb: null,
    top_name: null,
  };
  assert.equal(
    vitalsLine([row]),
    "2026-10-26T03:38:21Z  mem avail ?, swap free 12 MB, disk avail 79.3 GB, load ?, heaviest ? ?",
  );
});

function lastVitals(answer) {
  const proc = spawnSync("python3", ["-c", lastVitalsDriver, scriptsDir, answer], { encoding: "utf8" });
  assert.equal(proc.status, 0, proc.stderr);
  return proc.stdout.trim();
}

test("a vitals table that cannot be read is UNREADABLE with the reason, never the calm no-row line", () => {
  assert.equal(
    lastVitals("unreadable"),
    "UNREADABLE (D1 answered 500); the vitals table could not be read.",
  );
});

test("last_vitals asks for the run's id and prints the newest row's line", () => {
  const row = {
    at: "2026-10-26T03:38:21Z",
    mem_avail_mb: 409,
    swap_free_mb: 12,
    disk_avail_mb: 512,
    load1: 1,
    top_rss_mb: 7,
    top_name: "claude",
  };
  assert.equal(
    lastVitals(JSON.stringify([row])),
    "2026-10-26T03:38:21Z  mem avail 409 MB, swap free 12 MB, disk avail 512 MB, load 1.00, heaviest claude 7 MB",
  );
});

test("no vitals row says so, never a calm line", () => {
  assert.equal(vitalsLine([]), "no vitals row (a kill before #1665's sidecar, or one it could not write)");
});

// The session line's "denied N (tool)" names no wall; run 35842553330's
// (2026-09-23) was the harness's protected-path check on .cargo/, and the
// BDFL learned that from the docs, not the retrospect. Each distinct typed
// reason the audit kept prints under the session; an artifact from before
// the field existed prints the flag alone, no crash, no empty line.
const describeDriver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("retrospect", sys.argv[1] + "/retrospect")
spec = importlib.util.spec_from_loader("retrospect", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
run = {"event": "workflow_dispatch", "conclusion": "success",
       "updatedAt": "2026-09-23T09:24:10Z", "url": "https://example.test/run"}
mod.describe(run, json.loads(sys.argv[2]), "OK to proceed?")
`;

function describeWith(denials) {
  const meta = { role: "maintainer", telemetry: { num_turns: 14, permission_denials: denials } };
  const proc = spawnSync("python3", ["-c", describeDriver, scriptsDir, JSON.stringify(meta)], {
    encoding: "utf8",
  });
  assert.equal(proc.status, 0, proc.stderr);
  return proc.stdout;
}

test("a denial's wall prints under the session line", () => {
  const out = describeWith({
    count: 1,
    tools: ["Edit"],
    reasons: [{ tool: "Edit", type: "protectedPath", reason: ".cargo/ is protected" }],
  });
  assert.match(out, /\[denied 1 \(Edit\)\]/);
  assert.match(out, /^    denied Edit: protectedPath: \.cargo\/ is protected$/m);
});

test("an artifact without reasons prints the flag alone", () => {
  const out = describeWith({ count: 1, tools: ["Edit"] });
  assert.match(out, /\[denied 1 \(Edit\)\]/);
  assert.doesNotMatch(out, /^    denied Edit:/m);
});

// Issue #725: the offered set is the proof an envelope flag did what it
// says, and the use counts are what a per-seat trim starts from. Both
// print under the session; an artifact from before the record prints
// nothing, no crash, no empty line.
function describeWithTools(tools) {
  const meta = { role: "reviewer", telemetry: { num_turns: 14, tools } };
  const proc = spawnSync("python3", ["-c", describeDriver, scriptsDir, JSON.stringify(meta)], {
    encoding: "utf8",
  });
  assert.equal(proc.status, 0, proc.stderr);
  return proc.stdout;
}

test("offered tools and use counts print under the session line", () => {
  const out = describeWithTools({ offered: ["Bash", "Read", "Write"], used: { Read: 3, Bash: 9 } });
  assert.match(out, /^    tools used Bash 9, Read 3; offered 3 \(Bash, Read, Write\)$/m);
});

test("an unknown offered set says so, and no uses say none", () => {
  const out = describeWithTools({ offered: null, used: {} });
  assert.match(out, /^    tools used none; offered unknown$/m);
});

test("an artifact without a tools record prints no tools line", () => {
  const out = describeWith({ count: 0, tools: [] });
  assert.doesNotMatch(out, /^    tools /m);
});

// The fleet-share line (#896 item 6). SLOW DOWN says shed; this line says
// whether shedding reaches the cause. The subscription is shared with the
// operator's own sessions, so the window can move without a session row:
// on 2026-10-04 a burst moved 31 points while the fleet spent a tenth of
// the week's rate on them, and the shed that followed could not hold it.
// The share is a bound, (interval cost / window cost) / (burned / used),
// with no dollar price per point; usage outside the factory inflates only
// the denominators. Every branch prints one line, UNREADABLE named.
const shareDriver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("retrospect", sys.argv[1] + "/retrospect")
spec = importlib.util.spec_from_loader("retrospect", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
case = json.loads(sys.argv[2])
def query(sql, params=()):
    if case["window_usd"] == "unreadable":
        raise mod.d1.Unreadable("HTTP 500: edge")
    assert "SUM(cost_usd)" in sql and "sessions" in sql, sql
    assert list(params) == [case["opened"]], params
    return [{"usd": case["window_usd"]}]
mod.share(case["opened"], case["burned"], case["used"], case["spent"], query)
`;

const WINDOW_OPENED = "2026-10-01T02:00:00Z";
const shareCases = [
  {
    // The 2026-10-04 burst, the numbers of #896's comment of 2026-10-11:
    // 31 of the week's 32 points, $21 of its $224.
    name: "a burst the fleet did not make is at most a tenth the fleet's",
    case: { opened: WINDOW_OPENED, burned: 0.31, used: 0.32, spent: [21.0], window_usd: 224.0 },
    expect: [
      /fleet share: at most 10% of the 31\.0% burned is the fleet's \(\$21\.00 of the window's \$224\.00 notional, 9\.4%, against 31\.0 of its 32 points, 96\.9%\); the rest is usage outside the factory, which shedding cannot reach\./,
    ],
  },
  {
    // The fleet's spend share exceeds the burn's share of the window: the
    // bound passes one and says so, never a percentage over 100.
    name: "a burn the fleet's own spend can account for is at most all the fleet's",
    case: { opened: WINDOW_OPENED, burned: 0.005, used: 0.45, spent: [2.0, 2.5], window_usd: 224.0 },
    expect: [
      /fleet share: at most all of the 0\.5% burned is the fleet's \(\$4\.50 of the window's \$224\.00 notional, 2\.0%, against 0\.5 of its 45 points, 1\.1%\); the fleet cannot be ruled out as the cause, so shedding may reach it\./,
    ],
    reject: [/at most \d/, /1\d\d%/, /is all/],
  },
  {
    // :.0% rounds 0.996 to 100%, a percentage the "all" branch exists to avoid.
    name: "a bound that rounds to 100% reads as all, never as 100%",
    case: { opened: WINDOW_OPENED, burned: 0.005, used: 0.5, spent: [0.996], window_usd: 100.0 },
    expect: [/fleet share: at most all of the 0\.5% burned is the fleet's/],
    reject: [/at most 100%/],
  },
  {
    name: "a bound just under that prints its percentage",
    case: { opened: WINDOW_OPENED, burned: 0.005, used: 0.5, spent: [0.99], window_usd: 100.0 },
    expect: [/fleet share: at most 99% of the 0\.5% burned is the fleet's/],
  },
  {
    name: "a window with no use since it opened spends no read",
    case: { opened: WINDOW_OPENED, burned: 0.01, used: 0.0, spent: [1.0], window_usd: "unreadable" },
    expect: [/fleet share: the window shows no use since it opened; nothing to compare against\./],
    reject: [/UNREADABLE/],
  },
  {
    name: "a table that cannot be read says UNREADABLE, never a share",
    case: { opened: WINDOW_OPENED, burned: 0.31, used: 0.32, spent: [21.0], window_usd: "unreadable" },
    expect: [/fleet share: UNREADABLE \(HTTP 500: edge\); the session table could not be read\./],
    reject: [/at most/, /all of/],
  },
  {
    name: "no priced session audited: nothing to apportion, no read spent",
    case: { opened: WINDOW_OPENED, burned: 0.31, used: 0.32, spent: [], window_usd: "unreadable" },
    expect: [/fleet share: no audited session carried a price; nothing to apportion\./],
  },
  {
    name: "a window with no priced row yet has nothing to compare against",
    case: { opened: WINDOW_OPENED, burned: 0.02, used: 0.02, spent: [1.0], window_usd: null },
    expect: [
      /fleet share: no priced session since the window opened \(2026-10-01T02:00:00Z\); nothing to compare against\./,
    ],
  },
  {
    name: "a window that did not move at reporting precision apportions nothing",
    case: { opened: WINDOW_OPENED, burned: 0.0, used: 0.45, spent: [1.0], window_usd: 224.0 },
    expect: [
      /fleet share: the window did not move at reporting precision over the audited minutes; nothing to apportion\./,
    ],
  },
];

test("the fleet-share line", async (t) => {
  for (const { name, case: c, expect, reject = [] } of shareCases) {
    await t.test(name, () => {
      const r = spawnSync("python3", ["-c", shareDriver, scriptsDir, JSON.stringify(c)], { encoding: "utf8" });
      assert.equal(r.status, 0, r.stderr || r.stdout);
      for (const pattern of expect) assert.match(r.stdout, pattern);
      for (const pattern of reject) assert.doesNotMatch(r.stdout, pattern);
    });
  }
});

// Plumbing: the line rides the governed window's burn observation, under
// its governor line and nowhere else, and the window it prices opened one
// WEEK before the reset the reading carries. $9 of $224 is 4.0% of the
// spend against 1.0 of 33 points, 3.0% of the movement: the bound passes one.
const plumbingDriver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("retrospect", sys.argv[1] + "/retrospect")
spec = importlib.util.spec_from_loader("retrospect", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
samples = [(e["when"], e["info"]) for e in json.loads(sys.argv[2])]
asked = []
def query(sql, params=()):
    asked.append(list(params))
    return [{"usd": 224.0}]
mod.budget(samples, None, mod.stamp_to_dt(sys.argv[3]), spent=[9.0], query=query)
print("asked", json.dumps(asked))
`;

test("the fleet-share line rides the governed window's observation alone", () => {
  const events = [
    { when: "2026-09-12T19:00:00Z", info: windows(0.19, 0.32) },
    { when: "2026-09-12T19:16:00Z", info: windows(0.3, 0.33) },
  ];
  const r = spawnSync(
    "python3",
    ["-c", plumbingDriver, scriptsDir, JSON.stringify(events), "2026-09-12T19:24:00Z"],
    { encoding: "utf8" },
  );
  assert.equal(r.status, 0, r.stderr || r.stdout);
  assert.match(
    r.stdout,
    /seven_day: 33% used[^\n]*burned 1\.0% over the 16 min audited\n\s+governor:[^\n]*\n\s+fleet share: at most all of the 1\.0% burned is the fleet's/,
  );
  assert.doesNotMatch(r.stdout, /five_hour[^\n]*\n\s+fleet share/);
  // SEVEN_DAY_RESET is 2026-09-17T02:00:00Z; the window opened a WEEK before.
  assert.match(r.stdout, /asked \[\["2026-09-10T02:00:00Z"\]\]/);
});

test("a budget with no spent sessions still prints the share line, degraded", () => {
  const events = [
    { when: "2026-09-12T19:00:00Z", info: windows(0.19, 0.32) },
    { when: "2026-09-12T19:16:00Z", info: windows(0.3, 0.33) },
  ];
  const r = spawnSync(
    "python3",
    ["-c", driver, scriptsDir, JSON.stringify(events), "2026-09-12T19:24:00Z"],
    { encoding: "utf8" },
  );
  assert.equal(r.status, 0, r.stderr || r.stdout);
  assert.match(r.stdout, /fleet share: no audited session carried a price; nothing to apportion\./);
});

// The numerator's source: `spend` prices one artifact by the session table's
// own rule, and an artifact in an older shape is unpriced, never fatal.
const spendDriver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("retrospect", sys.argv[1] + "/retrospect")
spec = importlib.util.spec_from_loader("retrospect", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
print(json.dumps(mod.spend(json.loads(sys.argv[2]))))
`;

const spendOf = (meta) => {
  const r = spawnSync("python3", ["-c", spendDriver, scriptsDir, JSON.stringify(meta)], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr || r.stdout);
  return JSON.parse(r.stdout);
};

test("spend prices an artifact by the session table's rule", () => {
  const usage = { a: { costUSD: 1.5 }, b: { costUSD: 2.0, costBasis: "list" }, c: { costUSD: 9.0, costBasis: "api" } };
  assert.equal(spendOf({ telemetry: { modelUsage: usage } }), 3.5);
});

test("spend leaves an artifact in an older shape unpriced", async (t) => {
  const legacy = [
    ["no telemetry", {}],
    ["telemetry null", { telemetry: null }],
    ["list-shaped telemetry", { telemetry: [{ tool_name: "Bash" }] }],
    ["list-shaped modelUsage", { telemetry: { modelUsage: [{ costUSD: 1.0 }] } }],
    ["list-shaped denials beside a price", { telemetry: { permission_denials: [{ tool_name: "Bash" }] } }],
    ["a non-list costBasis entry", { telemetry: { modelUsage: { a: "x", b: { costUSD: "1", costBasis: ["list"] } } } }],
  ];
  for (const [name, meta] of legacy) await t.test(name, () => assert.equal(spendOf(meta), null));
});
