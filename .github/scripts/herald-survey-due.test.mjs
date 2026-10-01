// Fixtures for herald-survey-due, the detector that ends the six-day
// sentence three herald runs read three ways (marketing/JOURNAL.md,
// 2026-10-01). The first assertions are the ones the script exists for:
// the boundary is decided by subtraction (six is not-due, seven is due),
// and a journal it cannot read answers DUE, never not-due. `survey_dates`,
// `decide` and `render` are pure (text, dates and a clock are arguments)
// so both wrong directions pin here without a filesystem or a calendar.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = join(scriptsDir, "herald-survey-due");

const driver = `
import importlib.machinery, importlib.util, io, json, sys
from datetime import date
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("herald_survey_due", sys.argv[1] + "/herald-survey-due")
spec = importlib.util.spec_from_loader("herald_survey_due", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
if fn == "survey_dates":
    print(json.dumps([d.isoformat() for d in mod.survey_dates(raw["text"])]))
elif fn == "decide":
    newest = raw["newest"] and date.fromisoformat(raw["newest"])
    print(json.dumps(mod.decide(newest, date.fromisoformat(raw["today"]), raw["error"])))
else:
    buf = io.StringIO()
    mod.render(raw["verdict"], raw["detail"], raw["newest"], raw["count"], raw["state"], out=buf)
    print(json.dumps(buf.getvalue()))
`;

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const decide = (newest, today, error = null) => call("decide", { newest, today, error });

test("the journal's own heading shape parses, Editorials do not count, order is file order", () => {
  const text = [
    "# Marketing journal",
    "## 2026-10-01 — Survey: the repo doubled, the site halved",
    "## 2026-10-01 — Editorial: the Silesia caption named \"the same two\"",
    "## 2026-09-25 — Survey: every reader lands on `/`",
    "## 2026-09-23 — Editorial: the Survey said one thing, the site another",
  ].join("\n\n");
  assert.deepEqual(call("survey_dates", { text }), ["2026-10-01", "2026-09-25"]);
});

test("a heading that drops or swaps the dash still counts; a date that is not a day is dropped", () => {
  const text = [
    "## 2026-09-19 - Survey: hyphen",
    "## 2026-09-12 Survey: no dash at all",
    "## 2026-13-40 — Survey: not a day",
    "## 2026-09-05: Survey: colon",
  ].join("\n");
  assert.deepEqual(call("survey_dates", { text }), ["2026-09-19", "2026-09-12", "2026-09-05"]);
});

test("six days is not-due and seven is due: the boundary is arithmetic, not reading", () => {
  const [six, sixDetail] = decide("2026-09-25", "2026-10-01");
  assert.equal(six, "not-due");
  assert.match(sixDetail, /6 days old, under 7/);
  const [seven, sevenDetail] = decide("2026-09-25", "2026-10-02");
  assert.equal(seven, "due");
  assert.match(sevenDetail, /7 days old, 7 or more/);
  assert.equal(decide("2026-09-25", "2026-10-20")[0], "due");
});

test("no Survey heading answers due, and an unreadable journal answers due and says UNREADABLE", () => {
  const [none, noneDetail] = decide(null, "2026-10-01");
  assert.equal(none, "due");
  assert.match(noneDetail, /no Survey heading/);
  const [broken, brokenDetail] = decide("2026-10-01", "2026-10-01", "Permission denied");
  assert.equal(broken, "due", "an error outranks a fresh date: the date came from nowhere");
  assert.match(brokenDetail, /UNREADABLE/);
});

test("the footer carries the newest date, the count and the read state on every verdict", () => {
  const quiet = call("render", {
    verdict: "not-due",
    detail: "d",
    newest: "2026-10-01",
    count: 5,
    state: "readable",
  });
  assert.doesNotMatch(quiet, /DUE/);
  assert.match(quiet, /herald-survey-due: not-due \(d\) \| newest 2026-10-01 \| journal readable \| surveys 5/);
  const loud = call("render", { verdict: "due", detail: "d", newest: null, count: "-", state: "unreadable" });
  assert.match(loud, /survey: DUE\. d\./);
  assert.match(loud, /audience-survey/);
  assert.match(loud, /newest - \| journal unreadable \| surveys -/);
});

test("end to end: a journal file whose newest survey is old answers due, a missing file answers due", () => {
  const dir = mkdtempSync(join(tmpdir(), "herald-survey-due-"));
  const journal = join(dir, "JOURNAL.md");
  writeFileSync(journal, "# Marketing journal\n\n## 2020-01-01 — Survey: long ago\n");
  const old = spawnSync("python3", [script, journal], { encoding: "utf8" });
  assert.equal(old.status, 0, old.stderr);
  assert.match(old.stdout, /survey: DUE\./);
  assert.match(old.stdout, /newest 2020-01-01 \| journal readable \| surveys 1/);
  const missing = spawnSync("python3", [script, join(dir, "absent.md")], { encoding: "utf8" });
  assert.equal(missing.status, 0, missing.stderr);
  assert.match(missing.stdout, /UNREADABLE/);
  assert.match(missing.stdout, /journal unreadable \| surveys -/);
});
