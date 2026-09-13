// Fixtures for security-alerts, the reader the Security tab never had.
//
// The first assertion is the one the script exists for: an endpoint that
// answers 403 must render as UNREADABLE and must not contribute a zero to the
// footer. A swallowed permission error turns this script into a machine that
// prints reassurance, which is strictly worse than having no script, because
// the reassurance is manufactured exactly where somebody went looking for it.
//
// The row builders are pure by construction (no network, no clock: `now` is an
// argument) precisely so these can exist. A detector nobody can make fire is
// decoration.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, io, json, sys
from datetime import datetime, timezone
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("security_alerts", sys.argv[1] + "/security-alerts")
spec = importlib.util.spec_from_loader("security_alerts", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
if fn == "render":
    buf = io.StringIO()
    mod.render(raw["dep"], raw["dep_error"], raw["code"], raw["code_error"], out=buf)
    print(json.dumps(buf.getvalue()))
else:
    now = datetime.fromisoformat(raw["now"].replace("Z", "+00:00"))
    print(json.dumps(getattr(mod, fn)(raw["alert"], now)))
`;

const NOW = "2026-09-13T21:52:00Z";

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

const row = (over = {}) => ({
  sev: "high",
  what: "thing",
  where: "somewhere",
  age: 0,
  url: "",
  move: "do it",
  ...over,
});

test("an unreadable endpoint never reports as clean", () => {
  const out = call("render", {
    dep: [],
    dep_error: "gh: Resource not accessible by integration (HTTP 403)",
    code: [],
    code_error: null,
  });
  assert.match(out, /UNREADABLE, which is not the same as clean/);
  assert.match(out, /vulnerability-alerts: read/);
  // The footer is what a skimming reader takes away. It must not say 0.
  assert.match(out, /dependabot unreadable/);
  assert.doesNotMatch(out, /dependabot 0/);
});

test("each unreadable endpoint names its own missing grant", () => {
  const out = call("render", {
    dep: [],
    dep_error: null,
    code: [],
    code_error: "HTTP 403",
  });
  assert.match(out, /security-events: read/);
  assert.doesNotMatch(out, /vulnerability-alerts: read/);
  // Readable-and-empty is the other half of the distinction.
  assert.match(out, /dependabot 0/);
});

test("a genuinely clean repository says so quietly", () => {
  const out = call("render", { dep: [], dep_error: null, code: [], code_error: null });
  assert.match(out, /dependabot 0 \| code-scanning 0/);
  assert.doesNotMatch(out, /UNREADABLE/);
});

test("worst first, then oldest first", () => {
  const out = call("render", {
    dep: [],
    dep_error: null,
    code: [
      row({ sev: "note", where: "n" }),
      row({ sev: "critical", where: "c" }),
      row({ sev: "high", where: "h-new", age: 1 }),
      row({ sev: "high", where: "h-old", age: 40 }),
    ],
    code_error: null,
  });
  const seen = ["c", "h-old", "h-new", "n"].map((w) => out.indexOf(` ${w}\n`));
  assert.deepEqual(seen, [...seen].sort((a, b) => a - b), out);
});

test("a runtime dependency is marked differently from a dev-dependency", () => {
  const base = {
    state: "open",
    created_at: "2026-09-01T00:00:00Z",
    security_advisory: { severity: "critical", ghsa_id: "GHSA-x" },
    security_vulnerability: { first_patched_version: { identifier: "1.2.3" } },
  };
  const dev = call("dependabot", {
    now: NOW,
    alert: { ...base, dependency: { package: { name: "p" }, scope: "development" } },
  });
  const run = call("dependabot", {
    now: NOW,
    alert: { ...base, dependency: { package: { name: "p" }, scope: "runtime" } },
  });
  assert.equal(dev.where, "dev-dependency");
  assert.equal(run.where, "RUNTIME dependency");
  assert.equal(run.age, 12);
  assert.match(run.move, /bump to 1\.2\.3/);
});

test("an unknown scope counts as runtime, because the other guess lets one through", () => {
  const got = call("dependabot", {
    now: NOW,
    alert: {
      state: "open",
      created_at: NOW,
      dependency: { package: { name: "p" } },
      security_advisory: { severity: "low" },
      security_vulnerability: {},
    },
  });
  assert.equal(got.where, "RUNTIME dependency");
  assert.match(got.move, /no patched version/);
});

test("a code-scanning rule with no security severity still prints, under its own", () => {
  const got = call("code_scanning", {
    now: NOW,
    alert: {
      state: "open",
      created_at: "2026-09-13T12:19:15Z",
      rule: { id: "js/x", security_severity_level: null, severity: "warning" },
      tool: { name: "CodeQL" },
      most_recent_instance: { location: { path: "a.mjs", start_line: 7 } },
    },
  });
  assert.equal(got.sev, "warning");
  assert.equal(got.where, "a.mjs:7");
  assert.equal(got.what, "js/x (CodeQL)");
});

test("a closed alert is somebody's finished decision, not a row", () => {
  const shape = { state: "dismissed", created_at: NOW };
  assert.equal(call("dependabot", { now: NOW, alert: shape }), null);
  assert.equal(call("code_scanning", { now: NOW, alert: shape }), null);
});
