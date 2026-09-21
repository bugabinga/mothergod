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
  // ROADMAP.md links M6 to milestone 1 and M7 to milestone 2.
  assert.equal(status.M6, "active");
  assert.equal(status.M7, "done");
  assert.ok(
    !up.problems.some((p) => p.startsWith("milestones:")),
    up.problems.join("\n"),
  );
});

test("✅ is delivered history and a section with neither is a program", () => {
  for (const id of ["M0", "M1", "M2", "M4"]) assert.equal(status[id], "done", id);
  for (const id of ["M3", "M5"]) assert.equal(status[id], "ongoing", id);
});

test("order is ROADMAP.md's section order, which is rank", () => {
  assert.deepEqual(
    up.milestones.map((m) => m.id),
    ["M0", "M1", "M2", "M3", "M4", "M7", "M5", "M6"],
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
