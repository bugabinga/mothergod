// Fixtures for survey-due, the detector the survey's trigger never had.
//
// The first assertion is the one the script exists for: an ops log that
// cannot be read must answer DUE. The bug that produced this script (#558)
// was a run announcing that no survey had happened twelve hours after one
// had, so both directions of the wrong answer are pinned here, and the
// asymmetry between them is deliberate: a wrong DUE costs a duplicated
// Sunday, a wrong RAN loses the week's most expensive duty in silence.
//
// `render`, `sunday_of` and `marked` are pure by construction (no network, no
// clock: `now` and the rows are arguments) precisely so these can exist. A
// detector nobody can make fire is decoration.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, io, json, sys
from datetime import datetime, timezone
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("survey_due", sys.argv[1] + "/survey-due")
spec = importlib.util.spec_from_loader("survey_due", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
if fn == "render":
    buf = io.StringIO()
    mod.render(raw["sunday"], raw["trigger"], raw["rows"], raw["error"], out=buf)
    print(json.dumps(buf.getvalue()))
elif fn == "sunday_of":
    now = datetime.fromisoformat(raw["now"].replace("Z", "+00:00"))
    print(json.dumps(mod.sunday_of(now)))
else:
    hit = mod.marked(raw["rows"], raw["sunday"])
    print(json.dumps(None if hit is None else hit["created_at"]))
`;

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const SUNDAY = "2026-09-20";

const digest = (over = {}) => ({
  created_at: "2026-09-20T00:38:19Z",
  html_url: "https://github.com/bugabinga/mothergod/issues/3#issuecomment-1",
  body: `**Deep survey, Sunday ${SUNDAY}**\n\n<!-- deep-survey ${SUNDAY} -->`,
  ...over,
});

const show = (over = {}) => call("render", { sunday: SUNDAY, trigger: "schedule", rows: [], error: null, ...over });

test("an unreadable ops log answers due, never ran", () => {
  const out = show({ rows: null, error: "gh: Not Found (HTTP 404)" });
  assert.match(out, /survey-due: due/);
  assert.doesNotMatch(out, /ran /);
  assert.match(out, /UNREADABLE, which is not the same as no survey/);
  // The footer is what a skimming reader takes away. It must not claim a read.
  assert.match(out, /ops-log unreadable/);
  assert.doesNotMatch(out, /ops-log readable/);
});

test("no marker for this Sunday is due", () => {
  const out = show({ rows: [{ body: "an ordinary digest", created_at: "x" }] });
  assert.match(out, /survey-due: due/);
  assert.match(out, /no deep-survey marker for this Sunday/);
  assert.match(out, /ops-log readable \| markers 0/);
});

test("a marker for this Sunday reports ran, with when", () => {
  const out = show({ rows: [digest()] });
  assert.match(out, /survey-due: ran 2026-09-20T00:38:19Z/);
  assert.match(out, /markers 1/);
  // A duty already done prints no instructions. Quiet is the result.
  assert.doesNotMatch(out, /DUE/);
  assert.doesNotMatch(out, /deep-survey` skill/);
});

test("last week's marker does not satisfy this week", () => {
  // The 2026-09-13 failure, inverted: a real marker, wrong Sunday. Matching
  // on the word `deep-survey` alone would answer ran and skip the week.
  const out = show({ rows: [digest({ body: "<!-- deep-survey 2026-09-13 -->" })] });
  assert.match(out, /survey-due: due/);
  // It is still a marker: the liveness count says the detector is working.
  assert.match(out, /markers 1/);
});

test("the marker's own date wins over when the comment was posted", () => {
  // A survey that starts Sunday evening posts its digest on Monday. The
  // survey it reports is still Sunday's.
  const posted = call("marked", {
    rows: [digest({ created_at: "2026-09-21T00:12:00Z" })],
    sunday: SUNDAY,
  });
  assert.equal(posted, "2026-09-21T00:12:00Z");
});

test("a malformed marker is not a marker", () => {
  for (const body of ["<!-- deep-survey -->", "<!-- deepsurvey 2026-09-20 -->", "deep-survey 2026-09-20"]) {
    assert.equal(call("marked", { rows: [digest({ body })], sunday: SUNDAY }), null, body);
  }
  // Reflowed whitespace is not malformed: an editor may rewrap the comment.
  assert.ok(call("marked", { rows: [digest({ body: "<!--   deep-survey\t2026-09-20  -->" })], sunday: SUNDAY }));
});

test("a wake that is not Sunday is not due and reads nothing", () => {
  const out = call("render", { sunday: null, trigger: "schedule", rows: null, error: null });
  assert.match(out, /survey-due: not-due/);
  assert.match(out, /not a Sunday in UTC/);
  assert.match(out, /ops-log unread \| markers -/);
});

test("a Sunday wake the operator triggered is not due", () => {
  // Chat and event wakes never carry the survey: reply latency outranks it.
  const out = show({ trigger: "issue_comment" });
  assert.match(out, /survey-due: not-due/);
  assert.match(out, /wake is issue_comment, not schedule/);
});

test("sunday_of never rounds back to a Sunday that has ended", () => {
  assert.equal(call("sunday_of", { now: "2026-09-20T23:59:00Z" }), SUNDAY);
  assert.equal(call("sunday_of", { now: "2026-09-20T00:00:00Z" }), SUNDAY);
  // Monday is a wake for the delta core, not for a window that closed.
  assert.equal(call("sunday_of", { now: "2026-09-21T00:01:00Z" }), null);
  assert.equal(call("sunday_of", { now: "2026-09-19T23:59:00Z" }), null);
});
