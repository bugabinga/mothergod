// Fixtures for mutants-sweep: ten shards must merge into one honest score and
// one refreshed issue. Both failure directions are pinned without spawning gh
// or cargo: a score over part of the crate (a shard missing, a shard cut) is
// the lie the null guards against; a decode-safety survivor dropped from the
// issue, or one filed twice beside its mutants-debt issue, is the silence and
// the duplicate the territory and dedup rules exist to end.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import dataclasses, importlib.machinery, importlib.util, json, sys
from collections import Counter
loader = importlib.machinery.SourceFileLoader("mutants_sweep", sys.argv[1] + "/mutants-sweep")
spec = importlib.util.spec_from_loader("mutants_sweep", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
raw = json.loads(sys.stdin.read())
def shard(s):
    return mod.Shard(name=s["name"], spec=s.get("spec"), listed=s.get("listed", []),
                     outcomes=s.get("outcomes", {}), error=s.get("error"))
def sweep(s):
    return mod.Sweep(caught=s.get("caught", []), missed=s.get("missed", []), unviable=s.get("unviable", []),
                     timeout=s.get("timeout", []), never_ran=s.get("never_ran", []), shards=s.get("shards", 1),
                     denominator=s.get("denominator", 1), problems=s.get("problems", []))
def owed(o):
    return mod.Owed(missed=o.get("missed", []), undecided=o.get("undecided", []), elsewhere=o.get("elsewhere", []),
                    excluded=o.get("excluded", 0), scaffold=Counter(o.get("scaffold", {})))
fn = raw["fn"]
if fn == "merge":
    s = mod.merge([shard(x) for x in raw["shards"]])
    d = dataclasses.asdict(s); d["score"] = s.score; d["unmeasured"] = s.unmeasured
    print(json.dumps(d))
elif fn == "read_shard":
    s = mod.read_shard(raw["path"])
    print(json.dumps(dataclasses.asdict(s)))
elif fn == "triage":
    o = mod.triage(sweep(raw["sweep"]), {int(k): set(v) for k, v in raw["filed"].items()})
    print(json.dumps({"missed": o.missed, "undecided": o.undecided, "elsewhere": o.elsewhere,
                      "excluded": o.excluded, "scaffold": dict(o.scaffold), "summary": o.summary}))
elif fn == "render":
    title, body = mod.render(sweep(raw["sweep"]), owed(raw["owed"]), raw["head"], raw["date"], raw["run_url"], raw.get("dedup_error"))
    print(json.dumps({"title": title, "body": body}))
elif fn == "select":
    print(json.dumps(mod.select_issue(raw["issues"])))
elif fn == "decide":
    print(json.dumps(mod.decide(owed(raw["owed"]), raw["selected"])))
elif fn == "filed":
    print(json.dumps(sorted(mod.filed_elsewhere(raw["body"]))))
`;

function call(payload) {
  // The payload rides stdin: the August fixture below is past Linux's single-argument limit.
  const run = spawnSync("python3", ["-c", driver, scriptsDir], { encoding: "utf8", input: JSON.stringify(payload) });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

// Decode-safety territory (ADR-0056) and scaffold, in cargo-mutants' name form.
const decodeSurvivor = "src/codec.rs:555:5: replace ensure_room -> Result<(), Error> with Ok(())";
const coderSurvivor = "src/coder.rs:303:26: replace & with | in Decoder<'a>::decode";
const scaffoldA = "src/literal.rs:952:55: replace + with - in Literal::mix_ppm";
const scaffoldB = "src/lz.rs:120:9: replace > with >= in find_match";
const caughtA = "src/codec.rs:10:5: replace compress -> Vec<u8> with vec![]";
const caughtB = "src/filters.rs:40:5: replace apply -> bool with true";
const unviableA = "src/lib.rs:5:1: replace abort_on_oom -> T with Default::default()";
const timeoutA = "src/coder.rs:23:9: replace << with >> in Encoder::encode";

const shard0 = {
  name: "mutants-sweep-shard-0",
  spec: "0/2",
  listed: [decodeSurvivor, caughtA, scaffoldA, unviableA],
  outcomes: { [decodeSurvivor]: "missed", [caughtA]: "caught", [scaffoldA]: "missed", [unviableA]: "unviable" },
};
const shard1 = {
  name: "mutants-sweep-shard-1",
  spec: "1/2",
  listed: [coderSurvivor, caughtB, scaffoldB, timeoutA],
  outcomes: { [coderSurvivor]: "missed", [caughtB]: "caught", [scaffoldB]: "missed", [timeoutA]: "timeout" },
};

test("two complete shards merge into one score over caught plus missed, timeouts and unviable outside it", () => {
  const s = call({ fn: "merge", shards: [shard0, shard1] });
  assert.deepEqual(s.caught, [caughtA, caughtB].sort());
  assert.deepEqual(s.missed, [decodeSurvivor, coderSurvivor, scaffoldA, scaffoldB].sort());
  assert.deepEqual(s.unviable, [unviableA]);
  assert.deepEqual(s.timeout, [timeoutA]);
  assert.deepEqual(s.never_ran, []);
  assert.deepEqual(s.problems, []);
  assert.equal(s.denominator, 2);
  assert.equal(s.score, 33.33); // 2 caught of 6 viable decided
  assert.deepEqual(s.unmeasured, []);
});

test("the August 2026 sweep's counts give 82.02", () => {
  const caught = Array.from({ length: 1172 }, (_, i) => `src/a.rs:${i}:1: replace x with y in f${i}`);
  const missed = Array.from({ length: 257 }, (_, i) => `src/b.rs:${i}:1: replace x with y in g${i}`);
  const outcomes = Object.fromEntries([...caught.map((n) => [n, "caught"]), ...missed.map((n) => [n, "missed"])]);
  const s = call({ fn: "merge", shards: [{ name: "s", spec: "0/1", listed: [...caught, ...missed], outcomes }] });
  assert.equal(s.score, 82.02);
});

test("a missing shard voids the score and is named; many missing are counted", () => {
  let s = call({ fn: "merge", shards: [{ ...shard0, spec: "0/3" }, { ...shard1, spec: "2/3" }] });
  assert.equal(s.score, null);
  assert.deepEqual(s.problems, ["shard 1 of 3 missing"]);
  assert.equal(s.denominator, 3);
  s = call({ fn: "merge", shards: [{ ...shard0, spec: "0/256" }] });
  assert.deepEqual(s.problems, ["255 of 256 shards missing, received 0"]);
});

test("a shard cut by its budget leaves never-run mutants and no score", () => {
  const cut = { ...shard1, outcomes: { [coderSurvivor]: "missed" } };
  const s = call({ fn: "merge", shards: [shard0, cut] });
  assert.equal(s.score, null);
  assert.deepEqual(s.never_ran, [caughtB, scaffoldB, timeoutA].sort());
  assert.deepEqual(s.problems, []);
  assert.deepEqual(s.unmeasured, ["3 listed mutants never ran: a shard cut by its budget"]);
});

test("shards that disagree on n, a repeated index, an unreadable shard and no shards each void the score", () => {
  let s = call({ fn: "merge", shards: [shard0, { ...shard1, spec: "1/4" }] });
  assert.deepEqual(s.problems, ["shards disagree on the denominator: [2, 4]"]);
  s = call({ fn: "merge", shards: [shard0, { ...shard1, spec: "0/2" }] });
  assert.deepEqual(s.problems, ["shard 1 of 2 missing", "shard 0 received twice"]);
  s = call({
    fn: "merge",
    shards: [shard0, { name: "mutants-sweep-shard-1", spec: "1/2", error: "no outcomes.json" }],
  });
  assert.deepEqual(s.problems, ["mutants-sweep-shard-1: no outcomes.json"]);
  assert.equal(s.score, null);
  s = call({ fn: "merge", shards: [] });
  assert.deepEqual(s.problems, ["no shard directory"]);
  assert.equal(s.score, null);
});

test("a mutant listed by two shards counts once", () => {
  const s = call({
    fn: "merge",
    shards: [shard0, {
      ...shard1,
      listed: [...shard1.listed, caughtA],
      outcomes: { ...shard1.outcomes, [caughtA]: "caught" },
    }],
  });
  assert.equal(s.caught.length, 2);
});

test("read_shard reads cargo-mutants' files and names what it cannot read", () => {
  const dir = mkdtempSync(join(tmpdir(), "mutants-sweep-"));
  const good = join(dir, "mutants-sweep-shard-0");
  mkdirSync(good);
  writeFileSync(join(good, "shard.txt"), "0/2\n");
  writeFileSync(
    join(good, "mutants.json"),
    JSON.stringify([{ name: caughtA }, { name: decodeSurvivor }, { name: timeoutA }]),
  );
  writeFileSync(
    join(good, "outcomes.json"),
    JSON.stringify({
      outcomes: [
        { scenario: "Baseline", summary: "Success" },
        { scenario: { Mutant: { name: caughtA } }, summary: "CaughtMutant" },
        { scenario: { Mutant: { name: decodeSurvivor } }, summary: "MissedMutant" },
      ],
    }),
  );
  let s = call({ fn: "read_shard", path: good });
  assert.equal(s.spec, "0/2");
  assert.equal(s.error, null);
  assert.deepEqual(s.listed, [caughtA, decodeSurvivor, timeoutA]);
  assert.deepEqual(s.outcomes, { [caughtA]: "caught", [decodeSurvivor]: "missed" });

  const truncated = join(dir, "mutants-sweep-shard-1");
  mkdirSync(truncated);
  writeFileSync(join(truncated, "shard.txt"), "1/2\n");
  writeFileSync(join(truncated, "mutants.json"), JSON.stringify([{ name: caughtB }]));
  writeFileSync(join(truncated, "outcomes.json"), "{\"outcomes\": [{\"scenario\": {\"Mut");
  s = call({ fn: "read_shard", path: truncated });
  assert.match(s.error, /^JSONDecodeError/);
  assert.deepEqual(s.listed, [caughtB]);

  const bare = join(dir, "mutants-sweep-shard-2");
  mkdirSync(bare);
  s = call({ fn: "read_shard", path: bare });
  assert.equal(s.error, "no shard.txt");
  assert.equal(s.spec, null);

  const odd = join(dir, "mutants-sweep-shard-3");
  mkdirSync(odd);
  writeFileSync(join(odd, "shard.txt"), "three\n");
  s = call({ fn: "read_shard", path: odd });
  assert.equal(s.error, "shard.txt holds 'three', not k/n");
});

const merged = {
  caught: [caughtA, caughtB],
  missed: [decodeSurvivor, coderSurvivor, scaffoldA, scaffoldB],
  unviable: [unviableA],
  timeout: [timeoutA],
  shards: 2,
  denominator: 2,
};

test("triage lists decode-safety survivors and undecided mutants, counts scaffold per file, excludes what is filed", () => {
  let o = call({ fn: "triage", sweep: merged, filed: {} });
  assert.deepEqual(o.missed, [decodeSurvivor, coderSurvivor]);
  assert.deepEqual(o.undecided, [timeoutA]);
  assert.deepEqual(o.elsewhere, []);
  assert.equal(o.excluded, 0);
  assert.deepEqual(o.scaffold, { "src/literal.rs": 1, "src/lz.rs": 1 });
  assert.equal(o.summary, "2 survivors, 1 undecided");

  o = call({ fn: "triage", sweep: merged, filed: { 697: [coderSurvivor], 705: [scaffoldA] } });
  assert.deepEqual(o.missed, [decodeSurvivor]);
  assert.deepEqual(o.elsewhere, [697]); // #705 carries only a scaffold line, which the sweep never lists
  assert.equal(o.excluded, 1);
  assert.equal(o.summary, "1 survivor, 1 undecided");

  o = call({ fn: "triage", sweep: { ...merged, missed: [scaffoldA], timeout: [] }, filed: {} });
  assert.equal(o.summary, "");
});

test("filed_elsewhere reads mutant names out of an issue body and nothing else", () => {
  const body = [
    "PR #690 merged while its mutants-check was still running.",
    "",
    "```",
    coderSurvivor,
    "```",
    "cargo mutants --config tests/mutants.toml --package mothergod -f src/coder.rs --re 'replace & with |'",
  ].join("\n");
  assert.deepEqual(call({ fn: "filed", body }), [coderSurvivor]);
});

