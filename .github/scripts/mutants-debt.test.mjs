// Fixtures for mutants-debt: a red that lands after the merge must become an
// issue, and every other state must not. `triage`, `decide`, `render` and
// `in_territory` are pure so both failure directions pin without spawning
// gh: filing on an open PR duplicates the annotations the review already
// reads; not filing a decode-safety survivor on a merged one is the silent
// loss the script exists to end (#676, #677, #690); filing a scaffold
// survivor is the debt ADR-0056 retired; reading a cut run's empty
// missed.txt as clean is the silence #947 ends.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import dataclasses, importlib.machinery, importlib.util, json, sys, tempfile
from pathlib import Path
loader = importlib.machinery.SourceFileLoader("mutants_debt", sys.argv[1] + "/mutants-debt")
spec = importlib.util.spec_from_loader("mutants_debt", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
raw = json.loads(sys.argv[2])
def debt():
    return mod.Debt(
        missed=raw.get("missed", []), outside=raw.get("outside", 0),
        undecided=raw.get("undecided", []), undecided_outside=raw.get("undecided_outside", 0),
        generated=raw.get("generated", 0),
    )
if raw["fn"] == "decide":
    verdict, detail = mod.decide(debt(), raw["merged"], raw["filed"])
    print(json.dumps({"verdict": verdict, "detail": detail}))
elif raw["fn"] == "territory":
    print(json.dumps([mod.in_territory(line) for line in raw["lines"]]))
elif raw["fn"] == "triage":
    print(json.dumps(dataclasses.asdict(mod.triage(raw["missed"], raw["undecided"], raw["generated"]))))
elif raw["fn"] == "read_report":
    with tempfile.TemporaryDirectory() as out:
        for name, content in raw["files"].items():
            (Path(out) / name).write_text(content)
        missed, undecided, generated = mod.read_report(out)
    print(json.dumps({"missed": missed, "undecided": undecided, "generated": generated}))
else:
    title, body = mod.render(raw["pr"], debt(), raw["run_url"])
    print(json.dumps({"title": title, "body": body}))
`;

function call(payload) {
  const run = spawnSync("python3", ["-c", driver, scriptsDir, JSON.stringify(payload)], {
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

// Decode-safety territory: filed after a merge (ADR-0056).
const survivors = [
  "src/codec.rs:555:5: replace ensure_room -> Result<(), Error> with Ok(())",
  "src/coder.rs:303:26: replace & with | in Decoder<'a>::decode",
];
// Scaffold territory: a map reading, never filed.
const scaffold = [
  "src/literal.rs:952:55: replace + with - in Literal::mix_ppm",
  "src/codec.rs:608:9: replace <impl TokenSink for PpmExpertCostSink<'_>>::flag with ()",
];
// The two mutants run 38191593517 listed and never ran (#947), both decode territory.
const undecided = [
  "src/codec.rs:1062:28: replace > with < in read_header",
  "src/codec.rs:1062:28: replace > with == in read_header",
];

test("territory is read off the function the line names, cheap direction is under-filing", () => {
  const lines = [
    ...survivors,
    "src/lib.rs:332:1: replace check_stored_bound -> Result<(), Error> with Ok(())",
    "src/codec.rs:418:1: replace undo_filter -> Result<Vec<u8>, Error> with Ok(vec![])",
    "src/codec.rs:897:1: replace read_u32_le -> Result<u32, Error> with Ok(0)",
    ...scaffold,
    "src/lz.rs:1200:9: replace < with <= in dp_round",
    // Proven override (#854 review): bare substring match on "bound" would
    // otherwise file SSE's logit-clamp constant as decode-safety.
    "src/sse.rs:315:1: replace stretch_bound -> f64 with 1.0",
  ];
  const out = call({ fn: "territory", lines });
  assert.deepEqual(out, [true, true, true, true, true, false, false, false, false]);
});

test("triage sorts survivors and never-run mutants by the one territory rule", () => {
  const out = call({
    fn: "triage",
    missed: [...survivors, ...scaffold],
    undecided: [...undecided, scaffold[0]],
    generated: 74,
  });
  assert.deepEqual(out, {
    missed: survivors,
    outside: 2,
    undecided,
    undecided_outside: 1,
    generated: 74,
  });
});

test("read_report names the listed mutants no outcome decided; an absent listing is zero, not an error", () => {
  const mutant = (name) => ({ name, package: "mothergod", file: name.split(":")[0] });
  const outcome = (name, summary) => ({ scenario: { Mutant: mutant(name) }, summary });
  const out = call({
    fn: "read_report",
    files: {
      "missed.txt": `${survivors[0]}\n`,
      "mutants.json": JSON.stringify([...undecided, ...survivors].map(mutant)),
      "outcomes.json": JSON.stringify({
        total_mutants: 2,
        outcomes: [
          { scenario: "Baseline", summary: "Success" },
          outcome(survivors[0], "MissedMutant"),
          outcome(survivors[1], "CaughtMutant"),
        ],
      }),
    },
  });
  assert.deepEqual(out, { missed: [survivors[0]], undecided, generated: 4 });

  // A diff with no src line leaves no mutants.out at all (run 37728310524).
  assert.deepEqual(call({ fn: "read_report", files: {} }), { missed: [], undecided: [], generated: 0 });
});

test("scaffold-only survivors are not filed, whatever the PR state", () => {
  for (const merged of [true, false, null]) {
    const out = call({ fn: "decide", missed: [], outside: scaffold.length, merged, filed: null });
    assert.equal(out.verdict, "scaffold");
    assert.match(out.detail, /2 survivor/);
  }
});

test("no survivors files nothing, whatever the PR state", () => {
  for (const merged of [true, false, null]) {
    assert.equal(call({ fn: "decide", missed: [], merged, filed: null }).verdict, "no survivors");
  }
});

test("never-run mutants outside decode-safety territory change no verdict", () => {
  const out = call({ fn: "decide", missed: [], undecided_outside: 3, generated: 10, merged: true, filed: null });
  assert.equal(out.verdict, "no survivors");
});

test("survivors on an open PR stay on the diff", () => {
  const out = call({ fn: "decide", missed: survivors, merged: false, filed: null });
  assert.equal(out.verdict, "open");
  assert.match(out.detail, /2 survivor/);
});

test("survivors on a merged PR are filed once", () => {
  assert.equal(call({ fn: "decide", missed: survivors, merged: true, filed: null }).verdict, "file");
  const dup = call({ fn: "decide", missed: survivors, merged: true, filed: 700 });
  assert.equal(dup.verdict, "filed already");
  assert.match(dup.detail, /#700/);
});

test("a cut run's decode-safety mutants are debt in the same sense as survivors (#947)", () => {
  const cut = { fn: "decide", missed: [], undecided, generated: 74, filed: null };
  assert.equal(call({ ...cut, merged: true }).verdict, "file");
  assert.match(call({ ...cut, merged: true }).detail, /2 undecided on merged code/);
  assert.equal(call({ ...cut, merged: false }).verdict, "open");
  assert.equal(call({ ...cut, merged: null }).verdict, "unreadable");
  const both = call({ ...cut, missed: survivors, merged: true });
  assert.match(both.detail, /2 survivors, 2 undecided/);
});

test("an unreadable PR state is its own verdict, never silently open", () => {
  assert.equal(call({ fn: "decide", missed: survivors, merged: null, filed: null }).verdict, "unreadable");
});

test("render names the PR, the files, and every survivor; partial runs say so", () => {
  const full = call({
    fn: "render",
    pr: 690,
    missed: survivors,
    generated: 104,
    run_url: "https://x/run/1",
  });
  assert.equal(full.title, "mutants: 2 survivors outlived PR #690 (src/codec.rs, src/coder.rs)");
  for (const line of survivors) assert.ok(full.body.includes(line), line);
  assert.ok(full.body.includes("https://x/run/1"));
  assert.ok(!full.body.includes("never ran"), "a complete run must not claim undecided mutants");
  assert.ok(!full.body.includes("outside decode-safety"), "no scaffold survivors, no scaffold line");

  const mixed = call({
    fn: "render",
    pr: 690,
    missed: survivors,
    outside: scaffold.length,
    generated: 104,
    run_url: "u",
  });
  assert.equal(mixed.title, "mutants: 2 survivors outlived PR #690 (src/codec.rs, src/coder.rs)");
  assert.ok(mixed.body.includes("2 more survivors sit outside decode-safety territory"));
  for (const line of scaffold) assert.ok(!mixed.body.includes(line), "scaffold lines stay off the issue");

  const partial = call({
    fn: "render",
    pr: 690,
    missed: survivors,
    undecided,
    undecided_outside: 18,
    generated: 104,
    run_url: "u",
  });
  assert.equal(partial.title, "mutants: 2 survivors, 2 undecided outlived PR #690 (src/codec.rs, src/coder.rs)");
  assert.ok(partial.body.includes("20 of\n104 listed never ran"));
  for (const line of undecided) assert.ok(partial.body.includes(line), line);
  assert.ok(partial.body.includes("An undecided mutant is run first"), "the body says how to decide one");
  assert.ok(
    partial.body.includes(
      "cargo mutants --config tests/mutants.toml --package mothergod -f src/codec.rs --re 'replace > with == in read_header'",
    ),
    "one pasteable run per mutant, the line left off",
  );

  // Run 38191593517's shape: nothing missed, two never run, and it merged.
  const cutOnly = call({ fn: "render", pr: 1085, undecided, generated: 74, run_url: "u" });
  assert.equal(cutOnly.title, "mutants: 2 undecided outlived PR #1085 (src/codec.rs)");
  assert.ok(!cutOnly.body.includes("Survivors, each"), "no survivors, no survivor block");

  const one = call({ fn: "render", pr: 5, missed: [survivors[0]], generated: 1, run_url: "u" });
  assert.equal(one.title, "mutants: 1 survivor outlived PR #5 (src/codec.rs)");
  assert.ok(!one.body.includes("never ran"), "a complete run must not claim undecided mutants");

  const oneOutside = call({
    fn: "render",
    pr: 5,
    missed: [survivors[0]],
    outside: 1,
    generated: 2,
    run_url: "u",
  });
  assert.ok(
    oneOutside.body.includes("1 more survivor sits outside decode-safety territory"),
    "singular agrees (#932 read \"1 more survivor sit\")",
  );
});

test("render's per-mutant run is a regex of the text, not the name (#1121)", () => {
  // `||` alternates: the bare name matched all 2157 mutants of the crate on
  // cargo-mutants 27.1.0. The escaped text with `-f` listed exactly the one.
  // A lifetime's quote has to survive the shell, hence the shlex form.
  const meta = [
    "src/bin/mothergod.rs:209:43: replace || with && in run",
    "src/lib.rs:326:37: replace + with - in Frames<'a>::split",
  ];
  const out = call({ fn: "render", pr: 7, missed: [meta[1]], undecided: [meta[0]], generated: 9, run_url: "u" });
  assert.ok(!out.body.includes("<its name>"), "no placeholder handed out as a recipe");
  const prefix = "cargo mutants --config tests/mutants.toml --package mothergod ";
  assert.ok(out.body.includes(`${prefix}-f src/bin/mothergod.rs --re 'replace \\|\\| with \\&\\& in run'`), out.body);
  assert.ok(
    out.body.includes(`${prefix}-f src/lib.rs --re 'replace \\+ with \\- in Frames<'"'"'a>::split'`),
    out.body,
  );
  assert.ok(!out.body.includes("209:43: replace || with && in run'"), "the line is off the pattern");
});

test("render keeps line and column only for a mutant that names no function (#1121 review)", () => {
  // A const's mutant has no `in <fn>`: without the line its text listed 50
  // mutants of src/literal.rs. The line is exact as of the filing.
  const constant = "src/literal.rs:69:36: replace + with -";
  const out = call({ fn: "render", pr: 7, missed: [constant], generated: 1, run_url: "u" });
  const prefix = "cargo mutants --config tests/mutants.toml --package mothergod ";
  assert.ok(out.body.includes(`${prefix}-f src/literal.rs --re ':69:36: replace \\+ with \\-'`), out.body);
  assert.ok(out.body.includes("a harmless extra"), "the body says what the unpinned run also lists");
  assert.ok(out.body.includes("keeps its line and column"), "the body says why this one keeps its line");
});

test("render's per-mutant run falls back to an unscoped escaped pattern for a name with no location", () => {
  const out = call({ fn: "render", pr: 7, missed: ["replace || with && in run"], generated: 1, run_url: "u" });
  const prefix = "cargo mutants --config tests/mutants.toml --package mothergod ";
  assert.ok(out.body.includes(`${prefix}--re 'replace \\|\\| with \\&\\& in run'`), out.body);
});
