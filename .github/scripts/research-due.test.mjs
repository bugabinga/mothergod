// Fixtures for research-due, the claim check issue #541 asked for: a tick
// that already owes nothing skips cheaply, and a missed tick costs at most
// half a day instead of a week. `decide` and `render` are pure (state is
// already fetched, passed in as arguments) precisely so this can pin both
// failure directions without spawning `gh`: a wrong `due` burns one extra,
// genuinely fresh experiment, a wrong `not-due` is the silent skip #541
// exists to end, and only the fixtures below can tell the two apart.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, json, sys
from datetime import datetime, timezone
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("research_due", sys.argv[1] + "/research-due")
spec = importlib.util.spec_from_loader("research_due", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
raw = json.loads(sys.argv[2])
newest = raw["newest"]
if newest is not None:
    newest = datetime.fromisoformat(newest.replace("Z", "+00:00"))
now = datetime.fromisoformat(raw["now"].replace("Z", "+00:00"))
verdict, detail = mod.decide(raw["flight"], raw["flight_error"], newest, raw["newest_error"], now, raw["window_days"])
print(json.dumps({"verdict": verdict, "detail": detail}))
`;

function decide(payload) {
  const run = spawnSync(
    "python3",
    ["-c", driver, scriptsDir, JSON.stringify({ window_days: 3, ...payload })],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

// in_flight()'s own ref-walking logic, exercised directly by monkeypatching
// gh_json rather than the higher-level in_flight itself: `decide()` never
// contained round 1's bug (a closed-unmerged-PR ref reported as in flight
// forever), `in_flight()` did, and testing only `decide()` and the crash
// wrapper below left that fix's own regression uncovered (review, PR #660).
const inFlightDriver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("research_due", sys.argv[1] + "/research-due")
spec = importlib.util.spec_from_loader("research_due", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
responses = iter(json.loads(sys.argv[2]))
def fake_gh_json(args):
    return tuple(next(responses))
mod.gh_json = fake_gh_json
flight, error = mod.in_flight("owner/repo")
print(json.dumps({"flight": flight, "error": error}))
`;

function inFlight(responses) {
  const run = spawnSync("python3", ["-c", inFlightDriver, scriptsDir, JSON.stringify(responses)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

test("an open claude/research-* PR is in flight, and no other read runs", () => {
  // A second gh_json call would throw StopIteration inside the driver and
  // fail the subprocess, so this also pins the short-circuit.
  const { flight, error } = inFlight([[[{ number: 7 }], null]]);
  assert.equal(error, null);
  assert.equal(flight, true);
});

test("no matching branch at all is not in flight", () => {
  const { flight, error } = inFlight([
    [[], null],
    [[], null],
  ]);
  assert.equal(error, null);
  assert.equal(flight, false);
});

test("a branch with no PR history at all is in flight (pushed, then crashed before opening one)", () => {
  const { flight, error } = inFlight([
    [[], null],
    [[{ ref: "refs/heads/claude/research-x" }], null],
    [[], null], // pr list --state all for that branch: no history
  ]);
  assert.equal(error, null);
  assert.equal(flight, true);
});

test("a branch whose only PR was closed unmerged is not in flight", () => {
  // The bug round 1 fixed: this ref must not block every later tick forever.
  const { flight, error } = inFlight([
    [[], null],
    [[{ ref: "refs/heads/claude/research-y" }], null],
    [[{ number: 42 }], null], // pr list --state all: one closed PR
  ]);
  assert.equal(error, null);
  assert.equal(flight, false);
});

test("an unreadable matching-refs read surfaces as an error, not a false negative", () => {
  const { flight, error } = inFlight([
    [[], null],
    [null, "gh: rate limited"],
  ]);
  assert.equal(flight, null);
  assert.match(error, /rate limited/);
});

// main()'s own fail-open contract, exercised by actually crashing in_flight,
// not by reasoning about the try/except: the caller here is a workflow `if:`
// that greps the exit code (unlike survey-due, whose caller reads prose), so
// an uncaught exception exiting 1 would read as a deliberate not-due with
// nothing red anywhere. This is the failure PR #660's review reproduced.
const mainDriver = `
import importlib.machinery, importlib.util, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("research_due", sys.argv[1] + "/research-due")
spec = importlib.util.spec_from_loader("research_due", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
def boom(repo):
    raise KeyError("unexpected gh api shape")
mod.in_flight = boom
code = mod.main()
print(f"EXITCODE:{code}")
`;

function crashedMain() {
  // render()'s \`out=sys.stdout\` default binds the real stream at module
  // load time, before any in-process redirect could intercept it, so this
  // reads the subprocess's actual stdout rather than trying to capture it
  // from inside the driver.
  const run = spawnSync("python3", ["-c", mainDriver, scriptsDir], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  const match = run.stdout.match(/EXITCODE:(\d+)/);
  return { code: Number(match[1]), output: run.stdout };
}

const NOW = "2026-09-21T12:00:00Z";
const base = (over = {}) => ({
  flight: false,
  flight_error: null,
  newest: null,
  newest_error: null,
  now: NOW,
  ...over,
});

test("an unreadable in-flight read answers due, never not-due", () => {
  const { verdict, detail } = decide(base({ flight_error: "gh: Not Found (HTTP 404)" }));
  assert.equal(verdict, "due");
  assert.match(detail, /open-PR\/branch read UNREADABLE/);
});

test("an unreadable merged-PR read answers due", () => {
  const { verdict, detail } = decide(base({ newest_error: "gh: rate limited" }));
  assert.equal(verdict, "due");
  assert.match(detail, /merged-PR read UNREADABLE/);
});

test("a session already in flight is not due, and the merge read never runs", () => {
  // flight=true with no newest supplied: main() never calls newest_merge in
  // this branch, and decide() must not need it to answer.
  const { verdict, detail } = decide(base({ flight: true }));
  assert.equal(verdict, "not-due");
  assert.match(detail, /already covers this cycle/);
});

test("a merge inside the window is not due", () => {
  const { verdict, detail } = decide(base({ newest: "2026-09-19T12:00:00Z" })); // 2 days ago
  assert.equal(verdict, "not-due");
  assert.match(detail, /inside the 3-day window/);
});

test("a merge exactly at the window boundary is due (strict less-than)", () => {
  const { verdict } = decide(base({ newest: "2026-09-18T12:00:00Z" })); // exactly 3 days ago
  assert.equal(verdict, "due");
});

test("a merge older than the window is due", () => {
  const { verdict, detail } = decide(base({ newest: "2026-09-17T12:00:00Z" })); // 4 days ago
  assert.equal(verdict, "due");
  assert.match(detail, /no research PR in flight or merged within 3 days/);
});

test("no research PR ever merged is due", () => {
  const { verdict } = decide(base());
  assert.equal(verdict, "due");
});

test("a crash mid-decision answers due with exit 0, never a bare exit 1", () => {
  const { code, output } = crashedMain();
  assert.equal(code, 0, "exit 0 is what makes `if research-due` in the workflow read this as due");
  assert.match(output, /research-due: due/);
  assert.match(output, /research-due crashed: KeyError/);
});
