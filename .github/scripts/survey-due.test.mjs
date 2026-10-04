// Fixtures for survey-due, the detector the survey's trigger never had.
//
// The first assertion is the one the script exists for: an ops log that
// cannot be read must answer DUE. The bug that produced this script (#558)
// was a run announcing that no survey had happened twelve hours after one
// had, so both directions of the wrong answer are pinned here, and the
// asymmetry between them is deliberate: a wrong DUE costs a duplicated
// survey, a wrong RAN loses the week's most expensive duty in silence.
//
// The second bug (#910) is pinned by the weekday fixtures: a Sunday shed
// under SLOW DOWN used to lose its week, because every later wake answered
// from the calendar alone. Now Monday is due by the seventh-day clause.
//
// `render` and `newest` are pure by construction (no network, no clock:
// `today` and the rows are arguments) precisely so these can exist. A
// detector nobody can make fire is decoration.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, io, json, sys
from datetime import date
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("survey_due", sys.argv[1] + "/survey-due")
spec = importlib.util.spec_from_loader("survey_due", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
today = date.fromisoformat(raw["today"])
if fn == "render":
    buf = io.StringIO()
    mod.render(today, raw["trigger"], raw["rows"], raw["error"], out=buf)
    print(json.dumps(buf.getvalue()))
else:
    hit = mod.newest(raw["rows"], today)
    print(json.dumps(None if hit is None else [hit[0].isoformat(), hit[1]["created_at"]]))
`;

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const SUNDAY = "2026-09-20";
const MONDAY = "2026-09-21";

const digest = (over = {}) => ({
  created_at: "2026-09-20T00:38:19Z",
  html_url: "https://github.com/bugabinga/mothergod/issues/3#issuecomment-1",
  body: `**Deep survey, Sunday ${SUNDAY}**\n\n<!-- deep-survey ${SUNDAY} -->`,
  ...over,
});

const marker = (day) => digest({ body: `<!-- deep-survey ${day} -->` });

const show = (over = {}) => call("render", { today: SUNDAY, trigger: "schedule", rows: [], error: null, ...over });

test("an unreadable ops log answers due, never ran", () => {
  const out = show({ rows: null, error: "gh: Not Found (HTTP 404)" });
  assert.match(out, /survey-due: due/);
  assert.doesNotMatch(out, /ran /);
  assert.match(out, /UNREADABLE, which is not the same as no survey/);
  // The footer is what a skimming reader takes away. It must not claim a read.
  assert.match(out, /ops-log unreadable/);
  assert.doesNotMatch(out, /ops-log readable/);
});

test("a Sunday with no marker is due by the Sunday clause", () => {
  const out = show({ rows: [{ body: "an ordinary digest", created_at: "x" }] });
  assert.match(out, /survey-due: due \(Sunday clause: no deep-survey marker dated today\)/);
  assert.match(out, /ops-log readable \| markers 0/);
});

test("a marker for this Sunday reports ran, with when", () => {
  const out = show({ rows: [digest()] });
  assert.match(out, /survey-due: ran 2026-09-20T00:38:19Z/);
  assert.match(out, /newest 2026-09-20 \| ops-log readable \| markers 1/);
  // A duty already done prints no instructions. Quiet is the result.
  assert.doesNotMatch(out, /DUE/);
  assert.doesNotMatch(out, /deep-survey` skill/);
});