test("render names the score, the debt, the exclusions, the scaffold map and the deciding runs", () => {
  const owed = {
    missed: [decodeSurvivor],
    undecided: [timeoutA],
    elsewhere: [697],
    excluded: 1,
    scaffold: { "src/lz.rs": 1, "src/literal.rs": 1 },
  };
  const r = call({
    fn: "render",
    sweep: merged,
    owed,
    head: "31deb96abc",
    date: "2026-11-01",
    run_url: "https://x/run/1",
  });
  assert.equal(r.title, "mutants: 1 survivor, 1 undecided on main, whole-crate sweep 2026-11-01 at 31deb96");
  assert.ok(r.body.startsWith("<!-- mutants-sweep -->\n"));
  assert.match(
    r.body,
    /decided 8 of 8 mutants of `src\/`\nacross 2 shards: 2 caught, 4 missed, 1 unviable, 1 timed out\./,
  );
  assert.match(r.body, /Mutation score 33\.33%/);
  assert.doesNotMatch(r.body, /unmeasured/);
  assert.match(r.body, /Decode-safety survivors \(ADR-0056\)[^]*```\n.*ensure_room.*\n```/);
  assert.match(r.body, /Undecided in decode-safety territory[^]*```\n.*Encoder::encode\n```/);
  assert.match(r.body, /1 more in decode-safety territory already filed, left to #697\./);
  assert.match(
    r.body,
    /2 survivors sit outside decode-safety territory: scaffold, not debt \(ADR-0056\)\.\nPer file: `src\/lz.rs` 1, `src\/literal.rs` 1\./,
  );
  assert.match(r.body, /Each survivor takes one of three answers/);
  assert.match(r.body, /An undecided mutant is run first/);
  assert.match(r.body, /cargo mutants --config tests\/mutants.toml --package mothergod -f src\/codec.rs --re /);
  assert.match(
    r.body,
    /Refreshed by every sweep; closed by one that finds nothing owed\.\n\nRun: https:\/\/x\/run\/1$/,
  );
  assert.doesNotMatch(r.body, /—/);
});

test("render says why a score is unmeasured, and a clean sweep says nothing owed", () => {
  let r = call({
    fn: "render",
    sweep: { ...merged, never_ran: [caughtB], caught: [caughtA], problems: ["shard 1 of 2 missing"] },
    owed: { missed: [], undecided: [], elsewhere: [], excluded: 0, scaffold: {} },
    head: "",
    date: "2026-11-01",
    run_url: "u",
  });
  assert.equal(r.title, "mutants: nothing owed on main, whole-crate sweep 2026-11-01 at main");
  assert.match(r.body, /Mutation score unmeasured,/);
  assert.match(
    r.body,
    /The score is unmeasured this month because:\n- shard 1 of 2 missing\n- 1 listed mutants never ran: a shard cut by its budget/,
  );
  assert.doesNotMatch(r.body, /Each survivor takes/);
  r = call({
    fn: "render",
    sweep: merged,
    owed: { missed: [decodeSurvivor], undecided: [], elsewhere: [], excluded: 0, scaffold: {} },
    head: "abc",
    date: "d",
    run_url: "u",
    dedup_error: "gh exited 1",
  });
  assert.match(r.body, /Open `mutants-debt` issues could not be read \(gh exited 1\)/);
});

test("select prefers the open sweep issue by the bot in either login form and ignores lookalikes", () => {
  const issues = [
    { number: 5, state: "CLOSED", body: "<!-- mutants-sweep -->\nold", author: { login: "github-actions[bot]" } },
    { number: 7, state: "OPEN", body: "whole-crate sweep by hand", author: { login: "app/github-actions" } },
    { number: 9, state: "OPEN", body: "<!-- mutants-sweep -->\ncurrent", author: { login: "app/github-actions" } },
    { number: 11, state: "OPEN", body: "<!-- mutants-sweep -->\nforged", author: { login: "someone" } },
  ];
  assert.deepEqual(call({ fn: "select", issues }), { number: 9, state: "OPEN" });
  assert.deepEqual(call({ fn: "select", issues: issues.slice(0, 2) }), { number: 5, state: "CLOSED" });
  assert.equal(call({ fn: "select", issues: [] }), null);
});

test("decide: owed creates, edits or reopens; nothing owed closes an open issue and leaves the rest", () => {
  const owed = { missed: [decodeSurvivor] };
  const clean = { missed: [], undecided: [] };
  assert.equal(call({ fn: "decide", owed, selected: null }), "create");
  assert.equal(call({ fn: "decide", owed, selected: { number: 9, state: "OPEN" } }), "edit");
  assert.equal(call({ fn: "decide", owed, selected: { number: 5, state: "CLOSED" } }), "reopen");
  assert.equal(call({ fn: "decide", owed: { undecided: [timeoutA] }, selected: null }), "create");
  assert.equal(call({ fn: "decide", owed: clean, selected: { number: 9, state: "OPEN" } }), "close");
  assert.equal(call({ fn: "decide", owed: clean, selected: { number: 5, state: "CLOSED" } }), "nothing");
  assert.equal(call({ fn: "decide", owed: clean, selected: null }), "nothing");
});

test("the script scores a shard directory end to end with --no-issue and writes the ledger entry", () => {
  const dir = mkdtempSync(join(tmpdir(), "mutants-sweep-e2e-"));
  const shards = join(dir, "shards");
  for (const [k, names] of [[0, [caughtA, decodeSurvivor]], [1, [caughtB, scaffoldA]]]) {
    const d = join(shards, `mutants-sweep-shard-${k}`);
    mkdirSync(d, { recursive: true });
    writeFileSync(join(d, "shard.txt"), `${k}/2\n`);
    writeFileSync(join(d, "mutants.json"), JSON.stringify(names.map((name) => ({ name }))));
    writeFileSync(
      join(d, "outcomes.json"),
      JSON.stringify({
        outcomes: names.map((name, i) => ({
          scenario: { Mutant: { name } },
          summary: i === 0 ? "CaughtMutant" : "MissedMutant",
        })),
      }),
    );
  }
  const entry = join(dir, "entry.json");
  const report = join(dir, "report");
  const run = spawnSync(
    `${scriptsDir}/mutants-sweep`,
    [shards, entry, "--report", report, "--no-issue"],
    { encoding: "utf8", env: { ...process.env, GITHUB_RUN_ID: "42", GITHUB_SHA: "deadbeef00" } },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.match(
    run.stdout,
    /^mutants-sweep: score 50\.0% \| caught 2 missed 2 unviable 0 timeout 0 never-ran 0 \| shards 2\/2 \| owed 1 survivor \| issue skipped$/m,
  );
  const written = JSON.parse(spawnSync("cat", [entry], { encoding: "utf8" }).stdout);
  assert.equal(written.role, "mutants");
  assert.equal(written.run_id, "42");
  assert.equal(written.mutation_score, 50.0);
  assert.equal(written.fuzz_cpu_s, null);
  assert.equal(written.coverage_region_pct, null);
  assert.match(written.date, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
  const missed = spawnSync("cat", [join(report, "missed.txt")], { encoding: "utf8" }).stdout;
  assert.equal(missed, `${decodeSurvivor}\n${scaffoldA}\n`);
  const summary = spawnSync("cat", [join(report, "summary.md")], { encoding: "utf8" }).stdout;
  assert.ok(summary.startsWith("The monthly whole-crate sweep at `deadbee`"));
  assert.doesNotMatch(summary, /<!-- mutants-sweep -->/);
});

test("a shards directory with nothing in it still writes a null entry and says so", () => {
  const dir = mkdtempSync(join(tmpdir(), "mutants-sweep-empty-"));
  mkdirSync(join(dir, "shards"));
  const entry = join(dir, "entry.json");
  const run = spawnSync(`${scriptsDir}/mutants-sweep`, [join(dir, "shards"), entry, "--no-issue"], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /^::warning::mutants-sweep: no shard directory$/m);
  assert.match(run.stdout, /score unmeasured \| .* \| shards 0\/\? \| owed nothing \| issue skipped$/m);
  assert.equal(JSON.parse(spawnSync("cat", [entry], { encoding: "utf8" }).stdout).mutation_score, null);
});
