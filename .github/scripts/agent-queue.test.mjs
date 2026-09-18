// Fixtures for agent-queue, the ranking step 5 did by eye until 2026-09-18.
//
// The first assertion is the one the script exists for: a queue that cannot be
// read must print UNREADABLE, never empty. The prompt answers an empty queue by
// stopping, so a wrong `empty` turns a backlog into a run that reports finding
// nothing. The second is the drift that produced the script: a fresh wording
// nit must not outrank a 25-day token-exposure bug (#583 over #106).
//
// `classify`, `render`, `rank_of`, `claims` and `blockers` are pure by
// construction (no network, no clock: rows and `now` are arguments) precisely
// so these can exist. A ranking nobody can make fire is decoration.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import importlib.machinery, importlib.util, io, json, sys
from datetime import datetime, timezone
loader = importlib.machinery.SourceFileLoader("agent_queue", sys.argv[1] + "/agent-queue")
spec = importlib.util.spec_from_loader("agent_queue", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
now = datetime(2026, 9, 18, tzinfo=timezone.utc)
if fn == "render":
    ranked, excluded = mod.classify(raw["rows"], set(raw.get("held") or []), now)
    buf = io.StringIO()
    mod.render(ranked, excluded, raw.get("error"), raw.get("all", False), out=buf)
    print(json.dumps(buf.getvalue()))
elif fn == "unreadable":
    buf = io.StringIO()
    empty = {bucket: [] for bucket in mod.BUCKETS}
    mod.render([], empty, raw["error"], False, out=buf)
    print(json.dumps(buf.getvalue()))
elif fn == "classify":
    ranked, excluded = mod.classify(raw["rows"], set(raw.get("held") or []), now)
    print(json.dumps({
        "ranked": [item["number"] for item in ranked],
        "excluded": {key: [item["number"] for item in value] for key, value in excluded.items()},
    }))
elif fn == "claims":
    print(json.dumps(sorted(mod.claims(raw["prs"]))))
else:
    print(json.dumps(sorted(mod.blockers(raw["rows"]))))
`;

function call(fn, payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, fn, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

// Ages are measured against the driver's frozen 2026-09-18.
const issue = (number, over = {}) => ({
  number,
  title: `issue ${number}`,
  createdAt: "2026-09-18T00:00:00Z",
  body: "",
  labels: [{ name: "agent-system" }],
  ...over,
  ...(over.labels ? { labels: over.labels.map((name) => ({ name })) } : {}),
});

test("an unreadable issue list prints UNREADABLE, never empty", () => {
  const out = call("unreadable", { error: "gh: Bad credentials (HTTP 401)" });
  assert.match(out, /agent-queue: UNREADABLE/);
  assert.match(out, /Bad credentials/);
  // The prompt stops on an empty queue, so the verdict must never read empty.
  // The guidance line may say the word; the verdict and footer may not.
  assert.doesNotMatch(out, /agent-queue: empty/);
  assert.doesNotMatch(out, /\d+ ranked/);
  assert.match(out, /issues unreadable/);
  assert.doesNotMatch(out, /issues readable/);
});

test("urgency beats recency: the old bug outranks the fresh nit", () => {
  // The 2026-09-18 drift, pinned. #583 filed today, #106 open 25 days with a
  // bug label. Reading the list by eye put the fresh one on top.
  const rows = [
    issue(583, { title: "BDFL prompt describes a lever branch protection lacks" }),
    issue(106, {
      title: "Reviewer executes PR code with live tokens",
      // 25 days and 15 hours before the driver's frozen now, so 25 whole days.
      createdAt: "2026-08-23T08:46:19Z",
      labels: ["agent-system", "bug"],
    }),
  ];
  const out = call("render", { rows });
  assert.match(out, /agent-queue: top is #106 \(bug, 25d old\)/);
  const { ranked } = call("classify", { rows });
  assert.deepEqual(ranked, [106, 583]);
});

test("ledgers are excluded, so a permanent issue never sorts to the top", () => {
  // Six of twenty-two agent-system issues were ledgers on 2026-09-18. The only
  // correct action on one is to leave it alone.
  const rows = [
    issue(3, {
      title: "Operations log",
      createdAt: "2026-08-21T00:00:00Z",
      labels: ["agent-system", "ops-log", "ledger"],
    }),
    issue(552, { title: "CodeQL reports a red check on every non-Rust PR" }),
  ];
  const { ranked, excluded } = call("classify", { rows });
  assert.deepEqual(ranked, [552]);
  assert.deepEqual(excluded.ledger, [3]);
  assert.match(call("render", { rows }), /excluded .*ledger 1/);
});

test("the prompt's ladder orders bug, blocker, plain, enhancement", () => {
  const rows = [
    issue(10, { labels: ["agent-system", "enhancement"] }),
    issue(20),
    issue(30, { labels: ["agent-system", "bug"] }),
    // #40 is named as a blocker by #20's body, not by any label.
    issue(40),
    issue(21, { body: "this is blocked by #40 until that lands" }),
  ];
  const { ranked } = call("classify", { rows });
  assert.deepEqual(ranked, [30, 40, 20, 21, 10]);
});

test("age breaks ties in the older item's favour", () => {
  const rows = [
    issue(2, { createdAt: "2026-09-17T00:00:00Z" }),
    issue(1, { createdAt: "2026-09-01T00:00:00Z" }),
    issue(3, { createdAt: "2026-09-10T00:00:00Z" }),
  ];
  assert.deepEqual(call("classify", { rows }).ranked, [1, 3, 2]);
});

test("a claimed issue is printed but never ranked", () => {
  // ADR-0014 runs agents concurrently; taking a claimed issue duplicates work.
  const rows = [issue(500), issue(501)];
  const { ranked, excluded } = call("classify", { rows, held: [500] });
  assert.deepEqual(ranked, [501]);
  assert.deepEqual(excluded.claimed, [500]);
  const out = call("render", { rows, held: [500] });
  assert.match(out, /claimed by an open PR, do not take:/);
  assert.match(out, /#500/);
});

test("a claim needs a closing keyword, not a passing mention", () => {
  // The dangerous direction: a PR body citing an issue in prose would
  // exclude it from the queue, shrinking the backlog silently.
  assert.deepEqual(
    call("claims", { prs: [{ body: "the drift #589 recorded, see also #12", headRefName: "claude/x" }] }),
    [],
  );
  assert.deepEqual(
    call("claims", { prs: [{ body: "Closes #106 and fixes #107", headRefName: "claude/x" }] }),
    [106, 107],
  );
  // Branch names never claim: harvesting digits would have `tans-s1-p6`
  // claim #1 and #6, excluding two real issues for a slug.
  assert.deepEqual(call("claims", { prs: [{ body: "", headRefName: "claude/tans-s1-p6" }] }), []);
  assert.deepEqual(call("claims", { prs: [] }), []);
});

test("blocked-on-human is latched, not queued", () => {
  // Only the BDFL or the operator clears the latch (GOVERNANCE.md).
  const rows = [issue(537, { labels: ["agent-system", "blocked-on-human"] }), issue(538)];
  const { ranked, excluded } = call("classify", { rows });
  assert.deepEqual(ranked, [538]);
  assert.deepEqual(excluded.latched, [537]);
});

test("an unrouted issue is surfaced for routing, not ranked into the queue", () => {
  const rows = [issue(900, { labels: [] }), issue(901)];
  const { ranked, excluded } = call("classify", { rows });
  assert.deepEqual(ranked, [901]);
  assert.deepEqual(excluded.unrouted, [900]);
  const out = call("render", { rows });
  assert.match(out, /unrouted, route these before you ship/);
  assert.match(out, /#900/);
});

test("product and marketing issues are another seat's queue", () => {
  const rows = [
    issue(800, { labels: ["product"] }),
    issue(801, { labels: ["product", "marketing"] }),
    issue(802),
  ];
  const { ranked, excluded } = call("classify", { rows });
  assert.deepEqual(ranked, [802]);
  assert.deepEqual(excluded["other-realm"], [800, 801]);
  // Another seat's work is not the BDFL's reading. It stays off the listing.
  const out = call("render", { rows });
  assert.doesNotMatch(out, /#800/);
});

test("blockers are read from every open issue, excluded rows included", () => {
  const rows = [
    issue(1, { labels: ["product"], body: "blocked by #2" }),
    issue(3, { body: "depends on #4" }),
    issue(5, { body: "blocking #6, and unrelated text #notanumber" }),
  ];
  assert.deepEqual(call("blockers", { rows }), [2, 4, 6]);
});

test("a genuinely empty queue says so, and says it differently from unreadable", () => {
  const out = call("render", { rows: [] });
  assert.match(out, /agent-queue: empty/);
  assert.match(out, /issues readable/);
  assert.match(out, /top -/);
  assert.match(out, /excluded none/);
});

test("the listing caps at five without --all, and says how many it hid", () => {
  const rows = [1, 2, 3, 4, 5, 6, 7].map((number) => issue(number));
  const capped = call("render", { rows });
  assert.match(capped, /\.\.\. 2 more, with --all/);
  assert.match(capped, /7 ranked/);
  const full = call("render", { rows, all: true });
  assert.doesNotMatch(full, /more, with --all/);
  assert.match(full, /#7/);
});