test("last week's marker does not satisfy this Sunday", () => {
  // The 2026-09-13 failure, inverted: a real marker, wrong Sunday. Matching
  // on the word `deep-survey` alone would answer ran and skip the week.
  const out = show({ rows: [marker("2026-09-13")] });
  assert.match(out, /survey-due: due \(Sunday clause/);
  // It is still a marker: the liveness count says the detector is working.
  assert.match(out, /newest 2026-09-13 .* markers 1/);
});

test("a shed Sunday makes Monday due by the seventh-day clause", () => {
  // The #910 week: 2026-09-20 surveyed, 2026-09-27 shed under SLOW DOWN.
  // Monday 2026-09-28 used to read not-due from the calendar alone.
  const out = show({ today: "2026-09-28", rows: [marker(SUNDAY)] });
  assert.match(out, /survey-due: due \(seventh-day clause: newest marker 2026-09-20 is 8 days old\)/);
  assert.match(out, /DUE on Monday 2026-09-28/);
  assert.match(out, /today 2026-09-28 Mon/);
});

test("a weekday with no marker in the window is due by the seventh-day clause", () => {
  const out = show({ today: MONDAY, rows: [{ body: "an ordinary digest", created_at: "x" }] });
  assert.match(out, /survey-due: due \(seventh-day clause: no deep-survey marker in the 8 days read\)/);
  assert.match(out, /newest - \| ops-log readable \| markers 0/);
});

test("a weekday inside the week reports ran and prints no instructions", () => {
  const out = show({ today: "2026-09-26", rows: [digest()] });
  assert.match(out, /survey-due: ran 2026-09-20T00:38:19Z/);
  assert.doesNotMatch(out, /DUE/);
});

test("a survey healed on Monday makes the next Sunday due by the Sunday clause", () => {
  // The anchor holds: the heal costs one short week, once.
  const out = show({ today: "2026-10-04", rows: [marker("2026-09-28")] });
  assert.match(out, /survey-due: due \(Sunday clause/);
  assert.match(out, /newest 2026-09-28/);
});

test("the seventh day is due, the sixth is not", () => {
  assert.match(show({ today: "2026-09-26", rows: [marker(SUNDAY)] }), /survey-due: ran /);
  // The seventh day after a Sunday is a Sunday, so test the clause on a
  // Monday survey: 2026-09-21 plus six is Sunday, which the Sunday clause
  // takes first; plus seven is Monday 2026-09-28.
  assert.match(show({ today: "2026-09-27", rows: [marker(MONDAY)] }), /Sunday clause/);
  assert.match(
    show({ today: "2026-09-28", rows: [marker(MONDAY)] }),
    /seventh-day clause: newest marker 2026-09-21 is 7 days old/,
  );
});

test("the marker's own date wins over when the comment was posted", () => {
  // A survey that starts Sunday evening posts its digest on Monday. The
  // survey it reports is still Sunday's.
  const hit = call("newest", {
    today: MONDAY,
    rows: [digest({ created_at: "2026-09-21T00:12:00Z" })],
  });
  assert.deepEqual(hit, [SUNDAY, "2026-09-21T00:12:00Z"]);
});

test("the newest marker wins, whatever order the rows come in", () => {
  const hit = call("newest", {
    today: "2026-10-04",
    rows: [marker("2026-09-27"), marker("2026-09-13"), marker("2026-09-20")],
  });
  assert.equal(hit[0], "2026-09-27");
});

test("a malformed, impossible or future-dated marker is not a marker", () => {
  for (
    const body of [
      "<!-- deep-survey -->",
      "<!-- deepsurvey 2026-09-20 -->",
      "deep-survey 2026-09-20",
      "<!-- deep-survey 2026-13-40 -->",
    ]
  ) {
    assert.equal(call("newest", { today: SUNDAY, rows: [digest({ body })] }), null, body);
  }
  // A marker dated after today was produced by no survey; counting it would
  // answer ran, the unsafe direction.
  assert.equal(call("newest", { today: SUNDAY, rows: [marker("2026-10-04")] }), null);
  // Reflowed whitespace is not malformed: an editor may rewrap the comment.
  assert.ok(call("newest", { today: SUNDAY, rows: [digest({ body: "<!--   deep-survey\t2026-09-20  -->" })] }));
});

test("a wake the operator triggered is not due and reads nothing, any day", () => {
  // Chat and event wakes never carry the survey: reply latency outranks it.
  for (const today of [SUNDAY, MONDAY]) {
    const out = call("render", { today, trigger: "issue_comment", rows: null, error: null });
    assert.match(out, /survey-due: not-due \(wake is issue_comment, not schedule\)/);
    assert.match(out, /newest - \| ops-log unread \| markers -/);
  }
});
