// status-data.py publishes /status-data.json, and since ADR-0047 its
// milestone field reads the tracker: a ROADMAP.md section linking a GitHub
// Milestone takes its status from that milestone's item counts, a ✅ section
// is delivered history, anything else is a program. These pin the join and
// the failure contract with a fake `gh` on PATH, because the real one needs
// a token and a network, and the contract is exactly what happens without
// them: a null field and a problem line, never a guess.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("status-data.py", import.meta.url).pathname;

// Answers `gh api repos/o/r/milestones/N --jq ...` from the number alone:
// 1 is mid-flight, 2 is delivered, anything else unstarted. FAKE_GH=down
// fails every call the way a runner without GH_TOKEN does.
const fakeGh = `#!/bin/sh
if [ "$FAKE_GH" = down ]; then
  echo "gh: To use GitHub CLI in a GitHub Actions workflow, set the GH_TOKEN environment variable." >&2
  exit 4
fi
case "$2" in
  */milestones/1) echo '{"open":2,"closed":3}' ;;
  */milestones/2) echo '{"open":0,"closed":9}' ;;
  */milestones/*) echo '{"open":1,"closed":0}' ;;
  *) echo "fake gh: unexpected $*" >&2; exit 1 ;;
esac
`;

function generate(mode) {
  const dir = mkdtempSync(join(tmpdir(), "status-data-"));
  const gh = join(dir, "gh");
  writeFileSync(gh, fakeGh);
  chmodSync(gh, 0o755);
  const out = join(dir, "status-data.json");
  const run = spawnSync("python3", [script, out], {
    encoding: "utf-8",
    env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, FAKE_GH: mode },
  });
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(readFileSync(out, "utf-8"));
}

// One generation per mode; every other field runs too and costs seconds.
const up = generate("up");
const down = generate("down");
const status = Object.fromEntries((up.milestones ?? []).map((m) => [m.id, m.status]));

test("a section linking a milestone takes the tracker's counts", () => {
  // ROADMAP.md links M6 to milestone 1, M7 to milestone 2, M8 to milestone 3,
  // M5 to milestone 4 (ADR-0074).
  assert.equal(status.M6, "active");
  assert.equal(status.M7, "done");
  assert.equal(status.M8, "pending");
  assert.equal(status.M5, "pending");
  assert.ok(
    !up.problems.some((p) => p.startsWith("milestones:")),
    up.problems.join("\n"),
  );
});

test("✅ is delivered history and a section with neither is a program", () => {
  for (const id of ["M0", "M1", "M2", "M4"]) assert.equal(status[id], "done", id);
  assert.equal(status.M3, "ongoing");
});

test("order is ROADMAP.md's section order, which is rank", () => {
  // The expectation is read from the file, never pinned here: a roadmap PR
  // reorders sections without touching this test, and a pinned list turned
  // main red when #1451 moved M5 among the delivered milestones.
  const roadmap = readFileSync(new URL("../../ROADMAP.md", import.meta.url).pathname, "utf-8");
  const sections = [...roadmap.matchAll(/^## (M\d+)/gm)].map((m) => m[1]);
  assert.ok(sections.length > 1, "ROADMAP.md has milestone sections");
  assert.deepEqual(
    up.milestones.map((m) => m.id),
    sections,
  );
});

test("an unreadable tracker nulls the field and says so, never guesses", () => {
  assert.equal(down.milestones, null);
  // phase derives from milestones and falls with it.
  assert.equal(down.phase, null);
  assert.ok(
    down.problems.some((p) => /^milestones: .*GH_TOKEN/.test(p)),
    down.problems.join("\n"),
  );
});

test("benchmarks publishes bits/byte per case, never the gate's digest objects", () => {
  const baseline = JSON.parse(
    readFileSync(new URL("../../bench/baseline.json", import.meta.url).pathname, "utf-8"),
  );
  const published = up.benchmarks.bits_per_byte;
  assert.deepEqual(Object.keys(published), Object.keys(baseline));
  for (const [name, bpb] of Object.entries(published)) {
    assert.equal(typeof bpb, "number", name);
    assert.equal(bpb, baseline[name].bpb, name);
  }
  assert.ok(!up.problems.some((p) => p.startsWith("benchmarks:")), up.problems.join("\n"));
});

for (const [corpus, file] of [["Canterbury", "canterbury.md"], ["Silesia", "silesia.md"]]) {
  test(`frontier names ${corpus}'s two neighbours as its report's own lines`, () => {
    const report = readFileSync(
      new URL(`../../docs/benchmarks/${file}`, import.meta.url).pathname,
      "utf-8",
    );
    const line = (what) => report.match(new RegExp(`^${what}: (.+)$`, "m"))[1];
    const published = up.frontier.corpora.find((c) => c.corpus === corpus);
    assert.equal(published.source, `docs/benchmarks/${file}`);
    for (
      const [key, what] of [
        ["as_good", "fastest level at least as good"],
        ["as_fast", "best level at least as fast"],
      ]
    ) {
      const level = line(what);
      assert.equal(published[key]?.level ?? "none", level, key);
      if (level !== "none") {
        // The neighbour's figures are its row of the report's table.
        const row = report
          .split("\n")
          .find((l) => l.startsWith(`| ${level} |`))
          .split("|")
          .map((c) => c.trim());
        assert.equal(published[key].bits_per_byte, Number(row[2]), key);
        assert.equal(published[key].encode_mbps, Number(row[3]), key);
      }
    }
    assert.match(published.moved, /^\d{4}-\d{2}-\d{2}$/);
    assert.ok(!up.problems.some((p) => p.startsWith("frontier:")), up.problems.join("\n"));
  });
}

for (const half of ["cli", "lib"]) {
  test(`fit_${half} counts the matrix's holds rows over every row, declined included`, () => {
    const matrix = new URL(`../../tests/fit/${half}.txt`, import.meta.url).pathname;
    const states = readFileSync(matrix, "utf-8")
      .split("\n")
      .filter((line) => line.trim() && !line.startsWith("#"))
      .map((line) => line.split(" | ")[1]);
    if (half === "cli") {
      assert.ok(states.includes("declined"), "the fixture should exercise a declined row");
    }
    assert.equal(up[`fit_${half}`].holds, states.filter((s) => s === "holds").length);
    assert.equal(up[`fit_${half}`].total, states.length);
    assert.ok(!up.problems.some((p) => p.startsWith(`fit_${half}:`)), up.problems.join("\n"));
  });
}
